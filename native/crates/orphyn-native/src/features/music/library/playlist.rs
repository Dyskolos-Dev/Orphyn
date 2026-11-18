//! `pages/PlaylistPage.tsx`: playlist header (inline rename, delete), its songs and total.
use dioxus::prelude::*;
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::BaseItemDto;

use super::cards::container_menu;
use super::detail::DetailHeader;
use crate::design::{Button, EmptyState, MenuItem, Spinner};
use crate::features::music::format_total;
use crate::features::music::rows::SongRows;
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::{PageHeader, ShellPage};
use crate::state::{use_app, use_services};

#[component]
pub fn PlaylistPage(id: String) -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let router = use_router();
    let mut renaming = use_signal(|| false);
    let mut confirming = use_signal(|| false);
    let Some(session) = (app.session)() else {
        return rsx! {};
    };
    let acc = session.account_id.clone();
    let playlist = use_query(keys::item(&acc, &id), stale::ITEM, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.item(&id).await }
        }
    });
    let items = use_query(keys::playlist_items(&acc, &id), stale::ITEM, {
        let (s, id) = (session.clone(), id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.playlist_items(&id).await }
        }
    });
    if playlist.is_loading() || items.is_loading() {
        return rsx! {
            ShellPage { title: "", header: PageHeader::Inline,
                div { class: "jrm-center", Spinner { size: "large" } }
            }
        };
    }
    let Some(item) = playlist.data.clone().map(|p| (*p).clone()) else {
        return rsx! {
            ShellPage { title: t.t("music:nav.playlists"), header: PageHeader::Inline,
                EmptyState { icon: "list-music", title: t.t("music:playlist.notFound") }
            }
        };
    };
    let songs: Vec<BaseItemDto> = items.data.as_deref().cloned().unwrap_or_default();
    let total: f64 = songs.iter().map(|s| s.duration_seconds()).sum();
    let summary = (!songs.is_empty()).then(|| {
        format!(
            "{} · {}",
            t.tc("music:album.footer", songs.len() as i64, &[]),
            format_total(t, total)
        )
    });

    let mut menu = container_menu(t, &svc, Some(session.clone()), &item);
    menu.push(MenuItem::Separator);
    menu.push(MenuItem::action(
        t.t("music:playlist.rename"),
        Some("pencil"),
        move |_| renaming.set(true),
    ));
    menu.push(MenuItem::Action {
        label: t.t("music:playlistMenu.delete"),
        icon: Some("trash-2"),
        checked: None,
        destructive: true,
        disabled: false,
        on_select: Callback::new(move |_| confirming.set(true)),
    });

    let (p1, p2) = (svc.music.clone(), svc.music.clone());
    let (l1, l2) = (songs.clone(), songs.clone());
    let name = item.name().to_string();
    let title_el = rsx! {
        InlineTitle {
            value: name.clone(),
            editing: renaming(),
            on_editing: move |v| renaming.set(v),
            on_commit: {
                let (session, svc, id) = (session.clone(), svc.clone(), id.clone());
                move |new: String| {
                    // Optimistic: the cached item takes the new name at once, the server follows.
                    let acc = session.account_id.clone();
                    let n = new.clone();
                    svc.cache
                        .update::<BaseItemDto>(&keys::item(&acc, &id), move |i| i.name = Some(n));
                    let (session, cache, id) = (session.clone(), svc.cache.clone(), id.clone());
                    spawn(async move {
                        if session.rename_playlist(&id, &new).await.is_err() {
                            cache.invalidate(&keys::all(&acc));
                        }
                    });
                }
            },
        }
    };

    rsx! {
        ShellPage { title: name.clone(), header: PageHeader::Inline, gutter: true,
            DetailHeader {
                item: item.clone(),
                title: title_el,
                fallback: "list-music",
                subtitle: t.t("music:playlist.kind"),
                meta: summary.clone(),
                description: item.overview.clone().filter(|o| !o.is_empty()),
                on_play: move |_| p1.play(&l1, Some(0), Some(false)),
                on_shuffle: move |_| p2.play(&l2, None, Some(true)),
                menu,
            }
            if confirming() {
                div { class: "jr-empty jr-empty--inline",
                    h2 { class: "jr-empty__title", {t.tf("music:playlistDelete.title", &[("name", name.clone())])} }
                    p { class: "jr-empty__message", {t.t("music:playlistDelete.message")} }
                    div { class: "jr-empty__action",
                        Button { variant: "gray", onclick: move |_| confirming.set(false), {t.t("music:playlistSheet.cancel")} }
                        Button {
                            onclick: {
                                let (session, svc, id) = (session.clone(), svc.clone(), id.clone());
                                move |_| {
                                    confirming.set(false);
                                    let (session, cache, id) = (session.clone(), svc.cache.clone(), id.clone());
                                    spawn(async move {
                                        if session.delete_playlist(&id).await.is_ok() {
                                            cache.invalidate(&keys::all(&session.account_id));
                                            router.replace("/music/playlists");
                                        }
                                    });
                                }
                            },
                            {t.t("music:playlistDelete.confirm")}
                        }
                    }
                }
            }
            if songs.is_empty() {
                EmptyState { size: "inline", icon: "list-music", title: t.t("music:playlist.empty"), message: rsx! { {t.t("music:playlist.emptyHint")} } }
            } else {
                SongRows { tracks: songs.clone(), lead: "artwork", show_album: true, label: name.clone() }
                if let Some(s) = summary {
                    p { class: "jr-text-footnote jr-color-secondary jrm-footer", "{s}" }
                }
            }
        }
    }
}

/// `InlineTitle`: the name turns into a text field on click (Enter saves, Escape cancels).
#[component]
fn InlineTitle(
    value: String,
    editing: bool,
    on_editing: EventHandler<bool>,
    on_commit: EventHandler<String>,
) -> Element {
    let t = use_i18n();
    let mut draft = use_signal(|| value.clone());
    let finish = move |save: bool, current: &str| {
        on_editing.call(false);
        let next = draft.peek().trim().to_string();
        if save && !next.is_empty() && next != current {
            on_commit.call(next);
        }
    };
    if !editing {
        let v = value.clone();
        return rsx! {
            button {
                r#type: "button",
                class: "jrm-inline-title",
                title: t.t("music:playlist.rename"),
                onclick: move |_| {
                    draft.set(v.clone());
                    on_editing.call(true)
                },
                "{value}"
            }
        };
    }
    let (v1, v2) = (value.clone(), value);
    rsx! {
        input {
            class: "jrm-inline-title jrm-inline-title--input",
            "aria-label": t.t("music:playlist.rename"),
            value: "{draft}",
            maxlength: "120",
            autofocus: true,
            oninput: move |e| draft.set(e.value()),
            onblur: move |_| finish(true, &v1),
            onkeydown: move |e: KeyboardEvent| match e.key() {
                Key::Named(NamedKey::Enter) => finish(true, &v2),
                Key::Named(NamedKey::Escape) => finish(false, &v2),
                _ => {}
            },
        }
    }
}
