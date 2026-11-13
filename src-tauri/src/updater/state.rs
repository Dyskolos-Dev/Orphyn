//! Update state machine, shared shape with `UpdaterState` in `src/shared/ipc/updater.ts`.

use serde::Serialize;

pub const RELEASES_URL: &str = "https://github.com/Dyskolos-Dev/Orphyn/releases";
pub const FIRST_CHECK_DELAY_SECS: u64 = 10;
pub const CHECK_INTERVAL_SECS: u64 = 6 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdaterStatus {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterState {
    pub status: UpdaterStatus,
    pub current_version: String,
    pub version: Option<String>,
    pub percent: u8,
    pub error: Option<String>,
    pub auto_update: bool,
    pub supported: bool,
}

/// True when this build can install an update by itself. Windows (NSIS) and macOS (the updater
/// swaps the .app bundle, no Squirrel signature needed) can; on Linux only an AppImage can (it
/// sets `APPIMAGE`), a .deb/.rpm gets the guided download.
pub fn can_apply_in_app(os: &str, appimage: Option<&str>) -> bool {
    match os {
        "windows" | "macos" => true,
        "linux" => appimage.is_some_and(|v| !v.is_empty()),
        _ => false,
    }
}

pub fn release_url(version: Option<&str>) -> String {
    match version {
        Some(v) => format!("{RELEASES_URL}/tag/v{v}"),
        None => format!("{RELEASES_URL}/latest"),
    }
}

impl UpdaterState {
    pub fn new(current_version: String, auto_update: bool, supported: bool) -> Self {
        Self {
            status: UpdaterStatus::Idle,
            current_version,
            version: None,
            percent: 0,
            error: None,
            auto_update,
            supported,
        }
    }

    /// A check may start: updates are supported and nothing is running or waiting to install.
    pub fn can_check(&self) -> bool {
        self.supported
            && !matches!(
                self.status,
                UpdaterStatus::Checking | UpdaterStatus::Downloading | UpdaterStatus::Ready
            )
    }

    pub fn checking(&mut self) {
        self.status = UpdaterStatus::Checking;
        self.error = None;
    }

    pub fn found(&mut self, version: String, can_apply: bool) {
        self.status = if can_apply {
            UpdaterStatus::Downloading
        } else {
            UpdaterStatus::Available
        };
        self.version = Some(version);
        self.percent = 0;
    }

    pub fn up_to_date(&mut self) {
        self.status = UpdaterStatus::UpToDate;
        self.version = None;
    }

    /// Returns true when the rounded percentage changed (to avoid flooding the renderer).
    pub fn progress(&mut self, downloaded: u64, total: Option<u64>) -> bool {
        let Some(total) = total.filter(|t| *t > 0) else {
            return false;
        };
        let percent = ((downloaded.min(total) * 100) / total) as u8;
        let changed = percent != self.percent;
        self.percent = percent;
        changed
    }

    pub fn ready(&mut self) {
        self.status = UpdaterStatus::Ready;
        self.percent = 100;
    }

    pub fn failed(&mut self, message: String) {
        self.status = UpdaterStatus::Error;
        self.error = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_that_install_in_app() {
        assert!(can_apply_in_app("windows", None));
        assert!(can_apply_in_app("macos", None));
        assert!(can_apply_in_app("linux", Some("/tmp/Orphyn.AppImage")));
        assert!(!can_apply_in_app("linux", None));
        assert!(!can_apply_in_app("linux", Some("")));
    }

    #[test]
    fn download_flow() {
        let mut s = UpdaterState::new("1.1.0".into(), true, true);
        assert!(s.can_check());
        s.checking();
        assert!(!s.can_check());
        s.found("1.2.0".into(), true);
        assert_eq!(s.status, UpdaterStatus::Downloading);
        assert!(s.progress(50, Some(200)));
        assert!(!s.progress(51, Some(200)));
        assert_eq!(s.percent, 25);
        assert!(!s.progress(10, None));
        s.ready();
        assert_eq!((s.status, s.percent), (UpdaterStatus::Ready, 100));
        assert!(!s.can_check());
    }

    #[test]
    fn guided_download_and_errors() {
        let mut s = UpdaterState::new("1.1.0".into(), true, true);
        s.found("1.2.0".into(), false);
        assert_eq!(s.status, UpdaterStatus::Available);
        assert_eq!(
            release_url(s.version.as_deref()),
            "https://github.com/Dyskolos-Dev/Orphyn/releases/tag/v1.2.0"
        );
        assert_eq!(
            release_url(None),
            "https://github.com/Dyskolos-Dev/Orphyn/releases/latest"
        );
        s.failed("offline".into());
        assert!(s.can_check());
        s.checking();
        assert_eq!(s.error, None);
        s.up_to_date();
        assert_eq!(s.status, UpdaterStatus::UpToDate);
        assert_eq!(s.version, None);
        assert!(!UpdaterState::new("1".into(), true, false).can_check());
    }

    #[test]
    fn serializes_like_the_typescript_contract() {
        let json = serde_json::to_value(UpdaterState::new("1.1.0".into(), true, false)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "status": "idle", "currentVersion": "1.1.0", "version": null, "percent": 0,
                "error": null, "autoUpdate": true, "supported": false
            })
        );
        let mut s = UpdaterState::new("1".into(), true, true);
        s.up_to_date();
        assert_eq!(serde_json::to_value(&s).unwrap()["status"], "up-to-date");
    }
}
