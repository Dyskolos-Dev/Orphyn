//! `RemoteAudioOutput.ts` + `musicCast.ts`: the music engine plays on a cast device. The engine
//! keeps the queue, the state and the Jellyfin reporting; this output turns its calls into cast
//! commands and the device's session events (position, state, "ended") back into engine events.
use std::sync::{Arc, Mutex};

use orphyn_core::cast::{CastEvent, CastMedia, CastMediaKind, CastSessionState, CastStatus};
use orphyn_jellyfin::endpoints::Session;
use orphyn_player::music::{MusicPlayer, RemoteEvent, RemoteOutput, Track};

use super::hub::{hub, CastHub};
use crate::features::music::{track_art, track_artist};
use crate::preferences::AudioQuality;

#[derive(Default)]
struct Inner {
    /// URL of what was asked for; session state about other media is stale.
    url: Option<String>,
    loaded: bool,
    last_status: Option<CastStatus>,
    gen: u64,
}

pub struct MusicRemote {
    device_id: String,
    hub: Arc<CastHub>,
    player: MusicPlayer,
    session: Session,
    quality: AudioQuality,
    inner: Mutex<Inner>,
    handler: Mutex<Option<u64>>,
}

/// Device-reachable stream URL, metadata and artwork of `track` (`describeCurrentTrackForCast`).
pub fn describe_track(session: &Session, quality: AudioQuality, track: &Track) -> CastMedia {
    let url = match quality {
        AudioQuality::Bitrate(b) => session.audio_transcode_url(&track.id, b as u32),
        // Receivers cope badly with raw lossless files: cap the original at a high MP3 rate.
        AudioQuality::Original => session.audio_transcode_url(&track.id, 320_000),
    };
    let artist = track_artist(track);
    CastMedia {
        url,
        mime_type: "audio/mpeg".into(),
        kind: CastMediaKind::Audio,
        title: track.name().to_string(),
        artist: (!artist.is_empty()).then_some(artist),
        album: track.album.clone(),
        artwork_url: track_art(session.client.base_url(), track, 640),
        duration_sec: Some(track.duration_seconds()).filter(|d| *d > 0.0),
        start_position_sec: None,
    }
}

impl MusicRemote {
    /// Creates the output and starts listening to the device's session; the caller hands it to
    /// `MusicPlayer::set_remote`.
    pub fn new(
        device_id: String,
        player: MusicPlayer,
        session: Session,
        quality: AudioQuality,
    ) -> Arc<Self> {
        let hub = hub();
        let remote = Arc::new(Self {
            device_id,
            hub: hub.clone(),
            player,
            session,
            quality,
            inner: Mutex::new(Inner::default()),
            handler: Mutex::new(None),
        });
        let weak = Arc::downgrade(&remote);
        let id = hub.add_handler(Arc::new(move |event| {
            if let Some(r) = weak.upgrade() {
                r.on_event(event);
            }
        }));
        *remote.handler.lock().unwrap_or_else(|p| p.into_inner()) = Some(id);
        remote
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Stops listening (the device is released by the caller).
    pub fn dispose(&self) {
        if let Some(id) = self.handler.lock().unwrap_or_else(|p| p.into_inner()).take() {
            self.hub.remove_handler(id);
        }
        self.state().gen += 1;
    }

    fn state(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn fail(&self, message: String) {
        self.hub.fail(&self.device_id, message);
    }

    fn on_event(&self, event: &CastEvent) {
        match event {
            CastEvent::SessionChanged(s) if s.device_id == self.device_id => self.on_session(s),
            CastEvent::Ended(e) if e.device_id == self.device_id => {
                let url = self.state().url.clone();
                let Some(url) = url else { return };
                if e.media.as_ref().is_some_and(|m| m.url != url) {
                    return; // a previous track
                }
                self.state().loaded = false;
                self.player.remote_event(RemoteEvent::Ended);
            }
            _ => {}
        }
    }

    fn on_session(&self, s: &CastSessionState) {
        let (first, changed) = {
            let mut i = self.state();
            let Some(url) = i.url.clone() else { return };
            // Ignore state still describing the previous media while a load is in flight.
            if s.media.as_ref().is_some_and(|m| m.url != url) || s.status == CastStatus::Idle {
                return;
            }
            if s.status == CastStatus::Error {
                drop(i);
                self.fail(s.error.clone().unwrap_or_else(|| "Cast error".into()));
                return;
            }
            let active = matches!(s.status, CastStatus::Playing | CastStatus::Paused);
            let first = active && !i.loaded;
            if first {
                i.loaded = true;
            }
            let changed = i.last_status != Some(s.status);
            i.last_status = Some(s.status);
            (first.then_some(()), changed.then_some(s.status))
        };
        let active = matches!(s.status, CastStatus::Playing | CastStatus::Paused);
        if first.is_some() {
            self.player.remote_event(RemoteEvent::Loaded {
                duration: Some(s.duration_sec).filter(|d| *d > 0.0),
            });
        }
        if active {
            self.player.remote_event(RemoteEvent::Position(s.position_sec));
        }
        match changed {
            Some(CastStatus::Playing) => self.player.remote_event(RemoteEvent::Playing(true)),
            Some(CastStatus::Paused) => self.player.remote_event(RemoteEvent::Playing(false)),
            _ => {}
        }
    }
}

impl RemoteOutput for MusicRemote {
    fn load(&self, track: &Track, start_at: f64, autoplay: bool) {
        let mut media = describe_track(&self.session, self.quality, track);
        media.start_position_sec = Some(start_at);
        {
            let mut i = self.state();
            i.gen += 1;
            i.url = Some(media.url.clone());
            i.loaded = false;
            i.last_status = None;
        }
        let (hub, device) = (self.hub.clone(), self.device_id.clone());
        hub.clone().service.load(&device.clone(), media, move |result| match result {
            Err(e) => hub.fail(&device, e),
            // The receiver starts playing on load: pause it when the player did not want that.
            Ok(()) if !autoplay => hub.service.pause(&device),
            Ok(()) => {}
        });
    }

    fn play(&self) {
        self.hub.service.play(&self.device_id);
    }
    fn pause(&self) {
        self.hub.service.pause(&self.device_id);
    }
    fn seek(&self, seconds: f64) {
        self.hub.service.seek(&self.device_id, seconds.max(0.0));
    }
    fn set_volume(&self, volume: f32) {
        // Some receivers (AirPlay) have no remote volume: not an error worth showing.
        self.hub.service.set_volume(&self.device_id, f64::from(volume));
    }
    fn stop(&self) {
        self.hub.service.stop(&self.device_id);
    }
}
