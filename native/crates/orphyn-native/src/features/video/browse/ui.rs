//! RSX twins of the `design` components the video screens use and `design/mod.rs` does not have
//! yet (cards, hero banner, grid, skeleton, menu button). Each renders the very DOM of its React
//! counterpart, so the shared CSS styles it unchanged.
use std::rc::Rc;

use dioxus::prelude::*;

use crate::design::{use_overlay, Icon, IconButton, MenuItem};
use crate::i18n::use_i18n;
use crate::router::use_router;

/// `initials` of `Artwork.tsx` ("Daft Punk" -> "DP").
pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let first = parts.first().and_then(|p| p.chars().next());
    let last = if parts.len() > 1 {
        parts.last().and_then(|p| p.chars().next())
    } else {
        None
    };
    first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect()
}

/// `Artwork` with `fill`, initials fallback and an object position (the design one has neither).
#[component]
pub fn Art(
    src: Option<String>,
    #[props(default = "1 / 1")] ratio: &'static str,
    #[props(default = "rounded")] shape: &'static str,
    #[props(default = false)] shadow: bool,
    #[props(default = false)] fill: bool,
    #[props(default = "image-off")] fallback: &'static str,
    fallback_text: Option<String>,
    object_position: Option<&'static str>,
) -> Element {
    let mut failed = use_signal(|| false);
    let mut last_src = use_signal(|| src.clone());
    if *last_src.peek() != src {
        last_src.set(src.clone());
        failed.set(false);
    }
    let status = if src.is_none() || failed() {
        "error"
    } else {
        "loaded"
    };
    let shadow_cls = if shadow { " jr-artwork--shadow" } else { "" };
    let fill_cls = if fill { " jr-artwork--fill" } else { "" };
    let style = (!fill).then(|| format!("aspect-ratio: {ratio};"));
    rsx! {
        div {
            class: "jr-artwork jr-artwork--{shape} jr-artwork--{status}{shadow_cls}{fill_cls}",
            style,
            "data-status": status,
            if status == "error" {
                div { class: "jr-artwork__fallback", "aria-hidden": "true",
                    if let Some(text) = fallback_text {
                        span { class: "jr-card__initials", "{text}" }
                    } else {
                        Icon { name: fallback, size: 24, stroke_width: 1.5 }
                    }
                }
            } else {
                img {
                    class: "jr-artwork__img",
                    src: src.clone().unwrap_or_default(),
                    draggable: "false",
                    style: object_position.map(|p| format!("object-position: {p};")),
                    onerror: move |_| failed.set(true),
                }
            }
        }
    }
}

/// `Skeleton` (`design/feedback/Skeleton.tsx`).
#[component]
pub fn Skeleton(
    #[props(default = "rect")] shape: &'static str,
    aspect_ratio: Option<&'static str>,
    radius: Option<&'static str>,
) -> Element {
    let mut style = String::new();
    if let Some(a) = aspect_ratio {
        style.push_str(&format!("aspect-ratio: {a};"));
    }
    if let Some(r) = radius {
        style.push_str(&format!("border-radius: {r};"));
    }
    rsx! {
        span {
            "aria-hidden": "true",
            class: "jr-skeleton jr-skeleton--{shape}",
            style: (!style.is_empty()).then_some(style),
        }
    }
}

/// `CardProgress` of `cards.tsx`.
#[component]
fn CardProgress(value: f64, #[props(default = false)] inline: bool) -> Element {
    let t = use_i18n();
    let pct = (value.clamp(0.0, 1.0) * 100.0).round() as i64;
    let cls = if inline {
        "jr-card__progress jr-card__progress--inline"
    } else {
        "jr-card__progress"
    };
    rsx! {
        span {
            class: "{cls}",
            role: "progressbar",
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-valuenow": "{pct}",
            "aria-label": t.tf("shell:design.progress", &[("percent", pct.to_string())]),
            span { class: "jr-card__progress-fill", style: "width: {pct}%;" }
        }
    }
}

/// The shapes of `MediaCard`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Poster,
    Landscape,
    Round,
}

