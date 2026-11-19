//! Settings page (`SettingsPage.tsx`): a pane list on the left, the pane on the right.
mod about;
mod appearance;
mod home;
mod playback;
mod servers_pane;
mod style_gallery;
mod ui;

use dioxus::prelude::*;

pub use about::open_external;
pub use playback::apply_playback_prefs;
pub use servers_pane::switch_server;

use crate::design::Icon;
use crate::i18n::use_i18n;
use crate::shell::ShellPage;

const PANES: [(&str, &str); 8] = [
    ("general", "settings"),
    ("appearance", "palette"),
    ("home", "layout-dashboard"),
    ("servers", "server"),
    ("playback", "play"),
    ("music", "music"),
    ("discord", "message-circle"),
    ("about", "info"),
];

#[component]
pub fn SettingsPage() -> Element {
    let t = use_i18n();
    let mut pane = use_signal(|| "general");
    let current = pane();
    let title = t.t(&format!("settings:panes.{current}"));
    rsx! {
        ShellPage { title: t.t("settings:title"), gutter: true,
            div { class: "jr-settings",
                nav { class: "jr-settings__nav", "aria-label": t.t("settings:nav"),
                    ul { class: "jr-sidebar-section__list",
                        for (id , icon) in PANES {
                            li { key: "{id}", class: "jr-sidebar-item",
                                button {
                                    r#type: "button",
                                    class: if id == current { "jr-sidebar-item__link jr-sidebar-item__link--active" } else { "jr-sidebar-item__link" },
                                    "aria-current": (id == current).then_some("page"),
                                    onmousedown: move |e| e.stop_propagation(),
                                    onclick: move |_| pane.set(id),
                                    span { class: "jr-sidebar-item__icon", Icon { name: icon, size: 18 } }
                                    span { class: "jr-sidebar-item__label", {t.t(&format!("settings:panes.{id}"))} }
                                }
                            }
                        }
                    }
                }
                section { class: "jr-settings__pane", "aria-label": "{title}",
                    h2 { class: "jr-settings__pane-title", "{title}" }
                    match current {
                        "appearance" => rsx! { appearance::AppearancePane {} },
                        "home" => rsx! { home::HomePane {} },
                        "about" => rsx! { about::AboutPane {} },
                        "discord" => rsx! { crate::features::discord::DiscordPane {} },
                        "servers" => rsx! { servers_pane::ServersPane {} },
                        "playback" => rsx! { playback::PlaybackPane {} },
                        "music" => rsx! { playback::MusicPane {} },
                        _ => rsx! { playback::GeneralPane {} },
                    }
                }
            }
        }
    }
}
