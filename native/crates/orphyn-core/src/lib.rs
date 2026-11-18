//! Framework-free logic of Orphyn native.
//!
//! The modules marked "copied" are copies of `src-tauri/src` (the Tauri app keeps shipping and is
//! not modified): keep them in step by hand until the Tauri app is retired. Only the imports were
//! adapted (`crate::error`, `crate::profile`) and the `#[tauri::command]` attributes removed.
pub mod accounts;
/// Copied from `src-tauri/src/artwork` (Deezer artist images, top tracks, related artists).
pub mod artwork;
/// Copied from `src-tauri/src/discord`, with the IPC worker as a framework-free service.
pub mod discord;
/// Copied from `src-tauri/src/cast` (discovery, Chromecast, UPnP/DLNA, AirPlay, sessions).
pub mod cast;
pub mod error;
pub mod profile;
pub mod storage;
/// OS media controls (souvlaki), ported from `src-tauri/src/media_controls`.
pub mod media_controls;
/// State machine copied from `src-tauri/src/updater`; manifest check, download, minisign
/// verification and install written without Tauri.
pub mod updater;
