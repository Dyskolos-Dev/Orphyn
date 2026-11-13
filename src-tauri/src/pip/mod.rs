//! Picture-in-picture / mini player window. STUB: filled by the PiP migration task. Contract:
//! `src/shared/ipc/pip.ts`. Event to emit: `orphyn:pip:state-changed` with a `PipState`.
#![allow(unused_variables)]

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::error::{Error, Result};

pub fn init<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn pip_get_state() -> Result<Value> {
    Err(Error::NotImplemented("pip_get_state"))
}

/// `kind` is `"music"` or `"video"`.
#[tauri::command]
pub fn pip_open(kind: String) -> Result<()> {
    Err(Error::NotImplemented("pip_open"))
}

#[tauri::command]
pub fn pip_close() -> Result<()> {
    Err(Error::NotImplemented("pip_close"))
}
