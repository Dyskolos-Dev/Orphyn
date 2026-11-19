//! Google Cast transport (Default Media Receiver) over `rust_cast`.
//!
//! `rust_cast` is blocking and not `Send` (without its `thread_safe` feature), so each connection
//! lives on its own worker thread: commands arrive on a channel and are answered through oneshot
//! replies, and the same thread polls the receiver once a second (which also keeps the
//! heartbeat alive). A panic or socket error on that thread surfaces as `on_closed`, never as a
//! crash.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rust_cast::channels::media::{
    IdleReason, Image, Media, Metadata, MovieMediaMetadata, MusicTrackMediaMetadata, PlayerState,
    StatusEntry, StreamType,
};
use rust_cast::channels::receiver::{CastDeviceApp, Volume};
use rust_cast::CastDevice as RcDevice;
use tokio::sync::oneshot;

use super::model::{CastMedia, CastMediaKind, CastStatus};
use super::transport::{BoxFut, CastTransport, TResult, TransportHandlers, TransportUpdate};

const RECEIVER: &str = "receiver-0";
const MAX_POLL_FAILURES: u32 = 3;

/// Builds the Cast media object (Music/Movie metadata, images) for the Default Media Receiver.
pub fn build_cast_media(media: &CastMedia) -> Media {
    let images: Vec<Image> = media
        .artwork_url
        .iter()
        .filter(|u| !u.is_empty())
        .map(|u| Image::new(u.clone()))
        .collect();
    let metadata = match media.kind {
        CastMediaKind::Video => Metadata::Movie(MovieMediaMetadata {
            title: Some(media.title.clone()),
            images,
            ..Default::default()
        }),
        CastMediaKind::Audio => Metadata::MusicTrack(MusicTrackMediaMetadata {
            title: Some(media.title.clone()),
            artist: media.artist.clone().filter(|a| !a.is_empty()),
            album_name: media.album.clone().filter(|a| !a.is_empty()),
            images,
            ..Default::default()
        }),
    };
    Media {
        content_id: media.url.clone(),
        stream_type: StreamType::Buffered,
        content_type: media.mime_type.clone(),
        metadata: Some(metadata),
        duration: media.duration_sec.filter(|d| *d > 0.0).map(|d| d as f32),
    }
}

/// The few fields of a media status the session needs.
#[derive(Debug, Clone)]
pub struct PlayerStatus {
    pub player_state: PlayerState,
    pub idle_reason: Option<IdleReason>,
    pub current_time: Option<f64>,
    pub duration: Option<f64>,
}

impl PlayerStatus {
    pub fn from_entry(e: &StatusEntry) -> Self {
        Self {
            player_state: e.player_state,
            idle_reason: e.idle_reason,
            current_time: e.current_time.map(f64::from),
            duration: e.media.as_ref().and_then(|m| m.duration).map(f64::from),
        }
    }
}

pub fn map_cast_status(s: &PlayerStatus) -> TransportUpdate {
    let mut update = TransportUpdate {
        position_sec: s.current_time,
        duration_sec: s.duration,
        ..Default::default()
    };
    match s.player_state {
        PlayerState::Playing => update.status = Some(CastStatus::Playing),
        PlayerState::Paused => update.status = Some(CastStatus::Paused),
        PlayerState::Buffering => update.status = Some(CastStatus::Buffering),
        PlayerState::Idle => match s.idle_reason {
            Some(IdleReason::Finished) => update.ended = true,
            Some(IdleReason::Error) => {
                update.error = Some("The device could not play this media".into());
            }
            _ => update.status = Some(CastStatus::Idle),
        },
    }
    update
}

type Reply = oneshot::Sender<TResult<()>>;

enum Msg {
    Load(CastMedia, Reply),
    Play(Reply),
    Pause(Reply),
    Seek(f64, Reply),
    Stop(Reply),
    Volume(f64, Reply),
    Muted(bool, Reply),
    Close,
}

type SharedHandlers = Arc<Mutex<Option<TransportHandlers>>>;

