//! `components/Cards.tsx`: `AlbumCard`, `PlaylistCard`, `ArtistCard` plus the container menu
//! (`useContainerMenu`) and the queue helpers of `player/api.ts` (`playItem`, `queueItem`).
use dioxus::prelude::*;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};
use orphyn_player::music::MusicPlayer;

use crate::design::{use_overlay, Artwork, IconButton, MenuItem};
use crate::features::music::panels::use_track_menu;
use crate::features::music::{art, toggle_favorite, use_base_url};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::{use_app, use_services, Services};

/// `fetchTracksOf`: the songs behind an album, playlist, artist or song id.
pub async fn tracks_of(svc: &Services, session: &Session, item: &BaseItemDto) -> Vec<BaseItemDto> {
    let acc = session.account_id.clone();
    let id = item.id.clone();
    let s = session.clone();
    let cache = svc.cache.clone();
    let r = match item.item_type.as_deref() {
        Some("MusicAlbum") => cache
            .fetch(
                keys::album_tracks(&acc, &id),
                stale::CATALOG,
                move || async move { s.album_tracks(&id).await },
            )
            .await
            .map(|a| (*a).clone()),
        Some("Playlist") => cache
            .fetch(
                keys::playlist_items(&acc, &id),
                stale::ITEM,
                move || async move { s.playlist_items(&id).await },
            )
            .await
            .map(|a| (*a).clone()),
        Some("MusicArtist") => artist_songs(session, &id, 500).await,
        _ => Ok(vec![item.clone()]),
    };
    r.unwrap_or_default()
}

/// `fetchArtistSongs`: every song of an artist, album by album.
pub async fn artist_songs(
    session: &Session,
    artist_id: &str,
    limit: u32,
) -> orphyn_jellyfin::JfResult<Vec<BaseItemDto>> {
    let p = ItemsParams {
        include_item_types: vec!["Audio".into()],
        artist_ids: vec![artist_id.to_string()],
        sort_by: vec![
            "Album".into(),
            "ParentIndexNumber".into(),
            "IndexNumber".into(),
        ],
        sort_order: Some("Ascending".into()),
        page_size: Some(limit),
        ..ItemsParams::default()
    };
    Ok(session.items_page(&p, 0).await?.items)
}

/// `musicPlayer.playItem` / `playAlbum` / `playPlaylist` / `playArtist`.
pub fn play_item(svc: &Services, session: Option<Session>, item: &BaseItemDto, shuffle: bool) {
    let Some(session) = session else { return };
    let (svc, item) = (svc.clone(), item.clone());
    spawn(async move {
        let tracks = tracks_of(&svc, &session, &item).await;
        if tracks.is_empty() {
            return;
        }
        svc.music
            .play(&tracks, (!shuffle).then_some(0), Some(shuffle));
    });
}

/// `musicPlayer.queueItem`.
pub fn queue_item(svc: &Services, session: Option<Session>, item: &BaseItemDto, next: bool) {
    let Some(session) = session else { return };
    let (svc, item) = (svc.clone(), item.clone());
    spawn(async move {
        let tracks = tracks_of(&svc, &session, &item).await;
        let p: &MusicPlayer = &svc.music;
        if next {
            p.play_next(&tracks)
        } else {
            p.add_to_queue(&tracks)
        }
    });
}

/// `useContainerMenu`: play, shuffle, queue and favourite entries of an album / playlist / artist.
pub fn container_menu(
    t: crate::i18n::I18n,
    svc: &Services,
    session: Option<Session>,
    item: &BaseItemDto,
) -> Vec<MenuItem> {
    let favorite = item.is_favorite();
    let mut out = Vec::new();
    let mk = |label: String, icon: &'static str, f: Box<dyn Fn()>| {
        MenuItem::action(label, Some(icon), move |_| f())
    };
    let (s, ses, it) = (svc.clone(), session.clone(), item.clone());
    out.push(mk(
        t.t("music:actions.play"),
        "play",
        Box::new(move || play_item(&s, ses.clone(), &it, false)),
    ));
    let (s, ses, it) = (svc.clone(), session.clone(), item.clone());
    out.push(mk(
        t.t("music:actions.shuffle"),
        "shuffle",
        Box::new(move || play_item(&s, ses.clone(), &it, true)),
    ));
    out.push(MenuItem::Separator);
    let (s, ses, it) = (svc.clone(), session.clone(), item.clone());
    out.push(mk(
        t.t("music:actions.playNext"),
        "list-start",
        Box::new(move || queue_item(&s, ses.clone(), &it, true)),
    ));
    let (s, ses, it) = (svc.clone(), session.clone(), item.clone());
    out.push(mk(
        t.t("music:actions.addToQueue"),
        "list-end",
        Box::new(move || queue_item(&s, ses.clone(), &it, false)),
    ));
    out.push(MenuItem::Separator);
    let (s, ses, it) = (svc.clone(), session, item.clone());
    out.push(mk(
        if favorite {
            t.t("music:actions.unfavorite")
        } else {
            t.t("music:actions.favorite")
        },
        "heart",
        Box::new(move || toggle_favorite(&s, ses.clone(), &it, !favorite)),
    ));
    out
}

