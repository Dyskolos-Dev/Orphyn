//! Casting to Chromecast, UPnP/DLNA (WiiM) and AirPlay receivers.
//!
//! Copied from `src-tauri/src/cast` (protocol code unchanged); the Tauri commands and events are
//! replaced by [`CastService`], a plain facade that reports changes through a [`CastEvent`]
//! callback. Background tasks run on a private tokio runtime (`runtime`).

mod airplay;
mod chromecast;
mod description;
mod didl;
mod discovery;
mod http;
mod manager;
mod model;
mod registry;
mod runtime;
mod session;
mod soap;
mod transport;
mod txt;
mod upnp;
mod xml;

use std::future::Future;
use std::sync::Arc;

use discovery::Discovery;
use manager::{CastManager, ManagerDeps, ManagerEvents, STARTUP_SCAN_MS};
pub use model::{
    CastCapabilities, CastDevice, CastEndedEvent, CastErrorEvent, CastKind, CastMedia,
    CastMediaKind, CastSessionState, CastStatus,
};
use registry::{DeviceRegistry, DEFAULT_TTL_MS};
use session::SessionOptions;
use transport::CastTransport;

/// A change reported by the cast service (same four events as the Tauri app).
#[derive(Debug, Clone)]
pub enum CastEvent {
    DevicesChanged(Vec<CastDevice>),
    SessionChanged(CastSessionState),
    Ended(CastEndedEvent),
    Error(CastErrorEvent),
}

pub type CastListener = Arc<dyn Fn(CastEvent) + Send + Sync>;

struct Events(CastListener);

impl ManagerEvents for Events {
    fn devices_changed(&self, devices: Vec<CastDevice>) {
        (self.0)(CastEvent::DevicesChanged(devices));
    }
    fn session_changed(&self, state: CastSessionState) {
        (self.0)(CastEvent::SessionChanged(state));
    }
    fn ended(&self, device_id: &str, media: Option<CastMedia>) {
        (self.0)(CastEvent::Ended(CastEndedEvent {
            device_id: device_id.to_string(),
            media,
        }));
    }
    fn error(&self, device_id: Option<&str>, message: String) {
        (self.0)(CastEvent::Error(CastErrorEvent {
            device_id: device_id.map(String::from),
            message,
        }));
    }
}

/// Wires registry + discovery + the real protocol transports into a `CastManager`.
fn create_manager(events: Arc<dyn ManagerEvents>) -> Arc<CastManager> {
    let on_change = events.clone();
    let registry = Arc::new(DeviceRegistry::new(
        Arc::new(move |devices| on_change.devices_changed(devices)),
        DEFAULT_TTL_MS,
    ));
    let discovery = Discovery::new(registry.clone());
    let (list_reg, get_reg, transports) = (registry.clone(), registry, discovery.clone());
    CastManager::new(ManagerDeps {
        discovery,
        list_devices: Box::new(move || list_reg.list()),
        get_device: Arc::new(move |id| get_reg.get(id)),
        create_transport: Arc::new(
            move |device: &CastDevice| -> Option<Arc<dyn CastTransport>> {
                match device.kind {
                    CastKind::Chromecast => Some(chromecast::ChromecastTransport::new(
                        &device.address,
                        device.port,
                    )),
                    CastKind::Upnp => transports.upnp_endpoints(&device.id).map(|endpoints| {
                        upnp::UpnpTransport::new(endpoints, upnp::UpnpOptions::default())
                            as Arc<dyn CastTransport>
                    }),
                    CastKind::Airplay => {
                        Some(airplay::AirplayTransport::new(&device.address, device.port))
                    }
                }
            },
        ),
        events,
        session_options: SessionOptions::default(),
    })
}

/// Validates media handed over by the UI: an http(s) URL is required.
pub fn validate_media(media: &CastMedia) -> Result<(), String> {
    let lower = media.url.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(())
    } else {
        Err("Invalid media".into())
    }
}

