//! In-app updates from the GitHub release `latest.json` (contract: `src/shared/ipc/updater.ts`).
//! Same behaviour as the Electron app: a check 10 s after launch then every 6 h when automatic
//! updates are on, a background download where the build can install itself, then "Restart";
//! elsewhere the user is sent to the release page. A downloaded update also installs on quit.

pub mod state;

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{Error, Result};
use state::{
    can_apply_in_app, release_url, UpdaterState, CHECK_INTERVAL_SECS, FIRST_CHECK_DELAY_SECS,
};

pub const STATE_CHANGED: &str = "orphyn:updater:state-changed";
const SETTINGS_FILE: &str = "updater.json";

pub struct Updater {
    state: Mutex<UpdaterState>,
    /// The downloaded update, installed by "Restart" or on quit.
    pending: Mutex<Option<(Update, Vec<u8>)>>,
    can_apply: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    auto_update: bool,
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(SETTINGS_FILE))
}

fn load_auto_update<R: Runtime>(app: &AppHandle<R>) -> bool {
    settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
        .is_none_or(|s| s.auto_update)
}

fn save_auto_update<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let Some(path) = settings_path(app) else {
        return;
    };
    let json = serde_json::json!({ "autoUpdate": enabled }).to_string();
    // A read-only profile must not break the app: the choice then applies for this session.
    if let Err(e) = crate::accounts::store::write_atomic(&path, &json) {
        log::warn!("[updater] could not save the setting: {e}");
    }
}

impl Updater {
    fn snapshot(&self) -> UpdaterState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Applies `f`; when it returns true the new state is pushed to the renderer.
    fn update<R: Runtime>(&self, app: &AppHandle<R>, f: impl FnOnce(&mut UpdaterState) -> bool) {
        let next = {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if !f(&mut s) {
                return;
            }
            s.clone()
        };
        let _ = app.emit(STATE_CHANGED, next);
    }
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let appimage = std::env::var("APPIMAGE").ok();
    let can_apply = can_apply_in_app(std::env::consts::OS, appimage.as_deref());
    // Only release builds check: a dev build has no matching release.
    let supported = !cfg!(debug_assertions);
    let state = UpdaterState::new(
        app.package_info().version.to_string(),
        load_auto_update(app),
        supported,
    );
    app.manage(Updater {
        state: Mutex::new(state),
        pending: Mutex::new(None),
        can_apply,
    });
    if supported {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(FIRST_CHECK_DELAY_SECS)).await;
            loop {
                if app.state::<Updater>().snapshot().auto_update {
                    check(&app).await;
                }
                tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
            }
        });
    }
    Ok(())
}

/// Manual or scheduled check. Ignored while a check or download runs.
async fn check<R: Runtime>(app: &AppHandle<R>) {
    let me = app.state::<Updater>();
    let mut started = false;
    me.update(app, |s| {
        started = s.can_check();
        if started {
            s.checking();
        }
        started
    });
    if !started {
        return;
    }
    if let Err(e) = run_check(app, &me).await {
        log::warn!("[updater] {e}");
        me.update(app, |s| {
            s.failed(e.to_string());
            true
        });
    }
}

async fn run_check<R: Runtime>(app: &AppHandle<R>, me: &Updater) -> Result<()> {
    let found = app
        .updater()
        .map_err(|e| Error::Other(e.to_string()))?
        .check()
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    let Some(update) = found else {
        me.update(app, |s| {
            s.up_to_date();
            true
        });
        return Ok(());
    };
    let version = update.version.clone();
    me.update(app, |s| {
        s.found(version, me.can_apply);
        true
    });
    if !me.can_apply {
        return Ok(());
    }
    let mut downloaded: u64 = 0;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                me.update(app, |s| s.progress(downloaded, total));
            },
            || {},
        )
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    *me.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some((update, bytes));
    me.update(app, |s| {
        s.ready();
        true
    });
    Ok(())
}

/// Installs a downloaded update when the app quits (the Electron app did the same).
pub fn install_on_exit<R: Runtime>(app: &AppHandle<R>) {
    let Some(me) = app.try_state::<Updater>() else {
        return;
    };
    let pending = me.pending.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some((update, bytes)) = pending {
        if let Err(e) = update.install(bytes) {
            log::warn!("[updater] install on exit failed: {e}");
        }
    }
}

#[tauri::command]
pub fn updater_get_state(state: State<'_, Updater>) -> UpdaterState {
    state.snapshot()
}

#[tauri::command]
pub async fn updater_check<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    check(&app).await;
    Ok(())
}

/// Installs the downloaded update and restarts (on Windows the installer closes the app).
#[tauri::command]
pub async fn updater_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Updater>,
) -> Result<()> {
    let pending = state
        .pending
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let Some((update, bytes)) = pending else {
        return Ok(());
    };
    update
        .install(bytes)
        .map_err(|e| Error::Other(e.to_string()))?;
    app.restart()
}

#[tauri::command]
pub fn updater_open_release<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Updater>,
) -> Result<()> {
    let url = release_url(state.snapshot().version.as_deref());
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))
}

#[tauri::command]
pub fn updater_set_auto_update<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Updater>,
    enabled: bool,
) {
    save_auto_update(&app, enabled);
    state.update(&app, |s| {
        s.auto_update = enabled;
        true
    });
}
