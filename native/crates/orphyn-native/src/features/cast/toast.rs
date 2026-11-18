//! Small toast stack shared by the cast and SyncPlay features (`toast()` of the design system),
//! rendered with the React app's `.jr-toast` classes.
use std::time::Duration;

use dioxus::prelude::*;

use crate::design::Icon;

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub error: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Toasts {
    items: Signal<Vec<Toast>>,
    next: Signal<u64>,
}

const MAX_VISIBLE: usize = 3;
const LIFETIME: Duration = Duration::from_secs(5);

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
    fn push(mut self, title: String, description: Option<String>, error: bool) {
        let id = *self.next.peek();
        self.next.set(id + 1);
        self.items.with_mut(|v| {
            v.push(Toast { id, title, description, error });
            let extra = v.len().saturating_sub(MAX_VISIBLE);
            v.drain(..extra);
        });
        let mut items = self.items;
        spawn(async move {
            tokio::time::sleep(LIFETIME).await;
            items.with_mut(|v| v.retain(|t| t.id != id));
        });
    }

    pub fn info(self, title: String, description: Option<String>) {
        self.push(title, description, false);
    }

    pub fn error(self, title: String, description: Option<String>) {
        self.push(title, description, true);
    }
}

fn icon_for(error: bool) -> &'static str {
    if error { "triangle-alert" } else { "info" }
}

/// Renders the stack (bottom of the window, above the player bar like the React viewport).
#[component]
pub fn ToastHost() -> Element {
    let toasts = use_toasts();
    let items = (toasts.items)();
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "jr-toast-viewport",
            div { class: "jr-toast-stack", role: "status", "aria-live": "polite",
                for t in items {
                    div { key: "{t.id}", class: "jr-toast jr-material-thick",
                        span { class: "jr-toast__icon", "aria-hidden": "true",
                            Icon { name: icon_for(t.error), size: 20 }
                        }
                        span { class: "jr-toast__text",
                            span { class: "jr-toast__title", "{t.title}" }
                            if let Some(d) = &t.description {
                                span { class: "jr-toast__description", "{d}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
