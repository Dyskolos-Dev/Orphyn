//! One-time import of the servers saved by the Electron app.
//!
//! Electron kept `accounts.json` in its `userData` folder: `<config dir>/<productName>`, i.e.
//! `%APPDATA%\Orphyn` (or `Lumen` before 1.1) on Windows, `~/Library/Application Support/...` on
//! macOS and `~/.config/...` on Linux. The tokens in it are encrypted with Electron's
//! `safeStorage` (a key that only Electron can unlock), so they are dropped: each server comes
//! back with `hasToken: false` and the renderer shows its usual "sign in again" flow.

use std::fs;
use std::path::{Path, PathBuf};

use super::store::StoredFile;

/// Electron `userData` folder names, newest first.
pub const LEGACY_DIR_NAMES: [&str; 4] = ["Orphyn", "orphyn", "Lumen", "lumen"];

/// `accounts.json` files the Electron app may have left, in the order they are tried.
pub fn legacy_account_files() -> Vec<PathBuf> {
    let Some(config) = dirs::config_dir() else {
        return Vec::new();
    };
    LEGACY_DIR_NAMES
        .iter()
        .map(|name| config.join(name).join("accounts.json"))
        .collect()
}

/// Reads the first parseable legacy file with at least one account. Tokens are removed.
pub fn import_legacy(candidates: &[PathBuf]) -> Option<(PathBuf, StoredFile)> {
    candidates.iter().find_map(|path| {
        let mut file = read(path)?;
        if file.accounts.is_empty() {
            return None;
        }
        for a in &mut file.accounts {
            a.access_token = None;
        }
        file.version = 1;
        Some((path.clone(), file))
    })
}

fn read(path: &Path) -> Option<StoredFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::tests::FakeKeychain;
    use crate::accounts::store::AccountStore;

    const ELECTRON_FILE: &str = r#"{
      "version": 1,
      "deviceId": "electron-device",
      "activeId": "a1",
      "accounts": [
        {
          "id": "a1", "name": "Home", "url": "https://jf.example",
          "serverId": "srv", "serverName": "Jellyfin", "version": "10.10.7",
          "userId": "u1", "userName": "alice",
          "addedAt": "2025-01-01T00:00:00.000Z", "lastUsedAt": "2025-02-01T00:00:00.000Z",
          "accessToken": "djEwXXXXencryptedXXXX"
        }
      ]
    }"#;

    #[test]
    fn imports_servers_without_tokens_from_the_first_folder_found() {
        let dir = tempfile::tempdir().unwrap();
        let orphyn = dir.path().join("Orphyn").join("accounts.json");
        let lumen = dir.path().join("Lumen").join("accounts.json");
        fs::create_dir_all(lumen.parent().unwrap()).unwrap();
        fs::write(&lumen, ELECTRON_FILE).unwrap();

        let (from, file) = import_legacy(&[orphyn.clone(), lumen.clone()]).unwrap();
        assert_eq!(from, lumen);
        assert_eq!(file.device_id, "electron-device");
        assert_eq!(file.accounts[0].name, "Home");
        assert_eq!(file.accounts[0].access_token, None);

        // Seeded into a fresh store: listed, active, but needing a new sign-in.
        let keychain = FakeKeychain::default();
        let store = AccountStore::open(
            dir.path().join("new").join("accounts.json"),
            Box::new(keychain),
            Some(file),
        );
        let active = store.active().unwrap();
        assert_eq!(active.user_name, "alice");
        assert!(!active.has_token);
        assert_eq!(store.device_id(), "electron-device");
    }

    #[test]
    fn ignores_missing_empty_and_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.json");
        let corrupt = dir.path().join("corrupt.json");
        fs::write(
            &empty,
            r#"{"version":1,"deviceId":"d","activeId":null,"accounts":[]}"#,
        )
        .unwrap();
        fs::write(&corrupt, "nope").unwrap();
        assert!(import_legacy(&[dir.path().join("missing.json"), empty, corrupt]).is_none());
    }

    #[test]
    fn candidates_cover_the_old_product_names() {
        let files = legacy_account_files();
        if files.is_empty() {
            return; // no config dir on this machine
        }
        let names: Vec<String> = files
            .iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, LEGACY_DIR_NAMES);
    }
}
