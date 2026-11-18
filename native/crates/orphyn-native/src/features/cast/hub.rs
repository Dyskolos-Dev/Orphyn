//! Process-wide cast service plus the plumbing between its background threads and the UI:
//! handlers see every event on the cast thread (the music remote output answers the player
//! there), and everything the UI shows is forwarded through one channel into signals.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use orphyn_core::cast::{CastEvent, CastService};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Message for the UI loop (`CastUi` provider).
#[derive(Debug, Clone)]
pub enum UiMsg {
    Event(CastEvent),
    /// A device gave up while it played something for the app (refused media, link lost).
    Failure { device_id: String, message: String },
}

pub type Handler = Arc<dyn Fn(&CastEvent) + Send + Sync>;

pub struct CastHub {
    pub service: CastService,
    handlers: Mutex<HashMap<u64, Handler>>,
    next: AtomicU64,
    ui_tx: UnboundedSender<UiMsg>,
    ui_rx: Mutex<Option<UnboundedReceiver<UiMsg>>>,
}

static HUB: OnceLock<Arc<CastHub>> = OnceLock::new();

/// The cast service (created, with its warm-up scan, on first use).
pub fn hub() -> Arc<CastHub> {
    HUB.get_or_init(|| {
        let (ui_tx, ui_rx) = unbounded_channel();
        Arc::new_cyclic(|weak: &std::sync::Weak<CastHub>| {
            let weak = weak.clone();
            let tx = ui_tx.clone();
            let service = CastService::new(Arc::new(move |event| {
                if let Some(hub) = weak.upgrade() {
                    let handlers: Vec<Handler> = hub
                        .handlers
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .values()
                        .cloned()
                        .collect();
                    for h in handlers {
                        h(&event);
                    }
                }
                let _ = tx.send(UiMsg::Event(event));
            }));
            CastHub {
                service,
                handlers: Mutex::new(HashMap::new()),
                next: AtomicU64::new(1),
                ui_tx,
                ui_rx: Mutex::new(Some(ui_rx)),
            }
        })
    })
    .clone()
}

impl CastHub {
    /// Registers a handler called on the cast thread for every event; returns its id.
    pub fn add_handler(&self, handler: Handler) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.handlers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, handler);
        id
    }

    pub fn remove_handler(&self, id: u64) {
        self.handlers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
    }

    pub fn fail(&self, device_id: &str, message: String) {
        let _ = self.ui_tx.send(UiMsg::Failure {
            device_id: device_id.to_string(),
            message,
        });
    }

    /// The UI side of the channel; only the first caller gets it.
    pub fn take_ui_receiver(&self) -> Option<UnboundedReceiver<UiMsg>> {
        self.ui_rx.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}
