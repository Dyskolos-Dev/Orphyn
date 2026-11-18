//! `VideoLibrary.tsx`: a library of movies or series, sortable and filterable, loaded page by page
//! while scrolling, with the A-Z rail.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use orphyn_jellyfin::{BaseItemDto, ItemsParams};

use super::cards::ItemPoster;
use super::data::{items_of, use_items, use_views};
use super::format::{first_index_for_letter, letter_of};
use super::ui::{Grid, MenuButton, Skeleton};
use crate::design::{EmptyState, MenuItem, Segment, SegmentedControl, Spinner};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::shell::ShellPage;
use crate::state::use_app;

const PAGE_SIZE: u32 = 60;
const LETTERS: [char; 27] = [
    '#', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SortKey {
    Name,
    DateAdded,
    ReleaseDate,
    Rating,
}

impl SortKey {
    const ALL: [SortKey; 4] = [
        SortKey::Name,
        SortKey::DateAdded,
        SortKey::ReleaseDate,
        SortKey::Rating,
    ];

    fn by(self) -> Vec<String> {
        let list: &[&str] = match self {
            SortKey::Name => &["SortName"],
            SortKey::DateAdded => &["DateCreated", "SortName"],
            SortKey::ReleaseDate => &["PremiereDate", "ProductionYear", "SortName"],
            SortKey::Rating => &["CommunityRating", "SortName"],
        };
        list.iter().map(|s| s.to_string()).collect()
    }

    fn order(self) -> &'static str {
        match self {
            SortKey::Name => "Ascending",
            _ => "Descending",
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortKey::Name => "video:library.sortName",
            SortKey::DateAdded => "video:library.sortDateAdded",
            SortKey::ReleaseDate => "video:library.sortReleaseDate",
            SortKey::Rating => "video:library.sortRating",
        }
    }
}

fn check_item(label: String, checked: bool, on_select: impl FnMut(()) + 'static) -> MenuItem {
    MenuItem::Action {
        label,
        icon: None,
        checked: Some(checked),
        destructive: false,
        disabled: false,
        on_select: Callback::new(on_select),
    }
}

/// The part of a library page that depends on its filters: reset the extra pages when it changes.
#[derive(Clone, PartialEq)]
struct Filters {
    params: ItemsParams,
}

