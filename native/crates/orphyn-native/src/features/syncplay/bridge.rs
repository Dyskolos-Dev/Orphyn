//! `music.ts` + `video.ts`: SyncPlay ↔ the players, through their public APIs only. What the
//! person does with a player (play, pause, seek, next, a new queue…) is noticed on its state and
//! turned into group requests; what the group commands is applied with the same API, inside a
//! "quiet" window so it is not mistaken for the person.
use std::sync::Arc;

use orphyn_jellyfin::syncplay::{queue_move, QueueMove};
use orphyn_jellyfin::BaseItemDto;
use orphyn_player::music::{MusicPlayer, PlayerState, PlayerStatus};
use orphyn_player::video::{PlaybackSnapshot, PlaybackStatus, VideoController};

use super::manager::{manager, spawn, MediaKind, MusicBridge, MusicIntents, SyncPlayer};

fn normalize(id: &str) -> String {
    id.replace('-', "").to_lowercase()
}

fn ids_of(s: &PlayerState) -> Vec<String> {
    s.model
        .queue
        .iter()
        .map(|q| normalize(&q.track.id))
        .collect()
}

/// `next` is `prev` with entries added at the end: the extra ids, else `None`.
pub fn appended(prev: &[String], next: &[String]) -> Option<Vec<String>> {
    if next.len() <= prev.len() || !prev.iter().zip(next).all(|(a, b)| a == b) {
        return None;
    }
    Some(next[prev.len()..].to_vec())
}

/// `next` is `prev` with entries inserted right after `index` ("Play Next"): those ids, else `None`.
pub fn inserted_after(prev: &[String], next: &[String], index: i64) -> Option<Vec<String>> {
    if next.len() <= prev.len() || index < 0 {
        return None;
    }
    let added = next.len() - prev.len();
    let head_len = (index as usize + 1).min(prev.len());
    let (head, tail) = prev.split_at(head_len);
    if !head.iter().zip(next).all(|(a, b)| a == b) {
        return None;
    }
    if !tail
        .iter()
        .enumerate()
        .all(|(i, id)| next.get(head_len + added + i) == Some(id))
    {
        return None;
    }
    Some(next[head_len..head_len + added].to_vec())
}

// ------------------------------------------------------------------------------- music

pub struct MusicSync {
    player: MusicPlayer,
}

impl SyncPlayer for MusicSync {
    fn item_id(&self) -> Option<String> {
        self.player
            .state()
            .current()
            .map(|q| normalize(&q.track.id))
    }
    fn position(&self) -> f64 {
        self.player.progress().position
    }
    fn is_playing(&self) -> bool {
        self.player.state().status == PlayerStatus::Playing
    }
    fn is_ready(&self) -> bool {
        let s = self.player.state();
        matches!(s.status, PlayerStatus::Playing | PlayerStatus::Paused)
            && self.player.progress().duration > 0.0
    }
    fn play(&self) {
        self.player.resume();
    }
    fn pause(&self) {
        self.player.pause();
    }
    fn seek(&self, seconds: f64) {
        self.player.seek(seconds);
    }
}

impl MusicBridge for MusicSync {
    fn player(&self) -> Arc<dyn SyncPlayer> {
        Arc::new(MusicSync {
            player: self.player.clone(),
        })
    }

    fn local_queue(&self) -> (Vec<String>, i64, bool) {
        let s = self.player.state();
        (
            ids_of(&s),
            s.model.index as i64,
            s.status == PlayerStatus::Playing,
        )
    }

    fn load(&self, tracks: Vec<BaseItemDto>, index: usize) {
        self.player.play(&tracks, Some(index), Some(false));
        self.player.pause();
    }

    fn jump(&self, index: usize) {
        self.player.jump_to(index);
        self.player.pause();
    }

    fn extend(&self, tracks: Vec<BaseItemDto>) {
        let s = self.player.state();
        let local = ids_of(&s);
        let group: Vec<String> = tracks.iter().map(|t| normalize(&t.id)).collect();
        let pick = |ids: Vec<String>| -> Vec<BaseItemDto> {
            ids.iter()
                .filter_map(|id| tracks.iter().find(|t| normalize(&t.id) == *id).cloned())
                .collect()
        };
        if let Some(tail) = appended(&local, &group) {
            self.player.add_to_queue(&pick(tail));
        } else if let Some(next) = inserted_after(&local, &group, s.model.index as i64) {
            self.player.play_next(&pick(next));
        }
    }
}

