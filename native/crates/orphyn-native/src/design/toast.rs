//! `design/overlays/Toast.tsx`: the app-wide toast HUD. Same DOM and classes as React (title,
//! description, leading icon, one inline action, a close button or a text dismiss button), the
//! last three kept, errors announced in an assertive region, the timer paused while the pointer
//! or focus is on a toast.
//!
//! Mount `use_toast_provider()` once near the root and `ToastHost {}` once; any screen then calls
//! `use_toasts().show(ToastOptions { title: .., ..Default::default() })` (or `info` / `error`).
use std::time::Duration;

use dioxus::prelude::*;

use super::Icon;
use crate::i18n::use_i18n;

#[derive(Clone, Default)]
pub struct ToastOptions {
    pub title: String,
    pub description: Option<String>,
    /// Leading icon (lucide name).
    pub icon: Option<&'static str>,
    /// One inline action, e.g. "Undo".
    pub action: Option<(String, Callback<()>)>,
    /// Text button replacing the close icon (e.g. "Later").
    pub dismiss_label: Option<String>,
    /// How long it stays; `None` keeps it until dismissed. Default 4 s (`duration: 4000`).
    pub duration: Option<Duration>,
    /// `error` is announced assertively.
    pub error: bool,
}

impl ToastOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            duration: Some(DEFAULT_DURATION),
            ..Self::default()
        }
    }
}

#[derive(Clone)]
struct Record {
    id: u64,
    options: ToastOptions,
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

const DEFAULT_DURATION: Duration = Duration::from_millis(4000);
/// Like macOS, the stack stays short.
const MAX_VISIBLE: usize = 3;

#[derive(Clone, Copy, PartialEq)]
pub struct Toasts {
    items: Signal<Vec<Record>>,
    next: Signal<u64>,
}

pub fn use_toast_provider() -> Toasts {
    use_context_provider(|| Toasts {
        items: Signal::new(Vec::new()),
        next: Signal::new(1),
    })
}

pub fn use_toasts() -> Toasts {
    use_context::<Toasts>()
}

impl Toasts {
    pub fn show(mut self, options: ToastOptions) -> u64 {
        let id = *self.next.peek();
        self.next.set(id + 1);
        self.items.with_mut(|v| {
            v.push(Record { id, options });
            let extra = v.len().saturating_sub(MAX_VISIBLE);
            v.drain(..extra);
        });
        id
    }

    pub fn info(self, title: String, description: Option<String>) {
        let mut o = ToastOptions::new(title);
        o.description = description;
        o.icon = Some("info");
        self.show(o);
    }

    pub fn error(self, title: String, description: Option<String>) {
        let mut o = ToastOptions::new(title);
        o.description = description;
        o.error = true;
        o.icon = Some("triangle-alert");
        self.show(o);
    }

    pub fn dismiss(mut self, id: u64) {
        self.items.with_mut(|v| v.retain(|t| t.id != id));
    }
}

#[component]
pub fn ToastHost() -> Element {
    let toasts = use_toasts();
    let items = (toasts.items)();
    if items.is_empty() {
        return rsx! {};
    }
    let polite: Vec<Record> = items.iter().filter(|t| !t.options.error).cloned().collect();
    let assertive: Vec<Record> = items.iter().filter(|t| t.options.error).cloned().collect();
    rsx! {
        div { class: "jr-toast-viewport",
            div { class: "jr-toast-stack", role: "status", "aria-live": "polite",
                for r in polite {
                    ToastItem { key: "{r.id}", id: r.id }
                }
            }
            div { class: "jr-toast-stack", role: "alert", "aria-live": "assertive",
                for r in assertive {
                    ToastItem { key: "{r.id}", id: r.id }
                }
            }
        }
    }
}

#[component]
fn ToastItem(id: u64) -> Element {
    let t = use_i18n();
    let toasts = use_toasts();
    let Some(options) = toasts
        .items
        .peek()
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.options.clone())
    else {
        return rsx! {};
    };
    let mut paused = use_signal(|| false);
    // The timer only runs down while nothing hovers or focuses the toast.
    use_future({
        let duration = options.duration;
        move || async move {
            let Some(total) = duration else { return };
            let mut left = total;
            let tick = Duration::from_millis(100);
            while left > Duration::ZERO {
                tokio::time::sleep(tick).await;
                if !*paused.peek() {
                    left = left.saturating_sub(tick);
                }
            }
            toasts.dismiss(id);
        }
    });
    let tone = if options.error { "error" } else { "default" };
    let action = options.action.clone();
    rsx! {
        div {
            class: "jr-toast jr-material-thick",
            "data-tone": tone,
            onmouseenter: move |_| paused.set(true),
            onmouseleave: move |_| paused.set(false),
            onfocusin: move |_| paused.set(true),
            onfocusout: move |_| paused.set(false),
            if let Some(icon) = options.icon {
                span { class: "jr-toast__icon", "aria-hidden": "true", Icon { name: icon, size: 20 } }
            }
            span { class: "jr-toast__text",
                span { class: "jr-toast__title", "{options.title}" }
                if let Some(d) = &options.description {
                    span { class: "jr-toast__description", "{d}" }
                }
            }
            if let Some((label, on_click)) = action {
                button {
                    r#type: "button",
                    class: "jr-toast__action",
                    onclick: move |_| {
                        on_click.call(());
                        toasts.dismiss(id);
                    },
                    "{label}"
                }
            }
            if let Some(label) = &options.dismiss_label {
                button {
                    r#type: "button",
                    class: "jr-toast__action",
                    onclick: move |_| toasts.dismiss(id),
                    "{label}"
                }
            } else {
                button {
                    r#type: "button",
                    class: "jr-toast__close",
                    "aria-label": t.t("shell:design.dismiss"),
                    onclick: move |_| toasts.dismiss(id),
                    Icon { name: "x", size: 14, stroke_width: 2.5 }
                }
            }
        }
    }
}
