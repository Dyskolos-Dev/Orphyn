//! Year in review ("Ton année", `features/stats`): played items of the user aggregated into a
//! story (hero minutes, top artist, top songs, rankings, genres, films and series, fun facts).
pub mod aggregate;
mod reveal;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;
use orphyn_jellyfin::api::search::{played_key, plugin_key};
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::BaseItemDto;

use crate::design::{Button, EmptyState, Icon, Segment, SegmentedControl};
use crate::features::music::{track_art, use_base_url};
use crate::i18n::{use_i18n, I18n};
use crate::motion::{easings, use_tween};
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::ShellPage;
use crate::state::{use_app, use_services};
use aggregate::{
    aggregate_year, available_years, count_plugin_rows, days_from_civil, top_songs,
    year_of_secs, AggregateOptions, GenreEntry, PluginPlays, RankedEntry, YearStats,
};
use reveal::{format_number, CountUp, Reveal};

const PLAYED_STALE: std::time::Duration = std::time::Duration::from_secs(10 * 60);
const PLUGIN_STALE: std::time::Duration = std::time::Duration::from_secs(30 * 60);
const TOP_SONGS_LIMIT: usize = 50;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `SegmentedControl` values are `&'static str`: each year string is leaked once.
fn year_str(year: i32) -> &'static str {
    static CACHE: Mutex<Option<HashMap<i32, &'static str>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(HashMap::new)
        .entry(year)
        .or_insert_with(|| Box::leak(year.to_string().into_boxed_str()))
}

/// `isoDay` of a unix time.
fn iso_day(secs: i64) -> String {
    let (y, m, d) = aggregate::civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

#[component]
pub fn StatsPage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let mut picked: Signal<Option<i32>> = use_signal(|| None);
    let now = now_secs();
    let year = picked().unwrap_or_else(|| year_of_secs(now));

    let session = (app.session)();
    let acc = session.as_ref().map(|s| s.account_id.clone()).unwrap_or_default();
    let user = session.as_ref().map(|s| s.user_id.clone()).unwrap_or_default();

    // `usePlayedItems`: every played song, film and episode, cached for ten minutes.
    let played = use_query(played_key(&acc, &user), PLAYED_STALE, {
        let session = session.clone();
        move || {
            let session = session.clone();
            async move {
                match session {
                    Some(s) => s.played_items().await,
                    None => Ok(Vec::new()),
                }
            }
        }
    });
    // `usePluginPlays`: exact play counts of the Playback Reporting plugin, when installed.
    let plugin = use_query(plugin_key(&acc, &user, year), PLUGIN_STALE, {
        let session = session.clone();
        move || {
            let session = session.clone();
            async move {
                let Some(s) = session else { return Ok(None) };
                let now = now_secs();
                let end = if year == year_of_secs(now) {
                    now
                } else {
                    days_from_civil(year, 12, 31) * 86_400
                };
                let jan1 = days_from_civil(year, 1, 1) * 86_400;
                let days = ((end - jan1) as f64 / 86_400.0).ceil() as u32 + 1;
                let rows = s.plugin_playlist(days, &iso_day(end)).await;
                Ok(rows.as_ref().and_then(count_plugin_rows))
            }
        }
    });

    let items = played.data.clone();
    let plugin_loading = plugin.data.is_none() && plugin.error.is_none();
    let plugin_plays: Option<PluginPlays> = plugin.data.as_ref().and_then(|d| (**d).clone());
    let ready = items.is_some() && !plugin_loading;
    let is_loading = played.is_loading() || (items.is_some() && plugin_loading);
    let is_error = played.data.is_none() && played.error.is_some();

    let years = items.as_ref().map(|i| available_years(i, now)).unwrap_or_default();
    let stats: Option<YearStats> = items.as_ref().filter(|_| ready).map(|i| {
        aggregate_year(
            i,
            year,
            &AggregateOptions { plugin_plays: plugin_plays.as_ref(), now, limit: None },
        )
    });
    let top_tracks: Vec<BaseItemDto> = items
        .as_ref()
        .filter(|_| ready)
        .map(|i| top_songs(i, year, plugin_plays.as_ref(), TOP_SONGS_LIMIT))
        .unwrap_or_default();
    let exact = plugin_plays.is_some();

    let picker = (years.len() > 1).then(|| {
        let options: Vec<Segment> = years
            .iter()
            .take(5)
            .map(|y| Segment { value: year_str(*y), label: y.to_string() })
            .collect();
        rsx! {
            SegmentedControl {
                label: t.t("stats:year"),
                size: "small",
                value: year_str(year),
                on_change: move |v: &'static str| picked.set(v.parse().ok()),
                options,
            }
        }
    });

    let body = if is_loading {
        rsx! { Loading {} }
    } else if is_error || stats.is_none() {
        rsx! {
            EmptyState {
                icon: "music",
                title: t.t("stats:error.title"),
                message: rsx! { {t.t("stats:error.message")} },
            }
        }
    } else if let Some(st) = stats.as_ref().filter(|s| s.is_empty) {
        rsx! {
            EmptyState {
                icon: "sparkles",
                title: t.tf("stats:empty.title", &[("year", st.year.to_string())]),
                message: rsx! { {t.t("stats:empty.message")} },
            }
        }
    } else if let Some(stats) = stats {
        rsx! { Story { key: "{year}", stats, top_tracks, exact } }
    } else {
        rsx! {}
    };

    rsx! {
        ShellPage {
            title: t.t("stats:title"),
            subtitle: t.t("stats:subtitle"),
            header_accessory: picker,
            {body}
        }
    }
}

