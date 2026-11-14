//! Discord Rich Presence. Contract: `src/shared/ipc/discord.ts` (see `docs/DISCORD.md`).
//!
//! `activity.rs` builds the activity (artwork rules, timestamps), `service.rs` is the testable
//! state machine (lazy connect, rate limit, quiet reconnect) and this file runs it on one
//! worker thread, owns the real IPC client, persists the settings and emits
//! `orphyn:discord:state-changed`.

mod activity;
mod service;

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use discord_rich_presence::activity::{Activity as RpcActivity, ActivityType, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::error::{Error, Result};
use activity::{Activity, Presence};
use service::{Core, Rpc, Settings, SettingsPatch, State};

pub const STATE_CHANGED: &str = "orphyn:discord:state-changed";
const SETTINGS_FILE: &str = "discord.json";

/// The real client: a `discord-rich-presence` IPC connection.
struct IpcRpc {
    client: Option<DiscordIpcClient>,
}

impl Rpc for IpcRpc {
    fn connect(&mut self) -> std::result::Result<(), String> {
        let mut client = DiscordIpcClient::new(service::CLIENT_ID);
        client.connect().map_err(|e| e.to_string())?;
        self.client = Some(client);
        Ok(())
    }

    fn set_activity(&mut self, a: &Activity) -> std::result::Result<(), String> {
        let client = self.client.as_mut().ok_or("not connected")?;
        let mut assets = Assets::new()
            .large_image(&a.large_image)
            .large_text(&a.large_text)
            .large_url(a.link)
            .small_image(&a.small_image)
            .small_text(&a.small_text)
            .small_url(a.link);
        if a.large_image.is_empty() {
            assets = Assets::new();
        }
        let mut act = RpcActivity::new()
            .details(&a.details)
            .assets(assets)
            .activity_type(match a.kind {
                activity::ACTIVITY_WATCHING => ActivityType::Watching,
                _ => ActivityType::Listening,
            });
        if let Some(state) = &a.state {
            act = act.state(state);
        }
        if let Some(start) = a.start {
            let mut ts = Timestamps::new().start(start);
            if let Some(end) = a.end {
                ts = ts.end(end);
            }
            act = act.timestamps(ts);
        }
        client.set_activity(act).map_err(|e| e.to_string())
    }

    fn clear_activity(&mut self) -> std::result::Result<(), String> {
        let client = self.client.as_mut().ok_or("not connected")?;
        client.clear_activity().map_err(|e| e.to_string())
    }

    fn close(&mut self) {
        if let Some(mut c) = self.client.take() {
            let _ = c.close();
        }
    }
}

enum Msg {
    Presence(Option<Presence>),
    Settings(SettingsPatch),
    Poke,
    Shutdown,
}

/// Managed state: the channel to the worker and the last state it published.
struct Handle {
    tx: Sender<Msg>,
    state: Mutex<State>,
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    crate::profile::data_dir(app)
        .ok()
        .map(|d| d.join(SETTINGS_FILE))
}

fn load_settings(path: Option<&std::path::Path>) -> Settings {
    let raw = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<SettingsPatch>(&s).ok())
        .unwrap_or_default();
    Settings {
        enabled: raw.enabled != Some(false),
        show_artwork: raw.show_artwork != Some(false),
    }
}

fn save_settings(path: Option<&std::path::Path>, settings: &Settings) {
    let Some(path) = path else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Read-only profile: the choice applies for this session.
    let _ = serde_json::to_string(settings).map(|s| std::fs::write(path, s));
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn run_worker<R: Runtime>(
    app: AppHandle<R>,
    rx: mpsc::Receiver<Msg>,
    path: Option<std::path::PathBuf>,
) {
    let settings = load_settings(path.as_deref());
    let mut core = Core::new(
        service::is_configured(service::CLIENT_ID),
        settings,
        Box::new(|| IpcRpc { client: None }),
    );
    let publish = |core: &Core<IpcRpc>, last: &mut State| {
        let state = core.state();
        if state != *last {
            *last = state;
            if let Some(h) = app.try_state::<Handle>() {
                *h.state.lock().unwrap_or_else(|p| p.into_inner()) = state;
            }
            let _ = app.emit(STATE_CHANGED, state);
        }
    };
    let mut last = core.state();
    loop {
        let now = now_ms();
        let wait = core
            .next_deadline(now)
            .map(|t| Duration::from_millis((t - now).max(0) as u64))
            .unwrap_or(Duration::from_secs(3600));
        match rx.recv_timeout(wait) {
            Ok(Msg::Presence(p)) => core.set_presence(p, now_ms()),
            Ok(Msg::Settings(patch)) => {
                core.set_settings(patch, now_ms());
                save_settings(path.as_deref(), &core.settings());
            }
            Ok(Msg::Poke) => core.poke(),
            Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                core.dispose();
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        core.tick(now_ms());
        publish(&core, &mut last);
    }
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let path = settings_path(app);
    let initial = load_settings(path.as_deref());
    app.manage(Handle {
        tx,
        state: Mutex::new(State {
            enabled: initial.enabled,
            show_artwork: initial.show_artwork,
            status: if service::is_configured(service::CLIENT_ID) {
                if initial.enabled {
                    service::Status::Idle
                } else {
                    service::Status::Disabled
                }
            } else {
                service::Status::NotConfigured
            },
        }),
    });
    let handle = app.clone();
    std::thread::Builder::new()
        .name("discord-presence".into())
        .spawn(move || run_worker(handle, rx, path))?;
    Ok(())
}

/// Clears the activity when the app exits.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(h) = app.try_state::<Handle>() {
        let _ = h.tx.send(Msg::Shutdown);
    }
}

fn handle<R: Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, Handle>> {
    app.try_state::<Handle>()
        .ok_or_else(|| Error::Other("discord not initialised".into()))
}

#[tauri::command]
pub fn discord_get_state(app: AppHandle) -> Result<State> {
    let h = handle(&app)?;
    let _ = h.tx.send(Msg::Poke);
    let state = *h.state.lock().unwrap_or_else(|p| p.into_inner());
    Ok(state)
}

/// `presence` is a `DiscordPresence`, or `null` to clear the activity. Anything malformed clears.
#[tauri::command]
pub fn discord_set_presence(app: AppHandle, presence: Option<Value>) -> Result<()> {
    let parsed = presence.and_then(|v| serde_json::from_value::<Presence>(v).ok());
    let _ = handle(&app)?.tx.send(Msg::Presence(parsed));
    Ok(())
}

/// `settings` is a `Partial<DiscordSettings>`.
#[tauri::command]
pub fn discord_set_settings(app: AppHandle, settings: Value) -> Result<()> {
    let patch = serde_json::from_value::<SettingsPatch>(settings).unwrap_or_default();
    let _ = handle(&app)?.tx.send(Msg::Settings(patch));
    Ok(())
}