pub struct ChromecastTransport {
    host: String,
    port: u16,
    handlers: SharedHandlers,
    tx: Mutex<Option<Sender<Msg>>>,
}

impl ChromecastTransport {
    pub fn new(host: &str, port: u16) -> Arc<Self> {
        Arc::new(Self {
            host: host.to_string(),
            port,
            handlers: Arc::default(),
            tx: Mutex::new(None),
        })
    }

    fn tx(&self) -> MutexGuard<'_, Option<Sender<Msg>>> {
        self.tx.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn send(&self, make: impl FnOnce(Reply) -> Msg) -> TResult<()> {
        let (reply, rx) = oneshot::channel();
        {
            let guard = self.tx();
            let Some(tx) = guard.as_ref() else {
                return Err("Not connected".into());
            };
            tx.send(make(reply))
                .map_err(|_| "Connection closed".to_string())?;
        }
        rx.await.map_err(|_| "Connection closed".to_string())?
    }
}

impl CastTransport for ChromecastTransport {
    fn set_handlers(&self, handlers: TransportHandlers) {
        *self.handlers.lock().unwrap_or_else(|p| p.into_inner()) = Some(handlers);
    }

    fn connect(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            // Replace any previous worker.
            if let Some(old) = self.tx().take() {
                let _ = old.send(Msg::Close);
            }
            let (tx, rx) = channel::<Msg>();
            let (ready_tx, ready_rx) = oneshot::channel::<TResult<()>>();
            let (host, port, handlers) = (self.host.clone(), self.port, self.handlers.clone());
            std::thread::Builder::new()
                .name(format!("cast-{host}"))
                .spawn(move || worker(host, port, rx, ready_tx, handlers))
                .map_err(|e| format!("Could not start the Cast worker: {e}"))?;
            ready_rx
                .await
                .map_err(|_| "Cast worker stopped".to_string())??;
            *self.tx() = Some(tx);
            Ok(())
        })
    }

    fn load(&self, media: CastMedia) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(|r| Msg::Load(media, r)).await })
    }
    fn play(&self) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(Msg::Play).await })
    }
    fn pause(&self) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(Msg::Pause).await })
    }
    fn seek(&self, position_sec: f64) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(|r| Msg::Seek(position_sec, r)).await })
    }
    fn stop(&self) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(Msg::Stop).await })
    }
    fn set_volume(&self, volume: f64) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(|r| Msg::Volume(volume, r)).await })
    }
    fn set_muted(&self, muted: bool) -> BoxFut<'_, ()> {
        Box::pin(async move { self.send(|r| Msg::Muted(muted, r)).await })
    }

    fn close(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            *self.handlers.lock().unwrap_or_else(|p| p.into_inner()) = None;
            if let Some(tx) = self.tx().take() {
                let _ = tx.send(Msg::Close);
            }
            Ok(())
        })
    }
}

fn err_text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

struct Link {
    device: RcDevice<'static>,
    transport_id: String,
    session_id: String,
    media_session_id: Option<i32>,
}

fn open(host: &str, port: u16) -> TResult<Link> {
    // Cast receivers use self-signed certificates, so the host is not verified (same as the
    // Electron implementation).
    let device =
        RcDevice::connect_without_host_verification(host.to_string(), port).map_err(err_text)?;
    device.connection.connect(RECEIVER).map_err(err_text)?;
    device.heartbeat.ping().map_err(err_text)?;
    let app = device
        .receiver
        .launch_app(&CastDeviceApp::DefaultMediaReceiver)
        .map_err(err_text)?;
    device
        .connection
        .connect(app.transport_id.as_str())
        .map_err(err_text)?;
    Ok(Link {
        device,
        transport_id: app.transport_id,
        session_id: app.session_id,
        media_session_id: None,
    })
}

fn emit(handlers: &SharedHandlers, f: impl FnOnce(&TransportHandlers)) {
    let h = handlers.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(h) = h {
        f(&h);
    }
}

