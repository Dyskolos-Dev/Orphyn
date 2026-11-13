//! Discord Rich Presence. STUB: filled by the Discord migration task. Contract:
//! `src/shared/ipc/discord.ts` (see `docs/DISCORD.md`). Event to emit:
//! `orphyn:discord:state-changed` with a `DiscordState`.
#![allow(unused_variables)]

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::error::{Error, Result};

pub fn init<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Ok(())
}

#[tauri::command]
pub fn discord_get_state() -> Result<Value> {
    Err(Error::NotImplemented("discord_get_state"))
}

/// `presence` is a `DiscordPresence`, or `null` to clear the activity.
#[tauri::command]
pub fn discord_set_presence(presence: Option<Value>) -> Result<()> {
    Err(Error::NotImplemented("discord_set_presence"))
}

/// `settings` is a `Partial<DiscordSettings>`.
#[tauri::command]
pub fn discord_set_settings(settings: Value) -> Result<()> {
    Err(Error::NotImplemented("discord_set_settings"))
}
