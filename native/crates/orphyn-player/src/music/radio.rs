//! Radios: an endless queue seeded by a song, an album or an artist (port of
//! `features/mixes/radio.ts`). The queue is refilled in the background before it runs out, and the
//! radio ends by itself when the user plays something else.
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use tokio::sync::watch;

use super::player::MusicPlayer;
use super::types::{QueueItem, Track};

/// Songs left after the current one under which the queue is refilled.
pub const REFILL_THRESHOLD: usize = 8;
/// Failed (empty) refills in a row after which the radio gives up.
pub const MAX_EMPTY_REFILLS: u32 = 3;
const BATCH: usize = 25;
const MAX_PER_ARTIST: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioKind {
    Song,
    Album,
    Artist,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioSeed {
    pub id: String,
    pub name: String,
    pub kind: RadioKind,
}

/// More songs for a seed (`id`); the second argument is the song playing now.
pub type Fetcher = Arc<
    dyn Fn(String, Option<String>) -> Pin<Box<dyn Future<Output = Vec<Track>> + Send>>
        + Send
        + Sync,
>;

// ---------------------------------------------------------------------------------- rules

fn artist_key(t: &Track) -> String {
    t.artist_items
        .first()
        .and_then(|a| a.id.clone())
        .or_else(|| t.album_artists.first().and_then(|a| a.id.clone()))
        .or_else(|| t.artists.first().cloned())
        .or_else(|| t.album_artist.clone())
        .unwrap_or_default()
        .to_lowercase()
}

fn title_key(t: &Track) -> String {
    format!("{}|{}", t.name().trim().to_lowercase(), artist_key(t))
}

/// Keeps the first occurrence of every track (by id, and by title + artist) and at most
/// `max_per_artist` tracks per artist; `exclude` ids never appear (`diversify`).
pub fn diversify(
    tracks: &[Track],
    limit: usize,
    max_per_artist: usize,
    exclude: &HashSet<String>,
) -> Vec<Track> {
    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    let mut per_artist: std::collections::HashMap<String, usize> = Default::default();
    let mut out = Vec::new();
    for t in tracks {
        if out.len() >= limit {
            break;
        }
        if t.id.is_empty() || ids.contains(&t.id) || exclude.contains(&t.id) {
            continue;
        }
        let title = title_key(t);
        if titles.contains(&title) {
            continue;
        }
        let artist = artist_key(t);
        let n = per_artist.get(&artist).copied().unwrap_or(0);
        if !artist.is_empty() && n >= max_per_artist {
            continue;
        }
        ids.insert(t.id.clone());
        titles.insert(title);
        per_artist.insert(artist, n + 1);
        out.push(t.clone());
    }
    out
}

/// The songs of `candidates` a radio may append: none heard or queued already, at most 3 per
/// artist, and the artist of the last queued song not first.
pub fn pick_refill(
    candidates: &[Track],
    avoid: &HashSet<String>,
    last_artist: Option<&str>,
    limit: usize,
) -> Vec<Track> {
    let mut fresh = diversify(candidates, limit * 2, MAX_PER_ARTIST, avoid);
    let who = |t: &Track| {
        t.artist_items
            .first()
            .and_then(|a| a.id.clone())
            .or_else(|| t.artists.first().cloned())
            .unwrap_or_default()
    };
    let first = fresh
        .iter()
        .position(|t| last_artist.is_none_or(|l| l.is_empty()) || who(t) != last_artist.unwrap());
    if let Some(i) = first.filter(|i| *i > 0) {
        let t = fresh.remove(i);
        fresh.insert(0, t);
    }
    fresh.truncate(limit);
    fresh
}

/// The id used to compare artists when appending (`ArtistItems[0].Id ?? Artists[0]`).
pub fn last_artist_of(t: &Track) -> Option<String> {
    t.artist_items
        .first()
        .and_then(|a| a.id.clone())
        .or_else(|| t.artists.first().cloned())
}

/// Public wrapper: the first songs of a new radio (`pickRefill([...first, ...batch], {}, ∅, 30)`).
pub fn first_queue(first: &[Track], batch: &[Track]) -> Vec<Track> {
    let all: Vec<Track> = first.iter().chain(batch).cloned().collect();
    pick_refill(&all, &HashSet::new(), None, 30)
}

/// What to do after the queue moved.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Nothing,
    Stop,
    Refill,
}

