//! `/video/play/:itemId` (`player/VideoPlayer.tsx` + the Jellyfin side of `controller.ts`): the
//! full-window player. libmpv draws into an `object` widget; the controls are RSX on top.
//!
//! Playback: PlaybackInfo (direct play first), a transcoded HLS stream when mpv cannot play the
//! direct one, resume position, audio / subtitle tracks (embedded, external, burned in), skip
//! intro / recap / credits from MediaSegments, the Up Next card and autoplay, progress reports to
//! the server, keyboard shortcuts of the React app, fullscreen through the window.
//!
//! Not ported here (other features of the React player): SyncPlay, cast, Picture in Picture,
//! OS media session.
mod controls;
mod logic;
mod playback;

use std::collections::HashSet;
use std::time::Duration;

use dioxus::prelude::*;
use dioxus_native::CustomWidgetAttr;
use dioxus_native::winit::event::{ElementState, WindowEvent};
use dioxus_native::winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use dioxus_native::winit::monitor::Fullscreen;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};
use orphyn_player::video::{PlayOptions, PlaybackSnapshot, PlaybackStatus, VideoController, VideoWidget};

use crate::design::{Button, EmptyState, Spinner};
use crate::i18n::use_i18n;
use crate::motion::{durations, easings, use_tween};
use crate::query::use_query;
use crate::router::use_router;
use crate::shell::use_shell;
use crate::state::{use_app, use_services, use_session};
use controls::{PlayerControls, SkipPillButton};
use logic::{
    is_auto_skippable, next_episode, pick_initial_audio, pick_initial_subtitle, segment_key,
    skip_pill, skip_target, up_next_state, SkipKind,
};
use playback::{
    playback_info, report, secs_to_ticks, ticks_to_secs, transcode_fallback, InfoOptions,
    PlayMethod, PlaybackInfo, Report, ReportKind, SubtitleTrack,
};

const HIDE_AFTER: Duration = Duration::from_millis(3000);
const SEEK_STEP: f64 = 10.0;
const REPORT_EVERY: Duration = Duration::from_secs(10);
/// Local skip preferences (`skipPreferences.ts`).
const SKIP_PREFERENCES_KEY: &str = "orphyn.video.skip";

/// The Jellyfin side of the controller: what was resolved for the playing stream.
#[derive(Clone, Copy)]
struct Ctl {
    info: Signal<Option<PlaybackInfo>>,
    audio: Signal<Option<i64>>,
    /// Selected subtitle stream; `None` = off.
    sub: Signal<Option<i64>>,
    /// PlaybackInfo is being fetched (the snapshot is not `loading` yet).
    preparing: Signal<bool>,
    /// PlaybackInfo failed.
    error: Signal<Option<String>>,
    generation: Signal<u64>,
    fell_back: Signal<bool>,
    /// The "playing" report was sent for the current session.
    reporting: Signal<bool>,
}

/// What to load.
#[derive(Clone, Copy, Default)]
struct Want {
    audio: Option<i64>,
    sub: Option<i64>,
    /// First load of the item: tracks follow the language preferences.
    initial: bool,
    /// `sub` is an image subtitle the server must burn in.
    burn: bool,
}

#[derive(Clone)]
struct Prefs {
    audio: Option<String>,
    sub: Option<String>,
    max_bitrate: Option<f64>,
}

/// Runs a request detached from the page (it must outlive it, like the "stopped" report).
fn fire(fut: impl std::future::Future<Output = ()> + Send + 'static) {
    match tokio::runtime::Handle::try_current() {
        Ok(h) => {
            h.spawn(fut);
        }
        Err(_) => log::warn!("[video] no runtime for a playback report"),
    }
}

fn report_of(info: &PlaybackInfo, snap: &PlaybackSnapshot, audio: Option<i64>, sub: Option<i64>) -> Report {
    Report {
        item_id: info.item_id.clone(),
        media_source_id: info.media_source_id.clone(),
        play_session_id: info.play_session_id.clone(),
        method: info.method,
        position_ticks: secs_to_ticks(snap.position_seconds),
        is_paused: snap.status != PlaybackStatus::Playing,
        is_muted: snap.muted,
        volume_level: (snap.volume * 100.0).round() as i64,
        audio_index: audio,
        subtitle_index: Some(sub.unwrap_or(-1)),
    }
}

impl Ctl {
    fn send(self, s: &Session, video: &VideoController, kind: ReportKind) {
        if !*self.reporting.peek() {
            return;
        }
        let Some(info) = self.info.peek().clone() else {
            return;
        };
        let r = report_of(&info, &video.snapshot(), *self.audio.peek(), *self.sub.peek());
        let c = s.client.clone();
        fire(async move { report(&c, kind, &r).await });
    }

