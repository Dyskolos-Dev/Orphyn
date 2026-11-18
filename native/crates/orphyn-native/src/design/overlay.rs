//! The overlay layer: popovers and menus are not portals here but one host rendered last in the
//! shell (above everything, `position: fixed`), fed by a context. A screen opens a menu with
//! `use_overlay().menu(x, y, items)`; clicking outside, picking an item or Escape closes it.
//! Same DOM as `Popover` + `Menu` (`design/overlays`).
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use super::Icon;

/// Viewport margin kept around a popover (`max-width: calc(100vw - 16px)` of `.jr-popover`).
const EDGE: f64 = 8.0;

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

/// Blitz sizes a `position: fixed` box that only has `left`/`top` to the whole free space instead
/// of its content, so the popover gets an explicit size estimated from its entries (the metrics of
/// `.jr-menu`: 5px padding, 26px items, callout text).
fn menu_size(items: &[MenuItem], has_icon: bool, has_check: bool) -> (f64, f64) {
    let (mut w, mut h) = (0.0_f64, 10.0_f64);
    for item in items {
        match item {
            MenuItem::Separator => h += 10.5,
            MenuItem::Header(l) => {
                h += 22.0;
                w = w.max(l.chars().count() as f64 * 7.4 + 20.0);
            }
            MenuItem::Action { label, .. } => {
                h += 26.0;
                let extra =
                    20.0 + if has_icon { 24.0 } else { 0.0 } + if has_check { 22.0 } else { 0.0 };
                w = w.max(label.chars().count() as f64 * 7.4 + extra);
            }
        }
    }
    (w.clamp(180.0, 420.0) + 10.0, h)
}

/// Rendered once, last in the shell.
#[component]
pub fn OverlayHost() -> Element {
    let overlay = use_overlay();
    let win = dioxus_native::use_window();
    let mut cursor: Signal<Option<usize>> = use_signal(|| None);
    let mut mounted: Signal<HashMap<usize, Rc<MountedData>>> = use_signal(HashMap::new);
    let Some(open) = (overlay.menu)() else {
        return rsx! {};
    };
    // Enabled entries in order, for the arrow keys.
    let enabled: Vec<usize> = open
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            matches!(
                i,
                MenuItem::Action {
                    disabled: false,
                    ..
                }
            )
        })
        .map(|(i, _)| i)
        .collect();
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

    // Kept inside the window.
    let (mw, mh) = menu_size(&open.items, has_icon, has_check);
    let scale = win.scale_factor().max(0.1);
    let size = win.surface_size();
    let (vw, vh) = (size.width as f64 / scale, size.height as f64 / scale);
    let width = mw.min((vw - 2.0 * EDGE).max(120.0));
    let height = mh.min((vh - 2.0 * EDGE).max(60.0));
    let left = open.x.min(vw - width - EDGE).max(EDGE);
    let top = open.y.min(vh - height - EDGE).max(EDGE);

    let focus_at = move |idx: usize| {
        cursor.set(Some(idx));
        if let Some(m) = mounted.peek().get(&idx).cloned() {
            spawn(async move {
                let _ = m.set_focus(true).await;
            });
        }
    };
    let on_key = move |e: KeyboardEvent| {
        let cur = *cursor.peek();
        let next = match e.key() {
            Key::Named(NamedKey::Escape) => {
                overlay.close();
                return;
            }
            Key::Named(NamedKey::ArrowDown) => match cur {
                None => enabled.first().copied(),
                Some(c) => enabled
                    .iter()
                    .copied()
                    .find(|i| *i > c)
                    .or(enabled.first().copied()),
            },
            Key::Named(NamedKey::ArrowUp) => match cur {
                None => enabled.last().copied(),
                Some(c) => enabled
                    .iter()
                    .rev()
                    .copied()
                    .find(|i| *i < c)
                    .or(enabled.last().copied()),
            },
            Key::Named(NamedKey::Home) => enabled.first().copied(),
            Key::Named(NamedKey::End) => enabled.last().copied(),
            _ => return,
        };
        e.prevent_default();
        if let Some(n) = next {
            focus_at(n);
        }
    };

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
            tabindex: "-1",
            style: "left: {left}px; top: {top}px; width: {width}px; height: {height}px; z-index: 1000;",
            // Focus lands on the popover so Escape and the arrows work at once.
            onmounted: move |e| {
                cursor.set(None);
                mounted.write().clear();
                let m = e.data();
                spawn(async move { let _ = m.set_focus(true).await; });
            },
            onkeydown: on_key,
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
                                    onmounted: move |e| { mounted.write().insert(i, e.data()); },
                                    onmouseenter: move |_| if !disabled { focus_at(i) },
                                    onkeydown: move |e| {
                                        if !disabled && (e.key() == Key::Named(NamedKey::Enter) || e.key() == Key::Character(" ".into())) {
                                            e.prevent_default();
                                            overlay.close();
                                            on_select.call(());
                                        }
                                    },
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
