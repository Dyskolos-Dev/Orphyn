//! Design system: RSX twins of `src/renderer/src/design` components. Each renders the very DOM
//! (tags, classes, ARIA) of its React counterpart, so the shared CSS styles it unchanged.
//! Text styles have no component: write the classes (`jr-text-title-1 jr-color-primary`), as
//! `<Text>` renders them.
pub mod icons;
pub mod media;
pub mod overlay;

use dioxus::prelude::*;

pub use media::{Shelf, SquareCard};
pub use overlay::{use_overlay, MenuItem, Overlay, OverlayHost};

/// `formatDuration` of `design`: `m:ss`, `h:mm:ss`, `--:--` when unknown.
pub fn format_duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--:--".into();
    }
    let s = secs.floor() as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// A lucide icon (`name` = lucide file name, e.g. `disc-3`), as lucide-react renders it.
#[component]
pub fn Icon(
    name: &'static str,
    #[props(default = 24)] size: u32,
    #[props(default = false)] fill: bool,
    #[props(default = 2.0)] stroke_width: f64,
    #[props(default)] class: String,
) -> Element {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\" fill=\"{}\" stroke=\"currentColor\" stroke-width=\"{stroke_width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide {class}\" aria-hidden=\"true\">{}</svg>",
        if fill { "currentColor" } else { "none" },
        icons::icon_markup(name)
    );
    rsx! {
        span { class: "jr-icon-host", dangerous_inner_html: svg }
    }
}

/// `Button` (`design/controls/Button.tsx`).
#[component]
pub fn Button(
    #[props(default = "filled")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
    icon: Option<&'static str>,
    #[props(default = false)] icon_fill: bool,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] full_width: bool,
    #[props(default)] class: String,
    onclick: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    let full = if full_width { " jr-button--full" } else { "" };
    let icon_size = if size == "large" { 18 } else { 16 };
    rsx! {
        button {
            r#type: "button",
            class: "jr-button jr-button--{variant} jr-button--{size}{full} {class}",
            disabled,
            // Controls never start a window drag (`.jr-no-drag` in the React app).
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |e| if let Some(h) = &onclick { h.call(e) },
            if let Some(i) = icon {
                span { class: "jr-button__icon", "aria-hidden": "true",
                    Icon { name: i, size: icon_size, fill: icon_fill }
                }
            }
            span { class: "jr-button__label", {children} }
        }
    }
}

/// `IconButton` (`design/controls/IconButton.tsx`).
#[component]
pub fn IconButton(
    icon: &'static str,
    label: String,
    #[props(default = "plain")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
    pressed: Option<bool>,
    #[props(default = false)] fill: bool,
    #[props(default = 2.0)] stroke_width: f64,
    #[props(default = false)] disabled: bool,
    #[props(default)] class: String,
    #[props(default)] style: String,
    onclick: Option<EventHandler<MouseEvent>>,
) -> Element {
    let pressed_cls = if pressed == Some(true) { " jr-icon-button--pressed" } else { "" };
    let px = match size {
        "small" => 16,
        "large" => 28,
        _ => 20,
    };
    rsx! {
        button {
            r#type: "button",
            title: "{label}",
            "aria-label": "{label}",
            "aria-pressed": pressed.map(|p| p.to_string()),
            class: "jr-icon-button jr-icon-button--{variant} jr-icon-button--{size}{pressed_cls} {class}",
            style: (!style.is_empty()).then_some(style),
            disabled,
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |e| if let Some(h) = &onclick { h.call(e) },
            span { class: "jr-icon-button__icon", "aria-hidden": "true",
                Icon { name: icon, size: px, fill, stroke_width }
            }
        }
    }
}

