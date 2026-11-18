//! `ShellPage` (page frame: draggable toolbar, large title collapsing on scroll) and `StubPage`.
use std::rc::Rc;

use dioxus::prelude::*;

use crate::design::{EmptyState, IconButton, Toolbar};
use crate::i18n::use_i18n;
use crate::router::use_router;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageHeader {
    /// Large title that collapses into the toolbar on scroll.
    Large,
    /// Toolbar transparent over the first child (a hero), material once scrolled away.
    Hero,
    /// The title is in the toolbar, unless the content has its own large title registered with
    /// [`PageContext::title_el`]: the toolbar then shows it once that one scrolled away.
    Inline,
}

/// What a page's content shares with its frame.
#[derive(Clone, Copy, PartialEq)]
pub struct PageContext {
    /// The content's own large title (`data-page-title` in React): set it from `onmounted`.
    pub title_el: Signal<Option<Rc<MountedData>>>,
}

pub fn use_page() -> PageContext {
    use_context::<PageContext>()
}

/// The height of the window toolbar (`--toolbar-height`).
const TOOLBAR_HEIGHT: f64 = 52.0;

#[component]
pub fn ShellPage(
    title: String,
    subtitle: Option<String>,
    #[props(default = PageHeader::Large)] header: PageHeader,
    show_back: Option<bool>,
    #[props(default = false)] gutter: bool,
    actions: Option<Element>,
    header_accessory: Option<Element>,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let t = use_i18n();
    let router = use_router();
    let window = dioxus_native::use_window();
    let page = use_context_provider(|| PageContext {
        title_el: Signal::new(None),
    });
    let mut collapsed = use_signal(|| header == PageHeader::Inline);
    let mut sentinel: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let mut hero: Signal<Option<Rc<MountedData>>> = use_signal(|| None);

    // Collapse rule of ShellPage.tsx, re-evaluated on every scroll of the main scroller.
    let scroll = (router.scroll_y)();
    use_effect(use_reactive!(|scroll, header| {
        let _ = scroll;
        let mark = match header {
            PageHeader::Inline => (page.title_el)(),
            PageHeader::Hero => hero(),
            PageHeader::Large => sentinel(),
        };
        spawn(async move {
            let next = match mark {
                None => header == PageHeader::Inline,
                Some(el) => {
                    // A just-mounted element has no layout yet (0x0): wait a few frames.
                    let mut rect = el.get_client_rect().await.ok();
                    for _ in 0..20 {
                        if rect.is_some_and(|r| r.size.height > 0.0) {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                        rect = el.get_client_rect().await.ok();
                    }
                    match rect {
                        Some(r) if r.size.height > 0.0 => {
                            let edge = match header {
                                PageHeader::Large => r.origin.y,
                                _ => r.origin.y + r.size.height,
                            };
                            edge <= TOOLBAR_HEIGHT
                        }
                        _ => header == PageHeader::Inline,
                    }
                }
            };
            if *collapsed.peek() != next {
                collapsed.set(next);
            }
        });
    }));

    let can_go_back = show_back.unwrap_or(!router.history.read().is_first());
    let header_cls = match header {
        PageHeader::Large => "large",
        PageHeader::Hero => "hero",
        PageHeader::Inline => "inline",
    };
    let gutter_cls = if gutter {
        " jr-page__content--gutter"
    } else {
        ""
    };
    let is_collapsed = collapsed();
    let _ = &mut hero;
    let leading = if can_go_back {
        rsx! {
            IconButton {
                icon: "chevron-left",
                label: t.t("shell:nav.back"),
                stroke_width: 2.4,
                onclick: move |_| router.back(),
            }
        }
    } else {
        rsx! {}
    };
    let appearance = if is_collapsed {
        "material"
    } else {
        "transparent"
    };
    rsx! {
        div { class: "jr-page jr-page--{header_cls} {class}",
            div {
                class: "jr-page__toolbar",
                ondoubleclick: {
                    let w = window.clone();
                    move |_| crate::window::toggle_maximize(&w)
                },
                onmousedown: move |e| crate::window::drag_region_mousedown(&window, &e),
                Toolbar {
                    title: title.clone(),
                    show_title: is_collapsed,
                    appearance,
                    leading,
                    trailing: actions,
                }
            }
            if header == PageHeader::Large {
                header { class: "jr-page__header",
                    div { class: "jr-page__titles",
                        h1 { class: "jr-page__title", "{title}" }
                        if let Some(s) = subtitle { p { class: "jr-page__subtitle", "{s}" } }
                    }
                    if let Some(a) = header_accessory { div { class: "jr-page__accessory", {a} } }
                    div {
                        class: "jr-page__sentinel",
                        "aria-hidden": "true",
                        onmounted: move |e| sentinel.set(Some(e.data())),
                    }
                }
            }
            if header == PageHeader::Inline {
                h1 { class: "jr-visually-hidden", "{title}" }
            }
            div { class: "jr-page__content{gutter_cls}", {children} }
        }
    }
}

/// Temporary page of every route whose screen is not ported yet (`StubPage.tsx`).
#[component]
pub fn StubPage(name: String) -> Element {
    let t = use_i18n();
    let router = use_router();
    let path = router.route().path();
    rsx! {
        ShellPage { title: name.clone(),
            EmptyState {
                icon: "construction",
                title: t.tf("stub", &[("name", name.clone())]),
                message: rsx! { code { "{path}" } },
            }
        }
    }
}
