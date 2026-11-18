//! The overlay layer: popovers and menus are not portals here but one host rendered last in the
//! shell (above everything, `position: fixed`), fed by a context. A screen opens a menu with
//! `use_overlay().menu(x, y, items)`; clicking outside, picking an item or Escape closes it.
//! Same DOM as `Popover` + `Menu` (`design/overlays`).
use dioxus::prelude::*;

use super::Icon;

/// One entry of a menu (`MenuItem` of `design/overlays/Menu.tsx`).
#[derive(Clone, PartialEq)]
pub enum MenuItem {
    Action {
        label: String,
        icon: Option<&'static str>,
        checked: Option<bool>,
        destructive: bool,
        disabled: bool,
        on_select: Callback<()>,
    },
    Separator,
    Header(String),
}

impl MenuItem {
    pub fn action(
        label: impl Into<String>,
        icon: Option<&'static str>,
        on_select: impl FnMut(()) + 'static,
    ) -> Self {
        MenuItem::Action {
            label: label.into(),
            icon,
            checked: None,
            destructive: false,
            disabled: false,
            on_select: Callback::new(on_select),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct OpenMenu {
    pub x: f64,
    pub y: f64,
    pub items: Vec<MenuItem>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Overlay {
    pub menu: Signal<Option<OpenMenu>>,
}

impl Overlay {
    /// Opens a menu with its top-left corner at (`x`, `y`) in window coordinates.
    pub fn menu(mut self, x: f64, y: f64, items: Vec<MenuItem>) {
        self.menu.set(Some(OpenMenu { x, y, items }));
    }

    pub fn close(mut self) {
        self.menu.set(None);
    }
}

pub fn use_overlay() -> Overlay {
    use_context::<Overlay>()
}

/// Rendered once, last in the shell.
#[component]
pub fn OverlayHost() -> Element {
    let overlay = use_overlay();
    let Some(open) = (overlay.menu)() else {
        return rsx! {};
    };
    let has_icon = open
        .items
        .iter()
        .any(|i| matches!(i, MenuItem::Action { icon: Some(_), .. }));
    let has_check = open.items.iter().any(|i| {
        matches!(
            i,
            MenuItem::Action {
                checked: Some(_),
                ..
            }
        )
    });
    rsx! {
        // Click-catcher: a click anywhere else closes the menu (the capture listener of Popover).
        div {
            style: "position: fixed; inset: 0; z-index: 999;",
            onmousedown: move |_| overlay.close(),
            oncontextmenu: move |e| { e.prevent_default(); overlay.close() },
        }
        div {
            class: "jr-popover jr-material-thick",
            role: "presentation",
            style: "left: {open.x}px; top: {open.y}px; z-index: 1000;",
            onkeydown: move |e| if e.key() == Key::Named(NamedKey::Escape) { overlay.close() },
            div { role: "menu", class: "jr-menu",
                for (i, item) in open.items.into_iter().enumerate() {
                    match item {
                        MenuItem::Separator => rsx! { div { key: "{i}", role: "separator", class: "jr-menu__separator" } },
                        MenuItem::Header(label) => rsx! { div { key: "{i}", role: "presentation", class: "jr-menu__header", "{label}" } },
                        MenuItem::Action { label, icon, checked, destructive, disabled, on_select } => {
                            let mut cls = String::from("jr-menu__item");
                            if destructive { cls.push_str(" jr-menu__item--destructive"); }
                            if disabled { cls.push_str(" jr-menu__item--disabled"); }
                            rsx! {
                                div {
                                    key: "{i}",
                                    "data-menu-item": "",
                                    role: if checked.is_some() { "menuitemcheckbox" } else { "menuitem" },
                                    "aria-disabled": disabled.then_some("true"),
                                    tabindex: "-1",
                                    class: "{cls}",
                                    onclick: move |_| {
                                        if !disabled {
                                            overlay.close();
                                            on_select.call(());
                                        }
                                    },
                                    if has_check {
                                        span { class: "jr-menu__check", "aria-hidden": "true",
                                            if checked == Some(true) { Icon { name: "check", size: 14, stroke_width: 2.6 } }
                                        }
                                    }
                                    if has_icon {
                                        span { class: "jr-menu__icon", "aria-hidden": "true",
                                            if let Some(n) = icon { Icon { name: n, size: 16 } }
                                        }
                                    }
                                    span { class: "jr-menu__label", "{label}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