fn worker(
    host: String,
    port: u16,
    rx: std::sync::mpsc::Receiver<Msg>,
    ready: oneshot::Sender<TResult<()>>,
    handlers: SharedHandlers,
) {
    let opened = catch_unwind(AssertUnwindSafe(|| open(&host, port)))
        .unwrap_or_else(|_| Err("The Cast library failed while connecting".to_string()));
    let mut link = match opened {
        Ok(l) => {
            let _ = ready.send(Ok(()));
            l
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| run(&mut link, &rx, &handlers, &host)));
    match outcome {
        Ok(None) => {} // closed on purpose
        Ok(Some(reason)) => emit(&handlers, |h| (h.on_closed)(Some(reason))),
        Err(_) => emit(&handlers, |h| {
            (h.on_closed)(Some("The Cast library failed".into()))
        }),
    }
}

/// Runs until closed. Returns `Some(reason)` when the link died.
fn run(
    link: &mut Link,
    rx: &std::sync::mpsc::Receiver<Msg>,
    handlers: &SharedHandlers,
    host: &str,
) -> Option<String> {
    let mut failures = 0u32;
    let mut tick = 0u64;
    loop {
        match rx.recv_timeout(Duration::from_millis(1000)) {
            Ok(Msg::Close) | Err(RecvTimeoutError::Disconnected) => return None,
            Ok(msg) => handle(link, msg),
            Err(RecvTimeoutError::Timeout) => {
                tick += 1;
                match poll(link, handlers, tick) {
                    Ok(()) => failures = 0,
                    Err(e) => {
                        failures += 1;
                        log::warn!(
                            "[cast] cast poll failed ({host}, {failures}/{MAX_POLL_FAILURES}): {e}"
                        );
                        if failures >= MAX_POLL_FAILURES {
                            return Some(e);
                        }
                    }
                }
            }
        }
    }
}

fn require_session(link: &Link) -> TResult<i32> {
    link.media_session_id
        .ok_or_else(|| "Nothing is loaded on the device".to_string())
}

fn handle(link: &mut Link, msg: Msg) {
    match msg {
        Msg::Load(media, reply) => {
            let opts = rust_cast::channels::media::LoadOptions {
                current_time: media.start_position_sec.unwrap_or(0.0).max(0.0),
                autoplay: true,
            };
            let r = link
                .device
                .media
                .load_with_opts(
                    link.transport_id.as_str(),
                    link.session_id.as_str(),
                    &build_cast_media(&media),
                    opts,
                )
                .map_err(err_text)
                .map(|status| {
                    link.media_session_id = status.entries.first().map(|e| e.media_session_id);
                });
            let _ = reply.send(r);
        }
        Msg::Play(reply) => {
            let r = require_session(link).and_then(|id| {
                link.device
                    .media
                    .play(link.transport_id.as_str(), id)
                    .map(|_| ())
                    .map_err(err_text)
            });
            let _ = reply.send(r);
        }
        Msg::Pause(reply) => {
            let r = require_session(link).and_then(|id| {
                link.device
                    .media
                    .pause(link.transport_id.as_str(), id)
                    .map(|_| ())
                    .map_err(err_text)
            });
            let _ = reply.send(r);
        }
        Msg::Seek(pos, reply) => {
            let r = require_session(link).and_then(|id| {
                link.device
                    .media
                    .seek(link.transport_id.as_str(), id, Some(pos as f32), None)
                    .map(|_| ())
                    .map_err(err_text)
            });
            let _ = reply.send(r);
        }
        Msg::Stop(reply) => {
            let r = require_session(link).and_then(|id| {
                link.device
                    .media
                    .stop(link.transport_id.as_str(), id)
                    .map(|_| ())
                    .map_err(err_text)
            });
            link.media_session_id = None;
            let _ = reply.send(r);
        }
        Msg::Volume(v, reply) => {
            let r = link
                .device
                .receiver
                .set_volume(Volume {
                    level: Some(v as f32),
                    muted: None,
                })
                .map(|_| ())
                .map_err(err_text);
            let _ = reply.send(r);
        }
        Msg::Muted(m, reply) => {
            let r = link
                .device
                .receiver
                .set_volume(Volume {
                    level: None,
                    muted: Some(m),
                })
                .map(|_| ())
                .map_err(err_text);
            let _ = reply.send(r);
        }
        Msg::Close => {}
    }
}