    /// `closeReporter`: the "stopped" report of the running session, sent now.
    fn close(mut self, s: &Session, video: &VideoController) {
        self.send(s, video, ReportKind::Stopped);
        self.reporting.set(false);
    }
}

/// mpv's 1-based id of a Jellyfin stream inside the file (tracks of a kind, in stream order).
fn mpv_ordinal<T>(list: &[T], index: i64, idx: impl Fn(&T) -> (i64, bool)) -> Option<usize> {
    let mut internal: Vec<i64> = list
        .iter()
        .map(&idx)
        .filter(|(_, external)| !external)
        .map(|(i, _)| i)
        .collect();
    internal.sort();
    internal.iter().position(|i| *i == index).map(|p| p + 1)
}

fn audio_ordinal(info: &PlaybackInfo, index: i64) -> Option<usize> {
    mpv_ordinal(&info.tracks.audio, index, |a| (a.index, a.is_external))
}

fn sub_track(info: &PlaybackInfo, index: Option<i64>) -> Option<&SubtitleTrack> {
    let i = index?;
    info.tracks.subtitles.iter().find(|t| t.index == i)
}

/// Per-file mpv options selecting the tracks (direct play: the file's own streams).
fn mpv_options(info: &PlaybackInfo, audio: Option<i64>, sub: Option<i64>) -> String {
    if info.method != PlayMethod::DirectPlay {
        return "aid=auto,sid=no".into();
    }
    let aid = audio
        .and_then(|a| audio_ordinal(info, a))
        .map(|n| n.to_string())
        .unwrap_or_else(|| "auto".into());
    let sid = sub_track(info, sub)
        .filter(|t| !t.is_external)
        .and_then(|t| {
            mpv_ordinal(&info.tracks.subtitles, t.index, |s| (s.index, s.is_external))
        })
        .map(|n| n.to_string())
        .unwrap_or_else(|| "no".into());
    format!("aid={aid},sid={sid}")
}

/// A subtitle that mpv must fetch itself: external files, and text subtitles of an HLS stream.
fn needs_sub_add(info: &PlaybackInfo, sub: Option<i64>) -> Option<SubtitleTrack> {
    let t = sub_track(info, sub)?;
    let direct = info.method == PlayMethod::DirectPlay;
    (t.vtt_url.is_some() && (t.is_external || !direct)).then(|| t.clone())
}

/// Shows an external subtitle: the already added track, else `sub-add`.
async fn attach_sub(video: &VideoController, t: &SubtitleTrack) {
    let Some(url) = &t.vtt_url else {
        return;
    };
    let tracks = video.tracks().await;
    if let Some(tr) = tracks
        .iter()
        .find(|tr| tr.kind == "sub" && &tr.external_filename == url)
    {
        video.set_subtitle_track(Some(tr.id));
    } else {
        video.add_subtitle(url, &t.title, t.language.as_deref().unwrap_or(""));
    }
}

/// Waits until the loaded file plays (or failed): its tracks are known then.
async fn wait_loaded(video: &VideoController) {
    let mut rx = video.subscribe();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let st = rx.borrow_and_update().status;
        if !matches!(st, PlaybackStatus::Loading | PlaybackStatus::Idle) {
            return;
        }
        if tokio::time::timeout_at(deadline, rx.changed()).await.is_err() {
            return;
        }
    }
}

/// `load`: hands the stream to mpv, starts reporting, then shows an external subtitle.
async fn load(ctl: Ctl, s: Session, video: VideoController, info: PlaybackInfo, start: f64) {
    let mut c = ctl;
    let gen = *ctl.generation.peek();
    let (audio, sub) = (*ctl.audio.peek(), *ctl.sub.peek());
    c.info.set(Some(info.clone()));
    video.play(PlayOptions {
        item_id: info.item_id.clone(),
        url: info.url.clone(),
        start_seconds: start,
        options: mpv_options(&info, audio, sub),
    });
    c.preparing.set(false);
    if !*ctl.reporting.peek() {
        c.reporting.set(true);
        ctl.send(&s, &video, ReportKind::Start);
    }
    if let Some(t) = needs_sub_add(&info, sub) {
        wait_loaded(&video).await;
        if *ctl.generation.peek() == gen {
            attach_sub(&video, &t).await;
        }
    }
}

