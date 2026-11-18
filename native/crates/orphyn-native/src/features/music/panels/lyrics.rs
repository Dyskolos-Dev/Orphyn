//! `LyricsView.tsx`: synced lines with the current one large and bright, the others dimmed, smooth
//! auto-scroll, click a line to seek. Unsynced lyrics are plain text. The server's lyrics come
//! first; when it has none, LRCLIB is asked (if the preference is on).
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dioxus::html::geometry::PixelsVector2D;
use dioxus::html::ScrollBehavior;
use dioxus::prelude::*;
use orphyn_jellyfin::keys::stale;
use orphyn_jellyfin::JfError;

use super::lrclib::{fetch_lrclib, find_line_index, from_server, read_cache, Lyrics, TrackQuery};
use crate::design::{EmptyState, Spinner};
use crate::features::music::use_music;
use crate::i18n::use_i18n;
use crate::query::use_query;
use crate::state::{use_app, use_services};

/// Pause auto-scroll this long after the user scrolls by hand.
const MANUAL_SCROLL_GRACE: Duration = Duration::from_millis(3500);

/// The lyrics of the playing track `track_id`; `has_lyrics` is the track's `HasLyrics`: `false`
/// shows the empty state without asking the server.
#[component]
pub fn LyricsView(track_id: String, has_lyrics: Option<bool>) -> Element {
    let t = use_i18n();
    let music = use_music();
    let app = use_app();
    let storage = use_services().storage;
    let session = (app.session)();
    let (acc, user) = session
        .as_ref()
        .map(|s| (s.account_id.clone(), s.user_id.clone()))
        .unwrap_or_default();
    let _ = user;
    let key: Vec<String> = ["jf", &acc, "lyrics", &track_id]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let server = use_query(key, stale::LYRICS, {
        let (session, id) = (session.clone(), track_id.clone());
        move || {
            let (session, id) = (session.clone(), id.clone());
            async move {
                let Some(s) = session.filter(|_| has_lyrics != Some(false)) else {
                    return Ok(None);
                };
                match s
                    .client
                    .get::<serde_json::Value>(&format!("/Audio/{id}/Lyrics"), &[])
                    .await
                {
                    Ok(v) => Ok(from_server(&v)),
                    // 404: the track has no lyrics.
                    Err(JfError::Http(404)) => Ok(None),
                    Err(e) => Err(e),
                }
            }
        }
    });
    let server_lyrics: Option<Lyrics> = server
        .data
        .as_ref()
        .and_then(|d| d.as_ref().clone())
        .filter(|l| !l.lines.is_empty());
    let server_empty = !server.is_loading() && server_lyrics.is_none();

    // The track being played, read from the player so callers only pass its id.
    let current = (music.state)().current().map(|q| q.track.clone());
    let query: Option<TrackQuery> = current.filter(|c| c.id == track_id).map(|c| TrackQuery {
        title: c.name().to_string(),
        artist: c
            .artists
            .first()
            .cloned()
            .or_else(|| c.album_artist.clone())
            .unwrap_or_default(),
        album: c.album.clone(),
        duration: c.run_time_ticks.map(|t| t as f64 / 10_000_000.0),
    });
    let online_wanted = (app.preferences)().online_lyrics && server_empty;
    let online_key = if online_wanted { query } else { None };
    let online = use_resource(use_reactive!(|online_key| {
        let storage = storage.clone();
        async move {
            let q = online_key.clone()?;
            // Nothing is requested for a track already looked up.
            let result = match read_cache(&storage, &q) {
                Some(hit) => hit,
                None => {
                    let q2 = q.clone();
                    tokio::task::spawn_blocking(move || fetch_lrclib(&storage, &q2))
                        .await
                        .ok()
                        // Offline or LRCLIB down: behave as "no lyrics" without caching the failure.
                        .and_then(|r| r.ok())
                        .flatten()
                }
            };
            Some((q, result))
        }
    }));
    let online_state = online.value();
    let online_done = online_state()
        .flatten()
        .filter(|(q, _)| Some(q) == online_key.as_ref())
        .map(|(_, r)| r);
    let searching = server_empty && online_key.is_some() && online_done.is_none();
    let lyrics: Option<Lyrics> =
        server_lyrics.or_else(|| online_done.flatten().filter(|l| !l.lines.is_empty()));

    if server.is_loading() {
        return rsx! { div { class: "jrm-lyrics jrm-lyrics--empty", Spinner {} } };
    }
    let Some(lyrics) = lyrics else {
        return rsx! {
            div { class: "jrm-lyrics jrm-lyrics--empty", "aria-busy": searching.then_some("true"),
                if searching { Spinner {} }
                // While LRCLIB is asked, the empty state waits out of sight instead of flashing.
                div { class: if searching { "jrm-lyrics-pending" } else { "" },
                    EmptyState {
                        size: "inline",
                        icon: "mic-off",
                        title: t.t("music:lyrics.none"),
                        message: rsx! { {t.t("music:lyrics.noneHint")} },
                    }
                }
            }
        };
    };
    rsx! { LyricsBody { lyrics } }
}

