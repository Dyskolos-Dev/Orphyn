//! Owns one `CastSession` per device and the discovery on/off policy.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::runtime::{spawn, JoinHandle};

use super::model::{CastDevice, CastMedia, CastMediaKind, CastSessionState};
use super::session::{CastSession, CreateTransport, SessionHooks, SessionOptions};
use super::transport::{CastTransport, TResult};

pub const STARTUP_SCAN_MS: u64 = 8000;

pub trait DiscoveryControl: Send + Sync {
    fn start(&self);
    fn stop(&self);
}

pub trait ManagerEvents: Send + Sync {
    fn devices_changed(&self, devices: Vec<CastDevice>);
    fn session_changed(&self, state: CastSessionState);
    fn ended(&self, device_id: &str, media: Option<CastMedia>);
    fn error(&self, device_id: Option<&str>, message: String);
}

pub type GetDevice = Arc<dyn Fn(&str) -> Option<CastDevice> + Send + Sync>;
pub type MakeTransport = Arc<dyn Fn(&CastDevice) -> Option<Arc<dyn CastTransport>> + Send + Sync>;

pub struct ManagerDeps {
    pub discovery: Arc<dyn DiscoveryControl>,
    pub list_devices: Box<dyn Fn() -> Vec<CastDevice> + Send + Sync>,
    pub get_device: GetDevice,
    /// Builds the protocol transport for a device (None when it cannot be controlled).
    pub create_transport: MakeTransport,
    pub events: Arc<dyn ManagerEvents>,
    pub session_options: SessionOptions,
}

struct Hooks(Arc<dyn ManagerEvents>);

impl SessionHooks for Hooks {
    fn on_state(&self, state: CastSessionState) {
        self.0.session_changed(state);
    }
    fn on_ended(&self, device_id: &str, media: Option<CastMedia>) {
        self.0.ended(device_id, media);
    }
    fn on_error(&self, device_id: &str, message: String) {
        self.0.error(Some(device_id), message);
    }
}

#[derive(Default)]
struct State {
    renderer_wants_discovery: bool,
    startup_active: bool,
    startup_task: Option<JoinHandle<()>>,
    disposed: bool,
    sessions: HashMap<String, Arc<CastSession>>,
}

pub struct CastManager {
    deps: ManagerDeps,
    state: Mutex<State>,
}

impl CastManager {
    pub fn new(deps: ManagerDeps) -> Arc<Self> {
        Arc::new(Self {
            deps,
            state: Mutex::new(State::default()),
        })
    }

