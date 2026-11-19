//! Global search (`features/search`): `/search?q=`, a debounced search across the active server
//! grouped by type. The sidebar field that drives it lives in the shell.
mod fold;
mod grouping;
mod recents;

// The palette lives in `features/palette`; until `features/mod.rs` declares it (`pub mod palette;`)
// it is compiled from here. Remove this line when that declaration is added.
#[path = "../palette/mod.rs"]
pub mod palette;

use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::api::search::{self, SearchResults};
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::keys::{stale, Key};
use orphyn_jellyfin::QueryState;
use orphyn_jellyfin::BaseItemDto;

use crate::design::{Artwork, Button, EmptyState, Icon, TrackRow};
use crate::features::music::use_base_url;
use crate::i18n::{use_i18n, I18n};
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::ShellPage;
use crate::state::{use_app, use_services};

pub use fold::normalize;
pub use grouping::{group_results, ResultKind};
use grouping::{route_for, ResultSection};

const DEBOUNCE_MS: u64 = 250;

/// `imageUrl(item, kind, { maxWidth })`, at the 2x scale of a typical screen.
fn image(base: &str, item: &BaseItemDto, kind: ImageKind, max_width: u32) -> Option<String> {
    build_image_url(
        base,
        item,
        kind,
        &ImageOptions {
            max_width: Some(max_width * 2),
            max_height: None,
            quality: Some(80),
            tag: None,
        },
    )
}

/// First letters of the first and last word (`initials` of `Artwork.tsx`).
fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let first = parts.first().and_then(|p| p.chars().next());
    let last = if parts.len() > 1 {
        parts.last().and_then(|p| p.chars().next())
    } else {
        None
    };
    first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect()
}

fn item_subtitle(kind: ResultKind, item: &BaseItemDto) -> Option<String> {
    let joined = |parts: Vec<String>| {
        let s = parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        (!s.is_empty()).then_some(s)
    };
    match kind {
        ResultKind::Movie | ResultKind::Series => item.production_year.map(|y| y.to_string()),
        ResultKind::Episode => item.series_name.clone(),
        ResultKind::Album => joined(vec![
            item.album_artist.clone().unwrap_or_default(),
            item.production_year.map(|y| y.to_string()).unwrap_or_default(),
        ]),
        ResultKind::Song => joined(vec![
            item.album_artist
                .clone()
                .or_else(|| item.artists.first().cloned())
                .unwrap_or_default(),
            item.album.clone().unwrap_or_default(),
        ]),
        ResultKind::Artist => None,
    }
}

/// `useSearch`: the grouped search of `term` (disabled below 2 characters). Also returns the cache
/// key, to refetch it.
pub fn use_search(term: &str) -> (QueryState<SearchResults>, Key) {
    let app = use_app();
    let enabled = term.chars().count() >= 2;
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let mut key = search::key(&acc, term);
    if !enabled {
        key.push("off".into());
    }
    let term = term.to_string();
    let query = use_query(key.clone(), stale::SEARCH, move || {
        let (session, term) = (session.clone().filter(|_| enabled), term.clone());
        async move {
            match session {
                Some(s) => s.search(&term, search::LIMIT_PER_TYPE).await,
                None => Ok(SearchResults::default()),
            }
        }
    });
    (query, key)
}

