//! Where Orphyn keeps its files, and the test profile of debug builds.
//!
//! Release builds always use the OS locations (`app_data_dir`, `app_config_dir`, the default
//! webview profile, the keychain service `Orphyn`). Debug builds accept a few variables for the
//! end-to-end suite (`e2e/`, see `docs/QA.md`), so it never touches the real profile:
//!
//! - `ORPHYN_USER_DATA_DIR=<dir>`: data, settings and the webview profile all live in `<dir>`; the
//!   Electron import reads `<dir>/electron/<Orphyn|Lumen|...>/accounts.json` instead of the real
//!   ones; tokens use the keychain service `Orphyn (test)`; the single-instance lock and the saved
//!   window bounds are skipped, so a test run works beside an open Orphyn.
//! - `ORPHYN_CDP_PORT=<port>` (Windows): WebView2 opens the DevTools protocol on that port.
//! - `ORPHYN_LANG=<bcp47>` (Windows): the webview's UI language (`navigator.language`).

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::error::Result;

/// The test profile directory (debug builds with `ORPHYN_USER_DATA_DIR` only).
pub fn test_dir() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        std::env::var_os("ORPHYN_USER_DATA_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// True when running on a test profile.
pub fn is_test() -> bool {
    test_dir().is_some()
}

/// Data directory (accounts, Discord and PiP settings).
pub fn data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    match test_dir() {
        Some(dir) => Ok(dir),
        None => Ok(app.path().app_data_dir()?),
    }
}

/// Config directory (updater settings).
pub fn config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    match test_dir() {
        Some(dir) => Ok(dir),
        None => Ok(app.path().app_config_dir()?),
    }
}

/// Webview profile directory for every window, None for the default one.
pub fn webview_dir() -> Option<PathBuf> {
    test_dir().map(|d| d.join("webview"))
}

/// Folders searched for the accounts of the Electron builds (1.0.x), in order.
pub fn legacy_config_dir() -> Option<PathBuf> {
    match test_dir() {
        Some(dir) => Some(dir.join("electron")),
        None => dirs::config_dir(),
    }
}

/// Keychain service holding the access tokens.
pub fn keychain_service() -> &'static str {
    if is_test() {
        "Orphyn (test)"
    } else {
        "Orphyn"
    }
}

/// Extra WebView2 arguments of debug builds (`ORPHYN_CDP_PORT`, `ORPHYN_LANG`). WebView2 shares
/// one browser process, and so one set of arguments, per profile: every window must use these.
#[cfg(target_os = "windows")]
pub fn webview2_args() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        let port = std::env::var("ORPHYN_CDP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok());
        let lang = std::env::var("ORPHYN_LANG")
            .ok()
            .filter(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        if port.is_none() && lang.is_none() {
            return None;
        }
        // wry's own defaults, which `additional_browser_args` replaces.
        let mut args =
            String::from("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection");
        if let Some(port) = port {
            args.push_str(&format!(" --remote-debugging-port={port}"));
        }
        if let Some(lang) = lang {
            args.push_str(&format!(" --lang={lang}"));
        }
        Some(args)
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}
