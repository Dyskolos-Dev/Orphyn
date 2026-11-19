//! `MusicPip.tsx`: artwork full-bleed over a blurred copy, controls on hover.
use std::rc::Rc;

use dioxus::prelude::*;
use dioxus_native::winit::event::{ElementState, WindowEvent};
use dioxus_native::winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::{keys, BaseItemDto};

use super::chrome::{PipChrome, ResizeHandles};
use super::geometry::artwork_step;
use crate::design::Icon;
use crate::features::music::{toggle_favorite, track_artist, use_base_url, use_music};
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services};

#[component]
pub fn MusicPip(chrome: PipChrome) -> Element {
    let t = use_i18n();
    let music = use_music();
    let svc = use_services();
    let app = use_app();
    let player = svc.music.clone();
    let base = use_base_url();
    crate::query::use_cache_updates();

    let s = (music.state)();
    let track = s.current().map(|q| q.track.clone());
    let has_track = track.is_some();
    let playing = s.is_playing();
    let progress = (music.progress)();
    let duration = if progress.duration > 0.0 {
        progress.duration
    } else {
        track.as_ref().map(|t| t.duration_seconds()).unwrap_or(0.0)
    };
    let fraction = if duration > 0.0 {
        (progress.position / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Artwork sized for the window: width x DPI rounded up to a step, raised when the window
    // grows and kept while it shrinks (a bitmap is never upscaled, shrinking does not reload).
    let track_id = track.as_ref().map(|t| t.id.clone());
    let mut best = use_signal(|| (None::<String>, 0u32));
    let step = artwork_step((chrome.width)() * chrome.win.scale_factor());
    let art_step = {
        let current = best.peek().clone();
        let next = if current.0 == track_id {
            current.1.max(step)
        } else {
            step
        };
        if current != (track_id.clone(), next) {
            best.set((track_id.clone(), next));
        }
        next
    };
    let art = track.as_ref().and_then(|tr| {
        build_image_url(
            &base,
            tr,
            ImageKind::Primary,
            &ImageOptions {
                max_width: Some(art_step),
                max_height: None,
                quality: Some(90),
                tag: None,
            },
        )
    });

    // Favourite of the playing track: the cached item first, so a toggle anywhere shows here.
    let session = (app.session)();
    let favorite = track.as_ref().is_some_and(|tr| {
        session
            .as_ref()
            .and_then(|s| {
                svc.cache
                    .peek::<BaseItemDto>(&keys::item(&s.account_id, &tr.id))
                    .data
            })
            .map(|c| c.is_favorite())
            .unwrap_or(tr.is_favorite())
    });

    // Space play / pause, arrows previous / next, Esc and Ctrl+W close.
    let mut modifiers = use_signal(ModifiersState::default);
    {
        let (player, chrome) = (player.clone(), chrome.clone());
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
                    WKey::Character(c) if c.as_str() == " " && !event.repeat => player.toggle(),
                    WKey::Named(NamedKey::ArrowRight) => player.next(),
                    WKey::Named(NamedKey::ArrowLeft) => player.previous(),
                    _ => {}
                }
            }
            _ => {}
        });
    }

    let unit = chrome.unit();
    let fav_label = if favorite {
        t.t("pip:unfavorite")
    } else {
        t.t("pip:favorite")
    };
    let play_label = if playing {
        t.t("pip:pause")
    } else {
        t.t("pip:play")
    };
    let play_icon = if playing { "pause" } else { "play" };
    let back_label = t.t("pip:backToOrphyn");
    let close_label = t.t("pip:close");
    let pip_scale = format!("--pip-scale: {unit};");
    let mut bar: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let (p_prev, p_toggle, p_next, p_seek) = (
        player.clone(),
        player.clone(),
        player.clone(),
        player.clone(),
    );
    let (c_press, c_move, c_up, c_dbl, c_back, c_close) = (
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
        chrome.clone(),
    );
    let stop = |e: MouseEvent| e.stop_propagation();
    rsx! {
        div {
            class: "pip pip--music",
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
            ondoubleclick: move |_| c_dbl.toggle_compact(),
            div { class: "pip__stage", style: "--u: {unit}px;",
                if let Some(src) = art.clone() {
                    div { class: "pip__bg", style: "background-image: url(\"{src}\");" }
                    img { class: "pip__art", src: "{src}", alt: "", draggable: "false" }
                } else {
                    div { class: "pip__art pip__art--empty",
                        Icon { name: "music", size: 48 }
                    }
                }
                div { class: "pip__shade" }

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

                div { class: "pip__bottom",
                    div { class: "pip__meta",
                        div { class: "pip__title",
                            if let Some(tr) = track.as_ref() {
                                "{tr.name()}"
                            } else {
                                {t.t("pip:nothingPlaying")}
                            }
                        }
                        if let Some(tr) = track.as_ref() {
                            div { class: "pip__artist", {track_artist(tr)} }
                        }
                    }
                    div { class: "pip__controls",
                        button {
                            r#type: "button",
                            class: "pip__btn",
                            "aria-label": "{fav_label}",
                            "aria-pressed": "{favorite}",
                            "data-on": favorite.then_some("true"),
                            disabled: !has_track,
                            onmousedown: stop,
                            onclick: {
                                let (svc, session, track) = (svc.clone(), session.clone(), track.clone());
                                move |_| {
                                    if let Some(tr) = track.as_ref() {
                                        let mut item = (**tr).clone();
                                        item.user_data.get_or_insert_with(Default::default).is_favorite = favorite;
                                        toggle_favorite(&svc, session.clone(), &item, !favorite);
                                    }
                                }
                            },
                            Icon { name: "heart", size: 16, fill: favorite }
                        }
                        button {
                            r#type: "button",
                            class: "pip__btn",
                            "aria-label": t.t("pip:previous"),
                            disabled: !has_track,
                            onmousedown: stop,
                            onclick: move |_| p_prev.previous(),
                            Icon { name: "skip-back", size: 16, fill: true }
                        }
                        button {
                            r#type: "button",
                            class: "pip__btn pip__btn--main",
                            "aria-label": "{play_label}",
                            disabled: !has_track,
                            onmousedown: stop,
                            onclick: move |_| p_toggle.toggle(),
                            Icon { name: play_icon, size: 20, fill: true }
                        }
                        button {
                            r#type: "button",
                            class: "pip__btn",
                            "aria-label": t.t("pip:next"),
                            disabled: !has_track,
                            onmousedown: stop,
                            onclick: move |_| p_next.next(),
                            Icon { name: "skip-forward", size: 16, fill: true }
                        }
                    }
                    div {
                        class: "pip__progress",
                        role: "progressbar",
                        "aria-valuemin": "0",
                        "aria-valuemax": "{duration.round()}",
                        "aria-valuenow": "{progress.position.round()}",
                        onmounted: move |e| bar.set(Some(e.data())),
                        onmousedown: move |e: MouseEvent| {
                            e.stop_propagation();
                            if duration <= 0.0 {
                                return;
                            }
                            let x = e.client_coordinates().x;
                            let (player, mounted) = (p_seek.clone(), bar.peek().clone());
                            spawn(async move {
                                if let Some(m) = mounted {
                                    if let Ok(r) = m.get_client_rect().await {
                                        let f = ((x - r.origin.x) / r.size.width).clamp(0.0, 1.0);
                                        player.seek(f * duration);
                                    }
                                }
                            });
                        },
                        div { class: "pip__progress-fill", style: "width: {fraction * 100.0}%;" }
                    }
                }
            }
            ResizeHandles { chrome: chrome.clone() }
        }
    }
}