/// `/search?q=`.
#[component]
pub fn SearchPage(q: String) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let router = use_router();
    let trimmed = q.trim().to_string();

    // `useDebounced`: the term follows the field 250 ms after the last keystroke.
    let mut term = use_signal(|| trimmed.clone());
    let mut generation = use_signal(|| 0u64);
    use_effect(use_reactive!(|trimmed| {
        let g = *generation.peek() + 1;
        generation.set(g);
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
            if *generation.peek() == g {
                term.set(trimmed);
            }
        });
    }));
    let term_now = term();
    let (query, key) = use_search(&term_now);

    let recents = use_signal({
        let storage = svc.storage.clone();
        move || recents::read(&storage)
    });
    let remember = {
        let storage = svc.storage.clone();
        let term_now = term_now.clone();
        let mut recents = recents;
        EventHandler::new(move |_: ()| {
            let current = recents.peek().clone();
            if let Some(next) = recents::with_term(&current, &term_now) {
                recents::write(&storage, &next);
                recents.set(next);
            }
        })
    };

    // The query follows the field straight away; a stale debounced term never shows as current.
    let settled = term_now == trimmed;
    let ready = term_now.chars().count() >= 2 && settled;
    let title = t.t("search:title");
    let subtitle = (trimmed.chars().count() >= 2)
        .then(|| t.tf("search:subtitle", &[("query", trimmed.clone())]));

    let body = if trimmed.is_empty() {
        let storage = svc.storage.clone();
        let mut recents = recents;
        let list = recents();
        if list.is_empty() {
            rsx! {
                EmptyState {
                    icon: "search",
                    title: t.t("search:empty.title"),
                    message: rsx! { {t.t("search:empty.message")} },
                }
            }
        } else {
            rsx! {
                div { class: "jr-search__recent",
                    div { class: "jr-list-group jr-list-group--inset",
                        div { class: "jr-list-group__header",
                            span { class: "jr-search__recent-header",
                                {t.t("search:recent.title")}
                                Button {
                                    variant: "plain",
                                    size: "small",
                                    onclick: move |_| {
                                        recents::write(&storage, &[]);
                                        recents.set(Vec::new());
                                    },
                                    {t.t("search:recent.clear")}
                                }
                            }
                        }
                        ul { class: "jr-list",
                            for s in list {
                                li { key: "{s}", class: "jr-list__item",
                                    button {
                                        r#type: "button",
                                        class: "jr-row jr-row--interactive",
                                        onclick: {
                                            let s = s.clone();
                                            move |_| router.replace(&format!("/search?q={}", crate::router::encode(&s)))
                                        },
                                        span { class: "jr-row__leading", Icon { name: "clock", size: 20 } }
                                        span { class: "jr-row__text", span { class: "jr-row__title", "{s}" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if trimmed.chars().count() < 2 {
        rsx! { EmptyState { size: "inline", icon: "search", title: t.t("search:tooShort") } }
    } else if !ready || query.is_loading() && query.error.is_none() {
        rsx! {
            div { class: "jr-search__loading", "aria-busy": "true",
                div { class: "jr-skeleton jr-skeleton--rect", style: "aspect-ratio: 3 / 1; border-radius: var(--radius-xl);" }
                span { class: "jr-skeleton-lines", "aria-hidden": "true",
                    span { class: "jr-skeleton jr-skeleton--text", style: "width: 100%;" }
                    span { class: "jr-skeleton jr-skeleton--text", style: "width: 100%;" }
                    span { class: "jr-skeleton jr-skeleton--text", style: "width: 60%;" }
                }
            }
        }
    } else if query.data.is_none() {
        let cache = svc.cache.clone();
        rsx! {
            EmptyState {
                icon: "triangle-alert",
                title: t.t("search:error.title"),
                message: rsx! { {t.t("search:error.message")} },
                action: rsx! {
                    Button {
                        variant: "tinted",
                        onclick: move |_| cache.invalidate(&key),
                        {t.t("search:error.retry")}
                    }
                },
            }
        }
    } else {
        let data = query.data.clone().unwrap_or_default();
        rsx! { Results { term: term_now.clone(), data: (*data).clone(), remember } }
    };

    rsx! {
        ShellPage { title, subtitle,
            div { class: "jr-search", {body} }
        }
    }
}

#[component]
fn Results(term: String, data: SearchResults, remember: EventHandler<()>) -> Element {
    let t = use_i18n();
    let base = use_base_url();
    let grouped = group_results(&data, &term);
    if grouped.total == 0 {
        return rsx! {
            EmptyState {
                icon: "search-x",
                title: t.tf("search:noResults.title", &[("query", term.clone())]),
                message: rsx! { {t.t("search:noResults.message")} },
            }
        };
    }
    rsx! {
        if let Some(top) = grouped.top {
            section { class: "jr-search__top", "aria-label": t.t("search:topResult"),
                h2 { class: "jr-text-title-2", {t.t("search:topResult")} }
                TopCard { kind: top.kind, item: top.item, base: base.clone(), remember }
            }
        }
        for section in grouped.sections {
            SectionView { key: "{section.id}", section, base: base.clone(), remember }
        }
    }
}

#[component]
fn TopCard(kind: ResultKind, item: BaseItemDto, base: String, remember: EventHandler<()>) -> Element {
    let t = use_i18n();
    let router = use_router();
    let round = kind == ResultKind::Artist;
    let (ratio, mod_) = match kind {
        ResultKind::Movie | ResultKind::Series => ("2 / 3", "2x3"),
        ResultKind::Episode => ("16 / 9", "16x9"),
        _ => ("1 / 1", "1x1"),
    };
    let name = item.name().to_string();
    let subtitle = item_subtitle(kind, &item);
    let to = route_for(kind, &item);
    let src = image(&base, &item, ImageKind::Primary, 400);
    let shape = if round { "circle" } else { "rounded" };
    rsx! {
        a {
            class: "jr-search__top-card",
            draggable: "false",
            onclick: move |_| {
                remember.call(());
                router.push(&to);
            },
            div { class: "jr-search__top-art jr-search__top-art--{mod_}",
                if round && src.is_none() {
                    Artwork { src: None::<String>, ratio, shape, shadow: true, fallback: kind.icon(), class: "jr-artwork--initials" }
                    span { class: "jr-card__initials", {initials(&name)} }
                } else {
                    Artwork { src, ratio, shape, shadow: true, fallback: kind.icon() }
                }
            }
            div { class: "jr-search__top-text",
                span { class: "jr-text-caption jr-color-secondary", {t.t(&format!("search:kinds.{}", kind.key()))} }
                span { class: "jr-text-title-1", "{name}" }
                if let Some(s) = subtitle { span { class: "jr-text-callout jr-color-secondary", "{s}" } }
            }
        }
    }
}

/// One shelf card (`PosterCard`, `LandscapeCard`, `RoundCard`, `SquareCard` DOM).
#[component]
fn ResultCard(
    shape: &'static str,
    title: String,
    subtitle: Option<String>,
    image: Option<String>,
    fallback: &'static str,
    to: String,
    remember: EventHandler<()>,
) -> Element {
    let router = use_router();
    let (ratio, art_shape) = match shape {
        "poster" => ("2 / 3", "rounded"),
        "landscape" => ("16 / 9", "rounded"),
        "round" => ("1 / 1", "circle"),
        _ => ("1 / 1", "rounded"),
    };
    let round_text = if shape == "round" {
        " jr-card__text--center"
    } else {
        ""
    };
    rsx! {
        li { class: "jr-shelf__item",
            div { class: "jr-card jr-card--{shape}", style: "--jr-card-ratio: {ratio};",
                a {
                    class: "jr-card__hit",
                    draggable: "false",
                    onclick: move |_| {
                        remember.call(());
                        router.push(&to);
                    },
                    div { class: "jr-card__art",
                        if shape == "round" && image.is_none() {
                            Artwork { src: None::<String>, ratio, shape: art_shape, shadow: true, fallback, class: "jr-artwork--initials" }
                            span { class: "jr-card__initials", {initials(&title)} }
                        } else {
                            Artwork { src: image, ratio, shape: art_shape, shadow: true, fallback }
                        }
                    }
                    div { class: "jr-card__text{round_text}",
                        span { class: "jr-card__title", "{title}" }
                        if let Some(s) = subtitle { span { class: "jr-card__subtitle", "{s}" } }
                    }
                }
            }
        }
    }
}

fn episode_label(t: I18n, item: &BaseItemDto) -> Option<String> {
    match (item.parent_index_number, item.index_number) {
        (Some(s), Some(e)) => Some(t.tf(
            "search:episodeLabel",
            &[("season", s.to_string()), ("episode", e.to_string())],
        )),
        _ => None,
    }
}

#[component]
fn SectionView(section: ResultSection, base: String, remember: EventHandler<()>) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let title = t.t(&format!("search:sections.{}", section.id));

    if section.id == "songs" {
        return rsx! {
            section { class: "jr-search__songs", "aria-label": title.clone(),
                h2 { class: "jr-text-title-2", "{title}" }
                ol { class: "jr-track-list", "aria-label": title.clone(),
                    for item in section.items {
                        {
                            let name = item.name().to_string();
                            let artist = item
                                .album_artist
                                .clone()
                                .or_else(|| item.artists.first().cloned());
                            let duration = Some(item.duration_seconds()).filter(|d| *d > 0.0);
                            let (svc, play_item) = (svc.clone(), item.clone());
                            rsx! {
                                TrackRow {
                                    key: "{item.id}",
                                    artwork: image(&base, &item, ImageKind::Primary, 80),
                                    title: name.clone(),
                                    artist: artist.map(|a| rsx! { "{a}" }),
                                    album: item.album.clone(),
                                    duration,
                                    play_label: t.tf("search:playSong", &[("title", name.clone())]),
                                    on_play: move |_| {
                                        remember.call(());
                                        if !play_item.id.is_empty() {
                                            svc.music.play(std::slice::from_ref(&play_item), Some(0), Some(false));
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    let (shelf_kind, shape) = match section.id {
        "episodes" => ("landscape", "landscape"),
        "artists" => ("round", "round"),
        "albums" => ("square", "square"),
        _ => ("poster", "poster"),
    };
    let kind = section.kind;
    rsx! {
        section { class: "jr-shelf jr-shelf--{shelf_kind}", "aria-label": title.clone(),
            header { class: "jr-shelf__header",
                div { class: "jr-shelf__titles", h2 { class: "jr-shelf__title", "{title}" } }
            }
            div { class: "jr-shelf__viewport",
                ul { class: "jr-shelf__list",
                    for item in section.items {
                        {
                            let to = route_for(kind, &item);
                            let (subtitle, img) = match section.id {
                                "episodes" => (
                                    Some(
                                        [episode_label(t, &item), item.series_name.clone()]
                                            .into_iter()
                                            .flatten()
                                            .filter(|s| !s.is_empty())
                                            .collect::<Vec<_>>()
                                            .join(" · "),
                                    ),
                                    image(&base, &item, ImageKind::Thumb, 480),
                                ),
                                "artists" => (None, image(&base, &item, ImageKind::Primary, 300)),
                                "albums" => (
                                    item_subtitle(ResultKind::Album, &item),
                                    image(&base, &item, ImageKind::Primary, 300),
                                ),
                                _ => (
                                    item_subtitle(kind, &item),
                                    image(&base, &item, ImageKind::Primary, 300),
                                ),
                            };
                            rsx! {
                                ResultCard {
                                    key: "{item.id}",
                                    shape,
                                    title: item.name().to_string(),
                                    subtitle,
                                    image: img,
                                    fallback: kind.icon(),
                                    to,
                                    remember,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
