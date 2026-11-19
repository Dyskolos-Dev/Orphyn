//! `ShellSidebar` + `Sidebar` / `SidebarSection` / `SidebarItem` of the design system.
use dioxus::prelude::*;

use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::ItemsParams;

use crate::design::{Icon, IconButton};
use crate::features::servers::ServerSwitcher;
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::router::{use_router, Route};
use crate::state::{use_app, use_services};

pub struct SidebarEntry {
    pub id: &'static str,
    pub section: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
    pub to: &'static str,
}

/// `BUILTIN_SIDEBAR_ITEMS` of `ShellSidebar.tsx` (same order). Features add theirs (stats...) in
/// their own port, after these.
pub const BUILTIN_SIDEBAR_ITEMS: &[SidebarEntry] = &[
    SidebarEntry {
        id: "watch.home",
        section: "watch",
        label_key: "shell:nav.home",
        icon: "circle-play",
        to: "/video",
    },
    SidebarEntry {
        id: "watch.movies",
        section: "watch",
        label_key: "shell:nav.movies",
        icon: "clapperboard",
        to: "/video/library/movies",
    },
    SidebarEntry {
        id: "watch.series",
        section: "watch",
        label_key: "shell:nav.series",
        icon: "tv",
        to: "/video/library/series",
    },
    SidebarEntry {
        id: "music.home",
        section: "music",
        label_key: "shell:nav.home",
        icon: "house",
        to: "/music",
    },
    SidebarEntry {
        id: "music.albums",
        section: "music",
        label_key: "shell:nav.albums",
        icon: "disc-3",
        to: "/music/albums",
    },
    SidebarEntry {
        id: "music.artists",
        section: "music",
        label_key: "shell:nav.artists",
        icon: "mic-vocal",
        to: "/music/artists",
    },
    SidebarEntry {
        id: "music.playlists",
        section: "music",
        label_key: "shell:nav.playlists",
        icon: "list-music",
        to: "/music/playlists",
    },
    SidebarEntry {
        id: "music.favorites",
        section: "music",
        label_key: "music:nav.favorites",
        icon: "heart",
        to: "/music/favorites",
    },
    SidebarEntry {
        id: "music.songs",
        section: "music",
        label_key: "shell:nav.songs",
        icon: "music",
        to: "/music/songs",
    },
    SidebarEntry {
        id: "stats.stats",
        section: "more",
        label_key: "stats:nav",
        icon: "bar-chart-3",
        to: "/stats",
    },
];

/// The user's playlists under "Music" (`PlaylistHost.tsx` registers them as sidebar items).
#[component]
fn PlaylistEntries() -> Element {
    let app = use_app();
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let params = ItemsParams {
        include_item_types: vec!["Playlist".into()],
        media_types: vec!["Audio".into()],
        sort_by: vec!["SortName".into()],
        sort_order: Some("Ascending".into()),
        ..ItemsParams::default()
    };
    let list = use_query(keys::items(&session.account_id, &params), stale::ITEM, {
        let (s, p) = (session.clone(), params.clone());
        move || {
            let (s, p) = (s.clone(), p.clone());
            async move { s.items_page(&p, 0).await }
        }
    });
    let items = list
        .data
        .as_deref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    rsx! {
        for p in items {
            SidebarItem {
                key: "playlist.{p.id}",
                icon: "list-music",
                label: p.name().to_string(),
                to: format!("/music/playlist/{}", p.id),
            }
        }
    }
}

#[component]
fn SidebarItem(icon: &'static str, label: String, to: String) -> Element {
    let router = use_router();
    let active = router.route().is_active_link(&to);
    let cls = if active {
        "jr-sidebar-item__link jr-sidebar-item__link--active"
    } else {
        "jr-sidebar-item__link"
    };
    rsx! {
        li { class: "jr-sidebar-item",
            a {
                class: "{cls}",
                "aria-current": active.then_some("page"),
                onclick: move |e| { e.prevent_default(); router.push(&to) },
                span { class: "jr-sidebar-item__icon", "aria-hidden": "true", Icon { name: icon, size: 18 } }
                span { class: "jr-sidebar-item__label", "{label}" }
            }
        }
    }
}

