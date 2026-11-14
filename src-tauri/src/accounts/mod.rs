//! Saved servers and accounts (contract: `src/shared/ipc/accounts.ts`). Tokens never leave this
//! module except through `accounts_get_token`.

mod keychain;
pub mod migrate;
pub mod store;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime, State};

use crate::error::{Error, Result};
use store::{AccountStore, AccountsStatus, NewServerAccount, ServerAccount};

pub struct Accounts(Mutex<AccountStore>);

impl Accounts {
    fn with<T>(&self, f: impl FnOnce(&mut AccountStore) -> Result<T>) -> Result<T> {
        let mut store = self
            .0
            .lock()
            .map_err(|_| Error::Other("accounts store poisoned".into()))?;
        f(&mut store)
    }
}

/// Opens the store (importing the Electron servers on first start). Must run before the window
/// loads: the renderer asks for the accounts as soon as it starts.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let path = crate::profile::data_dir(app)?.join("accounts.json");
    let seed = if path.exists() {
        None
    } else {
        migrate::import_legacy(&migrate::legacy_account_files()).map(|(from, file)| {
            log::info!(
                "[accounts] imported {} server(s) from {} (sign-in needed)",
                file.accounts.len(),
                from.display()
            );
            file
        })
    };
    let store = AccountStore::open(path, Box::new(keychain::Keychain::detect()), seed);
    if let Some(warning) = store.status().warning {
        log::warn!("[accounts] {warning}");
    }
    app.manage(Accounts(Mutex::new(store)));
    Ok(())
}

// Commands are async so keychain calls never block the main (UI) thread.

#[tauri::command]
pub async fn accounts_list(state: State<'_, Accounts>) -> Result<Vec<ServerAccount>> {
    state.with(|s| Ok(s.list()))
}

#[tauri::command]
pub async fn accounts_add(
    state: State<'_, Accounts>,
    input: NewServerAccount,
) -> Result<ServerAccount> {
    state.with(|s| s.add(input))
}

#[tauri::command]
pub async fn accounts_rename(
    state: State<'_, Accounts>,
    id: String,
    name: String,
) -> Result<Option<ServerAccount>> {
    state.with(|s| s.rename(&id, &name))
}

#[tauri::command]
pub async fn accounts_remove(state: State<'_, Accounts>, id: String) -> Result<()> {
    state.with(|s| s.remove(&id))
}

#[tauri::command]
pub async fn accounts_set_active(
    state: State<'_, Accounts>,
    id: Option<String>,
) -> Result<Option<ServerAccount>> {
    state.with(|s| s.set_active(id.as_deref()))
}

#[tauri::command]
pub async fn accounts_get_active(state: State<'_, Accounts>) -> Result<Option<ServerAccount>> {
    state.with(|s| Ok(s.active()))
}

#[tauri::command]
pub async fn accounts_get_token(state: State<'_, Accounts>, id: String) -> Result<Option<String>> {
    state.with(|s| Ok(s.token(&id)))
}

#[tauri::command]
pub async fn accounts_get_device_id(state: State<'_, Accounts>) -> Result<String> {
    state.with(|s| Ok(s.device_id().to_string()))
}

#[tauri::command]
pub async fn accounts_get_status(state: State<'_, Accounts>) -> Result<AccountsStatus> {
    state.with(|s| Ok(s.status()))
}
