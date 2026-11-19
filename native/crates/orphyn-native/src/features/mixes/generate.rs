//! `mixes/generate.ts`: mix generation rules. Pure (no network, no UI): the data comes from a
//! [`MixSource`] so the rules run on plain tracks in tests.
use std::collections::{HashMap, HashSet};
use std::future::Future;

use orphyn_jellyfin::BaseItemDto;
use serde::{Deserialize, Serialize};

pub type Track = BaseItemDto;

pub const MAX_TRACKS: usize = 50;
/// A mix smaller than this is not worth showing (small libraries).
pub const MIN_SHOWN: usize = 10;
pub const MAX_PER_ARTIST: usize = 3;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const FOR_YOU: usize = 3;
const GENRE_MIXES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MixKind {
    ForYou,
    Discovery,
    OnRepeat,
    Rediscover,
    Genre,
    TopYear,
}

impl MixKind {
    /// The key of the `mixes` namespace (`names.<key>`, `hints.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            MixKind::ForYou => "forYou",
            MixKind::Discovery => "discovery",
            MixKind::OnRepeat => "onRepeat",
            MixKind::Rediscover => "rediscover",
            MixKind::Genre => "genre",
            MixKind::TopYear => "topYear",
        }
    }

    fn hue(self) -> u32 {
        match self {
            MixKind::ForYou => 330,
            MixKind::Discovery => 265,
            MixKind::OnRepeat => 8,
            MixKind::Rediscover => 38,
            MixKind::Genre => 160,
            MixKind::TopYear => 205,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    /// Stable id, used in the URL of the mix page.
    pub id: String,
    pub kind: MixKind,
    /// 1-3 for the "For You" mixes.
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub genre: Option<String>,
    /// A few artists heard in the mix, for its subtitle.
    pub artists: Vec<String>,
    /// Hue (0-360) of the generated cover.
    pub hue: u32,
    pub generated_at: i64,
    pub tracks: Vec<Track>,
}

/// What the generator reads from the server.
pub trait MixSource {
    /// The user's played tracks (most played first) and loved tracks.
    fn signals(&self) -> impl Future<Output = (Vec<Track>, Vec<Track>)> + Send;
    /// Tracks close to a song (InstantMix).
    fn similar_to(&self, id: &str, limit: u32) -> impl Future<Output = Vec<Track>> + Send;
    /// Songs of the artists most similar to the given ones, and the artists' own songs.
    fn discovery_pool(
        &self,
        artist_ids: &[String],
        limit: u32,
    ) -> impl Future<Output = Vec<Track>> + Send;
    /// Random songs of a genre.
    fn genre_tracks(&self, genre: &str, limit: u32) -> impl Future<Output = Vec<Track>> + Send;
}

// ---------------------------------------------------------------- track helpers

pub fn artist_key(t: &Track) -> String {
    t.artist_items
        .first()
        .and_then(|a| a.id.clone())
        .or_else(|| t.album_artists.first().and_then(|a| a.id.clone()))
        .or_else(|| t.artists.first().cloned())
        .or_else(|| t.album_artist.clone())
        .unwrap_or_default()
        .to_lowercase()
}

pub fn album_key(t: &Track) -> String {
    t.album_id
        .clone()
        .or_else(|| t.album.clone())
        .unwrap_or_default()
}

fn title_key(t: &Track) -> String {
    format!("{}|{}", t.name().trim().to_lowercase(), artist_key(t))
}

pub fn play_count(t: &Track) -> i64 {
    t.user_data.as_ref().and_then(|u| u.play_count).unwrap_or(0)
}

/// `Date.parse` of the last play (UTC, as Jellyfin writes it), in ms; 0 when absent.
fn last_played(t: &Track) -> i64 {
    t.user_data
        .as_ref()
        .and_then(|u| u.last_played_date.as_deref())
        .and_then(parse_iso_ms)
        .unwrap_or(0)
}

pub fn parse_iso_ms(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T').unwrap_or((s, "00:00:00"));
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.trim_end_matches('Z');
    let time = time.split('+').next().unwrap_or(time);
    let mut parts = time.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let mi: i64 = parts.next().unwrap_or("0").parse().ok()?;
    let sec: f64 = parts.next().unwrap_or("0").parse().ok()?;
    let days = days_from_civil(y, m, day);
    Some(((days * 86_400 + h * 3600 + mi * 60) as f64 * 1000.0 + sec * 1000.0) as i64)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `YYYY-MM-DD` of a time in ms (UTC): the unit of the daily refresh and of the daily seed.
pub fn day_of(ms: i64) -> String {
    let (y, m, d) = civil_from_days(ms.div_euclid(DAY_MS));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Deterministic generator (mulberry32) so a mix of a given day does not change on every launch.
pub struct Rng(u32);

impl Rng {
    pub fn seeded(seed: &str) -> Self {
        let units: Vec<u16> = seed.encode_utf16().collect();
        let mut h: u32 = 1_779_033_703 ^ units.len() as u32;
        for c in units {
            h = (h ^ u32::from(c)).wrapping_mul(3_432_918_353);
            h = h.rotate_left(13);
        }
        Rng(h)
    }

    /// In [0, 1).
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

pub fn shuffled<T: Clone>(items: &[T], rng: &mut Rng) -> Vec<T> {
    let mut out = items.to_vec();
    for i in (1..out.len()).rev() {
        let j = (rng.next() * (i + 1) as f64).floor() as usize;
        out.swap(i, j);
    }
    out
}

// ---------------------------------------------------------------- diversity & order

#[derive(Default)]
pub struct Diversify<'a> {
    pub limit: Option<usize>,
    pub max_per_artist: Option<usize>,
    /// Ids that must not appear, e.g. what a radio queued already.
    pub exclude: Option<&'a HashSet<String>>,
}

/// Keeps the first occurrence of every track (by id, and by title + artist) and at most
/// `max_per_artist` tracks per artist.
pub fn diversify(tracks: &[Track], opts: &Diversify) -> Vec<Track> {
    let limit = opts.limit.unwrap_or(MAX_TRACKS);
    let max = opts.max_per_artist.unwrap_or(MAX_PER_ARTIST);
    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    let mut per_artist: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for t in tracks {
        if out.len() >= limit {
            break;
        }
        if t.id.is_empty()
            || ids.contains(&t.id)
            || opts.exclude.is_some_and(|e| e.contains(&t.id))
        {
            continue;
        }
        let title = title_key(t);
        if titles.contains(&title) {
            continue;
        }
        let artist = artist_key(t);
        let n = per_artist.get(&artist).copied().unwrap_or(0);
        if !artist.is_empty() && n >= max {
            continue;
        }
        ids.insert(t.id.clone());
        titles.insert(title);
        per_artist.insert(artist, n + 1);
        out.push(t.clone());
    }
    out
}

/// Shuffles, then walks the result picking at each step the first track that shares neither album
/// nor artist with the previous one (falling back to "not the same album", then to anything).
pub fn order_pleasantly(tracks: &[Track], rng: &mut Rng) -> Vec<Track> {
    let mut rest = shuffled(tracks, rng);
    let mut out: Vec<Track> = Vec::new();
    while !rest.is_empty() {
        let mut at = 0;
        if let Some(prev) = out.last() {
            let (album, artist) = (album_key(prev), artist_key(prev));
            let differs = |t: &Track, strict: bool| {
                (album.is_empty() || album_key(t) != album)
                    && (!strict || artist.is_empty() || artist_key(t) != artist)
            };
            at = rest
                .iter()
                .position(|t| differs(t, true))
                .or_else(|| rest.iter().position(|t| differs(t, false)))
                .unwrap_or(0);
        }
        out.push(rest.remove(at));
    }
    out
}

pub struct Pool {
    pub tracks: Vec<Track>,
    /// Relative share of the mix this pool fills.
    pub weight: f64,
}

/// Takes from the pools in proportion to their weights, keeping each pool's own order.
pub fn blend_pools(pools: &[Pool]) -> Vec<Track> {
    let queues: Vec<&Pool> = pools.iter().filter(|p| p.weight > 0.0).collect();
    let mut taken = vec![0usize; queues.len()];
    let mut out = Vec::new();
    loop {
        let mut best: Option<usize> = None;
        for (i, q) in queues.iter().enumerate() {
            if taken[i] >= q.tracks.len() {
                continue;
            }
            let ratio = |k: usize| taken[k] as f64 / queues[k].weight;
            if best.is_none_or(|b| ratio(i) < ratio(b)) {
                best = Some(i);
            }
        }
        let Some(b) = best else { return out };
        out.push(queues[b].tracks[taken[b]].clone());
        taken[b] += 1;
    }
}

/// One mix: the pools are blended by weight, deduplicated, capped per artist, cut at
/// [`MAX_TRACKS`] and put in a pleasant order. `None` when too few tracks are left.
pub fn assemble_tracks(pools: &[Pool], rng: &mut Rng) -> Option<Vec<Track>> {
    let picked = diversify(
        &blend_pools(pools),
        &Diversify {
            limit: Some(MAX_TRACKS),
            ..Diversify::default()
        },
    );
    (picked.len() >= MIN_SHOWN).then(|| order_pleasantly(&picked, rng))
}

// ---------------------------------------------------------------- listening signals

/// Genres by how much they are listened to (every play counts, a track never played counts once).
pub fn top_genres(tracks: &[Track], count: usize) -> Vec<(String, i64)> {
    let mut weights: Vec<(String, i64)> = Vec::new();
    for t in tracks {
        for g in t.genres.iter().filter(|g| !g.is_empty()) {
            let w = play_count(t) + 1;
            match weights.iter_mut().find(|(n, _)| n == g) {
                Some(e) => e.1 += w,
                None => weights.push((g.clone(), w)),
            }
        }
    }
    weights.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    weights.truncate(count);
    weights
}

/// Most listened artists (ids), by play count.
pub fn top_artist_ids(tracks: &[Track], count: usize) -> Vec<String> {
    let mut weights: Vec<(String, i64)> = Vec::new();
    for t in tracks {
        let id = t
            .artist_items
            .first()
            .and_then(|a| a.id.clone())
            .or_else(|| t.album_artists.first().and_then(|a| a.id.clone()));
        if let Some(id) = id {
            let w = play_count(t) + 1;
            match weights.iter_mut().find(|(n, _)| *n == id) {
                Some(e) => e.1 += w,
                None => weights.push((id, w)),
            }
        }
    }
    weights.sort_by_key(|w| std::cmp::Reverse(w.1));
    weights.into_iter().take(count).map(|(id, _)| id).collect()
}

/// Splits played tracks into one cluster per top genre (a track goes to its best ranked genre),
/// the most played tracks first.
pub fn cluster_by_genre(played: &[Track], count: usize) -> Vec<(String, Vec<Track>)> {
    let genres: Vec<String> = top_genres(played, count).into_iter().map(|g| g.0).collect();
    let mut clusters: Vec<(String, Vec<Track>)> =
        genres.iter().map(|g| (g.clone(), Vec::new())).collect();
    for t in played {
        if let Some(at) = genres.iter().position(|g| t.genres.contains(g)) {
            clusters[at].1.push(t.clone());
        }
    }
    for c in &mut clusters {
        c.1.sort_by_key(|t| std::cmp::Reverse(play_count(t)));
    }
    clusters.retain(|c| !c.1.is_empty());
    clusters
}

/// Picks `n` tracks at random, favouring the most played ones.
pub fn sample_by_plays(tracks: &[Track], n: usize, rng: &mut Rng) -> Vec<Track> {
    let mut rest: Vec<(Track, i64)> = tracks
        .iter()
        .map(|t| (t.clone(), play_count(t) + 1))
        .collect();
    let mut out = Vec::new();
    while out.len() < n && !rest.is_empty() {
        let total: i64 = rest.iter().map(|x| x.1).sum();
        let mut r = rng.next() * total as f64;
        let mut at = rest.len() - 1;
        for (i, x) in rest.iter().enumerate() {
            r -= x.1 as f64;
            if r < 0.0 {
                at = i;
                break;
            }
        }
        out.push(rest.remove(at).0);
    }
    out
}

/// Played in the last 30 days, the most played first.
pub fn on_repeat_tracks(played: &[Track], now: i64) -> Vec<Track> {
    let mut out: Vec<Track> = played
        .iter()
        .filter(|t| play_count(t) > 0 && last_played(t) > 0 && now - last_played(t) <= 30 * DAY_MS)
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        play_count(b)
            .cmp(&play_count(a))
            .then_with(|| last_played(b).cmp(&last_played(a)))
    });
    out
}

/// Loved tracks (hearts, and songs played a lot) not played for 3 months or more.
pub fn rediscover_tracks(favourites: &[Track], played: &[Track], now: i64) -> Vec<Track> {
    let stale = |t: &Track| last_played(t) == 0 || now - last_played(t) >= 90 * DAY_MS;
    let mut hearts: Vec<Track> = favourites.iter().filter(|t| stale(t)).cloned().collect();
    let mut heard: Vec<Track> = played
        .iter()
        .filter(|t| play_count(t) >= 3 && stale(t))
        .cloned()
        .collect();
    hearts.sort_by_key(last_played);
    heard.sort_by_key(last_played);
    hearts.extend(heard);
    hearts
}

/// Never or rarely played (at most one play).
pub fn rarely_played(tracks: &[Track]) -> Vec<Track> {
    tracks
        .iter()
        .filter(|t| play_count(t) <= 1)
        .cloned()
        .collect()
}

// ---------------------------------------------------------------- the mixes

fn hash(s: &str) -> u32 {
    let mut h: i32 = 0;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(i32::from(u));
    }
    h.unsigned_abs()
}

