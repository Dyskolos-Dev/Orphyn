//! The protocol-agnostic transport contract every device protocol implements.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use super::model::{CastMedia, CastStatus};

/// Partial device-side update pushed by a transport (poll result or device event).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransportUpdate {
    /// Only `Idle`, `Playing`, `Paused` or `Buffering`.
    pub status: Option<CastStatus>,
    pub position_sec: Option<f64>,
    pub duration_sec: Option<f64>,
    /// 0..1
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    /// The current media finished naturally (not stopped by the user).
    pub ended: bool,
    /// The device reported a playback error (bad media, unsupported format).
    pub error: Option<String>,
}

impl TransportUpdate {
    pub fn ended() -> Self {
        Self {
            ended: true,
            ..Default::default()
        }
    }
}

#[derive(Clone)]
pub struct TransportHandlers {
    pub on_update: Arc<dyn Fn(TransportUpdate) + Send + Sync>,
    /// The link to the device dropped (device vanished, socket closed, repeated poll failures).
    pub on_closed: Arc<dyn Fn(Option<String>) + Send + Sync>,
}

pub type TResult<T> = Result<T, String>;
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = TResult<T>> + Send + 'a>>;

/// One protocol connection to one device. Every method returns an error string on failure and
/// never panics.
pub trait CastTransport: Send + Sync {
    fn set_handlers(&self, handlers: TransportHandlers);
    fn connect(&self) -> BoxFut<'_, ()>;
    fn load(&self, media: CastMedia) -> BoxFut<'_, ()>;
    fn play(&self) -> BoxFut<'_, ()>;
    fn pause(&self) -> BoxFut<'_, ()>;
    fn seek(&self, position_sec: f64) -> BoxFut<'_, ()>;
    fn stop(&self) -> BoxFut<'_, ()>;
    fn set_volume(&self, volume: f64) -> BoxFut<'_, ()>;
    fn set_muted(&self, muted: bool) -> BoxFut<'_, ()>;
    /// Idempotent teardown: timers, sockets, listeners.
    fn close(&self) -> BoxFut<'_, ()>;
}

pub async fn with_timeout<T>(
    fut: impl Future<Output = TResult<T>>,
    ms: u64,
    what: &str,
) -> TResult<T> {
    match tokio::time::timeout(Duration::from_millis(ms), fut).await {
        Ok(r) => r,
        Err(_) => Err(format!("{what} timed out after {ms} ms")),
    }
}