/// `Slider` (`design/controls/Slider.tsx`): click or drag. `on_change` fires while dragging,
/// `on_change_end` on release (seek bars commit there, like the React app).
#[component]
pub fn Slider(
    value: f64,
    max: f64,
    #[props(default = 0.0)] min: f64,
    #[props(default = "default")] variant: &'static str,
    #[props(default = false)] on_media: bool,
    #[props(default = false)] disabled: bool,
    buffered: Option<f64>,
    #[props(default)] label: String,
    #[props(default)] class: String,
    on_change_start: Option<EventHandler<()>>,
    on_change: Option<EventHandler<f64>>,
    on_change_end: Option<EventHandler<f64>>,
) -> Element {
    let mut rect = use_signal(|| (0.0f64, 0.0f64));
    let mut element: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);
    let mut dragging = use_signal(|| false);
    let span = (max - min).max(f64::EPSILON);
    // A 0..1 fraction, like `pct` of Slider.tsx (the CSS multiplies it by 100%).
    let pct = |v: f64| ((v - min) / span).clamp(0.0, 1.0);
    let value_pct = pct(value);
    let buffered_pct = buffered.map(pct).unwrap_or(0.0);
    let to_value = move |client_x: f64| {
        let (left, width) = rect();
        if width <= 0.0 {
            return min;
        }
        min + ((client_x - left) / width).clamp(0.0, 1.0) * span
    };
    let dragging_cls = if dragging() { " jr-slider--dragging" } else { "" };
    let disabled_cls = if disabled { " jr-slider--disabled" } else { "" };
    let media_cls = if on_media { " jr-slider--on-media" } else { "" };
    rsx! {
        div {
            role: "slider",
            tabindex: if disabled { "-1" } else { "0" },
            "aria-label": "{label}",
            "aria-valuemin": "{min}",
            "aria-valuemax": "{max}",
            "aria-valuenow": "{value}",
            class: "jr-slider jr-slider--{variant}{dragging_cls}{disabled_cls}{media_cls} {class}",
            style: "--jr-slider-value: {value_pct}; --jr-slider-buffered: {buffered_pct};",
            onmounted: move |e| element.set(Some(e.data())),
            onmousedown: move |e| {
                // Never a window drag (the Now Playing header is a drag region).
                e.stop_propagation();
                if disabled {
                    return;
                }
                dragging.set(true);
                if let Some(h) = &on_change_start { h.call(()) }
                let x = e.client_coordinates().x;
                // Measured now, not at mount: a just-mounted box has no layout yet, and the bar
                // moves with the window.
                let el = element.peek().clone();
                spawn(async move {
                    if let Some(el) = el {
                        if let Ok(r) = el.get_client_rect().await {
                            rect.set((r.origin.x, r.size.width));
                        }
                    }
                    let v = to_value(x);
                    if let Some(h) = &on_change { h.call(v) }
                });
            },
            onmousemove: move |e| {
                if dragging() {
                    let v = to_value(e.client_coordinates().x);
                    if let Some(h) = &on_change { h.call(v) }
                }
            },
            onmouseup: move |e| {
                if dragging() {
                    dragging.set(false);
                    let v = to_value(e.client_coordinates().x);
                    if let Some(h) = &on_change_end { h.call(v) }
                }
            },
            onmouseleave: move |e| {
                if dragging() {
                    dragging.set(false);
                    let v = to_value(e.client_coordinates().x);
                    if let Some(h) = &on_change_end { h.call(v) }
                }
            },
            span { class: "jr-slider__track", "aria-hidden": "true",
                if buffered.is_some() { span { class: "jr-slider__buffered" } }
                span { class: "jr-slider__fill" }
            }
            span { class: "jr-slider__thumb", "aria-hidden": "true" }
        }
    }
}

/// One option of a [`SegmentedControl`].
#[derive(Clone, PartialEq)]
pub struct Segment {
    pub value: &'static str,
    pub label: String,
}

/// `SegmentedControl` (`design/controls/SegmentedControl.tsx`).
#[component]
pub fn SegmentedControl(
    options: Vec<Segment>,
    value: &'static str,
    on_change: EventHandler<&'static str>,
    #[props(default = "medium")] size: &'static str,
    #[props(default)] label: String,
    #[props(default)] class: String,
) -> Element {
    let index = options.iter().position(|o| o.value == value).unwrap_or(0);
    let count = options.len();
    rsx! {
        div {
            role: "radiogroup",
            "aria-label": "{label}",
            class: "jr-segmented jr-segmented--{size} {class}",
            style: "--jr-seg-count: {count}; --jr-seg-index: {index};",
            span { class: "jr-segmented__thumb", "aria-hidden": "true" }
            for (i, o) in options.into_iter().enumerate() {
                button {
                    key: "{o.value}",
                    r#type: "button",
                    role: "radio",
                    "aria-checked": (i == index).to_string(),
                    tabindex: if i == index { "0" } else { "-1" },
                    class: if i == index { "jr-segmented__item jr-segmented__item--selected" } else { "jr-segmented__item" },
                    onclick: move |_| on_change.call(o.value),
                    "{o.label}"
                }
            }
        }
    }
}

