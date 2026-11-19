//! `features/music/playlists` of the React app: the "New Playlist" sheet, the "Add to Playlist…"
//! picker entries and the toasts of every playlist change (`usePlaylistToasts`).
//!
//! `use_playlist_provider()` once at the root, `PlaylistHost {}` once beside the toast host; then
//! any menu or button calls `use_playlist_ui().open_new(ids)`.
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::client::Q;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::keys;

use crate::design::toast::{use_toasts, ToastOptions, Toasts};
use crate::design::Button;
use crate::features::servers::{Sheet, TextField};
use crate::i18n::{use_i18n, I18n};
use crate::router::use_router;
use crate::state::{use_app, Services};

#[derive(Clone, Copy, PartialEq)]
pub struct PlaylistUi {
    /// "New Playlist" sheet; the ids are added to it right after creation.
    sheet: Signal<Option<Vec<String>>>,
}

pub fn use_playlist_provider() -> PlaylistUi {
    use_context_provider(|| PlaylistUi {
        sheet: Signal::new(None),
    })
}

pub fn use_playlist_ui() -> PlaylistUi {
    use_context::<PlaylistUi>()
}

impl PlaylistUi {
    /// `openNewPlaylist`.
    pub fn open_new(mut self, ids: Vec<String>) {
        self.sheet.set(Some(ids));
    }
}

fn toast(toasts: Toasts, title: String, icon: &'static str) {
    let mut o = ToastOptions::new(title);
    o.icon = Some(icon);
    toasts.show(o);
}

pub fn fail(toasts: Toasts, t: I18n) {
    toasts.error(t.t("music:playlistToast.failed"), None);
}

/// `POST /Playlists/{id}/Items` (`addToPlaylist`), the playlist's cached items refreshed, then the
/// "Added to <name>" toast (`addTo` of `usePlaylistToasts`).
pub fn add_to_playlist(
    svc: Services,
    session: Session,
    toasts: Toasts,
    t: I18n,
    playlist: String,
    name: String,
    ids: Vec<String>,
) {
    let url = session.client.authed_url(
        &format!("/Playlists/{playlist}/Items"),
        &[
            ("ids", Some(Q::List(ids))),
            ("userId", Some(Q::Str(session.user_id.clone()))),
        ],
    );
    spawn(async move {
        let done = tokio::task::spawn_blocking(move || {
            orphyn_player::music::http::post_empty(&url, Duration::from_secs(20))
        })
        .await;
        match done {
            Ok(Ok(())) => {
                svc.cache
                    .invalidate(&keys::playlist_items(&session.account_id, &playlist));
                toast(
                    toasts,
                    t.tf("music:playlistToast.added", &[("name", name)]),
                    "check",
                );
            }
            _ => fail(toasts, t),
        }
    });
}

/// `createWith`: creates the playlist (the server call replaces a playlist of the same name, as
/// the year's "Save as playlist" does) and toasts. `done` gets the new id.
pub fn create_playlist(
    session: Session,
    toasts: Toasts,
    t: I18n,
    name: String,
    ids: Vec<String>,
    done: impl FnOnce(Option<String>) + 'static,
) {
    spawn(async move {
        let name = name.trim().to_string();
        let had = !ids.is_empty();
        let s = session.clone();
        let n = name.clone();
        let made = tokio::spawn(async move { s.save_playlist(&n, &ids).await }).await;
        match made {
            Ok(Ok(id)) if !id.is_empty() => {
                let key = if had {
                    "music:playlistToast.added"
                } else {
                    "music:playlistToast.created"
                };
                toast(toasts, t.tf(key, &[("name", name)]), "list-music");
                done(Some(id));
            }
            _ => {
                fail(toasts, t);
                done(None);
            }
        }
    });
}

/// The "New Playlist" sheet, shared by the Playlists page, the picker and the multi-select bar.
#[component]
pub fn PlaylistHost() -> Element {
    let mut ui = use_playlist_ui();
    let Some(ids) = (ui.sheet)() else {
        return rsx! {};
    };
    rsx! { NewPlaylistSheet { ids, on_close: move |_| ui.sheet.set(None) } }
}

#[component]
fn NewPlaylistSheet(ids: Vec<String>, on_close: EventHandler<()>) -> Element {
    let t = use_i18n();
    let toasts = use_toasts();
    let router = use_router();
    let session = (use_app().session)();
    let name = use_signal(String::new);
    let description = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let ids_for_submit = ids.clone();
    let submit = move || {
        let n = name.peek().trim().to_string();
        let Some(session) = session.clone() else {
            return;
        };
        if n.is_empty() || *busy.peek() {
            return;
        }
        busy.set(true);
        let had_items = !ids_for_submit.is_empty();
        create_playlist(session, toasts, t, n, ids_for_submit.clone(), move |id| {
            busy.set(false);
            if let Some(id) = id {
                on_close.call(());
                if !had_items {
                    router.push(&format!("/music/playlist/{id}"));
                }
            }
        });
    };
    let mut submit_form = submit.clone();
    let mut submit = submit;
    rsx! {
        Sheet {
            title: t.t("music:playlistSheet.title"),
            size: "small",
            on_close,
            footer: rsx! {
                Button { variant: "gray", onclick: move |_| on_close.call(()), {t.t("music:playlistSheet.cancel")} }
                Button {
                    disabled: name().trim().is_empty() || busy(),
                    onclick: move |_| submit(),
                    {t.t("music:playlistSheet.create")}
                }
            },
            div { class: "jrm-sheet-form",
                TextField {
                    label: t.t("music:playlistSheet.name"),
                    value: name,
                    on_enter: move |_| submit_form(),
                }
                TextField {
                    label: t.t("music:playlistSheet.description"),
                    value: description,
                }
            }
        }
    }
}
