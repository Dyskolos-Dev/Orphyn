//! `CastButtons.tsx` (video part) + `videoCast.ts`: the cast button of the player controls and
//! the "Playing on <device>" screen. While casting, local playback is paused; stopping (or the
//! device ending / failing) returns to this computer at the position the device reached.
use dioxus::prelude::*;
use orphyn_core::cast::{CastDevice, CastMedia, CastMediaKind, CastStatus};
use orphyn_jellyfin::client::Q;
use orphyn_jellyfin::endpoints::Session;

use super::devices::CastScope;
use super::hub::hub;
use super::picker::CastPicker;
use super::use_cast;
use crate::design::{format_duration, Button, IconButton, Slider};
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services, Services};

/// What the player tells the cast feature about the item it shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VideoCastRequest {
    pub item_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<String>,
}

/// H.264/AAC HLS stream the receiver can read (original files are rarely playable on a TV).
pub fn video_cast_media(
    session: &Session,
    r: &VideoCastRequest,
    start_sec: f64,
    duration_sec: f64,
) -> CastMedia {
    let url = session.client.authed_url(
        &format!("/Videos/{}/master.m3u8", r.item_id),
        &[
            ("mediaSourceId", Some(Q::from(r.item_id.as_str()))),
            ("deviceId", Some(Q::from(session.client.device_id()))),
            ("videoCodec", Some("h264".into())),
            ("audioCodec", Some("aac".into())),
            ("segmentContainer", Some("ts".into())),
            ("maxAudioChannels", Some(2u32.into())),
            ("minSegments", Some(1u32.into())),
            ("breakOnNonKeyFrames", Some(true.into())),
        ],
    );
    CastMedia {
        url,
        mime_type: "application/x-mpegURL".into(),
        kind: CastMediaKind::Video,
        title: r.title.clone(),
        artist: r.subtitle.clone(),
        album: None,
        artwork_url: r.artwork_url.clone(),
        duration_sec: Some(duration_sec).filter(|d| *d > 0.0),
        start_position_sec: Some(start_sec),
    }
}

/// Stops casting and returns local playback to where the device was (`ended`: its end).
pub(super) fn stop_video_cast(mut ui: super::CastUi, svc: &Services, ended: bool) {
    let Some(target) = ui.target_of(CastScope::Video) else {
        return;
    };
    let session = ui.session_of(&target.device_id);
    ui.target.set(None);
    hub().service.disconnect(&target.device_id);
    if let Some(s) = session {
        svc.video.seek(if ended {
            s.duration_sec
        } else {
            s.position_sec
        });
        svc.video.resume();
    }
}

/// Context of the player: the item being played, for the button and the screen.
#[derive(Clone, Copy, PartialEq)]
struct VideoCastInfo(Signal<VideoCastRequest>);

