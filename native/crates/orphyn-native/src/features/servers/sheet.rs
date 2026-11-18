//! `Sheet` (`design/overlays/Sheet.tsx`) and `AddServerSheet.tsx`.
use dioxus::prelude::*;

use super::ConnectFlow;
use crate::design::IconButton;
use crate::i18n::use_i18n;

#[component]
pub fn Sheet(
    title: String,
    #[props(default)] description: String,
    #[props(default = "medium")] size: &'static str,
    on_close: EventHandler<()>,
    footer: Option<Element>,
    children: Element,
) -> Element {
    let t = use_i18n();
    rsx! {
        div { class: "jr-sheet-layer", onkeydown: move |e| if e.key() == Key::Named(NamedKey::Escape) { on_close.call(()) },
            div { class: "jr-sheet-backdrop", "aria-hidden": "true", onclick: move |_| on_close.call(()) }
            div { class: "jr-sheet jr-sheet--{size}", role: "dialog", "aria-modal": "true",
                header { class: "jr-sheet__header",
                    div { class: "jr-sheet__titles",
                        h2 { class: "jr-sheet__title", "{title}" }
                        if !description.is_empty() {
                            p { class: "jr-sheet__description", "{description}" }
                        }
                    }
                    IconButton { icon: "x", label: t.t("shell:design.close"), variant: "gray", size: "small", stroke_width: 2.5, onclick: move |_| on_close.call(()) }
                }
                div { class: "jr-sheet__body", {children} }
                if let Some(f) = footer {
                    footer { class: "jr-sheet__footer", {f} }
                }
            }
        }
    }
}

/// The onboarding flow in a sheet, to add another server.
#[component]
pub fn AddServerSheet(on_close: EventHandler<()>) -> Element {
    let t = use_i18n();
    rsx! {
        Sheet { title: t.t("servers:add.title"), description: t.t("servers:add.description"), size: "small", on_close,
            ConnectFlow { variant: "sheet", on_done: move |_| on_close.call(()) }
        }
    }
}
