//! The main window, like `src-tauri/src/window.rs`:
//! - 1280x800 by default, at least 1000x640, size / position / maximized restored at start;
//! - Windows: no native frame; the caption buttons are drawn here ([`CaptionButtons`], same
//!   look and 140 px as `bridge/captionButtons.ts`), drag regions are the `.jr-drag` areas;
//! - macOS: transparent title bar over the content with the traffic lights in the sidebar's
//!   corner (winit 0.31 has no traffic-light offset yet: they sit at the system position);
//! - Linux: native decorations.
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_native::winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use dioxus_native::winit::window::{Window, WindowAttributes};
use serde::{Deserialize, Serialize};

pub const MIN_WIDTH: f64 = 1000.0;
pub const MIN_HEIGHT: f64 = 640.0;
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 800.0;
/// Saved bounds of the native app. Separate from the Tauri app's `.window-state.json` so both
/// builds can be installed side by side (README, "Migration").
const STATE_FILE: &str = "native-window-state.json";

pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(windows) {
        "win32"
    } else {
        "linux"
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedBounds {
    /// Physical pixels.
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

fn state_path() -> Option<std::path::PathBuf> {
    orphyn_core::profile::config_dir()
        .ok()
        .map(|d| d.join(STATE_FILE))
}

pub fn load_bounds() -> Option<SavedBounds> {
    if orphyn_core::profile::is_test() {
        return None;
    }
    let raw = std::fs::read_to_string(state_path()?).ok()?;
    serde_json::from_str::<SavedBounds>(&raw)
        .ok()
        .filter(|b| b.width >= 200 && b.height >= 200)
}

pub fn save_bounds(b: &SavedBounds) {
    if orphyn_core::profile::is_test() {
        return;
    }
    if let (Some(path), Ok(text)) = (state_path(), serde_json::to_string_pretty(b)) {
        let _ = orphyn_core::accounts::store::write_atomic(&path, &text);
    }
}

/// The attributes the window opens with. `ORPHYN_WIN=WxH` forces a logical size (screenshots).
pub fn attributes() -> WindowAttributes {
    let mut attrs = WindowAttributes::default()
        .with_title("Orphyn")
        .with_window_icon(window_icon())
        .with_min_surface_size(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT));
    let forced = std::env::var("ORPHYN_WIN").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?))
    });
    match (forced, load_bounds()) {
        (Some((w, h)), _) => {
            attrs = attrs
                .with_min_surface_size(LogicalSize::new(w.min(MIN_WIDTH), h.min(MIN_HEIGHT)))
                .with_surface_size(LogicalSize::new(w, h));
        }
        (None, Some(b)) => {
            attrs = attrs
                .with_surface_size(PhysicalSize::new(b.width, b.height))
                .with_position(PhysicalPosition::new(b.x, b.y))
                .with_maximized(b.maximized);
        }
        (None, None) => {
            attrs = attrs.with_surface_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT));
        }
    }
    platform_attributes(attrs)
}

#[cfg(windows)]
fn platform_attributes(attrs: WindowAttributes) -> WindowAttributes {
    use dioxus_native::winit::platform::windows::WindowAttributesWindows;
    attrs
        .with_decorations(false)
        .with_platform_attributes(Box::new(
            WindowAttributesWindows::default().with_undecorated_shadow(true),
        ))
}

#[cfg(target_os = "macos")]
fn platform_attributes(attrs: WindowAttributes) -> WindowAttributes {
    use dioxus_native::winit::platform::macos::WindowAttributesMacOS;
    attrs.with_platform_attributes(Box::new(
        WindowAttributesMacOS::default()
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true),
    ))
}

/// Wayland app_id / X11 WM_CLASS "orphyn": the name of the installed .desktop file and its
/// StartupWMClass, so docks and Alt+Tab use the installed icon.
#[cfg(not(any(windows, target_os = "macos")))]
fn platform_attributes(attrs: WindowAttributes) -> WindowAttributes {
    use dioxus_native::winit::platform::{
        wayland::WindowAttributesWayland, x11::WindowAttributesX11,
    };
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        && std::env::var("XDG_SESSION_TYPE").map_or(true, |t| t != "x11");
    if wayland {
        attrs.with_platform_attributes(Box::new(
            WindowAttributesWayland::default().with_name("orphyn", "orphyn"),
        ))
    } else {
        attrs.with_platform_attributes(Box::new(
            WindowAttributesX11::default().with_name("orphyn", "orphyn"),
        ))
    }
}

/// Current bounds of a window (to save on resize / move).
pub fn bounds_of(w: &dyn Window) -> Option<SavedBounds> {
    let maximized = w.is_maximized();
    let size = w.surface_size();
    let pos = w.outer_position().ok()?;
    Some(SavedBounds {
        width: size.width,
        height: size.height,
        x: pos.x,
        y: pos.y,
        maximized,
    })
}

/// Starts a window drag (the `.jr-drag` regions); a double click maximizes / restores.
pub fn drag_region_mousedown(window: &Arc<dyn Window>, e: &MouseEvent) {
    if e.trigger_button() != Some(dioxus::html::input_data::MouseButton::Primary) {
        return;
    }
    let _ = window.drag_window();
}

pub fn toggle_maximize(window: &Arc<dyn Window>) {
    window.set_maximized(!window.is_maximized());
}

/// Saves the bounds and quits (the caption close button).
pub fn close_window(window: &Arc<dyn Window>) {
    if let Some(b) = bounds_of(window.as_ref()) {
        save_bounds(&b);
    }
    std::process::exit(0)
}

/// Windows caption buttons (minimize, maximize / restore, close), drawn in the 140 px the shell
/// reserves (`--titlebar-inset-right`). Styled in `native/styles/overrides.css`.
#[component]
pub fn CaptionButtons(fullscreen: bool, focused: bool, maximized: bool, dark: bool) -> Element {
    let window = dioxus_native::use_window();
    if platform() != "win32" || fullscreen {
        return rsx! {};
    }
    let (w1, w2, w3) = (window.clone(), window.clone(), window.clone());
    let fg = if dark { "#ffffff" } else { "#000000" };
    let (max_title, max_glyph) = if maximized {
        ("Restore", "\u{E923}")
    } else {
        ("Maximize", "\u{E922}")
    };
    rsx! {
        div {
            id: "orphyn-caption-buttons",
            "data-focused": "{focused}",
            style: "--caption-fg: {fg};",
            button { r#type: "button", "data-action": "minimize", title: "Minimize", tabindex: "-1",
                onclick: move |_| w1.set_minimized(true), "\u{E921}" }
            button { r#type: "button", "data-action": "maximize", title: max_title, tabindex: "-1",
                onclick: move |_| toggle_maximize(&w2), "{max_glyph}" }
            button { r#type: "button", "data-action": "close", title: "Close", tabindex: "-1",
                onclick: move |_| close_window(&w3),
                "\u{E8BB}"
            }
        }
    }
}

/// The Orphyn icon (build/icons/icon-512.png), decoded for the window: title bar, taskbar and
/// Alt+Tab on Windows, the dock / task switcher on Linux (macOS takes the bundle's .icns).
fn window_icon() -> Option<dioxus_native::winit::icon::Icon> {
    use dioxus_native::winit::icon::{Icon, RgbaIcon};
    const PNG: &[u8] = include_bytes!("../../../../build/icons/icon-512.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(PNG))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    RgbaIcon::new(buf, info.width, info.height)
        .ok()
        .map(Icon::from)
}