fn decide(
    known: &HashSet<String>,
    queue: &[QueueItem],
    index: isize,
    empty: u32,
    threshold: usize,
) -> Decision {
    let current = (index >= 0)
        .then(|| queue.get(index as usize))
        .flatten()
        .map(|q| q.track.id.as_str());
    // Queue emptied, or the user started something else: the radio is over.
    if queue.is_empty() || current.is_some_and(|c| !known.contains(c)) {
        return Decision::Stop;
    }
    let left = queue.len() as isize - 1 - index;
    if left < threshold as isize && empty < MAX_EMPTY_REFILLS {
        Decision::Refill
    } else {
        Decision::Nothing
    }
}

// ---------------------------------------------------------------------------------- controller

#[derive(Default)]
struct Core {
    seed: Option<RadioSeed>,
    /// Bumped by every start / stop so stale tasks give up.
    epoch: u64,
    /// Ids the radio put in the queue (also its "already heard" memory).
    known: HashSet<String>,
    busy: bool,
    empty: u32,
    fetcher: Option<Fetcher>,
}

struct Inner {
    player: MusicPlayer,
    core: Mutex<Core>,
    state_tx: watch::Sender<Option<RadioSeed>>,
    threshold: usize,
}

#[derive(Clone)]
pub struct Radio {
    inner: Arc<Inner>,
}

/// The app's radio (one player, one radio).
pub fn radio(player: &MusicPlayer) -> Radio {
    static RADIO: OnceLock<Radio> = OnceLock::new();
    RADIO
        .get_or_init(|| Radio::new(player.clone(), REFILL_THRESHOLD))
        .clone()
}

impl Radio {
    pub fn new(player: MusicPlayer, threshold: usize) -> Self {
        let (state_tx, _) = watch::channel(None);
        Self {
            inner: Arc::new(Inner {
                player,
                core: Mutex::new(Core::default()),
                state_tx,
                threshold,
            }),
        }
    }

