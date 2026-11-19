//! `VideoItem.tsx`: the detail page of a movie, series, season or episode, with the season and
//! episode shelves, synopsis, cast and crew and similar titles.
use dioxus::prelude::*;
use orphyn_jellyfin::api::video::{Person, VideoItemExt};
use orphyn_jellyfin::images::ImageKind;
use orphyn_jellyfin::BaseItemDto;

use super::cards::{image, landscape_image, use_item_actions, ItemPoster};
use super::data::{list_of, use_episodes, use_item, use_seasons, use_similar};
use super::format::{
    badge_list, format_runtime, is_played, item_path, meta_parts, pick_series_episode, play_path,
    primary_action, progress_of, remaining_minutes, sort_episodes, PrimaryAction,
};
use super::trailer::TrailerButton;
use super::ui::{CardKind, HeroBanner, MediaCard, MenuButton, Skeleton};
use crate::design::{Button, EmptyState, IconButton, MenuItem, Shelf};
use crate::features::music::use_base_url;
use crate::i18n::{use_i18n, I18n};
use crate::router::use_router;
use crate::shell::{PageHeader, ShellPage};

fn season_label(t: I18n, s: &BaseItemDto) -> String {
    match s.index_number {
        Some(n) if n != 0 => t.tf("video:item.season", &[("number", n.to_string())]),
        _ => s.name.clone().unwrap_or_else(|| t.t("video:item.specials")),
    }
}

/// `Badges`: 4K, HDR, audio layout of the first media source.
#[component]
fn Badges(item: BaseItemDto) -> Element {
    let list = badge_list(&item.media_streams());
    rsx! {
        for b in list {
            span { key: "{b}", class: "jr-video-badge", "{b}" }
        }
    }
}

/// The season picker: a segmented control up to five seasons, else a menu (`EpisodeShelf`).
#[component]
fn SeasonSegments(
    options: Vec<(String, String)>,
    value: String,
    label: String,
    on_change: EventHandler<String>,
) -> Element {
    let index = options.iter().position(|o| o.0 == value).unwrap_or(0);
    let count = options.len();
    rsx! {
        div {
            role: "radiogroup",
            "aria-label": "{label}",
            class: "jr-segmented jr-segmented--medium",
            style: "--jr-seg-count: {count}; --jr-seg-index: {index};",
            span { class: "jr-segmented__thumb", "aria-hidden": "true" }
            for (i, (val, text)) in options.into_iter().enumerate() {
                button {
                    key: "{val}",
                    r#type: "button",
                    role: "radio",
                    "aria-checked": (i == index).to_string(),
                    tabindex: if i == index { "0" } else { "-1" },
                    class: if i == index { "jr-segmented__item jr-segmented__item--selected" } else { "jr-segmented__item" },
                    onmousedown: move |e| e.stop_propagation(),
                    onclick: move |_| on_change.call(val.clone()),
                    "{text}"
                }
            }
        }
    }
}

