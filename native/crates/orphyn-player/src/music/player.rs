//! The music player: the state of `store.ts` and the API of `musicPlayer` (`api.ts`), driving the
//! audio thread of `output.rs`. Cheap to clone; every clone is the same player.
//!
//! The UI listens with [`MusicPlayer::subscribe_state`] (queue, status, volume: changes rarely)
//! and [`MusicPlayer::subscribe_progress`] (position / duration, ~4 times a second while playing).
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use tokio::sync::watch;

use super::crossfade::{plan_fade, CrossfadeSettings, FadePlanInput};
use super::output::{self, Event, Msg};
use super::queue::{self, Advance, PreviousAction};
use super::types::{PlayerState, PlayerStatus, Progress, QueueModel, RepeatMode, Track};

/// Turns a track into the URL to play (direct stream or transcode), given by the app.
pub type Resolver = Arc<dyn Fn(&Track) -> Option<String> + Send + Sync>;

/// A receiver the music engine plays on instead of the local audio thread (cast). The engine keeps
/// owning the queue, the state and the reporting; the output only turns its calls into commands
/// and reports back through [`MusicPlayer::remote_event`].
pub trait RemoteOutput: Send + Sync {
    /// Replaces what plays with `track`, starting at `start_at` seconds.
    fn load(&self, track: &Track, start_at: f64, autoplay: bool);
    fn play(&self);
    fn pause(&self);
    fn seek(&self, seconds: f64);
    fn set_volume(&self, volume: f32);
    /// Stops playback on the device (the session stays; the app disconnects it).
    fn stop(&self);
}

/// What a [`RemoteOutput`] reports back.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteEvent {
    /// The current track is loaded on the device.
    Loaded { duration: Option<f64> },
    Position(f64),
    /// The device finished the track.
    Ended,
    /// The device itself started (`true`) or paused (`false`).
    Playing(bool),
}

/// The next track is downloaded and queued this long before the current one ends.
const PRELOAD_BEFORE_END: f64 = 20.0;

struct Inner {
    state: Mutex<Core>,
    state_tx: watch::Sender<Arc<PlayerState>>,
    progress_tx: watch::Sender<Progress>,
    resolver: RwLock<Option<Resolver>>,
    audio: Mutex<Option<Sender<Msg>>>,
    remote: RwLock<Option<Arc<dyn RemoteOutput>>>,
    /// Output device (`None` = system default), applied when the audio thread starts.
    device: Mutex<Option<String>>,
}

#[derive(Default)]
struct Core {
    pub s: PlayerState,
    progress: Progress,
    /// Generation of the load in flight / playing.
    load_gen: u64,
    /// Generation and queue uid of the preload asked for, if any.
    preload: Option<(u64, String)>,
    /// The audio thread holds the current entry, ready to play / pause.
    loaded: bool,
    crossfade: CrossfadeSettings,
}

#[derive(Clone)]
pub struct MusicPlayer {
    inner: Arc<Inner>,
}

