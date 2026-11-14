//! The main window (contract: `src/shared/ipc/window.ts`).
//!
//! - macOS: overlay title bar (traffic lights inset at 20,20), hidden title, sidebar vibrancy.
//! - Windows: no native frame; Mica on Windows 11 (opaque background on Windows 10). The caption
//!   buttons and drag regions are drawn by the renderer bridge (`src/renderer/src/bridge/`).
//! - Linux: native decorations.
//!
//! Size, position, maximized and fullscreen state are restored by `tauri-plugin-window-state`.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, Runtime, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::error::{Error, Result};

pub const MAIN: &str = "main";
pub const STATE_CHANGED: &str = "window:state-changed";
pub const MIN_WIDTH: f64 = 1000.0;
pub const MIN_HEIGHT: f64 = 640.0;
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 800.0;

/// `WindowMaterial` of the TypeScript contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    Vibrancy,
    Mica,
    None,
}

/// `WindowState` of the TypeScript contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub is_fullscreen: bool,
    pub is_maximized: bool,
    pub is_focused: bool,
    pub material: Material,
}

/// `WindowAppearance` of the TypeScript contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn theme(self) -> Option<Theme> {
        match self {
            Appearance::System => None,
            Appearance::Light => Some(Theme::Light),
            Appearance::Dark => Some(Theme::Dark),
        }
    }
}

/// The material actually applied and the last state sent to the renderer.
struct Shell {
    material: Mutex<Material>,
    last: Mutex<Option<WindowState>>,
}

/// Node-style platform name the renderer expects in `window.orphyn.platform`.
pub fn node_platform(os: &str) -> &'static str {
    match os {
        "windows" => "win32",
        "macos" => "darwin",
        "linux" => "linux",
        _ => "web",
    }
}

/// Webview engine, used by the renderer to pick a device profile (`src/shared/webview`).
pub fn webview_engine(os: &str) -> &'static str {
    match os {
        "windows" => "webview2",
        "macos" => "wkwebview",
        _ => "webkitgtk",
    }
}

/// Script run before the page: tells the bridge it is in the Tauri shell and on which platform.
pub fn shell_info_script(os: &str) -> String {
    format!(
        "Object.defineProperty(window, '__ORPHYN_SHELL__', {{ value: Object.freeze({{ platform: '{}', webview: '{}' }}) }});",
        node_platform(os),
        webview_engine(os)
    )
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    app.manage(Shell {
        material: Mutex::new(Material::None),
        last: Mutex::new(None),
    });
    let window = build(app)?;
    let material = apply_material(&window, None);
    *app.state::<Shell>()
        .material
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = material;

    let handle = window.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_)
                | WindowEvent::Focused(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) {
            emit_if_changed(&handle);
        }
    });
    Ok(())
}

fn build<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>> {
    let os = std::env::consts::OS;
    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Orphyn")
        .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .center()
        // Shown by `lib.rs` once the saved bounds are restored, to avoid a jump.
        .visible(false)
        .initialization_script(shell_info_script(os));

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(20.0, 20.0))
        .transparent(true);

    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false).transparent(true);

    // Debug builds only: the test profile, the DevTools protocol port (Playwright
    // `connectOverCDP`) and the UI language of the end-to-end suite (see `profile.rs`). wry
    // passes its own browser arguments, which makes WebView2 ignore
    // `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`.
    let builder = match crate::profile::webview_dir() {
        Some(dir) => builder.data_directory(dir),
        None => builder,
    };
    #[cfg(target_os = "windows")]
    let builder = match crate::profile::webview2_args() {
        Some(args) => builder.additional_browser_args(&args),
        None => builder,
    };

    Ok(builder.build()?)
}

