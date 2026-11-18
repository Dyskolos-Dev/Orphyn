//! Pure queue model: a 1:1 port of `features/music/player/queue.ts` (same rules, same tests).
use std::sync::atomic::{AtomicU64, Ordering};

use super::types::{QueueItem, QueueModel, RepeatMode, Track};

/// "Previous" restarts the track instead when it has played longer than this.
pub const PREVIOUS_RESTART_SECONDS: f64 = 3.0;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_uid() -> String {
    format!("q{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

pub fn to_items(tracks: &[Track]) -> Vec<QueueItem> {
    tracks
        .iter()
        .map(|t| QueueItem::new(new_uid(), t.clone()))
        .collect()
}

/// Random source in [0, 1), injectable for deterministic tests.
pub type Rng<'a> = &'a mut dyn FnMut() -> f64;

/// A small xorshift seeded from the clock: good enough to shuffle a queue.
pub fn default_rng() -> impl FnMut() -> f64 {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1;
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub fn shuffled<T: Clone>(items: &[T], rng: Rng) -> Vec<T> {
    let mut a = items.to_vec();
    for i in (1..a.len()).rev() {
        let j = ((rng() * (i + 1) as f64).floor() as usize).min(i);
        a.swap(i, j);
    }
    a
}

pub fn current_item(m: &QueueModel) -> Option<&QueueItem> {
    usize::try_from(m.index).ok().and_then(|i| m.queue.get(i))
}

pub fn create_queue(
    tracks: &[Track],
    start_index: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    rng: Rng,
) -> QueueModel {
    let items = to_items(tracks);
    if items.is_empty() {
        return QueueModel {
            shuffle,
            repeat,
            ..QueueModel::default()
        };
    }
    if !shuffle {
        let index = start_index.unwrap_or(0).min(items.len() - 1);
        return QueueModel {
            queue: items,
            original: None,
            index: index as isize,
            shuffle: false,
            repeat,
        };
    }
    let start = match start_index {
        None => ((rng() * items.len() as f64).floor() as usize).min(items.len() - 1),
        Some(i) => i.min(items.len() - 1),
    };
    let first = items[start].clone();
    let rest: Vec<QueueItem> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != start)
        .map(|(_, x)| x.clone())
        .collect();
    let mut queue = vec![first];
    queue.extend(shuffled(&rest, rng));
    QueueModel {
        queue,
        original: Some(items),
        index: 0,
        shuffle: true,
        repeat,
    }
}

pub fn insert_next(m: &QueueModel, tracks: &[Track]) -> QueueModel {
    let items = to_items(tracks);
    if items.is_empty() {
        return m.clone();
    }
    let at = (m.index + 1).max(0) as usize;
    let mut queue = m.queue[..at.min(m.queue.len())].to_vec();
    queue.extend(items.iter().cloned());
    queue.extend(m.queue[at.min(m.queue.len())..].iter().cloned());
    let original = m.original.as_ref().map(|orig| {
        let pos = current_item(m)
            .and_then(|cur| orig.iter().position(|i| i.uid == cur.uid))
            .map(|p| p + 1)
            .unwrap_or(0);
        let mut o = orig[..pos].to_vec();
        o.extend(items.iter().cloned());
        o.extend(orig[pos..].iter().cloned());
        o
    });
    QueueModel {
        queue,
        original,
        index: if m.index < 0 { 0 } else { m.index },
        ..m.clone()
    }
}

pub fn append(m: &QueueModel, tracks: &[Track]) -> QueueModel {
    let items = to_items(tracks);
    if items.is_empty() {
        return m.clone();
    }
    let mut queue = m.queue.clone();
    queue.extend(items.iter().cloned());
    let original = m.original.as_ref().map(|o| {
        let mut o = o.clone();
        o.extend(items.iter().cloned());
        o
    });
    QueueModel {
        queue,
        original,
        index: if m.index < 0 { 0 } else { m.index },
        ..m.clone()
    }
}

pub struct RemoveResult {
    pub model: QueueModel,
    pub removed_current: bool,
}

pub fn remove_at(m: &QueueModel, i: usize) -> RemoveResult {
    let Some(item) = m.queue.get(i) else {
        return RemoveResult {
            model: m.clone(),
            removed_current: false,
        };
    };
    let uid = item.uid.clone();
    let queue: Vec<QueueItem> = m
        .queue
        .iter()
        .enumerate()
        .filter(|(k, _)| *k != i)
        .map(|(_, x)| x.clone())
        .collect();
    let original = m
        .original
        .as_ref()
        .map(|o| o.iter().filter(|x| x.uid != uid).cloned().collect());
    let removed_current = i as isize == m.index;
    let mut index = m.index;
    if queue.is_empty() {
        index = -1;
    } else if (i as isize) < m.index {
        index = m.index - 1;
    } else if removed_current {
        index = m.index.min(queue.len() as isize - 1);
    }
    RemoveResult {
        model: QueueModel {
            queue,
            original,
            index,
            ..m.clone()
        },
        removed_current,
    }
}

pub fn move_item(m: &QueueModel, from: usize, to: usize) -> QueueModel {
    if from == to || from >= m.queue.len() || to >= m.queue.len() {
        return m.clone();
    }
    let mut queue = m.queue.clone();
    let item = queue.remove(from);
    queue.insert(to, item);
    let (from, to) = (from as isize, to as isize);
    let mut index = m.index;
    if from == m.index {
        index = to;
    } else if from < m.index && to >= m.index {
        index = m.index - 1;
    } else if from > m.index && to <= m.index {
        index = m.index + 1;
    }
    QueueModel {
        queue,
        index,
        ..m.clone()
    }
}

pub fn clear_upcoming(m: &QueueModel) -> QueueModel {
    if m.index < 0 {
        return m.clone();
    }
    let keep: Vec<QueueItem> = m.queue[..=(m.index as usize)].to_vec();
    let original = m.original.as_ref().map(|o| {
        o.iter()
            .filter(|x| keep.iter().any(|k| k.uid == x.uid))
            .cloned()
            .collect()
    });
    QueueModel {
        queue: keep,
        original,
        ..m.clone()
    }
}

pub fn set_shuffle(m: &QueueModel, on: bool, rng: Rng) -> QueueModel {
    if on == m.shuffle {
        return m.clone();
    }
    if on {
        if m.index < 0 {
            return QueueModel {
                shuffle: true,
                original: Some(m.queue.clone()),
                ..m.clone()
            };
        }
        let split = m.index as usize + 1;
        let mut queue = m.queue[..split].to_vec();
        queue.extend(shuffled(&m.queue[split..], rng));
        return QueueModel {
            shuffle: true,
            original: Some(m.queue.clone()),
            queue,
            ..m.clone()
        };
    }
    let cur = current_item(m).map(|c| c.uid.clone());
    let source = m.original.as_ref().unwrap_or(&m.queue);
    let mut queue: Vec<QueueItem> = source
        .iter()
        .filter(|i| m.queue.iter().any(|q| q.uid == i.uid))
        .cloned()
        .collect();
    for i in &m.queue {
        if !queue.iter().any(|q| q.uid == i.uid) {
            queue.push(i.clone());
        }
    }
    let index = cur
        .and_then(|u| queue.iter().position(|i| i.uid == u))
        .map(|p| p as isize)
        .unwrap_or(-1);
    QueueModel {
        shuffle: false,
        original: None,
        queue,
        index,
        ..m.clone()
    }
}

pub fn cycle_repeat(r: RepeatMode) -> RepeatMode {
    match r {
        RepeatMode::Off => RepeatMode::All,
        RepeatMode::All => RepeatMode::One,
        RepeatMode::One => RepeatMode::Off,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    /// The track ended by itself.
    Auto,
    /// The user pressed "next".
    User,
}

pub fn next_index(m: &QueueModel, reason: Advance) -> Option<usize> {
    if m.queue.is_empty() || m.index < 0 {
        return None;
    }
    let i = m.index as usize;
    if reason == Advance::Auto && m.repeat == RepeatMode::One {
        return Some(i);
    }
    if i + 1 < m.queue.len() {
        return Some(i + 1);
    }
    if m.repeat == RepeatMode::Off {
        None
    } else {
        Some(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviousAction {
    Restart,
    Go(usize),
}

pub fn previous_action(m: &QueueModel, position_seconds: f64) -> PreviousAction {
    if position_seconds > PREVIOUS_RESTART_SECONDS || m.index <= 0 {
        PreviousAction::Restart
    } else {
        PreviousAction::Go(m.index as usize - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                id: format!("t{i}"),
                ..Track::default()
            })
            .collect()
    }

    fn ids(m: &QueueModel) -> Vec<String> {
        m.queue.iter().map(|i| i.track.id.clone()).collect()
    }

    fn fixed(v: f64) -> impl FnMut() -> f64 {
        move || v
    }

    #[test]
    fn create_without_shuffle_starts_at_the_index() {
        let m = create_queue(&tracks(3), Some(1), false, RepeatMode::Off, &mut fixed(0.0));
        assert_eq!(ids(&m), ["t0", "t1", "t2"]);
        assert_eq!(m.index, 1);
        assert!(m.original.is_none());
    }

    #[test]
    fn create_with_shuffle_puts_the_start_first_and_keeps_the_original() {
        let m = create_queue(&tracks(4), Some(2), true, RepeatMode::Off, &mut fixed(0.0));
        assert_eq!(m.queue[0].track.id, "t2");
        assert_eq!(m.index, 0);
        assert_eq!(m.original.as_ref().map(Vec::len), Some(4));
        let mut sorted = ids(&m);
        sorted.sort();
        assert_eq!(sorted, ["t0", "t1", "t2", "t3"]);
    }

    #[test]
    fn insert_next_and_append() {
        let m = create_queue(&tracks(3), Some(0), false, RepeatMode::Off, &mut fixed(0.0));
        let m = insert_next(&m, &tracks(1));
        assert_eq!(ids(&m), ["t0", "t0", "t1", "t2"]);
        let m = append(
            &m,
            &[Track {
                id: "x".into(),
                ..Track::default()
            }],
        );
        assert_eq!(ids(&m).last().map(String::as_str), Some("x"));
        assert_eq!(m.index, 0);
    }

    #[test]
    fn removing_before_or_at_the_current_entry_keeps_it_current() {
        let m = create_queue(&tracks(4), Some(2), false, RepeatMode::Off, &mut fixed(0.0));
        let r = remove_at(&m, 0);
        assert!(!r.removed_current);
        assert_eq!(r.model.index, 1);
        assert_eq!(current_item(&r.model).unwrap().track.id, "t2");
        let r = remove_at(&r.model, 1);
        assert!(r.removed_current);
        assert_eq!(current_item(&r.model).unwrap().track.id, "t3");
    }

    #[test]
    fn moving_follows_the_current_entry() {
        let m = create_queue(&tracks(4), Some(1), false, RepeatMode::Off, &mut fixed(0.0));
        let moved = move_item(&m, 1, 3);
        assert_eq!(moved.index, 3);
        let moved = move_item(&m, 3, 0);
        assert_eq!(moved.index, 2);
    }

    #[test]
    fn shuffle_off_restores_the_original_order() {
        let m = create_queue(&tracks(5), Some(0), false, RepeatMode::Off, &mut fixed(0.0));
        let mut r = default_rng();
        let s = set_shuffle(&m, true, &mut r);
        assert_eq!(s.queue[0].track.id, "t0");
        let back = set_shuffle(&s, false, &mut r);
        assert_eq!(ids(&back), ["t0", "t1", "t2", "t3", "t4"]);
        assert_eq!(back.index, 0);
    }

    #[test]
    fn next_and_previous_follow_repeat() {
        let mut m = create_queue(&tracks(2), Some(1), false, RepeatMode::Off, &mut fixed(0.0));
        assert_eq!(next_index(&m, Advance::User), None);
        m.repeat = RepeatMode::All;
        assert_eq!(next_index(&m, Advance::User), Some(0));
        m.repeat = RepeatMode::One;
        assert_eq!(next_index(&m, Advance::Auto), Some(1));
        assert_eq!(next_index(&m, Advance::User), Some(0));
        assert_eq!(previous_action(&m, 10.0), PreviousAction::Restart);
        assert_eq!(previous_action(&m, 1.0), PreviousAction::Go(0));
        assert_eq!(cycle_repeat(RepeatMode::Off), RepeatMode::All);
    }

    #[test]
    fn clear_upcoming_keeps_history_and_current() {
        let m = create_queue(&tracks(4), Some(1), false, RepeatMode::Off, &mut fixed(0.0));
        assert_eq!(ids(&clear_upcoming(&m)), ["t0", "t1"]);
    }
}
