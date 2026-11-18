//! Saved servers and accounts. Same JSON file as the Electron version (`accounts.json`, version 1,
//! camelCase), except that `accessToken` is always written as `null`: tokens live in the OS
//! keychain ([`SecretStore`]), or in memory only when there is no keychain.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const NO_KEYCHAIN_WARNING: &str = "Secure storage is unavailable on this system: access tokens are kept in memory only and you will have to sign in again after restarting Orphyn.";

/// Where access tokens are kept, keyed by account id. Injectable for tests.
pub trait SecretStore: Send + Sync {
    /// False when the OS has no usable keychain: tokens then stay in memory only.
    fn available(&self) -> bool;
    fn get(&self, id: &str) -> std::result::Result<Option<String>, String>;
    fn set(&self, id: &str, token: &str) -> std::result::Result<(), String>;
    fn delete(&self, id: &str) -> std::result::Result<(), String>;
}

/// On-disk record. `accessToken` is kept in the model for compatibility and is never filled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAccount {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub server_id: String,
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub added_at: String,
    #[serde(default)]
    pub last_used_at: String,
    #[serde(default)]
    pub access_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFile {
    #[serde(default = "version_one")]
    pub version: u32,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub active_id: Option<String>,
    #[serde(default)]
    pub accounts: Vec<StoredAccount>,
}

fn version_one() -> u32 {
    1
}

/// `ServerAccount` of `src/shared/jellyfin/index.ts`: never carries the token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAccount {
    pub id: String,
    pub name: String,
    pub url: String,
    pub server_id: String,
    pub server_name: String,
    pub version: String,
    pub user_id: String,
    pub user_name: String,
    pub added_at: String,
    pub last_used_at: String,
    pub has_token: bool,
}

/// `NewServerAccount` of `src/shared/jellyfin/index.ts`. Passwords are never passed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewServerAccount {
    pub name: String,
    pub url: String,
    pub server_id: String,
    pub server_name: String,
    pub version: String,
    pub user_id: String,
    pub user_name: String,
    pub access_token: String,
}

/// `AccountsStatus` of `src/shared/jellyfin/index.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsStatus {
    pub encrypted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

pub struct AccountStore {
    path: PathBuf,
    secrets: Box<dyn SecretStore>,
    data: StoredFile,
    /// Clear tokens, in memory only: the authoritative source for `get_token`.
    tokens: HashMap<String, String>,
    now: Box<dyn Fn() -> String + Send + Sync>,
    uuid: Box<dyn Fn() -> String + Send + Sync>,
}