/// Turns a person's change of the music player into a group request (pure, for tests).
pub fn music_intent(
    s: &PlayerState,
    prev: &PlayerState,
    following_audio: bool,
    intents: &dyn MusicIntents,
) {
    let prev_ids = ids_of(prev);
    let ids = ids_of(s);
    if prev_ids != ids {
        if ids.is_empty() || s.model.index < 0 {
            return;
        }
        let same_index = following_audio && s.model.index == prev.model.index;
        if same_index {
            if let Some(tail) = appended(&prev_ids, &ids) {
                return intents.queue(tail, false);
            }
            if let Some(next) = inserted_after(&prev_ids, &ids, prev.model.index as i64) {
                return intents.queue(next, true);
            }
        }
        return intents.new_queue(ids, s.model.index as usize, 0.0);
    }
    if !following_audio {
        return;
    }
    if s.model.index != prev.model.index && s.model.index >= 0 {
        let from = prev.model.index.max(0) as usize;
        let to = s.model.index as usize;
        match queue_move(from, to, ids.len()) {
            QueueMove::Next => intents.next(),
            QueueMove::Previous => intents.previous(),
            QueueMove::Jump => intents.jump(ids.get(to).map_or("", String::as_str)),
            QueueMove::None => {}
        }
        return;
    }
    if s.seek_nonce != prev.seek_nonce {
        return;
    }
    if s.want_playing != prev.want_playing {
        if s.want_playing {
            // Wait for the group: it starts everyone at the same moment.
            intents.play();
        } else {
            intents.pause();
        }
    }
}

struct ManagerIntents {
    player: MusicPlayer,
}

impl MusicIntents for ManagerIntents {
    fn play(&self) {
        let m = manager();
        m.quiet_now();
        self.player.pause();
        m.request_play();
    }
    fn pause(&self) {
        manager().request_pause();
    }
    fn seek(&self, seconds: f64) {
        manager().request_seek(seconds);
    }
    fn next(&self) {
        manager().request_next();
    }
    fn previous(&self) {
        manager().request_previous();
    }
    fn jump(&self, item_id: &str) {
        manager().request_jump(item_id);
    }
    fn new_queue(&self, item_ids: Vec<String>, index: usize, _start_seconds: f64) {
        let start = self.player.progress().position;
        manager().share_queue(item_ids, index, start);
    }
    fn queue(&self, item_ids: Vec<String>, next: bool) {
        manager().queue_items(item_ids, next);
    }
}

/// Registers the music player with the manager and watches it for the person's changes.
pub fn start_music(player: MusicPlayer) {
    let m = manager();
    m.set_music_bridge(Arc::new(MusicSync {
        player: player.clone(),
    }));
    let mut state_rx = player.subscribe_state();
    spawn(async move {
        let intents = ManagerIntents {
            player: player.clone(),
        };
        let mut prev = state_rx.borrow_and_update().clone();
        while state_rx.changed().await.is_ok() {
            let s = state_rx.borrow_and_update().clone();
            let m = manager();
            if m.is_active() {
                let following = m.state().current_kind == Some(MediaKind::Audio);
                if !m.is_quiet() {
                    music_intent(&s, &prev, following, &intents);
                }
                if following {
                    m.player_changed();
                }
            }
            prev = s;
        }
    });
}

// ------------------------------------------------------------------------------- video

pub struct VideoSync {
    video: VideoController,
}

impl SyncPlayer for VideoSync {
    fn item_id(&self) -> Option<String> {
        self.video.snapshot().item_id.map(|id| normalize(&id))
    }
    fn position(&self) -> f64 {
        self.video.position()
    }
    fn is_playing(&self) -> bool {
        self.video.snapshot().status == PlaybackStatus::Playing
    }
    fn is_ready(&self) -> bool {
        matches!(
            self.video.snapshot().status,
            PlaybackStatus::Playing | PlaybackStatus::Paused
        )
    }
    fn play(&self) {
        self.video.resume();
    }
    fn pause(&self) {
        self.video.pause();
    }
    fn seek(&self, seconds: f64) {
        self.video.seek(seconds);
    }
    fn set_rate(&self, rate: f64) -> bool {
        if (self.video.snapshot().rate - rate).abs() > 1e-6 {
            self.video.set_rate(rate);
        }
        true
    }
}

