//! Casting to Chromecast, UPnP/DLNA (WiiM) and AirPlay receivers. STUB: filled by the cast
//! migration task. Contract: `src/shared/ipc/cast.ts`; the payload types live in
//! `src/shared/cast/model.ts`. Events to emit (same names as the Electron channels):
//! `cast:devices-changed`, `cast:session-changed`, `cast:ended`, `cast:error`.
#![allow(unused_variables)]

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::error::{Error, Result};

pub fn init<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn cast_start_discovery() -> Result<()> {
    Err(Error::NotImplemented("cast_start_discovery"))
}

#[tauri::command]
pub fn cast_stop_discovery() -> Result<()> {
    Err(Error::NotImplemented("cast_stop_discovery"))
}

#[tauri::command]
pub fn cast_list_devices() -> Result<Vec<Value>> {
    Err(Error::NotImplemented("cast_list_devices"))
}

#[tauri::command]
pub fn cast_get_session(device_id: String) -> Result<Option<Value>> {
    Err(Error::NotImplemented("cast_get_session"))
}

#[tauri::command]
pub fn cast_load(device_id: String, media: Value) -> Result<()> {
    Err(Error::NotImplemented("cast_load"))
}

#[tauri::command]
pub fn cast_play(device_id: String) -> Result<()> {
    Err(Error::NotImplemented("cast_play"))
}

#[tauri::command]
pub fn cast_pause(device_id: String) -> Result<()> {
    Err(Error::NotImplemented("cast_pause"))
}

#[tauri::command]
pub fn cast_seek(device_id: String, position_sec: f64) -> Result<()> {
    Err(Error::NotImplemented("cast_seek"))
}

#[tauri::command]
pub fn cast_stop(device_id: String) -> Result<()> {
    Err(Error::NotImplemented("cast_stop"))
}

#[tauri::command]
pub fn cast_set_volume(device_id: String, volume: f64) -> Result<()> {
    Err(Error::NotImplemented("cast_set_volume"))
}

#[tauri::command]
pub fn cast_set_muted(device_id: String, muted: bool) -> Result<()> {
    Err(Error::NotImplemented("cast_set_muted"))
}

#[tauri::command]
pub fn cast_disconnect(device_id: String) -> Result<()> {
    Err(Error::NotImplemented("cast_disconnect"))
}
