//! The native counterpart of the web app's `localStorage`: one flat string-to-string map, persisted
//! as `<data dir>/local-storage.json`. Every key and value is the one the React app writes
//! (`orphyn.theme.mode` = `dark`, `orphyn.playback.preferences` = a JSON text...), so importing a
//! 1.5 profile is a plain copy of the webview's localStorage entries (README, "Migration").
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::accounts::store::write_atomic;

/// File name inside the data directory.
pub const FILE_NAME: &str = "local-storage.json";

#[derive(Clone)]
pub struct LocalStorage {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    path: Option<PathBuf>,
    map: BTreeMap<String, String>,
}

impl LocalStorage {
    /// Opens (or starts) the store at `path`. A missing or corrupt file starts empty.
    pub fn open(path: PathBuf) -> Self {
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<BTreeMap<String, String>>(&raw).ok())
            .unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(Inner {
                path: Some(path),
                map,
            })),
        }
    }

    /// The store of the current profile.
    pub fn open_default() -> Self {
        match crate::profile::data_dir() {
            Ok(dir) => Self::open(dir.join(FILE_NAME)),
            Err(e) => {
                log::warn!("[storage] {e}: settings are kept in memory only");
                Self::in_memory()
            }
        }
    }

    /// A store that is never written to disk (tests, no data directory).
    pub fn in_memory() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                path: None,
                map: BTreeMap::new(),
            })),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.lock().map.get(key).cloned()
    }

    pub fn set(&self, key: &str, value: &str) {
        let mut inner = self.lock();
        if inner.map.get(key).map(String::as_str) == Some(value) {
            return;
        }
        inner.map.insert(key.to_string(), value.to_string());
        inner.save();
    }

    pub fn remove(&self, key: &str) {
        let mut inner = self.lock();
        if inner.map.remove(key).is_some() {
            inner.save();
        }
    }

    /// Typed read of a JSON value stored as text (`None` when missing or invalid).
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key)
            .and_then(|raw| serde_json::from_str(&raw).ok())
    }

    pub fn set_json<T: serde::Serialize>(&self, key: &str, value: &T) {
        if let Ok(text) = serde_json::to_string(value) {
            self.set(key, &text);
        }
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.lock().path.clone()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Inner {
    fn save(&self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = serde_json::to_string_pretty(&self.map) else {
            return;
        };
        if let Err(e) = write_atomic(path, &text) {
            log::warn!("[storage] could not write {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let s = LocalStorage::open(path.clone());
        s.set("orphyn.theme.mode", "dark");
        s.set_json(
            "orphyn.interface.appearance",
            &serde_json::json!({ "corners": "round" }),
        );
        let again = LocalStorage::open(path);
        assert_eq!(again.get("orphyn.theme.mode").as_deref(), Some("dark"));
        assert_eq!(
            again.get("orphyn.interface.appearance").as_deref(),
            Some(r#"{"corners":"round"}"#)
        );
        again.remove("orphyn.theme.mode");
        assert_eq!(again.get("orphyn.theme.mode"), None);
    }

    #[test]
    fn a_corrupt_file_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, "{ nope").unwrap();
        assert_eq!(LocalStorage::open(path).get("x"), None);
    }
}
