//! OS media controls (Windows SMTC, macOS Now Playing, Linux MPRIS) through `souvlaki`; port of
//! `src-tauri/src/media_controls`. The app publishes metadata and playback, the OS buttons come
//! back as [`Action`]s through the callback given to [`MediaControlsService::start`].
//!
//! The controls live on one worker thread created with the first message, so nothing blocks on
//! the OS. SMTC needs the window handle, given at `start`.

use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};

/// Step of a bare "seek forward / backward" key (the OS gives no amount).
const SEEK_STEP_SEC: f64 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekBy(f64),
    Seek(f64),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork_url: Option<String>,
    pub duration_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    pub status: Status,
    pub position_sec: Option<f64>,
}

/// Maps an OS event to an [`Action`]; `None` for events Orphyn ignores.
pub fn action_for(event: &MediaControlEvent) -> Option<Action> {
    let signed = |dir: &SeekDirection, secs: f64| match dir {
        SeekDirection::Forward => secs,
        SeekDirection::Backward => -secs,
    };
    Some(match event {
        MediaControlEvent::Play => Action::Play,
        MediaControlEvent::Pause => Action::Pause,
        MediaControlEvent::Toggle => Action::Toggle,
        MediaControlEvent::Next => Action::Next,
        MediaControlEvent::Previous => Action::Previous,
        MediaControlEvent::Stop => Action::Stop,
        MediaControlEvent::Seek(dir) => Action::SeekBy(signed(dir, SEEK_STEP_SEC)),
        MediaControlEvent::SeekBy(dir, by) => Action::SeekBy(signed(dir, by.as_secs_f64())),
        MediaControlEvent::SetPosition(MediaPosition(at)) => Action::Seek(at.as_secs_f64()),
        _ => return None,
    })
}

fn position(sec: Option<f64>) -> Option<MediaPosition> {
    sec.filter(|s| s.is_finite() && *s >= 0.0)
        .map(|s| MediaPosition(Duration::from_secs_f64(s)))
}

fn playback_of(p: &Playback) -> MediaPlayback {
    match p.status {
        Status::Playing => MediaPlayback::Playing {
            progress: position(p.position_sec),
        },
        Status::Paused => MediaPlayback::Paused {
            progress: position(p.position_sec),
        },
        Status::Stopped => MediaPlayback::Stopped,
    }
}

enum Msg {
    Metadata(Metadata),
    Playback(Playback),
    Clear,
}

#[derive(Clone)]
pub struct MediaControlsService {
    tx: Sender<Msg>,
}

impl MediaControlsService {
    /// `hwnd` is the main window's native handle on Windows (0 elsewhere).
    pub fn start(hwnd: usize, on_action: impl Fn(Action) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        let spawned = std::thread::Builder::new()
            .name("media-controls".into())
            .spawn(move || run_worker(hwnd, rx, on_action));
        if let Err(e) = spawned {
            log::warn!("[media-controls] worker not started: {e}");
        }
        Self { tx }
    }

    pub fn set_metadata(&self, m: Metadata) {
        let _ = self.tx.send(Msg::Metadata(m));
    }

    pub fn set_playback(&self, p: Playback) {
        let _ = self.tx.send(Msg::Playback(p));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(Msg::Clear);
    }
}

fn run_worker(hwnd: usize, rx: mpsc::Receiver<Msg>, on_action: impl Fn(Action) + Send + 'static) {
    let config = PlatformConfig {
        dbus_name: "orphyn",
        display_name: "Orphyn",
        hwnd: (hwnd != 0).then_some(hwnd as *mut std::ffi::c_void),
    };
    let mut controls = match MediaControls::new(config) {
        Ok(mut c) => {
            let attached = c.attach(move |event| {
                if let Some(a) = action_for(&event) {
                    on_action(a);
                }
            });
            if let Err(e) = attached {
                log::warn!("[media-controls] attach failed: {e:?}");
                return;
            }
            c
        }
        Err(e) => {
            log::warn!("[media-controls] unavailable: {e:?}");
            return;
        }
    };
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Metadata(m) => {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(m.title.as_str()),
                    artist: m.artist.as_deref(),
                    album: m.album.as_deref(),
                    cover_url: m.artwork_url.as_deref(),
                    duration: m
                        .duration_sec
                        .filter(|d| d.is_finite() && *d > 0.0)
                        .map(Duration::from_secs_f64),
                });
            }
            Msg::Playback(p) => {
                let _ = controls.set_playback(playback_of(&p));
            }
            Msg::Clear => {
                let _ = controls.set_playback(MediaPlayback::Stopped);
                let _ = controls.set_metadata(MediaMetadata::default());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_buttons_map_to_actions() {
        assert_eq!(action_for(&MediaControlEvent::Toggle), Some(Action::Toggle));
        assert_eq!(action_for(&MediaControlEvent::Next), Some(Action::Next));
        assert_eq!(
            action_for(&MediaControlEvent::SetPosition(MediaPosition(
                Duration::from_secs(42)
            ))),
            Some(Action::Seek(42.0))
        );
        assert_eq!(
            action_for(&MediaControlEvent::Seek(SeekDirection::Backward)),
            Some(Action::SeekBy(-10.0))
        );
        assert_eq!(
            action_for(&MediaControlEvent::SeekBy(
                SeekDirection::Forward,
                Duration::from_secs(5)
            )),
            Some(Action::SeekBy(5.0))
        );
        assert_eq!(action_for(&MediaControlEvent::Raise), None);
    }

    #[test]
    fn playback_carries_the_position() {
        let p = Playback {
            status: Status::Paused,
            position_sec: Some(3.5),
        };
        assert_eq!(
            playback_of(&p),
            MediaPlayback::Paused {
                progress: Some(MediaPosition(Duration::from_millis(3500)))
            }
        );
        let bad = Playback {
            status: Status::Playing,
            position_sec: Some(f64::NAN),
        };
        assert_eq!(playback_of(&bad), MediaPlayback::Playing { progress: None });
    }
}
