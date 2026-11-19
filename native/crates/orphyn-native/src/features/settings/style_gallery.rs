//! `StyleGallery.tsx`: a live miniature of the app per interface style (the preview card grid of
//! Settings > Appearance) and the large preview on top of the pane.
use dioxus::prelude::*;

use crate::design::Icon;
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services};
use crate::theme::{style_defaults, INTERFACE_STYLES};

/// Size of the miniature window before it is scaled to the card (`MINI_WIDTH`).
const MINI_WIDTH: f64 = 720.0;

#[derive(Clone, Copy, PartialEq)]
pub struct PreviewLayout {
    pub sidebar: &'static str,
    pub backdrop: &'static str,
    pub video_cards: &'static str,
}

#[component]
pub fn StylePreview(
    style: &'static str,
    layout: PreviewLayout,
    #[props(default = false)] large: bool,
) -> Element {
    let mut scale = use_signal(|| 0.4_f64);
    let landscape = layout.video_cards == "landscape";
    let cards = if landscape { 5 } else { 8 };
    let rows = if large { 6 } else { 4 };
    let class = if large { "jr-style-preview jr-style-preview--large" } else { "jr-style-preview" };
    rsx! {
        div {
            class: "{class}",
            "aria-hidden": "true",
            onmounted: move |e| async move {
                // Measured after the first layout passes (the card width settles once the grid is laid out).
                for ms in [0u64, 60, 250] {
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                    if let Ok(r) = e.data().get_client_rect().await {
                        if r.size.width > 0.0 {
                            scale.set(r.size.width / MINI_WIDTH);
                        }
                    }
                }
            },
            div {
                class: "jr-style-preview__window",
                "data-style": style,
                "data-sidebar": layout.sidebar,
                "data-backdrop": layout.backdrop,
                "data-cards": layout.video_cards,
                style: "--jr-preview-scale: {scale};",
                div { class: "jr-style-preview__backdrop", div { class: "jr-style-preview__art" } }
                div { class: "jr-style-preview__sidebar",
                    span { class: "jr-style-preview__search" }
                    for i in 0..6usize {
                        span { key: "{i}", class: if i == 1 { "jr-style-preview__nav is-active" } else { "jr-style-preview__nav" },
                            i {}
                            b { style: "width: {46 + ((i * 17) % 34)}%;" }
                        }
                    }
                }
                div { class: "jr-style-preview__main",
                    div { class: "jr-style-preview__toolbar" }
                    div { class: "jr-style-preview__content",
                        div { class: "jr-style-preview__title", "Aa" }
                        div { class: "jr-style-preview__shelf",
                            for i in 0..cards {
                                span { key: "{i}", class: "jr-style-preview__card", "data-tone": "{i % 4}" }
                            }
                        }
                        div { class: "jr-style-preview__rows",
                            for i in 0..rows {
                                span { key: "{i}", class: if i == 1 { "jr-style-preview__row is-current" } else { "jr-style-preview__row" },
                                    i {}
                                    b {}
                                }
                            }
                        }
                    }
                    div { class: "jr-style-preview__player",
                        span { class: "jr-style-preview__play" }
                        span { class: "jr-style-preview__lcd" }
                    }
                }
            }
        }
    }
}

/// Settings > Appearance: the interface style picker, one live preview card per style.
#[component]
pub fn StyleGallery() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let current = app.interface.read().style;
    rsx! {
        div { class: "jr-style-gallery", role: "radiogroup", "aria-label": t.t("settings:appearance.style.label"),
            for style in INTERFACE_STYLES {
                {
                    let selected = style == current;
                    let d = style_defaults(style);
                    let svc = svc.clone();
                    let class = if selected { "jr-style-card jr-style-card--selected" } else { "jr-style-card" };
                    rsx! {
                        button {
                            key: "{style}",
                            r#type: "button",
                            role: "radio",
                            "aria-checked": selected.to_string(),
                            class: "{class}",
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |_| app.set_style(&svc, style),
                            StylePreview { style, layout: PreviewLayout { sidebar: d.sidebar, backdrop: d.backdrop, video_cards: d.video_cards } }
                            span { class: "jr-style-card__text",
                                span { class: "jr-style-card__name",
                                    {t.t(&format!("settings:appearance.styles.{style}.name"))}
                                    if selected {
                                        span { class: "jr-style-card__check", title: t.t("settings:appearance.style.selected"),
                                            Icon { name: "check", size: 13, stroke_width: 3.0 }
                                        }
                                    }
                                }
                                span { class: "jr-style-card__description", {t.t(&format!("settings:appearance.styles.{style}.description"))} }
                            }
                        }
                    }
                }
            }
        }
    }
}
