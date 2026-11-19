//! `LibraryPage.tsx` and the grid pages built on it: albums, artists (with grouping),
//! playlists; plus `favorites/FavoritesPage.tsx`.
use dioxus::prelude::*;
use orphyn_jellyfin::api::music::{ArtistList, FavoriteKind};
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsPage, ItemsParams};

use super::artist_groups::group_artists;
use super::cards::{AlbumCard, ArtistCard, GridSkeleton, PlaylistCard};
use super::paged::{
    open_sort_menu, use_end_reached, use_paged, use_sort_pref, Paged, SortMenu, SortOption,
};
use crate::design::{
    use_overlay, Button, EmptyState, IconButton, Segment, SegmentedControl, Spinner,
};
use crate::features::music::rows::SongRows;
use crate::i18n::use_i18n;
use crate::shell::{PageHeader, ShellPage};
use crate::state::{use_app, use_services};

#[derive(Clone, Copy, PartialEq)]
enum Card {
    Album,
    Artist,
    Playlist,
}

fn sort_option(
    id: &'static str,
    label: String,
    sort_by: &'static [&'static str],
    default_order: &'static str,
) -> SortOption {
    SortOption {
        id,
        label,
        sort_by,
        default_order,
    }
}

/// `LibraryPage`: title, count, sort button, paginated grid with its loading / error / empty
/// states.
#[component]
fn LibraryGrid(
    title: String,
    paged: Paged,
    card: Card,
    empty_icon: &'static str,
    empty_title: String,
    empty_message: String,
    sort: Option<Element>,
    actions: Option<Element>,
    on_more: Callback<()>,
    on_retry: Callback<()>,
) -> Element {
    let t = use_i18n();
    let items: Vec<BaseItemDto> = paged.items.read().clone();
    let total = (paged.total)();
    let kind = if card == Card::Artist {
        "round"
    } else {
        "square"
    };
    let min = if card == Card::Artist { 140 } else { 160 };
    let sentinel = use_end_reached(items.len(), move || {
        if paged.has_next() {
            on_more.call(())
        }
    });
    rsx! {
        ShellPage {
            title: title.clone(),
            subtitle: total.map(|n| t.tc("music:library.count", n as i64, &[])),
            gutter: true,
            actions: rsx! { {actions} {sort} },
            if (paged.loading)() {
                GridSkeleton { kind }
            } else if (paged.error)() && items.is_empty() {
                EmptyState {
                    title: t.t("music:library.error"),
                    action: rsx! { Button { onclick: move |_| on_retry.call(()), {t.t("music:library.retry")} } },
                }
            } else if items.is_empty() {
                EmptyState { icon: empty_icon, title: empty_title, message: rsx! { "{empty_message}" } }
            } else {
                ul { class: "jr-grid jr-grid--{kind} jrm-grid", style: "--jr-grid-min: {min}px;", "aria-label": title.clone(),
                    for item in items {
                        li { key: "{item.id}", class: "jr-grid__item",
                            div {
                                match card {
                                    Card::Album => rsx! { AlbumCard { album: item.clone() } },
                                    Card::Artist => rsx! { ArtistCard { artist: item.clone() } },
                                    Card::Playlist => rsx! { PlaylistCard { playlist: item.clone() } },
                                }
                            }
                        }
                    }
                }
                div { class: "jrm-sentinel", "aria-hidden": "true", onmounted: move |e| sentinel.call(e.data()),
                    if (paged.fetching_more)() { Spinner {} }
                }
            }
        }
    }
}

/// `useItems` / `useList` for a `GET /Items` list, paged by `page_size`.
fn use_items_list(params: ItemsParams) -> (Paged, Callback<()>) {
    let app = use_app();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let key = keys::items(&acc, &params);
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
    let paged = use_paged(key, fetch.clone());
    let more = Callback::new(move |_| paged.fetch_more(fetch.clone()));
    (paged, more)
}

#[component]
pub fn AlbumsPage() -> Element {
    let t = use_i18n();
    let options = vec![
        sort_option("name", t.t("music:sort.name"), &["SortName"], "Ascending"),
        sort_option(
            "artist",
            t.t("music:sort.artist"),
            &["AlbumArtist", "SortName"],
            "Ascending",
        ),
        sort_option(
            "year",
            t.t("music:sort.year"),
            &["ProductionYear", "SortName"],
            "Descending",
        ),
        sort_option(
            "added",
            t.t("music:sort.added"),
            &["DateCreated", "SortName"],
            "Descending",
        ),
    ];
    let pref = use_sort_pref("albums", &options);
    let params = ItemsParams {
        include_item_types: vec!["MusicAlbum".into()],
        sort_by: pref.sort_by(&options),
        sort_order: Some(pref.order()),
        page_size: Some(60),
        ..ItemsParams::default()
    };
    let (paged, more) = use_items_list(params);
    rsx! {
        LibraryGrid {
            title: t.t("music:nav.albums"),
            paged,
            card: Card::Album,
            empty_icon: "disc-3",
            empty_title: t.t("music:albums.empty"),
            empty_message: t.t("music:albums.emptyHint"),
            sort: rsx! { SortMenu { pref, options } },
            on_more: more,
            on_retry: Callback::new(move |_| {}),
        }
    }
}

