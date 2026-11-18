//! Video playback controller with the API of `features/video/controller.ts`
//! (`PlaybackController` / `PlaybackSnapshot`), on top of libmpv. Screens subscribe to the
//! snapshot and draw the frames with [`super::VideoWidget`].
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use super::mpv::{Mpv, MpvState, MpvTrack};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Ended,
    Error,
}

/// `PlaybackSnapshot` of `controller.ts`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSnapshot {
    pub item_id: Option<String>,
    pub status: PlaybackStatus,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub buffered_seconds: f64,
    pub volume: f64,
    pub muted: bool,
    pub rate: f64,
    /// Decoder in use (`d3d11va-copy`, `vaapi-copy`, empty = software).
    pub hwdec: String,
    pub error: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            item_id: None,
            status: PlaybackStatus::Idle,
            position_seconds: 0.0,
            duration_seconds: 0.0,
            buffered_seconds: 0.0,
            volume: 1.0,
            muted: false,
            rate: 1.0,
            hwdec: String::new(),
            error: None,
        }
    }
}

/// `PlayOptions` of `controller.ts` (the stream URL is resolved by the caller).
#[derive(Clone, Debug, Default)]
pub struct PlayOptions {
    pub item_id: String,
    pub url: String,
    pub start_seconds: f64,
    /// More per-file mpv options, comma separated (`aid=2,sid=no`).
    pub options: String,
}

/// One video player. libmpv is loaded on the first `play`; when it is missing the snapshot goes
/// to `Error` with the reason.
#[derive(Clone)]
pub struct VideoController {
    mpv: Arc<Mutex<Option<Arc<Mpv>>>>,
    tx: Arc<watch::Sender<PlaybackSnapshot>>,
    /// `loadfile` commands sent: states of an older file are ignored.
    issued: Arc<AtomicU64>,
}

impl Default for VideoController {
    fn default() -> Self {
        Self::new()
    }
}

fn status_of(s: &MpvState, previous: PlaybackStatus) -> PlaybackStatus {
    if s.error.is_some() {
        PlaybackStatus::Error
    } else if s.ended {
        PlaybackStatus::Ended
    } else if s.loaded && s.buffering && !s.paused {
        // `waiting` of the <video> element.
        PlaybackStatus::Loading
    } else if !s.loaded {
        if previous == PlaybackStatus::Idle {
            PlaybackStatus::Idle
        } else {
            PlaybackStatus::Loading
        }
    } else if s.paused {
        PlaybackStatus::Paused
    } else {
        PlaybackStatus::Playing
    }
}

