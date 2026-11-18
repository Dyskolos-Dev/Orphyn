//! `pages/HomePage.tsx` + `HomeRows.tsx`: "Listen Now", the rows the user chose
//! (`features/home/layout.ts`: stored per server and user, recently played first by default).
//! The mix rows (`mixForYou`, `mixDaily`, `mixGenres`) belong to `features/mixes`.
use dioxus::prelude::*;
use orphyn_jellyfin::api::music::ArtistList;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};
use serde_json::Value;

use super::cards::{AlbumCard, ArtistCard, MediaCard, PlaylistCard};
use crate::design::{EmptyState, Shelf};
use crate::features::music::{art, track_artist, use_base_url};
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::shell::ShellPage;
use crate::state::{use_app, use_services};

const LAYOUT_VERSION: i64 = 2;

#[derive(Clone, PartialEq, Debug)]
struct HomeRow {
    id: String,
    kind: String,
    hidden: bool,
    param: Option<String>,
    label: Option<String>,
    size: Option<String>,
    count: Option<usize>,
}

/// `ROW_DEFS.music`: (type, unique, takes a param, default count).
const DEFS: &[(&str, bool, bool, usize)] = &[
    ("recentlyPlayed", true, false, 20),
    ("recentlyAdded", true, false, 20),
    ("favAlbums", true, false, 20),
    ("favArtists", true, false, 20),
    ("favSongs", true, false, 20),
    ("playlists", true, false, 20),
    ("instantMix", true, false, 20),
    ("genre", false, true, 20),
    ("artist", false, true, 20),
    ("randomAlbums", true, false, 20),
    ("mixForYou", true, false, 10),
    ("mixDaily", true, false, 10),
    ("mixGenres", true, false, 10),
];

fn row(kind: &str) -> HomeRow {
    HomeRow {
        id: kind.into(),
        kind: kind.into(),
        hidden: false,
        param: None,
        label: None,
        size: None,
        count: None,
    }
}

/// `defaultLayout('music')`.
fn default_rows() -> Vec<HomeRow> {
    [
        "recentlyPlayed",
        "mixForYou",
        "mixDaily",
        "recentlyAdded",
        "instantMix",
        "mixGenres",
        "favAlbums",
        "favArtists",
        "playlists",
    ]
    .iter()
    .map(|k| row(k))
    .collect()
}

/// `sanitizeLayout` + `migrateLayout`: the stored layout, or the default one.
fn parse_layout(json: Option<Value>) -> Vec<HomeRow> {
    let Some(raw) = json else {
        return default_rows();
    };
    let Some(list) = raw.get("rows").and_then(Value::as_array) else {
        return default_rows();
    };
    let mut rows: Vec<HomeRow> = Vec::new();
    for r in list {
        let (Some(id), Some(kind)) = (
            r.get("id").and_then(Value::as_str),
            r.get("type").and_then(Value::as_str),
        ) else {
            continue;
        };
        let Some(def) = DEFS.iter().find(|d| d.0 == kind) else {
            continue;
        };
        if rows.iter().any(|x| x.id == id) || (def.1 && rows.iter().any(|x| x.kind == kind)) {
            continue;
        }
        let param = r
            .get("param")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty());
        if def.2 && param.is_none() {
            continue;
        }
        rows.push(HomeRow {
            id: id.into(),
            kind: kind.into(),
            hidden: r.get("hidden").and_then(Value::as_bool) == Some(true),
            param: def.2.then(|| param.map(str::to_string)).flatten(),
            label: def
                .2
                .then(|| r.get("label").and_then(Value::as_str).map(str::to_string))
                .flatten(),
            size: r
                .get("size")
                .and_then(Value::as_str)
                .filter(|s| matches!(*s, "small" | "large"))
                .map(str::to_string),
            count: r
                .get("count")
                .and_then(Value::as_f64)
                .map(|n| (n.round() as usize).clamp(5, 60)),
        });
    }
    let version = raw.get("version").and_then(Value::as_i64).unwrap_or(0);
    if version < LAYOUT_VERSION {
        // Version 2 brings the music mixes.
        for (kind, after) in [
            ("mixForYou", "recentlyPlayed"),
            ("mixDaily", "mixForYou"),
            ("mixGenres", "instantMix"),
        ] {
            if rows.iter().any(|r| r.kind == kind) {
                continue;
            }
            let at = rows
                .iter()
                .rposition(|r| r.kind == after)
                .map(|i| i + 1)
                .unwrap_or(rows.len());
            rows.insert(at, row(kind));
        }
    }
    rows
}

fn count_of(r: &HomeRow) -> usize {
    r.count
        .or_else(|| DEFS.iter().find(|d| d.0 == r.kind).map(|d| d.3))
        .unwrap_or(20)
}