/// `Artwork` (`design/media/Artwork.tsx`): an image with a fallback icon when there is none or
/// it fails. `ratio` is the CSS aspect ratio (`"1 / 1"`).
#[component]
pub fn Artwork(
    src: Option<String>,
    #[props(default = "1 / 1")] ratio: &'static str,
    #[props(default = "rounded")] shape: &'static str,
    #[props(default = false)] shadow: bool,
    #[props(default = "music")] fallback: &'static str,
    #[props(default)] class: String,
) -> Element {
    let mut failed = use_signal(|| false);
    let mut last_src = use_signal(|| src.clone());
    if *last_src.peek() != src {
        last_src.set(src.clone());
        failed.set(false);
    }
    let status = if src.is_none() || failed() { "error" } else { "loaded" };
    let shadow_cls = if shadow { " jr-artwork--shadow" } else { "" };
    rsx! {
        div {
            class: "jr-artwork jr-artwork--{shape} jr-artwork--{status}{shadow_cls} {class}",
            style: "aspect-ratio: {ratio};",
            "data-status": status,
            if status == "error" {
                div { class: "jr-artwork__fallback", "aria-hidden": "true",
                    Icon { name: fallback, size: 24, stroke_width: 1.5 }
                }
            } else {
                img {
                    class: "jr-artwork__img",
                    src: src.clone().unwrap_or_default(),
                    draggable: "false",
                    onerror: move |_| failed.set(true),
                }
            }
        }
    }
}

/// `Spinner` (`design/feedback/Spinner.tsx`): 8 spokes, the rotation is static (no CSS
/// animation engine): it reads as the activity indicator at rest.
#[component]
pub fn Spinner(#[props(default = "medium")] size: &'static str) -> Element {
    const SPOKES: usize = 8;
    rsx! {
        span { class: "jr-spinner jr-spinner--{size}", "aria-hidden": "true",
            for i in 0..SPOKES {
                span {
                    key: "{i}",
                    class: "jr-spinner__spoke",
                    style: "transform: rotate({360.0 / SPOKES as f64 * i as f64}deg); opacity: {0.25 + 0.75 * (i as f64 / (SPOKES - 1) as f64)};",
                }
            }
        }
    }
}

/// `EmptyState` (`design/feedback/EmptyState.tsx`).
#[component]
pub fn EmptyState(
    icon: Option<&'static str>,
    title: String,
    message: Option<Element>,
    action: Option<Element>,
    #[props(default = "medium")] size: &'static str,
) -> Element {
    rsx! {
        div { class: "jr-empty jr-empty--{size}",
            if let Some(i) = icon {
                div { class: "jr-empty__icon", "aria-hidden": "true", Icon { name: i, size: 48, stroke_width: 1.5 } }
            }
            h2 { class: "jr-empty__title", "{title}" }
            if let Some(m) = message { p { class: "jr-empty__message", {m} } }
            if let Some(a) = action { div { class: "jr-empty__action", {a} } }
        }
    }
}

/// `Toolbar` (`design/navigation/Toolbar.tsx`).
#[component]
pub fn Toolbar(
    title: String,
    #[props(default = false)] show_title: bool,
    #[props(default = "transparent")] appearance: &'static str,
    leading: Option<Element>,
    trailing: Option<Element>,
) -> Element {
    let visible = if show_title && !title.is_empty() { " jr-toolbar__title--visible" } else { "" };
    rsx! {
        div { class: "jr-toolbar jr-toolbar--{appearance} jr-drag",
            div { class: "jr-toolbar__leading", {leading} }
            div { class: "jr-toolbar__title{visible}", "aria-hidden": (!show_title).then_some("true"), "{title}" }
            div { class: "jr-toolbar__trailing", {trailing} }
        }
    }
}

