//! `pages/ArtistPage.tsx` and `pages/artist/*`: header, popular songs, discography, biography and
//! similar artists, plus `library/releaseType.ts`, `popularSongs.ts`, `similarArtists.ts`.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams, JfError};

use super::artist_groups::{artist_key, group_artists, use_artist_lists};
use super::cards::{artist_songs, track_menu, ArtistCard, MediaCard};
use crate::design::{
    use_overlay, Artwork, Button, EmptyState, IconButton, Shelf, Spinner, TrackRow,
};
use crate::features::music::rows::FavoriteAccessory;
use crate::features::music::{
    art, toggle_favorite, track_art, track_artist, track_artist_id, use_base_url, use_music,
};
use crate::i18n::{use_i18n, Lang};
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::{use_page, PageHeader, ShellPage};
use crate::state::{use_app, use_services};

const COLLAPSED: usize = 5;
const EXPANDED: usize = 10;
/// Releases with this many tracks or fewer are singles / EPs.
const MAX_SINGLE_TRACKS: i32 = 6;

// ------------------------------------------------------------------- releaseType.ts

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ReleaseType {
    Album,
    Ep,
    Compilation,
}

fn has_word(hay: &str, words: &[&str]) -> bool {
    let lower = hay.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .any(|w| words.contains(&w))
        || words
            .iter()
            .filter(|w| w.contains(' '))
            .any(|w| lower.contains(*w))
}

/// `classifyRelease`: the server exposes no release type, so title, credit and track count tell.
pub fn classify_release(album: &BaseItemDto) -> ReleaseType {
    let name = album.name();
    const COMPILATION: &[&str] = &[
        "compilation",
        "compilations",
        "best of",
        "greatest hits",
        "the very best",
        "anthology",
        "collection",
        "essential",
        "essentials",
        "hits",
        "retrospective",
        "rarities",
        "b-sides",
    ];
    let various = matches!(
        album.album_artist.as_deref().unwrap_or("").trim().to_lowercase().as_str(),
        "various artist" | "various artists" | "artiste varié" | "artistes variés" | "artistes varies" | "artiste varie" | "va"
    );
    if has_word(name, COMPILATION) || various {
        return ReleaseType::Compilation;
    }
    if has_word(name, &["single", "ep", "maxi"]) {
        return ReleaseType::Ep;
    }
    match album.child_count {
        Some(n) if n > 0 && n <= MAX_SINGLE_TRACKS => ReleaseType::Ep,
        _ => ReleaseType::Album,
    }
}

