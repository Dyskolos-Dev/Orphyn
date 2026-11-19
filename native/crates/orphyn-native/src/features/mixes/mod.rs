//! `features/mixes` of the React app: the daily mixes (For You 1-3, Discovery, On Repeat,
//! Rediscover, genre mixes, Top Songs of the year), their generated covers, the mix rows of the
//! music home and the mix page. The radios live in `features/music/panels/radio.rs`.
pub mod generate;

use std::time::Duration;

use dioxus::prelude::*;
use orphyn_core::storage::LocalStorage;
use orphyn_jellyfin::api::search::played_key;
use orphyn_jellyfin::endpoints::{Session, DEFAULT_FIELDS};
use orphyn_jellyfin::{ItemsParams, JfResult};

use crate::design::toast::use_toasts;
use crate::design::{Button, EmptyState, Spinner};
use crate::features::music::playlists::create_playlist;
use crate::features::music::rows::SongRows;
use crate::features::music::{format_total, track_art, use_base_url};
use crate::features::stats::aggregate::top_songs;
use crate::i18n::{use_i18n, I18n};
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::{PageHeader, ShellPage};
use crate::state::{use_app, use_services};
use generate::{generate_mixes, make_top_year_mix, MixKind, MixSource, Track};

pub use generate::Mix;

/// Generated once a day (the day cache answers repeat loads), never refetched on focus.
const STALE: Duration = Duration::from_secs(60 * 60);
const PLAYED_STALE: Duration = Duration::from_secs(10 * 60);

pub fn mix_path(id: &str) -> String {
    format!("/music?mix={}", crate::router::encode(id))
}

/// Home row types of the mixes and the kinds of mix each one shows (`MIX_ROWS`).
pub fn row_kinds(row_type: &str) -> &'static [MixKind] {
    match row_type {
        "mixForYou" => &[MixKind::ForYou],
        "mixDaily" => &[
            MixKind::Discovery,
            MixKind::OnRepeat,
            MixKind::Rediscover,
            MixKind::TopYear,
        ],
        "mixGenres" => &[MixKind::Genre],
        _ => &[],
    }
}

// ---------------------------------------------------------------- data

