//! Per-device session: owns one transport, serializes commands, reduces transport updates into a
//! `CastSessionState`, emits "ended" once per track and reconnects when the link drops mid-playback.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use super::runtime::{spawn, JoinHandle};

use super::model::{CastMedia, CastSessionState, CastStatus};
use super::transport::{with_timeout, CastTransport, TResult, TransportHandlers, TransportUpdate};

pub trait SessionHooks: Send + Sync {
    fn on_state(&self, state: CastSessionState);
    fn on_ended(&self, device_id: &str, media: Option<CastMedia>);
    fn on_error(&self, device_id: &str, message: String);
}

#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// Delays before each reconnect attempt after an unexpected link loss.
    pub reconnect_delays_ms: Vec<u64>,
    pub connect_timeout_ms: u64,
    pub command_timeout_ms: u64,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            reconnect_delays_ms: vec![500, 2000, 5000],
            connect_timeout_ms: 10_000,
            command_timeout_ms: 10_000,
        }
    }
}

pub type CreateTransport = Arc<dyn Fn() -> TResult<Arc<dyn CastTransport>> + Send + Sync>;

struct Inner {
    state: CastSessionState,
    transport: Option<Arc<dyn CastTransport>>,
    /// Identifies the current transport so late callbacks from an old one are ignored.
    transport_seq: u64,
    connected: bool,
    disposed: bool,
    /// Bumped by load/stop/disconnect to invalidate pending reconnects.
    generation: u64,
    reconnect: Option<JoinHandle<()>>,
}

pub struct CastSession {
    pub device_id: String,
    create: CreateTransport,
    hooks: Arc<dyn SessionHooks>,
    opts: SessionOptions,
    inner: Mutex<Inner>,
    /// Serializes commands (the equivalent of the promise chain in the Electron version).
    chain: tokio::sync::Mutex<()>,
    me: Weak<CastSession>,
}

