//! Watch / listen together (Jellyfin SyncPlay), `features/syncplay`: the rooms sheet (join, create,
//! members), the button and badge of the players, and the toasts. The protocol is in
//! `orphyn_jellyfin::syncplay`; `manager` follows the group, `bridge` hooks it to the players.
mod bridge;
pub mod manager;

use std::sync::Arc;

use dioxus::prelude::*;
use orphyn_jellyfin::syncplay::{
    self as sp, member_color, member_initials, MemberState, SyncPlayGroup,
};
use orphyn_jellyfin::JfError;

use manager::manager;
pub use manager::{MediaKind, Notice, Phase, SyncPlayState, UiEvent};

use crate::design::{Button, Icon, IconButton, Spinner};
use crate::features::cast::use_toasts;
use crate::i18n::{use_i18n, I18n};
use crate::router::use_router;
use crate::state::{use_app, use_services};

/// The manager's state as a signal.
#[derive(Clone, Copy, PartialEq)]
pub struct SyncPlayUi {
    pub state: Signal<Arc<SyncPlayState>>,
}

pub fn use_syncplay() -> SyncPlayUi {
    use_context::<SyncPlayUi>()
}

/// Provides [`SyncPlayUi`] at the root, hooks the players to the manager and shows its notices.
pub fn use_syncplay_provider() -> SyncPlayUi {
    let t = use_i18n();
    let svc = use_services();
    let toasts = use_toasts();
    let router = use_router();
    let ui = use_context_provider(|| SyncPlayUi {
        state: Signal::new(manager().state()),
    });
    use_hook(move || {
        bridge::start_music(svc.music.clone());
        bridge::start_video(svc.video.clone());
        let m = manager();
        let mut state = ui.state;
        let mut rx = m.subscribe();
        spawn(async move {
            while rx.changed().await.is_ok() {
                let s = rx.borrow_and_update().clone();
                state.set(s);
            }
        });
        let Some(mut events) = m.take_events() else {
            return;
        };
        spawn(async move {
            while let Some(event) = events.recv().await {
                match event {
                    UiEvent::Notice(n) => {
                        let (title, error) = notice_text(&t, &n);
                        if error {
                            toasts.error(title, None);
                        } else {
                            toasts.info(title, None);
                        }
                    }
                    UiEvent::Navigate(path) => router.push(&path),
                }
            }
        });
    });
    ui
}

/// `noticeText`: the toast of a notice, and whether it is an error.
pub fn notice_text(t: &I18n, n: &Notice) -> (String, bool) {
    let f = |key: &str, args: &[(&str, String)]| t.tf(&format!("syncplay:notice.{key}"), args);
    let name = |n: &str| [("name", n.to_string())];
    let by = |base: &str, who: &Option<String>| match who {
        Some(w) => f(&format!("{base}By"), &name(w)),
        None => f(base, &[]),
    };
    match n {
        Notice::UserJoined(u) => (f("userJoined", &name(u)), false),
        Notice::UserLeft(u) => (f("userLeft", &name(u)), false),
        Notice::Paused(w) => (by("paused", w), false),
        Notice::Resumed(w) => (by("resumed", w), false),
        Notice::Seeked(w) => (by("seeked", w), false),
        Notice::NowPlaying { title, by } => (
            match by {
                Some(w) => f(
                    "nowPlayingBy",
                    &[("name", w.clone()), ("title", title.clone())],
                ),
                None => f("nowPlaying", &[("title", title.clone())]),
            },
            false,
        ),
        Notice::Waiting(w) => (
            match w {
                Some(w) => f("waitingFor", &name(w)),
                None => f("waiting", &[]),
            },
            false,
        ),
        Notice::Joined(g) => (f("joined", &[("group", g.clone())]), false),
        Notice::Left => (f("left", &[]), false),
        Notice::GroupClosed => (f("groupClosed", &[]), false),
        Notice::ConnectionLost => (f("connectionLost", &[]), false),
        Notice::Reconnected => (f("reconnected", &[]), false),
        Notice::ItemUnavailable => (f("itemUnavailable", &[]), true),
        Notice::LibraryDenied => (f("libraryDenied", &[]), true),
        Notice::JoinDenied => (f("joinDenied", &[]), true),
        Notice::CreateDenied => (f("createDenied", &[]), true),
        Notice::Error => (f("error", &[]), true),
    }
}

