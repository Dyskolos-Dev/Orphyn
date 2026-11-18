//! `cards.tsx`: the poster and landscape cards of the video screens, their context menu, and the
//! item mutations (mark played, favourite) shared by every screen.
use dioxus::prelude::*;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::{keys, BaseItemDto};

use super::format::{
    is_played, item_path, play_path, position_ticks, progress_of, remaining_minutes,
};
use super::ui::{CardKind, MediaCard};
use crate::design::{use_overlay, MenuItem, Overlay};
use crate::features::music::use_base_url;
use crate::i18n::{use_i18n, I18n};
use crate::router::{use_router, Router};
use crate::state::{use_app, use_services, Services};

/// Image of an item at `max_width` px (React passes the same literal widths), `None` when the
/// item has no such image.
pub fn image(
    base: &str,
    item: &BaseItemDto,
    kind: ImageKind,
    max_width: u32,
    quality: u32,
) -> Option<String> {
    build_image_url(
        base,
        item,
        kind,
        &ImageOptions {
            max_width: Some(max_width),
            max_height: None,
            quality: Some(quality),
            tag: None,
        },
    )
}

/// 16:9 artwork: episode still, else thumb, else backdrop, else the poster (`landscapeImage`).
pub fn landscape_image(base: &str, item: &BaseItemDto, max_width: u32) -> Option<String> {
    let get = |k| image(base, item, k, max_width, 80);
    if item.is_type("Episode") {
        return get(ImageKind::Primary).or_else(|| get(ImageKind::Thumb));
    }
    get(ImageKind::Thumb)
        .or_else(|| get(ImageKind::Backdrop))
        .or_else(|| get(ImageKind::Primary))
}

fn type_icon(item: &BaseItemDto) -> &'static str {
    if item.is_type("Series") {
        "tv"
    } else {
        "film"
    }
}

/// The mutations of `useMarkPlayed` / `useToggleFavorite`: the cached item flips at once, the
/// server is called, then every query of this server is refreshed (item flags show up in many
/// lists: resume, next up, libraries...).
#[derive(Clone)]
pub struct ItemActions {
    svc: Services,
    session: Option<Session>,
}

pub fn use_item_actions() -> ItemActions {
    let app = use_app();
    ItemActions {
        svc: use_services(),
        session: (app.session)(),
    }
}