impl CastSession {
    pub fn new(
        device_id: &str,
        create: CreateTransport,
        hooks: Arc<dyn SessionHooks>,
        opts: SessionOptions,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            device_id: device_id.to_string(),
            create,
            hooks,
            opts,
            inner: Mutex::new(Inner {
                state: CastSessionState::idle(device_id),
                transport: None,
                transport_seq: 0,
                connected: false,
                disposed: false,
                generation: 0,
                reconnect: None,
            }),
            chain: tokio::sync::Mutex::new(()),
            me: me.clone(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn get_state(&self) -> CastSessionState {
        self.lock().state.clone()
    }

    fn is_disposed(&self) -> bool {
        self.lock().disposed
    }

    // --- commands -----------------------------------------------------------------------------

    pub async fn load(&self, media: CastMedia) -> TResult<()> {
        let _turn = self.chain.lock().await;
        self.bump_generation();
        self.cancel_reconnect();
        let m = media.clone();
        self.patch(move |s| {
            s.status = CastStatus::Connecting;
            s.position_sec = m.start_position_sec.unwrap_or(0.0);
            s.duration_sec = m.duration_sec.unwrap_or(0.0);
            s.media = Some(m);
            s.error = None;
        });
        let result: TResult<()> = async {
            self.ensure_connected().await?;
            self.patch(|s| s.status = CastStatus::Buffering);
            let t = self.require_transport()?;
            self.call(t.load(media), "load").await
        }
        .await;
        if let Err(e) = &result {
            self.fail(e);
        }
        result
    }

    pub async fn play(&self) -> TResult<()> {
        let _turn = self.chain.lock().await;
        let t = self.connected_transport()?;
        let r = self.call(t.play(), "play").await;
        self.finish_command("play", r)
    }

    pub async fn pause(&self) -> TResult<()> {
        let _turn = self.chain.lock().await;
        let t = self.connected_transport()?;
        let r = self.call(t.pause(), "pause").await;
        if r.is_ok() {
            self.patch(|s| s.status = CastStatus::Paused);
        }
        self.finish_command("pause", r)
    }

    pub async fn seek(&self, position_sec: f64) -> TResult<()> {
        let pos = if position_sec.is_finite() {
            position_sec.max(0.0)
        } else {
            0.0
        };
        let _turn = self.chain.lock().await;
        let t = self.connected_transport()?;
        let r = self.call(t.seek(pos), "seek").await;
        if r.is_ok() {
            self.patch(move |s| s.position_sec = pos);
        }
        self.finish_command("seek", r)
    }

    pub async fn stop(&self) -> TResult<()> {
        let _turn = self.chain.lock().await;
        self.bump_generation();
        self.cancel_reconnect();
        let (t, connected) = {
            let g = self.lock();
            (g.transport.clone(), g.connected)
        };
        self.patch(|s| {
            s.status = CastStatus::Idle;
            s.media = None;
            s.position_sec = 0.0;
            s.duration_sec = 0.0;
        });
        if let (Some(t), true) = (t, connected) {
            if let Err(e) = self.call(t.stop(), "stop").await {
                log::warn!("[cast] stop failed on {}: {e}", self.device_id);
            }
        }
        Ok(())
    }

    pub async fn set_volume(&self, volume: f64) -> TResult<()> {
        let v = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let _turn = self.chain.lock().await;
        let t = self.connected_transport()?;
        let r = self.call(t.set_volume(v), "setVolume").await;
        if r.is_ok() {
            self.patch(move |s| s.volume = v);
        }
        self.finish_command("setVolume", r)
    }

    pub async fn set_muted(&self, muted: bool) -> TResult<()> {
        let _turn = self.chain.lock().await;
        let t = self.connected_transport()?;
        let r = self.call(t.set_muted(muted), "setMuted").await;
        if r.is_ok() {
            self.patch(move |s| s.muted = muted);
        }
        self.finish_command("setMuted", r)
    }

    /// Stops playback on the device (best effort), closes the transport and releases everything.
    pub async fn disconnect(&self) -> TResult<()> {
        let _turn = self.chain.lock().await;
        self.bump_generation();
        self.cancel_reconnect();
        let (t, connected, has_media) = {
            let g = self.lock();
            (g.transport.clone(), g.connected, g.state.media.is_some())
        };
        if let (Some(t), true, true) = (t, connected, has_media) {
            // Device already gone: nothing to do on failure.
            let _ = self.call(t.stop(), "stop").await;
        }
        self.close_transport().await;
        self.patch(|s| {
            s.status = CastStatus::Idle;
            s.media = None;
            s.position_sec = 0.0;
            s.duration_sec = 0.0;
            s.error = None;
        });
        Ok(())
    }

    /// Teardown for app quit: no further events are emitted.
    pub async fn dispose(&self) {
        {
            let mut g = self.lock();
            g.disposed = true;
        }
        self.cancel_reconnect();
        self.close_transport().await;
    }

    // --- internals ----------------------------------------------------------------------------

    async fn call<T>(
        &self,
        fut: impl std::future::Future<Output = TResult<T>>,
        what: &str,
    ) -> TResult<T> {
        with_timeout(fut, self.opts.command_timeout_ms, what).await
    }

    fn connected_transport(&self) -> TResult<Arc<dyn CastTransport>> {
        let g = self.lock();
        match (&g.transport, g.connected) {
            (Some(t), true) => Ok(t.clone()),
            _ => Err(format!("Not connected to {}", self.device_id)),
        }
    }

    fn require_transport(&self) -> TResult<Arc<dyn CastTransport>> {
        self.lock()
            .transport
            .clone()
            .ok_or_else(|| "No transport".to_string())
    }

    fn finish_command(&self, name: &str, result: TResult<()>) -> TResult<()> {
        if let Err(e) = &result {
            self.hooks
                .on_error(&self.device_id, format!("{name} failed: {e}"));
        }
        result
    }

    fn bump_generation(&self) {
        self.lock().generation += 1;
    }

    fn is_current(&self, seq: u64) -> bool {
        let g = self.lock();
        g.transport.is_some() && g.transport_seq == seq
    }

    async fn ensure_connected(&self) -> TResult<()> {
        {
            let g = self.lock();
            if g.transport.is_some() && g.connected {
                return Ok(());
            }
        }
        self.close_transport().await;
        let t = (self.create)()?;
        let seq = {
            let mut g = self.lock();
            g.transport_seq += 1;
            g.transport = Some(t.clone());
            g.transport_seq
        };
        let on_update = {
            let me = self.me.clone();
            Arc::new(move |u: TransportUpdate| {
                if let Some(s) = me.upgrade() {
                    if s.is_current(seq) {
                        s.apply_update(u);
                    }
                }
            })
        };
        let on_closed = {
            let me = self.me.clone();
            Arc::new(move |err: Option<String>| {
                if let Some(s) = me.upgrade() {
                    if s.is_current(seq) {
                        s.handle_closed(err);
                    }
                }
            })
        };
        t.set_handlers(TransportHandlers {
            on_update,
            on_closed,
        });
        with_timeout(t.connect(), self.opts.connect_timeout_ms, "connect").await?;
        let mut g = self.lock();
        if g.transport_seq != seq || g.disposed {
            return Err("Session closed while connecting".into());
        }
        g.connected = true;
        Ok(())
    }

    /// Detaches the current transport synchronously; returns it so the caller can close it.
    fn take_transport(&self) -> Option<Arc<dyn CastTransport>> {
        let mut g = self.lock();
        g.connected = false;
        g.transport.take()
    }

    async fn close_transport(&self) {
        if let Some(t) = self.take_transport() {
            if let Err(e) = t.close().await {
                log::warn!("[cast] close failed on {}: {e}", self.device_id);
            }
        }
    }

    fn apply_update(&self, u: TransportUpdate) {
        let media = match self.lock().state.media.clone() {
            // After stop() / before load() nothing should revive the session.
            None => return,
            Some(m) => m,
        };
        if let Some(err) = u.error {
            self.fail(&err);
            return;
        }
        if u.ended {
            self.patch(|s| {
                s.status = CastStatus::Idle;
                s.media = None;
                s.position_sec = 0.0;
                s.duration_sec = 0.0;
            });
            if !self.is_disposed() {
                self.hooks.on_ended(&self.device_id, Some(media));
            }
            return;
        }
        self.patch(move |s| {
            if let Some(status) = u.status {
                // A transient 'idle' while a load is in flight is not a state change we want to show.
                if !(status == CastStatus::Idle && s.status == CastStatus::Buffering) {
                    s.status = status;
                }
            }
            if let Some(p) = u.position_sec {
                s.position_sec = p;
            }
            if let Some(d) = u.duration_sec.filter(|d| *d > 0.0) {
                s.duration_sec = d;
            }
            if let Some(v) = u.volume {
                s.volume = v;
            }
            if let Some(m) = u.muted {
                s.muted = m;
            }
        });
    }

    fn handle_closed(&self, err: Option<String>) {
        let (media, status, position, generation) = {
            let g = self.lock();
            (
                g.state.media.clone(),
                g.state.status,
                g.state.position_sec,
                g.generation,
            )
        };
        let was_active = media.is_some() && status != CastStatus::Idle;
        let resume_paused = status == CastStatus::Paused;
        let old = self.take_transport();
        if let Some(t) = old {
            let id = self.device_id.clone();
            spawn(async move {
                if let Err(e) = t.close().await {
                    log::warn!("[cast] close failed on {id}: {e}");
                }
            });
        }
        let Some(media) = media.filter(|_| was_active) else {
            self.patch(|s| s.status = CastStatus::Idle);
            return;
        };
        log::warn!(
            "[cast] link to {} lost{}, reconnecting",
            self.device_id,
            err.map(|e| format!(": {e}")).unwrap_or_default()
        );
        self.patch(|s| s.status = CastStatus::Connecting);
        let media = CastMedia {
            start_position_sec: Some(position),
            ..media
        };
        self.schedule_reconnect(0, generation, media, resume_paused);
    }

    fn schedule_reconnect(&self, attempt: usize, gen: u64, media: CastMedia, resume_paused: bool) {
        let Some(delay) = self.opts.reconnect_delays_ms.get(attempt).copied() else {
            self.fail("Device unreachable");
            return;
        };
        let me = self.me.clone();
        let handle = spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            if let Some(s) = me.upgrade() {
                s.reconnect_attempt(attempt, gen, media, resume_paused)
                    .await;
            }
        });
        let mut g = self.lock();
        if let Some(old) = g.reconnect.replace(handle) {
            // The previous timer already fired (it is the task calling us): nothing to abort.
            drop(old);
        }
    }

