//! System integrations that follow the players without a screen: Discord Rich Presence and the OS
//! media controls (SMTC / Now Playing / MPRIS) with the media keys. [`use_system_provider`] is
//! mounted once at the root; it also provides the [`System`] handles the Settings panes use.
use std::time::Instant;

use dioxus::prelude::*;
use dioxus_native::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use orphyn_core::discord::activity::Presence;
use orphyn_core::discord::DiscordService;
use orphyn_core::media_controls::{
    Action, MediaControlsService, Metadata, Playback, Status as OsStatus,
};
use orphyn_core::updater::UpdaterService;
use orphyn_jellyfin::BaseItemDto;
use orphyn_player::music::{MusicPlayer, PlayerStatus};
use orphyn_player::video::{PlaybackStatus, VideoController};

use crate::features::discord::presence::{
    artwork, music_presence, should_send, video_poster, video_presence,
};
use crate::state::{use_app, Services};

/// Shared handles of the integrations (cheap to clone).
#[derive(Clone)]
pub struct System {
    pub discord: DiscordService,
    pub updater: UpdaterService,
}

pub fn use_system() -> System {
    use_context::<System>()
}

fn hwnd(win: &dyn dioxus_native::winit::window::Window) -> usize {
    match win.window_handle().map(|h| h.as_raw()) {
        #[cfg(target_os = "windows")]
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get() as usize,
        _ => 0,
    }
}

/// Where the OS buttons go: the video when one is playing, else the music player.
fn apply_action(music: &MusicPlayer, video: &VideoController, action: Action) {
    let snap = video.snapshot();
    let video_active = matches!(
        snap.status,
        PlaybackStatus::Playing | PlaybackStatus::Paused | PlaybackStatus::Loading
    );
    if video_active {
        match action {
            Action::Play => video.resume(),
            Action::Pause => video.pause(),
            Action::Toggle => video.toggle(),
            Action::Stop => video.stop(),
            Action::Seek(s) => video.seek(s),
            Action::SeekBy(d) => video.seek((video.position() + d).max(0.0)),
            Action::Next | Action::Previous => {}
        }
        return;
    }
    match action {
        Action::Play => music.resume(),
        Action::Pause => music.pause(),
        Action::Toggle => music.toggle(),
        Action::Next => music.next(),
        Action::Previous => music.previous(),
        Action::Stop => music.stop(),
        Action::Seek(s) => music.seek(s),
        Action::SeekBy(d) => music.seek((music.progress().position + d).max(0.0)),
    }
}

/// What the OS overlay shows, derived from the same facts as the Discord presence.
struct Now {
    presence: Option<Presence>,
    metadata: Metadata,
    status: OsStatus,
    position: f64,
}

fn now_playing(
    base: &str,
    music: &MusicPlayer,
    video: &VideoController,
    video_item: Option<&BaseItemDto>,
) -> Option<Now> {
    let snap = video.snapshot();
    if let Some(p) = video_presence(
        base,
        snap.item_id.as_deref(),
        video_item,
        snap.status,
        snap.position_seconds,
        snap.duration_seconds,
    ) {
        let item = video_item?;
        let series = item
            .series_name
            .clone()
            .filter(|_| item.item_type.as_deref() == Some("Episode"));
        let duration = match &p {
            Presence::Video { duration_sec, .. } => *duration_sec,
            Presence::Music { duration_sec, .. } => *duration_sec,
        };
        return Some(Now {
            status: if snap.status == PlaybackStatus::Paused {
                OsStatus::Paused
            } else {
                OsStatus::Playing
            },
            position: snap.position_seconds,
            metadata: Metadata {
                title: item.name.clone().unwrap_or_default(),
                artist: series,
                album: None,
                artwork_url: video_poster(base, item),
                duration_sec: Some(duration),
            },
            presence: Some(p),
        });
    }
    let state = music.state();
    let progress = music.progress();
    let track = state.current().map(|q| &*q.track);
    let p = music_presence(
        base,
        track,
        state.status,
        progress.position,
        progress.duration,
    )?;
    let track = track?;
    let duration = match &p {
        Presence::Music { duration_sec, .. } | Presence::Video { duration_sec, .. } => {
            *duration_sec
        }
    };
    Some(Now {
        status: if state.status == PlayerStatus::Paused {
            OsStatus::Paused
        } else {
            OsStatus::Playing
        },
        position: progress.position,
        metadata: Metadata {
            title: track.name.clone().unwrap_or_default(),
            artist: if track.artists.is_empty() {
                track.album_artist.clone()
            } else {
                Some(track.artists.join(", "))
            },
            album: track.album.clone(),
            artwork_url: artwork(base, track),
            duration_sec: Some(duration),
        },
        presence: Some(p),
    })
}