#[component]
pub fn MusicHomePage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let acc = session.account_id.clone();
    // Any album at all? Otherwise the library is empty and no row would show anything.
    let probe_params = ItemsParams {
        include_item_types: vec!["MusicAlbum".into()],
        page_size: Some(1),
        ..ItemsParams::default()
    };
    let probe = use_query(keys::items(&acc, &probe_params), stale::CATALOG, {
        let s = session.clone();
        move || {
            let (s, p) = (s.clone(), probe_params.clone());
            async move { s.items_page(&p, 0).await }
        }
    });
    let empty = probe.data.as_ref().is_some_and(|p| p.total == 0);

    let server_id = (app.accounts)()
        .iter()
        .find(|a| a.id == session.account_id)
        .map(|a| a.server_id.clone())
        .unwrap_or_default();
    let key = format!("orphyn.home.music.{server_id}.{}", session.user_id);
    let stored: Option<Value> = svc.storage.get_json(&key);
    let rows = parse_layout(stored);

    rsx! {
        ShellPage { title: t.t("music:home.title"),
            if empty {
                EmptyState { icon: "music", title: t.t("music:home.empty"), message: rsx! { {t.t("music:home.emptyHint")} } }
            }
            for r in rows.into_iter().filter(|r| !r.hidden) {
                MusicRow { key: "{r.id}", row: r.clone() }
            }
        }
    }
}

/// `MusicShelf`: nothing is drawn for an empty row; `size` shifts the shelf density.
#[component]
fn MusicShelf(
    row: HomeRow,
    title: String,
    subtitle: Option<String>,
    #[props(default = "square")] kind: &'static str,
    see_all_to: Option<String>,
    count: usize,
    children: Element,
) -> Element {
    if count == 0 {
        return rsx! {};
    }
    let shift = match row.size.as_deref() {
        Some("small") => Some(1),
        Some("large") => Some(-1),
        _ => None,
    };
    let shelf = rsx! {
        Shelf { title, subtitle, kind, see_all_to, {children} }
    };
    match shift {
        Some(s) => rsx! {
            div {
                class: "jr-home-frame",
                style: "--shelf-density: calc(var(--shelf-density-base, 0) + var(--density-items, 0) + {s});",
                {shelf}
            }
        },
        None => shelf,
    }
}

fn title_of(t: crate::i18n::I18n, row: &HomeRow) -> String {
    let name = row
        .label
        .clone()
        .or_else(|| row.param.clone())
        .unwrap_or_default();
    t.tf(
        &format!("home:rows.music.{}.title", row.kind),
        &[("name", name)],
    )
}

/// `useItems`/`useAlbums` as one query: the items of a `GET /Items` page.
fn use_items(params: ItemsParams, enabled: bool) -> Vec<BaseItemDto> {
    let app = use_app();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let q = use_query(keys::items(&acc, &params), stale::CATALOG, move || {
        let (s, p) = (session.clone(), params.clone());
        async move {
            match s {
                Some(s) if enabled => Ok(s.items_page(&p, 0).await?.items),
                _ => Ok(Vec::new()),
            }
        }
    });
    q.data.as_deref().cloned().unwrap_or_default()
}

fn albums_params(sort: &[&str], order: &str, count: usize) -> ItemsParams {
    ItemsParams {
        include_item_types: vec!["MusicAlbum".into()],
        sort_by: sort.iter().map(|s| s.to_string()).collect(),
        sort_order: (!order.is_empty()).then(|| order.to_string()),
        page_size: Some(count as u32),
        ..ItemsParams::default()
    }
}

/// Last played songs: "Recently Played" and the seed of the "Instant Mix".
fn recent_params(page_size: usize) -> ItemsParams {
    ItemsParams {
        include_item_types: vec!["Audio".into()],
        filters: vec!["IsPlayed".into()],
        sort_by: vec!["DatePlayed".into()],
        sort_order: Some("Descending".into()),
        page_size: Some(page_size as u32),
        ..ItemsParams::default()
    }
}

#[component]
fn AlbumShelf(row: HomeRow, albums: Vec<BaseItemDto>, see_all_to: Option<String>) -> Element {
    let t = use_i18n();
    rsx! {
        MusicShelf { row: row.clone(), title: title_of(t, &row), count: albums.len(), see_all_to,
            for a in albums {
                li { key: "{a.id}", class: "jr-shelf__item", AlbumCard { album: a.clone() } }
            }
        }
    }
}

