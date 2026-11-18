//! Discord Rich Presence: `activity` builds the activity, `service` is the testable state machine
//! (copied from `src-tauri/src/discord`), and [`DiscordService`] runs it on one worker thread
//! with the real IPC client. Settings live in `discord.json` of the data directory.

pub mod activity;
pub mod service;

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use discord_rich_presence::activity::{Activity as RpcActivity, ActivityType, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use activity::{Activity, Presence};
use service::{Core, Rpc, Settings, SettingsPatch, State};

const SETTINGS_FILE: &str = "discord.json";

struct IpcRpc {
    client: Option<DiscordIpcClient>,
}

impl Rpc for IpcRpc {
    fn connect(&mut self) -> Result<(), String> {
        let mut client = DiscordIpcClient::new(service::CLIENT_ID);
        client.connect().map_err(|e| e.to_string())?;
        self.client = Some(client);
        Ok(())
    }

    fn set_activity(&mut self, a: &Activity) -> Result<(), String> {
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

    fn clear_activity(&mut self) -> Result<(), String> {
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

/// Cheap to clone; the worker stops when [`DiscordService::shutdown`] is called.
#[derive(Clone)]
pub struct DiscordService {
    tx: Sender<Msg>,
    state: Arc<Mutex<State>>,
}

fn settings_path() -> Option<PathBuf> {
    crate::profile::data_dir().ok().map(|d| d.join(SETTINGS_FILE))
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
    let _ = serde_json::to_string(settings).map(|s| std::fs::write(path, s));
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn run_worker(rx: mpsc::Receiver<Msg>, shared: Arc<Mutex<State>>, path: Option<PathBuf>) {
    let mut core = Core::new(
        service::is_configured(service::CLIENT_ID),
        load_settings(path.as_deref()),
        Box::new(|| IpcRpc { client: None }),
    );
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
        *shared.lock().unwrap_or_else(|p| p.into_inner()) = core.state();
    }
}

impl DiscordService {
    pub fn start() -> Self {
        let path = settings_path();
        let initial = load_settings(path.as_deref());
        let configured = service::is_configured(service::CLIENT_ID);
        let status = match (configured, initial.enabled) {
            (false, _) => service::Status::NotConfigured,
            (true, false) => service::Status::Disabled,
            (true, true) => service::Status::Idle,
        };
        let state = Arc::new(Mutex::new(State {
            enabled: initial.enabled,
            show_artwork: initial.show_artwork,
            status,
        }));
        let (tx, rx) = mpsc::channel();
        let shared = state.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run_worker(rx, shared, path))
        {
            log::warn!("[discord] worker not started: {e}");
        }
        Self { tx, state }
    }

    /// The last state the worker published (also nudges it to look for Discord).
    pub fn state(&self) -> State {
        let _ = self.tx.send(Msg::Poke);
        *self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `None` clears the activity.
    pub fn set_presence(&self, presence: Option<Presence>) {
        let _ = self.tx.send(Msg::Presence(presence));
    }

    pub fn set_settings(&self, patch: SettingsPatch) {
        let _ = self.tx.send(Msg::Settings(patch));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Msg::Shutdown);
    }
}
