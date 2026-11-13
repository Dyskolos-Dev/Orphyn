//! Access tokens in the OS keychain: Windows Credential Manager, macOS Keychain, Linux Secret
//! Service. One entry per account, service `Orphyn`, user = the account id.

use keyring::{Entry, Error as KeyringError};

use super::store::SecretStore;

const SERVICE: &str = "Orphyn";
const PROBE_USER: &str = "__orphyn_probe__";

pub struct Keychain {
    available: bool,
}

impl Keychain {
    /// Probes the keychain with a read (nothing is written): a missing entry means it works.
    pub fn detect() -> Self {
        let available = match Entry::new(SERVICE, PROBE_USER).and_then(|e| e.get_password()) {
            Ok(_) | Err(KeyringError::NoEntry) => true,
            Err(e) => {
                log::warn!("[accounts] OS keychain unavailable: {e}");
                false
            }
        };
        Self { available }
    }

    fn entry(id: &str) -> Result<Entry, String> {
        Entry::new(SERVICE, id).map_err(|e| e.to_string())
    }
}

impl SecretStore for Keychain {
    fn available(&self) -> bool {
        self.available
    }

    fn get(&self, id: &str) -> Result<Option<String>, String> {
        match Self::entry(id)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, id: &str, token: &str) -> Result<(), String> {
        Self::entry(id)?
            .set_password(token)
            .map_err(|e| e.to_string())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        match Self::entry(id)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
