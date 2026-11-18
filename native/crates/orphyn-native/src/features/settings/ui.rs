//! The list design components Settings uses (`design/lists/ListRow.tsx`, `controls/Toggle.tsx`,
//! `ValueMenu.tsx`), same DOM and classes as React.
use dioxus::prelude::*;

use crate::design::overlay::{use_overlay, MenuItem};
use crate::design::{Button, Icon};

#[component]
pub fn List(
    #[props(default = "inset")] variant: &'static str,
    #[props(default)] header: String,
    #[props(default)] footer: String,
    #[props(default)] label: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "jr-list-group jr-list-group--{variant}",
            if !header.is_empty() {
                div { class: "jr-list-group__header", "{header}" }
            }
            ul { class: "jr-list", "aria-label": "{label}", {children} }
            if !footer.is_empty() {
                div { class: "jr-list-group__footer", "{footer}" }
            }
        }
    }
}

#[component]
pub fn ListRow(
    title: Element,
    subtitle: Option<String>,
    leading: Option<Element>,
    trailing: Option<Element>,
    onclick: Option<EventHandler<MouseEvent>>,
    #[props(default = false)] selected: bool,
) -> Element {
    let interactive = if onclick.is_some() { " jr-row--interactive" } else { "" };
    let selected_cls = if selected { " jr-row--selected" } else { "" };
    rsx! {
        li { class: "jr-list__item",
            div {
                class: "jr-row{interactive}{selected_cls}",
                onclick: move |e| if let Some(h) = &onclick { h.call(e) },
                if let Some(l) = leading {
                    span { class: "jr-row__leading", {l} }
                }
                span { class: "jr-row__text",
                    span { class: "jr-row__title", {title} }
                    if let Some(s) = subtitle {
                        span { class: "jr-row__subtitle", "{s}" }
                    }
                }
                if let Some(t) = trailing {
                    span { class: "jr-row__trailing", {t} }
                }
            }
        }
    }
}

#[component]
pub fn Toggle(
    checked: bool,
    on_change: EventHandler<bool>,
    #[props(default)] label: String,
    #[props(default)] description: String,
) -> Element {
    let control = rsx! {
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
    };
    if label.is_empty() {
        return control;
    }
    rsx! {
        div { class: "jr-toggle-row",
            div { class: "jr-toggle-row__text",
                label { class: "jr-toggle-row__label", onclick: move |_| on_change.call(!checked), "{label}" }
                if !description.is_empty() {
                    span { class: "jr-toggle-row__description", "{description}" }
                }
            }
            {control}
        }
    }
}

/// A gray pill showing the current value; opens a checkmarked menu.
#[component]
pub fn ValueMenu<T: Clone + PartialEq + 'static>(
    label: String,
    value: T,
    options: Vec<(T, String)>,
    on_change: EventHandler<T>,
) -> Element {
    let overlay = use_overlay();
    let current = options
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, l)| l.clone())
        .unwrap_or_default();
    let opts = options.clone();
    let current_value = value.clone();
    rsx! {
        button {
            r#type: "button",
            class: "jr-button jr-button--gray jr-button--small",
            "aria-label": "{label}",
            "aria-haspopup": "menu",
            onmousedown: move |e| e.stop_propagation(),
            onclick: move |e| {
                let items = opts
                    .iter()
                    .map(|(v, l)| {
                        let v = v.clone();
                        MenuItem::Action {
                            label: l.clone(),
                            icon: None,
                            checked: Some(v == current_value),
                            destructive: false,
                            disabled: false,
                            on_select: Callback::new(move |_| on_change.call(v.clone())),
                        }
                    })
                    .collect();
                let c = e.client_coordinates();
                overlay.menu((c.x - 120.0).max(8.0), c.y + 12.0, items);
            },
            span { class: "jr-button__label", "{current}" }
            span { class: "jr-button__icon", "aria-hidden": "true", Icon { name: "chevrons-up-down", size: 16 } }
        }
    }
}

/// A small gray action button.
#[component]
pub fn GrayButton(onclick: EventHandler<MouseEvent>, icon: Option<&'static str>, children: Element) -> Element {
    rsx! {
        Button { variant: "gray", size: "small", icon, onclick: move |e| onclick.call(e), {children} }
    }
}