impl CardKind {
    fn parts(self) -> (&'static str, &'static str, &'static str) {
        // (class suffix, ratio, artwork shape)
        match self {
            CardKind::Poster => ("poster", "2 / 3", "poster"),
            CardKind::Landscape => ("landscape", "16 / 9", "poster"),
            CardKind::Round => ("round", "1 / 1", "circle"),
        }
    }
}

/// `PosterCard`, `LandscapeCard` and `RoundCard` of `design/media/cards.tsx`.
#[component]
pub fn MediaCard(
    kind: CardKind,
    title: String,
    subtitle: Option<String>,
    /// Third line of a landscape card (an episode synopsis).
    description: Option<String>,
    image: Option<String>,
    to: Option<String>,
    /// 0..1 watched progress.
    progress: Option<f64>,
    /// Text in the artwork's bottom-left corner of a landscape card ("24 min left").
    caption: Option<String>,
    /// Top-right badge (unplayed count).
    badge: Option<String>,
    #[props(default = "film")] fallback: &'static str,
    #[props(default = false)] selected: bool,
    on_play: Option<EventHandler<()>>,
    on_context: Option<EventHandler<MouseEvent>>,
) -> Element {
    let t = use_i18n();
    let router = use_router();
    let (suffix, ratio, shape) = kind.parts();
    let round = kind == CardKind::Round;
    let mut cls = format!("jr-card jr-card--{suffix}");
    if selected {
        cls.push_str(" jr-card--selected");
    }
    if on_play.is_some() {
        cls.push_str(" jr-card--has-actions");
    }
    let footer = kind == CardKind::Landscape && (progress.is_some() || caption.is_some());
    let text_cls = if round {
        "jr-card__text jr-card__text--center"
    } else {
        "jr-card__text"
    };
    let fallback_text = round.then(|| initials(&title));
    let body = rsx! {
        div { class: "jr-card__art",
            Art { src: image, ratio, shape, shadow: true, fallback, fallback_text }
            if kind == CardKind::Poster {
                if let Some(p) = progress { CardProgress { value: p } }
            }
            if footer {
                div { class: "jr-card__landscape-footer",
                    if let Some(c) = caption.clone() { span { class: "jr-card__caption", "{c}" } }
                    if let Some(p) = progress { CardProgress { value: p, inline: true } }
                }
            }
            if let Some(b) = badge { span { class: "jr-card__badge", "{b}" } }
        }
        div { class: "{text_cls}",
            span { class: "jr-card__title", "{title}" }
            if description.is_some() {
                span { class: "jr-card__subtitle",
                    if let Some(s) = subtitle.clone() { span { class: "jr-card__line", "{s}" } }
                    if let Some(d) = description.clone() { span { class: "jr-card__description", "{d}" } }
                }
            } else if let Some(s) = subtitle.clone() {
                span { class: "jr-card__subtitle", "{s}" }
            }
        }
    };
    rsx! {
        div { class: "{cls}", style: "--jr-card-ratio: {ratio};",
            if let Some(to) = to {
                a {
                    class: "jr-card__hit",
                    draggable: "false",
                    onclick: move |_| router.push(&to),
                    oncontextmenu: move |e| {
                        if let Some(h) = &on_context {
                            e.prevent_default();
                            h.call(e);
                        }
                    },
                    {body}
                }
            } else {
                div { class: "jr-card__hit", {body} }
            }
            if let Some(play) = on_play {
                div { class: "jr-card__actions",
                    IconButton {
                        class: "jr-card__play",
                        variant: "glass",
                        icon: "play",
                        fill: true,
                        label: t.tf("shell:design.playItem", &[("title", title.clone())]),
                        onclick: move |_| play.call(()),
                    }
                }
            }
        }
    }
}

/// `Grid` (`design/media/Grid.tsx`): the caller renders the `li.jr-grid__item` cells.
#[component]
pub fn Grid(
    #[props(default = 150)] min: u32,
    #[props(default = "poster")] kind: &'static str,
    #[props(default)] label: String,
    children: Element,
) -> Element {
    rsx! {
        ul {
            class: "jr-grid jr-grid--{kind}",
            style: "--jr-grid-min: {min}px;",
            "aria-label": (!label.is_empty()).then_some(label),
            {children}
        }
    }
}

