//! `components/DetailHeader.tsx`: the artwork + title + Play / Shuffle / favourite / more header
//! of the playlist page.
use dioxus::prelude::*;
use orphyn_jellyfin::BaseItemDto;

use crate::design::{use_overlay, Artwork, Button, IconButton, MenuItem};
use crate::features::music::{art, toggle_favorite, use_base_url};
use crate::i18n::use_i18n;
use crate::shell::use_page;
use crate::state::{use_app, use_services};

#[component]
pub fn DetailHeader(
    item: BaseItemDto,
    /// Replaces the plain title (an inline-editable name).
    title: Option<Element>,
    fallback: &'static str,
    subtitle: Option<String>,
    meta: Option<String>,
    description: Option<String>,
    on_play: EventHandler<()>,
    on_shuffle: EventHandler<()>,
    menu: Vec<MenuItem>,
) -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let overlay = use_overlay();
    let page = use_page();
    let base = use_base_url();
    let favorite = item.is_favorite();
    let fav_item = item.clone();
    rsx! {
        header { class: "jrm-detail",
            Artwork { src: art(&base, &item, 232), shadow: true, fallback, class: "jrm-detail__art" }
            div { class: "jrm-detail__info",
                h1 {
                    class: "jr-text-title-1 jr-color-primary jrm-detail__title",
                    "data-page-title": "",
                    onmounted: move |e| page.title_el.clone().set(Some(e.data())),
                    if let Some(el) = title { {el} } else { "{item.name()}" }
                }
                if let Some(s) = subtitle { h3 { class: "jr-text-title-3 jr-color-primary jrm-detail__subtitle", "{s}" } }
                if let Some(m) = meta { p { class: "jr-text-footnote jr-color-secondary jrm-detail__meta", "{m}" } }
                if let Some(d) = description { p { class: "jr-text-callout jr-color-secondary jrm-detail__desc", "{d}" } }
                div { class: "jrm-detail__actions",
                    Button { icon: "play", icon_fill: true, onclick: move |_| on_play.call(()), {t.t("music:actions.play")} }
                    Button { variant: "tinted", icon: "shuffle", onclick: move |_| on_shuffle.call(()), {t.t("music:actions.shuffle")} }
                    IconButton {
                        icon: "heart",
                        fill: favorite,
                        label: if favorite { t.t("music:actions.unfavorite") } else { t.t("music:actions.favorite") },
                        variant: "gray",
                        pressed: favorite,
                        onclick: move |_| toggle_favorite(&svc, app.session.peek().clone(), &fav_item, !favorite),
                    }
                    IconButton {
                        icon: "ellipsis",
                        label: t.t("music:actions.more"),
                        variant: "gray",
                        onclick: move |e: MouseEvent| {
                            let p = e.client_coordinates();
                            overlay.menu(p.x, p.y, menu.clone());
                        },
                    }
                }
            }
        }
    }
}
