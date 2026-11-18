//! `Transport.tsx`: transport buttons, seek bar, volume.
use dioxus::prelude::*;
use orphyn_player::music::RepeatMode;

use super::use_music;
use crate::design::{format_duration, IconButton, Slider};
use crate::i18n::use_i18n;
use crate::state::use_services;

/// Shuffle, previous, play / pause, next, repeat.
#[component]
pub fn TransportButtons(
    #[props(default = "medium")] size: &'static str,
    #[props(default = true)] extras: bool,
) -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let s = (music.state)();
    let playing = s.is_playing();
    let has_queue = s.has_queue();
    let repeat = s.model.repeat;
    let (p1, p2, p3, p4, p5) = (
        player.clone(),
        player.clone(),
        player.clone(),
        player.clone(),
        player,
    );
    let play_icon = if playing { "pause" } else { "play" };
    let repeat_icon = if repeat == RepeatMode::One {
        "repeat-1"
    } else {
        "repeat"
    };
    rsx! {
        div { class: "jrm-transport", role: "group", "aria-label": t.t("music:player.controls"),
            if extras {
                IconButton {
                    icon: "shuffle",
                    label: t.t("music:player.shuffle"),
                    pressed: s.model.shuffle,
                    class: "jrm-toggle",
                    onclick: move |_| p1.toggle_shuffle(),
                }
            }
            IconButton {
                icon: "skip-back", fill: true, size,
                label: t.t("music:player.previous"),
                disabled: !has_queue,
                onclick: move |_| p2.previous(),
            }
            IconButton {
                icon: play_icon, fill: true, size,
                label: if playing { t.t("music:player.pause") } else { t.t("music:player.play") },
                disabled: !has_queue,
                onclick: move |_| p3.toggle(),
            }
            IconButton {
                icon: "skip-forward", fill: true, size,
                label: t.t("music:player.next"),
                disabled: !has_queue,
                onclick: move |_| p4.next(),
            }
            if extras {
                IconButton {
                    icon: repeat_icon,
                    label: t.t(&format!("music:player.repeat.{}", repeat.as_str())),
                    pressed: repeat != RepeatMode::Off,
                    class: "jrm-toggle",
                    onclick: move |_| p5.cycle_repeat(),
                }
            }
        }
    }
}

/// `useTrackDuration`: the loaded duration, or the metadata's while nothing is loaded yet.
pub fn track_duration() -> f64 {
    let music = use_music();
    let d = (music.progress)().duration;
    if d > 0.0 {
        return d;
    }
    (music.state)()
        .current()
        .map(|q| q.track.duration_seconds())
        .unwrap_or(0.0)
}

/// Scrubber with elapsed / remaining time; dragging does not fight the playback position.
#[component]
pub fn SeekBar(#[props(default = false)] on_media: bool) -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let position = (music.progress)().position;
    let duration = track_duration();
    let mut scrub = use_signal(|| None::<f64>);
    let shown = scrub().unwrap_or(position);
    let remaining = if duration > 0.0 {
        format!("-{}", format_duration((duration - shown).max(0.0)))
    } else {
        "--:--".into()
    };
    rsx! {
        div { class: "jrm-seek",
            span { class: "jrm-seek__time jr-tabular", {format_duration(shown)} }
            Slider {
                label: t.t("music:player.seek"),
                value: shown.min(duration.max(0.0)),
                max: if duration > 0.0 { duration } else { 1.0 },
                disabled: duration <= 0.0,
                on_media,
                on_change_start: move |_| scrub.set(Some(position)),
                on_change: move |v| scrub.set(Some(v)),
                on_change_end: move |v| {
                    player.seek(v);
                    scrub.set(None);
                },
            }
            span { class: "jrm-seek__time jr-tabular", "{remaining}" }
        }
    }
}

#[component]
pub fn VolumeControl(#[props(default = false)] on_media: bool) -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let s = (music.state)();
    let icon = if s.muted || s.volume == 0.0 {
        "volume-x"
    } else if s.volume < 0.5 {
        "volume-1"
    } else {
        "volume-2"
    };
    let (p1, p2) = (player.clone(), player);
    rsx! {
        div { class: "jrm-volume",
            IconButton {
                icon,
                size: "small",
                label: if s.muted { t.t("music:player.unmute") } else { t.t("music:player.mute") },
                onclick: move |_| p1.toggle_mute(),
            }
            Slider {
                label: t.t("music:player.volume"),
                value: if s.muted { 0.0 } else { s.volume as f64 },
                max: 1.0,
                variant: "regular",
                on_media,
                on_change: move |v: f64| p2.set_volume(v as f32),
            }
        }
    }
}