#[component]
fn LyricsBody(lyrics: Lyrics) -> Element {
    let t = use_i18n();
    let music = use_music();
    let player = use_services().music;
    let synced = lyrics.synced;
    let mut scroller: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let mut line_els: Signal<HashMap<usize, Rc<MountedData>>> = use_signal(HashMap::new);
    let mut last_manual: Signal<Option<Instant>> = use_signal(|| None);

    // Only the active line index matters: the memo does not wake the lines at every position tick.
    let active = use_memo(use_reactive!(|lyrics| {
        if lyrics.synced {
            find_line_index(&lyrics.lines, (music.progress)().position * 1000.0)
        } else {
            -1
        }
    }));

    use_effect(use_reactive!(|(synced,)| {
        let active = active();
        if !synced || active < 0 {
            return;
        }
        if last_manual
            .peek()
            .is_some_and(|at| at.elapsed() < MANUAL_SCROLL_GRACE)
        {
            return;
        }
        let (Some(box_el), Some(line)) = (
            scroller.peek().clone(),
            line_els.peek().get(&(active as usize)).cloned(),
        ) else {
            return;
        };
        // Scroll only the lyrics box: the active line sits at ~35 % of its height.
        spawn(async move {
            let (Ok(b), Ok(l), Ok(off)) = (
                box_el.get_client_rect().await,
                line.get_client_rect().await,
                box_el.get_scroll_offset().await,
            ) else {
                return;
            };
            let top =
                off.y + l.origin.y + l.size.height / 2.0 - (b.origin.y + b.size.height * 0.35);
            let _ = box_el
                .scroll(
                    PixelsVector2D::new(0.0, top.max(0.0)),
                    ScrollBehavior::Smooth,
                )
                .await;
        });
    }));

    let kind = if synced { "synced" } else { "plain" };
    let active = active();
    rsx! {
        div {
            class: "jrm-lyrics jrm-lyrics--{kind}",
            "aria-label": t.t("music:lyrics.title"),
            onmounted: move |e| scroller.set(Some(e.data())),
            onwheel: move |_| last_manual.set(Some(Instant::now())),
            if synced {
                for (i, line) in lyrics.lines.iter().enumerate() {
                    {
                        let start = line.start_ms.unwrap_or(0) as f64 / 1000.0;
                        let state = if i as isize == active { "active" } else if (i as isize) < active { "past" } else { "future" };
                        let text = if line.text.trim().is_empty() { "♪".to_string() } else { line.text.clone() };
                        let p = player.clone();
                        rsx! {
                            button {
                                key: "{i}",
                                r#type: "button",
                                class: "jrm-lyric",
                                "data-state": state,
                                "aria-current": (i as isize == active).then_some("true"),
                                onmounted: move |e| { line_els.write().insert(i, e.data()); },
                                onclick: move |_| {
                                    last_manual.set(None);
                                    p.seek(start);
                                },
                                "{text}"
                            }
                        }
                    }
                }
            } else {
                for (i, line) in lyrics.lines.iter().enumerate() {
                    p { key: "{i}", class: "jrm-lyric-plain", "{line.text}" }
                }
            }
            if let Some(source) = lyrics.source {
                p { class: "jrm-lyrics-source", {t.tf("music:lyrics.source", &[("source", source.to_string())])} }
            }
        }
    }
}
