//! OS media controls (Windows SMTC, macOS Now Playing, Linux MPRIS) through `souvlaki`. STUB:
//! filled by the media-controls migration task. Contract: `src/shared/ipc/mediaControls.ts`.
//! Event to emit: `orphyn:media-controls:action` with a `MediaControlAction`.
#![allow(unused_variables)]

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::error::{Error, Result};

pub fn init<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Ok(())
}

/// `metadata` is a `MediaControlsMetadata`.
#[tauri::command]
pub fn media_controls_set_metadata(metadata: Value) -> Result<()> {
    Err(Error::NotImplemented("media_controls_set_metadata"))
}

/// `playback` is a `MediaControlsPlayback`.
#[tauri::command]
pub fn media_controls_set_playback(playback: Value) -> Result<()> {
    Err(Error::NotImplemented("media_controls_set_playback"))
}

#[tauri::command]
pub fn media_controls_clear() -> Result<()> {
    Err(Error::NotImplemented("media_controls_clear"))
}