pub fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl AccountStore {
    /// Opens `path`. When it does not exist yet, `seed` (the accounts imported from the Electron
    /// app) becomes the initial content. A corrupt file starts clean, like the Electron store.
    pub fn open(path: PathBuf, secrets: Box<dyn SecretStore>, seed: Option<StoredFile>) -> Self {
        Self::open_with(path, secrets, seed, Box::new(iso_now), Box::new(new_uuid))
    }

    pub fn open_with(
        path: PathBuf,
        secrets: Box<dyn SecretStore>,
        seed: Option<StoredFile>,
        now: Box<dyn Fn() -> String + Send + Sync>,
        uuid: Box<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        let mut store = Self {
            path,
            secrets,
            data: StoredFile {
                version: 1,
                device_id: String::new(),
                active_id: None,
                accounts: Vec::new(),
            },
            tokens: HashMap::new(),
            now,
            uuid,
        };
        store.data.device_id = (store.uuid)();
        store.load(seed);
        store
    }

    pub fn encrypted(&self) -> bool {
        self.secrets.available()
    }

    pub fn status(&self) -> AccountsStatus {
        if self.encrypted() {
            AccountsStatus {
                encrypted: true,
                warning: None,
            }
        } else {
            AccountsStatus {
                encrypted: false,
                warning: Some(NO_KEYCHAIN_WARNING.to_string()),
            }
        }
    }

    pub fn device_id(&self) -> &str {
        &self.data.device_id
    }

    pub fn list(&self) -> Vec<ServerAccount> {
        self.data
            .accounts
            .iter()
            .map(|a| self.to_public(a))
            .collect()
    }

    pub fn active(&self) -> Option<ServerAccount> {
        let id = self.data.active_id.as_deref()?;
        self.data
            .accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| self.to_public(a))
    }

    pub fn token(&self, id: &str) -> Option<String> {
        self.tokens.get(id).cloned()
    }

    /// Adds an account; the same server+user is updated in place (new token, url, name).
    pub fn add(&mut self, input: NewServerAccount) -> Result<ServerAccount> {
        let now = (self.now)();
        let idx = self
            .data
            .accounts
            .iter()
            .position(|a| a.server_id == input.server_id && a.user_id == input.user_id);
        let existing = idx.map(|i| self.data.accounts[i].clone());
        let trimmed = input.name.trim();
        let name = if !trimmed.is_empty() {
            trimmed.to_string()
        } else if let Some(e) = existing.as_ref().filter(|e| !e.name.is_empty()) {
            // Signing in again without a name keeps the one the user chose.
            e.name.clone()
        } else {
            input.server_name.clone()
        };
        let stored = StoredAccount {
            id: existing
                .as_ref()
                .map(|e| e.id.clone())
                .unwrap_or_else(|| (self.uuid)()),
            name,
            url: input.url,
            server_id: input.server_id,
            server_name: input.server_name,
            version: input.version,
            user_id: input.user_id,
            user_name: input.user_name,
            added_at: existing
                .as_ref()
                .map(|e| e.added_at.clone())
                .unwrap_or_else(|| now.clone()),
            last_used_at: now,
            access_token: None,
        };
        if self.secrets.available() {
            if let Err(e) = self.secrets.set(&stored.id, &input.access_token) {
                log::warn!("[accounts] could not save the token in the keychain: {e}");
            }
        }
        self.tokens
            .insert(stored.id.clone(), input.access_token.clone());
        match idx {
            Some(i) => self.data.accounts[i] = stored.clone(),
            None => self.data.accounts.push(stored.clone()),
        }
        if self.data.active_id.is_none() {
            self.data.active_id = Some(stored.id.clone());
        }
        self.save()?;
        Ok(self.to_public(&stored))
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<Option<ServerAccount>> {
        let Some(a) = self.data.accounts.iter_mut().find(|a| a.id == id) else {
            return Ok(None);
        };
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            a.name = trimmed.to_string();
        }
        let a = a.clone();
        self.save()?;
        Ok(Some(self.to_public(&a)))
    }

    /// Forgets an account. When it was the active one, the most recently used remaining account
    /// takes over (only removing the last one leads back to onboarding).
    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.data.accounts.retain(|a| a.id != id);
        self.tokens.remove(id);
        if self.secrets.available() {
            if let Err(e) = self.secrets.delete(id) {
                log::warn!("[accounts] could not delete the token from the keychain: {e}");
            }
        }
        if self.data.active_id.as_deref() == Some(id) {
            self.data.active_id = self
                .data
                .accounts
                .iter()
                .max_by(|a, b| a.last_used_at.cmp(&b.last_used_at))
                .map(|a| a.id.clone());
        }
        self.save()
    }

    pub fn set_active(&mut self, id: Option<&str>) -> Result<Option<ServerAccount>> {
        let Some(id) = id else {
            self.data.active_id = None;
            self.save()?;
            return Ok(None);
        };
        let now = (self.now)();
        let Some(a) = self.data.accounts.iter_mut().find(|a| a.id == id) else {
            return Ok(None);
        };
        a.last_used_at = now;
        let a = a.clone();
        self.data.active_id = Some(a.id.clone());
        self.save()?;
        Ok(Some(self.to_public(&a)))
    }

    fn to_public(&self, a: &StoredAccount) -> ServerAccount {
        ServerAccount {
            id: a.id.clone(),
            name: a.name.clone(),
            url: a.url.clone(),
            server_id: a.server_id.clone(),
            server_name: a.server_name.clone(),
            version: a.version.clone(),
            user_id: a.user_id.clone(),
            user_name: a.user_name.clone(),
            added_at: a.added_at.clone(),
            last_used_at: a.last_used_at.clone(),
            has_token: self.tokens.contains_key(&a.id),
        }
    }

    fn load(&mut self, seed: Option<StoredFile>) {
        let parsed = if self.path.exists() {
            fs::read_to_string(&self.path)
                .ok()
                .and_then(|raw| serde_json::from_str::<StoredFile>(&raw).ok())
        } else {
            seed
        };
        if let Some(mut file) = parsed {
            file.version = 1;
            if file.device_id.is_empty() {
                file.device_id = self.data.device_id.clone();
            }
            for a in &mut file.accounts {
                // Never trust a token found in the file: tokens only come from the keychain.
                a.access_token = None;
            }
            if let Some(active) = file.active_id.clone() {
                if !file.accounts.iter().any(|a| a.id == active) {
                    file.active_id = None;
                }
            }
            self.data = file;
        }
        if self.secrets.available() {
            for a in &self.data.accounts {
                match self.secrets.get(&a.id) {
                    Ok(Some(token)) => {
                        self.tokens.insert(a.id.clone(), token);
                    }
                    Ok(None) => {}
                    Err(e) => log::warn!("[accounts] keychain read failed for {}: {e}", a.id),
                }
            }
        }
        if let Err(e) = self.save() {
            log::warn!("[accounts] could not write {}: {e}", self.path.display());
        }
    }

    fn save(&self) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_string_pretty(&self.data)?)
    }
}