    async fn reconnect_attempt(
        &self,
        attempt: usize,
        gen: u64,
        media: CastMedia,
        resume_paused: bool,
    ) {
        let _turn = self.chain.lock().await;
        {
            let g = self.lock();
            if gen != g.generation || g.disposed {
                return;
            }
        }
        let result: TResult<()> = async {
            self.ensure_connected().await?;
            let t = self.require_transport()?;
            self.call(t.load(media.clone()), "load").await?;
            if resume_paused {
                let t = self.require_transport()?;
                self.call(t.pause(), "pause").await?;
            }
            self.patch(move |s| {
                s.status = if resume_paused {
                    CastStatus::Paused
                } else {
                    CastStatus::Buffering
                }
            });
            Ok(())
        }
        .await;
        if let Err(e) = result {
            log::warn!(
                "[cast] reconnect {} to {} failed: {e}",
                attempt + 1,
                self.device_id
            );
            self.close_transport().await;
            if gen == self.lock().generation {
                self.schedule_reconnect(attempt + 1, gen, media, resume_paused);
            }
        }
    }

    fn cancel_reconnect(&self) {
        if let Some(h) = self.lock().reconnect.take() {
            h.abort();
        }
    }

    fn fail(&self, message: &str) {
        log::error!("[cast] {}: {message}", self.device_id);
        let m = message.to_string();
        self.patch(move |s| {
            s.status = CastStatus::Error;
            s.error = Some(m);
        });
        if !self.is_disposed() {
            self.hooks.on_error(&self.device_id, message.to_string());
        }
    }