/// Mounted once at the root: starts the services, mirrors the players to them, and provides
/// [`System`].
pub fn use_system_provider(svc: &Services) {
    let win = dioxus_native::use_window();
    let app = use_app();
    let system = use_context_provider(|| System {
        discord: DiscordService::start(),
        updater: UpdaterService::new(&app_version(), !cfg!(debug_assertions)),
    });
    let (music, video) = (svc.music.clone(), svc.video.clone());
    use_hook(move || {
        let handle = hwnd(win.as_ref());
        let (am, av) = (music.clone(), video.clone());
        let controls = MediaControlsService::start(handle, move |a| apply_action(&am, &av, a));
        let discord = system.discord.clone();
        let updater = system.updater.clone();
        spawn(updater.clone().run_schedule());

        let mut state_rx = music.subscribe_state();
        let mut progress_rx = music.subscribe_progress();
        let mut video_rx = video.subscribe();
        let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel::<BaseItemDto>();
        spawn(async move {
            let mut video_item: Option<BaseItemDto> = None;
            let mut fetching: Option<String> = None;
            let mut sent: Option<(Option<Presence>, Instant)> = None;
            let mut last_os: Option<(OsStatus, String)> = None;
            loop {
                tokio::select! {
                    r = state_rx.changed() => if r.is_err() { break },
                    r = progress_rx.changed() => if r.is_err() { break },
                    r = video_rx.changed() => if r.is_err() { break },
                    Some(item) = item_rx.recv() => {
                        fetching = None;
                        video_item = Some(item);
                    }
                }
                let Some(session) = app.session.peek().clone() else {
                    continue;
                };
                let base = session.client.base_url().to_string();
                // A video whose metadata is not loaded yet is fetched; it must not let a stale
                // music presence flash meanwhile.
                let snap = video.snapshot();
                if let Some(id) = snap.item_id.as_deref() {
                    let active =
                        !matches!(snap.status, PlaybackStatus::Idle | PlaybackStatus::Ended);
                    let known = video_item.as_ref().is_some_and(|i| i.id == id);
                    if active && !known && fetching.as_deref() != Some(id) {
                        fetching = Some(id.to_string());
                        let (tx, id, session) = (item_tx.clone(), id.to_string(), session.clone());
                        spawn(async move {
                            if let Ok(item) = session.item(&id).await {
                                let _ = tx.send(item);
                            }
                        });
                    }
                }
                let now = now_playing(&base, &music, &video, video_item.as_ref());
                let presence = now.as_ref().and_then(|n| n.presence.clone());
                let elapsed = sent
                    .as_ref()
                    .map_or(0.0, |(_, at)| at.elapsed().as_secs_f64());
                if should_send(
                    sent.as_ref().and_then(|(p, _)| p.as_ref()),
                    presence.as_ref(),
                    elapsed,
                ) {
                    sent = Some((presence.clone(), Instant::now()));
                    discord.set_presence(presence);
                    // The OS overlay follows the same triggers: item, state or a seek.
                    match &now {
                        Some(n) => {
                            let key = (n.status, n.metadata.title.clone());
                            if last_os.as_ref() != Some(&key) {
                                controls.set_metadata(n.metadata.clone());
                                last_os = Some(key);
                            }
                            controls.set_playback(Playback {
                                status: n.status,
                                position_sec: Some(n.position),
                            });
                        }
                        None => {
                            if last_os.take().is_some() {
                                controls.clear();
                            }
                        }
                    }
                }
            }
            discord.set_presence(None);
            controls.clear();
        });
    });
}

/// The app version shown to people and compared with releases: the one of the root
/// `package.json` (the release number shared by every build of Orphyn).
pub fn app_version() -> String {
    const PACKAGE_JSON: &str = include_str!("../../../../../../package.json");
    serde_json::from_str::<serde_json::Value>(PACKAGE_JSON)
        .ok()
        .and_then(|v| {
            v.get("version")
                .and_then(|s| s.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}
