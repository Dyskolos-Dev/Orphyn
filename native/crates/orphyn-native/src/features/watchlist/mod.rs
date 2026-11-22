//! The watchlist ("À regarder"): the list itself (shared state, kept in the server's display
//! preferences and cached locally), the bookmark button, the reminder for new episodes and the
//! `/video/watchlist` page. Format and detection live in `orphyn_jellyfin::api::watchlist`.
mod page;

use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::api::watchlist::{new_episodes, now_ms, EpisodeInfo, Watchlist};
use orphyn_jellyfin::BaseItemDto;

use crate::design::toast::{use_toasts, ToastOptions};
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::{use_app, use_services, Services};

pub use page::WatchlistPage;

/// How often the library is checked for new episodes while the app stays open.
const CHECK_EVERY: Duration = Duration::from_secs(3 * 60 * 60);
/// Episodes fetched per check (the newest unplayed ones of the whole library).
const CHECK_WINDOW: u32 = 200;
/// Toasts per check; the rest is in the badge and on the page.
const MAX_TOASTS: usize = 2;

/// Lucide `bookmark` (not in the generated icon set, which this feature does not own).
const BOOKMARK: &str = "<path d=\"m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z\"/>";

#[component]
pub fn BookmarkIcon(
    #[props(default = 20)] size: u32,
    #[props(default = false)] fill: bool,
) -> Element {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\" fill=\"{}\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" class=\"lucide\" aria-hidden=\"true\">{BOOKMARK}</svg>",
        if fill { "currentColor" } else { "none" },
    );
    rsx! { span { class: "jr-icon-host", dangerous_inner_html: svg } }
}

/// Per account, per device: when the library was last checked and when the person last looked at
/// the new episodes.
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
struct Marks {
    check: i64,
    seen: i64,
}

fn list_key(account: &str) -> String {
    format!("orphyn.watchlist.{account}")
}
fn marks_key(account: &str) -> String {
    format!("orphyn.watchlist.marks.{account}")
}

/// The shared watchlist state (`use_watchlist`).
#[derive(Clone)]
pub struct WatchlistStore {
    list: Signal<Watchlist>,
    /// Unplayed episodes of the last check, whatever their date.
    episodes: Signal<Vec<EpisodeInfo>>,
    marks: Signal<Marks>,
    svc: Services,
    app: crate::state::AppState,
}

pub fn use_watchlist_provider() -> WatchlistStore {
    let svc = use_services();
    let app = use_app();
    use_context_provider(|| WatchlistStore {
        list: Signal::new(Watchlist::default()),
        episodes: Signal::new(Vec::new()),
        marks: Signal::new(Marks::default()),
        svc,
        app,
    })
}

pub fn use_watchlist() -> WatchlistStore {
    use_context::<WatchlistStore>()
}

impl WatchlistStore {
    pub fn contains(&self, id: &str) -> bool {
        self.list.read().contains(id)
    }

    pub fn list(&self) -> Watchlist {
        self.list.read().clone()
    }

    /// Episodes added since the person last looked, of series on the list.
    pub fn fresh(&self) -> Vec<EpisodeInfo> {
        new_episodes(&self.list.read(), &self.episodes.read(), self.marks.read().seen)
    }

    /// The person opened the new episodes: the badge goes away until the next one arrives.
    pub fn mark_seen(&self) {
        let mut marks = self.marks;
        let now = now_ms();
        if marks.peek().seen >= now {
            return;
        }
        marks.with_mut(|m| m.seen = now);
        self.save_marks();
    }

    fn account(&self) -> Option<String> {
        (self.app.session)().map(|s| s.account_id)
    }

    fn save_local(&self) {
        if let Some(a) = self.account() {
            self.svc
                .storage
                .set(&list_key(&a), &self.list.peek().encode());
        }
    }

    fn save_marks(&self) {
        if let Some(a) = self.account() {
            self.svc.storage.set_json(&marks_key(&a), &*self.marks.peek());
        }
    }

    /// Adds or removes a title (a film or a series): the UI flips at once, then the server list
    /// is read, changed the same way and written back, so another device's changes are kept.
    pub fn set(&self, item: &BaseItemDto, on: bool) {
        let kind = if item.is_type("Series") { "Series" } else { "Movie" };
        if item.id.is_empty() || !(item.is_type("Series") || item.is_type("Movie")) {
            return;
        }
        self.apply(&item.id, kind, on);
    }

    pub fn toggle(&self, item: &BaseItemDto) {
        self.set(item, !self.contains(&item.id));
    }

    pub fn remove(&self, id: &str) {
        if self.contains(id) {
            self.apply(id, "", false);
        }
    }