    fn patch(&self, f: impl FnOnce(&mut CastSessionState)) {
        let next = {
            let mut g = self.lock();
            let mut next = g.state.clone();
            f(&mut next);
            if next == g.state {
                return;
            }
            g.state = next.clone();
            if g.disposed {
                return;
            }
            next
        };
        self.hooks.on_state(next);
    }
}

#[cfg(test)]
pub mod testutil {
    use super::*;
    use crate::cast::transport::BoxFut;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    pub struct FakeTransport {
        pub handlers: Mutex<Option<TransportHandlers>>,
        pub calls: Mutex<Vec<String>>,
        pub fail_connect: AtomicBool,
        pub fail_load: AtomicBool,
        pub fail_pause: AtomicBool,
    }

    impl FakeTransport {
        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
        pub fn update(&self, u: TransportUpdate) {
            let h = self.handlers.lock().unwrap().clone().expect("handlers set");
            (h.on_update)(u);
        }
        pub fn closed(&self, err: Option<&str>) {
            let h = self.handlers.lock().unwrap().clone().expect("handlers set");
            (h.on_closed)(err.map(String::from));
        }
        fn record(&self, c: String) {
            self.calls.lock().unwrap().push(c);
        }
    }

    impl CastTransport for FakeTransport {
        fn set_handlers(&self, h: TransportHandlers) {
            *self.handlers.lock().unwrap() = Some(h);
        }
        fn connect(&self) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record("connect".into());
                if self.fail_connect.load(Ordering::SeqCst) {
                    return Err("unreachable".into());
                }
                Ok(())
            })
        }
        fn load(&self, m: CastMedia) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record(format!("load:{}", m.start_position_sec.unwrap_or(0.0)));
                if self.fail_load.load(Ordering::SeqCst) {
                    return Err("refused".into());
                }
                Ok(())
            })
        }
        fn play(&self) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record("play".into());
                Ok(())
            })
        }
        fn pause(&self) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record("pause".into());
                if self.fail_pause.load(Ordering::SeqCst) {
                    return Err("boom".into());
                }
                Ok(())
            })
        }
        fn seek(&self, s: f64) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record(format!("seek:{s}"));
                Ok(())
            })
        }
        fn stop(&self) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record("stop".into());
                Ok(())
            })
        }
        fn set_volume(&self, v: f64) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record(format!("vol:{v}"));
                Ok(())
            })
        }
        fn set_muted(&self, m: bool) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record(format!("mute:{m}"));
                Ok(())
            })
        }
        fn close(&self) -> BoxFut<'_, ()> {
            Box::pin(async move {
                self.record("close".into());
                Ok(())
            })
        }
    }

    #[derive(Default)]
    pub struct Recorder {
        pub states: Mutex<Vec<CastSessionState>>,
        pub ended: Mutex<Vec<Option<CastMedia>>>,
        pub errors: Mutex<Vec<String>>,
    }

    impl SessionHooks for Recorder {
        fn on_state(&self, s: CastSessionState) {
            self.states.lock().unwrap().push(s);
        }
        fn on_ended(&self, _id: &str, m: Option<CastMedia>) {
            self.ended.lock().unwrap().push(m);
        }
        fn on_error(&self, _id: &str, message: String) {
            self.errors.lock().unwrap().push(message);
        }
    }

    pub struct Setup {
        pub session: Arc<CastSession>,
        pub transports: Arc<Mutex<Vec<Arc<FakeTransport>>>>,
        pub rec: Arc<Recorder>,
        /// Applied to every transport created afterwards.
        pub fail_connect: Arc<AtomicBool>,
        pub fail_load: Arc<AtomicBool>,
        pub fail_pause: Arc<AtomicBool>,
    }

    impl Setup {
        pub fn t(&self, i: usize) -> Arc<FakeTransport> {
            self.transports.lock().unwrap()[i].clone()
        }
        pub fn count(&self) -> usize {
            self.transports.lock().unwrap().len()
        }
        pub fn statuses(&self) -> Vec<CastStatus> {
            self.rec
                .states
                .lock()
                .unwrap()
                .iter()
                .map(|s| s.status)
                .collect()
        }
    }

    pub fn setup() -> Setup {
        let transports: Arc<Mutex<Vec<Arc<FakeTransport>>>> = Arc::default();
        let (fc, fl, fp) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let rec = Arc::new(Recorder::default());
        let (tl, c1, c2, c3) = (transports.clone(), fc.clone(), fl.clone(), fp.clone());
        let create: CreateTransport = Arc::new(move || {
            let t = Arc::new(FakeTransport::default());
            t.fail_connect
                .store(c1.load(Ordering::SeqCst), Ordering::SeqCst);
            t.fail_load
                .store(c2.load(Ordering::SeqCst), Ordering::SeqCst);
            t.fail_pause
                .store(c3.load(Ordering::SeqCst), Ordering::SeqCst);
            tl.lock().unwrap().push(t.clone());
            Ok(t as Arc<dyn CastTransport>)
        });
        let session = CastSession::new(
            "dev",
            create,
            rec.clone(),
            SessionOptions {
                reconnect_delays_ms: vec![10, 20],
                ..Default::default()
            },
        );
        Setup {
            session,
            transports,
            rec,
            fail_connect: fc,
            fail_load: fl,
            fail_pause: fp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use crate::cast::model::test_media;
    use std::sync::atomic::Ordering;

    fn upd(status: CastStatus) -> TransportUpdate {
        TransportUpdate {
            status: Some(status),
            ..Default::default()
        }
    }

    async fn settle(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    #[tokio::test]
    async fn goes_idle_connecting_buffering_playing_paused() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        assert_eq!(
            s.statuses(),
            vec![CastStatus::Connecting, CastStatus::Buffering]
        );
        s.t(0).update(TransportUpdate {
            status: Some(CastStatus::Playing),
            position_sec: Some(3.0),
            duration_sec: Some(100.0),
            volume: Some(0.4),
            ..Default::default()
        });
        let st = s.session.get_state();
        assert_eq!(
            (st.status, st.position_sec, st.duration_sec, st.volume),
            (CastStatus::Playing, 3.0, 100.0, 0.4)
        );
        s.session.pause().await.unwrap();
        assert_eq!(s.session.get_state().status, CastStatus::Paused);
        s.session.seek(42.0).await.unwrap();
        assert_eq!(s.session.get_state().position_sec, 42.0);
        s.session.set_volume(2.0).await.unwrap();
        assert_eq!(s.session.get_state().volume, 1.0); // clamped
        s.session.set_muted(true).await.unwrap();
        assert!(s.session.get_state().muted);
        assert_eq!(
            s.t(0).calls(),
            vec![
                "connect",
                "load:0",
                "pause",
                "seek:42",
                "vol:1",
                "mute:true"
            ]
        );
    }

    #[tokio::test]
    async fn emits_ended_exactly_once_and_returns_to_idle() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        let t = s.t(0);
        t.update(upd(CastStatus::Playing));
        t.update(TransportUpdate::ended());
        t.update(TransportUpdate::ended());
        assert_eq!(*s.rec.ended.lock().unwrap(), vec![Some(test_media())]);
        let st = s.session.get_state();
        assert_eq!(st.status, CastStatus::Idle);
        assert!(st.media.is_none());
    }

    #[tokio::test]
    async fn does_not_emit_ended_or_revive_the_session_after_a_user_stop() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.session.stop().await.unwrap();
        s.t(0).update(upd(CastStatus::Playing));
        s.t(0).update(TransportUpdate::ended());
        assert!(s.rec.ended.lock().unwrap().is_empty());
        assert_eq!(s.session.get_state().status, CastStatus::Idle);
    }

    #[tokio::test]
    async fn turns_a_failed_connect_or_load_into_an_error_state_and_a_rejected_result() {
        let a = setup();
        a.fail_connect.store(true, Ordering::SeqCst);
        assert_eq!(
            a.session.load(test_media()).await.unwrap_err(),
            "unreachable"
        );
        let st = a.session.get_state();
        assert_eq!(
            (st.status, st.error.as_deref()),
            (CastStatus::Error, Some("unreachable"))
        );
        assert_eq!(*a.rec.errors.lock().unwrap(), vec!["unreachable"]);

        let b = setup();
        b.fail_load.store(true, Ordering::SeqCst);
        assert_eq!(b.session.load(test_media()).await.unwrap_err(), "refused");
        assert_eq!(b.session.get_state().status, CastStatus::Error);
    }

    #[tokio::test]
    async fn reports_a_device_playback_error() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.t(0).update(TransportUpdate {
            error: Some("bad media".into()),
            ..Default::default()
        });
        let st = s.session.get_state();
        assert_eq!(
            (st.status, st.error.as_deref()),
            (CastStatus::Error, Some("bad media"))
        );
        assert!(s
            .rec
            .errors
            .lock()
            .unwrap()
            .contains(&"bad media".to_string()));
    }

    #[tokio::test]
    async fn rejects_commands_when_not_connected() {
        let s = setup();
        assert!(s
            .session
            .play()
            .await
            .unwrap_err()
            .contains("Not connected"));
    }

    #[tokio::test]
    async fn keeps_the_queue_usable_after_a_failing_command() {
        let s = setup();
        s.fail_pause.store(true, Ordering::SeqCst);
        s.session.load(test_media()).await.unwrap();
        assert_eq!(s.session.pause().await.unwrap_err(), "boom");
        s.session.play().await.unwrap();
        assert!(s.t(0).calls().contains(&"play".to_string()));
        assert!(s
            .rec
            .errors
            .lock()
            .unwrap()
            .iter()
            .any(|e| e == "pause failed: boom"));
    }

    #[tokio::test]
    async fn reconnects_when_the_link_drops_mid_playback_and_resumes_at_the_last_position() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.t(0).update(TransportUpdate {
            status: Some(CastStatus::Playing),
            position_sec: Some(30.0),
            ..Default::default()
        });
        s.t(0).closed(Some("socket closed"));
        assert_eq!(s.session.get_state().status, CastStatus::Connecting);
        settle(150).await;
        assert_eq!(s.count(), 2);
        assert_eq!(s.t(1).calls(), vec!["connect", "load:30"]);
        assert_eq!(s.session.get_state().status, CastStatus::Buffering);
    }

    #[tokio::test]
    async fn gives_up_with_an_error_when_the_device_vanished_for_good() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.t(0).update(TransportUpdate {
            status: Some(CastStatus::Playing),
            position_sec: Some(5.0),
            ..Default::default()
        });
        s.fail_connect.store(true, Ordering::SeqCst);
        s.t(0).closed(Some("gone"));
        settle(300).await;
        let st = s.session.get_state();
        assert_eq!(
            (st.status, st.error.as_deref()),
            (CastStatus::Error, Some("Device unreachable"))
        );
        assert_eq!(*s.rec.errors.lock().unwrap(), vec!["Device unreachable"]);
    }

    #[tokio::test]
    async fn goes_quietly_idle_when_an_idle_session_loses_its_link() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.session.stop().await.unwrap();
        s.t(0).closed(Some("gone"));
        assert_eq!(s.session.get_state().status, CastStatus::Idle);
        assert!(s.rec.errors.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn disconnect_cancels_a_pending_reconnect() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        s.t(0).update(upd(CastStatus::Playing));
        s.t(0).closed(None);
        s.session.disconnect().await.unwrap();
        settle(150).await;
        assert_eq!(s.count(), 1);
        assert_eq!(s.session.get_state().status, CastStatus::Idle);
    }

    #[tokio::test]
    async fn dispose_closes_the_transport_and_emits_nothing_more() {
        let s = setup();
        s.session.load(test_media()).await.unwrap();
        let before = s.rec.states.lock().unwrap().len();
        s.session.dispose().await;
        s.t(0).update(upd(CastStatus::Playing));
        assert!(s.t(0).calls().contains(&"close".to_string()));
        assert_eq!(s.rec.states.lock().unwrap().len(), before);
    }
}