#[component]
fn SidebarSection(title: String, children: Element) -> Element {
    let mut collapsed = use_signal(|| false);
    let chevron = if collapsed() {
        "jr-sidebar-section__chevron jr-sidebar-section__chevron--collapsed"
    } else {
        "jr-sidebar-section__chevron"
    };
    rsx! {
        section { class: "jr-sidebar-section",
            h2 { class: "jr-sidebar-section__title",
                button {
                    r#type: "button",
                    class: "jr-sidebar-section__toggle",
                    "aria-expanded": (!collapsed()).to_string(),
                    onclick: move |_| collapsed.toggle(),
                    "{title}"
                    Icon { name: "chevron-down", size: 24, class: chevron.to_string() }
                }
            }
            ul { class: "jr-sidebar-section__list", hidden: collapsed(), {children} }
        }
    }
}

#[component]
pub fn ShellSidebar() -> Element {
    let t = use_i18n();
    let router = use_router();
    let app = use_app();
    let svc = use_services();
    let window = dioxus_native::use_window();
    // The field mirrors ?q= while on /search; typing navigates there.
    let url_query = match router.route() {
        Route::Search { q } => q,
        _ => String::new(),
    };
    let mut draft = use_signal(|| url_query.clone());
    let mut synced = use_signal(|| url_query.clone());
    if *synced.peek() != url_query {
        synced.set(url_query.clone());
        draft.set(url_query);
    }
    let items = |section: &str| {
        BUILTIN_SIDEBAR_ITEMS
            .iter()
            .filter(|i| i.section == section)
            .map(|i| (i.id, i.icon, t.t(i.label_key), i.to.to_string()))
            .collect::<Vec<_>>()
    };
    let refresh_title = format!(
        "{} ({})",
        t.t("shell:refresh.label"),
        crate::platform_shortcut("R")
    );
    rsx! {
        nav { class: "jr-sidebar", "aria-label": t.t("shell:nav.label"),
            div {
                class: "jr-sidebar__titlebar jr-drag",
                "aria-hidden": "true",
                onmousedown: move |e| crate::window::drag_region_mousedown(&window, &e),
            }
            div { class: "jr-sidebar__header",
                div { class: "jr-field jr-field--medium jr-search-field",
                    div { class: "jr-field__box",
                        span { class: "jr-field__leading", "aria-hidden": "true", Icon { name: "search", size: 15, stroke_width: 2.2 } }
                        input {
                            class: "jr-field__input",
                            r#type: "search",
                            placeholder: t.t("shell:search.placeholder"),
                            "aria-label": t.t("shell:search.label"),
                            value: "{draft}",
                            oninput: move |e| {
                                let q = e.value();
                                draft.set(q.clone());
                                synced.set(q.clone());
                                let path = format!("/search?q={}", crate::router::encode(&q));
                                if matches!(router.route(), Route::Search { .. }) {
                                    router.replace(&path);
                                } else {
                                    router.push(&path);
                                }
                            },
                        }
                    }
                }
            }
            div { class: "jr-sidebar__scroll",
                SidebarSection { title: t.t("shell:nav.watch"),
                    for (id, icon, label, to) in items("watch") {
                        SidebarItem { key: "{id}", icon, label, to }
                    }
                }
                SidebarSection { title: t.t("shell:nav.music"),
                    for (id, icon, label, to) in items("music") {
                        SidebarItem { key: "{id}", icon, label, to }
                    }
                    PlaylistEntries {}
                }
                SidebarSection { title: t.t("shell:nav.more"),
                    for (id, icon, label, to) in items("more") {
                        SidebarItem { key: "{id}", icon, label, to }
                    }
                }
            }
            div { class: "jr-sidebar__footer",
                div { class: "jr-shell__footer-row",
                    div { class: "jr-shell__server-switcher", ServerSwitcher {} }
                    IconButton {
                        class: "jr-shell__refresh",
                        icon: "rotate-cw",
                        label: refresh_title,
                        size: "small",
                        onclick: move |_| {
                            // Marks every query of the account stale: the open screens refetch.
                            if let Some(s) = (app.session)() {
                                svc.cache.invalidate(&keys::all(&s.account_id));
                            }
                        },
                    }
                }
                ul { class: "jr-sidebar-section__list",
                    SidebarItem { icon: "settings", label: t.t("shell:nav.settings"), to: "/settings" }
                }
            }
        }
    }
}