/// Writes through a temporary file then renames it, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// In-memory keychain shared between store instances (simulates a restart).
    #[derive(Clone, Default)]
    pub struct FakeKeychain {
        pub entries: Arc<Mutex<HashMap<String, String>>>,
        pub unavailable: bool,
    }

    impl SecretStore for FakeKeychain {
        fn available(&self) -> bool {
            !self.unavailable
        }
        fn get(&self, id: &str) -> std::result::Result<Option<String>, String> {
            Ok(self.entries.lock().unwrap().get(id).cloned())
        }
        fn set(&self, id: &str, token: &str) -> std::result::Result<(), String> {
            self.entries
                .lock()
                .unwrap()
                .insert(id.to_string(), token.to_string());
            Ok(())
        }
        fn delete(&self, id: &str) -> std::result::Result<(), String> {
            self.entries.lock().unwrap().remove(id);
            Ok(())
        }
    }

    fn open(path: &Path, keychain: &FakeKeychain, seed: Option<StoredFile>) -> AccountStore {
        let counter = Arc::new(AtomicUsize::new(0));
        let clock = Arc::new(AtomicUsize::new(0));
        AccountStore::open_with(
            path.to_path_buf(),
            Box::new(keychain.clone()),
            seed,
            Box::new(move || {
                let n = clock.fetch_add(1, Ordering::SeqCst);
                format!("2026-01-01T00:00:{n:02}.000Z")
            }),
            Box::new(move || format!("uuid-{}", counter.fetch_add(1, Ordering::SeqCst))),
        )
    }

    pub fn input(server: &str, user: &str, token: &str) -> NewServerAccount {
        NewServerAccount {
            name: String::new(),
            url: format!("https://{server}.example"),
            server_id: server.to_string(),
            server_name: format!("{server} name"),
            version: "10.10.0".to_string(),
            user_id: user.to_string(),
            user_name: user.to_string(),
            access_token: token.to_string(),
        }
    }

    #[test]
    fn add_persists_without_the_token_and_restores_it_from_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let keychain = FakeKeychain::default();
        let mut store = open(&path, &keychain, None);
        let a = store.add(input("s1", "u1", "secret-token")).unwrap();
        assert!(a.has_token);
        assert_eq!(a.name, "s1 name");
        assert_eq!(store.active().unwrap().id, a.id);

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("secret-token"));
        assert!(raw.contains("\"accessToken\": null"));
        assert!(raw.contains("\"deviceId\""));

        let reopened = open(&path, &keychain, None);
        assert_eq!(reopened.token(&a.id).as_deref(), Some("secret-token"));
        assert_eq!(reopened.device_id(), store.device_id());
        assert!(reopened.list()[0].has_token);
    }

    #[test]
    fn same_server_and_user_is_updated_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let keychain = FakeKeychain::default();
        let mut store = open(&dir.path().join("accounts.json"), &keychain, None);
        let first = store.add(input("s1", "u1", "t1")).unwrap();
        store.rename(&first.id, "  Home  ").unwrap();
        let again = store.add(input("s1", "u1", "t2")).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(again.name, "Home");
        assert_eq!(again.added_at, first.added_at);
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.token(&first.id).as_deref(), Some("t2"));
    }

    #[test]
    fn removing_the_active_account_activates_the_most_recent_one() {
        let dir = tempfile::tempdir().unwrap();
        let keychain = FakeKeychain::default();
        let mut store = open(&dir.path().join("accounts.json"), &keychain, None);
        let a = store.add(input("s1", "u1", "t1")).unwrap();
        let b = store.add(input("s2", "u2", "t2")).unwrap();
        let c = store.add(input("s3", "u3", "t3")).unwrap();
        store.set_active(Some(&b.id)).unwrap();
        store.set_active(Some(&a.id)).unwrap();
        store.remove(&a.id).unwrap();
        assert_eq!(store.active().unwrap().id, b.id);
        assert!(keychain.entries.lock().unwrap().get(&a.id).is_none());
        store.remove(&b.id).unwrap();
        assert_eq!(store.active().unwrap().id, c.id);
        store.remove(&c.id).unwrap();
        assert!(store.active().is_none());
        assert_eq!(store.set_active(Some("missing")).unwrap(), None);
    }

    #[test]
    fn without_a_keychain_tokens_stay_in_memory_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let keychain = FakeKeychain {
            unavailable: true,
            ..Default::default()
        };
        let mut store = open(&path, &keychain, None);
        let status = store.status();
        assert!(!status.encrypted);
        assert_eq!(status.warning.as_deref(), Some(NO_KEYCHAIN_WARNING));
        let a = store.add(input("s1", "u1", "t1")).unwrap();
        assert_eq!(store.token(&a.id).as_deref(), Some("t1"));
        assert!(keychain.entries.lock().unwrap().is_empty());
        let reopened = open(&path, &keychain, None);
        assert!(!reopened.list()[0].has_token);
        assert_eq!(reopened.active().unwrap().id, a.id);
    }

    #[test]
    fn seed_is_used_only_when_there_is_no_file_and_corrupt_files_start_clean() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let keychain = FakeKeychain::default();
        let seed = StoredFile {
            version: 1,
            device_id: "legacy-device".into(),
            active_id: Some("gone".into()),
            accounts: vec![],
        };
        let store = open(&path, &keychain, Some(seed.clone()));
        assert_eq!(store.device_id(), "legacy-device");
        assert!(store.active().is_none());

        fs::write(&path, "{ not json").unwrap();
        let store = open(&path, &keychain, Some(seed));
        assert_ne!(store.device_id(), "legacy-device");
        assert!(store.list().is_empty());
    }

    #[test]
    fn status_and_public_model_serialize_like_the_typescript_contract() {
        let status = AccountsStatus {
            encrypted: true,
            warning: None,
        };
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"encrypted":true}"#
        );
        let dir = tempfile::tempdir().unwrap();
        let keychain = FakeKeychain::default();
        let mut store = open(&dir.path().join("accounts.json"), &keychain, None);
        let a = store.add(input("s1", "u1", "t1")).unwrap();
        let json = serde_json::to_value(&a).unwrap();
        for key in [
            "id",
            "name",
            "url",
            "serverId",
            "serverName",
            "version",
            "userId",
            "userName",
            "addedAt",
            "lastUsedAt",
            "hasToken",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert!(json.get("accessToken").is_none());
    }
}
