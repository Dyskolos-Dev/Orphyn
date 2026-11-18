//! `ServerSwitcher.tsx`: the sidebar footer button (active server + user) and its menu to switch,
//! add or manage servers. Mount it in the sidebar footer.
use dioxus::prelude::*;

use super::{AddServerSheet, UserAvatar};
use crate::design::overlay::{use_overlay, MenuItem};
use crate::design::Icon;
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::{use_app, use_services};

#[component]
pub fn ServerSwitcher() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let router = use_router();
    let overlay = use_overlay();
    let mut adding = use_signal(|| false);

    let accounts = (app.accounts)();
    let active_id = (app.session)().map(|s| s.account_id);
    let active = accounts.iter().find(|a| Some(&a.id) == active_id.as_ref()).cloned();
    if active.is_none() && accounts.is_empty() {
        return rsx! {};
    }
    let label = match &active {
        Some(a) => t.tf("servers:switcher.active", &[("name", a.name.clone())]),
        None => t.t("servers:switcher.label"),
    };

    let open = move |e: MouseEvent| {
        let accounts = (app.accounts)();
        let active_id = (app.session)().map(|s| s.account_id);
        let mut items: Vec<MenuItem> = accounts
            .iter()
            .map(|a| {
                let id = a.id.clone();
                let svc = svc.clone();
                MenuItem::Action {
                    label: a.name.clone(),
                    icon: None,
                    checked: Some(Some(&a.id) == active_id.as_ref()),
                    destructive: false,
                    disabled: false,
                    on_select: Callback::new(move |_| {
                        crate::features::settings::switch_server(app, &svc, &id);
                    }),
                }
            })
            .collect();
        items.push(MenuItem::Separator);
        items.push(MenuItem::action(t.t("servers:switcher.add"), Some("plus"), move |_| adding.set(true)));
        items.push(MenuItem::action(t.t("servers:switcher.manage"), Some("settings-2"), move |_| {
            router.push("/settings")
        }));
        let rows = items.len() as f64;
        let c = e.client_coordinates();
        overlay.menu(c.x, (c.y - rows * 32.0 - 16.0).max(8.0), items);
    };

    rsx! {
        div { class: "jr-switcher",
            button {
                r#type: "button",
                class: "jr-switcher__button",
                "aria-haspopup": "menu",
                "aria-label": "{label}",
                onmousedown: move |e| e.stop_propagation(),
                onclick: open,
                if let Some(a) = active.clone() {
                    UserAvatar { account: a }
                }
                span { class: "jr-switcher__text",
                    span { class: "jr-switcher__name",
                        {active.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| t.t("servers:switcher.none"))}
                    }
                    if let Some(a) = &active {
                        span { class: "jr-switcher__user", "{a.user_name}" }
                    }
                }
                Icon { name: "chevrons-up-down", size: 16, class: "jr-switcher__chevron" }
            }
            if adding() {
                AddServerSheet { on_close: move |_| adding.set(false) }
            }
        }
    }
}
