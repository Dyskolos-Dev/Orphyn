//! Casting to Chromecast, UPnP/DLNA (WiiM) and AirPlay receivers.
//!
//! Same contract as the Electron implementation (`src/shared/ipc/cast.ts`): commands
//! `cast_*`, and the events `cast:devices-changed`, `cast:session-changed`, `cast:ended` and
//! `cast:error`. Every log line is prefixed `[cast]`; every failure is mapped to a string error.

mod airplay;
mod chromecast;
mod description;
mod didl;
mod discovery;
mod http;
mod manager;
mod model;
mod registry;
mod session;
mod soap;
mod transport;
mod txt;
mod upnp;
mod xml;

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WindowEvent};

use crate::error::{Error, Result};
use discovery::Discovery;
use manager::{CastManager, ManagerDeps, ManagerEvents, STARTUP_SCAN_MS};
use model::{CastDevice, CastEndedEvent, CastErrorEvent, CastKind, CastMedia, CastSessionState};
use registry::{DeviceRegistry, DEFAULT_TTL_MS};
use session::SessionOptions;
use transport::CastTransport;

const EVT_DEVICES: &str = "cast:devices-changed";
const EVT_SESSION: &str = "cast:session-changed";
const EVT_ENDED: &str = "cast:ended";
const EVT_ERROR: &str = "cast:error";

/// Managed state of the cast module.
pub struct CastState {
    manager: Arc<CastManager>,
}

struct TauriEvents<R: Runtime>(AppHandle<R>);

impl<R: Runtime> TauriEvents<R> {
    fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) {
        if let Err(e) = self.0.emit(event, payload) {
            log::warn!("[cast] emit {event} failed: {e}");
        }
    }
}

impl<R: Runtime> ManagerEvents for TauriEvents<R> {
    fn devices_changed(&self, devices: Vec<CastDevice>) {
        self.emit(EVT_DEVICES, devices);
    }
    fn session_changed(&self, state: CastSessionState) {
        self.emit(EVT_SESSION, state);
    }
    fn ended(&self, device_id: &str, media: Option<CastMedia>) {
        self.emit(
            EVT_ENDED,
            CastEndedEvent {
                device_id: device_id.to_string(),
                media,
            },
        );
    }
    fn error(&self, device_id: Option<&str>, message: String) {
        self.emit(
            EVT_ERROR,
            CastErrorEvent {
                device_id: device_id.map(String::from),
                message,
            },
        );
    }
}

/// Wires registry + discovery + the real protocol transports into a `CastManager`.
fn create_service(events: Arc<dyn ManagerEvents>) -> Arc<CastManager> {
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

pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let manager = create_service(Arc::new(TauriEvents(app.clone())));
    app.manage(CastState {
        manager: manager.clone(),
    });
    // Short startup scan so the first device list is already warm.
    manager.startup_scan(STARTUP_SCAN_MS);
    // Release every transport when the main window goes away.
    if let Some(window) = app.get_webview_window(crate::window::MAIN) {
        let m = manager;
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                let m = m.clone();
                tauri::async_runtime::spawn(async move { m.dispose().await });
            }
        });
    }
    Ok(())
}

/// Logs a failed command and maps its message to the shared error type.
fn done<T>(command: &str, result: std::result::Result<T, String>) -> Result<T> {
    result.map_err(|message| {
        log::warn!("[cast] {command} failed: {message}");
        Error::Other(message)
    })
}

fn parse_media(value: Value) -> Result<CastMedia> {
    let media: CastMedia = serde_json::from_value(value).map_err(|e| {
        log::warn!("[cast] invalid media: {e}");
        Error::Other("Invalid media".into())
    })?;
    let lower = media.url.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(Error::Other("Invalid media".into()));
    }
    Ok(media)
}

#[tauri::command]
pub async fn cast_start_discovery(state: State<'_, CastState>) -> Result<()> {
    state.manager.start_discovery();
    Ok(())
}

#[tauri::command]
pub async fn cast_stop_discovery(state: State<'_, CastState>) -> Result<()> {
    state.manager.stop_discovery();
    Ok(())
}

#[tauri::command]
pub async fn cast_list_devices(state: State<'_, CastState>) -> Result<Vec<Value>> {
    let devices = state.manager.list_devices();
    Ok(devices
        .into_iter()
        .filter_map(|d| serde_json::to_value(d).ok())
        .collect())
}

#[tauri::command]
pub async fn cast_get_session(
    state: State<'_, CastState>,
    device_id: String,
) -> Result<Option<Value>> {
    Ok(state
        .manager
        .get_session(&device_id)
        .and_then(|s| serde_json::to_value(s).ok()))
}

#[tauri::command]
pub async fn cast_load(state: State<'_, CastState>, device_id: String, media: Value) -> Result<()> {
    let media = parse_media(media)?;
    done("cast_load", state.manager.load(&device_id, media).await)
}

#[tauri::command]
pub async fn cast_play(state: State<'_, CastState>, device_id: String) -> Result<()> {
    done("cast_play", state.manager.play(&device_id).await)
}

#[tauri::command]
pub async fn cast_pause(state: State<'_, CastState>, device_id: String) -> Result<()> {
    done("cast_pause", state.manager.pause(&device_id).await)
}

#[tauri::command]
pub async fn cast_seek(
    state: State<'_, CastState>,
    device_id: String,
    position_sec: f64,
) -> Result<()> {
    if !position_sec.is_finite() {
        return Err(Error::Other("Invalid argument".into()));
    }
    done(
        "cast_seek",
        state.manager.seek(&device_id, position_sec).await,
    )
}

#[tauri::command]
pub async fn cast_stop(state: State<'_, CastState>, device_id: String) -> Result<()> {
    done("cast_stop", state.manager.stop(&device_id).await)
}

#[tauri::command]
pub async fn cast_set_volume(
    state: State<'_, CastState>,
    device_id: String,
    volume: f64,
) -> Result<()> {
    if !volume.is_finite() {
        return Err(Error::Other("Invalid argument".into()));
    }
    done(
        "cast_set_volume",
        state.manager.set_volume(&device_id, volume).await,
    )
}

#[tauri::command]
pub async fn cast_set_muted(
    state: State<'_, CastState>,
    device_id: String,
    muted: bool,
) -> Result<()> {
    done(
        "cast_set_muted",
        state.manager.set_muted(&device_id, muted).await,
    )
}

#[tauri::command]
pub async fn cast_disconnect(state: State<'_, CastState>, device_id: String) -> Result<()> {
    done(
        "cast_disconnect",
        state.manager.disconnect(&device_id).await,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_from_the_renderer_is_validated() {
        let ok = parse_media(json!({
            "url": "http://h/a.mp3", "mimeType": "audio/mpeg", "kind": "audio", "title": "T",
            "artworkUrl": "http://h/a.jpg", "durationSec": 12.5
        }))
        .unwrap();
        assert_eq!(ok.duration_sec, Some(12.5));
        assert!(parse_media(
            json!({ "url": "file:///a.mp3", "mimeType": "x", "kind": "audio", "title": "T" })
        )
        .is_err());
        assert!(parse_media(
            json!({ "url": "http://h", "mimeType": "x", "kind": "image", "title": "T" })
        )
        .is_err());
        assert!(parse_media(json!(null)).is_err());
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