fn sort_newest_first(albums: &mut [BaseItemDto]) {
    albums.sort_by(|a, b| {
        b.production_year
            .unwrap_or(0)
            .cmp(&a.production_year.unwrap_or(0))
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
}

#[derive(Default, Clone, PartialEq)]
pub struct DiscographyData {
    pub albums: Vec<BaseItemDto>,
    pub eps: Vec<BaseItemDto>,
    pub compilations: Vec<BaseItemDto>,
    pub appears_on: Vec<BaseItemDto>,
}

/// `buildDiscography`: an album credited to one of `ids` is the artist's own, any other album only
/// holds a track of theirs.
pub fn build_discography(albums: &[BaseItemDto], ids: &[String]) -> DiscographyData {
    let (mut own, mut appears_on): (Vec<_>, Vec<_>) = albums.iter().cloned().partition(|a| {
        a.album_artists
            .iter()
            .any(|x| x.id.as_ref().is_some_and(|i| ids.contains(i)))
    });
    let by = |t: ReleaseType| {
        let mut v: Vec<BaseItemDto> = own.iter().filter(|a| classify_release(a) == t).cloned().collect();
        sort_newest_first(&mut v);
        v
    };
    let (albums, eps, compilations) = (by(ReleaseType::Album), by(ReleaseType::Ep), by(ReleaseType::Compilation));
    own.clear();
    sort_newest_first(&mut appears_on);
    DiscographyData {
        albums,
        eps,
        compilations,
        appears_on,
    }
}

// ------------------------------------------------------------------- popularSongs.ts

const NOISE: &[&str] = &[
    "remaster", "remastered", "remastering", "live", "version", "mono", "stereo", "radio edit",
    "single edit", "album version", "bonus track", "deluxe", "explicit", "clean", "acoustic version",
];

fn is_noise(s: &str) -> bool {
    let l = s.to_lowercase();
    NOISE.iter().any(|n| {
        l.match_indices(n).any(|(i, _)| {
            let before = l[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let after = l[i + n.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
            before && after
        })
    })
}

fn feat_at(s: &str) -> Option<usize> {
    let l = s.to_lowercase();
    if l.len() != s.len() {
        return None;
    }
    ["featuring", "feat", "ft"].iter().find_map(|w| {
        l.match_indices(w).find_map(|(i, _)| {
            let before = l[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let rest = l[i + w.len()..].trim_start_matches('.');
            let after = rest.chars().next().is_some_and(char::is_whitespace);
            (before && after).then(|| {
                let head = &s[..i];
                head.trim_end().trim_end_matches(['(', '[']).len()
            })
        })
    })
}

/// `normalizeTitle`: comparable form of a song title.
pub fn normalize_title(title: &str) -> String {
    // Bracketed groups holding a qualifier or a feature are dropped.
    let mut t = String::new();
    let mut rest = title;
    while let Some(open) = rest.find(['(', '[']) {
        let close = rest[open..].find([')', ']']).map(|c| open + c);
        let Some(close) = close else { break };
        let group = &rest[open..=close];
        t.push_str(&rest[..open]);
        if is_noise(group) || feat_at(group).is_some() {
            t.push(' ');
        } else {
            t.push_str(group);
        }
        rest = &rest[close + 1..];
    }
    t.push_str(rest);
    // "Song - Remastered 2011": a dash tail made of a qualifier.
    if let Some(i) = [" - ", " – ", " — "].iter().filter_map(|d| t.find(d).map(|i| (i, d.len()))).min() {
        if is_noise(&t[i.0 + i.1..]) {
            t.truncate(i.0);
        }
    }
    if let Some(i) = feat_at(&t) {
        t.truncate(i);
    }
    t.chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn plays(s: &BaseItemDto) -> i64 {
    s.user_data.as_ref().and_then(|u| u.play_count).unwrap_or(0)
}

/// `pickPopularSongs`: online-ranked songs first, then the library's own play count.
pub fn pick_popular_songs(songs: &[BaseItemDto], ranked: &[String], limit: usize) -> Vec<BaseItemDto> {
    let mut best: HashMap<String, &BaseItemDto> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for s in songs {
        let key = normalize_title(s.name());
        if key.is_empty() {
            continue;
        }
        match best.get(&key) {
            Some(cur) if plays(s) <= plays(cur) => {}
            Some(_) => {
                best.insert(key, s);
            }
            None => {
                order.push(key.clone());
                best.insert(key, s);
            }
        }
    }
    let mut out = Vec::new();
    let mut used = HashSet::new();
    for title in ranked {
        let key = normalize_title(title);
        if let Some(hit) = best.get(&key) {
            if used.insert(key) {
                out.push((*hit).clone());
            }
        }
    }
    let mut rest: Vec<&BaseItemDto> = order
        .iter()
        .filter(|k| !used.contains(*k))
        .filter_map(|k| best.get(k).copied())
        .collect();
    rest.sort_by_key(|s| std::cmp::Reverse(plays(s)));
    out.extend(rest.into_iter().cloned());
    out.truncate(limit);
    out
}

// ------------------------------------------------------------------- similarArtists.ts

fn merge_similar(
    from_server: &[BaseItemDto],
    related: &[String],
    library: &[BaseItemDto],
    self_ids: &[String],
    self_name: &str,
) -> Vec<BaseItemDto> {
    let self_key = artist_key(self_name);
    let mut out: Vec<BaseItemDto> = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |a: &BaseItemDto| {
        let key = artist_key(a.name());
        if a.id.is_empty() || key.is_empty() || key == self_key || self_ids.contains(&a.id) || !seen.insert(key) {
            return;
        }
        out.push(a.clone());
    };
    for a in from_server {
        if a.item_type.as_deref().is_none_or(|t| t == "MusicArtist") {
            add(a);
        }
    }
    let mut by_key: HashMap<String, &BaseItemDto> = HashMap::new();
    for a in library {
        let k = artist_key(a.name());
        if !k.is_empty() {
            by_key.entry(k).or_insert(a);
        }
    }
    for name in related {
        if let Some(hit) = by_key.get(&artist_key(name)) {
            add(hit);
        }
    }
    out.truncate(16);
    out
}

/// `cleanOverview`: server biographies may hold HTML or BBCode.
fn clean_overview(raw: Option<&str>) -> String {
    let s = raw.unwrap_or("").replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

// ------------------------------------------------------------------- the page

type Names = Result<Vec<String>, JfError>;

fn net<T>(r: anyhow_like::Res<T>) -> Result<T, JfError> {
    r.map_err(|e| JfError::Network(e.to_string()))
}

mod anyhow_like {
    pub type Res<T> = Result<T, orphyn_core::error::Error>;
}

#[component]
pub fn ArtistPage(id: String) -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let acc = session.account_id.clone();
    let prefs = (app.preferences)();
    let grouping = prefs.group_artists;
    let online = prefs.artist_images;

    let artist = use_query(keys::item(&acc, &id), stale::ITEM, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.item(&id).await }
        }
    });
    let lists = use_artist_lists(&session, grouping || online);
    let ids: Vec<String> = match (&lists.album_artists, &lists.everyone) {
        (Some(a), Some(e)) if grouping => group_artists(a, e)
            .into_iter()
            .find(|g| g.ids.contains(&id))
            .map(|g| g.ids)
            .unwrap_or_else(|| vec![id.clone()]),
        _ => vec![id.clone()],
    };

    let album_params = ItemsParams {
        include_item_types: vec!["MusicAlbum".into()],
        artist_ids: ids.clone(),
        sort_by: vec!["ProductionYear".into(), "SortName".into()],
        sort_order: Some("Descending".into()),
        fields: Some(
            ["PrimaryImageAspectRatio", "Genres", "ProductionYear", "DateCreated", "ChildCount"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        page_size: Some(200),
        ..ItemsParams::default()
    };
    let albums = use_query(keys::items(&acc, &album_params), stale::CATALOG, {
        let s = session.clone();
        move || {
            let (s, p) = (s.clone(), album_params.clone());
            async move { Ok(s.items_page(&p, 0).await?.items) }
        }
    });
    let song_params = ItemsParams {
        include_item_types: vec!["Audio".into()],
        artist_ids: ids.clone(),
        sort_by: vec!["Album".into(), "ParentIndexNumber".into(), "IndexNumber".into(), "SortName".into()],
        page_size: Some(1000),
        ..ItemsParams::default()
    };
    let songs_q = use_query(keys::items(&acc, &song_params), stale::CATALOG, {
        let s = session.clone();
        move || {
            let (s, p) = (s.clone(), song_params.clone());
            async move { Ok(s.items_page(&p, 0).await?.items) }
        }
    });
    let similar = use_query(keys::similar(&acc, &id), stale::CATALOG, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.similar(&id, 12).await }
        }
    });

    let name = artist.data.as_ref().map(|a| a.name().to_string()).unwrap_or_default();
    let overview = clean_overview(artist.data.as_ref().and_then(|a| a.overview.as_deref()));
    let lang = if t.lang() == Lang::Fr { "fr" } else { "en" };
    let key = artist_key(&name);
    let online_on = online && !name.is_empty();
    let forever = Duration::from_secs(3600 * 24 * 30);
    let top = use_query(vec!["artistInfo".into(), "top".into(), key.clone()], forever, {
        let name = name.clone();
        move || {
            let name = name.clone();
            async move {
                if !online_on {
                    return Names::Ok(Vec::new());
                }
                net(orphyn_core::artwork::artwork_artist_top_tracks(name).await)
            }
        }
    });
    let related = use_query(vec!["artistInfo".into(), "related".into(), key.clone()], forever, {
        let name = name.clone();
        move || {
            let name = name.clone();
            async move {
                if !online_on {
                    return Names::Ok(Vec::new());
                }
                net(orphyn_core::artwork::artwork_related_artists(name).await)
            }
        }
    });
    let want_bio = online_on && overview.is_empty() && artist.data.is_some();
    let online_bio = use_query(vec!["artistInfo".into(), "bio".into(), lang.into(), key.clone()], forever, {
        let name = name.clone();
        move || {
            let name = name.clone();
            async move {
                if !want_bio {
                    return Ok(None::<String>);
                }
                net(orphyn_core::artwork::artwork_artist_bio(name, lang.to_string()).await)
            }
        }
    });

    if artist.is_loading() {
        return rsx! {
            ShellPage { title: "", header: PageHeader::Inline,
                div { class: "jrm-center", Spinner { size: "large" } }
            }
        };
    }
    let Some(item) = artist.data.clone().map(|a| (*a).clone()) else {
        return rsx! {
            ShellPage { title: t.t("music:nav.artists"), header: PageHeader::Inline,
                EmptyState { icon: "user", title: t.t("music:artist.notFound") }
            }
        };
    };

    let songs: Vec<BaseItemDto> = songs_q.data.as_deref().cloned().unwrap_or_default();
    let discography = build_discography(albums.data.as_deref().map(Vec::as_slice).unwrap_or(&[]), &ids);
    let library: Vec<BaseItemDto> = lists
        .album_artists
        .iter()
        .chain(lists.everyone.iter())
        .flat_map(|l| l.iter().cloned())
        .collect();
    let related_names: Vec<String> = related.data.as_deref().cloned().unwrap_or_default();
    let similar_artists = merge_similar(
        similar.data.as_deref().map(Vec::as_slice).unwrap_or(&[]),
        &related_names,
        &library,
        &ids,
        &name,
    );
    let ranking: Vec<String> = top.data.as_deref().cloned().unwrap_or_default();
    let popular_loading = songs_q.is_loading() || (online && top.is_loading());
    let bio: Option<(String, bool)> = if !overview.is_empty() {
        Some((overview.clone(), true))
    } else {
        online_bio.data.as_deref().cloned().flatten().filter(|b| !b.is_empty()).map(|b| (b, false))
    };
    let bio_loading = overview.is_empty() && want_bio && online_bio.is_loading();
    let similar_loading = similar.is_loading() || (online && related.is_loading());

    let play_all = {
        let (svc, session, songs, id) = (svc.clone(), session.clone(), songs.clone(), id.clone());
        move |shuffle: bool| {
            if !songs.is_empty() {
                svc.music.play(&songs, (!shuffle).then_some(0), Some(shuffle));
            } else {
                let (svc, session, id) = (svc.clone(), session.clone(), id.clone());
                spawn(async move {
                    if let Ok(list) = artist_songs(&session, &id, 500).await {
                        svc.music.play(&list, (!shuffle).then_some(0), Some(shuffle));
                    }
                });
            }
        }
    };
    let (pa, pb) = (play_all.clone(), play_all);

    rsx! {
        ShellPage { title: name.clone(), header: PageHeader::Inline, gutter: true,
            ArtistHeader { artist: item.clone(), on_play: move |_| pa(false), on_shuffle: move |_| pb(true) }
            PopularSongs { songs, ranking, loading: popular_loading }
            Discography { data: discography }
            ArtistAbout { bio, loading: bio_loading }
            SimilarArtists { artists: similar_artists, loading: similar_loading }
        }
    }
}

#[component]
fn ArtistHeader(artist: BaseItemDto, on_play: EventHandler<()>, on_shuffle: EventHandler<()>) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let page = use_page();
    let base = use_base_url();
    let prefs = (app.preferences)();
    let favorite = artist.is_favorite();
    let jf_image = art(&base, &artist, 480);
    // The online picture is only looked up when the server has none (`useArtistImage`).
    let name = artist.name().to_string();
    let online = prefs.artist_images && jf_image.is_none();
    let key = artist_key(&name);
    let found = use_query(vec!["artistInfo".into(), "image".into(), key], Duration::from_secs(3600 * 24 * 30), {
        let name = name.clone();
        move || {
            let name = name.clone();
            async move {
                if !online {
                    return Ok(None::<String>);
                }
                net(orphyn_core::artwork::artwork_artist_image(name).await)
            }
        }
    });
    let src = jf_image.or_else(|| found.data.as_deref().cloned().flatten());
    let fav_item = artist.clone();
    let radio_item = artist.id.clone();
    let radio_session = app.session;
    let radio_svc = svc.clone();
    rsx! {
        header { class: "jrm-detail jrm-artist__header",
            Artwork { src, shape: "circle", shadow: true, fallback: "user", class: "jrm-detail__art jrm-artist__photo" }
            div { class: "jrm-detail__info",
                p { class: "jr-text-callout jr-color-secondary", {t.t("music:artist.eyebrow")} }
                h1 {
                    class: "jr-text-title-1 jr-color-primary jrm-detail__title",
                    "data-page-title": "",
                    onmounted: move |e| page.title_el.clone().set(Some(e.data())),
                    "{name}"
                }
                if !artist.genres.is_empty() {
                    p { class: "jr-text-footnote jr-color-secondary jrm-detail__meta", {artist.genres.iter().take(3).cloned().collect::<Vec<_>>().join(" · ")} }
                }
                div { class: "jrm-detail__actions",
                    Button { icon: "play", icon_fill: true, onclick: move |_| on_play.call(()), {t.t("music:actions.play")} }
                    Button { variant: "tinted", icon: "shuffle", onclick: move |_| on_shuffle.call(()), {t.t("music:actions.shuffle")} }
                    Button {
                        variant: "tinted",
                        icon: "radio",
                        onclick: move |_| {
                            // startRadio: an instant mix seeded by the artist.
                            let Some(s) = radio_session.peek().clone() else { return };
                            let (svc, id) = (radio_svc.clone(), radio_item.clone());
                            spawn(async move {
                                if let Ok(list) = s.instant_mix(&id, 50).await {
                                    if !list.is_empty() {
                                        svc.music.play(&list, Some(0), Some(false));
                                    }
                                }
                            });
                        },
                        {t.t("mixes:radio.start")}
                    }
                    IconButton {
                        icon: "heart",
                        fill: favorite,
                        label: if favorite { t.t("music:actions.unfavorite") } else { t.t("music:actions.favorite") },
                        variant: "gray",
                        pressed: favorite,
                        onclick: move |_| toggle_favorite(&svc, app.session.peek().clone(), &fav_item, !favorite),
                    }
                }
            }
        }
    }
}

#[component]
fn PopularSongs(songs: Vec<BaseItemDto>, ranking: Vec<String>, loading: bool) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let router = use_router();
    let overlay = use_overlay();
    let music = use_music();
    let base = use_base_url();
    let mut expanded = use_signal(|| false);
    let top = pick_popular_songs(&songs, &ranking, EXPANDED);
    let shown: Vec<BaseItemDto> = if expanded() { top.clone() } else { top.iter().take(COLLAPSED).cloned().collect() };
    if loading {
        return rsx! {
            section { class: "jrm-section jrm-artist__section", "aria-busy": "true",
                h2 { class: "jr-text-title-2 jr-color-primary jrm-section__title", {t.t("music:artist.topSongs")} }
                for i in 0..COLLAPSED {
                    div { key: "{i}", class: "jr-skeleton", style: "height: 52px; border-radius: var(--radius-md);" }
                }
            }
        };
    }
    if top.is_empty() {
        return rsx! {};
    }
    let state = (music.state)();
    let current_id = state.current().map(|q| q.track.id.clone());
    let playing = state.is_playing();
    rsx! {
        section { class: "jrm-section jrm-artist__section",
            h2 { class: "jr-text-title-2 jr-color-primary jrm-section__title", {t.t("music:artist.topSongs")} }
            ol { class: "jr-track-list jrm-tracklist", "aria-label": t.t("music:artist.topSongs"),
                for (i, track) in shown.iter().enumerate() {
                    {
                        let track = track.clone();
                        let artist = track_artist(&track);
                        let artist_id = track_artist_id(&track);
                        let count = plays(&track);
                        let is_current = current_id.as_deref() == Some(track.id.as_str());
                        let title = track.name().to_string();
                        let (list, p_play, p_pause, p_menu) = (top.clone(), svc.music.clone(), svc.music.clone(), svc.music.clone());
                        let more = track.clone();
                        let artist_el = match (artist.is_empty(), artist_id) {
                            (false, Some(aid)) => Some(rsx! {
                                a { class: "jrm-link", onclick: move |e| { e.stop_propagation(); router.push(&format!("/music/artist/{aid}")) }, "{artist}" }
                            }),
                            _ => None,
                        };
                        rsx! {
                            TrackRow {
                                key: "{track.id}",
                                index: i as i32 + 1,
                                artwork: track_art(&base, &track, 80),
                                title: title.clone(),
                                artist: artist_el,
                                album: track.album.clone(),
                                duration: track.duration_seconds(),
                                is_current,
                                is_playing: playing,
                                play_label: if is_current && playing { t.t("shell:design.nowPlaying") } else { t.tf("shell:design.playItem", &[("title", title.clone())]) },
                                more_label: t.tf("shell:design.moreFor", &[("title", title.clone())]),
                                accessory: rsx! {
                                    if count > 0 {
                                        span { class: "jr-text-footnote jr-color-secondary jrm-artist__plays", {t.tc("music:artist.plays", count, &[])} }
                                    }
                                    FavoriteAccessory { track: track.clone() }
                                },
                                on_play: move |_| {
                                    if is_current && !playing { p_play.resume() } else { p_play.play(&list, Some(i), Some(false)) }
                                },
                                on_pause: move |_| p_pause.pause(),
                                on_more: move |e: MouseEvent| {
                                    let p = e.client_coordinates();
                                    overlay.menu(p.x, p.y, track_menu(t, &p_menu, router, more.clone()));
                                },
                            }
                        }
                    }
                }
            }
            if top.len() > COLLAPSED {
                Button { variant: "plain", onclick: move |_| expanded.set(!expanded()),
                    if expanded() { {t.t("music:artist.showLess")} } else { {t.t("music:artist.showMore")} }
                }
            }
        }
    }
}