fn hue_of(kind: MixKind, key: &str) -> u32 {
    match kind {
        MixKind::Genre => hash(key) % 360,
        MixKind::ForYou => (kind.hue() + hash(key) % 50) % 360,
        _ => kind.hue() % 360,
    }
}

/// The few artists of a mix, for its subtitle.
pub fn artists_of(tracks: &[Track]) -> Vec<String> {
    let mut artists: Vec<String> = Vec::new();
    for t in tracks {
        let a = t
            .artist_items
            .first()
            .and_then(|a| a.name.clone())
            .or_else(|| t.artists.first().cloned())
            .or_else(|| t.album_artist.clone());
        if let Some(a) = a.filter(|a| !a.is_empty()) {
            if !artists.contains(&a) {
                artists.push(a);
            }
        }
        if artists.len() == 3 {
            break;
        }
    }
    artists
}

fn to_base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(char::from_digit(n % 36, 36).unwrap_or('0'));
        n /= 36;
    }
    out.iter().rev().collect()
}

fn make(
    kind: MixKind,
    tracks: Option<Vec<Track>>,
    now: i64,
    n: Option<u32>,
    genre: Option<String>,
) -> Option<Mix> {
    let tracks = tracks?;
    let key = genre
        .clone()
        .or_else(|| n.map(|n| n.to_string()))
        .unwrap_or_default();
    let id = if key.is_empty() {
        kind.key().to_string()
    } else if kind == MixKind::Genre {
        format!("{}-{}", kind.key(), to_base36(hash(&key)))
    } else {
        format!("{}-{key}", kind.key())
    };
    Some(Mix {
        id,
        kind,
        n,
        year: None,
        genre,
        artists: artists_of(&tracks),
        hue: hue_of(kind, &key),
        generated_at: now,
        tracks,
    })
}

