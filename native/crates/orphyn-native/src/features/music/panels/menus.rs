//! The track "…" menu (`useTrackMenuWithPlaylists` + the radio entry of `TrackRows.tsx`): play
//! next, add to queue, add to playlist, start radio, favourite, go to the album / artist.
//!
//! ```ignore
//! let menu = use_track_menu();
//! onclick: move |e| menu.open(e.client_coordinates().x, e.client_coordinates().y, track.clone())
//! ```
use dioxus::prelude::*;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{keys, BaseItemDto, ItemsParams};
use orphyn_player::music::MusicPlayer;

use super::super::playlists::{add_to_playlist, use_playlist_ui, PlaylistUi};
use super::radio::use_radio_starter;
use crate::design::toast::{use_toasts, Toasts};
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
    toasts: Toasts,
    ui: PlaylistUi,
    radio: std::rc::Rc<dyn Fn(String)>,
}

pub fn use_track_menu() -> TrackMenu {
    let svc = use_services();
    TrackMenu {
        t: use_i18n(),
        player: svc.music.clone(),
        router: use_router(),
        session: (use_app().session)(),
        overlay: use_overlay(),
        toasts: use_toasts(),
        ui: use_playlist_ui(),
        radio: std::rc::Rc::new(use_radio_starter()),
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
            MenuItem::action(
                t.t("music:actions.playNext"),
                Some("list-start"),
                move |_| p1.play_next(std::slice::from_ref(&t1)),
            ),
            MenuItem::action(
                t.t("music:actions.addToQueue"),
                Some("list-end"),
                move |_| p2.add_to_queue(std::slice::from_ref(&t2)),
            ),
        ];
        if self.session.is_some() {
            let (me, id) = (self.clone(), track.id.clone());
            items.push(MenuItem::action(
                t.t("music:playlistMenu.add"),
                Some("list-plus"),
                move |_| me.pick_playlist(x, y, vec![id.clone()]),
            ));
            let (radio, id) = (self.radio.clone(), track.id.clone());
            items.push(MenuItem::action(
                t.t("mixes:radio.start"),
                Some("radio"),
                move |_| radio(id.clone()),
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

    /// `useContainerMenuWithPlaylists`: an album's menu gets "Add to Playlist…" after the queue
    /// actions. `radio` adds the album header's "Start Radio" right after Play / Shuffle.
    pub fn container_items(
        &self,
        x: f64,
        y: f64,
        item: &BaseItemDto,
        radio: bool,
        items: &mut Vec<MenuItem>,
    ) {
        if self.session.is_none() || !item.is_type("MusicAlbum") {
            return;
        }
        let (me, id) = (self.clone(), item.id.clone());
        let at = 5.min(items.len());
        items.insert(
            at,
            MenuItem::action(
                self.t.t("music:playlistMenu.add"),
                Some("list-plus"),
                move |_| me.pick_playlist(x, y, vec![id.clone()]),
            ),
        );
        if radio {
            let (start, id) = (self.radio.clone(), item.id.clone());
            let at = 3.min(items.len());
            items.insert(
                at,
                MenuItem::action(self.t.t("mixes:radio.start"), Some("radio"), move |_| {
                    start(id.clone())
                }),
            );
        }
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

    /// "New Playlist…" and the user's playlists in a menu at (`x`, `y`); adds `ids` to the picked
    /// one (`PlaylistPicker`).
    pub fn pick_playlist(&self, x: f64, y: f64, ids: Vec<String>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let (overlay, svc, t, toasts, ui) =
            (self.overlay, self.svc.clone(), self.t, self.toasts, self.ui);
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
            let new_ids = ids.clone();
            let mut items = vec![MenuItem::action(
                t.t("music:playlistMenu.new"),
                Some("plus"),
                move |_| ui.open_new(new_ids.clone()),
            )];
            if !lists.is_empty() {
                items.push(MenuItem::Separator);
            }
            for list in lists {
                let (session, svc, ids) = (session.clone(), svc.clone(), ids.clone());
                let name = list.name().to_string();
                items.push(MenuItem::action(
                    name.clone(),
                    Some("list-music"),
                    move |_| {
                        add_to_playlist(
                            svc.clone(),
                            session.clone(),
                            toasts,
                            t,
                            list.id.clone(),
                            name.clone(),
                            ids.clone(),
                        )
                    },
                ));
            }
            overlay.menu(x, y, items);
        });
    }
}
