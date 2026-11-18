//! Saved servers and accounts: the same `accounts.json` (in the same folder) and the same keychain
//! entries as the Tauri app, so a 1.5 profile opens as is. `store`, `keychain` and `migrate` are
//! copied from `src-tauri/src/accounts`.
mod keychain;
pub mod migrate;
pub mod store;

pub use keychain::Keychain;
pub use store::{AccountStore, AccountsStatus, NewServerAccount, ServerAccount};

use crate::error::Result;

/// Opens the account store of the current profile (`<data dir>/accounts.json`), importing the
/// Electron servers on first start exactly like the Tauri app does.
pub fn open_default() -> Result<AccountStore> {
    let path = crate::profile::data_dir()?.join("accounts.json");
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
    let store = AccountStore::open(path, Box::new(Keychain::detect()), seed);
    if let Some(warning) = store.status().warning {
        log::warn!("[accounts] {warning}");
    }
    Ok(store)
}
