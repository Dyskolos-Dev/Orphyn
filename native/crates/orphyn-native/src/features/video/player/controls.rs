//! `PlayerControls.tsx` and `SkipPill.tsx`: the overlay (top bar, centre transport, scrubber,
//! bottom row) and the skip pill, same DOM and classes as the React app.
//!
//! The track and speed popovers are not portals: they render in the player itself, fixed above
//! their button (`placement="top-end"`), each button wrapped in a measuring box.
use std::rc::Rc;

use dioxus::prelude::*;

use super::logic::{format_clock, rate_label, SkipKind, PLAYBACK_RATES};
use super::playback::PlaybackTracks;
use crate::design::{Icon, IconButton, Slider};
use crate::i18n::use_i18n;
use crate::motion::{springs, use_spring};
use orphyn_player::video::{PlaybackSnapshot, PlaybackStatus};

/// Which popover is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pop {
    Speed,
    Subtitles,
    Audio,
}

/// An open popover: what, and its anchor's right / top edges from the window's right / bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OpenPop {
    which: Pop,
    right: f64,
    bottom: f64,
}

#[derive(Clone, PartialEq, Props)]
pub struct ControlsProps {
    pub snapshot: PlaybackSnapshot,
    /// Fade of the overlay (the CSS opacity transition, driven in Rust).
    pub opacity: f64,
    pub title: String,
    pub subtitle: Option<String>,
    pub is_fullscreen: bool,
    pub tracks: Option<PlaybackTracks>,
    pub audio_index: Option<i64>,
    /// Selected subtitle stream; `None` = off.
    pub subtitle_index: Option<i64>,
    pub trailer: bool,
    /// "Skip intros automatically"; `None` hides the toggle (nothing to skip here).
    pub skip_intro_auto: Option<bool>,
    pub on_back: EventHandler<()>,
    pub on_toggle_play: EventHandler<()>,
    pub on_seek_by: EventHandler<f64>,
    pub on_seek: EventHandler<f64>,
    pub on_volume: EventHandler<f64>,
    pub on_toggle_mute: EventHandler<()>,
    pub on_rate: EventHandler<f64>,
    pub on_toggle_fullscreen: EventHandler<()>,
    pub on_select_audio: EventHandler<i64>,
    pub on_select_subtitle: EventHandler<Option<i64>>,
    pub on_toggle_skip_intro_auto: EventHandler<()>,
    /// A popover opened / closed: the controls do not auto-hide under it.
    pub on_hold_change: EventHandler<bool>,
}

/// A box around a control, measured when its popover opens.
#[component]
fn Anchor(which: Pop, on_open: EventHandler<(Pop, Rc<MountedData>)>, children: Element) -> Element {
    let mut el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    rsx! {
        div {
            style: "display: flex;",
            onmounted: move |e| el.set(Some(e.data())),
            onclick: move |_| {
                if let Some(m) = el.peek().clone() {
                    on_open.call((which, m));
                }
            },
            {children}
        }
    }
}

#[component]
fn TrackOption(label: String, selected: bool, on_select: EventHandler<()>) -> Element {
    rsx! {
        button {
            r#type: "button",
            role: "radio",
            "aria-checked": "{selected}",
            class: "jr-video-tracklist__option",
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |_| on_select.call(()),
            span { class: "jr-video-tracklist__check", "aria-hidden": "true",
                if selected { "✓" }
            }
            span { class: "jr-video-tracklist__label", "{label}" }
        }
    }
}