/// `TrackRow` (`design/lists/TrackRow.tsx`): one row of a track list (`<li>` in an `<ol>`).
#[component]
pub fn TrackRow(
    index: Option<i32>,
    artwork: Option<String>,
    title: String,
    artist: Option<Element>,
    album: Option<String>,
    duration: Option<f64>,
    #[props(default = false)] is_current: bool,
    #[props(default = false)] is_playing: bool,
    #[props(default = false)] selected: bool,
    #[props(default)] play_label: String,
    #[props(default)] more_label: String,
    accessory: Option<Element>,
    on_play: EventHandler<()>,
    on_pause: Option<EventHandler<()>>,
    on_more: Option<EventHandler<MouseEvent>>,
) -> Element {
    let playing_now = is_current && is_playing;
    let mut cls = String::from("jr-track");
    if is_current {
        cls.push_str(" jr-track--current");
    }
    if selected {
        cls.push_str(" jr-track--selected");
    }
    if artwork.is_some() {
        cls.push_str(" jr-track--with-art");
    }
    if album.is_some() {
        cls.push_str(" jr-track--with-album");
    }
    let play_icon = if playing_now { "pause" } else { "play" };
    rsx! {
        li {
            tabindex: "0",
            "aria-current": is_current.then_some("true"),
            class: "{cls}",
            ondoubleclick: move |_| on_play.call(()),
            span { class: "jr-track__lead",
                if let Some(a) = artwork.clone() {
                    Artwork { src: Some(a), class: "jr-track__art" }
                } else {
                    span { class: "jr-track__number",
                        if playing_now {
                            span { class: "jr-equalizer", "aria-hidden": "true", span {} span {} span {} span {} }
                        } else if let Some(i) = index {
                            "{i}"
                        }
                    }
                }
                button {
                    r#type: "button",
                    tabindex: "-1",
                    class: "jr-track__play",
                    "aria-label": "{play_label}",
                    onclick: move |e| {
                        e.stop_propagation();
                        match (&on_pause, playing_now) {
                            (Some(p), true) => p.call(()),
                            _ => on_play.call(()),
                        }
                    },
                    ondoubleclick: move |e| e.stop_propagation(),
                    Icon { name: play_icon, size: 14, fill: true }
                }
            }
            span { class: "jr-track__main",
                span { class: "jr-track__title", span { class: "jr-track__title-text", "{title}" } }
                if let Some(a) = artist { span { class: "jr-track__artist", {a} } }
            }
            if let Some(al) = album { span { class: "jr-track__album", "{al}" } }
            if let Some(acc) = accessory { span { class: "jr-track__accessory", {acc} } }
            span { class: "jr-track__duration", {duration.map(format_duration).unwrap_or_default()} }
            span { class: "jr-track__more-slot",
                if let Some(m) = on_more {
                    button {
                        r#type: "button",
                        tabindex: "-1",
                        class: "jr-track__more",
                        "aria-label": "{more_label}",
                        onclick: move |e| { e.stop_propagation(); m.call(e) },
                        Icon { name: "ellipsis", size: 16 }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_format_like_the_react_helper() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(61.9), "1:01");
        assert_eq!(format_duration(3725.0), "1:02:05");
        assert_eq!(format_duration(f64::NAN), "--:--");
    }

    #[test]
    fn icons_used_by_the_native_app_exist() {
        for name in [
            "play", "pause", "shuffle", "repeat", "repeat-1", "skip-back", "skip-forward", "heart",
            "ellipsis", "chevron-left", "chevron-down", "music", "disc-3", "house", "list-music",
            "mic-vocal", "clapperboard", "tv", "circle-play", "settings", "search", "rotate-cw",
            "volume-1", "volume-2", "volume-x", "message-square-quote", "construction",
        ] {
            assert!(!icons::icon_markup(name).is_empty(), "{name}");
        }
    }
}
