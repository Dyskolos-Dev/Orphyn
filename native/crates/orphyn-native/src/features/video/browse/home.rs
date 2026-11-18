//! `VideoHome.tsx`, `HeroCarousel.tsx` and `homeRows.tsx`: "Watch Now", arranged by the user.
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::images::ImageKind;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};

use super::cards::{image, ItemLandscape, ItemPoster};
use super::data::{items_of, use_items, use_resume, use_session_query, use_similar};
use super::format::{
    hero_meta, item_path, play_path, position_ticks, progress_of, remaining_minutes, resume_minutes,
};
use super::layout::{self, count_of, HomeRow, RowSize};
use super::ui::{HeroBanner, Skeleton};
use crate::design::{Button, EmptyState, Shelf};
use crate::features::music::use_base_url;
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::{PageHeader, ShellPage};
use crate::state::{use_app, use_services};

const AUTO_ADVANCE: Duration = Duration::from_millis(8000);
const VIDEO_TYPES: [&str; 2] = ["Movie", "Series"];

type Libraries = Vec<(String, String)>;

fn types(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn has_backdrop(i: &BaseItemDto) -> bool {
    !i.backdrop_image_tags.is_empty() || !i.parent_backdrop_image_tags.is_empty()
}

fn row_title(row: &HomeRow, label: Option<&str>) -> String {
    let t = use_i18n();
    let name = label
        .map(str::to_string)
        .or_else(|| row.label.clone())
        .or_else(|| row.param.clone())
        .unwrap_or_default();
    t.tf(
        &format!("home:rows.video.{}.title", row.kind),
        &[("name", name)],
    )
}

/// `RowFrame`: applies the card size chosen for a row (one step of items-per-row).
#[component]
fn RowFrame(size: RowSize, children: Element) -> Element {
    let shift = match size {
        RowSize::Auto => return rsx! { {children} },
        RowSize::Small => 1,
        RowSize::Large => -1,
    };
    rsx! {
        div {
            class: "jr-home-frame",
            style: "--shelf-density: calc(var(--shelf-density-base, 0) + var(--density-items, 0) + {shift});",
            {children}
        }
    }
}

/// `ShelfSkeleton`.
#[component]
fn ShelfSkeleton(#[props(default = false)] landscape: bool) -> Element {
    let (kind, ratio) = if landscape {
        ("landscape", "16 / 9")
    } else {
        ("poster", "2 / 3")
    };
    rsx! {
        section { class: "jr-shelf jr-shelf--{kind}",
            header { class: "jr-shelf__header",
                div { class: "jr-shelf__titles",
                    h2 { class: "jr-shelf__title", Skeleton { shape: "text" } }
                }
            }
            div { class: "jr-shelf__viewport",
                ul { class: "jr-shelf__list",
                    for i in 0..8 {
                        li { key: "{i}", class: "jr-shelf__item",
                            Skeleton { aspect_ratio: ratio, radius: "var(--radius-poster)" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PosterShelf(
    row: HomeRow,
    items: Vec<BaseItemDto>,
    title: String,
    see_all_to: Option<String>,
) -> Element {
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        RowFrame { size: row.size,
            Shelf { title, kind: "poster", see_all_to,
                for item in items {
                    li { key: "{item.id}", class: "jr-shelf__item", ItemPoster { item: item.clone() } }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct RowProps {
    row: HomeRow,
    libraries: Libraries,
}

#[component]
fn ContinueRow(props: RowProps) -> Element {
    let row = props.row;
    let resume = use_resume(count_of(&row));
    let title = row_title(&row, None);
    let items = resume.data.as_deref().cloned().unwrap_or_default();
    if items.is_empty() {
        return if resume.is_loading() {
            rsx! { ShelfSkeleton { landscape: true } }
        } else {
            rsx! {}
        };
    }
    rsx! {
        RowFrame { size: row.size,
            Shelf { title, kind: "landscape",
                for item in items {
                    li { key: "{item.id}", class: "jr-shelf__item", ItemLandscape { item: item.clone() } }
                }
            }
        }
    }
}

#[component]
fn NextUpRow(props: RowProps) -> Element {
    let row = props.row;
    let count = count_of(&row);
    let resume = use_resume(20);
    let next_up = use_session_query(
        move |acc| {
            let mut k = keys::next_up(acc, None);
            k.push(count.to_string());
            k
        },
        stale::PROGRESS,
        true,
        move |s| async move { s.next_up(None, count as u32).await },
    );
    let title = row_title(&row, None);
    let resume_ids: Vec<String> = resume
        .data
        .as_deref()
        .map(|r| r.iter().map(|i| i.id.clone()).collect())
        .unwrap_or_default();
    let items: Vec<BaseItemDto> = next_up
        .data
        .as_deref()
        .map(|v| {
            v.iter()
                .filter(|i| !resume_ids.contains(&i.id))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        RowFrame { size: row.size,
            Shelf { title, kind: "landscape",
                for item in items {
                    li { key: "{item.id}", class: "jr-shelf__item",
                        ItemLandscape { item: item.clone(), show_progress: false }
                    }
                }
            }
        }
    }
}

#[component]
fn LatestRow(props: RowProps) -> Element {
    let (row, libraries) = (props.row, props.libraries);
    let library = libraries
        .iter()
        .find(|(id, _)| Some(id.as_str()) == row.param.as_deref())
        .cloned();
    let lib_id = library.as_ref().map(|l| l.0.clone());
    let count = count_of(&row);
    let latest = {
        let key_lib = lib_id.clone().unwrap_or_default();
        let fetch_lib = key_lib.clone();
        use_session_query(
            move |acc| {
                let mut k = keys::latest(acc, &key_lib);
                k.push(count.to_string());
                k
            },
            stale::CATALOG,
            lib_id.is_some(),
            move |s| {
                let lib = fetch_lib.clone();
                async move { s.latest(&lib, count as u32).await }
            },
        )
    };
    let title = row_title(&row, library.as_ref().map(|l| l.1.as_str()));
    let Some((lib_id, _)) = library else {
        return rsx! {};
    };
    let items = latest.data.as_deref().cloned().unwrap_or_default();
    rsx! { PosterShelf { row, items, title, see_all_to: Some(format!("/video/library/{lib_id}")) } }
}

#[component]
fn FavouritesRow(props: RowProps) -> Element {
    let row = props.row;
    let q = use_items(
        ItemsParams {
            include_item_types: types(&VIDEO_TYPES),
            is_favorite: Some(true),
            sort_by: types(&["SortName"]),
            page_size: Some(count_of(&row) as u32),
            ..ItemsParams::default()
        },
        false,
        true,
    );
    let title = row_title(&row, None);
    rsx! { PosterShelf { row, items: items_of(&q), title } }
}

#[component]
fn GenreRow(props: RowProps) -> Element {
    let row = props.row;
    let q = use_items(
        ItemsParams {
            include_item_types: types(&VIDEO_TYPES),
            genres: row.param.iter().cloned().collect(),
            sort_by: types(&["DateCreated"]),
            sort_order: Some("Descending".into()),
            page_size: Some(count_of(&row) as u32),
            ..ItemsParams::default()
        },
        false,
        row.param.is_some(),
    );
    let title = row_title(&row, None);
    rsx! { PosterShelf { row, items: items_of(&q), title } }
}

#[component]
fn CollectionRow(props: RowProps) -> Element {
    let row = props.row;
    let q = use_items(
        ItemsParams {
            parent_id: row.param.clone(),
            sort_by: types(&["PremiereDate", "SortName"]),
            page_size: Some(count_of(&row) as u32),
            ..ItemsParams::default()
        },
        false,
        row.param.is_some(),
    );
    let title = row_title(&row, None);
    let see_all_to = row.param.as_ref().map(|p| format!("/video/item/{p}"));
    rsx! { PosterShelf { row, items: items_of(&q), title, see_all_to } }
}

/// Similar titles to the last thing watched (`BecauseRow`).
#[component]
fn BecauseRow(props: RowProps) -> Element {
    let t = use_i18n();
    let row = props.row;
    let played = use_items(
        ItemsParams {
            include_item_types: types(&["Movie", "Episode"]),
            filters: types(&["IsPlayed"]),
            sort_by: types(&["DatePlayed"]),
            sort_order: Some("Descending".into()),
            page_size: Some(1),
            ..ItemsParams::default()
        },
        false,
        true,
    );
    let last = items_of(&played).into_iter().next();
    // An episode stands for its series.
    let (seed_id, seed_name) = match &last {
        Some(l) if l.is_type("Episode") => (l.series_id.clone(), l.series_name.clone()),
        Some(l) => (Some(l.id.clone()), l.name.clone()),
        None => (None, None),
    };
    let count = count_of(&row);
    let similar = use_similar(seed_id.clone(), count);
    let title = t.tf(
        "home:rows.video.because.title",
        &[("name", seed_name.unwrap_or_default())],
    );
    let items = similar.data.as_deref().cloned().unwrap_or_default();
    rsx! { PosterShelf { row, items, title } }
}

#[component]
fn ReleasedRow(props: RowProps) -> Element {
    let row = props.row;
    let q = use_items(
        ItemsParams {
            include_item_types: types(&VIDEO_TYPES),
            sort_by: types(&["PremiereDate"]),
            sort_order: Some("Descending".into()),
            page_size: Some(count_of(&row) as u32),
            ..ItemsParams::default()
        },
        true,
        true,
    );
    let title = row_title(&row, None);
    rsx! { PosterShelf { row, items: items_of(&q), title } }
}

#[component]
fn RandomRow(props: RowProps) -> Element {
    let row = props.row;
    let q = use_items(
        ItemsParams {
            include_item_types: types(&VIDEO_TYPES),
            sort_by: types(&["Random"]),
            page_size: Some(count_of(&row) as u32),
            ..ItemsParams::default()
        },
        false,
        true,
    );
    let title = row_title(&row, None);
    rsx! { PosterShelf { row, items: items_of(&q), title } }
}

/// A shelf of the video home, by row type (the hero is handled by the page).
#[component]
fn VideoRow(props: RowProps) -> Element {
    match props.row.kind.as_str() {
        "continue" => rsx! { ContinueRow { ..props } },
        "nextUp" => rsx! { NextUpRow { ..props } },
        "latest" => rsx! { LatestRow { ..props } },
        "favourites" => rsx! { FavouritesRow { ..props } },
        "genre" => rsx! { GenreRow { ..props } },
        "collection" => rsx! { CollectionRow { ..props } },
        "because" => rsx! { BecauseRow { ..props } },
        "released" => rsx! { ReleasedRow { ..props } },
        "random" => rsx! { RandomRow { ..props } },
        _ => rsx! {},
    }
}

/// "Watch Now" hero: a `HeroBanner` per featured item that advances by itself and pauses while
/// the pointer is inside (`HeroCarousel`).
#[component]
fn HeroCarousel(items: Vec<BaseItemDto>) -> Element {
    let t = use_i18n();
    let router = use_router();
    let base = use_base_url();
    let mut index = use_signal(|| 0usize);
    let mut paused = use_signal(|| false);
    let count = items.len();
    use_resource(use_reactive!(|count| async move {
        let (p, i) = (paused(), index());
        if p || count < 2 {
            return;
        }
        tokio::time::sleep(AUTO_ADVANCE).await;
        index.set((i + 1) % count);
    }));
    let Some(current) = items.get(index().min(count.saturating_sub(1))) else {
        return rsx! {};
    };
    if current.id.is_empty() {
        return rsx! {};
    }
    let id = current.id.clone();
    let progress = progress_of(current);
    let playable = current.is_type("Movie") || current.is_type("Episode");
    let ticks = position_ticks(current);
    let play_label = if progress.is_some() {
        t.tf(
            "video:item.resume",
            &[("minutes", resume_minutes(ticks).to_string())],
        )
    } else {
        t.t("video:item.play")
    };
    let play_to = play_path(&id, ticks);
    let more_to = item_path(&id);
    let more_variant = if playable { "gray" } else { "contrast" };
    rsx! {
        div {
            class: "jr-video-carousel",
            onmouseenter: move |_| paused.set(true),
            onmouseleave: move |_| paused.set(false),
            div { class: "jr-video-carousel__slide", key: "{id}",
                HeroBanner {
                    backdrop: image(&base, current, ImageKind::Backdrop, 1920, 80),
                    logo: image(&base, current, ImageKind::Logo, 600, 90),
                    title: current.name().to_string(),
                    eyebrow: t.t("video:home.featured"),
                    meta: rsx! { "{hero_meta(current)}" },
                    description: current.overview.clone(),
                    progress,
                    progress_caption: progress.map(|_| t.tf(
                        "video:item.minutesLeft",
                        &[("minutes", remaining_minutes(current).to_string())],
                    )),
                    actions: rsx! {
                        if playable {
                            Button {
                                variant: "contrast",
                                size: "large",
                                icon: "play",
                                icon_fill: true,
                                onclick: move |_| router.push(&play_to),
                                "{play_label}"
                            }
                        }
                        Button {
                            variant: more_variant,
                            size: "large",
                            icon: "info",
                            onclick: move |_| router.push(&more_to),
                            {t.t("video:item.more")}
                        }
                    },
                }
            }
            if count > 1 {
                div {
                    class: "jr-video-carousel__dots",
                    role: "group",
                    "aria-label": t.t("video:home.featured"),
                    for i in 0..count {
                        button {
                            key: "{i}",
                            r#type: "button",
                            class: "jr-video-carousel__dot",
                            "aria-label": t.tf("video:home.slide", &[("n", (i + 1).to_string())]),
                            "aria-current": (i == index()).to_string(),
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |_| index.set(i),
                        }
                    }
                }
            }
        }
    }
}

/// /video: "Watch Now" (`VideoHome`).
#[component]
pub fn VideoHomePage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let acc = session.account_id.clone();
    let views = use_query(keys::views(&acc), stale::VIEWS, {
        let s = session.clone();
        move || {
            let s = s.clone();
            async move { s.user_views().await }
        }
    });
    let libraries: Libraries = views
        .data
        .as_deref()
        .map(|v| {
            v.iter()
                .filter(|i| matches!(i.collection_type.as_deref(), Some("movies" | "tvshows")))
                .filter(|i| !i.id.is_empty())
                .map(|i| (i.id.clone(), i.name().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let enabled = !libraries.is_empty();

    // The saved layout of this server and user (Settings > Home writes it).
    let server_id = (app.accounts)()
        .iter()
        .find(|a| a.id == acc)
        .map(|a| a.server_id.clone())
        .unwrap_or_default();
    let raw = svc
        .storage
        .get(&layout::storage_key(&server_id, &session.user_id));
    let home = layout::sync_libraries(layout::parse_layout(raw.as_deref()), &libraries);
    let visible: Vec<HomeRow> = home.rows.into_iter().filter(|r| !r.hidden).collect();
    let hero_row = visible.iter().find(|r| r.kind == "hero").cloned();

    let recent = use_items(
        ItemsParams {
            include_item_types: types(&VIDEO_TYPES),
            sort_by: types(&["DateCreated"]),
            sort_order: Some("Descending".into()),
            page_size: Some(24),
            ..ItemsParams::default()
        },
        false,
        enabled && hero_row.is_some(),
    );

    let title = t.t("video:home.title");
    if views.is_loading() {
        return rsx! {
            ShellPage { title, show_back: false,
                ShelfSkeleton { landscape: true }
                ShelfSkeleton {}
            }
        };
    }
    if views.error.is_some() && views.data.is_none() || !enabled {
        let failed = views.error.is_some();
        return rsx! {
            ShellPage { title, show_back: false,
                EmptyState {
                    icon: "clapperboard",
                    title: if failed { t.t("video:home.loadError") } else { t.t("video:home.noLibrary.title") },
                    message: (!failed).then(|| rsx! { {t.t("video:home.noLibrary.message")} }),
                }
            }
        };
    }

    let featured: Vec<BaseItemDto> = match &hero_row {
        Some(r) => items_of(&recent)
            .into_iter()
            .filter(has_backdrop)
            .take(count_of(r))
            .collect(),
        None => Vec::new(),
    };
    // The toolbar floats over the hero only when the hero is the first thing on the page.
    let hero_first = visible.first().is_some_and(|r| r.kind == "hero") && !featured.is_empty();
    let header = if hero_first {
        PageHeader::Hero
    } else {
        PageHeader::Large
    };
    let recent_loading = recent.is_loading();
    rsx! {
        ShellPage { title, header, show_back: false,
            for row in visible {
                if row.kind == "hero" {
                    if !featured.is_empty() {
                        HeroCarousel { key: "{row.id}", items: featured.clone() }
                    } else if recent_loading {
                        Skeleton { key: "{row.id}", aspect_ratio: "21 / 9", radius: "0" }
                    }
                } else {
                    VideoRow { key: "{row.id}", row: row.clone(), libraries: libraries.clone() }
                }
            }
        }
    }
}