impl Default for MusicPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicPlayer {
    pub fn new() -> Self {
        let s = PlayerState {
            volume: 1.0,
            ..PlayerState::default()
        };
        let (state_tx, _) = watch::channel(Arc::new(s.clone()));
        let (progress_tx, _) = watch::channel(Progress::default());
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(Core {
                    s,
                    ..Core::default()
                }),
                state_tx,
                progress_tx,
                resolver: RwLock::new(None),
                audio: Mutex::new(None),
                remote: RwLock::new(None),
                device: Mutex::new(None),
            }),
        }
    }

    /// Sets how tracks become URLs (called by the app when the active server changes).
    pub fn set_resolver(&self, resolver: Resolver) {
        *self
            .inner
            .resolver
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Some(resolver);
    }

    /// Crossfade settings (the `crossfade*` preferences), read when the next track is preloaded.
    pub fn set_crossfade(&self, s: CrossfadeSettings) {
        self.lock().crossfade = s;
    }

    pub fn crossfade(&self) -> CrossfadeSettings {
        self.lock().crossfade
    }

    /// Audio output device (`None` = system default); playback moves to it at once.
    pub fn set_output_device(&self, name: Option<String>) {
        {
            let mut d = self.inner.device.lock().unwrap_or_else(|p| p.into_inner());
            if *d == name {
                return;
            }
            d.clone_from(&name);
        }
        // Nothing plays yet: the thread picks the device up when it starts.
        let started = self
            .inner
            .audio
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some();
        if started {
            self.send(Msg::Device(name));
        }
    }

    pub fn output_device(&self) -> Option<String> {
        self.inner
            .device
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn state(&self) -> Arc<PlayerState> {
        self.inner.state_tx.borrow().clone()
    }

    pub fn progress(&self) -> Progress {
        *self.inner.progress_tx.borrow()
    }

    pub fn subscribe_state(&self) -> watch::Receiver<Arc<PlayerState>> {
        self.inner.state_tx.subscribe()
    }

    pub fn subscribe_progress(&self) -> watch::Receiver<Progress> {
        self.inner.progress_tx.subscribe()
    }

    pub fn current_track(&self) -> Option<Arc<Track>> {
        self.state().current().map(|i| i.track.clone())
    }

    // ------------------------------------------------------------------ musicPlayer API

    /// Plays `tracks` from `index` (a random first track when shuffling without an index).
    pub fn play(&self, tracks: &[Track], index: Option<usize>, shuffle: Option<bool>) {
        let mut c = self.lock();
        let shuffle = shuffle.unwrap_or(c.s.model.shuffle);
        let m = queue::create_queue(
            tracks,
            index,
            shuffle,
            c.s.model.repeat,
            &mut queue::default_rng(),
        );
        if m.index < 0 {
            return;
        }
        let i = m.index as usize;
        self.go_to(&mut c, m, i, true);
    }

    pub fn play_next(&self, tracks: &[Track]) {
        let mut c = self.lock();
        let empty = c.s.model.queue.is_empty();
        let m = queue::insert_next(&c.s.model, tracks);
        if empty && m.index >= 0 {
            let i = m.index as usize;
            self.go_to(&mut c, m, i, true);
        } else {
            c.s.model = m;
            self.publish(&c);
        }
    }

    pub fn add_to_queue(&self, tracks: &[Track]) {
        let mut c = self.lock();
        let empty = c.s.model.queue.is_empty();
        let m = queue::append(&c.s.model, tracks);
        if empty && m.index >= 0 {
            let i = m.index as usize;
            self.go_to(&mut c, m, i, true);
        } else {
            c.s.model = m;
            self.publish(&c);
        }
    }

    pub fn remove_at(&self, i: usize) {
        let mut c = self.lock();
        let r = queue::remove_at(&c.s.model, i);
        if !r.removed_current {
            c.s.model = r.model;
            return self.publish(&c);
        }
        if r.model.index < 0 {
            c.s.model = r.model;
            c.s.status = PlayerStatus::Idle;
            c.s.want_playing = false;
            c.progress = Progress::default();
            c.load_gen += 1;
            c.loaded = false;
            self.send(Msg::Stop);
            self.publish(&c);
        } else {
            let want = c.s.want_playing;
            let idx = r.model.index as usize;
            self.go_to(&mut c, r.model, idx, want);
        }
    }

    pub fn move_item(&self, from: usize, to: usize) {
        let mut c = self.lock();
        c.s.model = queue::move_item(&c.s.model, from, to);
        self.publish(&c);
    }

    pub fn clear_upcoming(&self) {
        let mut c = self.lock();
        c.s.model = queue::clear_upcoming(&c.s.model);
        self.publish(&c);
    }

    pub fn jump_to(&self, i: usize) {
        let mut c = self.lock();
        if i >= c.s.model.queue.len() {
            return;
        }
        let m = c.s.model.clone();
        self.go_to(&mut c, m, i, true);
    }

    pub fn next(&self) {
        let mut c = self.lock();
        if let Some(i) = queue::next_index(&c.s.model, Advance::User) {
            let m = c.s.model.clone();
            self.go_to(&mut c, m, i, true);
        }
    }

    pub fn previous(&self) {
        let mut c = self.lock();
        if c.s.model.index < 0 {
            return;
        }
        match queue::previous_action(&c.s.model, c.progress.position) {
            PreviousAction::Restart => {
                drop(c);
                self.seek(0.0);
            }
            PreviousAction::Go(i) => {
                let m = c.s.model.clone();
                self.go_to(&mut c, m, i, true);
            }
        }
    }

    pub fn pause(&self) {
        let mut c = self.lock();
        c.s.want_playing = false;
        if c.s.status == PlayerStatus::Playing {
            c.s.status = PlayerStatus::Paused;
        }
        self.send(Msg::Pause);
        self.publish(&c);
    }

    pub fn resume(&self) {
        let mut c = self.lock();
        if c.s.model.queue.is_empty() {
            return;
        }
        c.s.want_playing = true;
        if c.loaded {
            c.s.status = PlayerStatus::Playing;
            self.send(Msg::Play);
            self.publish(&c);
        } else if c.s.status == PlayerStatus::Loading {
            // The load in flight starts playing when it is done.
            self.send(Msg::Play);
            self.publish(&c);
        } else {
            // A restored session or the end of the queue: load the current entry now.
            if c.s.model.index < 0 {
                c.s.model.index = 0;
            }
            let start = c.progress.position;
            self.send_load(&mut c, start);
        }
    }

    pub fn toggle(&self) {
        if self.state().want_playing {
            self.pause()
        } else {
            self.resume()
        }
    }

    pub fn seek(&self, seconds: f64) {
        let mut c = self.lock();
        if c.s.model.index < 0 {
            return;
        }
        let max = if c.progress.duration > 0.0 {
            c.progress.duration
        } else {
            f64::INFINITY
        };
        c.progress.position = seconds.clamp(0.0, max);
        self.send(Msg::Seek(c.progress.position));
        self.inner.progress_tx.send_replace(c.progress);
    }

    pub fn set_volume(&self, v: f32) {
        let mut c = self.lock();
        c.s.volume = v.clamp(0.0, 1.0);
        c.s.muted = false;
        self.apply_volume(&c);
        self.publish(&c);
    }

    pub fn toggle_mute(&self) {
        let mut c = self.lock();
        c.s.muted = !c.s.muted;
        self.apply_volume(&c);
        self.publish(&c);
    }

    pub fn toggle_shuffle(&self) {
        let mut c = self.lock();
        let on = !c.s.model.shuffle;
        c.s.model = queue::set_shuffle(&c.s.model, on, &mut queue::default_rng());
        self.publish(&c);
    }

    pub fn cycle_repeat(&self) {
        let mut c = self.lock();
        c.s.model.repeat = queue::cycle_repeat(c.s.model.repeat);
        self.publish(&c);
    }

    pub fn set_repeat(&self, repeat: RepeatMode) {
        let mut c = self.lock();
        c.s.model.repeat = repeat;
        self.publish(&c);
    }

    pub fn stop(&self) {
        let mut c = self.lock();
        let (volume, muted) = (c.s.volume, c.s.muted);
        c.s = PlayerState {
            volume,
            muted,
            ..PlayerState::default()
        };
        c.progress = Progress::default();
        c.load_gen += 1;
        c.preload = None;
        c.loaded = false;
        self.send(Msg::Stop);
        self.publish(&c);
    }

    /// Restores a saved session paused at `position` (persistence of the queue).
    pub fn restore(&self, model: QueueModel, position: f64, volume: f32, muted: bool) {
        let mut c = self.lock();
        let has = queue::current_item(&model).is_some();
        c.s = PlayerState {
            model,
            status: if has {
                PlayerStatus::Paused
            } else {
                PlayerStatus::Idle
            },
            volume,
            muted,
            want_playing: false,
        };
        c.progress = Progress {
            position,
            duration: 0.0,
        };
        c.loaded = false;
        c.load_gen += 1;
        self.send(Msg::Stop);
        self.publish(&c);
    }

    // ------------------------------------------------------------------ internals

    fn lock(&self) -> MutexGuard<'_, Core> {
        self.inner.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn publish(&self, c: &Core) {
        self.inner.state_tx.send_replace(Arc::new(c.s.clone()));
        self.inner.progress_tx.send_replace(c.progress);
    }

    fn remote(&self) -> Option<Arc<dyn RemoteOutput>> {
        self.inner
            .remote
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Plays on `remote` (a cast device) instead of this computer, or back here with `None`. The
    /// current track and position carry over; going back to local leaves the music paused.
    pub fn set_remote(&self, remote: Option<Arc<dyn RemoteOutput>>) {
        let previous = self.remote();
        if remote.is_none() && previous.is_none() {
            return;
        }
        if previous.is_none() {
            self.send(Msg::Stop); // silence the local output
        }
        if let Some(p) = previous {
            p.stop();
        }
        let going_remote = remote.is_some();
        *self.inner.remote.write().unwrap_or_else(|p| p.into_inner()) = remote;
        let mut c = self.lock();
        if !going_remote {
            c.s.want_playing = false;
        }
        let at = c.progress.position;
        if c.s.current().is_some() {
            self.send_load(&mut c, at);
        }
    }

    /// Whether the music currently plays on a remote device.
    pub fn is_remote(&self) -> bool {
        self.remote().is_some()
    }

    /// Report of the remote output (position, end of track, ...).
    pub fn remote_event(&self, event: RemoteEvent) {
        match event {
            RemoteEvent::Loaded { duration } => {
                let gen = self.lock().load_gen;
                self.on_event(Event::Loaded { gen, duration });
            }
            RemoteEvent::Position(p) => self.on_event(Event::Position(p)),
            RemoteEvent::Ended => self.on_event(Event::Ended),
            RemoteEvent::Playing(playing) => {
                let mut c = self.lock();
                if c.loaded && c.s.want_playing != playing {
                    c.s.want_playing = playing;
                    c.s.status = if playing {
                        PlayerStatus::Playing
                    } else {
                        PlayerStatus::Paused
                    };
                    self.publish(&c);
                }
            }
        }
    }

    fn send(&self, msg: Msg) {
        if let Some(remote) = self.remote() {
            match msg {
                Msg::Play => remote.play(),
                Msg::Pause => remote.pause(),
                Msg::Seek(s) => remote.seek(s),
                Msg::Volume(v) => remote.set_volume(v),
                Msg::Stop => remote.stop(),
                _ => {}
            }
            return;
        }
        let mut audio = self.inner.audio.lock().unwrap_or_else(|p| p.into_inner());
        if audio.is_none() {
            if matches!(msg, Msg::Pause | Msg::Stop | Msg::Seek(_)) {
                return; // nothing plays yet
            }
            let weak = Arc::downgrade(&self.inner);
            let tx = output::spawn(move |event| {
                if let Some(inner) = weak.upgrade() {
                    MusicPlayer { inner }.on_event(event);
                }
            });
            if let Some(name) = self.output_device() {
                let _ = tx.send(Msg::Device(Some(name)));
            }
            *audio = Some(tx);
        }
        if let Some(tx) = audio.as_ref() {
            let _ = tx.send(msg);
        }
    }

    fn apply_volume(&self, c: &Core) {
        let v = if c.s.muted { 0.0 } else { c.s.volume };
        self.send(Msg::Volume(v));
    }

    fn resolve(&self, track: &Track) -> Option<String> {
        let r = self
            .inner
            .resolver
            .read()
            .unwrap_or_else(|p| p.into_inner());
        r.as_ref().and_then(|f| f(track))
    }

    /// Switches to the entry at `index` and loads it (`goTo` of `store.ts`).
    fn go_to(&self, c: &mut Core, model: QueueModel, index: usize, autoplay: bool) {
        c.s.model = model;
        c.s.model.index = index as isize;
        c.s.want_playing = autoplay;
        c.progress = Progress::default();
        self.send_load(c, 0.0);
    }

    fn send_load(&self, c: &mut Core, start_at: f64) {
        c.load_gen += 1;
        c.preload = None;
        c.loaded = false;
        let Some(track) = c.s.current().map(|i| i.track.clone()) else {
            return;
        };
        c.progress.duration = track.duration_seconds();
        if let Some(remote) = self.remote() {
            c.s.status = PlayerStatus::Loading;
            remote.load(&track, start_at, c.s.want_playing);
            self.publish(c);
            return;
        }
        match self.resolve(&track) {
            Some(url) => {
                c.s.status = PlayerStatus::Loading;
                self.apply_volume(c);
                self.send(Msg::Load {
                    gen: c.load_gen,
                    url,
                    start_at,
                    autoplay: c.s.want_playing,
                });
            }
            None => {
                c.s.status = PlayerStatus::Error;
                c.s.want_playing = false;
            }
        }
        self.publish(c);
    }

    fn on_event(&self, event: Event) {
        let mut c = self.lock();
        match event {
            Event::Loaded { gen, duration } => {
                if gen != c.load_gen {
                    return;
                }
                if let Some(d) = duration.filter(|d| *d > 0.0) {
                    c.progress.duration = d;
                }
                c.loaded = true;
                c.s.status = if c.s.want_playing {
                    PlayerStatus::Playing
                } else {
                    PlayerStatus::Paused
                };
                self.publish(&c);
            }
            Event::Position(p) => {
                c.progress.position = p;
                self.inner.progress_tx.send_replace(c.progress);
                self.maybe_preload(&mut c);
            }
            Event::Advanced { gen, duration } => {
                let expected = queue::next_index(&c.s.model, Advance::Auto);
                let matches = match (&c.preload, expected) {
                    (Some((g, uid)), Some(i)) => {
                        *g == gen && c.s.model.queue.get(i).is_some_and(|q| &q.uid == uid)
                    }
                    _ => false,
                };
                c.preload = None;
                match expected {
                    Some(i) if matches => {
                        c.load_gen = gen;
                        c.s.model.index = i as isize;
                        let meta =
                            c.s.current()
                                .map(|q| q.track.duration_seconds())
                                .unwrap_or(0.0);
                        c.progress = Progress {
                            position: 0.0,
                            duration: duration.filter(|d| *d > 0.0).unwrap_or(meta),
                        };
                        c.s.status = PlayerStatus::Playing;
                        c.loaded = true;
                        self.publish(&c);
                    }
                    // The queue changed after the preload: play what the queue says now.
                    Some(i) => {
                        let m = c.s.model.clone();
                        self.go_to(&mut c, m, i, true);
                    }
                    None => self.end_of_queue(&mut c),
                }
            }
            Event::Ended => match queue::next_index(&c.s.model, Advance::Auto) {
                Some(i) => {
                    let m = c.s.model.clone();
                    self.go_to(&mut c, m, i, true);
                }
                None => self.end_of_queue(&mut c),
            },
            Event::Failed { gen, message } => {
                if gen != c.load_gen {
                    return;
                }
                log::warn!("[music] cannot play this track: {message}");
                // `skipUnplayable`: go on with the next track, or stop on an error.
                match queue::next_index(&c.s.model, Advance::User) {
                    Some(i) if i as isize != c.s.model.index => {
                        let m = c.s.model.clone();
                        let want = c.s.want_playing;
                        self.go_to(&mut c, m, i, want);
                    }
                    _ => {
                        c.s.want_playing = false;
                        c.s.status = PlayerStatus::Error;
                        self.publish(&c);
                    }
                }
            }
        }
    }

    /// End of the queue: stay on the last entry, ready to be played again from the start.
    fn end_of_queue(&self, c: &mut Core) {
        c.s.want_playing = false;
        c.s.status = PlayerStatus::Paused;
        c.progress.position = 0.0;
        c.load_gen += 1;
        c.loaded = false;
        self.send(Msg::Stop);
        self.publish(c);
    }

    fn maybe_preload(&self, c: &mut Core) {
        if c.preload.is_some() || c.s.status != PlayerStatus::Playing || self.is_remote() {
            return;
        }
        let remaining = c.progress.duration - c.progress.position;
        if c.progress.duration <= 0.0 || remaining > PRELOAD_BEFORE_END {
            return;
        }
        let Some(i) = queue::next_index(&c.s.model, Advance::Auto) else {
            return;
        };
        let Some(item) = c.s.model.queue.get(i).cloned() else {
            return;
        };
        let Some(url) = self.resolve(&item.track) else {
            return;
        };
        let fade = plan_fade(&FadePlanInput {
            settings: c.crossfade,
            current: c.s.current().map(|q| &*q.track).unwrap_or(&item.track),
            next: &item.track,
            repeat: c.s.model.repeat,
            current_duration: Some(c.progress.duration),
        });
        c.load_gen += 1;
        let gen = c.load_gen;
        // The playing source keeps its generation until it is replaced: Loaded/Failed of the
        // current track can no longer arrive, and Advanced carries this one.
        c.preload = Some((gen, item.uid));
        self.send(Msg::Preload {
            gen,
            url,
            fade,
            current_duration: c.progress.duration,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                id: format!("t{i}"),
                run_time_ticks: Some(1_800_000_000),
                ..Track::default()
            })
            .collect()
    }

    /// No resolver: loads fail synchronously into the error state, so the logic is testable
    /// without an audio device.
    #[test]
    fn queue_actions_follow_the_store_rules() {
        let p = MusicPlayer::new();
        p.play(&tracks(3), Some(1), Some(false));
        let s = p.state();
        assert_eq!(s.model.index, 1);
        assert_eq!(s.status, PlayerStatus::Error);
        p.toggle_shuffle();
        assert!(p.state().model.shuffle);
        p.cycle_repeat();
        assert_eq!(p.state().model.repeat, RepeatMode::All);
        p.set_volume(1.4);
        assert_eq!(p.state().volume, 1.0);
        p.stop();
        let s = p.state();
        assert!(s.model.queue.is_empty());
        assert_eq!(s.volume, 1.0);
    }

    #[test]
    fn restore_is_paused_at_the_saved_position() {
        let p = MusicPlayer::new();
        let model = queue::create_queue(
            &tracks(2),
            Some(1),
            false,
            RepeatMode::Off,
            &mut queue::default_rng(),
        );
        p.restore(model, 42.0, 0.5, false);
        assert_eq!(p.state().status, PlayerStatus::Paused);
        assert_eq!(p.progress().position, 42.0);
        assert!(!p.state().is_playing());
    }
}