    fn core(&self) -> MutexGuard<'_, Core> {
        self.inner.core.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The running radio's seed (`useRadio`).
    pub fn seed(&self) -> Option<RadioSeed> {
        self.inner.state_tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<RadioSeed>> {
        self.inner.state_tx.subscribe()
    }

    /// Starts a radio whose first songs are `first` (already queued by the caller). Needs a
    /// tokio runtime.
    pub fn start(&self, seed: RadioSeed, first: &[Track], fetcher: Fetcher) {
        self.stop();
        let epoch = {
            let mut c = self.core();
            c.epoch += 1;
            c.seed = Some(seed.clone());
            c.known = first
                .iter()
                .map(|t| t.id.clone())
                .filter(|i| !i.is_empty())
                .collect();
            c.fetcher = Some(fetcher);
            c.epoch
        };
        self.inner.state_tx.send_replace(Some(seed));
        let me = self.clone();
        let mut rx = self.inner.player.subscribe_state();
        tokio::spawn(async move {
            me.check(epoch);
            while rx.changed().await.is_ok() {
                if me.core().epoch != epoch {
                    return;
                }
                me.check(epoch);
            }
        });
    }

    pub fn stop(&self) {
        {
            let mut c = self.core();
            if c.seed.is_none() {
                return;
            }
            c.epoch += 1;
            c.seed = None;
            c.known.clear();
            c.busy = false;
            c.empty = 0;
            c.fetcher = None;
        }
        self.inner.state_tx.send_replace(None);
    }

    /// Looks at the queue and refills it when it is running low.
    fn check(&self, epoch: u64) {
        let state = self.inner.player.state();
        let decision = {
            let c = self.core();
            if c.epoch != epoch || c.seed.is_none() {
                return;
            }
            if c.busy {
                return;
            }
            decide(
                &c.known,
                &state.model.queue,
                state.model.index,
                c.empty,
                self.inner.threshold,
            )
        };
        match decision {
            Decision::Nothing => {}
            Decision::Stop => self.stop(),
            Decision::Refill => {
                let me = self.clone();
                tokio::spawn(async move { me.refill(epoch).await });
            }
        }
    }

    async fn refill(&self, epoch: u64) {
        let (seed, fetcher) = {
            let mut c = self.core();
            let (Some(seed), Some(f)) = (c.seed.clone(), c.fetcher.clone()) else {
                return;
            };
            if c.epoch != epoch || c.busy {
                return;
            }
            c.busy = true;
            (seed, f)
        };
        let current = self.inner.player.current_track().map(|t| t.id.clone());
        let more = fetcher(seed.id.clone(), current).await;
        let state = self.inner.player.state();
        let picked = {
            let mut c = self.core();
            c.busy = false;
            if c.epoch != epoch {
                return;
            }
            let mut avoid = c.known.clone();
            avoid.extend(state.model.queue.iter().map(|q| q.track.id.clone()));
            let last = state
                .model
                .queue
                .last()
                .and_then(|q| last_artist_of(&q.track));
            let picked = pick_refill(&more, &avoid, last.as_deref(), BATCH);
            if picked.is_empty() {
                c.empty += 1;
                let give_up = c.empty >= MAX_EMPTY_REFILLS;
                drop(c);
                if give_up {
                    self.stop();
                }
                return;
            }
            c.empty = 0;
            c.known.extend(picked.iter().map(|t| t.id.clone()));
            picked
        };
        self.inner.player.add_to_queue(&picked);
        // The queue may still be short (small batch): look again.
        self.check(epoch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orphyn_jellyfin::NameIdPair;

    fn song(id: &str, artist: &str) -> Track {
        Track {
            id: id.into(),
            name: Some(format!("song {id}")),
            artist_items: vec![NameIdPair {
                id: Some(artist.into()),
                name: Some(artist.into()),
            }],
            ..Track::default()
        }
    }

    fn ids(v: &[Track]) -> Vec<&str> {
        v.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn diversify_dedups_and_caps_per_artist() {
        let mut dup = song("9", "a");
        dup.name = Some("song 1".into()); // same title + artist as "1"
        let list = vec![
            song("1", "a"),
            song("1", "a"),
            dup,
            song("2", "a"),
            song("3", "a"),
            song("4", "a"),
            song("5", "b"),
        ];
        let out = diversify(&list, 50, 3, &HashSet::new());
        assert_eq!(ids(&out), ["1", "2", "3", "5"]);
        let ex: HashSet<String> = ["2".to_string()].into();
        assert_eq!(ids(&diversify(&list, 2, 3, &ex)), ["1", "3"]);
    }

    #[test]
    fn refill_skips_heard_songs_and_avoids_the_last_artist_first() {
        let avoid: HashSet<String> = ["1".to_string()].into();
        let list = vec![
            song("1", "a"),
            song("2", "a"),
            song("3", "b"),
            song("4", "c"),
        ];
        let out = pick_refill(&list, &avoid, Some("a"), 25);
        assert_eq!(ids(&out), ["3", "2", "4"]);
        let out = pick_refill(&list, &avoid, None, 2);
        assert_eq!(ids(&out), ["2", "3"]);
    }

    fn queue(n: usize) -> Vec<QueueItem> {
        (0..n)
            .map(|i| QueueItem::new(format!("u{i}"), song(&format!("t{i}"), "a")))
            .collect()
    }

    #[test]
    fn refills_below_the_threshold_and_stops_when_the_user_plays_something_else() {
        let known: HashSet<String> = (0..20).map(|i| format!("t{i}")).collect();
        let q = queue(12);
        assert_eq!(decide(&known, &q, 0, 0, 8), Decision::Nothing);
        assert_eq!(decide(&known, &q, 5, 0, 8), Decision::Refill);
        assert_eq!(
            decide(&known, &q, 5, MAX_EMPTY_REFILLS, 8),
            Decision::Nothing
        );
        assert_eq!(decide(&known, &[], -1, 0, 8), Decision::Stop);
        // The current song is not one the radio queued.
        let other = vec![QueueItem::new("x".into(), song("zzz", "a"))];
        assert_eq!(decide(&known, &other, 0, 0, 8), Decision::Stop);
    }

    #[tokio::test]
    async fn radio_refills_the_player_queue_and_ends_by_itself() {
        let player = MusicPlayer::new();
        let first: Vec<Track> = (0..3)
            .map(|i| song(&format!("s{i}"), &format!("a{i}")))
            .collect();
        player.play(&first, Some(0), Some(false));
        let radio = Radio::new(player.clone(), 8);
        let fetcher: Fetcher = Arc::new(|_seed, _cur| {
            Box::pin(async {
                (0..30)
                    .map(|i| song(&format!("m{i}"), &format!("b{}", i / 2)))
                    .collect()
            })
        });
        let seed = RadioSeed {
            id: "seed".into(),
            name: "Seed".into(),
            kind: RadioKind::Song,
        };
        radio.start(seed.clone(), &first, fetcher);
        assert_eq!(radio.seed(), Some(seed));
        for _ in 0..100 {
            if player.state().model.queue.len() > 20 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let n = player.state().model.queue.len();
        assert!(n >= 3 + 8, "queue was refilled: {n}");
        // Playing something unrelated ends the radio.
        player.play(&[song("other", "z")], Some(0), Some(false));
        for _ in 0..100 {
            if radio.seed().is_none() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(radio.seed(), None);
    }
}