#[component]
fn TrackShelf(
    row: HomeRow,
    tracks: Vec<BaseItemDto>,
    subtitle: Option<String>,
    see_all_to: Option<String>,
) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let base = use_base_url();
    rsx! {
        MusicShelf { row: row.clone(), title: title_of(t, &row), subtitle, count: tracks.len(), see_all_to,
            for (i, track) in tracks.iter().enumerate() {
                {
                    let (list, player) = (tracks.clone(), svc.music.clone());
                    rsx! {
                        li { key: "{track.id}", class: "jr-shelf__item",
                            MediaCard {
                                item: track.clone(),
                                title: track.name().to_string(),
                                subtitle: Some(track_artist(track)).filter(|a| !a.is_empty()),
                                image: art(&base, track, 320),
                                playable: true,
                                on_activate: move |_| player.play(&list, Some(i), Some(false)),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MusicRow(row: HomeRow) -> Element {
    let count = count_of(&row);
    match row.kind.as_str() {
        "recentlyPlayed" => rsx! { RecentlyPlayed { row, count } },
        "recentlyAdded" => rsx! { RecentlyAdded { row, count } },
        "favAlbums" => rsx! { FavAlbums { row, count } },
        "favArtists" => rsx! { FavArtists { row, count } },
        "favSongs" => rsx! { FavSongs { row, count } },
        "playlists" => rsx! { PlaylistsRow { row, count } },
        "instantMix" => rsx! { InstantMix { row, count } },
        "genre" => rsx! { GenreRow { row, count } },
        "artist" => rsx! { ArtistRow { row, count } },
        "randomAlbums" => rsx! { RandomAlbums { row, count } },
        // The mix rows come with features/mixes.
        _ => rsx! {},
    }
}

#[component]
fn RecentlyPlayed(row: HomeRow, count: usize) -> Element {
    let recent = use_items(recent_params((count * 2).max(40)), true);
    // Recently played songs, collapsed to the albums they belong to.
    let mut seen = std::collections::HashSet::new();
    let albums: Vec<BaseItemDto> = recent
        .iter()
        .filter_map(|track| {
            let album_id = track.album_id.clone()?;
            if !seen.insert(album_id.clone()) {
                return None;
            }
            let mut image_tags = std::collections::HashMap::new();
            if let Some(tag) = &track.album_primary_image_tag {
                image_tags.insert("Primary".to_string(), tag.clone());
            }
            Some(BaseItemDto {
                id: album_id,
                item_type: Some("MusicAlbum".into()),
                name: track.album.clone(),
                album_artist: track.album_artist.clone(),
                production_year: track.production_year,
                image_tags,
                ..Default::default()
            })
        })
        .take(count)
        .collect();
    rsx! { AlbumShelf { row, albums } }
}

#[component]
fn RecentlyAdded(row: HomeRow, count: usize) -> Element {
    let albums = use_items(albums_params(&["DateCreated"], "Descending", count), true);
    rsx! { AlbumShelf { row, albums, see_all_to: "/music/albums".to_string() } }
}

#[component]
fn FavAlbums(row: HomeRow, count: usize) -> Element {
    let mut p = albums_params(&[], "", count);
    p.is_favorite = Some(true);
    let albums = use_items(p, true);
    rsx! { AlbumShelf { row, albums, see_all_to: "/music/albums".to_string() } }
}

#[component]
fn FavArtists(row: HomeRow, count: usize) -> Element {
    let t = use_i18n();
    let app = use_app();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let params = ItemsParams {
        is_favorite: Some(true),
        page_size: Some(count as u32),
        ..ItemsParams::default()
    };
    let mut key = keys::items(&acc, &params);
    key.insert(2, "album-artists".into());
    let q = use_query(key, stale::CATALOG, move || {
        let (s, p) = (session.clone(), params.clone());
        async move {
            match s {
                Some(s) => Ok(s.artists_page(ArtistList::Album, &p, 0).await?.items),
                None => Ok(Vec::new()),
            }
        }
    });
    let artists: Vec<BaseItemDto> = q.data.as_deref().cloned().unwrap_or_default();
    rsx! {
        MusicShelf { row: row.clone(), title: title_of(t, &row), kind: "round", count: artists.len(), see_all_to: "/music/artists".to_string(),
            for a in artists {
                li { key: "{a.id}", class: "jr-shelf__item", ArtistCard { artist: a.clone() } }
            }
        }
    }
}

#[component]
fn FavSongs(row: HomeRow, count: usize) -> Element {
    let p = ItemsParams {
        include_item_types: vec!["Audio".into()],
        is_favorite: Some(true),
        sort_by: vec!["SortName".into()],
        sort_order: Some("Ascending".into()),
        page_size: Some(count as u32),
        ..ItemsParams::default()
    };
    let tracks = use_items(p, true);
    rsx! { TrackShelf { row, tracks, see_all_to: "/music/favorites".to_string() } }
}

#[component]
fn PlaylistsRow(row: HomeRow, count: usize) -> Element {
    let t = use_i18n();
    let p = ItemsParams {
        include_item_types: vec!["Playlist".into()],
        sort_by: vec!["SortName".into()],
        sort_order: Some("Ascending".into()),
        page_size: Some(count as u32),
        ..ItemsParams::default()
    };
    let playlists = use_items(p, true);
    rsx! {
        MusicShelf { row: row.clone(), title: title_of(t, &row), count: playlists.len(), see_all_to: "/music/playlists".to_string(),
            for p in playlists {
                li { key: "{p.id}", class: "jr-shelf__item", PlaylistCard { playlist: p.clone() } }
            }
        }
    }
}

/// Instant mix seeded by the last played song, else by the newest album.
#[component]
fn InstantMix(row: HomeRow, count: usize) -> Element {
    let t = use_i18n();
    let app = use_app();
    let last = use_items(recent_params(1), true);
    let newest = use_items(albums_params(&["DateCreated"], "Descending", 1), true);
    let seed = last.first().or(newest.first()).cloned();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let seed_id = seed.as_ref().map(|s| s.id.clone()).unwrap_or_default();
    let mut key = keys::instant_mix(&acc, &seed_id);
    key.push(count.to_string());
    let has_seed = seed.is_some();
    let mix = use_query(key, stale::ITEM, {
        let id = seed_id.clone();
        move || {
            let (s, id) = (session.clone(), id.clone());
            async move {
                match s {
                    Some(s) if has_seed => s.instant_mix(&id, count as u32).await,
                    _ => Ok(Vec::new()),
                }
            }
        }
    });
    let tracks: Vec<BaseItemDto> = if has_seed {
        mix.data.as_deref().cloned().unwrap_or_default()
    } else {
        Vec::new()
    };
    let subtitle = seed.map(|s| t.tf("home:instantMixFrom", &[("name", s.name().to_string())]));
    rsx! { TrackShelf { row, tracks, subtitle } }
}

#[component]
fn GenreRow(row: HomeRow, count: usize) -> Element {
    let mut p = albums_params(&["DateCreated"], "Descending", count);
    p.genres = row.param.iter().cloned().collect();
    let albums = use_items(p, row.param.is_some());
    rsx! { AlbumShelf { row, albums } }
}

#[component]
fn ArtistRow(row: HomeRow, count: usize) -> Element {
    let app = use_app();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let artist_id = row.param.clone().unwrap_or_default();
    let q = use_query(keys::artist_albums(&acc, &artist_id), stale::CATALOG, {
        let id = artist_id.clone();
        move || {
            let (s, id) = (session.clone(), id.clone());
            async move {
                match s {
                    Some(s) if !id.is_empty() => s.artist_albums(&id).await,
                    _ => Ok(Vec::new()),
                }
            }
        }
    });
    let albums: Vec<BaseItemDto> = q
        .data
        .as_deref()
        .map(|a| a.iter().take(count).cloned().collect())
        .unwrap_or_default();
    let see_all = row.param.as_ref().map(|p| format!("/music/artist/{p}"));
    rsx! { AlbumShelf { row, albums, see_all_to: see_all } }
}

#[component]
fn RandomAlbums(row: HomeRow, count: usize) -> Element {
    let albums = use_items(albums_params(&["Random"], "", count), true);
    rsx! { AlbumShelf { row, albums } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_layout_gives_the_default_rows() {
        let rows = parse_layout(None);
        assert_eq!(rows.len(), 9);
        assert_eq!(rows[0].kind, "recentlyPlayed");
    }

    #[test]
    fn old_layouts_get_the_mix_rows_after_their_anchors() {
        let json = serde_json::json!({
            "version": 1,
            "rows": [
                {"id": "recentlyPlayed", "type": "recentlyPlayed", "hidden": false},
                {"id": "instantMix", "type": "instantMix", "hidden": true},
                {"id": "g", "type": "genre", "hidden": false},
            ]
        });
        let rows = parse_layout(Some(json));
        let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "recentlyPlayed",
                "mixForYou",
                "mixDaily",
                "instantMix",
                "mixGenres"
            ]
        );
        assert!(rows[3].hidden);
    }

    #[test]
    fn counts_are_clamped_and_defaulted() {
        let json = serde_json::json!({"version": 2, "rows": [{"id": "a", "type": "recentlyAdded", "count": 500}]});
        let rows = parse_layout(Some(json));
        assert_eq!(count_of(&rows[0]), 60);
        assert_eq!(count_of(&row("favSongs")), 20);
    }
}
