//! First start after an update from Orphyn 1.5 (Tauri): imports what the old app kept outside the
//! files the native app already reads.
//!
//! - Accounts: nothing to copy. `accounts.json` lives in the same folder (identifier
//!   `app.lumen.desktop`) and the tokens are the same keychain entries (service `Orphyn`).
//! - Preferences: the webview's `localStorage` entries (`orphyn.*`) become `local-storage.json`
//!   entries. WebView2 keeps them in a LevelDB (Windows), WebKit in a SQLite file per origin
//!   (macOS, Linux). Keys the native app already holds are never overwritten.
//! - Window state: `.window-state.json` (tauri-plugin-window-state) becomes `native-window-state.json`.
//!
//! The import runs once: `native-migration.json` in the data folder records it. The old files are
//! only read (the LevelDB is copied to a temporary folder first), so 1.5 keeps working beside it.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use orphyn_core::profile;
use orphyn_core::storage::LocalStorage;
use serde_json::Value;

const MARKER: &str = "native-migration.json";
const KEY_PREFIX: &str = "orphyn.";
const TAURI_WINDOW_STATE: &str = ".window-state.json";
const NATIVE_WINDOW_STATE: &str = "native-window-state.json";

/// What an import found, for the log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub accounts_found: bool,
    pub settings_imported: usize,
    pub window_state_imported: bool,
}

/// Runs the import once per profile. Call it before the settings are first read.
pub fn run_if_needed(storage: &LocalStorage) -> Option<Report> {
    if profile::is_test() {
        return None;
    }
    let data = profile::data_dir().ok()?;
    let config = profile::config_dir().ok()?;
    if data.join(MARKER).exists() {
        return None;
    }
    let report = run(&data, &config, &webview_storage_dirs(), storage);
    let body = serde_json::json!({
        "from": "1.5",
        "settings": report.settings_imported,
        "windowState": report.window_state_imported,
    });
    let _ = orphyn_core::accounts::store::write_atomic(&data.join(MARKER), &body.to_string());
    log::info!("[migration] 1.5 profile: {report:?}");
    Some(report)
}

/// The import itself, on explicit folders (tests).
pub fn run(data: &Path, config: &Path, webview_dirs: &[PathBuf], storage: &LocalStorage) -> Report {
    let mut report = Report {
        accounts_found: data.join("accounts.json").exists(),
        ..Report::default()
    };
    let mut old = BTreeMap::new();
    for dir in webview_dirs {
        let found = if dir.is_dir() && dir.join("CURRENT").exists() {
            read_leveldb(dir)
        } else {
            read_webkit_dir(dir)
        };
        for (k, v) in found {
            old.entry(k).or_insert(v);
        }
    }
    for (key, value) in old {
        if storage.get(&key).is_none() {
            storage.set(&key, &value);
            report.settings_imported += 1;
        }
    }
    report.window_state_imported = import_window_state(config);
    report
}

/// Where the 1.5 webview stored `localStorage`, per OS.
fn webview_storage_dirs() -> Vec<PathBuf> {
    let mut dirs_found = Vec::new();
    #[cfg(windows)]
    if let Some(local) = dirs::data_local_dir() {
        dirs_found.push(
            local
                .join(profile::APP_ID)
                .join("EBWebView/Default/Local Storage/leveldb"),
        );
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        let base = home.join("Library/WebKit").join(profile::APP_ID);
        dirs_found.push(base.join("WebsiteData/LocalStorage"));
        dirs_found.push(
            home.join("Library/Application Support")
                .join(profile::APP_ID)
                .join("localstorage"),
        );
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let (Some(data), Some(cache)) = (dirs::data_dir(), dirs::cache_dir()) {
        dirs_found.push(data.join(profile::APP_ID).join("localstorage"));
        dirs_found.push(cache.join(profile::APP_ID).join("localstorage"));
    }
    dirs_found
}

/// Entries `orphyn.*` of a Chromium LevelDB `Local Storage`. Keys read `_<origin>\0\x01<name>`;
/// values start with `0x01` (Latin-1) or `0x00` (UTF-16LE).
fn read_leveldb(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(tmp) = tempfile::tempdir() else {
        return out;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.file_name() != "LOCK" && entry.path().is_file() {
            let _ = fs::copy(entry.path(), tmp.path().join(entry.file_name()));
        }
    }
    let Ok(mut db) = rusty_leveldb::DB::open(tmp.path(), rusty_leveldb::Options::default()) else {
        return out;
    };
    let Ok(mut it) = db.new_iter() else {
        return out;
    };
    use rusty_leveldb::LdbIterator;
    while it.advance() {
        let Some((k, v)) = it.current() else { continue };
        if k.first() != Some(&b'_') {
            continue;
        }
        let Some(split) = k.windows(2).position(|w| w == [0, 1]) else {
            continue;
        };
        let name = String::from_utf8_lossy(&k[split + 2..]).into_owned();
        if !name.starts_with(KEY_PREFIX) {
            continue;
        }
        let value = match v.first() {
            Some(1) => v[1..].iter().map(|&b| b as char).collect::<String>(),
            Some(0) => {
                let units: Vec<u16> = v[1..]
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect();
                String::from_utf16_lossy(&units)
            }
            _ => continue,
        };
        out.insert(name, value);
    }
    out
}

/// Entries `orphyn.*` of the WebKit `*.localstorage` SQLite files in `dir` (`ItemTable`, UTF-16LE
/// blob values).
fn read_webkit_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_some_and(|e| e == "localstorage") {
            out.extend(read_sqlite(&path));
        }
    }
    out
}

