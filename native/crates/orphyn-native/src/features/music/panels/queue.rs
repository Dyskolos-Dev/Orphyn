//! `QueueView.tsx`: the queue with the current track highlighted, reordering and removal. Blitz has
//! no HTML drag and drop, so the reorder is done with the mouse: press the grip, move over the
//! row it should take, release (same `data-dragging` / `data-drop` attributes as the React app).
use dioxus::prelude::*;
use orphyn_player::music::{PlayerStatus, RepeatMode};

use super::radio::RadioIndicator;
use crate::design::{format_duration, Artwork, Button, EmptyState, Icon, IconButton};
use crate::features::music::{track_art, track_artist, use_base_url, use_music};
use crate::i18n::use_i18n;
use crate::state::use_services;

#[component]
pub fn QueueView() -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let base = use_base_url();
    let state = (music.state)();
    let model = &state.model;
    let (index, count) = (model.index, model.queue.len());
    let mut drag_from: Signal<Option<usize>> = use_signal(|| None);
    let mut drop_at: Signal<Option<usize>> = use_signal(|| None);

    if count == 0 {
        return rsx! {
            div { class: "jrm-queue jrm-queue--empty",
                EmptyState { size: "inline", icon: "list-music", title: t.t("music:queue.empty") }
            }
        };
    }
    let mut finish = move || {
        drag_from.set(None);
        drop_at.set(None);
    };
    let (p_shuffle, p_repeat, p_clear, p_end) =
        (player.clone(), player.clone(), player.clone(), player.clone());
    let repeat = model.repeat;
    let repeat_icon = if repeat == RepeatMode::One { "repeat-1" } else { "repeat" };
    let playing = state.status == PlayerStatus::Playing;
    let from = drag_from();
    let over = drop_at();
    rsx! {
        div {
            class: "jrm-queue",
            onmouseup: move |_| {
                if let (Some(f), Some(to)) = (drag_from(), drop_at()) {
                    if f != to {
                        p_end.move_item(f, to);
                    }
                }
                finish();
            },
            onmouseleave: move |_| finish(),
            div { class: "jrm-queue__header",
                h3 { class: "jr-text-title-3 jr-color-primary", {t.t("music:queue.title")} }
                RadioIndicator {}
                span { class: "jrm-queue__tools",
                    IconButton {
                        icon: "shuffle",
                        label: t.t("music:player.shuffle"),
                        pressed: model.shuffle,
                        size: "small",
                        onclick: move |_| p_shuffle.toggle_shuffle(),
                    }
                    IconButton {
                        icon: repeat_icon,
                        label: t.t(&format!("music:player.repeat.{}", repeat.as_str())),
                        pressed: repeat != RepeatMode::Off,
                        size: "small",
                        onclick: move |_| p_repeat.cycle_repeat(),
                    }
                    Button {
                        variant: "plain",
                        size: "small",
                        disabled: index >= count as isize - 1,
                        onclick: move |_| p_clear.clear_upcoming(),
                        {t.t("music:queue.clear")}
                    }
                }
            }
            ol { class: "jrm-queue__list", "aria-label": t.t("music:queue.title"),
                for (i, item) in model.queue.iter().enumerate() {
                    {
                        let track = item.track.clone();
                        let is_current = i as isize == index;
                        let title = track.name().to_string();
                        let drop_state = match (over, from) {
                            (Some(o), Some(f)) if o == i && f != i => Some(if f < i { "after" } else { "before" }),
                            _ => None,
                        };
                        let right = if is_current && playing {
                            t.t("music:queue.playing")
                        } else {
                            format_duration(track.duration_seconds())
                        };
                        let (p_jump, p_remove) = (player.clone(), player.clone());
                        rsx! {
                            li {
                                key: "{item.uid}",
                                class: "jrm-queue__item",
                                "data-current": is_current.then_some("true"),
                                "data-played": ((i as isize) < index).then_some("true"),
                                "data-dragging": (from == Some(i)).then_some("true"),
                                "data-drop": drop_state,
                                onmounted: move |e| {
                                    // The view opens on the current track.
                                    if is_current {
                                        let el = e.data();
                                        spawn(async move {
                                            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                                            let _ = el.scroll_to(dioxus::html::ScrollBehavior::Instant).await;
                                        });
                                    }
                                },
                                onmouseenter: move |_| if drag_from.peek().is_some() { drop_at.set(Some(i)) },
                                span {
                                    class: "jrm-queue__grip-hit",
                                    style: "display: contents;",
                                    onmousedown: move |e| {
                                        e.stop_propagation();
                                        drag_from.set(Some(i));
                                        drop_at.set(Some(i));
                                    },
                                    Icon { name: "grip-vertical", size: 16, class: "jrm-queue__grip" }
                                }
                                button {
                                    r#type: "button",
                                    class: "jrm-queue__play",
                                    "aria-label": t.tf("music:queue.play", &[("title", title.clone())]),
                                    "aria-current": is_current.then_some("true"),
                                    onmousedown: move |e| e.stop_propagation(),
                                    onclick: move |_| p_jump.jump_to(i),
                                    Artwork {
                                        src: track_art(&base, &track, 96),
                                        shape: "rounded",
                                        class: "jrm-queue__art",
                                    }
                                    span { class: "jrm-queue__text",
                                        p { class: "jr-text-subheadline jr-color-primary jr-truncate jrm-queue__title", "{title}" }
                                        p { class: "jr-text-footnote jr-color-secondary jr-truncate", {track_artist(&track)} }
                                    }
                                }
                                p { class: "jr-text-footnote jr-color-secondary jr-tabular", "{right}" }
                                IconButton {
                                    icon: "x",
                                    label: t.tf("music:queue.remove", &[("title", title.clone())]),
                                    size: "small",
                                    onclick: move |_| p_remove.remove_at(i),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
