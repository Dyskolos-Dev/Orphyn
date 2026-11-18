//! Shared tokio runtime of the cast module: the native UI thread has no ambient runtime, so every
//! background task (discovery, sessions, transports) is spawned here.
use std::future::Future;
use std::sync::OnceLock;

pub use tokio::task::JoinHandle;
use tokio::runtime::{Handle, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Handle of the cast runtime, created on first use.
pub fn handle() -> Handle {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("orphyn-cast")
                .enable_all()
                .build()
                .expect("cast runtime")
        })
        .handle()
        .clone()
}

/// Spawns on the cast runtime; works from any thread, with or without an ambient runtime.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(future)
}