    fn apply(&self, id: &str, kind: &str, on: bool) {
        let mut list = self.list;
        let now = now_ms();
        list.with_mut(|l| {
            if on {
                l.add(id, kind, now)
            } else {
                l.remove(id)
            }
        });
        self.save_local();
        let Some(session) = (self.app.session)() else {
            return;
        };
        let (id, kind) = (id.to_string(), kind.to_string());
        let this = self.clone();
        spawn(async move {
            let mut remote = match session.watchlist_fetch().await {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("[watchlist] read failed: {e}");
                    return;
                }
            };
            if on {
                remote.add(&id, &kind, now);
            } else {
                remote.remove(&id);
            }
            if let Err(e) = session.watchlist_store(&remote).await {
                log::warn!("[watchlist] write failed: {e}");
                return;
            }
            let mut list = this.list;
            list.set(remote);
            this.save_local();
        });
    }

    /// Local cache first (instant), then the server list.
    async fn load(&self) {
        let Some(session) = (self.app.session)() else {
            return;
        };
        let acc = session.account_id.clone();
        let mut list = self.list;
        let mut marks = self.marks;
        list.set(
            self.svc
                .storage
                .get(&list_key(&acc))
                .map(|r| Watchlist::decode(&r))
                .unwrap_or_default(),
        );
        marks.set(self.svc.storage.get_json(&marks_key(&acc)).unwrap_or_default());
        match session.watchlist_fetch().await {
            Ok(remote) => {
                list.set(remote);
                self.save_local();
            }
            Err(e) => log::warn!("[watchlist] sync failed: {e}"),
        }
    }

    /// One reminder pass: refreshes the candidates and returns the episodes to announce.
    async fn check(&self) -> Vec<EpisodeInfo> {
        let Some(session) = (self.app.session)() else {
            return Vec::new();
        };
        if self.list.peek().items.iter().all(|e| e.kind != "Series") {
            return Vec::new();
        }
        let eps = match session.recent_unplayed_episodes(CHECK_WINDOW).await {
            Ok(e) => e,
            Err(e) => {
                log::warn!("[watchlist] episode check failed: {e}");
                return Vec::new();
            }
        };
        let mut episodes = self.episodes;
        episodes.set(eps);
        let since = self.marks.peek().check;
        let mut marks = self.marks;
        marks.with_mut(|m| m.check = now_ms());
        self.save_marks();
        self.fresh()
            .into_iter()
            .filter(|e| e.created_ms > since)
            .collect()
    }
}

/// Mounted once in the shell: syncs the list when an account is active, then checks for new
/// episodes at start and every few hours (toast + badge).
#[component]
pub fn WatchlistHost() -> Element {
    let store = use_watchlist();
    let t = use_i18n();
    let router = use_router();
    let toasts = use_toasts();
    let account = (store.app.session)().map(|s| s.account_id);
    use_resource(use_reactive!(|account| {
        let store = store.clone();
        async move {
            if account.is_none() {
                return;
            }
            store.load().await;
            loop {
                for e in store.check().await.into_iter().take(MAX_TOASTS) {
                    let label = match (e.season, e.episode) {
                        (Some(s), Some(n)) => format!("S{s}E{n}"),
                        _ => String::new(),
                    };
                    let path = crate::features::video::browse::item_path(&e.id);
                    let mut o = ToastOptions::new(t.tf(
                        "video:watchlist.newEpisode",
                        &[("series", e.series_name.clone()), ("label", label)],
                    ));
                    o.icon = Some("tv");
                    o.duration = Some(Duration::from_secs(12));
                    o.action = Some((
                        t.t("video:watchlist.open"),
                        Callback::new(move |_| router.push(&path)),
                    ));
                    toasts.show(o);
                }
                tokio::time::sleep(CHECK_EVERY).await;
            }
        }
    }));
    rsx! {}
}

/// Bookmark toggle of the item page, shaped like the other `glass` icon buttons of the banner.
#[component]
pub fn WatchlistButton(item: BaseItemDto) -> Element {
    let t = use_i18n();
    let store = use_watchlist();
    let on = store.contains(&item.id);
    let label = if on {
        t.t("video:watchlist.remove")
    } else {
        t.t("video:watchlist.add")
    };
    rsx! {
        button {
            r#type: "button",
            title: "{label}",
            "aria-label": "{label}",
            "aria-pressed": on.to_string(),
            class: if on {
                "jr-icon-button jr-icon-button--glass jr-icon-button--large jr-icon-button--pressed"
            } else {
                "jr-icon-button jr-icon-button--glass jr-icon-button--large"
            },
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |_| store.toggle(&item),
            span { class: "jr-icon-button__icon", "aria-hidden": "true",
                BookmarkIcon { size: 28, fill: on }
            }
        }
    }
}
