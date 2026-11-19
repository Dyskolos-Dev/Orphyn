//! `components/MiniPlayer.tsx`: the persistent bar of the shell (transport left, "LCD" capsule in
//! the centre, volume / lyrics / queue right). Cast, PiP and SyncPlay buttons come with their
//! features.
use dioxus::prelude::*;

use super::transport::{track_duration, TransportButtons, VolumeControl};
use super::CurrentTrackHeart;
use super::{track_art, track_artist, use_base_url, use_music, NowPlayingView};
use crate::design::{format_duration, Artwork, IconButton, Slider};
use crate::i18n::use_i18n;
use crate::state::use_services;

/// Thin progress line at the bottom of the capsule; elapsed / remaining show on hover.
#[component]
fn CapsuleProgress() -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let position = (music.progress)().position;
    let duration = track_duration();
    let loading = (music.state)().status == orphyn_player::music::PlayerStatus::Loading;
    let mut scrub = use_signal(|| None::<f64>);
    let shown = scrub().unwrap_or(position).min(duration.max(0.0));
    rsx! {
        div { class: "jrm-mini__times jr-tabular", "aria-hidden": "true",
            span { {format_duration(shown)} }
            span { if duration > 0.0 { {format!("-{}", format_duration((duration - shown).max(0.0)))} } else { "--:--" } }
        }
        div { class: "jrm-mini__progress", "data-loading": loading.then_some("true"),
            Slider {
                label: t.t("music:player.seek"),
                value: shown,
                max: if duration > 0.0 { duration } else { 1.0 },
                disabled: duration <= 0.0,
                on_change_start: move |_| scrub.set(Some(position)),
                on_change: move |v| scrub.set(Some(v)),
                on_change_end: move |v| {
                    player.seek(v);
                    scrub.set(None);
                },
            }
        }
    }
}

#[component]
pub fn MiniPlayer() -> Element {
    let t = use_i18n();
    let music = use_music();
    let base = use_base_url();
    let s = (music.state)();
    let track = s.current().map(|q| q.track.clone());
    let lyrics_view = (music.open)() && (music.view)() == NowPlayingView::Lyrics;
    let queue_view = (music.open)() && (music.view)() == NowPlayingView::Queue;
    rsx! {
        div { class: "jrm-mini", role: "region", "aria-label": t.t("music:player.nowPlaying"),
            div { class: "jrm-mini__left", TransportButtons {} }
            div { class: "jrm-mini__capsule",
                if let Some(track) = track {
                    button {
                        r#type: "button",
                        class: "jrm-mini__info",
                        "aria-label": t.t("music:player.openNowPlaying"),
                        onclick: move |_| music.show(NowPlayingView::Player),
                        Artwork { src: track_art(&base, &track, 112), class: "jrm-mini__art" }
                        span { class: "jrm-mini__text",
                            span { class: "jr-text-subheadline jr-color-primary jr-truncate jrm-mini__title", "{track.name()}" }
                            span { class: "jr-text-footnote jr-color-secondary jr-truncate jrm-mini__subtitle",
                                {[track_artist(&track), track.album.clone().unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" — ")}
                            }
                        }
                    }
                    CurrentTrackHeart { size: "small", style: "flex: none; margin-bottom: 8px; margin-right: 4px;" }
                    CapsuleProgress {}
                } else {
                    div { class: "jrm-mini__info",
                        img { class: "jrm-mini__logo", src: crate::assets::logo_url(), width: "32", height: "32", alt: "" }
                        span { class: "jrm-mini__text",
                            span { class: "jr-text-subheadline jr-color-secondary jr-truncate jrm-mini__title", {t.t("music:player.notPlaying")} }
                        }
                    }
                }
            }
            div { class: "jrm-mini__right",
                crate::features::syncplay::SyncPlayButton { kind: crate::features::syncplay::MediaKind::Audio }
                crate::features::cast::MusicCastButton {}
                VolumeControl {}
                IconButton {
                    icon: "message-square-quote",
                    label: t.t("music:player.lyrics"),
                    pressed: lyrics_view,
                    onclick: move |_| music.toggle_view(NowPlayingView::Lyrics),
                }
                IconButton {
                    icon: "list-music",
                    label: t.t("music:player.queue"),
                    pressed: queue_view,
                    onclick: move |_| music.toggle_view(NowPlayingView::Queue),
                }
            }
        }
    }
}