/// Applies the native material; `dark` forces the Mica tint (None follows the system).
fn apply_material<R: Runtime>(window: &WebviewWindow<R>, dark: Option<bool>) -> Material {
    #[cfg(target_os = "macos")]
    {
        let _ = dark;
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        match apply_vibrancy(
            window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::FollowsWindowActiveState),
            None,
        ) {
            Ok(()) => Material::Vibrancy,
            Err(e) => {
                log::warn!("[window] vibrancy unavailable: {e}");
                Material::None
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        match window_vibrancy::apply_mica(window, dark) {
            Ok(()) => Material::Mica,
            Err(e) => {
                // Windows 10: no Mica. The window is transparent, so give it a solid colour.
                log::info!("[window] Mica unavailable, opaque window: {e}");
                let dark = dark.unwrap_or_else(|| matches!(window.theme(), Ok(Theme::Dark)));
                let color = if dark {
                    tauri::window::Color(0x1c, 0x1c, 0x1e, 0xff)
                } else {
                    tauri::window::Color(0xf2, 0xf2, 0xf7, 0xff)
                };
                let _ = window.set_background_color(Some(color));
                Material::None
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, dark);
        Material::None
    }
}

fn snapshot<R: Runtime>(window: &WebviewWindow<R>) -> WindowState {
    let material = window
        .try_state::<Shell>()
        .map(|s| *s.material.lock().unwrap_or_else(|p| p.into_inner()))
        .unwrap_or(Material::None);
    WindowState {
        is_fullscreen: window.is_fullscreen().unwrap_or(false),
        is_maximized: window.is_maximized().unwrap_or(false),
        is_focused: window.is_focused().unwrap_or(false),
        material,
    }
}

fn emit_if_changed<R: Runtime>(window: &WebviewWindow<R>) {
    let next = snapshot(window);
    if let Some(shell) = window.try_state::<Shell>() {
        let mut last = shell.last.lock().unwrap_or_else(|p| p.into_inner());
        if last.as_ref() == Some(&next) {
            return;
        }
        *last = Some(next);
    }
    let _ = window.emit_to(window.label(), STATE_CHANGED, next);
}

/// Brings the main window forward (second launch, notification click...).
pub fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn window_minimize<R: Runtime>(window: WebviewWindow<R>) -> Result<()> {
    Ok(window.minimize()?)
}

#[tauri::command]
pub fn window_toggle_maximize<R: Runtime>(window: WebviewWindow<R>) -> Result<()> {
    if window.is_maximized()? {
        window.unmaximize()?;
    } else {
        window.maximize()?;
    }
    Ok(())
}

#[tauri::command]
pub fn window_close<R: Runtime>(window: WebviewWindow<R>) -> Result<()> {
    Ok(window.close()?)
}

#[tauri::command]
pub fn window_get_state<R: Runtime>(window: WebviewWindow<R>) -> WindowState {
    snapshot(&window)
}

#[tauri::command]
pub fn window_set_fullscreen<R: Runtime>(window: WebviewWindow<R>, fullscreen: bool) -> Result<()> {
    window.set_fullscreen(fullscreen)?;
    emit_if_changed(&window);
    Ok(())
}

/// Native chrome (vibrancy, Mica tint, caption glyphs, context menus) follows the app theme.
#[tauri::command]
pub fn window_set_appearance<R: Runtime>(
    window: WebviewWindow<R>,
    appearance: Appearance,
) -> Result<()> {
    window.set_theme(appearance.theme())?;
    #[cfg(target_os = "windows")]
    {
        let current = window
            .try_state::<Shell>()
            .map(|s| *s.material.lock().unwrap_or_else(|p| p.into_inner()));
        if current == Some(Material::Mica) {
            let dark = appearance.theme().map(|t| t == Theme::Dark);
            apply_material(&window, dark);
        }
    }
    Ok(())
}

/// The main window, for modules that need it outside a command.
pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>> {
    app.get_webview_window(MAIN).ok_or(Error::NoWindow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_names_match_electron() {
        assert_eq!(node_platform("windows"), "win32");
        assert_eq!(node_platform("macos"), "darwin");
        assert_eq!(node_platform("linux"), "linux");
        assert_eq!(webview_engine("macos"), "wkwebview");
        assert_eq!(webview_engine("linux"), "webkitgtk");
    }

    #[test]
    fn shell_script_defines_the_platform() {
        let script = shell_info_script("windows");
        assert!(script.contains("__ORPHYN_SHELL__"));
        assert!(script.contains("platform: 'win32'"));
        assert!(script.contains("webview: 'webview2'"));
    }

    #[test]
    fn state_serializes_like_the_typescript_contract() {
        let state = WindowState {
            is_fullscreen: false,
            is_maximized: true,
            is_focused: true,
            material: Material::Mica,
        };
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({
                "isFullscreen": false, "isMaximized": true, "isFocused": true, "material": "mica"
            })
        );
        let a: Appearance = serde_json::from_str("\"dark\"").unwrap();
        assert_eq!(a.theme(), Some(Theme::Dark));
        assert_eq!(Appearance::System.theme(), None);
    }
}