/// `play`: PlaybackInfo, track choice, load. A newer call wins (generation).
async fn start_stream(ctl: Ctl, s: Session, video: VideoController, item_id: String, start_ticks: i64, want: Want, prefs: Prefs) {
    let mut c = ctl;
    let gen = *ctl.generation.peek() + 1;
    c.generation.set(gen);
    ctl.close(&s, &video);
    c.fell_back.set(false);
    c.error.set(None);
    c.preparing.set(true);
    let mut opts = InfoOptions {
        start_ticks,
        audio_index: want.audio,
        subtitle_index: if want.initial {
            None
        } else if want.burn {
            want.sub
        } else {
            Some(-1)
        },
        max_bitrate: prefs.max_bitrate,
    };
    let fetched = playback_info(&s, &item_id, &opts).await;
    if *ctl.generation.peek() != gen {
        return;
    }
    let mut info = match fetched {
        Ok(i) => i,
        Err(e) => {
            c.error.set(Some(e));
            c.preparing.set(false);
            return;
        }
    };
    let (audio, sub) = if want.initial {
        (
            pick_initial_audio(&info.tracks, prefs.audio.as_deref()).map(|a| a.index),
            pick_initial_subtitle(&info.tracks, prefs.sub.as_deref()).map(|t| t.index),
        )
    } else {
        (want.audio.or(info.tracks.default_audio_index), want.sub)
    };
    // A stream the server builds carries one audio track and burns image subtitles in: ask again
    // for the chosen ones.
    if want.initial && info.method != PlayMethod::DirectPlay {
        let burn = sub_track(&info, sub).is_some_and(|t| t.burn_in_required);
        if audio != info.tracks.default_audio_index || burn {
            opts.audio_index = audio;
            opts.subtitle_index = if burn { sub } else { Some(-1) };
            let again = playback_info(&s, &item_id, &opts).await;
            if *ctl.generation.peek() != gen {
                return;
            }
            match again {
                Ok(i) => info = i,
                Err(e) => {
                    c.error.set(Some(e));
                    c.preparing.set(false);
                    return;
                }
            }
        }
    }
    c.audio.set(audio);
    c.sub.set(sub);
    load(ctl, s, video, info, ticks_to_secs(start_ticks)).await;
}

fn landscape_image(base: &str, item: &BaseItemDto, max_width: u32) -> Option<String> {
    let opts = ImageOptions {
        max_width: Some(max_width),
        max_height: None,
        quality: Some(80),
        tag: None,
    };
    let order: &[ImageKind] = if item.item_type.as_deref() == Some("Episode") {
        &[ImageKind::Primary, ImageKind::Thumb]
    } else {
        &[ImageKind::Thumb, ImageKind::Backdrop, ImageKind::Primary]
    };
    order
        .iter()
        .find_map(|k| build_image_url(base, item, *k, &opts))
}

fn read_skip_auto(svc: &crate::state::Services) -> bool {
    svc.storage
        .get(SKIP_PREFERENCES_KEY)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v["skipIntroAuto"].as_bool())
        .unwrap_or(false)
}

/// `start` comes from `?start=ticks`; without it the item's saved position is used.
#[component]
pub fn VideoPlayerPage(item_id: String, start: Option<i64>) -> Element {
    if use_session().is_none() {
        return rsx! {};
    }
    rsx! { Player { item_id, start } }
}

