//! Discord Rich Presence: the presence builder and the Settings > Discord pane
//! (`settings/DiscordPane.tsx`). The sync with the players lives in `features/system`.
pub mod presence;

use std::time::Duration;

use dioxus::prelude::*;
use orphyn_core::discord::service::{SettingsPatch, State, Status};

use crate::features::system::use_system;
use crate::i18n::use_i18n;

fn status_key(s: Status) -> &'static str {
    match s {
        Status::NotConfigured => "not-configured",
        Status::Disabled => "disabled",
        Status::Idle => "idle",
        Status::Connected => "connected",
        Status::Unavailable => "unavailable",
    }
}

#[component]
fn Row(title: String, subtitle: String, children: Element) -> Element {
    rsx! {
        li { class: "jr-list__item",
            div { class: "jr-row",
                span { class: "jr-row__text",
                    span { class: "jr-row__title", "{title}" }
                    if !subtitle.is_empty() {
                        span { class: "jr-row__subtitle", "{subtitle}" }
                    }
                }
                span { class: "jr-row__trailing", {children} }
            }
        }
    }
}

#[component]
pub fn DiscordPane() -> Element {
    let t = use_i18n();
    let discord = use_system().discord;
    let mut state = use_signal(|| discord.state());
    // The worker connects lazily and reports later: look again while the pane is open.
    use_future({
        let discord = discord.clone();
        move || {
            let discord = discord.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let next: State = discord.state();
                    if next != *state.peek() {
                        state.set(next);
                    }
                }
            }
        }
    });
    let s = state();
    let configured = s.status != Status::NotConfigured;
    let mut set = {
        let discord = discord.clone();
        move |patch: SettingsPatch| {
            discord.set_settings(patch);
            // Optimistic: the worker confirms on the next poll.
            let mut next = *state.peek();
            if let Some(v) = patch.enabled {
                next.enabled = v;
            }
            if let Some(v) = patch.show_artwork {
                next.show_artwork = v;
            }
            state.set(next);
        }
    };
    let mut set_enabled = set.clone();
    rsx! {
        div { class: "jr-list-group jr-list-group--inset",
            div { class: "jr-list-group__header", {t.t("settings:discord.header")} }
            ul { class: "jr-list",
                Row {
                    title: t.t("settings:discord.enabled"),
                    subtitle: if configured { t.t("settings:discord.enabledDescription") } else { t.t("settings:discord.notConfiguredNote") },
                    Switch {
                        checked: s.enabled,
                        label: t.t("settings:discord.enabled"),
                        on_change: move |on| set_enabled(SettingsPatch { enabled: Some(on), show_artwork: None }),
                    }
                }
                Row {
                    title: t.t("settings:discord.artwork"),
                    subtitle: t.t("settings:discord.artworkDescription"),
                    Switch {
                        checked: s.show_artwork,
                        label: t.t("settings:discord.artwork"),
                        on_change: move |on| set(SettingsPatch { enabled: None, show_artwork: Some(on) }),
                    }
                }
                Row { title: t.t("settings:discord.status"), subtitle: String::new(),
                    span { class: "jr-settings__value", role: "status",
                        {t.t(&format!("settings:discord.statuses.{}", status_key(s.status)))}
                    }
                }
            }
            div { class: "jr-list-group__footer", {t.t("settings:discord.footer")} }
        }
    }
}

/// The `.jr-toggle` switch of the design system (same DOM as `controls/Toggle.tsx`).
#[component]
pub fn Switch(checked: bool, on_change: EventHandler<bool>, label: String) -> Element {
    rsx! {
        button {
            r#type: "button",
            role: "switch",
            "aria-checked": "{checked}",
            "aria-label": "{label}",
            class: "jr-toggle jr-toggle--medium",
            onmousedown: move |e| e.stop_propagation(),
            onclick: move |_| on_change.call(!checked),
            span { class: "jr-toggle__knob", "aria-hidden": "true" }
        }
    }
}
