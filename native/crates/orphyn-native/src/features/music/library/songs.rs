//! `pages/SongsPage.tsx`: every song of the library in a virtualised table. Only the visible rows
//! (plus a margin) are in the DOM, pages of 100 are fetched as the window nears the end, and the
//! scroll height already reflects the whole library.
use std::rc::Rc;

use dioxus::prelude::*;
use orphyn_jellyfin::keys;
use orphyn_jellyfin::{BaseItemDto, ItemsPage, ItemsParams};

use super::cards::track_menu;
use super::paged::use_paged;
use crate::design::{use_overlay, Button, EmptyState, Icon, TrackRow};
use crate::features::music::rows::FavoriteAccessory;
use crate::features::music::{track_art, track_artist, track_artist_id, use_base_url, use_music};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::shell::ShellPage;
use crate::state::{use_app, use_services};

/// Fixed row height (px): required by windowing, matches the `.jrm-songs` CSS.
const ROW_HEIGHT: f64 = 52.0;
const PAGE_SIZE: u32 = 100;
const OVERSCAN: usize = 12;
/// Rows drawn below the first visible one (a tall window: the viewport height is not queried).
const WINDOW_ROWS: usize = 34;

#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Name,
    Artist,
    Album,
    Time,
}

impl SortKey {
    fn fields(self) -> &'static [&'static str] {
        match self {
            SortKey::Name => &["SortName"],
            SortKey::Artist => &["Artist", "Album", "SortName"],
            SortKey::Album => &["Album", "ParentIndexNumber", "IndexNumber"],
            SortKey::Time => &["Runtime", "SortName"],
        }
    }
}