/// One poll cycle: keep the heartbeat alive, read the media status (and the volume every 5th).
fn poll(link: &mut Link, handlers: &SharedHandlers, tick: u64) -> TResult<()> {
    // Unsolicited pong answers the receiver's own pings, which this thread would otherwise only
    // buffer.
    let _ = link.device.heartbeat.pong();
    link.device.heartbeat.ping().map_err(err_text)?;
    if link.media_session_id.is_some() {
        let status = link
            .device
            .media
            .get_status(link.transport_id.as_str(), link.media_session_id)
            .map_err(err_text)?;
        if let Some(entry) = status.entries.first() {
            let update = map_cast_status(&PlayerStatus::from_entry(entry));
            if update.ended {
                // Receiver idle after FINISHED: nothing more to poll until the next load.
                link.media_session_id = None;
            }
            emit(handlers, |h| (h.on_update)(update));
        }
    }
    if tick.is_multiple_of(5) {
        let status = link.device.receiver.get_status().map_err(err_text)?;
        let update = TransportUpdate {
            volume: status.volume.level.map(|l| f64::from(l).clamp(0.0, 1.0)),
            muted: status.volume.muted,
            ..Default::default()
        };
        emit(handlers, |h| (h.on_update)(update));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cast::didl::tests::rich_media;

    #[test]
    fn uses_music_track_metadata_for_audio_and_movie_metadata_for_video() {
        let media = rich_media();
        let a = build_cast_media(&media);
        assert_eq!(a.content_type, "audio/flac");
        assert_eq!(a.content_id, media.url);
        assert_eq!(a.duration, Some(3725.0));
        match a.metadata.unwrap() {
            Metadata::MusicTrack(m) => {
                assert_eq!(m.title.as_deref(), Some(media.title.as_str()));
                assert_eq!(m.artist.as_deref(), Some("Guns N' Roses"));
                assert_eq!(m.album_name.as_deref(), Some("A > B"));
                assert_eq!(
                    m.images,
                    vec![Image::new(media.artwork_url.clone().unwrap())]
                );
            }
            other => panic!("expected music metadata, got {other:?}"),
        }
        let v = build_cast_media(&CastMedia {
            kind: CastMediaKind::Video,
            ..media
        });
        assert!(matches!(v.metadata, Some(Metadata::Movie(_))));
    }

    fn st(state: PlayerState, reason: Option<IdleReason>) -> PlayerStatus {
        PlayerStatus {
            player_state: state,
            idle_reason: reason,
            current_time: None,
            duration: None,
        }
    }

    #[test]
    fn maps_player_states_finish_and_error() {
        let playing = map_cast_status(&PlayerStatus {
            player_state: PlayerState::Playing,
            idle_reason: None,
            current_time: Some(4.0),
            duration: Some(9.0),
        });
        assert_eq!(
            playing,
            TransportUpdate {
                status: Some(CastStatus::Playing),
                position_sec: Some(4.0),
                duration_sec: Some(9.0),
                ..Default::default()
            }
        );
        assert!(map_cast_status(&st(PlayerState::Idle, Some(IdleReason::Finished))).ended);
        assert!(
            map_cast_status(&st(PlayerState::Idle, Some(IdleReason::Error)))
                .error
                .is_some()
        );
        assert_eq!(
            map_cast_status(&st(PlayerState::Idle, Some(IdleReason::Cancelled))).status,
            Some(CastStatus::Idle)
        );
        assert_eq!(
            map_cast_status(&st(PlayerState::Buffering, None)).status,
            Some(CastStatus::Buffering)
        );
    }

    #[tokio::test]
    async fn commands_fail_cleanly_before_connect() {
        let t = ChromecastTransport::new("127.0.0.1", 1);
        assert_eq!(t.play().await.unwrap_err(), "Not connected");
        t.close().await.unwrap();
    }
}