#[allow(non_snake_case)]
pub fn PlayerControls(p: ControlsProps) -> Element {
    let t = use_i18n();
    let ControlsProps {
        on_back,
        on_toggle_play,
        on_seek_by,
        on_seek,
        on_volume,
        on_toggle_mute,
        on_toggle_fullscreen,
        on_toggle_skip_intro_auto,
        ..
    } = p;
    let mut scrub: Signal<Option<f64>> = use_signal(|| None);
    let mut open: Signal<Option<OpenPop>> = use_signal(|| None);
    let mut root: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let snap = &p.snapshot;
    let playing = matches!(
        snap.status,
        PlaybackStatus::Playing | PlaybackStatus::Loading
    );
    let duration = snap.duration_seconds;
    let position = scrub().unwrap_or(snap.position_seconds);
    let volume_icon = if snap.muted || snap.volume == 0.0 {
        "volume-x"
    } else if snap.volume < 0.5 {
        "volume-1"
    } else {
        "volume-2"
    };
    let on_hold = p.on_hold_change;
    let mut set_open = move |v: Option<OpenPop>| {
        open.set(v);
        on_hold.call(v.is_some());
    };
    let toggle = move |(which, anchor): (Pop, Rc<MountedData>)| {
        if open.peek().as_ref().map(|o| o.which) == Some(which) {
            set_open(None);
            return;
        }
        let r = root.peek().clone();
        spawn(async move {
            let (Ok(a), Some(Ok(w))) = (
                anchor.get_client_rect().await,
                match r {
                    Some(r) => Some(r.get_client_rect().await),
                    None => None,
                },
            ) else {
                return;
            };
            let right = (w.size.width - (a.origin.x + a.size.width)).max(8.0);
            let bottom = w.size.height - a.origin.y + 8.0;
            set_open(Some(OpenPop {
                which,
                right,
                bottom,
            }));
        });
    };
    let tracks = p.tracks.clone().unwrap_or_default();
    let sub_on = p.subtitle_index.is_some_and(|i| i >= 0);
    let play_icon = if playing { "pause" } else { "play" };
    let fs_icon = if p.is_fullscreen {
        "minimize"
    } else {
        "maximize"
    };
    let fs_label = if p.is_fullscreen {
        t.t("video:player.exitFullscreen")
    } else {
        t.t("video:player.fullscreen")
    };
    let play_label = if playing {
        t.t("video:player.pause")
    } else {
        t.t("video:player.play")
    };
    let mute_label = if snap.muted {
        t.t("video:player.unmute")
    } else {
        t.t("video:player.mute")
    };
    let rate = snap.rate;
    let audio_index = p.audio_index;
    let subtitle_index = p.subtitle_index;
    let (on_select_audio, on_select_subtitle, on_rate) =
        (p.on_select_audio, p.on_select_subtitle, p.on_rate);
    let pos_now = snap.position_seconds;

    let popover = open().map(|o| {
        let (label, width, body) = match o.which {
            Pop::Speed => (
                t.t("video:player.speed"),
                None,
                rsx! {
                    div { role: "menu", class: "jr-menu",
                        for r in PLAYBACK_RATES {
                            div {
                                key: "{r}",
                                "data-menu-item": "",
                                role: "menuitemcheckbox",
                                "aria-checked": "{rate == r}",
                                tabindex: "-1",
                                class: "jr-menu__item",
                                onmousedown: move |e: MouseEvent| e.stop_propagation(),
                                onclick: move |_| { set_open(None); on_rate.call(r) },
                                span { class: "jr-menu__check", "aria-hidden": "true",
                                    if rate == r { Icon { name: "check", size: 14, stroke_width: 2.6 } }
                                }
                                span { class: "jr-menu__label", "{rate_label(r)}" }
                            }
                        }
                    }
                },
            ),
            Pop::Subtitles => {
                let label = t.t("video:player.subtitles");
                (
                    label.clone(),
                    Some(260),
                    rsx! {
                        div { class: "jr-video-tracklist", role: "radiogroup", "aria-label": "{label}",
                            TrackOption {
                                label: t.t("video:player.subtitlesOff"),
                                selected: !sub_on,
                                on_select: move |_| { on_select_subtitle.call(None); set_open(None) },
                            }
                            for s in tracks.subtitles.clone() {
                                TrackOption {
                                    key: "{s.index}",
                                    label: s.title.clone(),
                                    selected: subtitle_index == Some(s.index),
                                    on_select: move |_| { on_select_subtitle.call(Some(s.index)); set_open(None) },
                                }
                            }
                        }
                    },
                )
            }
            Pop::Audio => {
                let label = t.t("video:player.audio");
                (
                    label.clone(),
                    Some(260),
                    rsx! {
                        div { class: "jr-video-tracklist", role: "radiogroup", "aria-label": "{label}",
                            for a in tracks.audio.clone() {
                                TrackOption {
                                    key: "{a.index}",
                                    label: a.title.clone(),
                                    selected: audio_index == Some(a.index),
                                    on_select: move |_| { on_select_audio.call(a.index); set_open(None) },
                                }
                            }
                        }
                    },
                )
            }
        };
        let width = width.map(|w| format!(" width: {w}px;")).unwrap_or_default();
        let role = if o.which == Pop::Speed { "presentation" } else { "dialog" };
        rsx! {
            // Click-catcher: a click anywhere else closes the popover.
            div {
                style: "position: fixed; inset: 0; z-index: 999; pointer-events: auto;",
                onmousedown: move |e| { e.stop_propagation(); set_open(None) },
            }
            div {
                class: "jr-popover jr-material-thick",
                role,
                "aria-label": "{label}",
                tabindex: "-1",
                style: "right: {o.right}px; bottom: {o.bottom}px; z-index: 1000; pointer-events: auto;{width}",
                {body}
            }
        }
    });

    rsx! {
        div {
            class: "jr-video-controls",
            "data-theme": "dark",
            style: "opacity: {p.opacity};",
            onmounted: move |e| root.set(Some(e.data())),
            div { class: "jr-video-controls__top",
                IconButton {
                    variant: "glass",
                    icon: "chevron-left",
                    label: t.t("video:player.back"),
                    onclick: move |_| on_back.call(()),
                }
                div { class: "jr-video-controls__title",
                    p { class: "jr-text-headline jr-color-primary jr-truncate", "{p.title}" }
                    if let Some(s) = &p.subtitle {
                        p { class: "jr-text-footnote jr-color-secondary jr-truncate", "{s}" }
                    }
                }
                if p.trailer {
                    span { class: "jr-video-trailer-badge", {t.t("video:player.trailerBadge")} }
                }
            }
            div { class: "jr-video-controls__center",
                IconButton {
                    variant: "glass",
                    size: "large",
                    icon: "rotate-ccw",
                    label: t.t("video:player.rewind"),
                    onclick: move |_| on_seek_by.call(-10.0),
                }
                IconButton {
                    variant: "glass",
                    size: "large",
                    class: "jr-video-controls__play",
                    icon: play_icon,
                    fill: true,
                    label: play_label,
                    onclick: move |_| on_toggle_play.call(()),
                }
                IconButton {
                    variant: "glass",
                    size: "large",
                    icon: "rotate-cw",
                    label: t.t("video:player.forward"),
                    onclick: move |_| on_seek_by.call(10.0),
                }
            }
            div { class: "jr-video-controls__bottom",
                div { class: "jr-video-controls__scrubber",
                    p { class: "jr-text-footnote jr-color-primary jr-tabular", "{format_clock(position)}" }
                    Slider {
                        label: t.t("video:player.seek"),
                        on_media: true,
                        value: position,
                        max: if duration > 0.0 { duration } else { 1.0 },
                        buffered: snap.buffered_seconds,
                        disabled: duration <= 0.0,
                        on_change_start: move |_| scrub.set(Some(pos_now)),
                        on_change: move |v| scrub.set(Some(v)),
                        on_change_end: move |v| { scrub.set(None); on_seek.call(v) },
                    }
                    p { class: "jr-text-footnote jr-color-primary jr-tabular",
                        "-{format_clock((duration - position).max(0.0))}"
                    }
                }
                div { class: "jr-video-controls__row",
                    div { class: "jr-video-controls__volume",
                        IconButton {
                            variant: "glass",
                            icon: volume_icon,
                            label: mute_label,
                            onclick: move |_| on_toggle_mute.call(()),
                        }
                        Slider {
                            label: t.t("video:player.volume"),
                            variant: "regular",
                            on_media: true,
                            value: if snap.muted { 0.0 } else { snap.volume },
                            max: 1.0,
                            on_change: move |v| on_volume.call(v),
                            on_change_end: move |v| on_volume.call(v),
                        }
                    }
                    div { class: "jr-video-controls__right",
                        crate::features::cast::VideoCastButton { on_hold_change: Some(on_hold) }
                        Anchor { which: Pop::Speed, on_open: toggle,
                            button {
                                r#type: "button",
                                title: t.t("video:player.speed"),
                                "aria-label": t.t("video:player.speed"),
                                "aria-haspopup": "menu",
                                "aria-expanded": "{open().map(|o| o.which) == Some(Pop::Speed)}",
                                class: "jr-icon-button jr-icon-button--glass jr-icon-button--medium",
                                onmousedown: move |e: MouseEvent| e.stop_propagation(),
                                span { class: "jr-icon-button__icon", "aria-hidden": "true",
                                    p { class: "jr-text-callout jr-color-primary", "{rate_label(rate)}" }
                                }
                            }
                        }
                        if let Some(on) = p.skip_intro_auto {
                            IconButton {
                                variant: "glass",
                                icon: "fast-forward",
                                label: t.t("video:skip.auto"),
                                pressed: on,
                                onclick: move |_| on_toggle_skip_intro_auto.call(()),
                            }
                        }
                        if !tracks.subtitles.is_empty() {
                            Anchor { which: Pop::Subtitles, on_open: toggle,
                                IconButton {
                                    variant: "glass",
                                    icon: "subtitles",
                                    label: t.t("video:player.subtitles"),
                                    pressed: sub_on,
                                }
                            }
                        }
                        if tracks.audio.len() > 1 {
                            Anchor { which: Pop::Audio, on_open: toggle,
                                IconButton {
                                    variant: "glass",
                                    icon: "languages",
                                    label: t.t("video:player.audio"),
                                }
                            }
                        }
                        IconButton {
                            variant: "glass",
                            icon: fs_icon,
                            label: fs_label,
                            onclick: move |_| on_toggle_fullscreen.call(()),
                        }
                    }
                }
            }
        }
        {popover}
    }
}

/// `SkipPillButton`: springs in like `springs.bouncy` (a new kind is a new pill).
#[component]
pub fn SkipPillButton(kind: SkipKind, on_skip: EventHandler<()>) -> Element {
    let t = use_i18n();
    let mut shown = use_signal(|| false);
    use_effect(move || shown.set(true));
    let k = use_spring(if shown() { 1.0 } else { 0.0 }, springs::BOUNCY);
    let k = k();
    let opacity = k.clamp(0.0, 1.0);
    let y = 16.0 * (1.0 - k);
    let scale = 0.92 + 0.08 * k;
    rsx! {
        button {
            r#type: "button",
            class: "jr-video-skip",
            "data-theme": "dark",
            style: "opacity: {opacity}; transform: translateY({y}px) scale({scale});",
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |_| on_skip.call(()),
            span { {t.t(kind.label_key())} }
            Icon { name: "skip-forward", size: 24, fill: true }
            kbd { "aria-hidden": "true", "S" }
        }
    }
}
