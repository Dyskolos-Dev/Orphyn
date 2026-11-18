//! Settings > Servers (`ServersPane.tsx`): list, switch, rename, sign out, add.
use dioxus::prelude::*;
use orphyn_core::accounts::ServerAccount;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::Client;

use super::ui::{GrayButton, List, ListRow};
use crate::design::overlay::{use_overlay, MenuItem};
use crate::design::{Button, EmptyState, IconButton};
use crate::features::servers::{AddServerSheet, Sheet, TextField, UserAvatar};
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services, AppState, Services};

/// Makes `id` the active server (the session of its stored token), like `session.switchServer`.
pub fn switch_server(app: AppState, svc: &Services, id: &str) {
    let device = svc.device_id();
    let Some(store) = svc.accounts.as_ref() else { return };
    let session = {
        let mut s = store.lock().unwrap_or_else(|p| p.into_inner());
        let account = s.list().into_iter().find(|a| a.id == id);
        let token = s.token(id);
        match (account, token) {
            (Some(a), Some(token)) if s.set_active(Some(id)).is_ok() => Some(Session {
                account_id: a.id,
                user_id: a.user_id,
                client: Client::new(&a.url, &device, Some(&token)),
            }),
            _ => None,
        }
    };
    if session.is_some() {
        app.activate(svc, session);
    }
}

fn remove_server(app: AppState, svc: &Services, id: &str) {
    let was_active = app.session.peek().as_ref().map(|s| s.account_id.clone()).as_deref() == Some(id);
    if let Some(store) = svc.accounts.as_ref() {
        let mut s = store.lock().unwrap_or_else(|p| p.into_inner());
        let _ = s.remove(id);
    }
    svc.cache.remove(&orphyn_jellyfin::keys::all(id));
    if was_active {
        let next = svc.active_session();
        app.activate(svc, next);
    } else {
        let mut accounts = app.accounts;
        accounts.set(svc.list_accounts());
    }
}

#[component]
pub fn ServersPane() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let overlay = use_overlay();
    let mut adding = use_signal(|| false);
    let mut renaming = use_signal(|| None::<ServerAccount>);
    let mut removing = use_signal(|| None::<ServerAccount>);
    let accounts = (app.accounts)();
    let active_id = (app.session)().map(|s| s.account_id);
    let has_token = |a: &ServerAccount| svc.accounts.as_ref().is_some_and(|s| s.lock().map(|s| s.token(&a.id).is_some()).unwrap_or(false));

    rsx! {
        if accounts.is_empty() {
            EmptyState {
                size: "inline",
                title: t.t("settings:servers.empty"),
                message: rsx! { {t.t("settings:servers.emptyMessage")} },
                action: rsx! {
                    Button { icon: "plus", onclick: move |_| adding.set(true), {t.t("settings:servers.add")} }
                },
            }
        } else {
            List { header: t.t("settings:servers.header"), label: t.t("settings:servers.header"),
                for a in accounts.iter().cloned() {
                    {
                        let expired = !has_token(&a);
                        let is_active = Some(&a.id) == active_id.as_ref();
                        let (id, svc2, a2, a3) = (a.id.clone(), svc.clone(), a.clone(), a.clone());
                        let more = t.tf("settings:servers.more", &[("name", a.name.clone())]);
                        rsx! {
                            ListRow {
                                key: "{a.id}",
                                leading: rsx! { UserAvatar { account: a.clone(), size: 32 } },
                                title: rsx! {
                                    "{a.name}"
                                    if is_active {
                                        span { class: "jr-settings__badge", {t.t("settings:servers.active")} }
                                    }
                                },
                                subtitle: if expired { t.t("settings:servers.needsLogin") } else { t.tf("settings:servers.details", &[("user", a.user_name.clone()), ("url", a.url.clone())]) },
                                trailing: rsx! {
                                    span { class: "jr-settings__actions",
                                        IconButton {
                                            icon: "ellipsis",
                                            label: more,
                                            onclick: move |e: MouseEvent| {
                                                let (id, svc3) = (id.clone(), svc2.clone());
                                                let (r, d) = (a2.clone(), a3.clone());
                                                let mut sw = MenuItem::action(t.t("settings:servers.switch"), None, move |_| switch_server(app, &svc3, &id));
                                                if let MenuItem::Action { disabled, .. } = &mut sw { *disabled = is_active || expired; }
                                                let mut out = MenuItem::action(t.t("settings:servers.signOut"), None, move |_| removing.set(Some(d.clone())));
                                                if let MenuItem::Action { destructive, .. } = &mut out { *destructive = true; }
                                                let c = e.client_coordinates();
                                                overlay.menu((c.x - 160.0).max(8.0), c.y + 12.0, vec![
                                                    sw,
                                                    MenuItem::action(t.t("settings:servers.rename"), None, move |_| renaming.set(Some(r.clone()))),
                                                    MenuItem::Separator,
                                                    out,
                                                ]);
                                            },
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
            div { GrayButton { icon: "plus", onclick: move |_| adding.set(true), {t.t("settings:servers.add")} } }
        }
        if adding() {
            AddServerSheet { on_close: move |_| adding.set(false) }
        }
        if let Some(a) = renaming() {
            RenameSheet { key: "{a.id}", account: a, on_close: move |_| renaming.set(None) }
        }
        if let Some(a) = removing() {
            {
                let svc = svc.clone();
                let id = a.id.clone();
                rsx! {
                    Sheet {
                        size: "small",
                        title: t.tf("settings:servers.removeTitle", &[("name", a.name.clone())]),
                        description: t.t("settings:servers.removeMessage"),
                        on_close: move |_| removing.set(None),
                        footer: rsx! {
                            Button { variant: "gray", onclick: move |_| removing.set(None), {t.t("settings:servers.cancel")} }
                            Button {
                                onclick: move |_| {
                                    remove_server(app, &svc, &id);
                                    removing.set(None);
                                },
                                {t.t("settings:servers.removeConfirm")}
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn RenameSheet(account: ServerAccount, on_close: EventHandler<()>) -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let name = use_signal(|| account.name.clone());
    let id = account.id.clone();
    let current = account.name.clone();
    let save = Callback::new(move |_: ()| {
        let trimmed = name().trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if trimmed != current {
            if let Some(store) = svc.accounts.as_ref() {
                let _ = store.lock().unwrap_or_else(|p| p.into_inner()).rename(&id, &trimmed);
            }
            let mut accounts = app.accounts;
            accounts.set(svc.list_accounts());
        }
        on_close.call(());
    });
    rsx! {
        Sheet { size: "small", title: t.t("settings:servers.renameTitle"), on_close,
            div { class: "jr-settings__form",
                TextField { label: t.t("settings:servers.renameLabel"), size: "medium", value: name, on_enter: move |_| save.call(()) }
                div { class: "jr-settings__sheet-actions",
                    Button { variant: "gray", onclick: move |_| on_close.call(()), {t.t("settings:servers.cancel")} }
                    Button { disabled: name().trim().is_empty(), onclick: move |_| save.call(()), {t.t("settings:servers.save")} }
                }
            }
        }
    }
}
