//! The track "…" menu (`useTrackMenuWithPlaylists` + the radio entry of `TrackRows.tsx`): play
//! next, add to queue, add to playlist, start radio, favourite, go to the album / artist.
//!
//! ```ignore
//! let menu = use_track_menu();
//! onclick: move |e| menu.open(e.client_coordinates().x, e.client_coordinates().y, track.clone())
//! ```
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::client::Q;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{keys, BaseItemDto, ItemsParams};
use orphyn_player::music::MusicPlayer;

use super::radio::start_radio;
use crate::design::{use_overlay, MenuItem, Overlay};
use crate::features::music::{toggle_favorite, track_artist_id};
use crate::i18n::{use_i18n, I18n};
use crate::router::{use_router, Router};
use crate::state::{use_app, use_services, Services};

#[derive(Clone)]
pub struct TrackMenu {
    t: I18n,
    player: MusicPlayer,
    router: Router,
    svc: Services,
    session: Option<Session>,
    overlay: Overlay,
}

pub fn use_track_menu() -> TrackMenu {
    let svc = use_services();
    TrackMenu {
        t: use_i18n(),
        player: svc.music.clone(),
        router: use_router(),
        session: (use_app().session)(),
        overlay: use_overlay(),
        svc,
    }
}

impl TrackMenu {
    /// Opens the menu of `track` with its top-left corner at (`x`, `y`).
    pub fn open(&self, x: f64, y: f64, track: BaseItemDto) {
        self.overlay.menu(x, y, self.items(x, y, track));
    }

    /// The entries of a song's menu; `x` / `y` place the playlist picker that "Add to Playlist…"
    /// opens in its place.
    pub fn items(&self, x: f64, y: f64, track: BaseItemDto) -> Vec<MenuItem> {
        let t = self.t;
        let router = self.router;
        let (p1, p2) = (self.player.clone(), self.player.clone());
        let (t1, t2) = (track.clone(), track.clone());
        let favorite = self.is_favorite(&track);
        let mut items = vec![
            MenuItem::action(t.t("music:actions.playNext"), Some("list-start"), move |_| {
                p1.play_next(std::slice::from_ref(&t1))
            }),
            MenuItem::action(t.t("music:actions.addToQueue"), Some("list-end"), move |_| {
                p2.add_to_queue(std::slice::from_ref(&t2))
            }),
        ];
        if self.session.is_some() {
            let (me, id) = (self.clone(), track.id.clone());
            items.push(MenuItem::action(
                t.t("music:playlistMenu.add"),
                Some("list-plus"),
                move |_| me.pick_playlist(x, y, vec![id.clone()]),
            ));
            let (player, session, id) = (self.player.clone(), self.session.clone(), track.id.clone());
            items.push(MenuItem::action(
                t.t("mixes:radio.start"),
                Some("radio"),
                move |_| {
                    let Some(s) = session.clone() else {
                        return;
                    };
                    let (p, id) = (player.clone(), id.clone());
                    spawn(async move {
                        // The radio notice of the React app is a toast; the queue shows the radio.
                        if start_radio(p, s, id).await.is_none() {
                            log::warn!("[radio] could not start the radio");
                        }
                    });
                },
            ));
        }
        items.push(MenuItem::Separator);
        let (svc, session, fav_track) = (self.svc.clone(), self.session.clone(), track.clone());
        items.push(MenuItem::action(
            if favorite {
                t.t("music:actions.unfavorite")
            } else {
                t.t("music:actions.favorite")
            },
            Some("heart"),
            move |_| toggle_favorite(&svc, session.clone(), &fav_track, !favorite),
        ));
        if let Some(album) = track.album_id.clone() {
            items.push(MenuItem::action(
                t.t("music:actions.goToAlbum"),
                Some("disc-3"),
                move |_| router.push(&format!("/music/album/{album}")),
            ));
        }
        if let Some(artist) = track_artist_id(&track) {
            items.push(MenuItem::action(
                t.t("music:actions.goToArtist"),
                Some("mic-vocal"),
                move |_| router.push(&format!("/music/artist/{artist}")),
            ));
        }
        items
    }

    /// The cached item wins over the (possibly stale) row, like `CurrentTrackHeart`.
    fn is_favorite(&self, track: &BaseItemDto) -> bool {
        self.session
            .as_ref()
            .and_then(|s| {
                self.svc
                    .cache
                    .peek::<BaseItemDto>(&keys::item(&s.account_id, &track.id))
                    .data
            })
            .map(|c| c.is_favorite())
            .unwrap_or_else(|| track.is_favorite())
    }

    /// Lists the playlists in a menu at (`x`, `y`) and adds `ids` to the picked one.
    fn pick_playlist(&self, x: f64, y: f64, ids: Vec<String>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let (overlay, svc, t) = (self.overlay, self.svc.clone(), self.t);
        spawn(async move {
            let params = ItemsParams {
                include_item_types: vec!["Playlist".into()],
                sort_by: vec!["SortName".into()],
                page_size: Some(100),
                fields: Some(vec![]),
                ..ItemsParams::default()
            };
            let lists = session
                .items_page(&params, 0)
                .await
                .map(|p| p.items)
                .unwrap_or_default();
            let mut items = vec![MenuItem::Header(t.t("music:playlistMenu.add"))];
            for list in lists {
                let (session, svc, ids) = (session.clone(), svc.clone(), ids.clone());
                let name = list.name().to_string();
                items.push(MenuItem::action(name, Some("list-music"), move |_| {
                    add_to_playlist(svc.clone(), session.clone(), list.id.clone(), ids.clone())
                }));
            }
            overlay.menu(x, y, items);
        });
    }
}

/// `POST /Playlists/{id}/Items` (`addToPlaylist`), then refreshes the playlist's cached items.
fn add_to_playlist(svc: Services, session: Session, playlist: String, ids: Vec<String>) {
    let url = session.client.authed_url(
        &format!("/Playlists/{playlist}/Items"),
        &[
            ("ids", Some(Q::List(ids))),
            ("userId", Some(Q::Str(session.user_id.clone()))),
        ],
    );
    spawn(async move {
        let done = tokio::task::spawn_blocking(move || {
            orphyn_player::music::http::post_empty(&url, Duration::from_secs(20))
        })
        .await;
        match done {
            Ok(Ok(())) => svc
                .cache
                .invalidate(&keys::playlist_items(&session.account_id, &playlist)),
            _ => log::warn!("[playlist] could not add the songs"),
        }
    });
}