#[component]
pub fn SongsPage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let router = use_router();
    let overlay = use_overlay();
    let music = use_music();
    let base = use_base_url();
    let mut sort = use_signal(|| (SortKey::Name, true));
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();

    let params = ItemsParams {
        include_item_types: vec!["Audio".into()],
        sort_by: sort().0.fields().iter().map(|s| s.to_string()).collect(),
        sort_order: Some(if sort().1 { "Ascending" } else { "Descending" }.into()),
        page_size: Some(PAGE_SIZE),
        ..ItemsParams::default()
    };
    let fetch = {
        let (session, params) = (session.clone(), params.clone());
        move |start: usize| {
            let (session, params) = (session.clone(), params.clone());
            async move {
                match session {
                    Some(s) => s.items_page(&params, start).await,
                    None => Ok(ItemsPage::default()),
                }
            }
        }
    };
    let paged = use_paged(keys::items(&acc, &params), fetch.clone());
    let songs: Vec<BaseItemDto> = paged.items.read().clone();
    let total = (paged.total)().unwrap_or(0);

    // Window: first visible row from the list's measured offset, re-evaluated on scroll.
    let mut list: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let mut first = use_signal(|| 0usize);
    let scroll = (router.scroll_y)();
    use_effect(use_reactive!(|scroll| {
        let _ = scroll;
        if let Some(el) = list.peek().clone() {
            spawn(async move {
                if let Ok(r) = el.get_client_rect().await {
                    let above = (-r.origin.y).max(0.0);
                    let row = (above / ROW_HEIGHT) as usize;
                    if row.abs_diff(*first.peek()) >= 2 || row == 0 {
                        first.set(row);
                    }
                }
            });
        }
    }));
    let start = first().saturating_sub(OVERSCAN);
    let end = (first() + WINDOW_ROWS + OVERSCAN).min(total);
    // Load more when the window gets close to the end of what is loaded.
    use_effect(use_reactive!(|end| {
        if end + 30 > songs_len(&paged) && paged.has_next() {
            paged.fetch_more(fetch.clone());
        }
    }));

    let current_id = (music.state)().current().map(|q| q.track.id.clone());
    let playing = (music.state)().is_playing();

    let header = |key: SortKey, label: String, class: &'static str| {
        let (cur, asc) = sort();
        let icon = if asc { "arrow-up" } else { "arrow-down" };
        let aria = if cur == key {
            if asc {
                "ascending"
            } else {
                "descending"
            }
        } else {
            "none"
        };
        rsx! {
            button {
                r#type: "button",
                class: "jrm-songs__col {class}",
                "aria-sort": aria,
                onclick: move |_| {
                    let (cur, asc) = *sort.peek();
                    sort.set(if cur == key { (key, !asc) } else { (key, true) });
                },
                "{label}"
                if cur == key { Icon { name: icon, size: 12 } }
            }
        }
    };

    let loading = (paged.loading)();
    let failed = (paged.error)() && songs.is_empty();
    rsx! {
        ShellPage {
            title: t.t("music:nav.songs"),
            subtitle: (total > 0).then(|| t.tc("music:library.songCount", total as i64, &[])),
            gutter: true,
            if failed {
                EmptyState {
                    title: t.t("music:library.error"),
                    action: rsx! { Button { onclick: move |_| {}, {t.t("music:library.retry")} } },
                }
            } else if !loading && total == 0 {
                EmptyState { icon: "music", title: t.t("music:songs.empty"), message: rsx! { {t.t("music:songs.emptyHint")} } }
            } else {
                div { class: "jrm-songs", onmounted: move |e| list.set(Some(e.data())),
                    div { class: "jrm-songs__head", role: "row",
                        span { class: "jrm-songs__lead" }
                        span { class: "jrm-songs__main",
                            {header(SortKey::Name, t.t("music:songs.title"), "")}
                            {header(SortKey::Artist, t.t("music:songs.artist"), "")}
                        }
                        {header(SortKey::Album, t.t("music:songs.album"), "jrm-songs__album")}
                        {header(SortKey::Time, t.t("music:songs.time"), "jrm-songs__time")}
                        span { class: "jrm-songs__more" }
                    }
                    if loading {
                        p { class: "jr-text-footnote jr-color-secondary jrm-songs__loading", {t.t("music:library.loading")} }
                    } else {
                        div { style: "padding-top: {start as f64 * ROW_HEIGHT}px; padding-bottom: {total.saturating_sub(end) as f64 * ROW_HEIGHT}px;",
                            ol { class: "jr-track-list", "aria-label": t.t("music:nav.songs"),
                                for i in start..end {
                                    if let Some(song) = songs.get(i) {
                                        {
                                            let song = song.clone();
                                            let is_current = current_id.as_deref() == Some(song.id.as_str());
                                            let title = song.name().to_string();
                                            let (list_all, p_play, p_pause, p_menu) = (songs.clone(), svc.music.clone(), svc.music.clone(), svc.music.clone());
                                            let more = song.clone();
                                            let artist_id = track_artist_id(&song);
                                            let artist = track_artist(&song);
                                            let artist_el = (!artist.is_empty()).then(|| match artist_id {
                                                Some(aid) => rsx! {
                                                    a { class: "jrm-link", onclick: move |e| { e.stop_propagation(); router.push(&format!("/music/artist/{aid}")) }, "{artist}" }
                                                },
                                                None => rsx! { "{artist}" },
                                            });
                                            rsx! {
                                                TrackRow {
                                                    key: "{song.id}",
                                                    artwork: track_art(&base, &song, 80),
                                                    title: title.clone(),
                                                    artist: artist_el,
                                                    album: song.album.clone(),
                                                    duration: song.duration_seconds(),
                                                    is_current,
                                                    is_playing: playing,
                                                    play_label: if is_current && playing { t.t("shell:design.nowPlaying") } else { t.tf("shell:design.playItem", &[("title", title.clone())]) },
                                                    more_label: t.tf("shell:design.moreFor", &[("title", title.clone())]),
                                                    accessory: rsx! { FavoriteAccessory { track: song.clone() } },
                                                    on_play: move |_| {
                                                        if is_current && !playing {
                                                            p_play.resume();
                                                        } else {
                                                            // The queue is the loaded part of the list, from the chosen song.
                                                            p_play.play(&list_all, Some(i), Some(false));
                                                        }
                                                    },
                                                    on_pause: move |_| p_pause.pause(),
                                                    on_more: move |e: MouseEvent| {
                                                        let p = e.client_coordinates();
                                                        overlay.menu(p.x, p.y, track_menu(t, &p_menu, router, more.clone()));
                                                    },
                                                }
                                            }
                                        }
                                    } else {
                                        li { key: "sk-{i}", class: "jrm-songs__placeholder", "aria-hidden": "true",
                                            div { class: "jr-skeleton jr-skeleton--text", style: "height: 12px;" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn songs_len(p: &super::paged::Paged) -> usize {
    p.items.read().len()
}
