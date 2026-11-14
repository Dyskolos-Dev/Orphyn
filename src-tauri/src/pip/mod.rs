//! Picture-in-picture windows. Contract: `src/shared/ipc/pip.ts`; behaviour in `docs/PIP.md`.
//!
//! The PiP windows are small always-on-top, frameless, transparent windows that load the same
//! renderer with `?pip=music|video` (see `src/renderer/src/features/pip/`). Rust only owns the
//! windows (creation, size ratio, remembered position, one at a time); the player state travels
//! between the main window and the PiP window as plain Tauri events, so the players stay where
//! they are.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::error::{Error, Result};

pub const STATE_CHANGED: &str = "orphyn:pip:state-changed";
const POSITIONS_FILE: &str = "pip.json";
const MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Music,
    Video,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Music => "pip-music",
            Kind::Video => "pip-video",
        }
    }
    fn parse(s: &str) -> Option<Kind> {
        match s {
            "music" => Some(Kind::Music),
            "video" => Some(Kind::Video),
            _ => None,
        }
    }
    /// (default width, min width, width / height ratio).
    fn geometry(self) -> (f64, f64, f64) {
        match self {
            Kind::Music => (320.0, 240.0, 320.0 / 380.0),
            Kind::Video => (480.0, 320.0, 16.0 / 9.0),
        }
    }
}

/// `PipState` of the TypeScript contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct State {
    pub open: bool,
    pub kind: Option<Kind>,
}

/// Remembered window placement, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
}

#[derive(Default)]
struct Shared {
    placements: HashMap<Kind, Placement>,
    last_write: Option<Instant>,
}

struct Store(Mutex<Shared>);

/// Height that keeps `ratio` (width / height) for a given width.
pub fn height_for(width: u32, ratio: f64) -> u32 {
    (f64::from(width) / ratio).round() as u32
}

/// Whether a window of `size` placed at `(x, y)` overlaps one of the `monitors`
/// (`(x, y, width, height)`), i.e. a remembered position is still reachable.
pub fn is_visible(x: i32, y: i32, size: (u32, u32), monitors: &[(i32, i32, u32, u32)]) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        x + size.0 as i32 > mx + 40
            && x < mx + mw as i32 - 40
            && y + size.1 as i32 > my + 40
            && y < my + mh as i32 - 40
    })
}

fn positions_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    crate::profile::data_dir(app)
        .ok()
        .map(|d| d.join(POSITIONS_FILE))
}

fn load_positions<R: Runtime>(app: &AppHandle<R>) -> HashMap<Kind, Placement> {
    positions_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_positions<R: Runtime>(app: &AppHandle<R>, shared: &Shared) {
    let Some(path) = positions_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = serde_json::to_string(&shared.placements).map(|s| std::fs::write(path, s));
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    app.manage(Store(Mutex::new(Shared {
        placements: load_positions(app),
        last_write: None,
    })));
    // The PiP windows only get what they need: events (state sync), dragging, closing, and
    // bringing the main window back.
    app.add_capability(
        r#"{
          "identifier": "pip",
          "description": "Picture-in-picture windows: state events, dragging, back to the main window.",
          "windows": ["pip-music", "pip-video"],
          "permissions": [
            "core:default",
            "core:window:allow-start-dragging",
            "core:window:allow-close",
            "core:window:allow-show",
            "core:window:allow-unminimize",
            "core:window:allow-set-focus"
          ]
        }"#,
    )?;
    Ok(())
}

fn current<R: Runtime>(app: &AppHandle<R>) -> State {
    for kind in [Kind::Music, Kind::Video] {
        if app.get_webview_window(kind.label()).is_some() {
            return State {
                open: true,
                kind: Some(kind),
            };
        }
    }
    State {
        open: false,
        kind: None,
    }
}

fn emit_state<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(STATE_CHANGED, current(app));
}

fn remember<R: Runtime>(window: &WebviewWindow<R>, kind: Kind, force: bool) {
    let app = window.app_handle();
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    let Some(store) = app.try_state::<Store>() else {
        return;
    };
    let mut shared = store.0.lock().unwrap_or_else(|p| p.into_inner());
    shared.placements.insert(
        kind,
        Placement {
            x: pos.x,
            y: pos.y,
            width: size.width,
        },
    );
    let due = shared
        .last_write
        .is_none_or(|t| t.elapsed() > Duration::from_millis(500));
    if force || due {
        shared.last_write = Some(Instant::now());
        save_positions(app, &shared);
    }
}

fn monitors<R: Runtime>(app: &AppHandle<R>) -> Vec<(i32, i32, u32, u32)> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            (
                m.position().x,
                m.position().y,
                m.size().width,
                m.size().height,
            )
        })
        .collect()
}

/// Bottom-right corner of the primary monitor.
fn default_position<R: Runtime>(app: &AppHandle<R>, size: (u32, u32)) -> Option<(i32, i32)> {
    let m = app.primary_monitor().ok().flatten()?;
    let scale = m.scale_factor();
    let margin = (f64::from(MARGIN) * scale) as i32;
    Some((
        m.position().x + m.size().width as i32 - size.0 as i32 - margin,
        m.position().y + m.size().height as i32 - size.1 as i32 - margin * 2,
    ))
}

