//! Orphyn patch: open and close extra windows at run time, each with its own `VirtualDom`.
//!
//! Upstream dioxus-native can only open the window it is launched with. [`ExtraWindows`] (a
//! context of every window, see [`use_extra_windows`]) asks the event loop for another one. The
//! `VirtualDom` is built on the UI thread by a closure (it is not `Send`), so it can capture the
//! app's services (`Arc`s, clones of channels) and provide them as root contexts.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use blitz_shell::{BlitzShellEvent, BlitzShellProxy};
use dioxus_core::{VirtualDom, consume_context, use_hook};
use winit::window::{WindowAttributes, WindowId};

use crate::DioxusNativeEvent;

pub(crate) struct Pending {
    pub attributes: WindowAttributes,
    pub build: Box<dyn FnOnce() -> VirtualDom>,
    pub on_close: Box<dyn FnOnce()>,
}

thread_local! {
    /// Requests waiting for the event loop (everything runs on the UI thread).
    pub(crate) static PENDING: RefCell<HashMap<u64, Pending>> = RefCell::new(HashMap::new());
}

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

/// An extra window asked for with [`ExtraWindows::open`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExtraWindow(pub(crate) u64);

/// Opens and closes extra windows. Clone it freely; use it on the UI thread.
#[derive(Clone)]
pub struct ExtraWindows {
    pub(crate) proxy: BlitzShellProxy,
}

impl ExtraWindows {
    /// Opens a window. `build` runs on the event loop and returns the window's `VirtualDom`;
    /// `on_close` runs (in the root scope of the window that opened it, when it is still
    /// open) after the window is gone, whoever closed it.
    pub fn open(
        &self,
        attributes: WindowAttributes,
        build: impl FnOnce() -> VirtualDom + 'static,
        on_close: impl FnOnce() + 'static,
    ) -> ExtraWindow {
        let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
        PENDING.with(|p| {
            p.borrow_mut().insert(
                ticket,
                Pending {
                    attributes,
                    build: Box::new(build),
                    on_close: Box::new(on_close),
                },
            )
        });
        self.proxy
            .send_event(BlitzShellEvent::embedder_event(DioxusNativeEvent::OpenWindow {
                ticket,
            }));
        ExtraWindow(ticket)
    }

    /// Closes a window opened with [`open`](Self::open) (no-op when it is already closed).
    pub fn close(&self, window: ExtraWindow) {
        self.proxy
            .send_event(BlitzShellEvent::embedder_event(DioxusNativeEvent::CloseWindow {
                ticket: window.0,
            }));
    }

    /// Closes a window by its winit id (a window closing itself: `use_window().id()`).
    pub fn close_window(&self, id: WindowId) {
        self.proxy
            .send_event(BlitzShellEvent::CloseWindow { window_id: id });
    }
}

/// The [`ExtraWindows`] of this app.
pub fn use_extra_windows() -> ExtraWindows {
    use_hook(consume_context::<ExtraWindows>)
}
