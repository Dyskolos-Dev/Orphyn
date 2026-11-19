//! Settings > Home (`HomePane.tsx`, `HomeEditor.tsx`, `HomeMiniature.tsx`): arrange the Movies & TV
//! and Music home screens. Works on the stored JSON (same key and shape as React:
//! `orphyn.home.<scope>.<server>.<user>`), every change is saved at once.
use dioxus::prelude::*;
use serde_json::{json, Value};

use super::ui::GrayButton;
use crate::design::overlay::{use_overlay, MenuItem};
use crate::design::{Button, IconButton, Segment, SegmentedControl};
use crate::features::servers::Sheet;
use crate::features::video::browse::layout as video_layout;
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::state::{use_app, use_services};
use orphyn_jellyfin::keys::{self, stale};

/// (type, unique, takes a param, default count, sized, icon)
type Def = (&'static str, bool, bool, usize, bool, &'static str);

const VIDEO: &[Def] = &[
    ("hero", true, false, 8, false, "clapperboard"),
    ("continue", true, false, 20, true, "play"),
    ("nextUp", true, false, 20, true, "tv"),
    ("latest", false, true, 20, true, "library"),
    ("favourites", true, false, 30, true, "heart"),
    ("genre", false, true, 20, true, "tags"),
    ("collection", false, true, 20, true, "folder"),
    ("because", true, false, 20, true, "sparkles"),
    ("released", true, false, 20, true, "flame"),
    ("random", true, false, 20, true, "dices"),
];
const MUSIC: &[Def] = &[
    ("recentlyPlayed", true, false, 20, true, "history"),
    ("recentlyAdded", true, false, 20, true, "clock"),
    ("favAlbums", true, false, 20, true, "heart"),
    ("favArtists", true, false, 20, true, "user"),
    ("favSongs", true, false, 20, true, "music-2"),
    ("playlists", true, false, 20, true, "list-music"),
    ("instantMix", true, false, 20, true, "radio"),
    ("genre", false, true, 20, true, "tags"),
    ("artist", false, true, 20, true, "disc-3"),
    ("randomAlbums", true, false, 20, true, "dices"),
    ("mixForYou", true, false, 10, true, "sparkles"),
    ("mixDaily", true, false, 10, true, "sparkles"),
    ("mixGenres", true, false, 10, true, "tags"),
];
const MUSIC_DEFAULT: &[&str] = &[
    "recentlyPlayed",
    "mixForYou",
    "mixDaily",
    "recentlyAdded",
    "instantMix",
    "mixGenres",
    "favAlbums",
    "favArtists",
    "playlists",
];
const COUNTS: [usize; 7] = [5, 10, 15, 20, 30, 40, 60];

fn defs(scope: &str) -> &'static [Def] {
    if scope == "music" {
        MUSIC
    } else {
        VIDEO
    }
}

fn def_of(scope: &str, kind: &str) -> Option<&'static Def> {
    defs(scope).iter().find(|d| d.0 == kind)
}

fn str_of<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("")
}

fn count_of(scope: &str, row: &Value) -> usize {
    row.get("count")
        .and_then(Value::as_u64)
        .map(|c| c as usize)
        .or_else(|| def_of(scope, str_of(row, "type")).map(|d| d.3))
        .unwrap_or(20)
}

fn row_json(kind: &str, param: Option<(&str, &str)>) -> Value {
    let mut r = json!({"id": kind, "type": kind, "hidden": false});
    if let Some((p, label)) = param {
        r["id"] = json!(format!("{kind}:{p}"));
        r["param"] = json!(p);
        r["label"] = json!(label);
    }
    r
}

fn load(
    scope: &str,
    key: &str,
    svc: &crate::state::Services,
    libs: &[(String, String)],
) -> Vec<Value> {
    if scope == "video" {
        let raw = svc.storage.get(key);
        let l = video_layout::sync_libraries(video_layout::parse_layout(raw.as_deref()), libs);
        return video_layout::to_json(&l)["rows"]
            .as_array()
            .cloned()
            .unwrap_or_default();
    }
    let stored: Option<Value> = svc.storage.get_json(key);
    stored
        .as_ref()
        .and_then(|v| v.get("rows"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|r| def_of("music", str_of(r, "type")).is_some())
                .cloned()
                .collect()
        })
        .unwrap_or_else(|| MUSIC_DEFAULT.iter().map(|k| row_json(k, None)).collect())
}