/// The "Top titres <année>" mix; `None` when it has too few plays to be worth a mix.
pub fn make_top_year_mix(year: i32, tracks: Vec<Track>, now: i64) -> Option<Mix> {
    (tracks.len() >= 5).then(|| Mix {
        id: format!("topYear-{year}"),
        kind: MixKind::TopYear,
        n: None,
        year: Some(year),
        genre: None,
        artists: artists_of(&tracks),
        hue: 205,
        generated_at: now,
        tracks,
    })
}

/// The daily mixes of a user. Order: For You 1-3, Discovery, On Repeat, Rediscover, genre mixes.
/// A mix the library cannot fill is left out.
pub async fn generate_mixes<S: MixSource>(source: &S, now: i64) -> Vec<Mix> {
    let (played, favourites) = source.signals().await;
    let day = day_of(now);
    let rng_for = |id: &str| Rng::seeded(&format!("{day}:{id}"));
    let mut mixes: Vec<Mix> = Vec::new();

    // For You: one mix per cluster of favourite genres, plus similar tracks they play less.
    for (i, (genre, cluster)) in cluster_by_genre(&played, FOR_YOU).into_iter().enumerate() {
        let n = i as u32 + 1;
        let mut rng = rng_for(&format!("forYou-{n}"));
        let seeds = sample_by_plays(&cluster, 32, &mut rng);
        let seed = seeds.first().or(cluster.first());
        let similar = match seed {
            Some(s) if !s.id.is_empty() => source.similar_to(&s.id, 40).await,
            _ => Vec::new(),
        };
        let fresh = source.genre_tracks(&genre, 40).await;
        let mut mixed = similar;
        mixed.extend(rarely_played(&fresh));
        let tracks = assemble_tracks(
            &[
                Pool {
                    tracks: seeds,
                    weight: 3.0,
                },
                Pool {
                    tracks: shuffled(&mixed, &mut rng),
                    weight: 2.0,
                },
            ],
            &mut rng,
        );
        mixes.extend(make(MixKind::ForYou, tracks, now, Some(n), None));
    }

    // Discovery: unplayed tracks from artists close to the favourites.
    {
        let mut rng = rng_for("discovery");
        let all: Vec<Track> = favourites.iter().chain(played.iter()).cloned().collect();
        let pool = source.discovery_pool(&top_artist_ids(&all, 6), 80).await;
        let tracks = assemble_tracks(
            &[Pool {
                tracks: shuffled(&rarely_played(&pool), &mut rng),
                weight: 1.0,
            }],
            &mut rng,
        );
        mixes.extend(make(MixKind::Discovery, tracks, now, None, None));
    }

    {
        let mut rng = rng_for("onRepeat");
        let tracks = assemble_tracks(
            &[Pool {
                tracks: on_repeat_tracks(&played, now),
                weight: 1.0,
            }],
            &mut rng,
        );
        mixes.extend(make(MixKind::OnRepeat, tracks, now, None, None));
    }

    {
        let mut rng = rng_for("rediscover");
        let pool = rediscover_tracks(&favourites, &played, now);
        let tracks = assemble_tracks(
            &[Pool {
                tracks: shuffled(&pool, &mut rng),
                weight: 1.0,
            }],
            &mut rng,
        );
        mixes.extend(make(MixKind::Rediscover, tracks, now, None, None));
    }

    // Genre mixes: top genres, played tracks and a handful of others of the genre.
    for (genre, _) in top_genres(&played, GENRE_MIXES) {
        let mut rng = rng_for(&format!("genre-{genre}"));
        let own: Vec<Track> = played
            .iter()
            .filter(|t| t.genres.contains(&genre))
            .cloned()
            .collect();
        let others = source.genre_tracks(&genre, 60).await;
        let tracks = assemble_tracks(
            &[
                Pool {
                    tracks: sample_by_plays(&own, 30, &mut rng),
                    weight: 1.0,
                },
                Pool {
                    tracks: shuffled(&others, &mut rng),
                    weight: 1.0,
                },
            ],
            &mut rng,
        );
        mixes.extend(make(MixKind::Genre, tracks, now, None, Some(genre)));
    }
    mixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use orphyn_jellyfin::{NameIdPair, UserItemData};

    fn track(id: &str, artist: &str, album: &str, genre: &str, plays: i64, last: Option<&str>) -> Track {
        Track {
            id: id.into(),
            name: Some(format!("Song {id}")),
            album_id: Some(album.into()),
            genres: vec![genre.into()],
            artist_items: vec![NameIdPair {
                id: Some(artist.into()),
                name: Some(artist.into()),
            }],
            user_data: Some(UserItemData {
                play_count: Some(plays),
                last_played_date: last.map(str::to_string),
                ..UserItemData::default()
            }),
            ..Track::default()
        }
    }

    #[test]
    fn day_and_iso_parse_agree() {
        let ms = parse_iso_ms("2026-03-05T10:20:30.5Z").unwrap();
        assert_eq!(day_of(ms), "2026-03-05");
        assert_eq!(ms % 1000, 500);
        assert_eq!(day_of(0), "1970-01-01");
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (
            Rng::seeded("2026-03-05:forYou-1"),
            Rng::seeded("2026-03-05:forYou-1"),
        );
        for _ in 0..20 {
            let x = a.next();
            assert_eq!(x, b.next());
            assert!((0.0..1.0).contains(&x));
        }
        assert_ne!(Rng::seeded("a").next(), Rng::seeded("b").next());
    }

    #[test]
    fn diversify_dedups_and_caps_per_artist() {
        let tracks: Vec<Track> = (0..6)
            .map(|i| track(&format!("t{i}"), "a1", "al", "Rock", 0, None))
            .collect();
        let mut with_dup = tracks.clone();
        with_dup.push(tracks[0].clone());
        let out = diversify(&with_dup, &Diversify::default());
        assert_eq!(out.len(), MAX_PER_ARTIST);
    }

    #[test]
    fn blend_follows_weights() {
        let mk = |p: &str, n: usize| {
            (0..n)
                .map(|i| track(&format!("{p}{i}"), p, p, "x", 0, None))
                .collect::<Vec<_>>()
        };
        let out = blend_pools(&[
            Pool { tracks: mk("a", 6), weight: 3.0 },
            Pool { tracks: mk("b", 6), weight: 1.0 },
        ]);
        assert_eq!(out.len(), 12);
        let from_a = out.iter().take(8).filter(|t| t.id.starts_with('a')).count();
        assert_eq!(from_a, 6);
    }

    #[test]
    fn genres_repeat_and_rediscover() {
        let now = parse_iso_ms("2026-06-01T00:00:00Z").unwrap();
        let recent = track("r", "a", "x", "Rock", 5, Some("2026-05-20T00:00:00Z"));
        let old = track("o", "a", "x", "Jazz", 9, Some("2025-01-01T00:00:00Z"));
        let played = vec![recent, old];
        assert_eq!(top_genres(&played, 1)[0].0, "Jazz");
        assert_eq!(on_repeat_tracks(&played, now).len(), 1);
        let re = rediscover_tracks(&[], &played, now);
        assert_eq!(re.len(), 1);
        assert_eq!(re[0].id, "o");
    }

    #[test]
    fn assemble_needs_enough_tracks() {
        let mk = |n: usize| -> Vec<Track> {
            (0..n)
                .map(|i| track(&format!("t{i}"), &format!("a{i}"), &format!("b{i}"), "g", 0, None))
                .collect()
        };
        let mut rng = Rng::seeded("x");
        assert!(assemble_tracks(&[Pool { tracks: mk(4), weight: 1.0 }], &mut rng).is_none());
        let got = assemble_tracks(&[Pool { tracks: mk(12), weight: 1.0 }], &mut rng).unwrap();
        assert_eq!(got.len(), 12);
    }

    #[test]
    fn top_year_mix_threshold() {
        let ts: Vec<Track> = (0..5)
            .map(|i| track(&format!("t{i}"), "a", "b", "g", 1, None))
            .collect();
        assert_eq!(make_top_year_mix(2026, ts.clone(), 0).unwrap().id, "topYear-2026");
        assert!(make_top_year_mix(2026, ts[..4].to_vec(), 0).is_none());
    }
}
