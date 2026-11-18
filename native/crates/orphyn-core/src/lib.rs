//! Framework-free logic of Orphyn native.
//!
//! The modules marked "copied" are copies of `src-tauri/src` (the Tauri app keeps shipping and is
//! not modified): keep them in step by hand until the Tauri app is retired. Only the imports were
//! adapted (`crate::error`, `crate::profile`) and the `#[tauri::command]` attributes removed.
pub mod accounts;
/// Copied from `src-tauri/src/artwork` (Deezer artist images, top tracks, related artists).
pub mod artwork;
pub mod discord {
    //! Copied from `src-tauri/src/discord/activity.rs`; the IPC service is ported with the Discord
    //! feature.
    pub mod activity;
}
pub mod error;
pub mod profile;
pub mod storage;
pub mod updater {
    //! Copied from `src-tauri/src/updater/state.rs`; download and install are ported with the
    //! updater feature.
    pub mod state;
}