impl VideoController {
    pub fn new() -> Self {
        Self {
            mpv: Arc::default(),
            tx: Arc::new(watch::channel(PlaybackSnapshot::default()).0),
            issued: Arc::default(),
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<PlaybackSnapshot> {
        self.tx.subscribe()
    }

    /// The running mpv instance (for the widget), starting it if needed.
    pub fn mpv(&self) -> Result<Arc<Mpv>, String> {
        let mut slot = self.mpv.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(m) = slot.as_ref() {
            return Ok(m.clone());
        }
        let tx = self.tx.clone();
        let issued = self.issued.clone();
        let m = Arc::new(Mpv::start(Arc::new(move |s: MpvState| {
            tx.send_modify(|snap| {
                // Stopped (no item): late reports of the old file change nothing.
                if snap.item_id.is_none() || s.loads < issued.load(Relaxed) {
                    return;
                }
                snap.status = status_of(&s, snap.status);
                snap.error = s.error.clone();
                snap.position_seconds = s.position;
                snap.duration_seconds = s.duration;
                snap.buffered_seconds = s.buffered;
                snap.hwdec = s.hwdec.clone();
            });
        }))?);
        *slot = Some(m.clone());
        Ok(m)
    }

    pub fn play(&self, opts: PlayOptions) {
        match self.mpv() {
            Ok(m) => {
                self.tx.send_modify(|s| {
                    s.item_id = Some(opts.item_id.clone());
                    s.status = PlaybackStatus::Loading;
                    s.position_seconds = opts.start_seconds;
                    s.error = None;
                });
                self.issued.fetch_add(1, Relaxed);
                let mut start = format!("start={}", opts.start_seconds.max(0.0));
                if !opts.options.is_empty() {
                    start.push(',');
                    start.push_str(&opts.options);
                }
                m.command(&["loadfile", &opts.url, "replace", "-1", &start]);
                m.command(&["set", "pause", "no"]);
            }
            Err(e) => self.tx.send_modify(|s| {
                s.status = PlaybackStatus::Error;
                s.error = Some(e);
            }),
        }
    }

    fn with(&self, f: impl FnOnce(&Mpv)) {
        if let Some(m) = self.mpv.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            f(m);
        }
    }

    pub fn pause(&self) {
        self.with(|m| m.command(&["set", "pause", "yes"]));
    }

    pub fn resume(&self) {
        self.with(|m| m.command(&["set", "pause", "no"]));
    }

    pub fn toggle(&self) {
        self.with(|m| m.command(&["cycle", "pause"]));
    }

    pub fn seek(&self, seconds: f64) {
        let s = format!("{}", seconds.max(0.0));
        self.with(|m| m.command(&["seek", &s, "absolute"]));
        self.tx
            .send_modify(|snap| snap.position_seconds = seconds.max(0.0));
    }

    pub fn set_volume(&self, volume: f64) {
        let v = volume.clamp(0.0, 1.0);
        let s = format!("{}", v * 100.0);
        self.with(|m| m.command(&["set", "volume", &s]));
        self.tx.send_modify(|snap| snap.volume = v);
    }

    pub fn set_muted(&self, muted: bool) {
        self.with(|m| m.command(&["set", "mute", if muted { "yes" } else { "no" }]));
        self.tx.send_modify(|snap| snap.muted = muted);
    }

    pub fn set_rate(&self, rate: f64) {
        let s = format!("{rate}");
        self.with(|m| m.command(&["set", "speed", &s]));
        self.tx.send_modify(|snap| snap.rate = rate);
    }

    /// Audio / subtitle tracks by mpv id (`aid`, `sid`; `None` = off).
    pub fn set_audio_track(&self, id: Option<i64>) {
        let v = id.map(|i| i.to_string()).unwrap_or_else(|| "no".into());
        self.with(|m| m.command(&["set", "aid", &v]));
    }

    pub fn set_subtitle_track(&self, id: Option<i64>) {
        let v = id.map(|i| i.to_string()).unwrap_or_else(|| "no".into());
        self.with(|m| m.command(&["set", "sid", &v]));
    }

    /// Loads an external subtitle file (URL) and shows it.
    pub fn add_subtitle(&self, url: &str, title: &str, lang: &str) {
        self.with(|m| m.command(&["sub-add", url, "select", title, lang]));
    }

    /// mpv's track list of the current file (empty when nothing is loaded).
    pub async fn tracks(&self) -> Vec<MpvTrack> {
        let m = self.mpv.lock().unwrap_or_else(|p| p.into_inner()).clone();
        match m {
            Some(m) => m.tracks().await,
            None => Vec::new(),
        }
    }

    /// The current position as mpv knows it right now (seconds).
    pub fn position(&self) -> f64 {
        self.snapshot().position_seconds
    }

    pub fn stop(&self) {
        self.with(|m| m.command(&["stop"]));
        self.tx.send_replace(PlaybackSnapshot {
            volume: self.snapshot().volume,
            muted: self.snapshot().muted,
            ..PlaybackSnapshot::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_state_maps_to_the_contract_status() {
        let mut s = MpvState::default();
        assert_eq!(status_of(&s, PlaybackStatus::Idle), PlaybackStatus::Idle);
        assert_eq!(
            status_of(&s, PlaybackStatus::Loading),
            PlaybackStatus::Loading
        );
        s.loaded = true;
        assert_eq!(
            status_of(&s, PlaybackStatus::Loading),
            PlaybackStatus::Playing
        );
        s.paused = true;
        assert_eq!(
            status_of(&s, PlaybackStatus::Playing),
            PlaybackStatus::Paused
        );
        s.ended = true;
        assert_eq!(status_of(&s, PlaybackStatus::Paused), PlaybackStatus::Ended);
    }

    #[test]
    fn buffering_and_load_errors_map_like_the_video_element() {
        let mut s = MpvState {
            loaded: true,
            buffering: true,
            ..MpvState::default()
        };
        // `waiting` while playing, but a paused player stays paused.
        assert_eq!(
            status_of(&s, PlaybackStatus::Playing),
            PlaybackStatus::Loading
        );
        s.paused = true;
        assert_eq!(
            status_of(&s, PlaybackStatus::Playing),
            PlaybackStatus::Paused
        );
        s.error = Some("unrecognized file format".into());
        assert_eq!(
            status_of(&s, PlaybackStatus::Loading),
            PlaybackStatus::Error
        );
    }
}
