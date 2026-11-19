//! `/cast`: the devices found on the network, and a remote control for the one a session is
//! open on (the React app routes this page to a stub; the native one is a small device list).
use dioxus::prelude::*;
use orphyn_core::cast::CastStatus;

use super::devices::group_devices;
use super::hub::hub;
use super::use_cast;
use crate::design::{EmptyState, Icon, IconButton, Slider};
use crate::i18n::use_i18n;
use crate::shell::ShellPage;

#[component]
pub fn CastPage() -> Element {
    let t = use_i18n();
    let cast = use_cast();
    // Discovery runs while the page is open.
    use_hook(|| hub().service.start_discovery());
    use_drop(|| hub().service.stop_discovery());
    let devices = (cast.devices)();
    let groups = group_devices(&devices);
    rsx! {
        ShellPage { title: t.t("common:nav.cast"),
            if devices.is_empty() {
                EmptyState { icon: "cast", title: t.t("cast:title"), message: rsx! { {t.t("cast:none")} } }
            }
            for (group, list) in groups {
                section { key: "{group.key()}", class: "jr-cast-page__group",
                    h2 { class: "jr-text-headline", {t.t(group.key())} }
                    ul { class: "jr-cast__list",
                        for d in list {
                            {
                                let session = cast.session_of(&d.id).filter(|s| s.status != CastStatus::Idle);
                                let id = d.id.clone();
                                let (id_play, id_stop, id_vol) = (id.clone(), id.clone(), id);
                                let play_icon = if session.as_ref().is_some_and(|s| matches!(s.status, CastStatus::Playing | CastStatus::Buffering)) { "pause" } else { "play" };
                                let play_label = t.t(if play_icon == "pause" { "cast:pause" } else { "cast:play" });
                                let playing = session.as_ref().is_some_and(|s| matches!(s.status, CastStatus::Playing | CastStatus::Buffering));
                                rsx! {
                                    li { key: "{d.id}", class: "jr-cast-page__device",
                                        span { class: "jr-cast__icon", Icon { name: group.icon(), size: 18 } }
                                        span { class: "jr-cast-page__text",
                                            span { class: "jr-text-body jr-truncate", "{d.name}" }
                                            span { class: "jr-text-footnote jr-color-secondary jr-truncate",
                                                {[d.manufacturer.clone(), d.model.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")}
                                            }
                                        }
                                        if let Some(s) = session {
                                            span { class: "jr-text-footnote jr-color-secondary jr-truncate",
                                                {s.media.as_ref().map(|m| m.title.clone()).unwrap_or_default()}
                                            }
                                            IconButton {
                                                icon: play_icon,
                                                fill: true,
                                                label: play_label,
                                                onclick: move |_| {
                                                    if playing { hub().service.pause(&id_play) } else { hub().service.play(&id_play) }
                                                },
                                            }
                                            IconButton {
                                                icon: "x",
                                                label: t.t("cast:stop"),
                                                onclick: move |_| hub().service.disconnect(&id_stop),
                                            }
                                            div { class: "jr-cast-page__volume",
                                                Slider {
                                                    label: t.t("cast:volume"),
                                                    value: s.volume,
                                                    max: 1.0,
                                                    on_change: move |v: f64| hub().service.set_volume(&id_vol, v),
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
    }
}