#[component]
fn Player(item_id: String, start: Option<i64>) -> Element {
    // Mounted by `VideoPlayerPage` only with a session.
    let Some(session) = use_session() else {
        return rsx! {};
    };
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let router = use_router();
    let mut shell = use_shell();
    let window = dioxus_native::use_window();
    let video = svc.video.clone();
    // ShellImmersive: the player takes the whole window.
    use_hook(move || shell.immersive += 1);

    let ctl = Ctl {
        info: use_signal(|| None),
        audio: use_signal(|| None),
        sub: use_signal(|| None),
        preparing: use_signal(|| true),
        error: use_signal(|| None),
        generation: use_signal(|| 0),
        fell_back: use_signal(|| false),
        reporting: use_signal(|| false),
    };

    // The controller's snapshot (`useSyncExternalStore`).
    let mut snap = use_signal(|| video.snapshot());
    use_hook({
        let video = video.clone();
        move || {
            let mut rx = video.subscribe();
            spawn(async move {
                while rx.changed().await.is_ok() {
                    let v = rx.borrow_and_update().clone();
                    snap.set(v);
                }
            });
        }
    });
    let widget = use_hook({
        let video = video.clone();
        move || video.mpv().ok().map(|m| CustomWidgetAttr::new(VideoWidget::new(m)))
    });

    // --- the item and its series (next episode)
    let acc = session.account_id.clone();
    let item = use_query(keys::item(&acc, &item_id), stale::ITEM, {
        let (s, id) = (session.clone(), item_id.clone());
        move || {
            let (s, id) = (s.clone(), id.clone());
            async move { s.item(&id).await }
        }
    });
    let data: Option<BaseItemDto> = item.data.as_deref().cloned();
    let series_id = data
        .as_ref()
        .filter(|d| d.item_type.as_deref() == Some("Episode"))
        .and_then(|d| d.series_id.clone());
    let episodes_params = ItemsParams {
        parent_id: series_id.clone(),
        include_item_types: vec!["Episode".into()],
        sort_by: vec!["ParentIndexNumber".into(), "IndexNumber".into(), "SortName".into()],
        page_size: Some(1000),
        fields: Some(vec!["SortName".into()]),
        ..Default::default()
    };
    let episodes = use_query(keys::items(&acc, &episodes_params), stale::ITEM, {
        let (s, p) = (session.clone(), episodes_params.clone());
        move || {
            let (s, p) = (s.clone(), p.clone());
            async move {
                if p.parent_id.is_none() {
                    return Ok(Vec::new());
                }
                s.items_page(&p, 0).await.map(|page| page.items)
            }
        }
    });
    let next: Option<BaseItemDto> = episodes
        .data
        .as_deref()
        .and_then(|list| next_episode(list, &item_id));

    let preferences = app.preferences.read().clone();
    let prefs = Prefs {
        audio: preferences.preferred_audio_language.clone(),
        sub: preferences.preferred_subtitle_language.clone(),
        max_bitrate: preferences.max_video_bitrate,
    };
    let auto_play_next = preferences.auto_play_next;

    // --- lifecycle: start once the item is known (resume position), stop + report on leave
    let mut started = use_signal(|| false);
    if !*started.peek() && (start.is_some() || data.is_some() || item.error.is_some()) {
        started.set(true);
        let ticks = start.unwrap_or_else(|| {
            data.as_ref()
                .and_then(|d| d.user_data.as_ref())
                .and_then(|u| u.playback_position_ticks)
                .unwrap_or(0)
        });
        let (s, v, id, p) = (session.clone(), video.clone(), item_id.clone(), prefs.clone());
        spawn(async move {
            start_play(ctl, s, v, id, ticks, p).await;
        });
    }
    use_drop({
        let (s, v, id, cache, acc) = (
            session.clone(),
            video.clone(),
            item_id.clone(),
            svc.cache.clone(),
            acc.clone(),
        );
        let window = window.clone();
        move || {
            shell.immersive -= 1;
            let mut gen = ctl.generation;
            gen += 1;
            ctl.close(&s, &v);
            // The next page may already play another item on the shared controller.
            if v.snapshot().item_id.as_deref() == Some(id.as_str()) {
                v.stop();
            }
            if window.fullscreen().is_some() {
                window.set_fullscreen(None);
            }
            // Progress changed: item pages refetch (React invalidates `jfKeys.all`).
            cache.invalidate(&keys::all(&acc));
        }
    });

    // --- reporting: progress every 10 s, pause / unpause right away
    use_future({
        let (s, v) = (session.clone(), video.clone());
        move || {
            let (s, v) = (s.clone(), v.clone());
            async move {
                loop {
                    tokio::time::sleep(REPORT_EVERY).await;
                    ctl.send(&s, &v, ReportKind::Progress("timeupdate"));
                }
            }
        }
    });
    let status = snap().status;
    let mut last_status = use_signal(|| status);
    if *last_status.peek() != status {
        let prev = *last_status.peek();
        last_status.set(status);
        match (prev, status) {
            (PlaybackStatus::Playing, PlaybackStatus::Paused) => {
                ctl.send(&session, &video, ReportKind::Progress("pause"))
            }
            (PlaybackStatus::Paused, PlaybackStatus::Playing) => {
                ctl.send(&session, &video, ReportKind::Progress("unpause"))
            }
            _ => {}
        }
    }

    // --- direct play failed: the transcoded stream at the same position, once
    if status == PlaybackStatus::Error && !*ctl.fell_back.peek() {
        let info = ctl.info.peek().clone();
        if let Some(info) = info.filter(|i| i.method != PlayMethod::Transcode) {
            let mut fb = ctl.fell_back;
            fb.set(true);
            let fallback = transcode_fallback(&session, &info, prefs.max_bitrate);
            let pos = snap.peek().position_seconds;
            let (s, v) = (session.clone(), video.clone());
            spawn(async move { load(ctl, s, v, fallback, pos).await });
        }
    }

    // --- overlay visibility: hide 3 s after the last interaction while playing
    let mut controls_visible = use_signal(|| true);
    let mut held = use_signal(|| false);
    let mut hide_gen = use_signal(|| 0u64);
    let mut show_controls = move || {
        if !*controls_visible.peek() {
            controls_visible.set(true);
        }
        let g = *hide_gen.peek() + 1;
        hide_gen.set(g);
        spawn(async move {
            tokio::time::sleep(HIDE_AFTER).await;
            if *hide_gen.peek() == g {
                controls_visible.set(false);
            }
        });
    };
    let keep_visible = status != PlaybackStatus::Playing || held();
    use_effect(use_reactive!(|keep_visible| {
        if keep_visible {
            let g = *hide_gen.peek() + 1;
            hide_gen.set(g);
            controls_visible.set(true);
        } else {
            show_controls();
        }
    }));
    let opacity = use_tween(
        if controls_visible() { 1.0 } else { 0.0 },
        durations::BASE,
        easings::STANDARD,
    );

    // --- actions
    let is_fullscreen = (app.window)().fullscreen;
    let toggle_play = {
        let v = video.clone();
        move || {
            let st = v.snapshot().status;
            if st == PlaybackStatus::Ended {
                v.seek(0.0);
                v.resume();
            } else {
                v.toggle();
            }
        }
    };
    let seek_to = {
        let (v, s) = (video.clone(), session.clone());
        move |secs: f64| {
            let d = v.snapshot().duration_seconds;
            let target = if d > 0.0 { secs.clamp(0.0, d) } else { secs.max(0.0) };
            v.seek(target);
            // `seeked`: the server learns the new position.
            let (v, s) = (v.clone(), s.clone());
            spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                ctl.send(&s, &v, ReportKind::Progress("timeupdate"));
            });
        }
    };
    let seek_by = {
        let (v, seek_to) = (video.clone(), seek_to.clone());
        move |delta: f64| seek_to(v.snapshot().position_seconds + delta)
    };
    let set_volume = {
        let v = video.clone();
        move |vol: f64| {
            let vol = vol.clamp(0.0, 1.0);
            v.set_volume(vol);
            v.set_muted(vol == 0.0);
        }
    };
    let toggle_mute = {
        let v = video.clone();
        move || v.set_muted(!v.snapshot().muted)
    };
    let toggle_fullscreen = {
        let w = window.clone();
        move || {
            if w.fullscreen().is_some() {
                w.set_fullscreen(None);
            } else {
                w.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    };
    let leave = {
        let id = item_id.clone();
        move || {
            if router.history.peek().is_first() {
                router.replace(&format!("/video/item/{id}"));
            } else {
                router.back();
            }
        }
    };
    let play_next = {
        let next_id = next.as_ref().map(|n| n.id.clone());
        move || {
            if let Some(id) = &next_id {
                router.replace(&format!("/video/play/{id}"));
            }
        }
    };
    let select_audio = {
        let (s, v, id, p) = (session.clone(), video.clone(), item_id.clone(), prefs.clone());
        move |index: i64| {
            let Some(info) = ctl.info.peek().clone() else {
                return;
            };
            let mut a = ctl.audio;
            if info.method == PlayMethod::DirectPlay {
                if let Some(n) = audio_ordinal(&info, index) {
                    v.set_audio_track(Some(n as i64));
                    a.set(Some(index));
                    ctl.send(&s, &v, ReportKind::Progress("audiotrackchange"));
                }
                return;
            }
            // `switchTracks`: a new stream at the current position.
            let sub = *ctl.sub.peek();
            let want = Want {
                audio: Some(index),
                sub,
                initial: false,
                burn: sub_track(&info, sub).is_some_and(|t| t.burn_in_required),
            };
            let ticks = secs_to_ticks(v.snapshot().position_seconds);
            let (s, v, id, p) = (s.clone(), v.clone(), id.clone(), p.clone());
            spawn(async move { start_stream(ctl, s, v, id, ticks, want, p).await });
        }
    };
    let select_subtitle = {
        let (s, v, id, p) = (session.clone(), video.clone(), item_id.clone(), prefs.clone());
        move |index: Option<i64>| {
            let Some(info) = ctl.info.peek().clone() else {
                return;
            };
            let mut sub = ctl.sub;
            let from = *sub.peek();
            if from == index {
                return;
            }
            let burn = |i: Option<i64>| sub_track(&info, i).is_some_and(|t| t.burn_in_required);
            let direct = info.method == PlayMethod::DirectPlay;
            if !direct && (burn(from) || burn(index)) {
                // `selectSubtitle`: image subtitles are burned in by the server.
                let want = Want {
                    audio: *ctl.audio.peek(),
                    sub: index,
                    initial: false,
                    burn: burn(index),
                };
                let ticks = secs_to_ticks(v.snapshot().position_seconds);
                let (s, v, id, p) = (s.clone(), v.clone(), id.clone(), p.clone());
                spawn(async move { start_stream(ctl, s, v, id, ticks, want, p).await });
                return;
            }
            sub.set(index);
            ctl.send(&s, &v, ReportKind::Progress("subtitletrackchange"));
            match sub_track(&info, index) {
                None => v.set_subtitle_track(None),
                Some(t) if needs_sub_add(&info, index).is_some() => {
                    let (t, v) = (t.clone(), v.clone());
                    spawn(async move { attach_sub(&v, &t).await });
                }
                Some(t) => {
                    let n = mpv_ordinal(&info.tracks.subtitles, t.index, |s| (s.index, s.is_external));
                    v.set_subtitle_track(n.map(|n| n as i64));
                }
            }
        }
    };
    let retry = {
        let (s, v, id, p) = (session.clone(), video.clone(), item_id.clone(), prefs.clone());
        move || {
            let sub = *ctl.sub.peek();
            let info = ctl.info.peek().clone();
            let want = Want {
                audio: *ctl.audio.peek(),
                sub,
                initial: info.is_none(),
                burn: info
                    .as_ref()
                    .is_some_and(|i| sub_track(i, sub).is_some_and(|t| t.burn_in_required)),
            };
            let ticks = secs_to_ticks(v.snapshot().position_seconds);
            let (s, v, id, p) = (s.clone(), v.clone(), id.clone(), p.clone());
            spawn(async move { start_stream(ctl, s, v, id, ticks, want, p).await });
        }
    };

    // --- skip intro / credits, up next
    let snapshot = snap();
    let mut dismissed = use_signal(|| false);
    let segments = ctl.info.read().as_ref().map(|i| i.segments.clone()).unwrap_or_default();
    let (up_visible, seconds_left) = up_next_state(
        snapshot.position_seconds,
        snapshot.duration_seconds,
        next.is_some(),
        dismissed(),
    );
    let ended = status == PlaybackStatus::Ended;
    let show_up_next = up_visible || (ended && next.is_some() && !auto_play_next);
    let pill = skip_pill(
        &segments,
        snapshot.position_seconds,
        next.is_some(),
        show_up_next,
        false,
    );
    let do_skip = {
        let (pill, play_next, seek_to) = (pill.clone(), play_next.clone(), seek_to.clone());
        let d = snapshot.duration_seconds;
        move || match &pill {
            Some(p) if p.kind == SkipKind::NextEpisode => play_next(),
            Some(p) => seek_to(skip_target(&p.segment, d)),
            None => {}
        }
    };
    let svc_skip = svc.clone();
    let mut skip_auto = use_signal(move || read_skip_auto(&svc_skip));
    let mut auto_skipped = use_signal(HashSet::<String>::new);
    if let Some(p) = pill.as_ref().filter(|p| is_auto_skippable(p)) {
        if skip_auto() && status == PlaybackStatus::Playing {
            let key = segment_key(&item_id, &p.segment);
            if !auto_skipped.peek().contains(&key) {
                auto_skipped.write().insert(key);
                video.seek(skip_target(&p.segment, snapshot.duration_seconds));
            }
        }
    }
    let has_skippable = segments.iter().any(|s| {
        matches!(
            s.kind,
            playback::SegmentType::Intro | playback::SegmentType::Recap
        )
    });

    // --- end of the item: autoplay the next episode, or leave
    let mut handled_end = use_signal(|| false);
    if ended && !*handled_end.peek() {
        handled_end.set(true);
        if next.is_some() && auto_play_next && !dismissed() {
            play_next();
        } else if next.is_none() {
            leave();
        }
    } else if !ended && *handled_end.peek() {
        handled_end.set(false);
    }

    // --- keyboard: space / K, arrows, F, M, S / Enter (skip), Esc
    let mut modifiers = use_signal(ModifiersState::default);
    let mut popover_open = held;
    let key_actions = {
        let (toggle_play, seek_by, set_volume) = (toggle_play.clone(), seek_by.clone(), set_volume.clone());
        let (toggle_fullscreen, toggle_mute, leave) = (toggle_fullscreen.clone(), toggle_mute.clone(), leave.clone());
        let (do_skip, has_pill, v, w) = (do_skip.clone(), pill.is_some(), video.clone(), window.clone());
        move |key: &WKey| -> bool {
            let ch = |c: &str| matches!(key, WKey::Character(s) if s.as_str() == c);
            if ch(" ") || ch("k") {
                toggle_play();
            } else if matches!(key, WKey::Named(NamedKey::ArrowLeft)) {
                seek_by(-SEEK_STEP);
            } else if matches!(key, WKey::Named(NamedKey::ArrowRight)) {
                seek_by(SEEK_STEP);
            } else if matches!(key, WKey::Named(NamedKey::ArrowUp)) {
                set_volume(v.snapshot().volume + 0.1);
            } else if matches!(key, WKey::Named(NamedKey::ArrowDown)) {
                set_volume(v.snapshot().volume - 0.1);
            } else if ch("f") || ch("F") {
                toggle_fullscreen();
            } else if ch("m") || ch("M") {
                toggle_mute();
            } else if (ch("s") || ch("S") || matches!(key, WKey::Named(NamedKey::Enter))) && has_pill {
                do_skip();
            } else if matches!(key, WKey::Named(NamedKey::Escape)) {
                if w.fullscreen().is_some() {
                    w.set_fullscreen(None);
                } else {
                    leave();
                }
            } else {
                return false;
            }
            true
        }
    };
    let current_keys = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<std::rc::Rc<dyn Fn(&WKey) -> bool>>)));
    *current_keys.borrow_mut() = Some(std::rc::Rc::new(key_actions));
    let mut show_for_keys = show_controls;
    dioxus_native::use_window_event({
        let current_keys = current_keys.clone();
        move |event, _| match event {
            WindowEvent::ModifiersChanged(m) => modifiers.set(m.state()),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let m = *modifiers.peek();
                if m.control_key() || m.alt_key() || m.meta_key() {
                    return;
                }
                // An open popover takes Escape for itself.
                if *popover_open.peek() && matches!(event.logical_key, WKey::Named(NamedKey::Escape)) {
                    popover_open.set(false);
                    return;
                }
                let f = current_keys.borrow().clone();
                if let Some(f) = f {
                    if f(&event.logical_key) {
                        show_for_keys();
                    }
                }
            }
            _ => {}
        }
    });

    // --- view
    let base = session.client.base_url().to_string();
    let title = data
        .as_ref()
        .and_then(|d| d.series_name.clone().or_else(|| d.name.clone()))
        .unwrap_or_default();
    let subtitle = data
        .as_ref()
        .filter(|d| d.item_type.as_deref() == Some("Episode"))
        .map(|d| {
            let mut parts: Vec<String> = Vec::new();
            if let (Some(s), Some(e)) = (d.parent_index_number, d.index_number) {
                parts.push(t.tf(
                    "video:item.episodeLabel",
                    &[("season", s.to_string()), ("episode", e.to_string())],
                ));
            }
            if let Some(n) = &d.name {
                parts.push(n.clone());
            }
            parts.join(" · ")
        });
    let load_error = (ctl.error)();
    let failed = status == PlaybackStatus::Error && (*ctl.fell_back.read() || ctl.info.read().as_ref().is_some_and(|i| i.method == PlayMethod::Transcode))
        || load_error.is_some()
        || widget.is_none();
    let error_message = load_error
        .or_else(|| snapshot.error.clone())
        .or_else(|| video.mpv().err());
    let loading = (ctl.preparing)() || status == PlaybackStatus::Loading;
    let tracks = ctl.info.read().as_ref().map(|i| i.tracks.clone());
    let controls_attr = if controls_visible() { "visible" } else { "hidden" };
    let up_next_img = next.as_ref().and_then(|n| landscape_image(&base, n, 320));
    let next_name = next.as_ref().and_then(|n| n.name.clone()).unwrap_or_default();

    let (tp, tp2) = (toggle_play.clone(), toggle_fullscreen.clone());
    let leave2 = leave.clone();
    let leave3 = leave.clone();
    let retry2 = retry.clone();
    let pn1 = play_next.clone();
    let svc2 = svc.clone();
    let skip_key = pill.as_ref().map(|p| format!("{:?}", p.kind)).unwrap_or_default();

    rsx! {
        div {
            class: "jr-video-player",
            "data-controls": controls_attr,
            onmousemove: move |_| show_controls(),
            if let Some(w) = widget.clone() {
                object {
                    class: "jr-video-player__video",
                    data: w,
                    onclick: move |_| tp(),
                    ondoubleclick: move |_| tp2(),
                }
            }
            if loading && !failed {
                div { class: "jr-video-player__spinner", role: "status", "aria-label": t.t("video:player.loading"),
                    Spinner { size: "large" }
                }
            }
            if failed {
                div { class: "jr-video-player__error", "data-theme": "dark", role: "alert",
                    EmptyState {
                        icon: "clapperboard",
                        title: t.t("video:player.errorTitle"),
                        message: rsx! { {error_message.unwrap_or_else(|| t.t("video:player.errorMessage"))} },
                        action: rsx! {
                            Button { variant: "contrast", onclick: move |_| retry2(), {t.t("video:player.retry")} }
                            Button { variant: "gray", onclick: move |_| leave2(), {t.t("video:player.back")} }
                        },
                    }
                }
            } else {
                PlayerControls {
                            opacity: opacity(),
                            snapshot: snapshot.clone(),
                            title: title.clone(),
                            subtitle: subtitle.clone(),
                            is_fullscreen,
                            tracks,
                            audio_index: (ctl.audio)(),
                            subtitle_index: (ctl.sub)(),
                            trailer: false,
                            skip_intro_auto: if has_skippable { Some(skip_auto()) } else { None },
                            on_back: move |_| leave3(),
                            on_toggle_play: move |_| toggle_play(),
                            on_seek_by: seek_by,
                            on_seek: seek_to,
                            on_volume: set_volume,
                            on_toggle_mute: move |_| toggle_mute(),
                            on_rate: {
                                let v = video.clone();
                                move |r| v.set_rate(r)
                            },
                            on_toggle_fullscreen: move |_| toggle_fullscreen(),
                            on_select_audio: select_audio,
                            on_select_subtitle: select_subtitle,
                            on_toggle_skip_intro_auto: move |_| {
                                let on = !skip_auto();
                                skip_auto.set(on);
                                svc2.storage.set(SKIP_PREFERENCES_KEY, &serde_json::json!({ "skipIntroAuto": on }).to_string());
                            },
                            on_hold_change: move |h| held.set(h),
                        }
            }
            if let Some(p) = &pill {
                SkipPillButton { key: "{skip_key}", kind: p.kind, on_skip: move |_| do_skip() }
            }
            if show_up_next && next.is_some() {
                div { class: "jr-video-upnext", "data-theme": "dark",
                    div { class: "jr-video-upnext__thumb",
                        if let Some(src) = &up_next_img {
                            img { src: "{src}", alt: "", draggable: "false" }
                        }
                    }
                    div { class: "jr-video-upnext__body",
                        span { class: "jr-text-caption jr-color-secondary", {t.t("video:player.upNext")} }
                        p { class: "jr-text-headline jr-color-primary jr-truncate", "{next_name}" }
                        if auto_play_next && !ended {
                            p { class: "jr-text-footnote jr-color-secondary jr-tabular",
                                {t.tf("video:player.startsIn", &[("seconds", seconds_left.to_string())])}
                            }
                        }
                        div { class: "jr-video-upnext__actions",
                            Button { variant: "contrast", size: "small", onclick: move |_| pn1(), {t.t("video:player.playNow")} }
                            Button { variant: "gray", size: "small", onclick: move |_| dismissed.set(true), {t.t("video:player.cancel")} }
                        }
                    }
                }
            }
        }
    }

}