impl ItemActions {
    fn mutate(
        &self,
        id: &str,
        optimistic: impl Fn(&mut BaseItemDto) + Copy + 'static,
        call: impl FnOnce(
            Session,
            String,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let acc = session.account_id.clone();
        self.svc
            .cache
            .update::<BaseItemDto>(&keys::item(&acc, id), optimistic);
        let cache = self.svc.cache.clone();
        let fut = call(session, id.to_string());
        spawn(async move {
            fut.await;
            cache.invalidate(&keys::all(&acc));
        });
    }

    pub fn mark_played(&self, id: &str, played: bool) {
        self.mutate(
            id,
            move |i| {
                i.user_data.get_or_insert_with(Default::default).played = played;
            },
            move |s, id| Box::pin(async move { s.set_played(&id, played).await.is_ok() }),
        );
    }

    pub fn set_favourite(&self, id: &str, favourite: bool) {
        self.mutate(
            id,
            move |i| {
                i.user_data.get_or_insert_with(Default::default).is_favorite = favourite;
            },
            move |s, id| Box::pin(async move { s.set_favorite(&id, favourite).await.is_ok() }),
        );
    }
}

/// Right-click / "more" menu shared by every card: play, watched, favourite (`useItemMenu`).
pub fn item_menu(
    t: I18n,
    router: Router,
    actions: &ItemActions,
    item: &BaseItemDto,
) -> Vec<MenuItem> {
    if item.id.is_empty() {
        return Vec::new();
    }
    let id = item.id.clone();
    let played = is_played(item);
    let favourite = item.is_favorite();
    let mut items = Vec::new();
    if item.is_type("Movie") || item.is_type("Episode") {
        let path = play_path(&id, position_ticks(item));
        items.push(MenuItem::action(
            t.t("video:item.play"),
            Some("play"),
            move |_| router.push(&path),
        ));
        items.push(MenuItem::Separator);
    }
    let a = actions.clone();
    let pid = id.clone();
    items.push(MenuItem::action(
        if played {
            t.t("video:item.markUnwatched")
        } else {
            t.t("video:item.markWatched")
        },
        Some("check"),
        move |_| a.mark_played(&pid, !played),
    ));
    let a = actions.clone();
    items.push(MenuItem::action(
        if favourite {
            t.t("video:item.unfavourite")
        } else {
            t.t("video:item.favourite")
        },
        Some("heart"),
        move |_| a.set_favourite(&id, !favourite),
    ));
    items
}

/// Opens the card menu at the cursor.
fn open_menu(
    overlay: Overlay,
    t: I18n,
    router: Router,
    actions: &ItemActions,
    item: &BaseItemDto,
    e: &MouseEvent,
) {
    let c = e.client_coordinates();
    overlay.menu(c.x, c.y, item_menu(t, router, actions, item));
}

/// Movie / series poster with watched progress and an unwatched badge (`ItemPoster`). Renders
/// the card only: the caller wraps it in the shelf / grid cell.
#[component]
pub fn ItemPoster(item: BaseItemDto) -> Element {
    let t = use_i18n();
    let router = use_router();
    let base = use_base_url();
    let actions = use_item_actions();
    let overlay = use_overlay();
    let unplayed = if item.is_type("Series") {
        item.user_data
            .as_ref()
            .and_then(|u| u.unplayed_item_count)
            .unwrap_or(0)
    } else {
        0
    };
    let on_play = (item.is_type("Movie") && !item.id.is_empty()).then(|| {
        let path = play_path(&item.id, position_ticks(&item));
        EventHandler::new(move |_| router.push(&path))
    });
    let menu_item = item.clone();
    rsx! {
        MediaCard {
            kind: CardKind::Poster,
            title: item.name().to_string(),
            subtitle: item.production_year.map(|y| y.to_string()),
            image: image(&base, &item, ImageKind::Primary, 400, 80),
            to: item_path(&item.id),
            progress: progress_of(&item),
            badge: (unplayed > 0).then(|| unplayed.to_string()),
            fallback: type_icon(&item),
            on_play,
            on_context: move |e: MouseEvent| open_menu(overlay, t, router, &actions, &menu_item, &e),
        }
    }
}

/// Continue Watching / Next Up card, 16:9; episodes show the series name (`ItemLandscape`).
#[component]
pub fn ItemLandscape(item: BaseItemDto, #[props(default = true)] show_progress: bool) -> Element {
    let t = use_i18n();
    let router = use_router();
    let base = use_base_url();
    let actions = use_item_actions();
    let overlay = use_overlay();
    let is_episode = item.is_type("Episode");
    let left = remaining_minutes(&item);
    let progress = progress_of(&item);
    let episode_label = match (item.parent_index_number, item.index_number) {
        (Some(s), Some(e)) if is_episode => Some(t.tf(
            "video:item.episodeLabel",
            &[("season", s.to_string()), ("episode", e.to_string())],
        )),
        _ => None,
    };
    let title = if is_episode {
        item.series_name
            .clone()
            .unwrap_or_else(|| item.name().to_string())
    } else {
        item.name().to_string()
    };
    let subtitle = if is_episode {
        let s = [episode_label, item.name.clone()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        Some(s)
    } else {
        item.production_year.map(|y| y.to_string())
    };
    let caption = (show_progress && left > 0 && progress.is_some())
        .then(|| t.tf("video:item.minutesLeft", &[("minutes", left.to_string())]));
    let on_play = (!item.id.is_empty()).then(|| {
        let path = play_path(&item.id, position_ticks(&item));
        EventHandler::new(move |_| router.push(&path))
    });
    let menu_item = item.clone();
    rsx! {
        MediaCard {
            kind: CardKind::Landscape,
            title,
            subtitle,
            image: landscape_image(&base, &item, 640),
            to: item_path(&item.id),
            progress: if show_progress { progress } else { None },
            caption,
            fallback: type_icon(&item),
            on_play,
            on_context: move |e: MouseEvent| open_menu(overlay, t, router, &actions, &menu_item, &e),
        }
    }
}