fn save(scope: &str, key: &str, svc: &crate::state::Services, rows: &[Value], libs: &[(String, String)]) {
    let mut o = json!({"version": 2, "rows": rows});
    if scope == "video" {
        // Every library is known now, so a row the person removed stays removed.
        o["knownLibraries"] = json!(libs.iter().map(|l| l.0.clone()).collect::<Vec<_>>());
    }
    svc.storage.set_json(key, &o);
}

fn row_title(scope: &str, row: &Value, t: &crate::i18n::I18n) -> String {
    let kind = str_of(row, "type");
    if kind.starts_with("mix") {
        return t.t(&format!("mixes:rows.{kind}.title"));
    }
    let name = row
        .get("label")
        .and_then(Value::as_str)
        .or_else(|| row.get("param").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    t.tf(&format!("home:rows.{scope}.{kind}.title"), &[("name", name)])
}

#[component]
pub fn HomePane() -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let overlay = use_overlay();
    let mut scope = use_signal(|| "video");
    let mut adding = use_signal(|| false);
    let session = (app.session)();
    let Some(session) = session else {
        return rsx! {};
    };
    let account = (app.accounts)()
        .into_iter()
        .find(|a| a.id == session.account_id);
    let Some(account) = account else {
        return rsx! {};
    };
    let views = use_query(keys::views(&session.account_id), stale::ITEM, {
        let s = session.clone();
        move || {
            let s = s.clone();
            async move { s.user_views().await }
        }
    });
    let libs: Vec<(String, String)> = views
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
    let sc = scope();
    let key = format!("orphyn.home.{sc}.{}.{}", account.server_id, account.user_id);
    let mut rows = use_signal(Vec::<Value>::new);
    let mut loaded = use_signal(String::new);
    let tag = format!("{key}|{}", libs.len());
    if loaded() != tag {
        loaded.set(tag);
        rows.set(load(sc, &key, &svc, &libs));
    }
    let commit = {
        let (key, svc, libs) = (key.clone(), svc.clone(), libs.clone());
        Callback::new(move |next: Vec<Value>| {
            save(sc, &key, &svc, &next, &libs);
            rows.set(next);
        })
    };
    let current = rows();
    let last = current.len().saturating_sub(1);
    let _ = overlay;
    rsx! {
        div { class: "jr-settings-home",
            SegmentedControl {
                label: t.t("settings:home.scope"),
                value: sc,
                on_change: move |v| scope.set(v),
                options: vec![
                    Segment { value: "video", label: t.t("settings:home.video") },
                    Segment { value: "music", label: t.t("settings:home.music") },
                ],
            }
            div { class: "jr-settings-home__body",
                div { class: "jr-settings-home__main",
                    div { class: "jr-home-edit",
                        p { class: "jr-home-edit__hint", {t.t("home:hint")} }
                        if current.is_empty() {
                            p { class: "jr-home-edit__empty", {t.t("home:empty")} }
                        } else {
                            ul { class: "jr-home-edit__list",
                                for (i , row) in current.iter().enumerate() {
                                    EditRow {
                                        key: "{str_of(row, \"id\")}",
                                        scope: sc,
                                        row: row.clone(),
                                        index: i,
                                        last: i == last,
                                        rows: current.clone(),
                                        on_change: {
                                            let commit = commit;
                                            move |n| commit.call(n)
                                        },
                                    }
                                }
                            }
                        }
                    }
                    div { class: "jr-settings-home__actions",
                        Button { variant: "filled", size: "small", icon: "plus", onclick: move |_| adding.set(true), {t.t("settings:home.addRow")} }
                        Button {
                            variant: "plain",
                            size: "small",
                            icon: "rotate-ccw",
                            onclick: {
                                let (key, svc, libs, commit) = (key.clone(), svc.clone(), libs.clone(), commit.clone());
                                move |_| {
                                    svc.storage.remove(&key);
                                    commit.call(load(sc, &key, &svc, &libs));
                                }
                            },
                            {t.t("settings:home.reset")}
                        }
                    }
                }
                Miniature { scope: sc, rows: current.clone() }
            }
            if adding() {
                AddRowSheet {
                    scope: sc,
                    rows: current.clone(),
                    libraries: if sc == "video" { Some(libs.clone()) } else { None },
                    on_close: move |_| adding.set(false),
                    on_add: {
                        let commit = commit.clone();
                        move |(row, rest): (Value, Vec<Value>)| {
                            let mut next = rest;
                            next.push(row);
                            commit.call(next);
                            adding.set(false);
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn EditRow(
    scope: &'static str,
    row: Value,
    index: usize,
    last: bool,
    rows: Vec<Value>,
    on_change: EventHandler<Vec<Value>>,
) -> Element {
    let t = use_i18n();
    let overlay = use_overlay();
    let kind = str_of(&row, "type").to_string();
    let id = str_of(&row, "id").to_string();
    let hidden = row.get("hidden").and_then(Value::as_bool) == Some(true);
    let def = def_of(scope, &kind);
    let title = row_title(scope, &row, &t);
    let size = str_of(&row, "size").to_string();
    let sub = if hidden {
        t.t("home:hiddenRow")
    } else {
        let mut s = t.tc("home:summary.items", count_of(scope, &row) as i64, &[]);
        if def.is_some_and(|d| d.4) && (size == "small" || size == "large") {
            s = format!("{s} · {}", t.t(&format!("home:size.{size}")));
        }
        s
    };
    let icon = def.map(|d| d.5).unwrap_or("music-2");
    let eye_icon: &'static str = if hidden { "eye-off" } else { "eye" };
    let edit = {
        let rows = rows.clone();
        move |f: &dyn Fn(&mut Vec<Value>)| {
            let mut next = rows.clone();
            f(&mut next);
            on_change.call(next);
        }
    };
    let (e_up, e_down, e_hide, e_menu) = (edit.clone(), edit.clone(), edit.clone(), edit);
    let (id_up, id_down, id_hide) = (id.clone(), id.clone(), id.clone());
    let cls = if hidden {
        "jr-home-row jr-home-row--hidden"
    } else {
        "jr-home-row"
    };
    rsx! {
        li { class: cls,
            div { class: "jr-home-row__main",
                span { class: "jr-home-row__handle", "aria-hidden": "true",
                    span { class: "jr-home-row__moves",
                        IconButton {
                            icon: "chevron-up",
                            size: "small",
                            disabled: index == 0,
                            label: t.tf("home:dragHandle", &[("title", title.clone())]),
                            onclick: move |_| e_up(&|r: &mut Vec<Value>| {
                                if let Some(i) = r.iter().position(|x| str_of(x, "id") == id_up) { if i > 0 { r.swap(i, i - 1); } }
                            }),
                        }
                        IconButton {
                            icon: "chevron-down",
                            size: "small",
                            disabled: last,
                            label: t.tf("home:dragHandle", &[("title", title.clone())]),
                            onclick: move |_| e_down(&|r: &mut Vec<Value>| {
                                if let Some(i) = r.iter().position(|x| str_of(x, "id") == id_down) { if i + 1 < r.len() { r.swap(i, i + 1); } }
                            }),
                        }
                    }
                }
                span { class: "jr-home-row__icon", "aria-hidden": "true", crate::design::Icon { name: icon, size: 18 } }
                span { class: "jr-home-row__text",
                    span { class: "jr-home-row__title", "{title}" }
                    span { class: "jr-home-row__sub", "{sub}" }
                }
                div { class: "jr-home-row__actions",
                    IconButton {
                        icon: eye_icon,
                        size: "small",
                        label: t.tf(if hidden { "home:show" } else { "home:hide" }, &[("title", title.clone())]),
                        onclick: move |_| e_hide(&|r: &mut Vec<Value>| {
                            if let Some(x) = r.iter_mut().find(|x| str_of(x, "id") == id_hide) { x["hidden"] = json!(!hidden); }
                        }),
                    }
                    IconButton {
                        icon: "ellipsis",
                        size: "small",
                        label: t.t("home:options"),
                        onclick: move |ev: MouseEvent| {
                            let mut items: Vec<MenuItem> = Vec::new();
                            if def.is_some_and(|d| d.4) {
                                items.push(MenuItem::Header(t.t("home:cardSize")));
                                for s in ["auto", "small", "large"] {
                                    let (e, id) = (e_menu.clone(), id.clone());
                                    let mut it = MenuItem::action(t.t(&format!("home:size.{s}")), None, move |_| {
                                        e(&|r: &mut Vec<Value>| {
                                            if let Some(x) = r.iter_mut().find(|x| str_of(x, "id") == id) {
                                                if s == "auto" { x.as_object_mut().map(|o| o.remove("size")); } else { x["size"] = json!(s); }
                                            }
                                        })
                                    });
                                    let cur = if size.is_empty() { "auto" } else { size.as_str() };
                                    if let MenuItem::Action { checked, .. } = &mut it { *checked = Some(cur == s); }
                                    items.push(it);
                                }
                                items.push(MenuItem::Separator);
                                items.push(MenuItem::Header(t.t("home:itemCount")));
                                for c in COUNTS {
                                    let (e, id) = (e_menu.clone(), id.clone());
                                    let mut it = MenuItem::action(c.to_string(), None, move |_| {
                                        e(&|r: &mut Vec<Value>| {
                                            if let Some(x) = r.iter_mut().find(|x| str_of(x, "id") == id) { x["count"] = json!(c); }
                                        })
                                    });
                                    if let MenuItem::Action { checked, .. } = &mut it { *checked = Some(count_of(scope, &row) == c); }
                                    items.push(it);
                                }
                                items.push(MenuItem::Separator);
                            }
                            let (e, id) = (e_menu.clone(), id.clone());
                            let mut rm = MenuItem::action(t.tf("home:remove", &[("title", title.clone())]), None, move |_| {
                                e(&|r: &mut Vec<Value>| r.retain(|x| str_of(x, "id") != id))
                            });
                            if let MenuItem::Action { destructive, .. } = &mut rm { *destructive = true; }
                            items.push(rm);
                            let c = ev.client_coordinates();
                            overlay.menu((c.x - 180.0).max(8.0), c.y + 12.0, items);
                        },
                    }
                }
            }
        }
    }
}

/// "Add a row": the row types not on the home yet; a library row is added per library.
#[component]
fn AddRowSheet(
    scope: &'static str,
    rows: Vec<Value>,
    libraries: Option<Vec<(String, String)>>,
    on_close: EventHandler<()>,
    on_add: EventHandler<(Value, Vec<Value>)>,
) -> Element {
    let t = use_i18n();
    let mut options: Vec<(String, String, String, Value, bool)> = Vec::new();
    for d in defs(scope) {
        let name_key = if d.0.starts_with("mix") {
            format!("mixes:rows.{}.name", d.0)
        } else {
            format!("home:rows.{scope}.{}.name", d.0)
        };
        let desc_key = name_key.replace(".name", ".description");
        if d.2 {
            if d.0 == "latest" {
                for (id, name) in libraries.clone().unwrap_or_default() {
                    let have = rows.iter().any(|r| str_of(r, "type") == "latest" && str_of(r, "param") == id);
                    options.push((
                        t.tf("home:rows.video.latest.title", &[("name", name.clone())]),
                        t.t(&desc_key),
                        d.5.to_string(),
                        row_json("latest", Some((&id, &name))),
                        have,
                    ));
                }
            }
            continue;
        }
        let have = rows.iter().any(|r| str_of(r, "type") == d.0);
        options.push((t.t(&name_key), t.t(&desc_key), d.5.to_string(), row_json(d.0, None), have));
    }
    rsx! {
        Sheet { title: t.t("home:addRow"), description: t.t("home:addRowHint"), on_close: move |_| on_close.call(()),
            ul { class: "jr-home-edit__list",
                for (name , desc , icon , row , have) in options {
                    li { class: "jr-home-row", key: "{str_of(&row, \"id\")}",
                        div { class: "jr-home-row__main",
                            span { class: "jr-home-row__icon", "aria-hidden": "true", crate::design::Icon { name: Box::leak(icon.into_boxed_str()) as &'static str, size: 18 } }
                            span { class: "jr-home-row__text",
                                span { class: "jr-home-row__title", "{name}" }
                                span { class: "jr-home-row__sub", if have { {t.t("home:alreadyAdded")} } else { "{desc}" } }
                            }
                            GrayButton {
                                onclick: {
                                    let rows = rows.clone();
                                    move |_| if !have { on_add.call((row.clone(), rows.clone())) }
                                },
                                {t.t("home:addRow")}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `HomeMiniature`: a small, live drawing of the home.
#[component]
fn Miniature(scope: &'static str, rows: Vec<Value>) -> Element {
    let t = use_i18n();
    let shown: Vec<&Value> = rows
        .iter()
        .filter(|r| r.get("hidden").and_then(Value::as_bool) != Some(true))
        .collect();
    rsx! {
        figure { class: "jr-mini", "aria-label": t.t("settings:home.preview"),
            div { class: "jr-mini__screen", "aria-hidden": "true",
                for row in shown {
                    if str_of(row, "type") == "hero" {
                        div { key: "{str_of(row, \"id\")}", class: "jr-mini__hero" }
                    } else {
                        div { key: "{str_of(row, \"id\")}", class: "jr-mini__row",
                            span { class: "jr-mini__label" }
                            span { class: "jr-mini__cards",
                                for i in 0..count_of(scope, row).min(6) {
                                    span { key: "{i}", class: "jr-mini__card" }
                                }
                            }
                        }
                    }
                }
            }
            figcaption { class: "jr-mini__caption", {t.t("settings:home.previewCaption")} }
        }
    }
}
