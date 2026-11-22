//! /video/watchlist: the films and series marked "to watch", the new episodes of the listed series
//! on top, a sort menu. Removing goes through the card menu or the bookmark of the item page.
use dioxus::prelude::*;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::BaseItemDto;

use super::use_watchlist;
use crate::design::{EmptyState, MenuItem, Shelf};
use crate::features::video::browse::{Grid, ItemLandscape, ItemPoster, MenuButton, Skeleton};
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::shell::ShellPage;
use crate::state::use_app;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Added,
    Title,
    Release,
}

/// Newest first for dates, A to Z for titles.
fn sorted(mut rows: Vec<(BaseItemDto, i64)>, by: Sort) -> Vec<BaseItemDto> {
    match by {
        Sort::Added => rows.sort_by_key(|a| std::cmp::Reverse(a.1)),
        Sort::Title => rows.sort_by_key(|(i, _)| i.name().to_lowercase()),
        Sort::Release => rows.sort_by(|a, b| {
            let key = |i: &BaseItemDto| {
                i.premiere_date
                    .clone()
                    .unwrap_or_else(|| format!("{:04}", i.production_year.unwrap_or(0)))
            };
            key(&b.0).cmp(&key(&a.0))
        }),
    }
    rows.into_iter().map(|(i, _)| i).collect()
}

#[component]
pub fn WatchlistPage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let store = use_watchlist();
    let mut by = use_signal(|| Sort::Added);
    // Leaving the page counts as having seen the new episodes (the badge goes away).
    use_drop({
        let store = store.clone();
        move || store.mark_seen()
    });

    let list = store.list();
    let fresh = store.fresh();
    let mut ids: Vec<String> = list.items.iter().map(|e| e.id.clone()).collect();
    ids.extend(fresh.iter().map(|e| e.id.clone()));
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let mut key = keys::all(&acc);
    key.push("watchlist".into());
    key.push(ids.join(","));
    let query = use_query(key, stale::PROGRESS, {
        let ids = ids.clone();
        move || {
            let (s, ids) = (session.clone(), ids.clone());
            async move {
                match s {
                    Some(s) => s.items_by_ids(&ids).await,
                    None => Ok(Vec::new()),
                }
            }
        }
    });
    let loaded: Vec<BaseItemDto> = query.data.as_deref().cloned().unwrap_or_default();
    let title = t.t("video:watchlist.title");

    let rows: Vec<(BaseItemDto, i64)> = list
        .items
        .iter()
        .filter_map(|e| {
            loaded
                .iter()
                .find(|i| i.id == e.id)
                .map(|i| (i.clone(), e.added_ms))
        })
        .collect();
    let items = sorted(rows, by());
    let episodes: Vec<BaseItemDto> = fresh
        .iter()
        .filter_map(|f| loaded.iter().find(|i| i.id == f.id).cloned())
        .collect();

    let sort_items: Vec<MenuItem> = [
        (Sort::Added, "video:watchlist.sortAdded"),
        (Sort::Title, "video:watchlist.sortTitle"),
        (Sort::Release, "video:watchlist.sortRelease"),
    ]
    .into_iter()
    .map(|(s, label)| MenuItem::Action {
        label: t.t(label),
        icon: None,
        checked: Some(by() == s),
        destructive: false,
        disabled: false,
        on_select: Callback::new(move |_| by.set(s)),
    })
    .collect();
    let actions = rsx! {
        MenuButton { label: t.t("video:watchlist.sort"), icon: "arrow-up-down", items: sort_items }
    };
    let subtitle = Some(t.tc("video:watchlist.count", list.items.len() as i64, &[]));
    let loading = query.is_loading() && !list.items.is_empty();

    rsx! {
        ShellPage { title: title.clone(), subtitle, actions,
            if list.items.is_empty() {
                EmptyState {
                    icon: "clock",
                    title: t.t("video:watchlist.empty.title"),
                    message: rsx! { {t.t("video:watchlist.empty.message")} },
                }
            } else if loading {
                div { class: "jr-video-section",
                    Grid {
                        for i in 0..12 {
                            li { key: "{i}", class: "jr-grid__item",
                                Skeleton { aspect_ratio: "2 / 3", radius: "var(--radius-poster)" }
                            }
                        }
                    }
                }
            } else {
                if !episodes.is_empty() {
                    Shelf { title: t.t("video:watchlist.newEpisodes"), kind: "landscape",
                        for e in episodes {
                            li { key: "{e.id}", class: "jr-shelf__item",
                                ItemLandscape { item: e.clone(), show_progress: false }
                            }
                        }
                    }
                }
                div { class: "jr-video-section jr-watchlist",
                    Grid { label: title.clone(),
                        for item in items {
                            li { key: "{item.id}", class: "jr-grid__item",
                                div { class: "jr-video-library__cell", ItemPoster { item: item.clone() } }
                            }
                        }
                    }
                }
            }
        }
    }
}
