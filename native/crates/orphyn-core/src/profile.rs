//! Where Orphyn keeps its files: the very folders of the Tauri app (identifier
//! `app.lumen.desktop`), so the native build reads a 1.5 profile without moving anything.
//!
//! Tauri resolves `app_data_dir` / `app_config_dir` to `<OS data dir>/<identifier>` and
//! `<OS config dir>/<identifier>`: `%APPDATA%\app.lumen.desktop` on Windows,
//! `~/Library/Application Support/app.lumen.desktop` on macOS, `~/.local/share/app.lumen.desktop`
//! and `~/.config/app.lumen.desktop` on Linux.
//!
//! Debug builds accept `ORPHYN_USER_DATA_DIR=<dir>` like the Tauri app: everything then lives in
//! `<dir>` and tokens use the keychain service `Orphyn (test)`.
use std::path::PathBuf;

use crate::error::{Error, Result};

/// The Tauri bundle identifier, kept so the native app shares the 1.5 profile.
pub const APP_ID: &str = "app.lumen.desktop";

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

/// Data directory (accounts, local storage, Discord and PiP settings).
pub fn data_dir() -> Result<PathBuf> {
    match test_dir() {
        Some(dir) => Ok(dir),
        None => dirs::data_dir()
            .map(|d| d.join(APP_ID))
            .ok_or(Error::NoDataDir),
    }
}

/// Config directory (updater settings, saved window bounds).
pub fn config_dir() -> Result<PathBuf> {
    match test_dir() {
        Some(dir) => Ok(dir),
        None => dirs::config_dir()
            .map(|d| d.join(APP_ID))
            .ok_or(Error::NoDataDir),
    }
}

/// Folders searched for the accounts of the Electron builds (1.0.x), in order.
pub fn legacy_config_dir() -> Option<PathBuf> {
    match test_dir() {
        Some(dir) => Some(dir.join("electron")),
        None => dirs::config_dir(),
    }
}

/// Keychain service holding the access tokens (same as the Tauri app).
pub fn keychain_service() -> &'static str {
    if is_test() {
        "Orphyn (test)"
    } else {
        "Orphyn"
    }
}