/// `MenuButton` (`design/overlays/Menu.tsx`): an icon button that opens a menu below itself.
#[component]
pub fn MenuButton(
    label: String,
    icon: &'static str,
    items: Vec<MenuItem>,
    #[props(default = "plain")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
    /// `bottom-end` (default) aligns the menu's right edge with the button.
    #[props(default = false)]
    align_start: bool,
) -> Element {
    let overlay = use_overlay();
    let mut el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let px = match size {
        "small" => 16,
        "large" => 28,
        _ => 20,
    };
    // Approximate width of a menu, to right-align it under the button.
    const MENU_WIDTH: f64 = 220.0;
    rsx! {
        button {
            r#type: "button",
            title: "{label}",
            "aria-label": "{label}",
            "aria-haspopup": "menu",
            class: "jr-icon-button jr-icon-button--{variant} jr-icon-button--{size}",
            onmounted: move |e| el.set(Some(e.data())),
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |_| {
                let items = items.clone();
                let target = el.peek().clone();
                spawn(async move {
                    let Some(target) = target else { return };
                    if let Ok(r) = target.get_client_rect().await {
                        let x = if align_start {
                            r.origin.x
                        } else {
                            (r.origin.x + r.size.width - MENU_WIDTH).max(8.0)
                        };
                        overlay.menu(x, r.origin.y + r.size.height + 6.0, items);
                    }
                });
            },
            span { class: "jr-icon-button__icon", "aria-hidden": "true",
                Icon { name: icon, size: px }
            }
        }
    }
}

/// `HeroBanner` (`design/media/HeroBanner.tsx`): full-bleed backdrop, scrim, logo or title, meta,
/// actions. Always dark.
#[component]
pub fn HeroBanner(
    backdrop: Option<String>,
    logo: Option<String>,
    title: String,
    eyebrow: Option<String>,
    meta: Option<Element>,
    description: Option<String>,
    actions: Option<Element>,
    progress: Option<f64>,
    progress_caption: Option<String>,
    #[props(default = "tall")] size: &'static str,
) -> Element {
    let mut logo_failed = use_signal(|| false);
    let mut last_logo = use_signal(|| logo.clone());
    if *last_logo.peek() != logo {
        last_logo.set(logo.clone());
        logo_failed.set(false);
    }
    let show_logo = logo.is_some() && !logo_failed();
    let footer = actions.is_some() || progress.is_some();
    rsx! {
        section {
            class: "jr-hero jr-hero--{size}",
            "data-theme": "dark",
            "aria-label": "{title}",
            div { class: "jr-hero__backdrop",
                Art { src: backdrop, shape: "none", fill: true, object_position: "center top" }
            }
            div { class: "jr-hero__scrim", "aria-hidden": "true" }
            div { class: "jr-hero__content",
                if let Some(e) = eyebrow { p { class: "jr-hero__eyebrow", "{e}" } }
                if show_logo {
                    h1 { class: "jr-hero__logo-heading",
                        img {
                            class: "jr-hero__logo",
                            src: logo.clone().unwrap_or_default(),
                            alt: "{title}",
                            draggable: "false",
                            onerror: move |_| logo_failed.set(true),
                        }
                    }
                } else {
                    h1 { class: "jr-hero__title", "{title}" }
                }
                if let Some(m) = meta { p { class: "jr-hero__meta", {m} } }
                if let Some(d) = description { p { class: "jr-hero__description", "{d}" } }
                if footer {
                    div { class: "jr-hero__footer",
                        if let Some(a) = actions { div { class: "jr-hero__actions", {a} } }
                        if let Some(p) = progress {
                            div { class: "jr-hero__progress",
                                span { class: "jr-hero__progress-track", "aria-hidden": "true",
                                    span {
                                        class: "jr-hero__progress-fill",
                                        style: "width: {(p.clamp(0.0, 1.0) * 100.0).round() as i64}%;",
                                    }
                                }
                                if let Some(c) = progress_caption {
                                    span { class: "jr-hero__progress-caption", "{c}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_of_names() {
        assert_eq!(initials("Daft Punk"), "DP");
        assert_eq!(initials("  cher "), "C");
        assert_eq!(initials("Jean Claude Van Damme"), "JD");
        assert_eq!(initials(""), "");
    }
}