#[component]
pub fn PlaylistsPage() -> Element {
    let t = use_i18n();
    let options = vec![
        sort_option("name", t.t("music:sort.name"), &["SortName"], "Ascending"),
        sort_option(
            "added",
            t.t("music:sort.added"),
            &["DateCreated", "SortName"],
            "Descending",
        ),
    ];
    let pref = use_sort_pref("playlists", &options);
    let params = ItemsParams {
        include_item_types: vec!["Playlist".into()],
        media_types: vec!["Audio".into()],
        sort_by: pref.sort_by(&options),
        sort_order: Some(pref.order()),
        page_size: Some(60),
        ..ItemsParams::default()
    };
    let ui = crate::features::music::playlists::use_playlist_ui();
    let (paged, more) = use_items_list(params);
    rsx! {
        LibraryGrid {
            title: t.t("music:nav.playlists"),
            paged,
            card: Card::Playlist,
            empty_icon: "list-music",
            empty_title: t.t("music:playlists.empty"),
            empty_message: t.t("music:playlists.emptyHint"),
            sort: rsx! {
                Button { icon: "plus", onclick: move |_| ui.open_new(Vec::new()), {t.t("music:playlistSheet.title")} }
                SortMenu { pref, options }
            },
            on_more: more,
            on_retry: Callback::new(move |_| {}),
        }
    }
}

fn by_name(items: &mut [BaseItemDto], descending: bool) {
    let key = |a: &BaseItemDto| a.name().to_lowercase();
    items.sort_by_key(key);
    if descending {
        items.reverse();
    }
}

fn by_date(items: &mut [BaseItemDto], descending: bool) {
    items.sort_by(|a, b| a.date_created.cmp(&b.date_created));
    if descending {
        items.reverse();
    }
}

#[component]
pub fn ArtistsPage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let options = vec![
        sort_option("name", t.t("music:sort.name"), &["SortName"], "Ascending"),
        sort_option(
            "added",
            t.t("music:sort.added"),
            &["DateCreated", "SortName"],
            "Descending",
        ),
    ];
    let pref = use_sort_pref("artists", &options);
    let grouping = (app.preferences)().group_artists;
    let sort_by = pref.sort_by(&options);
    let order = pref.order();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();

    // Ungrouped: the paginated album-artist endpoint. Grouped: the two full lists, merged here.
    let params = ItemsParams {
        sort_by: sort_by.clone(),
        sort_order: Some(order.clone()),
        page_size: Some(if grouping { 1 } else { 80 }),
        ..ItemsParams::default()
    };
    let mut key = keys::items(&acc, &params);
    key.insert(
        2,
        if grouping {
            "artists-grouped-stub"
        } else {
            "album-artists"
        }
        .into(),
    );
    let fetch = {
        let (session, params) = (session.clone(), params.clone());
        move |start: usize| {
            let (session, params) = (session.clone(), params.clone());
            async move {
                match session {
                    Some(s) if !grouping => s.artists_page(ArtistList::Album, &params, start).await,
                    _ => Ok(ItemsPage::default()),
                }
            }
        }
    };
    let paged = use_paged(key, fetch.clone());
    let more = Callback::new(move |_| paged.fetch_more(fetch.clone()));

    let lists =
        super::artist_groups::use_artist_lists(session.as_ref().expect("signed in"), grouping);
    let descending = order == "Descending";
    let merged: Option<Vec<BaseItemDto>> = if grouping {
        match (&lists.album_artists, &lists.everyone) {
            (Some(a), Some(e)) => {
                let mut items: Vec<BaseItemDto> =
                    group_artists(a, e).into_iter().map(|g| g.item).collect();
                if sort_by.first().map(String::as_str) == Some("DateCreated") {
                    by_date(&mut items, descending)
                } else {
                    by_name(&mut items, descending)
                }
                Some(items)
            }
            _ => None,
        }
    } else {
        None
    };

    if grouping {
        let loading = merged.is_none() && !lists.error;
        let items = merged.unwrap_or_default();
        return rsx! {
            GroupedArtists { items, loading, error: lists.error, pref, options }
        };
    }
    rsx! {
        LibraryGrid {
            title: t.t("music:nav.artists"),
            paged,
            card: Card::Artist,
            empty_icon: "user",
            empty_title: t.t("music:artists.empty"),
            empty_message: t.t("music:artists.emptyHint"),
            sort: rsx! { SortMenu { pref, options } },
            on_more: more,
            on_retry: Callback::new(move |_| {}),
        }
    }
}

