//! In-app updates from the release `latest.json` (the Tauri updater manifest, same minisign key as
//! `src-tauri/tauri.conf.json`). A check 10 s after launch then every 6 h when automatic updates
//! are on, a background download where the build can install itself, then "Restart"; elsewhere
//! the person is sent to the release page.

pub mod state;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use tokio::sync::watch;

use state::{can_apply_in_app, UpdaterState, CHECK_INTERVAL_SECS, FIRST_CHECK_DELAY_SECS};

pub const ENDPOINT: &str =
    "https://github.com/Dyskolos-Dev/Orphyn/releases/latest/download/latest.json";
/// Base64 of the minisign public key file (`plugins.updater.pubkey` of `tauri.conf.json`).
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEZGQTNBOEM1QzUwMUQ1RkQKUldUOTFRSEZ4YWlqLy9taHA1YUZrT2hPaFFBK0U1Z25sSnZYUlZRSVRtSFByV0JhSFU3MG15NkMK";
const SETTINGS_FILE: &str = "updater.json";

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    platforms: HashMap<String, Target>,
}

#[derive(Debug, Clone, Deserialize)]
struct Target {
    url: String,
    signature: String,
}

/// `"windows-x86_64"`-style key of the manifest for this build.
pub fn platform_key(os: &str, arch: &str) -> String {
    let os = if os == "macos" { "darwin" } else { os };
    format!("{os}-{arch}")
}