fn in_group(s: &SyncPlayState) -> bool {
    matches!(s.phase, Phase::Joined | Phase::Reconnecting)
}

fn avatar_color(name: &str) -> String {
    format!("var(--system-{})", member_color(name))
}

#[component]
pub fn MemberAvatar(name: String, #[props(default = 32)] size: u32) -> Element {
    let bg = avatar_color(&name);
    rsx! {
        span { class: "jr-sp-avatar", style: "width: {size}px; height: {size}px; background: {bg};", "aria-hidden": "true",
            {member_initials(&name)}
        }
    }
}

/// The "together" button of the players: opens the sheet, shows the member count while in a room.
#[component]
pub fn SyncPlayButton(
    kind: MediaKind,
    #[props(default = "plain")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
) -> Element {
    let t = use_i18n();
    let app = use_app();
    let sp = use_syncplay();
    if (app.session)().is_none() {
        return rsx! {};
    }
    let s = (sp.state)();
    let inside = in_group(&s);
    let count = s.members.len();
    let label = t.t(if kind == MediaKind::Video {
        "syncplay:watchTogether"
    } else {
        "syncplay:listenTogether"
    });
    rsx! {
        span { class: "jr-sp-button",
            IconButton {
                icon: "users",
                label,
                variant,
                size,
                pressed: inside,
                onclick: move |_| manager().open_sheet(kind),
            }
            if inside && count > 1 {
                span { class: "jr-sp-button__count", "aria-hidden": "true", "{count}" }
            }
        }
    }
}

/// "Together · N" pill of Now Playing and the video player.
#[component]
pub fn SyncPlayBadge(kind: MediaKind) -> Element {
    let t = use_i18n();
    let sp = use_syncplay();
    let s = (sp.state)();
    let Some(group) = s.group.clone().filter(|_| in_group(&s)) else {
        return rsx! {};
    };
    let reconnecting = s.phase == Phase::Reconnecting;
    let count = s.members.len();
    let state = if reconnecting { "reconnecting" } else { "live" };
    let text = if reconnecting {
        t.t("syncplay:reconnecting")
    } else {
        t.tf("syncplay:badge", &[("count", count.to_string())])
    };
    let label = t.tf(
        "syncplay:badgeLabel",
        &[("name", group.name.clone()), ("count", count.to_string())],
    );
    rsx! {
        button {
            r#type: "button",
            class: "jr-sp-badge",
            "data-state": state,
            "aria-label": label,
            title: "{group.name}",
            onmousedown: move |e: MouseEvent| e.stop_propagation(),
            onclick: move |_| manager().open_sheet(kind),
            span { class: "jr-sp-badge__dot", "aria-hidden": "true" }
            Icon { name: "users", size: 14 }
            span { "{text}" }
        }
    }
}

fn member_state_key(s: MemberState) -> &'static str {
    match s {
        MemberState::Ready => "ready",
        MemberState::Buffering => "buffering",
        MemberState::Waiting => "waiting",
    }
}

fn state_name(state: sp::GroupState) -> &'static str {
    match state {
        sp::GroupState::Idle => "Idle",
        sp::GroupState::Waiting => "Waiting",
        sp::GroupState::Paused => "Paused",
        sp::GroupState::Playing => "Playing",
    }
}

