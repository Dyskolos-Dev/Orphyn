//! `TrailerButton.tsx`: the "Trailer" button of a detail page. Nothing when the item has no
//! trailer; one plays straight away; several open a small menu. A trailer file of the server plays
//! in Orphyn's player; a YouTube or other link opens in the default browser (there is no web
//! engine here to frame it).
use dioxus::prelude::*;
use orphyn_jellyfin::api::video::VideoItemExt;
use orphyn_jellyfin::BaseItemDto;

use super::data::{list_of, use_local_trailers};
use super::format::trailer_path;
use super::trailers::{open_external, pick_trailer_sources, youtube_watch_url, TrailerSource};
use super::ui::MenuButton;
use crate::design::{IconButton, MenuItem};
use crate::i18n::use_i18n;
use crate::router::{use_router, Router};

fn play(router: Router, source: &TrailerSource) {
    match source {
        TrailerSource::Local { item_id, .. } => router.push(&trailer_path(item_id)),
        TrailerSource::YouTube { video_id, .. } => open_external(&youtube_watch_url(video_id)),
        TrailerSource::External { url, .. } => open_external(url),
    }
}

/// The menu label of a trailer: its own name when unique, else "Trailer <n>".
fn label(trailer: &str, sources: &[TrailerSource], index: usize, name: &str) -> String {
    if !name.is_empty() && sources.iter().filter(|s| s.name() == name).count() == 1 {
        name.to_string()
    } else {
        format!("{trailer} {}", index + 1)
    }
}

#[component]
pub fn TrailerButton(item: BaseItemDto) -> Element {
    let t = use_i18n();
    let router = use_router();
    let local = use_local_trailers(&item.id, item.local_trailer_count());
    let sources = pick_trailer_sources(&list_of(&local), &item.remote_trailers());
    if sources.is_empty() {
        return rsx! {};
    }
    let name = t.t("video:item.trailer");
    if sources.len() == 1 {
        let only = sources[0].clone();
        return rsx! {
            IconButton {
                variant: "glass",
                size: "large",
                icon: "clapperboard",
                label: name,
                onclick: move |_| play(router, &only),
            }
        };
    }
    let items: Vec<MenuItem> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let s = s.clone();
            MenuItem::action(
                label(&name, &sources, i, s.name()),
                Some("clapperboard"),
                move |_| play(router, &s),
            )
        })
        .collect();
    rsx! {
        MenuButton {
            variant: "glass",
            size: "large",
            icon: "clapperboard",
            label: name,
            items,
            align_start: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_use_the_name_only_when_it_is_unique() {
        let s = |n: &str| TrailerSource::External {
            key: n.into(),
            name: n.into(),
            url: "https://x".into(),
        };
        let sources = vec![s("Official"), s("Teaser"), s("Teaser")];
        assert_eq!(label("Trailer", &sources, 0, "Official"), "Official");
        assert_eq!(label("Trailer", &sources, 1, "Teaser"), "Trailer 2");
        assert_eq!(label("Trailer", &sources, 2, ""), "Trailer 3");
    }
}