/// Cast facade: discovery, sessions and remote control of one or more receivers. Commands are
/// fire-and-forget; their outcome comes back through the listener (session/error events).
pub struct CastService {
    manager: Arc<CastManager>,
}

impl CastService {
    /// Starts the service (and a short warm-up scan); `listener` is called from background threads.
    pub fn new(listener: CastListener) -> Self {
        let _guard = runtime::handle().enter();
        let manager = create_manager(Arc::new(Events(listener)));
        manager.startup_scan(STARTUP_SCAN_MS);
        Self { manager }
    }

    pub fn start_discovery(&self) {
        let _guard = runtime::handle().enter();
        self.manager.start_discovery();
    }
    pub fn stop_discovery(&self) {
        self.manager.stop_discovery();
    }
    pub fn list_devices(&self) -> Vec<CastDevice> {
        self.manager.list_devices()
    }
    pub fn get_session(&self, device_id: &str) -> Option<CastSessionState> {
        self.manager.get_session(device_id)
    }

    fn run<F, Fut>(&self, name: &'static str, f: F, done: impl FnOnce(Result<(), String>) + Send + 'static)
    where
        F: FnOnce(Arc<CastManager>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let manager = self.manager.clone();
        runtime::spawn(async move {
            let result = f(manager).await;
            if let Err(message) = &result {
                log::warn!("[cast] {name} failed: {message}");
            }
            done(result);
        });
    }

    /// Loads `media` on the receiver; `done` gets the result.
    pub fn load(
        &self,
        device_id: &str,
        media: CastMedia,
        done: impl FnOnce(Result<(), String>) + Send + 'static,
    ) {
        if let Err(e) = validate_media(&media) {
            return done(Err(e));
        }
        let id = device_id.to_string();
        self.run("load", move |m| async move { m.load(&id, media).await }, done);
    }
    pub fn play(&self, device_id: &str) {
        let id = device_id.to_string();
        self.run("play", move |m| async move { m.play(&id).await }, |_| {});
    }
    pub fn pause(&self, device_id: &str) {
        let id = device_id.to_string();
        self.run("pause", move |m| async move { m.pause(&id).await }, |_| {});
    }
    pub fn seek(&self, device_id: &str, position_sec: f64) {
        if !position_sec.is_finite() {
            return;
        }
        let id = device_id.to_string();
        self.run("seek", move |m| async move { m.seek(&id, position_sec).await }, |_| {});
    }
    pub fn stop(&self, device_id: &str) {
        let id = device_id.to_string();
        self.run("stop", move |m| async move { m.stop(&id).await }, |_| {});
    }
    pub fn set_volume(&self, device_id: &str, volume: f64) {
        if !volume.is_finite() {
            return;
        }
        let id = device_id.to_string();
        self.run("set_volume", move |m| async move { m.set_volume(&id, volume).await }, |_| {});
    }
    pub fn set_muted(&self, device_id: &str, muted: bool) {
        let id = device_id.to_string();
        self.run("set_muted", move |m| async move { m.set_muted(&id, muted).await }, |_| {});
    }
    pub fn disconnect(&self, device_id: &str) {
        let id = device_id.to_string();
        self.run("disconnect", move |m| async move { m.disconnect(&id).await }, |_| {});
    }
    /// App quit: release every transport.
    pub fn dispose(&self) {
        let manager = self.manager.clone();
        runtime::spawn(async move { manager.dispose().await });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_must_be_http() {
        let mut m = model::test_media();
        m.url = "http://h/a.mp3".into();
        assert!(validate_media(&m).is_ok());
        m.url = "file:///a.mp3".into();
        assert!(validate_media(&m).is_err());
    }

    #[test]
    fn session_state_serializes_like_the_typescript_contract() {
        let mut s = CastSessionState::idle("dev");
        s.status = model::CastStatus::Buffering;
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(
            v,
            json!({ "deviceId": "dev", "status": "buffering", "positionSec": 0.0, "durationSec": 0.0,
                    "volume": 1.0, "muted": false, "media": null })
        );
    }
}
