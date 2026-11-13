//! OS media controls (Windows SMTC, macOS Now Playing, Linux MPRIS) through `souvlaki`.
//! Contract: `src/shared/ipc/mediaControls.ts`. The renderer publishes metadata and playback
//! (`media_controls_set_*`); the OS buttons come back as `orphyn:media-controls:action`.
//!
//! The controls live on one worker thread (created on the first command, once the main window
//! exists: SMTC needs its HWND), so the commands never block on the OS.

use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::error::{Error, Result};

pub const ACTION: &str = "orphyn:media-controls:action";
/// Step of a bare "seek forward / backward" key (the OS gives no amount).
const SEEK_STEP_SEC: f64 = 10.0;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    artwork_url: Option<String>,
    duration_sec: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Playback {
    status: Status,
    position_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Playing,
    Paused,
    Stopped,
}

/// Maps an OS event to the renderer's `MediaControlAction`; `None` for events Orphyn ignores.
pub fn action_for(event: &MediaControlEvent) -> Option<Value> {
    let signed = |dir: &SeekDirection, secs: f64| match dir {
        SeekDirection::Forward => secs,
        SeekDirection::Backward => -secs,
    };
    Some(match event {
        MediaControlEvent::Play => json!({ "type": "play" }),
        MediaControlEvent::Pause => json!({ "type": "pause" }),
        MediaControlEvent::Toggle => json!({ "type": "toggle" }),
        MediaControlEvent::Next => json!({ "type": "next" }),
        MediaControlEvent::Previous => json!({ "type": "previous" }),
        MediaControlEvent::Stop => json!({ "type": "stop" }),
        MediaControlEvent::Seek(dir) => {
            json!({ "type": "seekBy", "offsetSec": signed(dir, SEEK_STEP_SEC) })
        }
        MediaControlEvent::SeekBy(dir, by) => {
            json!({ "type": "seekBy", "offsetSec": signed(dir, by.as_secs_f64()) })
        }
        MediaControlEvent::SetPosition(MediaPosition(at)) => {
            json!({ "type": "seek", "positionSec": at.as_secs_f64() })
        }
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
    Metadata(usize, Metadata),
    Playback(usize, Playback),
    Clear,
}

struct Handle {
    tx: Mutex<Sender<Msg>>,
}

fn run_worker<R: Runtime>(app: AppHandle<R>, rx: mpsc::Receiver<Msg>) {
    let mut controls: Option<MediaControls> = None;
    let mut failed = false;
    let mut ensure = |hwnd: usize, controls: &mut Option<MediaControls>| {
        if controls.is_some() || failed {
            return;
        }
        let config = PlatformConfig {
            dbus_name: "orphyn",
            display_name: "Orphyn",
            hwnd: (hwnd != 0).then_some(hwnd as *mut std::ffi::c_void),
        };
        match MediaControls::new(config) {
            Ok(mut c) => {
                let handle = app.clone();
                let attached = c.attach(move |event| {
                    if let Some(action) = action_for(&event) {
                        let _ = handle.emit(ACTION, action);
                    }
                });
                match attached {
                    Ok(()) => *controls = Some(c),
                    Err(e) => {
                        log::warn!("[media-controls] attach failed: {e:?}");
                        failed = true;
                    }
                }
            }
            Err(e) => {
                log::warn!("[media-controls] unavailable: {e:?}");
                failed = true;
            }
        }
    };
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Metadata(hwnd, m) => {
                ensure(hwnd, &mut controls);
                if let Some(c) = controls.as_mut() {
                    let _ = c.set_metadata(MediaMetadata {
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
            }
            Msg::Playback(hwnd, p) => {
                ensure(hwnd, &mut controls);
                if let Some(c) = controls.as_mut() {
                    let _ = c.set_playback(playback_of(&p));
                }
            }
            Msg::Clear => {
                if let Some(c) = controls.as_mut() {
                    let _ = c.set_playback(MediaPlayback::Stopped);
                    let _ = c.set_metadata(MediaMetadata::default());
                }
            }
        }
    }
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    app.manage(Handle { tx: Mutex::new(tx) });
    let handle = app.clone();
    std::thread::Builder::new()
        .name("media-controls".into())
        .spawn(move || run_worker(handle, rx))?;
    Ok(())
}

/// The main window's native handle (Windows only; 0 elsewhere).
fn hwnd(app: &AppHandle) -> usize {
    #[cfg(target_os = "windows")]
    {
        if let Some(w) = app.get_webview_window(crate::window::MAIN) {
            if let Ok(h) = w.hwnd() {
                return h.0 as usize;
            }
        }
    }
    let _ = app;
    0
}

fn send(app: &AppHandle, msg: Msg) -> Result<()> {
    let handle = app
        .try_state::<Handle>()
        .ok_or_else(|| Error::Other("media controls not initialised".into()))?;
    let tx = handle.tx.lock().unwrap_or_else(|p| p.into_inner());
    let _ = tx.send(msg);
    Ok(())
}

/// `metadata` is a `MediaControlsMetadata`.
#[tauri::command]
pub fn media_controls_set_metadata(app: AppHandle, metadata: Value) -> Result<()> {
    let m = serde_json::from_value::<Metadata>(metadata)?;
    let hwnd = hwnd(&app);
    send(&app, Msg::Metadata(hwnd, m))
}

/// `playback` is a `MediaControlsPlayback`.
#[tauri::command]
pub fn media_controls_set_playback(app: AppHandle, playback: Value) -> Result<()> {
    let p = serde_json::from_value::<Playback>(playback)?;
    let hwnd = hwnd(&app);
    send(&app, Msg::Playback(hwnd, p))
}

#[tauri::command]
pub fn media_controls_clear(app: AppHandle) -> Result<()> {
    send(&app, Msg::Clear)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_buttons_map_to_renderer_actions() {
        assert_eq!(
            action_for(&MediaControlEvent::Toggle),
            Some(json!({ "type": "toggle" }))
        );
        assert_eq!(
            action_for(&MediaControlEvent::Next),
            Some(json!({ "type": "next" }))
        );
        assert_eq!(
            action_for(&MediaControlEvent::SetPosition(MediaPosition(
                Duration::from_secs(42)
            ))),
            Some(json!({ "type": "seek", "positionSec": 42.0 }))
        );
        assert_eq!(
            action_for(&MediaControlEvent::Seek(SeekDirection::Backward)),
            Some(json!({ "type": "seekBy", "offsetSec": -10.0 }))
        );
        assert_eq!(
            action_for(&MediaControlEvent::SeekBy(
                SeekDirection::Forward,
                Duration::from_secs(5)
            )),
            Some(json!({ "type": "seekBy", "offsetSec": 5.0 }))
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
        let stopped = Playback {
            status: Status::Stopped,
            position_sec: Some(9.0),
        };
        assert_eq!(playback_of(&stopped), MediaPlayback::Stopped);
    }
}