/// Numeric `major.minor.patch` comparison (a pre-release suffix is ignored).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }
    let (mut a, mut b) = (parts(candidate), parts(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

/// Checks `bytes` against the base64 minisign signature of the manifest with [`PUBKEY`].
pub fn verify(bytes: &[u8], signature_b64: &str) -> Result<(), String> {
    let dec = |s: &str| -> Result<String, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .map_err(|e| e.to_string())?;
        String::from_utf8(raw).map_err(|e| e.to_string())
    };
    let pk = minisign_verify::PublicKey::decode(&dec(PUBKEY)?).map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(&dec(signature_b64)?).map_err(|e| e.to_string())?;
    pk.verify(bytes, &sig, false).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct UpdaterService {
    tx: Arc<watch::Sender<UpdaterState>>,
    downloaded: Arc<Mutex<Option<Vec<u8>>>>,
    can_apply: bool,
    client: reqwest::Client,
}

fn settings_path() -> Option<PathBuf> {
    crate::profile::config_dir()
        .ok()
        .map(|d| d.join(SETTINGS_FILE))
}

fn load_auto_update() -> bool {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.get("autoUpdate").and_then(|b| b.as_bool()))
        .unwrap_or(true)
}

impl UpdaterService {
    /// `supported`: false in a dev build (no matching release), the UI then says so.
    pub fn new(current_version: &str, supported: bool) -> Self {
        let appimage = std::env::var("APPIMAGE").ok();
        // macOS needs the bundle swap, not ported yet: it gets the guided download.
        let can_apply = std::env::consts::OS != "macos"
            && can_apply_in_app(std::env::consts::OS, appimage.as_deref());
        let state = UpdaterState::new(current_version.to_string(), load_auto_update(), supported);
        Self {
            tx: Arc::new(watch::channel(state).0),
            downloaded: Arc::new(Mutex::new(None)),
            can_apply,
            client: reqwest::Client::builder()
                .user_agent("Orphyn-updater")
                .timeout(Duration::from_secs(600))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn state(&self) -> UpdaterState {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<UpdaterState> {
        self.tx.subscribe()
    }

    fn update(&self, f: impl FnOnce(&mut UpdaterState) -> bool) {
        self.tx.send_if_modified(f);
    }

    pub fn set_auto_update(&self, enabled: bool) {
        if let Some(path) = settings_path() {
            let json = serde_json::json!({ "autoUpdate": enabled }).to_string();
            if let Err(e) = crate::accounts::store::write_atomic(&path, &json) {
                log::warn!("[updater] could not save the setting: {e}");
            }
        }
        self.update(|s| {
            s.auto_update = enabled;
            true
        });
    }

    /// The scheduled loop; spawn it once on the async runtime.
    pub async fn run_schedule(self) {
        if !self.state().supported {
            return;
        }
        tokio::time::sleep(Duration::from_secs(FIRST_CHECK_DELAY_SECS)).await;
        loop {
            if self.state().auto_update {
                self.check().await;
            }
            tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
        }
    }

    /// Manual or scheduled check. Ignored while a check or download runs.
    pub async fn check(&self) {
        let mut started = false;
        self.update(|s| {
            started = s.can_check();
            if started {
                s.checking();
            }
            started
        });
        if !started {
            return;
        }
        if let Err(e) = self.run_check().await {
            log::warn!("[updater] {e}");
            self.update(|s| {
                s.failed(e);
                true
            });
        }
    }

    /// Fetches the manifest: `(version, url, signature)` of the newer release for this platform.
    pub async fn fetch_latest(&self) -> Result<Option<(String, String, String)>, String> {
        let manifest: Manifest = self
            .client
            .get(ENDPOINT)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let current = self.state().current_version;
        if !is_newer(&manifest.version, &current) {
            return Ok(None);
        }
        let key = platform_key(std::env::consts::OS, std::env::consts::ARCH);
        let target = manifest
            .platforms
            .get(&key)
            .ok_or_else(|| format!("no build for {key} in latest.json"))?;
        Ok(Some((
            manifest.version,
            target.url.clone(),
            target.signature.clone(),
        )))
    }

    async fn run_check(&self) -> Result<(), String> {
        let Some((version, url, signature)) = self.fetch_latest().await? else {
            self.update(|s| {
                s.up_to_date();
                true
            });
            return Ok(());
        };
        let can_apply = self.can_apply;
        self.update(|s| {
            s.found(version, can_apply);
            true
        });
        if !can_apply {
            return Ok(());
        }
        let mut res = self
            .client
            .get(&url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?;
        let total = res.content_length();
        let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
        while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
            bytes.extend_from_slice(&chunk);
            let done = bytes.len() as u64;
            self.update(|s| s.progress(done, total));
        }
        verify(&bytes, &signature)?;
        *self.downloaded.lock().unwrap_or_else(|p| p.into_inner()) = Some(bytes);
        self.update(|s| {
            s.ready();
            true
        });
        Ok(())
    }

    /// Installs the downloaded update. On Windows the installer closes the app and restarts it;
    /// on an AppImage the file is replaced and the new one started. `Ok(true)` = the caller
    /// should exit now.
    pub fn install(&self) -> Result<bool, String> {
        let bytes = self
            .downloaded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        let Some(bytes) = bytes else {
            return Ok(false);
        };
        install_bytes(&bytes)?;
        Ok(true)
    }

    /// Installs a downloaded update when the app quits.
    pub fn install_on_exit(&self) {
        if let Err(e) = self.install() {
            log::warn!("[updater] install on exit failed: {e}");
        }
    }
}

#[cfg(target_os = "windows")]
fn install_bytes(bytes: &[u8]) -> Result<(), String> {
    let dir = std::env::temp_dir().join("orphyn-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe = dir.join("Orphyn-setup.exe");
    std::fs::write(&exe, bytes).map_err(|e| e.to_string())?;
    // Passive install that restarts the app, as the Tauri updater runs the NSIS installer.
    std::process::Command::new(exe)
        .args(["/P", "/R", "/UPDATE"])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_bytes(bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let target = std::env::var("APPIMAGE").map_err(|_| "not an AppImage".to_string())?;
    let tmp = format!("{target}.new");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    std::process::Command::new(&target)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn install_bytes(_bytes: &[u8]) -> Result<(), String> {
    Err("in-app install is not available on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("v0.10.0", "0.9.0"));
    }

    #[test]
    fn platform_keys_follow_the_manifest() {
        assert_eq!(platform_key("windows", "x86_64"), "windows-x86_64");
        assert_eq!(platform_key("macos", "aarch64"), "darwin-aarch64");
    }

    #[test]
    fn a_bad_signature_is_rejected() {
        assert!(verify(b"data", "not base64!").is_err());
        let fake = base64::engine::general_purpose::STANDARD.encode("untrusted comment: x\nabc\n");
        assert!(verify(b"data", &fake).is_err());
    }

    #[test]
    fn the_public_key_decodes() {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(PUBKEY)
            .unwrap();
        assert!(minisign_verify::PublicKey::decode(&String::from_utf8(raw).unwrap()).is_ok());
    }
}
