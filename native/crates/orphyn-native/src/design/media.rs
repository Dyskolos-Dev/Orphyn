//! `Shelf` and the cards of `design/media` (`MediaCard` and its `SquareCard` shape).
use dioxus::prelude::*;

use super::{Artwork, Icon, IconButton};
use crate::i18n::use_i18n;
use crate::router::use_router;

/// `Shelf`: a titled horizontal row of cards. Paging chevrons need the scroll width: they are
/// shown when the list overflows (measured once mounted).
#[component]
pub fn Shelf(
    title: Option<String>,
    subtitle: Option<String>,
    #[props(default = "square")] kind: &'static str,
    see_all_to: Option<String>,
    children: Element,
) -> Element {
    let t = use_i18n();
    let router = use_router();
    let mut list: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);
    let mut edges = use_signal(|| (false, false));
    let page = move |dir: f64| {
        if let Some(el) = list.peek().clone() {
            spawn(async move {
                let (Ok(rect), Ok(off)) =
                    (el.get_client_rect().await, el.get_scroll_offset().await)
                else {
                    return;
                };
                let x = (off.x + dir * rect.size.width * 0.9).max(0.0);
                let _ = el
                    .scroll(
                        dioxus::html::geometry::PixelsVector2D::new(x, 0.0),
                        dioxus::html::ScrollBehavior::Smooth,
                    )
                    .await;
            });
        }
    };
    rsx! {
        section { class: "jr-shelf jr-shelf--{kind}", "aria-label": title.clone(),
            if title.is_some() || see_all_to.is_some() {
                header { class: "jr-shelf__header",
                    div { class: "jr-shelf__titles",
                        if let Some(tt) = &title { h2 { class: "jr-shelf__title", "{tt}" } }
                        if let Some(s) = &subtitle { p { class: "jr-shelf__subtitle", "{s}" } }
                    }
                    if let Some(to) = see_all_to.clone() {
                        a { class: "jr-shelf__see-all", onclick: move |_| router.push(&to), {t.t("shell:design.seeAll")} }
                    }
                }
            }
            div { class: "jr-shelf__viewport",
                ul {
                    class: "jr-shelf__list",
                    onmounted: move |e| {
                        let el = e.data();
                        list.set(Some(el.clone()));
                        async move {
                            if let (Ok(r), Ok(s)) = (el.get_client_rect().await, el.get_scroll_size().await) {
                                edges.set((false, s.width > r.size.width + 1.0));
                            }
                        }
                    },
                    onscroll: move |e| {
                        let left = e.scroll_left();
                        let max = (e.scroll_width() - e.client_width()) as f64;
                        edges.set((left > 1.0, left < max - 1.0));
                    },
                    {children}
                }
                button {
                    r#type: "button", tabindex: "-1",
                    class: "jr-shelf__chevron jr-shelf__chevron--back",
                    "aria-label": t.t("shell:design.scrollBack"),
                    hidden: !edges().0,
                    onclick: move |_| page(-1.0),
                    Icon { name: "chevron-left", size: 24 }
                }
                button {
                    r#type: "button", tabindex: "-1",
                    class: "jr-shelf__chevron jr-shelf__chevron--forward",
                    "aria-label": t.t("shell:design.scrollForward"),
                    hidden: !edges().1,
                    onclick: move |_| page(1.0),
                    Icon { name: "chevron-right", size: 24 }
                }
            }
        }
    }
}

/// `SquareCard` (album, playlist) inside a shelf item `<li>`.
#[component]
pub fn SquareCard(
    title: String,
    subtitle: Option<String>,
    image: Option<String>,
    #[props(default = "music")] fallback: &'static str,
    to: String,
    on_play: Option<EventHandler<()>>,
) -> Element {
    let t = use_i18n();
    let router = use_router();
    let actions = if on_play.is_some() {
        " jr-card--has-actions"
    } else {
        ""
    };
    rsx! {
        li { class: "jr-shelf__item",
            div { class: "jr-card jr-card--square{actions}", style: "--jr-card-ratio: 1 / 1;",
                a { class: "jr-card__hit", draggable: "false", onclick: move |_| router.push(&to),
                    div { class: "jr-card__art",
                        Artwork { src: image, shape: "rounded", shadow: true, fallback }
                    }
                    div { class: "jr-card__text",
                        span { class: "jr-card__title", "{title}" }
                        if let Some(s) = subtitle { span { class: "jr-card__subtitle", "{s}" } }
                    }
                }
                if let Some(play) = on_play {
                    div { class: "jr-card__actions",
                        IconButton {
                            class: "jr-card__play",
                            variant: "glass",
                            icon: "play",
                            fill: true,
                            label: t.tf("shell:design.playItem", &[("title", title.clone())]),
                            onclick: move |_| play.call(()),
                        }
                    }
                }
            }
        }
    }
}