/// First letters of the first two words (`initials` of `Artwork.tsx`).
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// The `MediaCard` DOM (`<div class="jr-card">`, no list item around it). `round` = `RoundCard`.
#[component]
pub fn MediaCard(
    item: BaseItemDto,
    title: String,
    subtitle: Option<String>,
    image: Option<String>,
    #[props(default = "music")] fallback: &'static str,
    #[props(default = false)] round: bool,
    #[props(default)] to: String,
    /// Track cards: clicking (or the play button) plays instead of navigating.
    on_activate: Option<EventHandler<()>>,
    #[props(default = false)] playable: bool,
) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let router = use_router();
    let track_menu_ctx = use_track_menu();
    let overlay = use_overlay();
    let kind = if round { "round" } else { "square" };
    let actions = if playable {
        " jr-card--has-actions"
    } else {
        ""
    };
    let shape = if round { "circle" } else { "rounded" };
    let (menu_item, menu_svc) = (item.clone(), svc.clone());
    let (play_item_v, play_svc) = (item.clone(), svc.clone());
    let on_activate2 = on_activate;
    let center = if round { " jr-card__text--center" } else { "" };
    rsx! {
        div { class: "jr-card jr-card--{kind}{actions}", style: "--jr-card-ratio: 1 / 1;",
            a {
                class: "jr-card__hit",
                draggable: "false",
                onclick: move |_| match &on_activate { Some(h) => h.call(()), None => router.push(&to) },
                oncontextmenu: move |e: MouseEvent| {
                    e.prevent_default();
                    let p = e.client_coordinates();
                    let session = app.session.peek().clone();
                    let mut items = container_menu(t, &menu_svc, session, &menu_item);
                    track_menu_ctx.container_items(p.x, p.y, &menu_item, false, &mut items);
                    overlay.menu(p.x, p.y, items);
                },
                div { class: "jr-card__art",
                    if round && image.is_none() {
                        Artwork { src: None::<String>, shape, shadow: true, fallback, class: "jr-artwork--initials" }
                        span { class: "jr-card__initials", {initials(&title)} }
                    } else {
                        Artwork { src: image, shape, shadow: true, fallback }
                    }
                }
                div { class: "jr-card__text{center}",
                    span { class: "jr-card__title", "{title}" }
                    if let Some(s) = subtitle { span { class: "jr-card__subtitle", "{s}" } }
                }
            }
            if playable {
                div { class: "jr-card__actions",
                    IconButton {
                        class: "jr-card__play",
                        variant: "glass",
                        icon: "play",
                        fill: true,
                        label: t.tf("shell:design.playItem", &[("title", title.clone())]),
                        onclick: move |_| match &on_activate2 { Some(h) => h.call(()), None => play_item(&play_svc, app.session.peek().clone(), &play_item_v, false) },
                    }
                }
            }
        }
    }
}

/// `AlbumCard`.
#[component]
pub fn AlbumCard(album: BaseItemDto) -> Element {
    let base = use_base_url();
    let subtitle = [
        album.album_artist.clone(),
        album.production_year.map(|y| y.to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    rsx! {
        MediaCard {
            title: album.name().to_string(),
            subtitle: (!subtitle.is_empty()).then_some(subtitle),
            image: art(&base, &album, 320),
            fallback: "disc-3",
            to: format!("/music/album/{}", album.id),
            playable: true,
            item: album,
        }
    }
}

/// `PlaylistCard`.
#[component]
pub fn PlaylistCard(playlist: BaseItemDto) -> Element {
    let base = use_base_url();
    rsx! {
        MediaCard {
            title: playlist.name().to_string(),
            subtitle: playlist.child_count.map(|c| c.to_string()),
            image: art(&base, &playlist, 320),
            fallback: "list-music",
            to: format!("/music/playlist/{}", playlist.id),
            playable: true,
            item: playlist,
        }
    }
}

/// `ArtistCard`: a round card, initials when the artist has no picture.
#[component]
pub fn ArtistCard(artist: BaseItemDto) -> Element {
    let base = use_base_url();
    rsx! {
        MediaCard {
            title: artist.name().to_string(),
            image: art(&base, &artist, 320),
            fallback: "user",
            round: true,
            to: format!("/music/artist/{}", artist.id),
            item: artist,
        }
    }
}

/// `GridSkeleton`: placeholder grid while a library page loads.
#[component]
pub fn GridSkeleton(#[props(default = "square")] kind: &'static str) -> Element {
    let radius = if kind == "round" {
        "50%"
    } else {
        "var(--radius-artwork)"
    };
    rsx! {
        ul { class: "jr-grid jr-grid--{kind}", "aria-label": "…",
            for i in 0..12 {
                li { key: "{i}", class: "jr-grid__item",
                    div {
                        div { class: "jr-skeleton", style: "aspect-ratio: 1 / 1; border-radius: {radius};" }
                        div { class: "jr-skeleton jr-skeleton--text", style: "height: 12px; margin-top: 8px;" }
                    }
                }
            }
        }
    }
}

/// The "…" menu of a song (the base of `useTrackMenu`: queue actions and navigation).
pub fn track_menu(
    t: crate::i18n::I18n,
    player: &MusicPlayer,
    router: crate::router::Router,
    track: BaseItemDto,
) -> Vec<MenuItem> {
    let (a, b) = (player.clone(), player.clone());
    let (t1, t2) = (track.clone(), track.clone());
    let mut items = vec![
        MenuItem::action(
            t.t("music:actions.playNext"),
            Some("list-start"),
            move |_| a.play_next(std::slice::from_ref(&t1)),
        ),
        MenuItem::action(
            t.t("music:actions.addToQueue"),
            Some("list-end"),
            move |_| b.add_to_queue(std::slice::from_ref(&t2)),
        ),
        MenuItem::Separator,
    ];
    if let Some(album) = track.album_id.clone() {
        items.push(MenuItem::action(
            t.t("music:actions.goToAlbum"),
            Some("disc-3"),
            move |_| router.push(&format!("/music/album/{album}")),
        ));
    }
    if let Some(artist) = crate::features::music::track_artist_id(&track) {
        items.push(MenuItem::action(
            t.t("music:actions.goToArtist"),
            Some("mic-vocal"),
            move |_| router.push(&format!("/music/artist/{artist}")),
        ));
    }
    items
}
