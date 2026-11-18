//! `pages/AlbumPage.tsx` + `components/DetailHeader.tsx`: the reference screen port.
use dioxus::prelude::*;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsPage, ItemsParams};

use super::rows::SongRows;
use super::{art, format_total, toggle_favorite, use_base_url};
use crate::design::{
    use_overlay, Artwork, Button, EmptyState, IconButton, MenuItem, Shelf, Spinner, SquareCard,
};
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::{use_page, use_shell, PageHeader, ShellPage};
use crate::state::{use_app, use_services};

#[component]
pub fn AlbumPage(id: String) -> Element {
    let t = use_i18n();
    let app = use_app();
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let acc = session.account_id.clone();

    // useItem(id) / useAlbumTracks(id): two cached queries, the TanStack keys of keys.ts.
    let album = use_query(keys::item(&acc, &id), stale::ITEM, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.item(&id).await }
        }
    });
    let tracks = use_query(keys::album_tracks(&acc, &id), stale::CATALOG, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.album_tracks(&id).await }
        }
    });
    let artist_id = album
        .data
        .as_ref()
        .and_then(|a| a.album_artists.first())
        .and_then(|a| a.id.clone());
    let more_params = ItemsParams {
        include_item_types: vec!["MusicAlbum".into()],
        album_artist_ids: artist_id.iter().cloned().collect(),
        sort_by: vec!["ProductionYear".into(), "SortName".into()],
        sort_order: Some("Descending".into()),
        page_size: Some(20),
        ..ItemsParams::default()
    };
    let more = use_query::<ItemsPage, _, _>(keys::items(&acc, &more_params), stale::CATALOG, {
        let s = session.clone();
        let enabled = artist_id.is_some();
        move || {
            let (s, p) = (s.clone(), more_params.clone());
            async move {
                if enabled {
                    s.items_page(&p, 0).await
                } else {
                    Ok(ItemsPage::default())
                }
            }
        }
    });

    if album.is_loading() || tracks.is_loading() {
        return rsx! {
            ShellPage { title: "", header: PageHeader::Inline,
                div { class: "jrm-center", Spinner { size: "large" } }
            }
        };
    }
    let Some(item) = album.data.clone() else {
        return rsx! {
            ShellPage { title: t.t("music:nav.albums"), header: PageHeader::Inline,
                EmptyState { icon: "disc-3", title: t.t("music:album.notFound") }
            }
        };
    };
    let songs: Vec<BaseItemDto> = tracks.data.as_deref().cloned().unwrap_or_default();
    let total: f64 = songs.iter().map(|s| s.duration_seconds()).sum();
    let other: Vec<BaseItemDto> = more
        .data
        .as_ref()
        .map(|p| {
            p.items
                .iter()
                .filter(|a| a.id != item.id)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let base = use_base_url();
    let artist_name = item.album_artist.clone();
    let meta = [
        item.genres.first().cloned(),
        item.production_year.map(|y| y.to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");

    rsx! {
        ShellPage { title: item.name().to_string(), header: PageHeader::Inline, gutter: true,
            DetailHeader { item: (*item).clone(), songs: songs.clone(), meta, artist_id: artist_id.clone() }
            SongRows {
                tracks: songs.clone(),
                lead: "number",
                hide_artist: artist_name.clone(),
                group_by_disc: true,
                label: if item.name().is_empty() { t.t("music:nav.albums") } else { item.name().to_string() },
            }
            if !songs.is_empty() {
                p { class: "jr-text-footnote jr-color-secondary jrm-footer",
                    {format!("{} · {}", t.tc("music:album.footer", songs.len() as i64, &[]), format_total(t, total))}
                }
            }
            if !other.is_empty() {
                div { class: "jrm-bleed",
                    Shelf { title: t.tf("music:album.moreBy", &[("artist", artist_name.clone().unwrap_or_default())]), kind: "square",
                        for a in other {
                            AlbumCard { key: "{a.id}", album: a.clone(), image: art(&base, &a, 320) }
                        }
                    }
                }
            }
        }
    }
}

/// `components/Cards.tsx` `AlbumCard`.
#[component]
fn AlbumCard(album: BaseItemDto, image: Option<String>) -> Element {
    let svc = use_services();
    let app = use_app();
    let subtitle = [
        album.album_artist.clone(),
        album.production_year.map(|y| y.to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let id = album.id.clone();
    rsx! {
        SquareCard {
            title: album.name().to_string(),
            subtitle: (!subtitle.is_empty()).then_some(subtitle),
            image,
            fallback: "disc-3",
            to: format!("/music/album/{id}"),
            on_play: move |_| {
                // musicPlayer.playAlbum(id): fetch the tracks (cached), then play.
                let (player, cache) = (svc.music.clone(), svc.cache.clone());
                let Some(s) = app.session.peek().clone() else { return };
                let id = id.clone();
                spawn(async move {
                    let key = keys::album_tracks(&s.account_id, &id);
                    let s2 = s.clone();
                    if let Ok(list) = cache.fetch(key, stale::CATALOG, move || async move { s2.album_tracks(&id).await }).await {
                        player.play(&list, Some(0), Some(false));
                    }
                });
            },
        }
    }
}

/// Album header: big artwork with a soft shadow, title, buttons (`DetailHeader.tsx`).
#[component]
fn DetailHeader(
    item: BaseItemDto,
    songs: Vec<BaseItemDto>,
    meta: String,
    artist_id: Option<String>,
) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let router = use_router();
    let overlay = use_overlay();
    let page = use_page();
    let mut shell = use_shell();
    let base = use_base_url();
    let favorite = item.is_favorite();
    let image = art(&base, &item, 232);
    // The page's artwork feeds the shell's ambient backdrop (ShellAmbient).
    let ambient = image.clone();
    use_effect(move || shell.ambient_art.set(ambient.clone()));

    let (play_list, shuffle_list, menu_list) = (songs.clone(), songs.clone(), songs);
    let (p1, p2, p3, p4) = (
        svc.music.clone(),
        svc.music.clone(),
        svc.music.clone(),
        svc.music.clone(),
    );
    let fav_item = item.clone();
    rsx! {
        header { class: "jrm-detail",
            Artwork { src: image, shadow: true, class: "jrm-detail__art" }
            div { class: "jrm-detail__info",
                h1 {
                    class: "jr-text-title-1 jr-color-primary jrm-detail__title",
                    "data-page-title": "",
                    onmounted: move |e| page.title_el.clone().set(Some(e.data())),
                    "{item.name()}"
                }
                if let Some(artist) = item.album_artist.clone() {
                    h3 { class: "jr-text-title-3 jr-color-primary jrm-detail__subtitle",
                        match artist_id {
                            Some(aid) => rsx! { a { class: "jrm-link jrm-link--accent", onclick: move |_| router.push(&format!("/music/artist/{aid}")), "{artist}" } },
                            None => rsx! { "{artist}" },
                        }
                    }
                }
                if !meta.is_empty() {
                    p { class: "jr-text-footnote jr-color-secondary jrm-detail__meta", "{meta}" }
                }
                div { class: "jrm-detail__actions",
                    Button { icon: "play", icon_fill: true, onclick: move |_| p1.play(&play_list, Some(0), Some(false)),
                        {t.t("music:actions.play")}
                    }
                    Button { variant: "tinted", icon: "shuffle", onclick: move |_| p2.play(&shuffle_list, None, Some(true)),
                        {t.t("music:actions.shuffle")}
                    }
                    IconButton {
                        icon: "heart",
                        fill: favorite,
                        label: if favorite { t.t("music:actions.unfavorite") } else { t.t("music:actions.favorite") },
                        variant: "gray",
                        pressed: favorite,
                        onclick: move |_| toggle_favorite(&svc, app.session.peek().clone(), &fav_item, !favorite),
                    }
                    IconButton {
                        icon: "ellipsis",
                        label: t.t("music:actions.more"),
                        variant: "gray",
                        onclick: move |e: MouseEvent| {
                            // The base of `useContainerMenu`: queue actions (playlists and radio come
                            // with their features).
                            let p = e.client_coordinates();
                            let (a, b) = (p3.clone(), p4.clone());
                            let (next, last) = (menu_list.clone(), menu_list.clone());
                            overlay.menu(p.x, p.y, vec![
                                MenuItem::action(t.t("music:actions.playNext"), Some("list-start"), move |_| a.play_next(&next)),
                                MenuItem::action(t.t("music:actions.addToQueue"), Some("list-end"), move |_| b.add_to_queue(&last)),
                            ]);
                        },
                    }
                }
            }
        }
    }
}