/// A person's change of the video player (pure, for tests): `Some(true)` = play, `Some(false)` =
/// pause, none = nothing; a jump of the position beyond what playing explains is a seek.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoIntent {
    Play,
    Pause,
    Seek(f64),
}

pub fn video_intent(
    prev: &PlaybackSnapshot,
    now: &PlaybackSnapshot,
    elapsed_s: f64,
) -> Option<VideoIntent> {
    if prev.item_id != now.item_id {
        return None;
    }
    let was_playing = matches!(prev.status, PlaybackStatus::Playing);
    let is_playing = matches!(now.status, PlaybackStatus::Playing);
    if !was_playing && is_playing {
        return Some(VideoIntent::Play);
    }
    if was_playing && matches!(now.status, PlaybackStatus::Paused) {
        return Some(VideoIntent::Pause);
    }
    let both_active = matches!(
        prev.status,
        PlaybackStatus::Playing | PlaybackStatus::Paused
    ) && matches!(now.status, PlaybackStatus::Playing | PlaybackStatus::Paused);
    if both_active {
        let expected = prev.position_seconds
            + if was_playing {
                elapsed_s * prev.rate
            } else {
                0.0
            };
        if (now.position_seconds - expected).abs() > 1.5 {
            return Some(VideoIntent::Seek(now.position_seconds));
        }
    }
    None
}

