//! Paginated lists (`useList` infinite queries), sort preferences (`useSortPref`) and the
//! sort menu button of `LibraryPage.tsx`.
use std::future::Future;
use std::rc::Rc;

use dioxus::prelude::*;
use orphyn_jellyfin::keys::Key;
use orphyn_jellyfin::{BaseItemDto, ItemsPage, JfResult};
use serde::{Deserialize, Serialize};

use crate::design::{use_overlay, IconButton, MenuItem};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::use_services;

/// One entry of a sort menu (`SortOption`).
#[derive(Clone, PartialEq)]
pub struct SortOption {
    pub id: &'static str,
    pub label: String,
    pub sort_by: &'static [&'static str],
    pub default_order: &'static str,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct SortState {
    id: String,
    order: String,
}

/// Sort state persisted per page (`orphyn.music.sort.<key>`).
#[derive(Clone, Copy)]
#[derive(PartialEq)]
pub struct SortPref {
    state: Signal<(String, String)>,
    key: &'static str,
}

impl SortPref {
    /// The sort fields of the current option (reads the signal: the caller re-renders on change).
    pub fn sort_by(&self, options: &[SortOption]) -> Vec<String> {
        let id = self.state.read().0.clone();
        let cur = options.iter().find(|o| o.id == id).unwrap_or(&options[0]);
        cur.sort_by.iter().map(|s| s.to_string()).collect()
    }

    pub fn order(&self) -> String {
        self.state.read().1.clone()
    }
}

pub fn use_sort_pref(key: &'static str, options: &[SortOption]) -> SortPref {
    let svc = use_services();
    let first = options[0].clone();
    let saved: Option<SortState> = svc.storage.get_json(&format!("orphyn.music.sort.{key}"));
    let initial = match saved.filter(|s| options.iter().any(|o| o.id == s.id)) {
        Some(s) => (
            s.id,
            if s.order == "Descending" {
                "Descending".to_string()
            } else {
                "Ascending".to_string()
            },
        ),
        None => (first.id.to_string(), first.default_order.to_string()),
    };
    SortPref {
        state: use_signal(|| initial),
        key,
    }
}

fn menu_action(label: String, checked: bool, on_select: impl FnMut(()) + 'static) -> MenuItem {
    MenuItem::Action {
        label,
        icon: None,
        checked: Some(checked),
        destructive: false,
        disabled: false,
        on_select: Callback::new(on_select),
    }
}

/// Opens a sort menu under a click: sort fields, a separator, then ↑ / ↓.
pub fn open_sort_menu(
    overlay: crate::design::Overlay,
    x: f64,
    y: f64,
    cur: (String, String),
    fields: Vec<(String, String, String, bool)>,
    on_pick: Callback<(String, String)>,
) {
    let (cur_id, cur_order) = cur;
    let mut items: Vec<MenuItem> = fields
        .into_iter()
        .map(|(id, label, order, checked)| {
            menu_action(label, checked, move |_| on_pick.call((id.clone(), order.clone())))
        })
        .collect();
    items.push(MenuItem::Separator);
    for order in ["Ascending", "Descending"] {
        let id = cur_id.clone();
        items.push(menu_action(
            if order == "Ascending" { "↑" } else { "↓" }.to_string(),
            cur_order == order,
            move |_| on_pick.call((id.clone(), order.to_string())),
        ));
    }
    overlay.menu((x - 190.0).max(8.0), y + 16.0, items);
}

/// The `MenuButton` over a [`SortPref`].
#[component]
pub fn SortMenu(pref: SortPref, options: Vec<SortOption>) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let overlay = use_overlay();
    rsx! {
        IconButton {
            icon: "arrow-up-down",
            label: t.t("music:library.sort"),
            onclick: move |e: MouseEvent| {
                let cur = pref.state.peek().clone();
                let fields = options
                    .iter()
                    .map(|o| (o.id.to_string(), o.label.clone(), o.default_order.to_string(), o.id == cur.0))
                    .collect();
                let svc = svc.clone();
                let mut state = pref.state;
                let on_pick = Callback::new(move |(id, order): (String, String)| {
                    svc.storage.set_json(
                        &format!("orphyn.music.sort.{}", pref.key),
                        &SortState { id: id.clone(), order: order.clone() },
                    );
                    state.set((id, order));
                });
                let p = e.client_coordinates();
                open_sort_menu(overlay, p.x, p.y, cur, fields, on_pick);
            },
        }
    }
}

