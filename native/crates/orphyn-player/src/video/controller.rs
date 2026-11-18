//! Video playback controller with the API of `features/video/controller.ts`
//! (`PlaybackController` / `PlaybackSnapshot`), on top of libmpv. Screens subscribe to the
//! snapshot and draw the frames with [`super::VideoWidget`].
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use super::mpv::{Mpv, MpvState};

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
}

/// One video player. libmpv is loaded on the first `play`; when it is missing the snapshot goes
/// to `Error` with the reason.
#[derive(Clone)]
pub struct VideoController {
    mpv: Arc<Mutex<Option<Arc<Mpv>>>>,
    tx: Arc<watch::Sender<PlaybackSnapshot>>,
}

impl Default for VideoController {
    fn default() -> Self {
        Self::new()
    }
}

fn status_of(s: &MpvState, previous: PlaybackStatus) -> PlaybackStatus {
    if s.ended {
        PlaybackStatus::Ended
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
        let m = Arc::new(Mpv::start(Arc::new(move |s: MpvState| {
            tx.send_modify(|snap| {
                snap.status = status_of(&s, snap.status);
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
                let start = format!("start={}", opts.start_seconds.max(0.0));
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
}
