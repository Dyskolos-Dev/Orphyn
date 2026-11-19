//! `VideoPip.tsx`: the video on a rounded card, back / close on hover, a play / pause button,
//! the time and a seek bar. It draws the same mpv instance as the player page, so the position
//! shown is the player's and "back to Orphyn" just closes this window.
use dioxus::prelude::*;
use dioxus_native::winit::event::{ElementState, WindowEvent};
use dioxus_native::winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use dioxus_native::CustomWidgetAttr;
use orphyn_player::video::{PlaybackSnapshot, PlaybackStatus, VideoWidget};

use super::chrome::{PipChrome, ResizeHandles};
use crate::design::{format_duration, Icon, Slider};
use crate::i18n::use_i18n;
use crate::state::use_services;

#[component]
pub fn VideoPip(chrome: PipChrome, title: String) -> Element {
    let t = use_i18n();
    let video = use_services().video;
    let mut snap = use_signal(|| video.snapshot());
    use_hook({
        let video = video.clone();
        move || {
            let mut rx = video.subscribe();
            spawn(async move {
                while rx.changed().await.is_ok() {
                    let v: PlaybackSnapshot = rx.borrow_and_update().clone();
                    snap.set(v);
                }
            });
        }
    });
    let widget = use_hook({
        let video = video.clone();
        move || {
            video
                .mpv()
                .ok()
                .map(|m| CustomWidgetAttr::new(VideoWidget::new(m)))
        }
    });

    let s = snap();
    let playing = matches!(s.status, PlaybackStatus::Playing | PlaybackStatus::Loading);
    let duration = s.duration_seconds;
    let mut scrub = use_signal(|| None::<f64>);
    let position = scrub().unwrap_or(s.position_seconds).min(duration.max(0.0));

    // The player page owns the item: when it stops (the page closed), so does the PiP.
    let closed = s.item_id.is_none();
    use_effect({
        let chrome = chrome.clone();
        move || {
            if closed {
                chrome.close();
            }
        }
    });

    // Space play / pause, Esc and Ctrl+W close.
    {
        let (video, chrome) = (video.clone(), chrome.clone());
        let mut modifiers = use_signal(ModifiersState::default);
        dioxus_native::use_window_event(move |event, _| match event {
            WindowEvent::ModifiersChanged(m) => modifiers.set(m.state()),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match &event.logical_key {
                    WKey::Named(NamedKey::Escape) => chrome.close(),
                    WKey::Character(c)
                        if c.eq_ignore_ascii_case("w") && modifiers.peek().control_key() =>
                    {
                        chrome.close()
                    }
                    WKey::Character(c) if c.as_str() == " " && !event.repeat => video.toggle(),
                    _ => {}
                }
            }
            _ => {}
        });
    }

    let play_label = if playing {
        t.t("pip:pause")
    } else {
        t.t("pip:play")
    };
    let play_icon = if playing { "pause" } else { "play" };
    let back_label = t.t("pip:backToOrphyn");
    let close_label = t.t("pip:close");
    let pip_scale = format!("--pip-scale: {};", chrome.unit());
    let (c_press, c_move, c_up, c_back, c_close) = (
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
    );
    let (v_toggle, v_click, v_seek) = (video.clone(), video.clone(), video.clone());
    let stop = |e: MouseEvent| e.stop_propagation();
    rsx! {
        div {
            class: "pip pip--video",
            "data-theme": "dark",
            style: "{pip_scale}",
            onmousedown: move |e: MouseEvent| {
                if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                    let p = e.client_coordinates();
                    c_press.clone().press(p.x, p.y);
                }
            },
            onmousemove: move |e: MouseEvent| {
                let p = e.client_coordinates();
                c_move.clone().moved(p.x, p.y);
            },
            onmouseup: move |_| c_up.clone().release(),
            if let Some(w) = widget.clone() {
                object {
                    class: "pip__video",
                    data: w,
                    onclick: move |_| v_click.toggle(),
                }
            }
            div { class: "pip__shade pip__shade--video" }
            div { class: "pip__top",
                button {
                    r#type: "button",
                    class: "pip__btn",
                    "aria-label": t.t("pip:backToOrphyn"),
                    title: "{back_label}",
                    onmousedown: stop,
                    onclick: move |_| c_back.back_to_main(),
                    Icon { name: "maximize-2", size: 16 }
                }
                button {
                    r#type: "button",
                    class: "pip__btn",
                    "aria-label": t.t("pip:close"),
                    title: "{close_label}",
                    onmousedown: stop,
                    onclick: move |_| c_close.close(),
                    Icon { name: "x", size: 16 }
                }
            }
            div { class: "pip__bottom pip__bottom--video",
                if !title.is_empty() {
                    div { class: "pip__title", "{title}" }
                }
                div { class: "pip__scrubber", onmousedown: stop,
                    button {
                        r#type: "button",
                        class: "pip__btn pip__btn--main",
                        "aria-label": "{play_label}",
                        onclick: move |_| v_toggle.toggle(),
                        Icon { name: play_icon, size: 20, fill: true }
                    }
                    span { class: "pip__time", {format_duration(position)} }
                    div { class: "pip__range",
                        Slider {
                            label: t.t("pip:seek"),
                            value: position,
                            max: if duration > 0.0 { duration } else { 1.0 },
                            disabled: duration <= 0.0,
                            on_media: true,
                            on_change_start: move |_| scrub.set(Some(position)),
                            on_change: move |v| scrub.set(Some(v)),
                            on_change_end: move |v| {
                                v_seek.seek(v);
                                scrub.set(None);
                            },
                        }
                    }
                    span { class: "pip__time", {format!("-{}", format_duration((duration - position).max(0.0)))} }
                }
            }
            ResizeHandles { chrome: chrome.clone() }
        }
    }
}