fn read_sqlite(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return out;
    };
    let Ok(mut stmt) = conn.prepare("SELECT key, value FROM ItemTable WHERE key LIKE 'orphyn.%'")
    else {
        return out;
    };
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    });
    for (key, blob) in rows.into_iter().flatten().flatten() {
        let units: Vec<u16> = blob
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        out.insert(key, String::from_utf16_lossy(&units));
    }
    out
}

/// `.window-state.json` (`{"main": {width, height, x, y, maximized, ...}}`, physical pixels) to the
/// native file. Never overwrites the native file.
fn import_window_state(config: &Path) -> bool {
    let target = config.join(NATIVE_WINDOW_STATE);
    if target.exists() {
        return false;
    }
    let Ok(raw) = fs::read_to_string(config.join(TAURI_WINDOW_STATE)) else {
        return false;
    };
    let Ok(state) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    let Some(main) = state.get("main") else {
        return false;
    };
    let num = |k: &str| main.get(k).and_then(Value::as_i64);
    let (Some(width), Some(height)) = (num("width"), num("height")) else {
        return false;
    };
    let bounds = serde_json::json!({
        "width": width,
        "height": height,
        "x": num("x").unwrap_or(0),
        "y": num("y").unwrap_or(0),
        "maximized": main.get("maximized").and_then(Value::as_bool).unwrap_or(false),
    });
    orphyn_core::accounts::store::write_atomic(&target, &bounds.to_string()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn imports_webview2_leveldb_window_state_and_runs_over_existing_keys() {
        let root = tempfile::tempdir().unwrap();
        let ldb = root.path().join("leveldb");
        {
            let mut db = rusty_leveldb::DB::open(&ldb, rusty_leveldb::Options::default()).unwrap();
            let mut k = b"_http://tauri.localhost\0\x01orphyn.theme.mode".to_vec();
            db.put(&k, b"\x01dark").unwrap();
            k = b"_http://tauri.localhost\0\x01orphyn.playback.preferences".to_vec();
            let mut v = vec![0u8];
            v.extend(utf16("{\"crossfade\":true,\"x\":\"é\"}"));
            db.put(&k, &v).unwrap();
            db.put(b"_http://tauri.localhost\0\x01other.key", b"\x01no")
                .unwrap();
            db.put(b"_http://tauri.localhost\0\x01orphyn.language", b"\x01fr")
                .unwrap();
            db.flush().unwrap();
        }
        let config = root.path().join("config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join(TAURI_WINDOW_STATE),
            r#"{"main":{"width":1400,"height":900,"x":10,"y":20,"maximized":true,"visible":true}}"#,
        )
        .unwrap();
        let data = root.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("accounts.json"), "{}").unwrap();

        let storage = LocalStorage::in_memory();
        storage.set("orphyn.language", "en");
        let report = run(&data, &config, &[ldb], &storage);

        assert!(report.accounts_found && report.window_state_imported);
        assert_eq!(report.settings_imported, 2);
        assert_eq!(storage.get("orphyn.theme.mode").as_deref(), Some("dark"));
        assert_eq!(
            storage.get("orphyn.playback.preferences").as_deref(),
            Some("{\"crossfade\":true,\"x\":\"é\"}")
        );
        assert_eq!(storage.get("orphyn.language").as_deref(), Some("en"));
        assert_eq!(storage.get("other.key"), None);
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(config.join(NATIVE_WINDOW_STATE)).unwrap())
                .unwrap();
        assert_eq!(saved["width"], 1400);
        assert_eq!(saved["maximized"], true);
        // The native file is never overwritten.
        assert!(!import_window_state(&config));
    }

    #[test]
    fn imports_webkit_sqlite() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("https_tauri.localhost_0.localstorage");
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute(
            "CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ItemTable VALUES ('orphyn.theme.mode', ?1), ('zzz', ?1)",
            [utf16("light")],
        )
        .unwrap();
        drop(conn);
        let storage = LocalStorage::in_memory();
        let report = run(
            root.path(),
            root.path(),
            &[root.path().to_path_buf()],
            &storage,
        );
        assert_eq!(report.settings_imported, 1);
        assert_eq!(storage.get("orphyn.theme.mode").as_deref(), Some("light"));
    }
}