    fn st(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Short scan at app startup so the device list is warm; stops unless the renderer asked for more.
    pub fn startup_scan(self: &Arc<Self>, duration_ms: u64) {
        {
            let mut s = self.st();
            if s.disposed {
                return;
            }
            s.startup_active = true;
        }
        self.deps.discovery.start();
        let me = self.clone();
        let task = spawn(async move {
            tokio::time::sleep(Duration::from_millis(duration_ms)).await;
            let stop = {
                let mut s = me.st();
                s.startup_active = false;
                s.startup_task = None;
                !s.renderer_wants_discovery
            };
            if stop {
                me.deps.discovery.stop();
            }
        });
        self.st().startup_task = Some(task);
    }

    pub fn start_discovery(&self) {
        {
            let mut s = self.st();
            if s.disposed {
                return;
            }
            s.renderer_wants_discovery = true;
        }
        self.deps.discovery.start();
    }

    pub fn stop_discovery(&self) {
        let stop = {
            let mut s = self.st();
            s.renderer_wants_discovery = false;
            // Keep running if the startup scan is still in progress; it stops itself.
            !s.startup_active
        };
        if stop {
            self.deps.discovery.stop();
        }
    }

    pub fn list_devices(&self) -> Vec<CastDevice> {
        (self.deps.list_devices)()
    }

    pub fn get_session(&self, device_id: &str) -> Option<CastSessionState> {
        self.st().sessions.get(device_id).map(|s| s.get_state())
    }

    pub async fn load(&self, device_id: &str, media: CastMedia) -> TResult<()> {
        if let Some(device) = (self.deps.get_device)(device_id) {
            if media.kind == CastMediaKind::Video && !device.capabilities.video {
                return Err(format!("{} cannot play video", device.name));
            }
            if !device.capabilities.audio && !device.capabilities.video {
                return Err(format!("{} is not supported for playback", device.name));
            }
        }
        self.session_for(device_id, true)?.load(media).await
    }

    pub async fn play(&self, device_id: &str) -> TResult<()> {
        self.session_for(device_id, false)?.play().await
    }
    pub async fn pause(&self, device_id: &str) -> TResult<()> {
        self.session_for(device_id, false)?.pause().await
    }
    pub async fn seek(&self, device_id: &str, position_sec: f64) -> TResult<()> {
        self.session_for(device_id, false)?.seek(position_sec).await
    }
    pub async fn stop(&self, device_id: &str) -> TResult<()> {
        self.session_for(device_id, false)?.stop().await
    }
    pub async fn set_volume(&self, device_id: &str, volume: f64) -> TResult<()> {
        self.session_for(device_id, false)?.set_volume(volume).await
    }
    pub async fn set_muted(&self, device_id: &str, muted: bool) -> TResult<()> {
        self.session_for(device_id, false)?.set_muted(muted).await
    }

    pub async fn disconnect(&self, device_id: &str) -> TResult<()> {
        let Some(session) = self.st().sessions.get(device_id).cloned() else {
            return Ok(());
        };
        let result = session.disconnect().await;
        self.st().sessions.remove(device_id);
        result
    }

    /// App quit: stop discovery, close every transport, emit nothing more.
    pub async fn dispose(&self) {
        let sessions: Vec<Arc<CastSession>> = {
            let mut s = self.st();
            s.disposed = true;
            if let Some(t) = s.startup_task.take() {
                t.abort();
            }
            s.startup_active = false;
            s.sessions.drain().map(|(_, v)| v).collect()
        };
        self.deps.discovery.stop();
        for s in sessions {
            s.dispose().await;
        }
    }

    fn session_for(&self, device_id: &str, create: bool) -> TResult<Arc<CastSession>> {
        let mut s = self.st();
        if s.disposed {
            return Err("Casting is shut down".into());
        }
        if let Some(existing) = s.sessions.get(device_id) {
            return Ok(existing.clone());
        }
        if !create {
            return Err(format!("No active session for {device_id}"));
        }
        let device = (self.deps.get_device)(device_id)
            .ok_or_else(|| format!("Device {device_id} is not available"))?;
        let create_fn = self.deps.create_transport.clone();
        let dev = device.clone();
        let create: CreateTransport = Arc::new(move || {
            create_fn(&dev).ok_or_else(|| format!("{} cannot be controlled", dev.name))
        });
        let session = CastSession::new(
            device_id,
            create,
            Arc::new(Hooks(self.deps.events.clone())),
            self.deps.session_options.clone(),
        );
        s.sessions.insert(device_id.to_string(), session.clone());
        log::info!(
            "[cast] session created for {} ({:?})",
            device.name,
            device.kind
        );
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cast::model::{test_media, CastCapabilities, CastKind, CastStatus};
    use crate::cast::session::testutil::FakeTransport;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Counter {
        start: AtomicUsize,
        stop: AtomicUsize,
    }
    impl DiscoveryControl for Counter {
        fn start(&self) {
            self.start.fetch_add(1, Ordering::SeqCst);
        }
        fn stop(&self) {
            self.stop.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct NoEvents;
    impl ManagerEvents for NoEvents {
        fn devices_changed(&self, _: Vec<CastDevice>) {}
        fn session_changed(&self, _: CastSessionState) {}
        fn ended(&self, _: &str, _: Option<CastMedia>) {}
        fn error(&self, _: Option<&str>, _: String) {}
    }

    fn manager(
        discovery: Arc<Counter>,
        devices: Vec<CastDevice>,
        transport: bool,
    ) -> Arc<CastManager> {
        let (list, get) = (devices.clone(), devices);
        CastManager::new(ManagerDeps {
            discovery,
            list_devices: Box::new(move || list.clone()),
            get_device: Arc::new(move |id| get.iter().find(|d| d.id == id).cloned()),
            create_transport: Arc::new(move |_| {
                transport.then(|| Arc::new(FakeTransport::default()) as Arc<dyn CastTransport>)
            }),
            events: Arc::new(NoEvents),
            session_options: SessionOptions::default(),
        })
    }

    #[tokio::test]
    async fn works_with_no_devices_commands_reject_nothing_throws_dispose_is_clean() {
        let d = Arc::new(Counter::default());
        let m = manager(d.clone(), vec![], false);
        assert!(m.list_devices().is_empty());
        assert!(m
            .load("nope", test_media())
            .await
            .unwrap_err()
            .contains("not available"));
        assert!(m
            .play("nope")
            .await
            .unwrap_err()
            .contains("No active session"));
        m.disconnect("nope").await.unwrap();
        m.dispose().await;
        assert!(d.stop.load(Ordering::SeqCst) >= 1);
        assert!(m.load("nope", test_media()).await.is_err());
    }

    #[tokio::test]
    async fn runs_discovery_only_while_asked_after_a_short_startup_scan() {
        let d = Arc::new(Counter::default());
        let m = manager(d.clone(), vec![], false);
        m.startup_scan(60);
        assert_eq!(d.start.load(Ordering::SeqCst), 1);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(d.stop.load(Ordering::SeqCst), 1);
        m.start_discovery();
        m.stop_discovery();
        assert_eq!(d.stop.load(Ordering::SeqCst), 2);
        // Renderer asks during the startup scan: keeps running after the scan ends.
        m.startup_scan(60);
        m.start_discovery();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(d.stop.load(Ordering::SeqCst), 2);
        m.dispose().await;
    }

    #[tokio::test]
    async fn rejects_video_on_audio_only_devices_and_leaves_no_session_behind() {
        let dev = CastDevice {
            id: "upnp:x".into(),
            name: "WiiM".into(),
            kind: CastKind::Upnp,
            model: String::new(),
            manufacturer: String::new(),
            capabilities: CastCapabilities {
                audio: true,
                video: false,
            },
            address: "1.1.1.1".into(),
            port: 80,
            wiim: true,
        };
        let m = manager(Arc::new(Counter::default()), vec![dev.clone()], true);
        let video = CastMedia {
            kind: CastMediaKind::Video,
            ..test_media()
        };
        assert!(m
            .load(&dev.id, video)
            .await
            .unwrap_err()
            .contains("cannot play video"));
        assert!(m.get_session(&dev.id).is_none());
        m.load(&dev.id, test_media()).await.unwrap();
        assert_eq!(
            m.get_session(&dev.id).unwrap().status,
            CastStatus::Buffering
        );
        m.dispose().await;
    }

    #[tokio::test]
    async fn rejects_devices_that_cannot_play_anything() {
        let dev = CastDevice {
            id: "airplay:x".into(),
            name: "HomePod".into(),
            kind: CastKind::Airplay,
            model: String::new(),
            manufacturer: String::new(),
            capabilities: CastCapabilities {
                audio: false,
                video: false,
            },
            address: "1.1.1.1".into(),
            port: 7000,
            wiim: false,
        };
        let m = manager(Arc::new(Counter::default()), vec![dev.clone()], true);
        assert!(m
            .load(&dev.id, test_media())
            .await
            .unwrap_err()
            .contains("not supported"));
    }
}