/// Mounted by the player: provides the item to the cast button and shows the cast screen.
#[component]
pub fn VideoCastHost(request: VideoCastRequest) -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let svc = use_services();
    let mut info = use_context_provider(|| VideoCastInfo(Signal::new(request.clone())));
    if *info.0.peek() != request {
        info.0.set(request.clone());
    }
    // Leaving the player while casting stops the cast (without touching local playback).
    use_drop(move || {
        if let Some(target) = cast.target_of(CastScope::Video) {
            let mut target_sig = cast.target;
            target_sig.set(None);
            hub().service.disconnect(&target.device_id);
        }
    });
    let Some(target) = cast.target_of(CastScope::Video) else {
        return rsx! {};
    };
    let mut scrub = use_signal(|| None::<f64>);
    let session = cast.session_of(&target.device_id);
    let playing = session
        .as_ref()
        .is_some_and(|s| matches!(s.status, CastStatus::Playing | CastStatus::Buffering));
    let duration = session.as_ref().map_or(0.0, |s| s.duration_sec);
    let position = scrub().unwrap_or_else(|| session.as_ref().map_or(0.0, |s| s.position_sec));
    let play_icon = if playing { "pause" } else { "play" };
    let play_label = t.t(if playing { "cast:pause" } else { "cast:play" });
    let (seek_id, play_id) = (target.device_id.clone(), target.device_id.clone());
    let svc2 = svc.clone();
    rsx! {
        div { class: "jr-cast-screen", "data-theme": "dark", role: "region",
            "aria-label": t.tf("cast:playingOn", &[("device", target.name.clone())]),
            if let Some(src) = &request.artwork_url {
                img { class: "jr-cast-screen__art", src: "{src}", alt: "", draggable: "false" }
            }
            h2 { class: "jr-text-title-2", {t.tf("cast:playingOn", &[("device", target.name.clone())])} }
            span { class: "jr-text-headline", "{request.title}" }
            if let Some(s) = &request.subtitle {
                span { class: "jr-text-footnote jr-color-secondary", "{s}" }
            }
            div { class: "jr-cast-screen__scrubber",
                span { class: "jr-text-footnote jr-tabular", {format_duration(position)} }
                Slider {
                    label: t.t("cast:seek"),
                    on_media: true,
                    value: position,
                    max: if duration > 0.0 { duration } else { 1.0 },
                    disabled: duration <= 0.0,
                    on_change_start: move |_| scrub.set(Some(position)),
                    on_change: move |v: f64| scrub.set(Some(v)),
                    on_change_end: move |v: f64| {
                        scrub.set(None);
                        hub().service.seek(&seek_id, v);
                    },
                }
                span { class: "jr-text-footnote jr-tabular", {format!("-{}", format_duration((duration - position).max(0.0)))} }
            }
            div { class: "jr-cast-screen__controls",
                IconButton {
                    variant: "glass",
                    size: "large",
                    icon: play_icon,
                    fill: true,
                    label: play_label,
                    onclick: move |_| {
                        if playing { hub().service.pause(&play_id) } else { hub().service.play(&play_id) }
                    },
                }
                Button {
                    variant: "gray",
                    icon: "x",
                    onclick: move |_| stop_video_cast(cast, &svc2, false),
                    {t.t("cast:stop")}
                }
            }
        }
    }
}

/// The cast button of the player controls (video-capable devices only).
#[component]
pub fn VideoCastButton(on_hold_change: Option<EventHandler<bool>>) -> Element {
    let t = use_i18n();
    let cast = use_cast();
    let svc = use_services();
    let app = use_app();
    let Some(info) = try_use_context::<VideoCastInfo>() else {
        return rsx! {};
    };
    let mut open = use_signal(|| false);
    let mut set_open = move |v: bool| {
        open.set(v);
        if let Some(h) = &on_hold_change {
            h.call(v);
        }
    };
    let target = cast.target_of(CastScope::Video);
    let current = target.as_ref().map(|t| t.device_id.clone());
    rsx! {
        div { class: "jr-cast-anchor",
            IconButton {
                variant: "glass",
                icon: "cast",
                label: t.t("cast:button"),
                pressed: target.is_some(),
                onclick: move |_| set_open(!open()),
            }
            if open() {
                CastPicker {
                    scope: CastScope::Video,
                    current_id: current,
                    on_close: move |()| set_open(false),
                    on_select: move |(device, done): (Option<CastDevice>, Callback<Result<(), String>>)| {
                        let Some(device) = device else {
                            stop_video_cast(cast, &svc, false);
                            done.call(Ok(()));
                            return;
                        };
                        let Some(session) = (app.session)() else {
                            done.call(Err("No active session".into()));
                            return;
                        };
                        if !device.capabilities.video {
                            done.call(Err("This device cannot play video".into()));
                            return;
                        }
                        let snap = svc.video.snapshot();
                        let request = info.0.peek().clone();
                        let media = video_cast_media(&session, &request, snap.position_seconds, snap.duration_seconds);
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        hub().service.load(&device.id, media, move |r| {
                            let _ = tx.send(r);
                        });
                        let svc = svc.clone();
                        spawn(async move {
                            match rx.await.unwrap_or_else(|_| Err("Cast stopped".into())) {
                                Ok(()) => {
                                    svc.video.pause();
                                    cast.set_target(Some(&device), CastScope::Video);
                                    set_open(false);
                                    done.call(Ok(()));
                                }
                                Err(e) => {
                                    hub().service.disconnect(&device.id);
                                    done.call(Err(e));
                                }
                            }
                        });
                    },
                }
            }
        }
    }
}