fn storage_key(account: &str, user: &str) -> String {
    format!("orphyn.mixes.{account}.{user}")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Stored {
    day: String,
    mixes: Vec<Mix>,
}

/// The mixes saved today, or `None` (another day, nothing saved, unreadable).
fn read_mixes(storage: &LocalStorage, key: &str, now: i64) -> Option<Vec<Mix>> {
    let stored: Stored = storage.get_json(key)?;
    (stored.day == generate::day_of(now)).then_some(stored.mixes)
}

fn write_mixes(storage: &LocalStorage, key: &str, mixes: &[Mix], now: i64) {
    storage.set_json(
        key,
        &Stored {
            day: generate::day_of(now),
            mixes: mixes.to_vec(),
        },
    );
}

/// Mix data read from the active server (`createMixSource`).
struct ServerSource(Session);

impl ServerSource {
    async fn songs(&self, params: ItemsParams, limit: u32) -> Vec<Track> {
        let mut fields: Vec<String> = DEFAULT_FIELDS.iter().map(|f| f.to_string()).collect();
        fields.push("Genres".into());
        let p = ItemsParams {
            include_item_types: vec!["Audio".into()],
            fields: Some(fields),
            page_size: Some(limit),
            ..params
        };
        self.0
            .items_page(&p, 0)
            .await
            .map(|p| p.items)
            .unwrap_or_default()
    }
}

impl MixSource for ServerSource {
    async fn signals(&self) -> (Vec<Track>, Vec<Track>) {
        tokio::join!(
            self.songs(
                ItemsParams {
                    filters: vec!["IsPlayed".into()],
                    sort_by: vec!["PlayCount".into()],
                    sort_order: Some("Descending".into()),
                    ..ItemsParams::default()
                },
                400
            ),
            self.songs(
                ItemsParams {
                    is_favorite: Some(true),
                    sort_by: vec!["DatePlayed".into()],
                    sort_order: Some("Ascending".into()),
                    ..ItemsParams::default()
                },
                300
            )
        )
    }

    async fn similar_to(&self, id: &str, limit: u32) -> Vec<Track> {
        self.0.instant_mix(id, limit).await.unwrap_or_default()
    }

    async fn discovery_pool(&self, artist_ids: &[String], limit: u32) -> Vec<Track> {
        let mut close: Vec<String> = Vec::new();
        for id in artist_ids.iter().take(4) {
            for a in self.0.similar(id, 6).await.unwrap_or_default() {
                if !a.id.is_empty() && !close.contains(&a.id) {
                    close.push(a.id);
                }
            }
        }
        for id in artist_ids {
            if !close.contains(id) {
                close.push(id.clone());
            }
        }
        let mut out = Vec::new();
        for id in close.into_iter().take(10) {
            out.extend(
                self.songs(
                    ItemsParams {
                        artist_ids: vec![id],
                        filters: vec!["IsUnplayed".into()],
                        sort_by: vec!["Random".into()],
                        ..ItemsParams::default()
                    },
                    limit.div_ceil(6),
                )
                .await,
            );
        }
        out
    }

    async fn genre_tracks(&self, genre: &str, limit: u32) -> Vec<Track> {
        self.songs(
            ItemsParams {
                genres: vec![genre.to_string()],
                sort_by: vec!["Random".into()],
                ..ItemsParams::default()
            },
            limit,
        )
        .await
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `useMixes`' query function: the daily mixes (cached for the day) plus this year's top songs.
async fn load(
    session: Session,
    cache: orphyn_jellyfin::QueryCache,
    storage: LocalStorage,
) -> JfResult<Vec<Mix>> {
    let now = now_ms();
    let key = storage_key(&session.account_id, &session.user_id);
    let daily = match read_mixes(&storage, &key, now) {
        Some(m) => m,
        None => {
            let m = generate_mixes(&ServerSource(session.clone()), now).await;
            write_mixes(&storage, &key, &m, now);
            m
        }
    };
    // `loadTopYearMix`: the played history is the one the Your year page reads.
    let year: i32 = generate::day_of(now)[..4].parse().unwrap_or(1970);
    let played = {
        let s = session.clone();
        cache
            .fetch(
                played_key(&session.account_id, &session.user_id),
                PLAYED_STALE,
                move || async move { s.played_items().await },
            )
            .await
    };
    let top = played
        .ok()
        .and_then(|items| make_top_year_mix(year, top_songs(&items, year, None, 50), now));
    Ok(daily.into_iter().chain(top).collect())
}

/// The user's mixes of the day (`None` while loading or without a session).
pub fn use_mixes() -> (bool, Vec<Mix>) {
    let app = use_app();
    let svc = use_services();
    let session = (app.session)();
    let (acc, user) = session
        .as_ref()
        .map(|s| (s.account_id.clone(), s.user_id.clone()))
        .unwrap_or_default();
    let key: Vec<String> = ["jf", acc.as_str(), "mixes", user.as_str()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let (cache, storage) = (svc.cache.clone(), svc.storage.clone());
    let state = use_query(key, STALE, move || {
        let (session, cache, storage) = (session.clone(), cache.clone(), storage.clone());
        async move {
            match session {
                Some(s) => load(s, cache, storage).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let loading = state.is_loading();
    (loading, state.data.as_deref().cloned().unwrap_or_default())
}

/// `useMixLabels`: display name and subtitle of a mix in the current language.
pub fn mix_labels(t: I18n, mix: &Mix) -> (String, String) {
    let name = match mix.kind {
        MixKind::ForYou => t.tf(
            "mixes:names.forYou",
            &[("n", mix.n.unwrap_or(1).to_string())],
        ),
        MixKind::Genre => t.tf(
            "mixes:names.genre",
            &[("genre", mix.genre.clone().unwrap_or_default())],
        ),
        MixKind::TopYear => t.tf(
            "mixes:names.topYear",
            &[("year", mix.year.unwrap_or_default().to_string())],
        ),
        k => t.t(&format!("mixes:names.{}", k.key())),
    };
    let subtitle = if mix.artists.is_empty() {
        t.t(&format!("mixes:hints.{}", mix.kind.key()))
    } else {
        t.tf(
            "mixes:with",
            &[(
                "artists",
                mix.artists[..mix.artists.len().min(3)].join(", "),
            )],
        )
    };
    (name, subtitle)
}

// ---------------------------------------------------------------- UI

/// Up to four different album covers of the mix.
fn covers_of(mix: &Mix, base: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in &mix.tracks {
        let Some(album) = t.album_id.clone().filter(|a| !a.is_empty()) else {
            continue;
        };
        let Some(url) = track_art(base, t, 240) else {
            continue;
        };
        if seen.insert(album) {
            out.push(url);
        }
        if out.len() == 4 {
            break;
        }
    }
    out
}

/// Generated cover: a hue gradient with a mosaic of album covers behind the name.
#[component]
pub fn MixCover(mix: Mix, name: String) -> Element {
    let base = use_base_url();
    let covers = covers_of(&mix, &base);
    let count = covers.len();
    rsx! {
        div { class: "jrx-cover", style: "--mix-hue: {mix.hue};", role: "img", "aria-label": "{name}",
            if count > 0 {
                div { class: "jrx-cover__mosaic", "data-count": "{count}", "aria-hidden": "true",
                    for src in covers {
                        img { key: "{src}", src: "{src}", alt: "", draggable: "false" }
                    }
                }
            }
            div { class: "jrx-cover__tint", "aria-hidden": "true" }
            div { class: "jrx-cover__text", "aria-hidden": "true",
                span { class: "jrx-cover__name", "{name}" }
            }
        }
    }
}

/// `MixCard`: a square card whose artwork is the generated cover.
#[component]
pub fn MixCard(mix: Mix) -> Element {
    let t = use_i18n();
    let router = use_router();
    let player = use_services().music;
    let (name, subtitle) = mix_labels(t, &mix);
    let to = mix_path(&mix.id);
    let tracks = mix.tracks.clone();
    let play_label = t.tf("shell:design.playItem", &[("title", name.clone())]);
    rsx! {
        li { class: "jr-shelf__item",
            div { class: "jr-card jr-card--square jr-card--has-actions", style: "--jr-card-ratio: 1 / 1;",
                a { class: "jr-card__hit", draggable: "false", onclick: move |_| router.push(&to),
                    div { class: "jr-card__art",
                        div { class: "jr-artwork jr-artwork--rounded jr-artwork--error jr-artwork--shadow", style: "aspect-ratio: 1 / 1;",
                            MixCover { mix: mix.clone(), name: name.clone() }
                        }
                    }
                    div { class: "jr-card__text",
                        span { class: "jr-card__title", "{name}" }
                        span { class: "jr-card__subtitle", "{subtitle}" }
                    }
                }
                div { class: "jr-card__actions",
                    crate::design::IconButton {
                        class: "jr-card__play",
                        variant: "glass",
                        icon: "play",
                        fill: true,
                        label: play_label,
                        onclick: move |_| player.play(&tracks, Some(0), Some(false)),
                    }
                }
            }
        }
    }
}

/// The mixes a home row of the given type shows (`MixRow`), at most `count`.
pub fn mixes_for_row(row_type: &str, count: usize) -> Vec<Mix> {
    let kinds = row_kinds(row_type);
    let (_, mixes) = use_mixes();
    mixes
        .into_iter()
        .filter(|m| kinds.contains(&m.kind))
        .take(count)
        .collect()
}

/// A mix, laid out like a playlist: cover, Play / Shuffle / Save as playlist, then the songs.
#[component]
pub fn MixPage(id: String) -> Element {
    let t = use_i18n();
    let router = use_router();
    let player = use_services().music;
    let toasts = use_toasts();
    let session = (use_app().session)();
    let (loading, mixes) = use_mixes();
    if loading {
        return rsx! {
            ShellPage { title: String::new(), header: PageHeader::Inline,
                div { class: "jrm-center", Spinner { size: "large" } }
            }
        };
    }
    let Some(mix) = mixes.into_iter().find(|m| m.id == id) else {
        return rsx! {
            ShellPage { title: t.t("mixes:title"), header: PageHeader::Inline,
                EmptyState { icon: "sparkles", title: t.t("mixes:notFound") }
            }
        };
    };
    let (name, subtitle) = mix_labels(t, &mix);
    let total: f64 = mix.tracks.iter().map(|s| s.duration_seconds()).sum();
    let footer = format!(
        "{} · {}",
        t.tc("music:album.footer", mix.tracks.len() as i64, &[]),
        format_total(t, total)
    );
    let (p1, p2) = (player.clone(), player);
    let (l1, l2) = (mix.tracks.clone(), mix.tracks.clone());
    let save = {
        let name = name.clone();
        let ids: Vec<String> = mix
            .tracks
            .iter()
            .map(|x| x.id.clone())
            .filter(|i| !i.is_empty())
            .collect();
        move |_| {
            let Some(session) = session.clone() else {
                return;
            };
            create_playlist(
                session,
                toasts,
                t,
                name.clone(),
                ids.clone(),
                move |created| {
                    if let Some(id) = created {
                        router.push(&format!("/music/playlist/{id}"));
                    }
                },
            );
        }
    };
    rsx! {
        ShellPage { title: name.clone(), header: PageHeader::Inline, gutter: true,
            header { class: "jrx-detail",
                div { class: "jrx-detail__art",
                    MixCover { mix: mix.clone(), name: name.clone() }
                }
                div { class: "jrx-detail__info",
                    h1 { class: "jr-text-title-1", "data-page-title": "", "{name}" }
                    p { class: "jr-text-title-3 jr-color-secondary", "{subtitle}" }
                    p { class: "jr-text-footnote jr-color-secondary", "{footer}" }
                    div { class: "jrx-detail__actions",
                        Button { icon: "play", icon_fill: true, onclick: move |_| p1.play(&l1, Some(0), Some(false)),
                            {t.t("music:actions.play")}
                        }
                        Button { variant: "tinted", icon: "shuffle", onclick: move |_| p2.play(&l2, None, Some(true)),
                            {t.t("music:actions.shuffle")}
                        }
                        Button { variant: "tinted", icon: "list-plus", onclick: save,
                            {t.t("mixes:saveAsPlaylist")}
                        }
                    }
                }
            }
            SongRows { tracks: mix.tracks.clone(), lead: "artwork", show_album: true, label: name.clone() }
        }
    }
}