/// An accumulating paginated query: page `n` goes through the query cache, pages stay appended
/// until the key changes.
#[derive(Clone, Copy)]
#[derive(PartialEq)]
pub struct Paged {
    pub items: Signal<Vec<BaseItemDto>>,
    pub total: Signal<Option<usize>>,
    pub loading: Signal<bool>,
    pub error: Signal<bool>,
    pub fetching_more: Signal<bool>,
    next_start: Signal<usize>,
    done: Signal<bool>,
    generation: Signal<u64>,
    key: Signal<Key>,
}

impl Paged {
    pub fn has_next(&self) -> bool {
        !*self.done.read()
    }

    /// Fetches the next page (no-op while one is running or at the end).
    pub fn fetch_more<F, Fut>(mut self, fetch: F)
    where
        F: Fn(usize) -> Fut + 'static,
        Fut: Future<Output = JfResult<ItemsPage>> + 'static,
    {
        if *self.fetching_more.peek() || *self.done.peek() || *self.loading.peek() {
            return;
        }
        let start = *self.next_start.peek();
        let generation = *self.generation.peek();
        self.fetching_more.set(true);
        let fut = fetch(start);
        spawn(async move {
            let r = fut.await;
            if *self.generation.peek() != generation {
                return;
            }
            self.fetching_more.set(false);
            match r {
                Ok(page) => self.append(page, start),
                Err(_) => self.error.set(true),
            }
        });
    }

    fn append(mut self, page: ItemsPage, start: usize) {
        let n = page.items.len();
        self.items.write().extend(page.items);
        self.total.set(Some(page.total));
        self.next_start.set(start + n);
        self.done.set(n == 0 || start + n >= page.total);
    }
}

/// `useList`: loads the first page whenever `key` changes.
pub fn use_paged<F, Fut>(key: Key, fetch: F) -> Paged
where
    F: Fn(usize) -> Fut + 'static,
    Fut: Future<Output = JfResult<ItemsPage>> + Send + 'static,
{
    let svc = use_services();
    let paged = Paged {
        items: use_signal(Vec::new),
        total: use_signal(|| None),
        loading: use_signal(|| true),
        error: use_signal(|| false),
        fetching_more: use_signal(|| false),
        next_start: use_signal(|| 0),
        done: use_signal(|| false),
        generation: use_signal(|| 0),
        key: use_signal(Vec::new),
    };
    if *paged.key.peek() != key {
        let mut p = paged;
        p.key.set(key.clone());
        let generation = *p.generation.peek() + 1;
        p.generation.set(generation);
        p.items.set(Vec::new());
        p.total.set(None);
        p.loading.set(true);
        p.error.set(false);
        p.fetching_more.set(false);
        p.next_start.set(0);
        p.done.set(false);
        let cache = svc.cache.clone();
        let fut = fetch(0);
        spawn(async move {
            let r = cache
                .fetch(
                    page_key(&key, 0),
                    orphyn_jellyfin::keys::stale::CATALOG,
                    move || fut,
                )
                .await;
            if *p.generation.peek() != generation {
                return;
            }
            p.loading.set(false);
            match r {
                Ok(page) => p.append((*page).clone(), 0),
                Err(_) => p.error.set(true),
            }
        });
    }
    paged
}

pub fn page_key(base: &Key, start: usize) -> Key {
    let mut k = base.clone();
    k.push(format!("start:{start}"));
    k
}

/// `useEndReached`: calls `on_reach` once the sentinel is within ~2 screens of the viewport.
/// Blitz has no IntersectionObserver, so the page scroller offset triggers a measure.
pub fn use_end_reached(
    len: usize,
    on_reach: impl Fn() + Clone + 'static,
) -> Callback<Rc<MountedData>> {
    let router = use_router();
    let mut el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let scroll = (router.scroll_y)();
    use_effect(use_reactive!(|len, scroll| {
        let _ = (len, scroll);
        if let Some(e) = el.peek().clone() {
            let on_reach = on_reach.clone();
            spawn(async move {
                if let Ok(r) = e.get_client_rect().await {
                    if r.origin.y < 2400.0 {
                        on_reach();
                    }
                }
            });
        }
    }));
    Callback::new(move |m| el.set(Some(m)))
}
