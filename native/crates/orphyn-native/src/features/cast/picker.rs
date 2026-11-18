//! `CastPicker.tsx`: AirPlay-style popover. "This computer" then the devices, grouped by speakers
//! and TVs; the selected device shows a volume slider. Rendered next to its button (the
//! `.jr-cast-anchor` box), above it, with a transparent backdrop that closes it.
use dioxus::prelude::*;
use orphyn_core::cast::{CastDevice, CastKind};

use super::devices::{devices_for_scope, group_devices, CastScope};
use super::hub::hub;
use super::{use_cast, use_toasts};
use crate::design::{Icon, Slider, Spinner};
use crate::i18n::use_i18n;

#[component]
fn DeviceVolume(device_id: String) -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let mut drag = use_signal(|| None::<f64>);
    let volume = drag().or_else(|| cast.session_of(&device_id).map(|s| s.volume)).unwrap_or(1.0);
    let id = device_id.clone();
    rsx! {
        div { class: "jr-cast__volume",
            Slider {
                label: t.t("cast:volume"),
                value: volume,
                max: 1.0,
                on_change: move |v: f64| {
                    drag.set(Some(v));
                    hub().service.set_volume(&id, v);
                },
                on_change_end: move |_| drag.set(None),
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct CastPickerProps {
    pub scope: CastScope,
    /// Device the player currently plays on (`None` = this computer).
    pub current_id: Option<String>,
    pub on_close: EventHandler<()>,
    /// Moves playback to the device, or back to this computer with `None`.
    /// The handler answers through the callback (possibly later): `Ok` or the failure message.
    pub on_select: EventHandler<(Option<CastDevice>, Callback<Result<(), String>>)>,
}

#[component]
pub fn CastPicker(props: CastPickerProps) -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let toasts = use_toasts();
    let CastPickerProps { scope, current_id, on_close, on_select } = props;
    // Discovery runs only while the picker is open.
    use_hook(|| hub().service.start_discovery());
    use_drop(|| hub().service.stop_discovery());
    let devices = devices_for_scope(&(cast.devices)(), scope);

    let choose = {
        let current_id = current_id.clone();
        EventHandler::new(move |device: Option<CastDevice>| {
            let id = device.as_ref().map(|d| d.id.clone());
            let id2 = id.clone();
            if id2 == current_id || (cast.connecting)().is_some() {
                return;
            }
            let mut connecting = cast.connecting;
            connecting.set(Some(id.clone().unwrap_or_else(|| "local".into())));
            let name = device
                .as_ref()
                .map_or_else(|| t.t("cast:thisComputer"), |d| d.name.clone());
            let done = Callback::new(move |result: Result<(), String>| {
                connecting.set(None);
                match result {
                    Ok(()) if id.is_none() => on_close.call(()),
                    Ok(()) => {}
                    Err(e) => toasts.error(t.tf("cast:connectFailed", &[("device", name.clone())]), Some(e)),
                }
            });
            on_select.call((device, done));
        })
    };

    rsx! {
        div { class: "jr-cast-backdrop", onclick: move |_| on_close.call(()) }
        div { class: "jr-cast-popover jr-material-thick", role: "dialog", "aria-label": t.t("cast:title"),
            div { class: "jr-cast",
                h2 { class: "jr-text-headline jr-cast__title", {t.t("cast:title")} }
                ul { class: "jr-cast__list", role: "menu",
                    DeviceRow { device: None, icon: "laptop", current_id: current_id.clone(), on_choose: choose }
                }
                for (group, list) in group_devices(&devices) {
                    section { key: "{group.key()}", "aria-label": t.t(group.key()),
                        span { class: "jr-text-caption jr-color-secondary jr-cast__group", {t.t(group.key())} }
                        ul { class: "jr-cast__list", role: "menu",
                            for d in list { DeviceRow { key: "{d.id}", device: Some(d), icon: group.icon(), current_id: current_id.clone(), on_choose: choose } }
                        }
                    }
                }
                if devices.is_empty() {
                    p { class: "jr-text-footnote jr-color-secondary jr-cast__hint",
                        {t.t(if scope == CastScope::Video { "cast:noneVideo" } else { "cast:none" })}
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
struct DeviceRowProps {
    device: Option<CastDevice>,
    icon: &'static str,
    current_id: Option<String>,
    on_choose: EventHandler<Option<CastDevice>>,
}

/// One radio row: "This computer" (`device: None`) or a device.
#[component]
fn DeviceRow(props: DeviceRowProps) -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let DeviceRowProps { device, icon, current_id, on_choose } = props;
    let connecting = (cast.connecting)();
    let id = device.as_ref().map(|d| d.id.clone());
    let selected = id == current_id;
    let is_connecting = connecting.as_deref() == Some(id.as_deref().unwrap_or("local"));
    let name = device
        .as_ref()
        .map_or_else(|| t.t("cast:thisComputer"), |d| d.name.clone());
    let airplay = device.as_ref().is_some_and(|d| d.kind == CastKind::Airplay);
    let volume_for = if selected && !airplay { id } else { None };
    let disabled = connecting.is_some() && !is_connecting;
    rsx! {
        li {
            button {
                r#type: "button",
                role: "menuitemradio",
                "aria-checked": "{selected}",
                class: "jr-cast__row",
                disabled,
                onmousedown: move |e: MouseEvent| e.stop_propagation(),
                onclick: move |_| on_choose.call(device.clone()),
                span { class: "jr-cast__icon", "aria-hidden": "true", Icon { name: icon, size: 18 } }
                span { class: "jr-text-body jr-truncate jr-cast__name", "{name}" }
                if is_connecting {
                    Spinner { size: "small" }
                } else if selected {
                    span { class: "jr-cast__check", Icon { name: "check", size: 18 } }
                }
            }
            if let Some(vid) = volume_for { DeviceVolume { device_id: vid } }
        }
    }
}