#[component]
pub fn VideoLibraryPage(library_id: String) -> Element {
    let t = use_i18n();
    let app = use_app();
    let router = use_router();
    let views = use_views();
    let mut sort = use_signal(|| SortKey::Name);
    let mut played = use_signal(|| "all");
    let mut genre: Signal<Option<String>> = use_signal(|| None);
    let mut pending_letter: Signal<Option<char>> = use_signal(|| None);

    let alias = match library_id.as_str() {
        "movies" => Some("movies"),
        "series" => Some("series"),
        _ => None,
    };
    let view = if alias.is_some() {
        None
    } else {
        views
            .data
            .as_deref()
            .and_then(|v| v.iter().find(|i| i.id == library_id))
            .cloned()
    };
    let collection = view.as_ref().and_then(|v| v.collection_type.clone());
    let include: Vec<String> = if alias == Some("movies") || collection.as_deref() == Some("movies")
    {
        vec!["Movie".into()]
    } else if alias == Some("series") || collection.as_deref() == Some("tvshows") {
        vec!["Series".into()]
    } else {
        vec!["Movie".into(), "Series".into()]
    };
    let title = match alias {
        Some(a) => t.t(&format!("video:library.{a}")),
        None => view
            .as_ref()
            .map(|v| v.name().to_string())
            .unwrap_or_else(|| {
                t.t(if include.first().map(String::as_str) == Some("Series") {
                    "video:library.series"
                } else {
                    "video:library.movies"
                })
            }),
    };
    let ready = alias.is_some() || !views.is_loading();

    let spec = sort();
    let params = ItemsParams {
        parent_id: if alias.is_some() {
            None
        } else {
            Some(library_id.clone())
        },
        include_item_types: include,
        sort_by: spec.by(),
        sort_order: Some(spec.order().into()),
        filters: match played() {
            "played" => vec!["IsPlayed".into()],
            "unplayed" => vec!["IsUnplayed".into()],
            _ => Vec::new(),
        },
        genres: genre().into_iter().collect(),
        page_size: Some(PAGE_SIZE),
        ..ItemsParams::default()
    };
    let filters = Filters {
        params: params.clone(),
    };
    let query = use_items(params.clone(), false, ready);

    // Pages after the first (`fetchNextPage`): kept here, dropped when a filter changes.
    let mut extra: Signal<Vec<BaseItemDto>> = use_signal(Vec::new);
    let mut extra_for: Signal<Option<Filters>> = use_signal(|| None);
    let mut fetching = use_signal(|| false);
    let mut exhausted = use_signal(|| false);
    if extra_for.peek().as_ref() != Some(&filters) {
        extra_for.set(Some(filters.clone()));
        extra.set(Vec::new());
        exhausted.set(false);
    }
    let mut items = items_of(&query);
    for it in extra() {
        if !items.iter().any(|x| x.id == it.id) {
            items.push(it);
        }
    }
    let total = query.data.as_ref().map(|p| p.total).unwrap_or(items.len());
    let has_next = items.len() < total && !exhausted();

    let load = use_callback(
        move |(start, params, filters): (usize, ItemsParams, Filters)| {
            if *fetching.peek() {
                return;
            }
            let Some(session) = (app.session).peek().clone() else {
                return;
            };
            fetching.set(true);
            spawn(async move {
                let r = session.video_items_page(&params, start, false).await;
                fetching.set(false);
                if extra_for.peek().as_ref() != Some(&filters) {
                    return;
                }
                match r {
                    Ok(page) if !page.items.is_empty() => extra.write().extend(page.items),
                    // An empty page or an error ends the scroll (no retry loop).
                    _ => exhausted.set(true),
                }
            });
        },
    );

    // Infinite scroll: load the next page when the sentinel comes within 600 px of the viewport
    // bottom (the probe is a fixed 1 px box there).
    let mut sentinel: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let mut probe: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let scroll = (router.scroll_y)();
    {
        let (n, filters, params) = (items.len(), filters.clone(), params.clone());
        use_effect(use_reactive!(|scroll, n, has_next, filters, params| {
            let _ = (scroll, sentinel());
            if !has_next || pending_letter.peek().is_some() {
                return;
            }
            spawn(async move {
                let (Some(s), Some(p)) = (sentinel.peek().clone(), probe.peek().clone()) else {
                    return;
                };
                let (Ok(sr), Ok(pr)) = (s.get_client_rect().await, p.get_client_rect().await)
                else {
                    return;
                };
                if sr.size.height > 0.0 && sr.origin.y < pr.origin.y + 600.0 {
                    load.call((n, params, filters));
                }
            });
        }));
    }

    // A-Z rail: scroll to the letter, loading further pages until it is reached.
    let cells: Rc<RefCell<HashMap<usize, Rc<MountedData>>>> = use_hook(Default::default);
    {
        let (n, items, filters, params) = (items.len(), items.clone(), filters.clone(), params);
        let cells = cells.clone();
        use_effect(use_reactive!(|n, has_next, items, filters, params| {
            let _ = n;
            let Some(letter) = pending_letter() else {
                return;
            };
            let found = first_index_for_letter(&items, letter);
            let last = items.last().map(letter_of).unwrap_or('#');
            match found {
                Some(i) if letter == '#' || last >= letter || !has_next => {
                    if let Some(el) = cells.borrow().get(&i).cloned() {
                        spawn(async move {
                            let _ = el.scroll_to(ScrollBehavior::Smooth).await;
                        });
                    }
                    pending_letter.set(None);
                }
                _ if has_next => load.call((items.len(), params, filters)),
                _ => pending_letter.set(None),
            }
        }));
    }

    let sort_items: Vec<MenuItem> = SortKey::ALL
        .into_iter()
        .map(|k| check_item(t.t(k.label()), sort() == k, move |_| sort.set(k)))
        .collect();
    // The data layer has no genre list yet: offer the genres of what is loaded (+ the selected one).
    let mut genres: Vec<String> = genre().into_iter().collect();
    for it in &items {
        for g in &it.genres {
            if !genres.contains(g) {
                genres.push(g.clone());
            }
        }
    }
    genres.sort_by_key(|g| g.to_lowercase());
    let mut genre_items = vec![check_item(
        t.t("video:library.allGenres"),
        genre().is_none(),
        move |_| genre.set(None),
    )];
    genre_items.extend(genres.into_iter().map(|g| {
        let selected = genre().as_deref() == Some(g.as_str());
        let label = g.clone();
        check_item(label, selected, move |_| genre.set(Some(g.clone())))
    }));

    let subtitle = query
        .data
        .as_ref()
        .map(|_| t.tc("video:library.count", total as i64, &[]));
    let accessory = rsx! {
        SegmentedControl {
            label: t.t("video:library.filter"),
            value: played(),
            on_change: move |v| played.set(v),
            options: vec![
                Segment { value: "all", label: t.t("video:library.filterAll") },
                Segment { value: "unplayed", label: t.t("video:library.filterUnplayed") },
                Segment { value: "played", label: t.t("video:library.filterPlayed") },
            ],
        }
    };
    let actions = rsx! {
        MenuButton { label: t.t("video:library.sort"), icon: "arrow-up-down", items: sort_items }
        MenuButton { label: t.t("video:library.genre"), icon: "list-filter", items: genre_items }
    };

    let loading = query.is_loading() || !ready;
    rsx! {
        ShellPage {
            title: title.clone(),
            subtitle,
            gutter: true,
            header_accessory: accessory,
            actions,
            if loading {
                Grid {
                    for i in 0..18 {
                        li { key: "{i}", class: "jr-grid__item",
                            Skeleton { aspect_ratio: "2 / 3", radius: "var(--radius-poster)" }
                        }
                    }
                }
            } else if items.is_empty() {
                EmptyState {
                    icon: "clapperboard",
                    title: t.t("video:library.empty.title"),
                    message: rsx! { {t.t("video:library.empty.message")} },
                }
            } else {
                div { class: "jr-video-library",
                    Grid { label: title.clone(),
                        for (i, item) in items.iter().cloned().enumerate() {
                            li { key: "{item.id}", class: "jr-grid__item",
                                div {
                                    class: "jr-video-library__cell",
                                    "data-az-index": "{i}",
                                    onmounted: {
                                        let cells = cells.clone();
                                        move |e: MountedEvent| {
                                            cells.borrow_mut().insert(i, e.data());
                                        }
                                    },
                                    ItemPoster { item: item.clone() }
                                }
                            }
                        }
                    }
                    div {
                        class: "jr-video-library__sentinel",
                        "aria-hidden": "true",
                        onmounted: move |e| sentinel.set(Some(e.data())),
                    }
                    if fetching() {
                        div { class: "jr-video-library__loading", Spinner {} }
                    }
                    if sort() == SortKey::Name {
                        nav { class: "jr-video-az", "aria-label": t.t("video:library.jumpTo"),
                            for l in LETTERS {
                                button {
                                    key: "{l}",
                                    r#type: "button",
                                    onmousedown: move |e| e.stop_propagation(),
                                    onclick: move |_| pending_letter.set(Some(l)),
                                    "{l}"
                                }
                            }
                        }
                    }
                }
            }
            div {
                "aria-hidden": "true",
                style: "position: fixed; left: 0; bottom: 0; width: 1px; height: 1px; pointer-events: none;",
                onmounted: move |e| probe.set(Some(e.data())),
            }
        }
    }
}