#[component]
fn EpisodeShelf(
    series_id: String,
    initial_season_id: Option<String>,
    highlight_id: Option<String>,
) -> Element {
    let t = use_i18n();
    let router = use_router();
    let base = use_base_url();
    let seasons = use_seasons(Some(series_id.clone()));
    let episodes = use_episodes(Some(series_id));
    let mut chosen: Signal<Option<String>> = use_signal(|| initial_season_id.clone());
    use_effect(use_reactive!(|initial_season_id| {
        if let Some(i) = initial_season_id {
            chosen.set(Some(i));
        }
    }));
    let list = list_of(&seasons);
    let season_id = chosen().or_else(|| {
        list.iter()
            .find(|s| s.index_number.unwrap_or(0) > 0)
            .or(list.first())
            .map(|s| s.id.clone())
    });
    let options: Vec<(String, String)> = list
        .iter()
        .map(|s| (s.id.clone(), season_label(t, s)))
        .collect();

    if seasons.is_loading() || episodes.is_loading() {
        return rsx! {
            Shelf { title: t.t("video:item.episodes"), kind: "landscape",
                for i in 0..4 {
                    li { key: "{i}", class: "jr-shelf__item",
                        Skeleton { aspect_ratio: "16 / 9", radius: "var(--radius-poster)" }
                    }
                }
            }
        };
    }
    if list.is_empty() {
        return rsx! {};
    }
    let shown = sort_episodes(
        &list_of(&episodes)
            .into_iter()
            .filter(|e| e.season_id == season_id)
            .collect::<Vec<_>>(),
    );
    let current_label = options
        .iter()
        .find(|o| Some(&o.0) == season_id.as_ref())
        .map(|o| o.1.clone())
        .unwrap_or_default();
    let picker_items: Vec<MenuItem> = options
        .iter()
        .map(|(value, label)| {
            let value = value.clone();
            MenuItem::Action {
                label: label.clone(),
                icon: None,
                checked: Some(Some(&value) == season_id.as_ref()),
                destructive: false,
                disabled: false,
                on_select: Callback::new(move |_| chosen.set(Some(value.clone()))),
            }
        })
        .collect();
    let picker_label = t.t("video:item.seasonPicker");
    rsx! {
        section { class: "jr-video-section", "aria-label": t.t("video:item.episodes"),
            div { class: "jr-video-seasons",
                if options.len() <= 5 {
                    SeasonSegments {
                        options: options.clone(),
                        value: season_id.clone().unwrap_or_default(),
                        label: picker_label.clone(),
                        on_change: move |v| chosen.set(Some(v)),
                    }
                } else {
                    h2 { class: "jr-text-title-2 jr-color-primary", "{current_label}" }
                    MenuButton {
                        label: picker_label.clone(),
                        icon: "chevrons-up-down",
                        items: picker_items,
                        variant: "gray",
                    }
                }
            }
            Shelf { kind: "landscape",
                for e in shown {
                    li { key: "{e.id}", class: "jr-shelf__item",
                        {
                            let left = remaining_minutes(&e);
                            let progress = progress_of(&e);
                            let caption = if is_played(&e) {
                                Some(t.t("video:item.watched"))
                            } else if progress.is_some() && left > 0 {
                                Some(t.tf("video:item.minutesLeft", &[("minutes", left.to_string())]))
                            } else {
                                None
                            };
                            let title = format!(
                                "{}{}",
                                e.index_number.map(|n| format!("{n}. ")).unwrap_or_default(),
                                e.name()
                            );
                            let play_to = play_path(&e.id, super::format::position_ticks(&e));
                            rsx! {
                                MediaCard {
                                    kind: CardKind::Landscape,
                                    title,
                                    subtitle: Some(format_runtime(e.run_time_ticks)),
                                    description: e.overview.clone(),
                                    image: landscape_image(&base, &e, 640),
                                    progress,
                                    caption,
                                    selected: highlight_id.as_deref() == Some(e.id.as_str()),
                                    to: item_path(&e.id),
                                    fallback: "clapperboard",
                                    on_play: move |_| router.push(&play_to),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// People of the cast and crew without repeats, at most 30 (`CastShelf`).
fn unique_people(item: &BaseItemDto) -> Vec<Person> {
    let mut seen = std::collections::HashSet::new();
    item.people()
        .into_iter()
        .filter(|p| {
            let key = p.id.clone().or_else(|| p.name.clone()).unwrap_or_default();
            p.name.as_deref().is_some_and(|n| !n.is_empty()) && seen.insert(key)
        })
        .take(30)
        .collect()
}

#[component]
fn CastShelf(item: BaseItemDto) -> Element {
    let t = use_i18n();
    let base = use_base_url();
    let people = unique_people(&item);
    if people.is_empty() {
        return rsx! {};
    }
    rsx! {
        Shelf { title: t.t("video:item.castCrew"), kind: "round",
            for (i, p) in people.into_iter().enumerate() {
                li { key: "{i}", class: "jr-shelf__item",
                    {
                        let pic = match (&p.id, &p.primary_image_tag) {
                            (Some(id), Some(tag)) => {
                                let mut stub = BaseItemDto { id: id.clone(), ..Default::default() };
                                stub.image_tags.insert("Primary".into(), tag.clone());
                                image(&base, &stub, ImageKind::Primary, 300, 90)
                            }
                            _ => None,
                        };
                        let sub = p
                            .role
                            .clone()
                            .filter(|r| !r.is_empty())
                            .or_else(|| p.kind.clone().filter(|k| !k.is_empty()));
                        rsx! {
                            MediaCard {
                                kind: CardKind::Round,
                                title: p.name.clone().unwrap_or_default(),
                                subtitle: sub,
                                image: pic,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SimilarShelf(id: String) -> Element {
    let t = use_i18n();
    let similar = use_similar(Some(id), 12);
    let items = list_of(&similar);
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        Shelf { title: t.t("video:item.similar"), kind: "poster",
            for s in items {
                li { key: "{s.id}", class: "jr-shelf__item", ItemPoster { item: s.clone() } }
            }
        }
    }
}

/// /video/item/:itemId (`VideoItem`). A season shows its series with that season selected.
#[component]
pub fn VideoItemPage(item_id: String) -> Element {
    let t = use_i18n();
    let router = use_router();
    let actions = use_item_actions();
    let base = use_base_url();
    let requested = use_item(&item_id, true);
    // `Season` -> its series, with the season picked (React redirects with `?season=`).
    let season_series = requested
        .data
        .as_ref()
        .filter(|d| d.is_type("Season"))
        .and_then(|d| d.series_id.clone());
    let series_item = use_item(
        season_series.as_deref().unwrap_or(""),
        season_series.is_some(),
    );
    let from_season = season_series.is_some();
    let data: Option<BaseItemDto> = if from_season {
        series_item.data.as_deref().cloned()
    } else {
        requested.data.as_deref().cloned()
    };
    let initial_season = if from_season {
        Some(item_id.clone())
    } else {
        data.as_ref().and_then(|d| d.season_id.clone())
    };
    let series_id = data.as_ref().and_then(|d| {
        if d.is_type("Series") {
            Some(d.id.clone())
        } else {
            d.series_id.clone()
        }
    });
    let episodes = use_episodes(
        data.as_ref()
            .filter(|d| d.is_type("Series"))
            .map(|d| d.id.clone()),
    );

    let loading = requested.is_loading() || (from_season && series_item.is_loading());
    if loading {
        return rsx! {
            ShellPage { title: "", header: PageHeader::Inline,
                Skeleton { aspect_ratio: "21 / 9", radius: "0" }
            }
        };
    }
    let Some(data) = data.filter(|d| !d.id.is_empty()) else {
        return rsx! {
            ShellPage { title: "", header: PageHeader::Inline,
                EmptyState {
                    icon: "clapperboard",
                    title: t.t("video:item.notFound.title"),
                    message: rsx! { {t.t("video:item.notFound.message")} },
                }
            }
        };
    };

    let id = data.id.clone();
    let is_series = data.is_type("Series");
    let target_episode = if is_series {
        pick_series_episode(&list_of(&episodes), None)
    } else {
        None
    };
    let action = primary_action(&data, target_episode.as_ref());
    let played = is_played(&data);
    let favourite = data.is_favorite();
    let (play_id, start_ticks) = match &action {
        PrimaryAction::Episode {
            episode_id,
            position_ticks,
            ..
        } => (episode_id.clone(), *position_ticks),
        PrimaryAction::Resume { position_ticks, .. } => (id.clone(), *position_ticks),
        PrimaryAction::Play => (id.clone(), 0),
    };
    let playable = !is_series || matches!(action, PrimaryAction::Episode { .. });
    let label = match &action {
        PrimaryAction::Resume { minutes, .. } => {
            t.tf("video:item.resume", &[("minutes", minutes.to_string())])
        }
        PrimaryAction::Episode {
            season: Some(s),
            episode: Some(e),
            position_ticks,
            ..
        } => t.tf(
            if *position_ticks > 0 {
                "video:item.resumeEpisode"
            } else {
                "video:item.playEpisode"
            },
            &[("season", s.to_string()), ("episode", e.to_string())],
        ),
        _ => t.t("video:item.play"),
    };
    let episode_tag = match (data.parent_index_number, data.index_number) {
        (Some(s), Some(e)) if data.is_type("Episode") => Some(t.tf(
            "video:item.episodeLabel",
            &[("season", s.to_string()), ("episode", e.to_string())],
        )),
        _ => None,
    };
    let mut meta = meta_parts(&data);
    if is_series {
        if let Some(c) = data.child_count.filter(|c| *c > 0) {
            meta.push(t.tc("video:item.seasonsCount", c as i64, &[]));
        }
    }
    let rating = data
        .community_rating()
        .filter(|r| *r != 0.0)
        .map(|r| format!(" · ★ {r:.1}"))
        .unwrap_or_default();
    let meta_line = format!("{}{rating}", meta.join(" · "));
    let progress_item = target_episode.as_ref().unwrap_or(&data);
    let progress = if playable {
        progress_of(progress_item)
    } else {
        None
    };
    let title = match (&data.series_name, &episode_tag) {
        (Some(s), Some(_)) => s.clone(),
        _ => data.name().to_string(),
    };
    let eyebrow = episode_tag
        .as_ref()
        .map(|tag| format!("{tag} · {}", data.name()));
    let play_to = play_path(&play_id, start_ticks);
    let restart_to = play_path(&play_id, 0);
    let (a_played, a_fav) = (actions.clone(), actions);
    let (pid, fid) = (id.clone(), id.clone());
    let show_trailer = data.is_type("Movie") || is_series;
    let page_title = data.name().to_string();
    let highlight = data.is_type("Episode").then(|| id.clone());
    let overview = data.overview.clone().filter(|o| !o.is_empty());

    rsx! {
        ShellPage { title: page_title, header: PageHeader::Inline, class: "jr-video-detail",
            HeroBanner {
                size: "medium",
                backdrop: image(&base, &data, ImageKind::Backdrop, 1920, 80),
                logo: image(&base, &data, ImageKind::Logo, 600, 90),
                title,
                eyebrow,
                meta: rsx! {
                    "{meta_line} "
                    Badges { item: data.clone() }
                },
                description: None,
                progress,
                progress_caption: progress.map(|_| t.tf(
                    "video:item.minutesLeft",
                    &[("minutes", remaining_minutes(progress_item).to_string())],
                )),
                actions: rsx! {
                    if playable {
                        Button {
                            variant: "contrast",
                            size: "large",
                            icon: "play",
                            icon_fill: true,
                            onclick: move |_| router.push(&play_to),
                            "{label}"
                        }
                    }
                    if playable && start_ticks > 0 {
                        IconButton {
                            variant: "glass",
                            size: "large",
                            icon: "rotate-ccw",
                            label: t.t("video:item.fromStart"),
                            onclick: move |_| router.push(&restart_to),
                        }
                    }
                    if show_trailer { TrailerButton { item: data.clone() } }
                    IconButton {
                        variant: "glass",
                        size: "large",
                        icon: "check",
                        label: if played { t.t("video:item.markUnwatched") } else { t.t("video:item.markWatched") },
                        pressed: played,
                        onclick: move |_| a_played.mark_played(&pid, !played),
                    }
                    IconButton {
                        variant: "glass",
                        size: "large",
                        icon: "heart",
                        label: if favourite { t.t("video:item.unfavourite") } else { t.t("video:item.favourite") },
                        pressed: favourite,
                        onclick: move |_| a_fav.set_favourite(&fid, !favourite),
                    }
                },
            }
            if let Some(sid) = series_id.clone() {
                EpisodeShelf { series_id: sid, initial_season_id: initial_season, highlight_id: highlight }
            }
            if let Some(o) = overview {
                section { class: "jr-video-section jr-video-synopsis",
                    h2 { class: "jr-text-title-2 jr-color-primary", {t.t("video:item.synopsis")} }
                    p { class: "jr-text-body jr-color-secondary jr-selectable", "{o}" }
                }
            }
            CastShelf { item: data.clone() }
            SimilarShelf { id: series_id.unwrap_or(id) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_is_deduplicated_and_capped() {
        let people: Vec<serde_json::Value> = (0..40)
            .map(|i| serde_json::json!({"Id": format!("p{}", i % 35), "Name": format!("N{i}")}))
            .chain([serde_json::json!({"Id": "x", "Name": ""})])
            .collect();
        let item: BaseItemDto =
            serde_json::from_value(serde_json::json!({"Id": "m", "People": people})).unwrap();
        let out = unique_people(&item);
        assert_eq!(out.len(), 30);
        assert_eq!(out[0].id.as_deref(), Some("p0"));
        assert!(out.iter().all(|p| p.name.as_deref() != Some("")));
    }
}