#[component]
fn GroupPanel() -> Element {
    let t = use_i18n();
    let sp = use_syncplay();
    let s = (sp.state)();
    let Some(group) = s.group.clone() else {
        return rsx! {};
    };
    let item = s.current_item.as_ref().map(manager::item_title);
    let has_queue = s.queue.as_ref().is_some_and(|q| !q.playlist.is_empty());
    let now = item.unwrap_or_else(|| {
        if has_queue {
            "—".into()
        } else {
            t.t("syncplay:sheet.nothing")
        }
    });
    let people = t.tc("syncplay:sheet.people", s.members.len() as i64, &[]);
    let status = t.t(&format!(
        "syncplay:sheet.state.{}",
        state_name(s.group_state)
    ));
    rsx! {
        div { class: "jr-sp-panel",
            div { class: "jr-sp-group-head",
                span { class: "jr-sp-group-head__icon", "aria-hidden": "true", Icon { name: "users", size: 20 } }
                span { class: "jr-sp-group-head__text",
                    span { class: "jr-text-headline jr-truncate", "{group.name}" }
                    span { class: "jr-text-footnote jr-color-secondary", "{status} · {people}" }
                }
            }
            div { class: "jr-sp-now",
                span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.nowPlaying")} }
                span { class: "jr-text-callout jr-truncate", "{now}" }
            }
            span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.members")} }
            ul { class: "jr-sp-members",
                for (i, m) in s.members.iter().enumerate() {
                    {
                        let title = if m.is_self {
                            format!("{} ({})", m.name, t.t("syncplay:sheet.you"))
                        } else {
                            m.name.clone()
                        };
                        let label = t.t(&format!("syncplay:sheet.member.{}", member_state_key(m.state)));
                        let state = member_state_key(m.state);
                        rsx! {
                            li { key: "{m.name}-{i}", class: "jr-sp-member",
                                MemberAvatar { name: m.name.clone() }
                                span { class: "jr-text-body jr-truncate jr-sp-member__name", "{title}" }
                                span { class: "jr-sp-state", "data-state": state,
                                    span { class: "jr-sp-state__dot", "aria-hidden": "true" }
                                    "{label}"
                                }
                            }
                        }
                    }
                }
            }
            span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.leaveHint")} }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum LoadError {
    None,
    Failed,
    NoAccess,
}

#[component]
fn JoinPanel() -> Element {
    let t = use_i18n();
    let app = use_app();
    let sp = use_syncplay();
    let user_name = (app.accounts)()
        .iter()
        .find(|a| (app.session)().is_some_and(|s| s.account_id == a.id))
        .map(|a| a.user_name.clone())
        .unwrap_or_default();
    let default_name = t.tf("syncplay:sheet.defaultName", &[("user", user_name.clone())]);
    let mut name = use_signal({
        let d = default_name.clone();
        move || d
    });
    let mut groups = use_signal(|| None::<Vec<SyncPlayGroup>>);
    let mut error = use_signal(|| LoadError::None);
    let mut loading = use_signal(|| false);
    let connecting = (sp.state)().phase == Phase::Connecting;

    let load = Callback::new(move |()| {
        let Some(session) = (app.session)() else {
            return;
        };
        loading.set(true);
        spawn(async move {
            match sp::list_groups(&session.client).await {
                Ok(list) => {
                    groups.set(Some(list));
                    error.set(LoadError::None);
                }
                Err(JfError::Http(403)) => error.set(LoadError::NoAccess),
                Err(_) => error.set(LoadError::Failed),
            }
            loading.set(false);
        });
    });
    use_hook(move || {
        load.call(());
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                load.call(());
            }
        });
    });

    if error() == LoadError::NoAccess {
        return rsx! {
            div { class: "jr-sp-panel",
                span { class: "jr-text-callout jr-color-secondary", {t.t("syncplay:sheet.noAccess")} }
            }
        };
    }
    let create = {
        let user_name = user_name.clone();
        let default_name = default_name.clone();
        move |_| {
            let Some(session) = (app.session)() else {
                return;
            };
            let n = name.peek().trim().to_string();
            manager().create(
                session,
                user_name.clone(),
                if n.is_empty() {
                    default_name.clone()
                } else {
                    n
                },
            );
        }
    };
    let rooms = groups();
    rsx! {
        div { class: "jr-sp-panel",
            div { class: "jr-sp-create",
                span { class: "jr-text-headline", {t.t("syncplay:sheet.startSection")} }
                div { class: "jr-sp-create__row",
                    input {
                        class: "jr-field__input",
                        r#type: "text",
                        "aria-label": t.t("syncplay:sheet.roomName"),
                        value: "{name}",
                        maxlength: "80",
                        disabled: connecting,
                        oninput: move |e| name.set(e.value()),
                    }
                    Button { disabled: connecting, onclick: create, {t.t("syncplay:sheet.create")} }
                }
                span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.createHint")} }
            }
            div { class: "jr-sp-rooms",
                div { class: "jr-sp-rooms__head",
                    span { class: "jr-text-headline", {t.t("syncplay:sheet.roomsSection")} }
                    if loading() {
                        Spinner { size: "small" }
                    } else {
                        IconButton { size: "small", icon: "refresh-cw", label: t.t("syncplay:sheet.refresh"), onclick: move |_| load.call(()) }
                    }
                }
                if error() == LoadError::Failed {
                    span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.loadFailed")} }
                }
                if let Some(list) = rooms {
                    if list.is_empty() && error() == LoadError::None {
                        span { class: "jr-text-footnote jr-color-secondary", {t.t("syncplay:sheet.empty")} }
                    }
                    ul { class: "jr-sp-members",
                        for g in list {
                            {
                                let status = t.t(&format!("syncplay:sheet.state.{}", state_name(g.state)));
                                let people = t.tc("syncplay:sheet.people", g.participants.len() as i64, &[]);
                                let (gid, un) = (g.id.clone(), user_name.clone());
                                rsx! {
                                    li { key: "{g.id}", class: "jr-sp-member",
                                        MemberAvatar { name: g.participants.first().cloned().unwrap_or_else(|| g.name.clone()) }
                                        span { class: "jr-sp-member__name",
                                            span { class: "jr-text-body jr-truncate", "{g.name}" }
                                            span { class: "jr-text-footnote jr-color-secondary", "{status} · {people}" }
                                        }
                                        Button {
                                            size: "small",
                                            variant: "tinted",
                                            disabled: connecting,
                                            onclick: move |_| {
                                                if let Some(session) = (app.session)() {
                                                    manager().join(session, un.clone(), gid.clone());
                                                }
                                            },
                                            {t.t("syncplay:sheet.join")}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Always mounted: the rooms sheet.
#[component]
pub fn SyncPlayHost() -> Element {
    let t = use_i18n();
    let app = use_app();
    let sp = use_syncplay();
    let s = (sp.state)();
    if !s.sheet_open {
        return rsx! {};
    }
    let inside = in_group(&s);
    let server = (app.accounts)()
        .iter()
        .find(|a| (app.session)().is_some_and(|x| x.account_id == a.id))
        .map(|a| a.server_name.clone())
        .unwrap_or_default();
    let done_variant = if inside { "filled" } else { "gray" };
    let title = t.t(if s.sheet_kind == MediaKind::Video {
        "syncplay:watchTogether"
    } else {
        "syncplay:listenTogether"
    });
    let description = t.tf("syncplay:sheet.description", &[("server", server)]);
    rsx! {
        div { class: "jr-sheet-layer jr-sp-sheet",
            onkeydown: move |e| if e.key() == Key::Named(NamedKey::Escape) { manager().close_sheet() },
            div { class: "jr-sheet-backdrop", "aria-hidden": "true", onclick: move |_| manager().close_sheet() }
            div { class: "jr-sheet jr-sheet--medium", role: "dialog", "aria-modal": "true",
                header { class: "jr-sheet__header",
                    div { class: "jr-sheet__titles",
                        h2 { class: "jr-sheet__title", "{title}" }
                        p { class: "jr-sheet__description", "{description}" }
                    }
                    IconButton { icon: "x", label: t.t("shell:design.close"), variant: "gray", size: "small", stroke_width: 2.5, onclick: move |_| manager().close_sheet() }
                }
                div { class: "jr-sheet__body",
                    if inside { GroupPanel {} } else { JoinPanel {} }
                }
                footer { class: "jr-sheet__footer",
                    if inside {
                        Button { variant: "gray", onclick: move |_| manager().leave(), {t.t("syncplay:sheet.leave")} }
                    }
                    Button { variant: done_variant, onclick: move |_| manager().close_sheet(), {t.t("syncplay:sheet.done")} }
                }
            }
        }
    }
}