/// Registers the video player with the manager and watches it for the person's changes.
pub fn start_video(video: VideoController) {
    let m = manager();
    m.set_video(Arc::new(VideoSync {
        video: video.clone(),
    }));
    let mut rx = video.subscribe();
    spawn(async move {
        let mut prev = rx.borrow_and_update().clone();
        let mut last = std::time::Instant::now();
        while rx.changed().await.is_ok() {
            let now = rx.borrow_and_update().clone();
            let elapsed = last.elapsed().as_secs_f64();
            last = std::time::Instant::now();
            let m = manager();
            if !m.is_active() {
                prev = now;
                continue;
            }
            let following = m.state().current_kind == Some(MediaKind::Video);
            if prev.item_id != now.item_id && now.item_id.is_some() {
                if following {
                    m.video_loaded();
                } else {
                    // An item opened here while in a group becomes the group's.
                    m.video_item_started(
                        now.item_id.as_deref().unwrap_or(""),
                        now.position_seconds,
                    );
                }
            } else if following && !m.is_quiet() {
                match video_intent(&prev, &now, elapsed) {
                    Some(VideoIntent::Play) => m.request_play(),
                    Some(VideoIntent::Pause) => m.request_pause(),
                    Some(VideoIntent::Seek(s)) => m.request_seek(s),
                    None => {}
                }
            }
            if following {
                m.player_changed();
            }
            prev = now;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn v(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn appended_and_inserted_after() {
        assert_eq!(
            appended(&v(&["a", "b"]), &v(&["a", "b", "c", "d"])),
            Some(v(&["c", "d"]))
        );
        assert_eq!(appended(&v(&["a", "b"]), &v(&["a", "x", "c"])), None);
        assert_eq!(appended(&v(&["a"]), &v(&["a"])), None);
        assert_eq!(
            inserted_after(&v(&["a", "b", "c"]), &v(&["a", "x", "y", "b", "c"]), 0),
            Some(v(&["x", "y"]))
        );
        assert_eq!(
            inserted_after(&v(&["a", "b"]), &v(&["a", "b", "x"]), 1),
            Some(v(&["x"]))
        );
        assert_eq!(
            inserted_after(&v(&["a", "b"]), &v(&["a", "x", "c"]), 0),
            None
        );
        assert_eq!(inserted_after(&v(&["a"]), &v(&["a", "b"]), -1), None);
    }

    #[derive(Default)]
    struct Rec(Mutex<Vec<String>>);
    impl MusicIntents for Rec {
        fn play(&self) {
            self.0.lock().unwrap().push("play".into());
        }
        fn pause(&self) {
            self.0.lock().unwrap().push("pause".into());
        }
        fn seek(&self, s: f64) {
            self.0.lock().unwrap().push(format!("seek {s}"));
        }
        fn next(&self) {
            self.0.lock().unwrap().push("next".into());
        }
        fn previous(&self) {
            self.0.lock().unwrap().push("previous".into());
        }
        fn jump(&self, id: &str) {
            self.0.lock().unwrap().push(format!("jump {id}"));
        }
        fn new_queue(&self, ids: Vec<String>, index: usize, _: f64) {
            self.0
                .lock()
                .unwrap()
                .push(format!("new {} {index}", ids.join(",")));
        }
        fn queue(&self, ids: Vec<String>, next: bool) {
            self.0
                .lock()
                .unwrap()
                .push(format!("queue {} {next}", ids.join(",")));
        }
    }

    fn state(ids: &[&str], index: isize, want: bool, nonce: u64) -> PlayerState {
        use orphyn_player::music::{QueueItem, QueueModel};
        let queue = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                QueueItem::new(
                    format!("u{i}"),
                    BaseItemDto {
                        id: (*id).into(),
                        ..BaseItemDto::default()
                    },
                )
            })
            .collect();
        PlayerState {
            model: QueueModel {
                queue,
                index,
                ..QueueModel::default()
            },
            want_playing: want,
            seek_nonce: nonce,
            ..PlayerState::default()
        }
    }

    fn intent(prev: &PlayerState, s: &PlayerState, following: bool) -> Vec<String> {
        let rec = Rec::default();
        music_intent(s, prev, following, &rec);
        let out = rec.0.lock().unwrap().clone();
        out
    }

    #[test]
    fn music_changes_become_group_requests() {
        let base = state(&["a", "b", "c"], 0, false, 0);
        assert_eq!(
            intent(&base, &state(&["a", "b", "c"], 1, false, 0), true),
            ["next"]
        );
        assert_eq!(
            intent(&state(&["a", "b", "c"], 1, false, 0), &base, true),
            ["previous"]
        );
        assert_eq!(
            intent(&base, &state(&["a", "b", "c"], 2, false, 0), true),
            ["jump c"]
        );
        assert_eq!(
            intent(&base, &state(&["a", "b", "c"], 0, true, 0), true),
            ["play"]
        );
        assert_eq!(
            intent(&state(&["a", "b", "c"], 0, true, 0), &base, true),
            ["pause"]
        );
        assert!(intent(&base, &state(&["a", "b", "c"], 0, false, 1), true).is_empty());
        assert!(intent(&base, &state(&["a", "b", "c"], 1, false, 0), false).is_empty());
        assert_eq!(
            intent(&base, &state(&["a", "b", "c", "d"], 0, false, 0), true),
            ["queue d false"]
        );
        assert_eq!(
            intent(&base, &state(&["a", "x", "b", "c"], 0, false, 0), true),
            ["queue x true"]
        );
        assert_eq!(
            intent(&base, &state(&["z", "y"], 0, true, 0), true),
            ["new z,y 0"]
        );
    }

    fn snap(item: &str, status: PlaybackStatus, pos: f64) -> PlaybackSnapshot {
        PlaybackSnapshot {
            item_id: Some(item.into()),
            status,
            position_seconds: pos,
            ..PlaybackSnapshot::default()
        }
    }

    #[test]
    fn video_changes_become_intents() {
        use PlaybackStatus::{Paused, Playing};
        assert_eq!(
            video_intent(&snap("a", Paused, 5.0), &snap("a", Playing, 5.0), 0.2),
            Some(VideoIntent::Play)
        );
        assert_eq!(
            video_intent(&snap("a", Playing, 5.0), &snap("a", Paused, 5.2), 0.2),
            Some(VideoIntent::Pause)
        );
        // Playing on: the position follows the clock, no seek.
        assert_eq!(
            video_intent(&snap("a", Playing, 5.0), &snap("a", Playing, 5.25), 0.25),
            None
        );
        assert_eq!(
            video_intent(&snap("a", Playing, 5.0), &snap("a", Playing, 60.0), 0.25),
            Some(VideoIntent::Seek(60.0))
        );
        assert_eq!(
            video_intent(&snap("a", Paused, 5.0), &snap("a", Paused, 30.0), 0.25),
            Some(VideoIntent::Seek(30.0))
        );
        assert_eq!(
            video_intent(&snap("a", Playing, 5.0), &snap("b", Playing, 0.0), 0.25),
            None
        );
    }
}
