//! Casting UI (`features/cast`): device picker, the cast buttons of the music and video players,
//! the "Playing on" screen and the `/cast` page. The protocols live in `orphyn_core::cast`; the
//! music engine plays on a device through `MusicRemote`, the video through `video.rs`.
mod devices;
mod hub;
mod music;
mod page;
mod picker;
mod video;

use std::collections::HashMap;

use dioxus::prelude::*;
use orphyn_core::cast::{CastDevice, CastEvent, CastKind, CastSessionState};

pub use crate::design::toast::use_toasts;
pub use devices::CastScope;
pub use page::CastPage;
pub use picker::CastPicker;
pub use video::{VideoCastButton, VideoCastHost, VideoCastRequest};

use crate::i18n::use_i18n;
use crate::state::{use_app, use_services};
use hub::{hub, UiMsg};

/// Device the music player or the video player is currently playing on.
#[derive(Clone, Debug, PartialEq)]
pub struct CastTarget {
    pub device_id: String,
    pub name: String,
    pub kind: CastKind,
    pub scope: CastScope,
}

/// Cast state as signals (`store.ts` + the `useCastDevices` / `useCastSession` hooks).
#[derive(Clone, Copy, PartialEq)]
pub struct CastUi {
    pub devices: Signal<Vec<CastDevice>>,
    pub sessions: Signal<HashMap<String, CastSessionState>>,
    pub target: Signal<Option<CastTarget>>,
    /// Device a switch is in progress for (spinner in the picker); `"local"` = this computer.
    pub connecting: Signal<Option<String>>,
}

pub fn use_cast() -> CastUi {
    use_context::<CastUi>()
}

impl CastUi {
    pub fn target_of(self, scope: CastScope) -> Option<CastTarget> {
        (self.target)().filter(|t| t.scope == scope)
    }

    pub fn session_of(self, device_id: &str) -> Option<CastSessionState> {
        (self.sessions)().get(device_id).cloned()
    }

    pub fn set_target(mut self, device: Option<&CastDevice>, scope: CastScope) {
        self.target.set(device.map(|d| CastTarget {
            device_id: d.id.clone(),
            name: d.name.clone(),
            kind: d.kind,
            scope,
        }));
    }
}

/// Provides [`CastUi`] at the root and keeps it in step with the cast service.
pub fn use_cast_provider() -> CastUi {
    let t = use_i18n();
    let svc = use_services();
    let toasts = use_toasts();
    let ui = use_context_provider(|| CastUi {
        devices: Signal::new(Vec::new()),
        sessions: Signal::new(HashMap::new()),
        target: Signal::new(None),
        connecting: Signal::new(None),
    });
    use_hook(move || {
        let Some(mut rx) = hub().take_ui_receiver() else {
            return;
        };
        let (mut devices, mut sessions) = (ui.devices, ui.sessions);
        spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    UiMsg::Event(CastEvent::DevicesChanged(d)) => devices.set(d),
                    UiMsg::Event(CastEvent::SessionChanged(s)) => {
                        sessions.with_mut(|m| {
                            m.insert(s.device_id.clone(), s);
                        });
                    }
                    UiMsg::Event(CastEvent::Ended(e)) => {
                        let video = ui.target.peek().as_ref().is_some_and(|t| {
                            t.scope == CastScope::Video && t.device_id == e.device_id
                        });
                        if video {
                            video::stop_video_cast(ui, &svc, true);
                        }
                    }
                    UiMsg::Event(_) => {}
                    UiMsg::Failure { device_id, message } => {
                        let name = ui
                            .target
                            .peek()
                            .as_ref()
                            .filter(|t| t.device_id == device_id)
                            .map(|t| (t.name.clone(), t.scope));
                        let Some((name, scope)) = name else { continue };
                        toasts.error(
                            t.tf("cast:deviceFailed", &[("device", name)]),
                            Some(message),
                        );
                        match scope {
                            CastScope::Music => music_back_to_local(ui, &svc),
                            CastScope::Video => video::stop_video_cast(ui, &svc, false),
                        }
                    }
                }
            }
        });
    });
    ui
}

/// The remote output currently used by the music player (to release it on a switch).
static MUSIC_REMOTE: std::sync::Mutex<Option<std::sync::Arc<music::MusicRemote>>> =
    std::sync::Mutex::new(None);

fn take_music_remote() -> Option<std::sync::Arc<music::MusicRemote>> {
    MUSIC_REMOTE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
}

/// Device the music plays on (`None` = this computer).
pub fn current_music_device() -> Option<String> {
    MUSIC_REMOTE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|r| r.device_id().to_string())
}

/// Back to this computer: the music comes back paused, at the position the device had reached.
fn music_back_to_local(mut ui: CastUi, svc: &crate::state::Services) {
    let Some(previous) = take_music_remote() else {
        return;
    };
    svc.music.set_remote(None);
    previous.dispose();
    hub().service.disconnect(previous.device_id());
    ui.target.set(None);
}

/// Moves the music to `device`, or back to this computer with `None`. The engine carries the
/// current track, position and play / pause intent over.
pub fn switch_music_output(
    ui: CastUi,
    svc: &crate::state::Services,
    session: Option<orphyn_jellyfin::endpoints::Session>,
    quality: crate::preferences::AudioQuality,
    device: Option<CastDevice>,
) -> Result<(), String> {
    let Some(device) = device else {
        music_back_to_local(ui, svc);
        return Ok(());
    };
    if current_music_device().as_deref() == Some(device.id.as_str()) {
        return Ok(());
    }
    let session = session.ok_or("No active session")?;
    let previous = take_music_remote();
    let next = music::MusicRemote::new(device.id.clone(), svc.music.clone(), session, quality);
    *MUSIC_REMOTE.lock().unwrap_or_else(|p| p.into_inner()) = Some(next.clone());
    svc.music.set_remote(Some(next));
    ui.set_target(Some(&device), CastScope::Music);
    if let Some(p) = previous {
        p.dispose();
        hub().service.disconnect(p.device_id());
    }
    Ok(())
}

/// Music: the cast button of the mini player and of Now Playing.
#[component]
pub fn MusicCastButton() -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let svc = use_services();
    let app = use_app();
    let mut open = use_signal(|| false);
    let target = cast.target_of(CastScope::Music);
    let current = target.as_ref().map(|t| t.device_id.clone());
    rsx! {
        div { class: "jr-cast-anchor",
            crate::design::IconButton {
                icon: "cast",
                label: t.t("cast:button"),
                pressed: target.is_some(),
                onclick: move |_| open.toggle(),
            }
            if open() {
                CastPicker {
                    scope: CastScope::Music,
                    current_id: current,
                    on_close: move |()| open.set(false),
                    on_select: {
                        let svc = svc.clone();
                        move |(device, done): (Option<CastDevice>, Callback<Result<(), String>>)| {
                            let quality = app.preferences.peek().audio_quality;
                            done.call(switch_music_output(cast, &svc, (app.session)(), quality, device));
                        }
                    },
                }
            }
        }
    }
}
