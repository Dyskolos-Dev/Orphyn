//! `TrackRows.tsx`: `SongRows` (album / playlist track lists) and the favourite accessory.
use dioxus::prelude::*;
use orphyn_jellyfin::BaseItemDto;

use super::{toggle_favorite, track_art, track_artist, track_artist_id, use_base_url, use_music};
use crate::design::{use_overlay, Icon, MenuItem, TrackRow};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::{use_app, use_services};

#[component]
pub fn FavoriteAccessory(track: BaseItemDto) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let favorite = track.is_favorite();
    rsx! {
        button {
            r#type: "button",
            tabindex: "-1",
            class: if favorite { "jrm-heart" } else { "jrm-heart jrm-heart--off" },
            "aria-label": if favorite { t.t("music:actions.unfavorite") } else { t.t("music:actions.favorite") },
            "aria-pressed": favorite.to_string(),
            onclick: move |e| {
                e.stop_propagation();
                toggle_favorite(&svc, app.session.peek().clone(), &track, !favorite);
            },
            Icon { name: "heart", size: 14, fill: favorite }
        }
    }
}

/// Track list: clicking a row's play button (or double-clicking it) plays the list from it.
#[component]
pub fn SongRows(
    tracks: Vec<BaseItemDto>,
    /// `number` or `artwork` in the leading column.
    #[props(default = "number")]
    lead: &'static str,
    hide_artist: Option<String>,
    #[props(default = false)] show_album: bool,
    #[props(default = false)] group_by_disc: bool,
    label: String,
) -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let router = use_router();
    let overlay = use_overlay();
    let base = use_base_url();
    let s = (music.state)();
    let current_id = s.current().map(|q| q.track.id.clone());
    let playing = s.is_playing();
    let multi_disc = group_by_disc && {
        let mut discs: Vec<i32> = tracks.iter().map(|x| x.parent_index_number.unwrap_or(1)).collect();
        discs.dedup();
        discs.len() > 1
    };
    rsx! {
        ol { class: "jr-track-list jrm-tracklist", "aria-label": "{label}",
            for (i, track) in tracks.iter().enumerate() {
                if multi_disc && (i == 0 || track.parent_index_number.unwrap_or(1) != tracks[i - 1].parent_index_number.unwrap_or(1)) {
                    li { key: "disc-{i}", class: "jrm-disc", "aria-hidden": "true",
                        p { class: "jr-text-footnote jr-color-secondary",
                            {t.tf("music:album.disc", &[("n", track.parent_index_number.unwrap_or(1).to_string())])}
                        }
                    }
                }
                {
                    let artist = track_artist(track);
                    let artist_id = track_artist_id(track);
                    let show_artist = !artist.is_empty() && Some(&artist) != hide_artist.as_ref();
                    let is_current = current_id.as_deref() == Some(track.id.as_str());
                    let (list, p_play, p_pause, p_menu) = (tracks.clone(), player.clone(), player.clone(), player.clone());
                    let title = track.name().to_string();
                    let artist_el: Option<Element> = if show_artist {
                        Some(match artist_id {
                            Some(aid) => rsx! {
                                a { class: "jrm-link", onclick: move |e| { e.stop_propagation(); router.push(&format!("/music/artist/{aid}")) }, "{artist}" }
                            },
                            None => rsx! { "{artist}" },
                        })
                    } else {
                        None
                    };
                    let more_track = track.clone();
                    rsx! {
                        TrackRow {
                            key: "{track.id}-{i}",
                            index: track.index_number.unwrap_or(i as i32 + 1),
                            artwork: if lead == "artwork" { track_art(&base, track, 80) } else { None },
                            title: title.clone(),
                            artist: artist_el,
                            album: if show_album { track.album.clone() } else { None },
                            duration: track.duration_seconds(),
                            is_current,
                            is_playing: playing,
                            play_label: if is_current && playing { t.t("shell:design.nowPlaying") } else { t.tf("shell:design.playItem", &[("title", title.clone())]) },
                            more_label: t.tf("shell:design.moreFor", &[("title", title.clone())]),
                            accessory: rsx! { FavoriteAccessory { track: track.clone() } },
                            on_play: move |_| {
                                if is_current && !playing {
                                    p_play.resume();
                                } else {
                                    p_play.play(&list, Some(i), Some(false));
                                }
                            },
                            on_pause: move |_| p_pause.pause(),
                            on_more: move |e: MouseEvent| {
                                let p = e.client_coordinates();
                                overlay.menu(p.x, p.y, track_menu(t, &p_menu, router, more_track.clone()));
                            },
                        }
                    }
                }
            }
        }
    }
}

/// The "…" menu of a song (the base of `useTrackMenu`: queue actions and navigation; playlist and
/// radio entries come with their features).
fn track_menu(
    t: crate::i18n::I18n,
    player: &orphyn_player::music::MusicPlayer,
    router: crate::router::Router,
    track: BaseItemDto,
) -> Vec<MenuItem> {
    let (a, b) = (player.clone(), player.clone());
    let (t1, t2) = (track.clone(), track.clone());
    let mut items = vec![
        MenuItem::action(t.t("music:actions.playNext"), Some("list-start"), move |_| a.play_next(std::slice::from_ref(&t1))),
        MenuItem::action(t.t("music:actions.addToQueue"), Some("list-end"), move |_| b.add_to_queue(std::slice::from_ref(&t2))),
        MenuItem::Separator,
    ];
    if let Some(album) = track.album_id.clone() {
        items.push(MenuItem::action(t.t("music:actions.goToAlbum"), Some("disc-3"), move |_| {
            router.push(&format!("/music/album/{album}"))
        }));
    }
    if let Some(artist) = track_artist_id(&track) {
        items.push(MenuItem::action(t.t("music:actions.goToArtist"), Some("mic-vocal"), move |_| {
            router.push(&format!("/music/artist/{artist}"))
        }));
    }
    items
}