/// First load of the item (languages from the preferences).
async fn start_play(ctl: Ctl, s: Session, v: VideoController, id: String, ticks: i64, p: Prefs) {
    let want = Want {
        initial: true,
        ..Want::default()
    };
    start_stream(ctl, s, v, id, ticks, want, p).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback::{AudioTrack, PlaybackTracks};

    fn info(method: PlayMethod) -> PlaybackInfo {
        let a = |index, ext| AudioTrack {
            index,
            language: None,
            codec: None,
            title: String::new(),
            is_default: false,
            is_external: ext,
        };
        let s = |index, ext, vtt: bool| SubtitleTrack {
            index,
            language: None,
            codec: None,
            title: String::new(),
            is_default: false,
            is_forced: false,
            is_external: ext,
            vtt_url: vtt.then(|| format!("https://x/{index}.vtt")),
            burn_in_required: !vtt,
        };
        PlaybackInfo {
            item_id: "i".into(),
            url: "u".into(),
            is_hls: method != PlayMethod::DirectPlay,
            method,
            play_session_id: String::new(),
            media_source_id: "m".into(),
            run_time_ticks: None,
            tracks: PlaybackTracks {
                audio: vec![a(1, false), a(2, false)],
                subtitles: vec![s(3, false, false), s(4, false, true), s(9, true, true)],
                default_audio_index: Some(1),
                default_subtitle_index: None,
            },
            segments: Vec::new(),
        }
    }

    #[test]
    fn direct_play_selects_the_file_streams() {
        let i = info(PlayMethod::DirectPlay);
        assert_eq!(mpv_options(&i, Some(2), Some(4)), "aid=2,sid=2");
        assert_eq!(mpv_options(&i, Some(1), None), "aid=1,sid=no");
        // An external subtitle is added after the load.
        assert_eq!(mpv_options(&i, None, Some(9)), "aid=auto,sid=no");
        assert_eq!(needs_sub_add(&i, Some(9)).map(|t| t.index), Some(9));
        assert!(needs_sub_add(&i, Some(4)).is_none());
    }

    #[test]
    fn hls_streams_take_text_subtitles_from_the_server() {
        let i = info(PlayMethod::Transcode);
        assert_eq!(mpv_options(&i, Some(2), Some(4)), "aid=auto,sid=no");
        assert_eq!(needs_sub_add(&i, Some(4)).map(|t| t.index), Some(4));
        // Image subtitles are burned in: nothing to add.
        assert!(needs_sub_add(&i, Some(3)).is_none());
    }
}