#[component]
fn Loading() -> Element {
    rsx! {
        div { class: "jrs", "aria-busy": "true",
            span { class: "jr-skeleton jr-skeleton--rect", "aria-hidden": "true", style: "height: 220px; border-radius: var(--radius-2xl);" }
            span { class: "jr-skeleton jr-skeleton--rect", "aria-hidden": "true", style: "height: 28px; width: 40%;" }
            for i in 0..4 {
                span { key: "{i}", class: "jr-skeleton jr-skeleton--rect", "aria-hidden": "true", style: "height: 64px; border-radius: var(--radius-lg);" }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Square,
    Poster,
}

/// `Art`: artwork of an item at `size` CSS pixels (requested at 2x), a cover or a 2:3 poster.
#[component]
fn Art(item: BaseItemDto, size: u32, #[props(default = false)] round: bool, shape: Shape) -> Element {
    let base = use_base_url();
    let width = size * 2;
    let src = build_image_url(
        &base,
        &item,
        ImageKind::Primary,
        &ImageOptions {
            max_width: Some(width),
            max_height: (shape == Shape::Square).then_some(width),
            quality: Some(92),
            tag: None,
        },
    );
    let mut cls = String::from("jrs-art");
    if round {
        cls.push_str(" jrs-art--round");
    }
    if shape == Shape::Poster {
        cls.push_str(" jrs-art--poster");
    }
    let fallback = if shape == Shape::Poster { "clapperboard" } else { "music" };
    rsx! {
        div { class: "{cls}",
            if let Some(src) = src {
                img { src, alt: "", draggable: "false" }
            } else {
                Icon { name: fallback, size: 24 }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Artist,
    Album,
    Movie,
    Series,
}

/// Detail page of an entry, from the app's own routes.
fn path_of(kind: Kind, id: &str) -> String {
    match kind {
        Kind::Artist => format!("/music/artist/{id}"),
        Kind::Album => format!("/music/album/{id}"),
        Kind::Movie | Kind::Series => format!("/video/item/{id}"),
    }
}

/// A link to the entry's page when it has one, else a plain block with the same look.
#[component]
fn EntryLink(kind: Kind, entry: RankedEntry, class: String, children: Element) -> Element {
    let router = use_router();
    match entry.target_id {
        None => rsx! { div { class: "{class}", {children} } },
        Some(id) => {
            let to = path_of(kind, &id);
            rsx! {
                a {
                    class: "{class} jrs-link",
                    draggable: "false",
                    onclick: move |_| router.push(&to),
                    {children}
                }
            }
        }
    }
}

#[component]
fn Section(icon: &'static str, title: String, children: Element) -> Element {
    rsx! {
        section { class: "jrs-section",
            Reveal {
                h2 { class: "jrs-section__title",
                    Icon { name: icon, size: 24 }
                    span { "{title}" }
                }
            }
            {children}
        }
    }
}

#[component]
fn Ranking(entries: Vec<RankedEntry>, kind: Kind) -> Element {
    let t = use_i18n();
    rsx! {
        ol { class: "jrs-rank",
            for (i, e) in entries.into_iter().enumerate() {
                Reveal { key: "{e.key}", delay: i as f64 * 0.06,
                    li {
                        EntryLink { kind, entry: e.clone(), class: "jrs-rank__row",
                            span { class: "jrs-rank__n", "{i + 1}" }
                            Art { item: e.art.clone().unwrap_or_else(|| e.item.clone()), size: 56, shape: Shape::Square }
                            span { class: "jrs-rank__text",
                                span { class: "jrs-rank__name", "{e.name}" }
                                if let Some(s) = &e.subtitle { span { class: "jrs-rank__sub", "{s}" } }
                            }
                            span { class: "jrs-rank__plays", {t.tc("stats:plays", e.plays, &[])} }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Posters(entries: Vec<RankedEntry>, kind: Kind, shape: Shape) -> Element {
    let cls = if shape == Shape::Poster { "jrs-posters jrs-posters--poster" } else { "jrs-posters" };
    rsx! {
        div { class: "{cls}",
            for (i, e) in entries.into_iter().enumerate() {
                Reveal { key: "{e.key}", delay: i as f64 * 0.07,
                    EntryLink { kind, entry: e.clone(), class: "jrs-poster",
                        Art { item: e.art.clone().unwrap_or_else(|| e.item.clone()), size: 260, shape }
                        span { class: "jrs-poster__caption",
                            span { class: "jrs-rank__name", "{e.name}" }
                            if let Some(s) = &e.subtitle { span { class: "jrs-rank__sub", "{s}" } }
                        }
                    }
                }
            }
        }
    }
}

/// One genre row: the bar grows from 0 to its share the first time the row is in view.
#[component]
fn GenreRow(genre: GenreEntry, max: f64, index: usize) -> Element {
    let (seen, mut el) = reveal::use_in_view(index as f64 * 0.08);
    let target = if max > 0.0 { genre.share / max } else { 0.0 };
    let p = use_tween(if seen() { target } else { 0.0 }, 0.9, easings::OUT);
    rsx! {
        li { onmounted: move |e| el.set(Some(e.data())),
            span { class: "jrs-genres__name", "{genre.name}" }
            span { class: "jrs-genres__track",
                span { class: "jrs-genres__bar", style: "transform: scaleX({p()});" }
            }
            span { class: "jrs-genres__pct", "{(genre.share * 100.0).round()}%" }
        }
    }
}

#[component]
fn Genres(genres: Vec<GenreEntry>) -> Element {
    let max = genres.first().map(|g| g.share).filter(|s| *s > 0.0).unwrap_or(1.0);
    rsx! {
        ul { class: "jrs-genres",
            for (i, g) in genres.into_iter().enumerate() {
                GenreRow { key: "{g.name}", genre: g, max, index: i }
            }
        }
    }
}

fn mix_covers(base: &str, tracks: &[BaseItemDto]) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for t in tracks {
        let Some(album) = t.album_id.as_deref().filter(|a| !a.is_empty()) else { continue };
        if seen.contains(&album) {
            continue;
        }
        let Some(url) = track_art(base, t, 240) else { continue };
        seen.push(album);
        out.push(url);
        if out.len() == 4 {
            break;
        }
    }
    out
}

/// The year's most played songs: play now, shuffle, or keep them as a Jellyfin playlist.
#[component]
fn TopSongs(year: i32, tracks: Vec<BaseItemDto>) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let base = use_base_url();
    let mut saving = use_signal(|| false);
    let title = t.tf("stats:topSongs.title", &[("year", year.to_string())]);
    let covers = mix_covers(&base, &tracks);
    let cover_count = covers.len();
    let (play_tracks, shuffle_tracks) = (tracks.clone(), tracks.clone());
    let (play_svc, shuffle_svc) = (svc.clone(), svc.clone());
    let save_title = title.clone();
    rsx! {
        Reveal { class: "jrs-top",
            div { class: "jrs-top__cover",
                div { class: "jrx-cover", style: "--mix-hue: 205;", role: "img", "aria-label": "{title}",
                    if !covers.is_empty() {
                        div { class: "jrx-cover__mosaic", "data-count": "{cover_count}", "aria-hidden": "true",
                            for src in covers.iter().cloned() {
                                img { key: "{src}", src, alt: "", draggable: "false" }
                            }
                        }
                    }
                    div { class: "jrx-cover__tint", "aria-hidden": "true" }
                    div { class: "jrx-cover__text", "aria-hidden": "true",
                        span { class: "jrx-cover__name", "{title}" }
                    }
                }
            }
            div { class: "jrs-top__text",
                p { class: "jrs-hero__eyebrow", {t.tc("stats:topSongs.line", tracks.len() as i64, &[("count", tracks.len().to_string())])} }
                h3 { class: "jrs-top__title", "{title}" }
                div { class: "jrs-top__actions",
                    Button {
                        icon: "play",
                        icon_fill: true,
                        onclick: move |_| play_svc.music.play(&play_tracks, Some(0), Some(false)),
                        {t.t("stats:topSongs.play")}
                    }
                    Button {
                        variant: "tinted",
                        icon: "shuffle",
                        onclick: move |_| shuffle_svc.music.play(&shuffle_tracks, None, Some(true)),
                        {t.t("stats:topSongs.shuffle")}
                    }
                    Button {
                        variant: "tinted",
                        icon: "list-plus",
                        disabled: saving(),
                        onclick: move |_| {
                            let Some(session) = app.session.peek().clone() else { return };
                            let ids: Vec<String> = tracks.iter().map(|x| x.id.clone()).filter(|i| !i.is_empty()).collect();
                            let name = save_title.clone();
                            saving.set(true);
                            spawn(async move {
                                if !ids.is_empty() {
                                    let _ = session.save_playlist(&name, &ids).await;
                                }
                                saving.set(false);
                            });
                        },
                        {t.t("stats:topSongs.save")}
                    }
                }
            }
        }
    }
}

fn month_name(t: I18n, month0: usize) -> String {
    const EN: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    const FR: [&str; 12] = [
        "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre",
        "octobre", "novembre", "décembre",
    ];
    let names = if t.lang().code() == "fr" { FR } else { EN };
    names[month0.min(11)].to_string()
}

#[component]
fn Story(stats: YearStats, top_tracks: Vec<BaseItemDto>, exact: bool) -> Element {
    let t = use_i18n();
    let lang = t.lang().code();
    let YearStats { year, music, video, facts, .. } = stats;
    let top_artist = music.artists.first().cloned();
    let has_video = video.movie_plays + video.episode_plays > 0;
    let month = facts.busiest_month.map(|m| month_name(t, m));
    let hours = (facts.total_minutes as f64 / 60.0).round() as i64;
    let share = (facts.top_artist_share * 100.0).round() as i64;
    let per_day = facts.minutes_per_day.round().max(1.0) as i64;
    rsx! {
        div { class: "jrs",
            Reveal { class: "jrs-hero",
                p { class: "jrs-hero__eyebrow", {t.tf("stats:hero.eyebrow", &[("year", year.to_string())])} }
                p { class: "jrs-hero__big",
                    CountUp { value: facts.total_minutes }
                    span { class: "jrs-hero__unit", {t.t("stats:hero.minutes")} }
                }
                p { class: "jrs-hero__line", {t.tf("stats:hero.line", &[("hours", format_number(hours, lang))])} }
            }

            if let Some(artist) = top_artist.clone() {
                Section { icon: "disc-3", title: t.t("stats:music.title"),
                    Reveal {
                        EntryLink { kind: Kind::Artist, entry: artist.clone(), class: "jrs-spotlight",
                            Art { item: artist.item.clone(), size: 240, round: true, shape: Shape::Square }
                            div { class: "jrs-spotlight__text",
                                p { class: "jrs-hero__eyebrow", {t.t("stats:music.topArtist")} }
                                p { class: "jrs-spotlight__name", "{artist.name}" }
                                p { class: "jrs-spotlight__line", {t.tc("stats:music.topArtistLine", artist.plays, &[])} }
                            }
                        }
                    }
                    if !top_tracks.is_empty() { TopSongs { year, tracks: top_tracks.clone() } }
                    div { class: "jrs-grid",
                        div {
                            h3 { class: "jrs-sub", {t.t("stats:music.artists")} }
                            Ranking { kind: Kind::Artist, entries: music.artists.clone() }
                        }
                        div {
                            h3 { class: "jrs-sub", {t.t("stats:music.songs")} }
                            Ranking { kind: Kind::Album, entries: music.songs.clone() }
                        }
                    }
                    h3 { class: "jrs-sub", {t.t("stats:music.albums")} }
                    Posters { kind: Kind::Album, shape: Shape::Square, entries: music.albums.clone() }
                    if !music.genres.is_empty() {
                        h3 { class: "jrs-sub", {t.t("stats:music.genres")} }
                        Genres { genres: music.genres.clone() }
                    }
                }
            }

            if has_video {
                Section { icon: "clapperboard", title: t.t("stats:video.title"),
                    Reveal { class: "jrs-stats",
                        div {
                            CountUp { value: video.movie_plays }
                            span { {t.tc("stats:video.movies", video.movie_plays, &[])} }
                        }
                        div {
                            CountUp { value: video.episode_plays }
                            span { {t.tc("stats:video.episodes", video.episode_plays, &[])} }
                        }
                    }
                    if !video.movies.is_empty() {
                        h3 { class: "jrs-sub", {t.t("stats:video.topMovies")} }
                        Posters { kind: Kind::Movie, shape: Shape::Poster, entries: video.movies.clone() }
                    }
                    if !video.series.is_empty() {
                        h3 { class: "jrs-sub",
                            Icon { name: "tv", size: 20 }
                            {" "}
                            {t.t("stats:video.topSeries")}
                        }
                        Posters { kind: Kind::Series, shape: Shape::Poster, entries: video.series.clone() }
                    }
                }
            }

            Section { icon: "sparkles", title: t.t("stats:facts.title"),
                div { class: "jrs-facts",
                    if let Some(month) = month {
                        Reveal { class: "jrs-fact",
                            strong { "{month}" }
                            span { {t.t("stats:facts.busiestMonth")} }
                        }
                    }
                    if music.plays > 0 {
                        Reveal { class: "jrs-fact", delay: 0.06,
                            strong { "{share}%" }
                            span { {t.tf("stats:facts.artistShare", &[("artist", top_artist.as_ref().map(|a| a.name.clone()).unwrap_or_default())])} }
                        }
                    }
                    Reveal { class: "jrs-fact", delay: 0.12,
                        strong { "{per_day}" }
                        span { {t.t("stats:facts.perDay")} }
                    }
                    if music.distinct_songs > 0 {
                        Reveal { class: "jrs-fact", delay: 0.18,
                            strong { {format_number(music.distinct_songs as i64, lang)} }
                            span { {t.t("stats:facts.distinctSongs")} }
                        }
                    }
                }
                if !exact { p { class: "jrs-note", {t.t("stats:approx")} } }
            }
        }
    }
}
