//! `components/NowPlaying.tsx`: the full-window sheet, sliding in with framer-motion's
//! `springs.smooth` (here `motion::use_spring`), with its Lyrics and Up Next panels.
use dioxus::prelude::*;

use super::panels::{LyricsView, PanelsHost, QueueView};
use super::transport::{SeekBar, TransportButtons, VolumeControl};
use super::{
    track_art, track_artist, track_artist_id, use_base_url, use_music, CurrentTrackHeart,
    NowPlayingView,
};
use crate::design::{Artwork, IconButton, Segment, SegmentedControl};
use crate::i18n::use_i18n;
use crate::motion::{springs, use_spring};
use crate::router::use_router;

#[component]
pub fn NowPlaying() -> Element {
    let music = use_music();
    let has_track = (music.state)().current().is_some();
    // Nothing to show once the queue is gone.
    let mut open = music.open;
    if open() && !has_track {
        open.set(false);
    }
    let target = if open() { 0.0 } else { 100.0 };
    let y = use_spring(target, springs::SMOOTH);
    // AnimatePresence: unmounted once the exit animation reached the bottom.
    if y() >= 99.9 && !open() {
        return rsx! { PanelsHost {} };
    }
    rsx! {
        PanelsHost {}
        Sheet { y: y() }
    }
}

#[component]
fn Sheet(y: f64) -> Element {
    let t = use_i18n();
    let music = use_music();
    let router = use_router();
    let base = use_base_url();
    let view = (music.view)();
    let Some(track) = (music.state)().current().map(|q| q.track.clone()) else {
        return rsx! {};
    };
    let art = track_art(&base, &track, 640);
    let bg = track_art(&base, &track, 160);
    let artist_id = track_artist_id(&track);
    let mut open = music.open;
    let view_name = match view {
        NowPlayingView::Player => "player",
        NowPlayingView::Lyrics => "lyrics",
        NowPlayingView::Queue => "queue",
    };
    let window = dioxus_native::use_window();
    let panel_name = if view == NowPlayingView::Lyrics {
        t.t("music:player.lyrics")
    } else {
        t.t("music:player.queue")
    };
    rsx! {
        div {
            class: "jrm-np",
            "data-theme": "dark",
            "data-view": view_name,
            role: "dialog",
            "aria-modal": "true",
            "aria-label": t.t("music:player.nowPlaying"),
            tabindex: "-1",
            style: "transform: translateY({y}%);",
            onkeydown: move |e| if e.key() == Key::Named(NamedKey::Escape) { open.set(false) },
            div { class: "jrm-np__bg", "aria-hidden": "true",
                if let Some(bg) = &bg {
                    div { class: "jrm-np__blob jrm-np__blob--a", style: "background-image: url(\"{bg}\");" }
                    div { class: "jrm-np__blob jrm-np__blob--b", style: "background-image: url(\"{bg}\");" }
                }
                div { class: "jrm-np__scrim" }
            }
            header {
                class: "jrm-np__header jr-drag",
                onmousedown: move |e| crate::window::drag_region_mousedown(&window, &e),
                IconButton {
                    icon: "chevron-down",
                    label: t.t("music:player.close"),
                    variant: "glass",
                    onclick: move |_| open.set(false),
                }
                span { onmousedown: move |e| e.stop_propagation(), style: "display: contents;",
                    SegmentedControl {
                        label: t.t("music:player.view"),
                        class: "jrm-np__tabs",
                        value: view_name,
                        options: vec![
                            Segment { value: "player", label: t.t("music:player.nowPlaying") },
                            Segment { value: "lyrics", label: t.t("music:player.lyrics") },
                            Segment { value: "queue", label: t.t("music:player.queue") },
                        ],
                        on_change: move |v: &'static str| music.show(match v {
                            "lyrics" => NowPlayingView::Lyrics,
                            "queue" => NowPlayingView::Queue,
                            _ => NowPlayingView::Player,
                        }),
                    }
                }
                div { class: "jrm-np__header-end", style: "align-items: center; gap: 8px;",
                    CurrentTrackHeart { variant: "glass" }
                }
            }
            div { class: "jrm-np__body",
                section { class: "jrm-np__main", "aria-label": t.t("music:player.nowPlaying"),
                    Artwork { src: art, shadow: true, class: "jrm-np__art" }
                    div { class: "jrm-np__meta",
                        span { class: "jr-text-title-2 jr-color-primary jr-truncate jrm-np__title", "{track.name()}" }
                        span { class: "jr-text-body jr-color-secondary jr-truncate",
                            match artist_id {
                                Some(aid) => rsx! {
                                    a { class: "jrm-link", onclick: move |_| { open.set(false); router.push(&format!("/music/artist/{aid}")) },
                                        {track_artist(&track)}
                                    }
                                },
                                None => rsx! { {track_artist(&track)} },
                            }
                            if let (Some(album), Some(album_id)) = (track.album.clone(), track.album_id.clone()) {
                                " — "
                                a { class: "jrm-link", onclick: move |_| { open.set(false); router.push(&format!("/music/album/{album_id}")) }, "{album}" }
                            }
                        }
                    }
                    SeekBar { on_media: true }
                    TransportButtons { size: "large" }
                    VolumeControl { on_media: true }
                }
                if view != NowPlayingView::Player {
                    section { class: "jrm-np__panel", "aria-label": "{panel_name}",
                        if view == NowPlayingView::Lyrics {
                            LyricsView { key: "{track.id}", track_id: track.id.clone(), has_lyrics: track.has_lyrics }
                        } else {
                            QueueView {}
                        }
                    }
                }
            }
        }
    }
}