/// The artists page when grouping is on: everything is in memory, no pagination.
#[component]
fn GroupedArtists(
    items: Vec<BaseItemDto>,
    loading: bool,
    error: bool,
    pref: super::paged::SortPref,
    options: Vec<SortOption>,
) -> Element {
    let t = use_i18n();
    rsx! {
        ShellPage {
            title: t.t("music:nav.artists"),
            subtitle: (!loading && !items.is_empty()).then(|| t.tc("music:library.count", items.len() as i64, &[])),
            gutter: true,
            actions: rsx! { SortMenu { pref, options } },
            if loading {
                GridSkeleton { kind: "round" }
            } else if error {
                EmptyState { title: t.t("music:library.error") }
            } else if items.is_empty() {
                EmptyState { icon: "user", title: t.t("music:artists.empty"), message: rsx! { {t.t("music:artists.emptyHint")} } }
            } else {
                ul { class: "jr-grid jr-grid--round jrm-grid", style: "--jr-grid-min: 140px;", "aria-label": t.t("music:nav.artists"),
                    for item in items {
                        li { key: "{item.id}", class: "jr-grid__item",
                            div { ArtistCard { artist: item.clone() } }
                        }
                    }
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------- favourites

#[derive(Clone, Copy, PartialEq)]
enum FavTab {
    Songs,
    Albums,
    Artists,
}

impl FavTab {
    fn id(self) -> &'static str {
        match self {
            FavTab::Songs => "songs",
            FavTab::Albums => "albums",
            FavTab::Artists => "artists",
        }
    }
    fn from_id(id: &str) -> Self {
        match id {
            "albums" => FavTab::Albums,
            "artists" => FavTab::Artists,
            _ => FavTab::Songs,
        }
    }
    fn sorts(self) -> &'static [&'static str] {
        match self {
            FavTab::Songs => &["name", "artist", "album", "added"],
            FavTab::Albums => &["name", "artist", "year", "added"],
            FavTab::Artists => &["name", "added"],
        }
    }
}

/// `sortFavorites`: sorted copy, ties by name.
fn sort_favorites(items: &[BaseItemDto], sort: &str, descending: bool) -> Vec<BaseItemDto> {
    let text = |s: &Option<String>| s.clone().unwrap_or_default().to_lowercase();
    let key = |i: &BaseItemDto| -> (i64, String) {
        match sort {
            "artist" => (
                0,
                text(
                    &i.album_artist
                        .clone()
                        .or_else(|| i.artists.first().cloned()),
                ),
            ),
            "album" => (0, text(&i.album)),
            "year" => (i.production_year.unwrap_or(0) as i64, String::new()),
            "added" => (0, i.date_created.clone().unwrap_or_default()),
            _ => (0, i.name().to_lowercase()),
        }
    };
    let mut out = items.to_vec();
    out.sort_by(|a, b| {
        let r = key(a).cmp(&key(b));
        let r = if descending { r.reverse() } else { r };
        r.then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
    out
}

#[component]
pub fn FavoritesPage() -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let overlay = use_overlay();
    let saved: Option<serde_json::Value> = svc.storage.get_json("orphyn.music.favorites");
    let get = |k: &str| {
        saved
            .as_ref()
            .and_then(|v| v.get(k))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    let tab0 = FavTab::from_id(&get("tab").unwrap_or_default());
    let mut tab = use_signal(|| tab0);
    let mut sort = use_signal(|| {
        get("sort")
            .filter(|s| tab0.sorts().contains(&s.as_str()))
            .unwrap_or_else(|| "name".into())
    });
    let mut order = use_signal(|| get("order").unwrap_or_else(|| "Ascending".into()));
    let persist = {
        let svc = svc.clone();
        move |tab: FavTab, sort: &str, order: &str| {
            svc.storage.set_json(
                "orphyn.music.favorites",
                &serde_json::json!({ "tab": tab.id(), "sort": sort, "order": order }),
            );
        }
    };

    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let fav = |kind: FavoriteKind, name: &str| {
        let s = session.clone();
        crate::query::use_query(
            vec![
                "jf".into(),
                acc.clone(),
                "favorites".into(),
                name.to_string(),
            ],
            stale::ITEM,
            move || {
                let s = s.clone();
                async move {
                    match s {
                        Some(s) => s.favorites(kind).await,
                        None => Ok(Vec::new()),
                    }
                }
            },
        )
    };
    let songs = fav(FavoriteKind::Songs, "songs");
    let albums = fav(FavoriteKind::Albums, "albums");
    let artists = fav(FavoriteKind::Artists, "artists");
    let current = match tab() {
        FavTab::Songs => &songs,
        FavTab::Albums => &albums,
        FavTab::Artists => &artists,
    };
    let sorted = current
        .data
        .as_ref()
        .map(|d| sort_favorites(d, &sort(), order() == "Descending"))
        .unwrap_or_default();
    let count = |n: Option<usize>| {
        n.filter(|n| *n > 0)
            .map(|n| format!(" {n}"))
            .unwrap_or_default()
    };
    let segments = vec![
        Segment {
            value: "songs",
            label: format!(
                "{}{}",
                t.t("music:nav.songs"),
                count(songs.data.as_ref().map(|d| d.len()))
            ),
        },
        Segment {
            value: "albums",
            label: format!(
                "{}{}",
                t.t("music:nav.albums"),
                count(albums.data.as_ref().map(|d| d.len()))
            ),
        },
        Segment {
            value: "artists",
            label: format!(
                "{}{}",
                t.t("music:nav.artists"),
                count(artists.data.as_ref().map(|d| d.len()))
            ),
        },
    ];
    let kind = if tab() == FavTab::Artists {
        "round"
    } else {
        "square"
    };
    let min_w = if kind == "round" { 140 } else { 160 };
    let play_list = sorted.clone();
    let shuffle_list = sorted.clone();
    let (p1, p2) = (svc.music.clone(), svc.music.clone());

    rsx! {
        ShellPage {
            title: t.t("music:favorites.title"),
            subtitle: current.data.as_ref().map(|d| t.tc("music:library.count", d.len() as i64, &[])),
            gutter: true,
            actions: rsx! {
                SegmentedControl {
                    label: t.t("music:favorites.title"),
                    size: "small",
                    value: tab().id(),
                    options: segments,
                    on_change: {
                        let persist = persist.clone();
                        move |id: &'static str| {
                            let next = FavTab::from_id(id);
                            if !next.sorts().contains(&sort.peek().as_str()) {
                                sort.set("name".into());
                            }
                            tab.set(next);
                            persist(next, &sort.peek(), &order.peek());
                        }
                    },
                }
                IconButton {
                    icon: "arrow-up-down",
                    label: t.t("music:library.sort"),
                    onclick: {
                        let persist = persist.clone();
                        move |e: MouseEvent| {
                            let fields = tab()
                                .sorts()
                                .iter()
                                .map(|s| (s.to_string(), t.t(&format!("music:favorites.sort.{s}")), "Ascending".to_string(), *s == sort.peek().as_str()))
                                .collect();
                            let persist = persist.clone();
                            // Picking a field keeps the order; ↑ / ↓ keeps the field.
                            let on_pick = Callback::new(move |(id, ord): (String, String)| {
                                let is_field = tab.peek().sorts().contains(&id.as_str()) && id != *sort.peek();
                                if is_field {
                                    sort.set(id);
                                } else {
                                    order.set(ord);
                                }
                                persist(*tab.peek(), &sort.peek(), &order.peek());
                            });
                            let p = e.client_coordinates();
                            open_sort_menu(overlay, p.x, p.y, (sort.peek().clone(), order.peek().clone()), fields, on_pick);
                        }
                    },
                }
            },
            if current.is_loading() {
                if tab() == FavTab::Songs {
                    div { class: "jrm-center", Spinner { size: "large" } }
                } else {
                    GridSkeleton { kind }
                }
            } else if current.error.is_some() && current.data.is_none() {
                EmptyState { title: t.t("music:library.error") }
            } else if sorted.is_empty() {
                EmptyState {
                    icon: "heart",
                    title: t.t(&format!("music:favorites.empty.{}", tab().id())),
                    message: rsx! { {t.t("music:favorites.emptyHint")} },
                }
            } else if tab() == FavTab::Songs {
                div { class: "jrm-favorites__actions",
                    Button { icon: "play", icon_fill: true, onclick: move |_| p1.play(&play_list, Some(0), Some(false)), {t.t("music:favorites.playAll")} }
                    Button { variant: "tinted", icon: "shuffle", onclick: move |_| p2.play(&shuffle_list, None, Some(true)), {t.t("music:actions.shuffle")} }
                }
                SongRows { tracks: sorted, lead: "artwork", show_album: true, label: t.t("music:favorites.title") }
            } else {
                ul { class: "jr-grid jr-grid--{kind}", style: "--jr-grid-min: {min_w}px;", "aria-label": t.t("music:favorites.title"),
                    for item in sorted {
                        li { key: "{item.id}", class: "jr-grid__item",
                            div {
                                if tab() == FavTab::Albums { AlbumCard { album: item.clone() } } else { ArtistCard { artist: item.clone() } }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
const _: PageHeader = PageHeader::Large;