#[component]
fn Discography(data: DiscographyData) -> Element {
    let t = use_i18n();
    let base = use_base_url();
    let groups = [
        (t.t("music:artist.albums"), data.albums, false),
        (t.t("music:artist.singles"), data.eps, false),
        (t.t("music:artist.compilations"), data.compilations, false),
        (t.t("music:artist.appearsOn"), data.appears_on, true),
    ];
    rsx! {
        for (title, list, show_artist) in groups {
            if !list.is_empty() {
                div { key: "{title}", class: "jrm-bleed jrm-artist__shelf",
                    Shelf { title: title.clone(), kind: "square",
                        for a in list {
                            {
                                let subtitle = [if show_artist { a.album_artist.clone() } else { None }, a.production_year.map(|y| y.to_string())]
                                    .into_iter().flatten().collect::<Vec<_>>().join(" · ");
                                rsx! {
                                    li { key: "{a.id}", class: "jr-shelf__item",
                                        MediaCard {
                                            title: a.name().to_string(),
                                            subtitle: (!subtitle.is_empty()).then_some(subtitle),
                                            image: art(&base, &a, 320),
                                            fallback: "disc-3",
                                            to: format!("/music/album/{}", a.id),
                                            playable: true,
                                            item: a.clone(),
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

#[component]
fn ArtistAbout(bio: Option<(String, bool)>, loading: bool) -> Element {
    let t = use_i18n();
    let mut open = use_signal(|| false);
    if loading {
        return rsx! {
            section { class: "jrm-section jrm-artist__section", "aria-busy": "true",
                h2 { class: "jr-text-title-2 jr-color-primary jrm-section__title", {t.t("music:artist.about")} }
                div { class: "jr-skeleton", style: "height: 72px;" }
            }
        };
    }
    let Some((text, from_server)) = bio else {
        return rsx! {};
    };
    // Blitz has no -webkit-line-clamp: the clamped text is cut by length (4 lines ≈ 360 chars).
    let long = text.chars().count() > 360;
    let shown: String = if open() || !long { text.clone() } else { format!("{}…", text.chars().take(360).collect::<String>().trim_end()) };
    rsx! {
        section { class: "jrm-section jrm-artist__section",
            h2 { class: "jr-text-title-2 jr-color-primary jrm-section__title", {t.t("music:artist.about")} }
            div { class: "jrm-artist__bio",
                div { class: "jrm-artist__bio-text",
                    p { class: "jr-text-callout jr-color-secondary", "{shown}" }
                }
                if long {
                    Button { variant: "plain", size: "small", onclick: move |_| open.set(!open()),
                        if open() { {t.t("music:artist.less")} } else { {t.t("music:artist.more")} }
                    }
                }
                if !from_server {
                    p { class: "jr-text-caption jr-color-tertiary jrm-artist__source", {t.t("music:artist.source.audiodb")} }
                }
            }
        }
    }
}

#[component]
fn SimilarArtists(artists: Vec<BaseItemDto>, loading: bool) -> Element {
    let t = use_i18n();
    if loading {
        return rsx! {
            div { class: "jrm-bleed jrm-artist__shelf", "aria-busy": "true",
                Shelf { title: t.t("music:artist.similar"), kind: "round",
                    for i in 0..8 {
                        li { key: "{i}", class: "jr-shelf__item",
                            div { class: "jr-skeleton", style: "aspect-ratio: 1 / 1; border-radius: 50%;" }
                            div { class: "jr-skeleton jr-skeleton--text", style: "height: 12px; margin-top: 8px;" }
                        }
                    }
                }
            }
        };
    }
    if artists.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "jrm-bleed jrm-artist__shelf",
            Shelf { title: t.t("music:artist.similar"), kind: "round",
                for a in artists {
                    li { key: "{a.id}", class: "jr-shelf__item", ArtistCard { artist: a.clone() } }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(_: Arc<()>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(name: &str, artist: &str, count: Option<i32>) -> BaseItemDto {
        BaseItemDto {
            name: Some(name.into()),
            album_artist: Some(artist.into()),
            child_count: count,
            ..Default::default()
        }
    }

    #[test]
    fn releases_are_classified() {
        assert_eq!(classify_release(&album("Greatest Hits", "X", Some(14))), ReleaseType::Compilation);
        assert_eq!(classify_release(&album("Whatever", "Various Artists", Some(14))), ReleaseType::Compilation);
        assert_eq!(classify_release(&album("Thing (EP)", "X", Some(9))), ReleaseType::Ep);
        assert_eq!(classify_release(&album("Short", "X", Some(4))), ReleaseType::Ep);
        assert_eq!(classify_release(&album("Long", "X", Some(12))), ReleaseType::Album);
    }

    #[test]
    fn titles_are_normalised() {
        assert_eq!(normalize_title("Song (Remastered 2011)"), "song");
        assert_eq!(normalize_title("Song - Live at Wembley"), "song");
        assert_eq!(normalize_title("Song (feat. Someone)"), "song");
        assert_eq!(normalize_title("Café Del Mar"), "cafedelmar");
    }

    #[test]
    fn popular_songs_follow_the_ranking_then_plays() {
        let mk = |n: &str, p: i64| BaseItemDto {
            id: n.into(),
            name: Some(n.into()),
            user_data: Some(orphyn_jellyfin::types::UserItemData { play_count: Some(p), ..Default::default() }),
            ..Default::default()
        };
        let songs = [mk("A", 1), mk("B", 9), mk("C", 5)];
        let out = pick_popular_songs(&songs, &["C".into()], 10);
        assert_eq!(out.iter().map(|s| s.name()).collect::<Vec<_>>(), ["C", "B", "A"]);
    }
}