fn open_window<R: Runtime>(app: &AppHandle<R>, kind: Kind) -> Result<()> {
    // One PiP at a time: opening the other kind replaces it.
    for other in [Kind::Music, Kind::Video] {
        if other != kind {
            if let Some(w) = app.get_webview_window(other.label()) {
                let _ = w.destroy();
            }
        }
    }
    if let Some(existing) = app.get_webview_window(kind.label()) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let (width, min_width, ratio) = kind.geometry();
    let min_height = (min_width / ratio).round();
    let query = match kind {
        Kind::Music => "pip=music",
        Kind::Video => "pip=video",
    };
    let builder = WebviewWindowBuilder::new(
        app,
        kind.label(),
        WebviewUrl::App(format!("index.html?{query}").into()),
    )
    .title("Orphyn")
    .inner_size(width, (width / ratio).round())
    .min_inner_size(min_width, min_height)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(true)
    .visible(false)
    .focused(false);
    // WebView2/WebKit: no maximize/fullscreen for a PiP.
    let builder = builder.maximizable(false).minimizable(false);

    // WebView2 shares one browser process (and so one set of arguments) per profile: a window
    // created with other arguments than the main one fails with 0x8007139F. The PiP uses the same
    // profile and arguments as the main window (see `profile.rs`).
    let builder = match crate::profile::webview_dir() {
        Some(dir) => builder.data_directory(dir),
        None => builder,
    };
    #[cfg(target_os = "windows")]
    let builder = match crate::profile::webview2_args() {
        Some(args) => builder.additional_browser_args(&args),
        None => builder,
    };
    let window = builder.build()?;

    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
        let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, Some(16.0));
    }

    // Placement: the remembered one when it is still on a screen, else bottom-right.
    let scale = window.scale_factor().unwrap_or(1.0);
    let saved = app.try_state::<Store>().and_then(|s| {
        s.0.lock()
            .ok()
            .and_then(|g| g.placements.get(&kind).copied())
    });
    let mut size = (
        (width * scale) as u32,
        height_for((width * scale) as u32, ratio),
    );
    if let Some(p) = saved.filter(|p| p.width >= (min_width * scale) as u32) {
        size = (p.width, height_for(p.width, ratio));
        let _ = window.set_size(PhysicalSize::new(size.0, size.1));
        if is_visible(p.x, p.y, size, &monitors(app)) {
            let _ = window.set_position(PhysicalPosition::new(p.x, p.y));
        } else if let Some((x, y)) = default_position(app, size) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
    } else if let Some((x, y)) = default_position(app, size) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    let _ = window.show();

    let watched = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(s) if s.width > 0 => {
            // Keep the aspect ratio: the height follows the width.
            let want = height_for(s.width, ratio);
            if s.height.abs_diff(want) > 1 && !watched.is_minimized().unwrap_or(false) {
                let _ = watched.set_size(PhysicalSize::new(s.width, want));
            }
            remember(&watched, kind, false);
        }
        WindowEvent::Moved(_) => remember(&watched, kind, false),
        WindowEvent::Destroyed => {
            // `inner_size` is gone by now: the last `remember` already holds the placement.
            let app = watched.app_handle();
            if let Some(store) = app.try_state::<Store>() {
                let shared = store.0.lock().unwrap_or_else(|p| p.into_inner());
                save_positions(app, &shared);
            }
            emit_state(app);
        }
        _ => {}
    });
    emit_state(app);
    Ok(())
}

#[tauri::command]
pub fn pip_get_state(app: AppHandle) -> Result<State> {
    Ok(current(&app))
}

/// `kind` is `"music"` or `"video"`.
#[tauri::command]
pub async fn pip_open(app: AppHandle, kind: String) -> Result<()> {
    let kind =
        Kind::parse(&kind).ok_or_else(|| Error::Other(format!("unknown pip kind: {kind}")))?;
    open_window(&app, kind)
}

#[tauri::command]
pub fn pip_close(app: AppHandle) -> Result<()> {
    for kind in [Kind::Music, Kind::Video] {
        if let Some(w) = app.get_webview_window(kind.label()) {
            let _ = w.close();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_follows_the_ratio() {
        assert_eq!(height_for(320, 320.0 / 380.0), 380);
        assert_eq!(height_for(240, 320.0 / 380.0), 285);
        assert_eq!(height_for(480, 16.0 / 9.0), 270);
    }

    #[test]
    fn a_remembered_position_must_still_be_on_a_screen() {
        let screens = [(0, 0, 1920, 1080)];
        assert!(is_visible(100, 100, (320, 380), &screens));
        assert!(is_visible(1700, 700, (320, 380), &screens));
        // The second monitor was unplugged.
        assert!(!is_visible(2500, 100, (320, 380), &screens));
        assert!(!is_visible(100, -2000, (320, 380), &screens));
        assert!(is_visible(
            -200,
            100,
            (320, 380),
            &[(-1920, 0, 1920, 1080), (0, 0, 1920, 1080)]
        ));
    }

    #[test]
    fn kinds_parse_and_have_stable_labels() {
        assert_eq!(Kind::parse("music"), Some(Kind::Music));
        assert_eq!(Kind::parse("video"), Some(Kind::Video));
        assert_eq!(Kind::parse("x"), None);
        assert_eq!(Kind::Music.label(), "pip-music");
    }
}
