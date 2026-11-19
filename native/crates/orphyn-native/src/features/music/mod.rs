//! Music feature: the reference port (album page, mini player, Now Playing sheet). See
//! native/PORTING.md for the patterns it demonstrates.
mod album;
pub mod library;
mod mini_player;
mod now_playing;
pub mod panels;
pub mod playlists;
pub mod rows;
mod transport;

use std::sync::Arc;

use dioxus::prelude::*;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::{keys, BaseItemDto};
use orphyn_player::music::{PlayerState, Progress};

pub use album::AlbumPage;
pub use mini_player::MiniPlayer;
pub use now_playing::NowPlaying;

use crate::state::{use_app, Services};

/// Which panel the Now Playing sheet shows (`NowPlayingView` of `ui-state.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NowPlayingView {
    Player,
    Lyrics,
    Queue,
}

/// The player as signals (the `useMusicPlayer` selectors) plus the sheet's UI state
/// (`useNowPlayingUi`).
#[derive(Clone, Copy, PartialEq)]
pub struct MusicUi {
    pub state: Signal<Arc<PlayerState>>,
    pub progress: Signal<Progress>,
    pub open: Signal<bool>,
    pub view: Signal<NowPlayingView>,
}

impl MusicUi {
    pub fn show(mut self, view: NowPlayingView) {
        self.view.set(view);
        self.open.set(true);
    }

    /// `toggleView`: opens the sheet on `view`, or closes it when it already shows it.
    pub fn toggle_view(mut self, view: NowPlayingView) {
        if (self.open)() && (self.view)() == view {
            self.open.set(false);
        } else {
            self.show(view);
        }
    }
}

pub fn use_music() -> MusicUi {
    use_context::<MusicUi>()
}

/// Provides [`MusicUi`] at the root: two watch channels of the player turned into signals (no
/// polling: the coroutine sleeps until the player publishes).
pub fn use_music_provider(svc: &Services) -> MusicUi {
    let player = svc.music.clone();
    let ui = use_context_provider(|| MusicUi {
        state: Signal::new(player.state()),
        progress: Signal::new(player.progress()),
        open: Signal::new(false),
        view: Signal::new(NowPlayingView::Player),
    });
    use_hook(move || {
        let mut state_rx = player.subscribe_state();
        let mut progress_rx = player.subscribe_progress();
        let (mut state, mut progress) = (ui.state, ui.progress);
        spawn(async move {
            loop {
                tokio::select! {
                    r = state_rx.changed() => {
                        if r.is_err() { break; }
                        let s = state_rx.borrow_and_update().clone();
                        state.set(s);
                    }
                    r = progress_rx.changed() => {
                        if r.is_err() { break; }
                        let p = *progress_rx.borrow_and_update();
                        progress.set(p);
                    }
                }
            }
        });
    });
    ui
}

/// `art` of `components/hooks.tsx`: square artwork at twice `size` CSS pixels.
pub fn art(base: &str, item: &BaseItemDto, size: u32) -> Option<String> {
    build_image_url(
        base,
        item,
        ImageKind::Primary,
        &ImageOptions {
            max_width: Some(size * 2),
            max_height: Some(size * 2),
            quality: Some(80),
            tag: None,
        },
    )
}

/// `trackArt` of `player/track.ts`.
pub fn track_art(base: &str, item: &BaseItemDto, size: u32) -> Option<String> {
    build_image_url(
        base,
        item,
        ImageKind::Primary,
        &ImageOptions {
            max_width: Some(size),
            max_height: Some(size),
            quality: Some(85),
            tag: None,
        },
    )
}

/// Base URL of the signed-in server (empty when signed out: images then fall back).
pub fn use_base_url() -> String {
    let app = use_app();
    let s = (app.session)();
    s.map(|s| s.client.base_url().to_string())
        .unwrap_or_default()
}

/// `trackArtist`: every artist, else the album artist.
pub fn track_artist(t: &BaseItemDto) -> String {
    if !t.artists.is_empty() {
        t.artists.join(", ")
    } else {
        t.album_artist.clone().unwrap_or_default()
    }
}

/// `trackArtistId`: main artist, for the artist link.
pub fn track_artist_id(t: &BaseItemDto) -> Option<String> {
    t.artist_items
        .first()
        .and_then(|a| a.id.clone())
        .or_else(|| t.album_artists.first().and_then(|a| a.id.clone()))
}

/// `useToggleFavorite`: optimistic update of the cached item and album tracks, then the server.
pub fn toggle_favorite(svc: &Services, session: Option<Session>, item: &BaseItemDto, value: bool) {
    let Some(session) = session else {
        return;
    };
    let acc = session.account_id.clone();
    let id = item.id.clone();
    let set = move |t: &mut BaseItemDto| {
        t.user_data.get_or_insert_with(Default::default).is_favorite = value;
    };
    svc.cache.update::<BaseItemDto>(&keys::item(&acc, &id), set);
    if let Some(album) = item.album_id.as_deref().or(item.parent_id.as_deref()) {
        svc.cache
            .update::<Vec<BaseItemDto>>(&keys::album_tracks(&acc, album), |list| {
                list.iter_mut().filter(|t| t.id == id).for_each(set)
            });
    }
    let cache = svc.cache.clone();
    spawn(async move {
        if session.set_favorite(&id, value).await.is_err() {
            cache.invalidate(&keys::all(&acc));
        }
    });
}

/// `CurrentTrackHeart`: favourite heart of the playing track (reads the cached item first, so a
/// toggle anywhere shows here at once).
#[component]
pub fn CurrentTrackHeart(
    #[props(default = "plain")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
    #[props(default)] style: String,
) -> Element {
    let t = crate::i18n::use_i18n();
    let music = use_music();
    let app = use_app();
    let svc = crate::state::use_services();
    crate::query::use_cache_updates();
    let Some(track) = (music.state)().current().map(|q| q.track.clone()) else {
        return rsx! {};
    };
    let session = (app.session)();
    let cached = session.as_ref().and_then(|s| {
        svc.cache
            .peek::<BaseItemDto>(&keys::item(&s.account_id, &track.id))
            .data
    });
    let favorite = cached
        .map(|c| c.is_favorite())
        .unwrap_or(track.is_favorite());
    let style = if favorite {
        format!("color: var(--accent); {style}")
    } else {
        style
    };
    rsx! {
        crate::design::IconButton {
            icon: "heart",
            fill: favorite,
            variant,
            size,
            style,
            label: if favorite { t.t("music:actions.unfavorite") } else { t.t("music:actions.favorite") },
            pressed: favorite,
            onclick: move |_| {
                let mut item = (*track).clone();
                item.user_data.get_or_insert_with(Default::default).is_favorite = favorite;
                toggle_favorite(&svc, session.clone(), &item, !favorite);
            },
        }
    }
}

/// Returns `(seconds) => "1 h 12 min" / "48 min"` in the UI language (`useFormatTotal`).
pub fn format_total(t: crate::i18n::I18n, seconds: f64) -> String {
    let minutes = ((seconds / 60.0).round() as i64).max(1);
    if minutes >= 60 {
        t.tf(
            "music:album.durationHours",
            &[
                ("hours", (minutes / 60).to_string()),
                ("minutes", (minutes % 60).to_string()),
            ],
        )
    } else {
        t.tf(
            "music:album.durationMinutes",
            &[("minutes", minutes.to_string())],
        )
    }
}
