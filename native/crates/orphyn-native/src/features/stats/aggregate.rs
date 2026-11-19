//! `aggregate.ts`: pure aggregation of a user's played items into a year in review. No I/O.
//!
//! Dates are read in UTC (the crate has no time-zone source): a play within a few hours of New
//! Year's or of a month change can land one bucket away from the React app, which uses local time.
use std::collections::{HashMap, HashSet};

use orphyn_jellyfin::BaseItemDto;
use serde_json::json;

use crate::features::search::normalize;

const TICKS_PER_MINUTE: f64 = 600_000_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct RankedEntry {
    pub key: String,
    pub name: String,
    pub plays: i64,
    /// Representative item, used for its artwork.
    pub item: BaseItemDto,
    /// Item whose Primary image to show when it is not `item` itself (a series' own poster).
    pub art: Option<BaseItemDto>,
    /// Id of the page this entry opens (artist, album, film or series); `None` = not linkable.
    pub target_id: Option<String>,
    /// Secondary line: artist of a song or album, year of a film.
    pub subtitle: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenreEntry {
    pub name: String,
    pub plays: i64,
    /// Share of all genre plays, 0..1.
    pub share: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MusicStats {
    pub plays: i64,
    pub minutes: i64,
    pub distinct_songs: usize,
    pub artists: Vec<RankedEntry>,
    pub songs: Vec<RankedEntry>,
    pub albums: Vec<RankedEntry>,
    pub genres: Vec<GenreEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoStats {
    pub movies: Vec<RankedEntry>,
    pub series: Vec<RankedEntry>,
    pub movie_plays: i64,
    pub episode_plays: i64,
    pub minutes: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub total_minutes: i64,
    /// 0-based month with the most plays, `None` when there is no data.
    pub busiest_month: Option<usize>,
    /// Share of music plays going to the top artist, 0..1.
    pub top_artist_share: f64,
    /// Average minutes per day over the year (up to today for the current year).
    pub minutes_per_day: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct YearStats {
    pub year: i32,
    pub music: MusicStats,
    pub video: VideoStats,
    pub facts: Facts,
    pub is_empty: bool,
}

pub type PluginPlays = HashMap<String, i64>;

#[derive(Default)]
pub struct AggregateOptions<'a> {
    /// Exact play counts per item id from the Playback Reporting plugin; otherwise approximated.
    pub plugin_plays: Option<&'a PluginPlays>,
    /// Unix seconds; injected for tests.
    pub now: i64,
    pub limit: Option<usize>,
}

// ------------------------------------------------------------------------------------ dates

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date (year, month 1..=12, day) of a day count since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// Unix seconds of an ISO 8601 timestamp (`2026-03-04T21:15:00.1234567Z`, offsets accepted).
pub fn parse_iso(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    let num =
        |from: usize, len: usize| -> Option<i64> { iso.get(from..from + len)?.parse::<i64>().ok() };
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y as i32, mo as u32, d as u32) * 86_400;
    let mut rest = &iso[10..];
    if rest.starts_with(['T', ' ']) && rest.len() >= 6 {
        secs += num(11, 2)? * 3600 + num(14, 2)? * 60;
        rest = &rest[6..];
        if let Some(r) = rest.strip_prefix(':') {
            secs += r.get(0..2)?.parse::<i64>().ok()?;
            rest = &r[2..];
        }
        if let Some(r) = rest.strip_prefix('.') {
            rest = r.trim_start_matches(|c: char| c.is_ascii_digit());
        }
        if let Some(sign) = rest.chars().next().filter(|c| *c == '+' || *c == '-') {
            let off = &rest[1..];
            let h: i64 = off.get(0..2)?.parse().ok()?;
            let m: i64 = off
                .get(3..5)
                .or(off.get(2..4))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let total = h * 3600 + m * 60;
            secs -= if sign == '+' { total } else { -total };
        }
    }
    Some(secs)
}

/// (year, 0-based month) of an ISO timestamp.
fn year_month(iso: &str) -> Option<(i32, usize)> {
    let secs = parse_iso(iso)?;
    let (y, m, _) = civil_from_days(secs.div_euclid(86_400));
    Some((y, m as usize - 1))
}

pub fn year_of_secs(secs: i64) -> i32 {
    civil_from_days(secs.div_euclid(86_400)).0
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ------------------------------------------------------------------------------- aggregation

/// Plays of an item within `year`: exact from the plugin, else PlayCount when last played that year.
pub fn plays_in_year(item: &BaseItemDto, year: i32, plugin: Option<&PluginPlays>) -> i64 {
    if let Some(p) = plugin {
        return p.get(&item.id).copied().unwrap_or(0);
    }
    let Some(ud) = &item.user_data else { return 0 };
    let last = ud.last_played_date.as_deref().and_then(year_month);
    if last.map(|(y, _)| y) != Some(year) {
        return 0;
    }
    ud.play_count.unwrap_or(0).max(1)
}

/// Years (descending) in which something was last played, always including the current one.
pub fn available_years(items: &[BaseItemDto], now: i64) -> Vec<i32> {
    let mut years: HashSet<i32> = HashSet::from([year_of_secs(now)]);
    for it in items {
        if let Some((y, _)) = it
            .user_data
            .as_ref()
            .and_then(|u| u.last_played_date.as_deref())
            .and_then(year_month)
        {
            years.insert(y);
        }
    }
    let mut v: Vec<i32> = years.into_iter().collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

/// `localeCompare` stand-in: accent- and case-insensitive, then exact.
fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    normalize(a).cmp(&normalize(b)).then_with(|| a.cmp(b))
}

/// Entries by key, in first-seen order (the `Map` of the TypeScript).
#[derive(Default)]
struct Ranking {
    order: Vec<String>,
    map: HashMap<String, RankedEntry>,
}

impl Ranking {
    fn bump(&mut self, entry: RankedEntry, plays: i64) {
        if let Some(cur) = self.map.get_mut(&entry.key) {
            cur.plays += plays;
        } else {
            self.order.push(entry.key.clone());
            self.map
                .insert(entry.key.clone(), RankedEntry { plays, ..entry });
        }
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn top(&self, limit: usize) -> Vec<RankedEntry> {
        let mut v: Vec<RankedEntry> = self
            .order
            .iter()
            .filter_map(|k| self.map.get(k).cloned())
            .collect();
        v.sort_by(|a, b| {
            b.plays
                .cmp(&a.plays)
                .then_with(|| locale_cmp(&a.name, &b.name))
        });
        v.truncate(limit);
        v
    }
}

fn minutes_of(item: &BaseItemDto, plays: i64) -> f64 {
    item.run_time_ticks.unwrap_or(0) as f64 / TICKS_PER_MINUTE * plays as f64
}

/// The artist of a song: (id, name).
fn artist_of(item: &BaseItemDto) -> Option<(String, String)> {
    let first = item
        .artist_items
        .first()
        .or_else(|| item.album_artists.first());
    if let Some(a) = first {
        if let Some(name) = a.name.as_deref().filter(|n| !n.is_empty()) {
            return Some((
                a.id.clone().unwrap_or_else(|| name.to_string()),
                name.to_string(),
            ));
        }
    }
    let name = item
        .album_artist
        .clone()
        .filter(|n| !n.is_empty())
        .or_else(|| item.artists.first().cloned())?;
    Some((name.clone(), name))
}

/// The series an episode belongs to, as an item whose Primary image is the series poster.
fn series_item_of(ep: &BaseItemDto) -> Option<BaseItemDto> {
    let id = ep.series_id.as_deref().filter(|s| !s.is_empty())?;
    let tag = ep
        .series_primary_image_tag
        .as_deref()
        .filter(|s| !s.is_empty())?;
    serde_json::from_value(json!({
        "Id": id,
        "Name": ep.series_name,
        "Type": "Series",
        "ImageTags": { "Primary": tag },
    }))
    .ok()
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.clone().filter(|s| !s.is_empty())
}

/// The songs of the year, most played first (ties by title), at most `limit`. Only songs with at
/// least one play in `year` count; each song appears once.
pub fn top_songs(
    items: &[BaseItemDto],
    year: i32,
    plugin: Option<&PluginPlays>,
    limit: usize,
) -> Vec<BaseItemDto> {
    let mut seen = HashSet::new();
    let mut ranked: Vec<(&BaseItemDto, i64)> = Vec::new();
    for item in items {
        if !item.is_type("Audio") || item.id.is_empty() || seen.contains(&item.id) {
            continue;
        }
        let plays = plays_in_year(item, year, plugin);
        if plays <= 0 {
            continue;
        }
        seen.insert(item.id.clone());
        ranked.push((item, plays));
    }
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            locale_cmp(
                a.0.name.as_deref().unwrap_or(""),
                b.0.name.as_deref().unwrap_or(""),
            )
        })
    });
    ranked
        .into_iter()
        .take(limit)
        .map(|(i, _)| i.clone())
        .collect()
}

pub fn aggregate_year(items: &[BaseItemDto], year: i32, opts: &AggregateOptions) -> YearStats {
    let limit = opts.limit.unwrap_or(5);
    let mut artists = Ranking::default();
    let mut songs = Ranking::default();
    let mut albums = Ranking::default();
    let mut genres: Vec<(String, i64)> = Vec::new();
    let mut movies = Ranking::default();
    let mut series = Ranking::default();
    let mut months = [0i64; 12];
    let (mut music_plays, mut movie_plays, mut episode_plays) = (0i64, 0i64, 0i64);
    let (mut music_minutes, mut video_minutes) = (0f64, 0f64);

    for item in items {
        let plays = plays_in_year(item, year, opts.plugin_plays);
        if plays <= 0 || item.id.is_empty() {
            continue;
        }
        if let Some((y, m)) = item
            .user_data
            .as_ref()
            .and_then(|u| u.last_played_date.as_deref())
            .and_then(year_month)
        {
            if y == year {
                months[m] += plays;
            }
        }

        if item.is_type("Audio") {
            music_plays += plays;
            music_minutes += minutes_of(item, plays);
            let artist = artist_of(item);
            songs.bump(
                RankedEntry {
                    key: item.id.clone(),
                    name: item.name().to_string(),
                    plays: 0,
                    item: item.clone(),
                    art: None,
                    // A song opens its album.
                    target_id: non_empty(&item.album_id),
                    subtitle: artist.as_ref().map(|a| a.1.clone()),
                },
                plays,
            );
            if let Some((id, name)) = artist {
                let real_id = item
                    .artist_items
                    .first()
                    .and_then(|a| a.id.clone())
                    .or_else(|| item.album_artists.first().and_then(|a| a.id.clone()));
                artists.bump(
                    RankedEntry {
                        key: id,
                        name,
                        plays: 0,
                        item: item.clone(),
                        art: None,
                        target_id: real_id,
                        subtitle: None,
                    },
                    plays,
                );
            }
            if let (Some(album_id), Some(album)) = (
                item.album_id.as_deref().filter(|s| !s.is_empty()),
                item.album.as_deref().filter(|s| !s.is_empty()),
            ) {
                albums.bump(
                    RankedEntry {
                        key: album_id.to_string(),
                        name: album.to_string(),
                        plays: 0,
                        item: item.clone(),
                        art: None,
                        target_id: Some(album_id.to_string()),
                        subtitle: non_empty(&item.album_artist),
                    },
                    plays,
                );
            }
            for g in &item.genres {
                match genres.iter_mut().find(|(n, _)| n == g) {
                    Some(e) => e.1 += plays,
                    None => genres.push((g.clone(), plays)),
                }
            }
        } else if item.is_type("Movie") {
            movie_plays += plays;
            video_minutes += minutes_of(item, plays);
            movies.bump(
                RankedEntry {
                    key: item.id.clone(),
                    name: item.name().to_string(),
                    plays: 0,
                    item: item.clone(),
                    art: None,
                    target_id: Some(item.id.clone()),
                    subtitle: item
                        .production_year
                        .filter(|y| *y != 0)
                        .map(|y| y.to_string()),
                },
                plays,
            );
        } else if item.is_type("Episode") {
            episode_plays += plays;
            video_minutes += minutes_of(item, plays);
            let key = non_empty(&item.series_id).or_else(|| non_empty(&item.series_name));
            if let Some(key) = key {
                series.bump(
                    RankedEntry {
                        key,
                        name: item
                            .series_name
                            .clone()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| item.name().to_string()),
                        plays: 0,
                        item: item.clone(),
                        art: series_item_of(item),
                        target_id: non_empty(&item.series_id),
                        subtitle: None,
                    },
                    plays,
                );
            }
        }
    }

    let genre_total: i64 = genres.iter().map(|g| g.1).sum();
    genres.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| locale_cmp(&a.0, &b.0)));
    let genre_list: Vec<GenreEntry> = genres
        .into_iter()
        .take(limit)
        .map(|(name, p)| GenreEntry {
            name,
            plays: p,
            share: if genre_total > 0 {
                p as f64 / genre_total as f64
            } else {
                0.0
            },
        })
        .collect();

    let top_artists = artists.top(limit);
    let total_minutes = (music_minutes + video_minutes).round() as i64;
    let max_month = months.iter().copied().max().unwrap_or(0);
    let now_year = year_of_secs(opts.now);
    let day_count: f64 = if year == now_year {
        let jan1 = days_from_civil(year, 1, 1) * 86_400;
        (((opts.now - jan1) as f64) / 86_400.0).ceil().max(1.0)
    } else if is_leap(year) {
        366.0
    } else {
        365.0
    };

    YearStats {
        year,
        music: MusicStats {
            plays: music_plays,
            minutes: music_minutes.round() as i64,
            distinct_songs: songs.len(),
            artists: top_artists.clone(),
            songs: songs.top(limit),
            albums: albums.top(limit),
            genres: genre_list,
        },
        video: VideoStats {
            movies: movies.top(limit),
            series: series.top(limit),
            movie_plays,
            episode_plays,
            minutes: video_minutes.round() as i64,
        },
        facts: Facts {
            total_minutes,
            busiest_month: (max_month > 0)
                .then(|| months.iter().position(|m| *m == max_month))
                .flatten(),
            top_artist_share: match top_artists.first() {
                Some(a) if music_plays > 0 => a.plays as f64 / music_plays as f64,
                _ => 0.0,
            },
            minutes_per_day: total_minutes as f64 / day_count,
        },
        is_empty: music_plays + movie_plays + episode_plays == 0,
    }
}

/// Counts plugin rows per item id; rows without a recognisable id are ignored. `None` when no row
/// has an id.
pub fn count_plugin_rows(rows: &serde_json::Value) -> Option<PluginPlays> {
    let rows = rows.as_array()?;
    let mut counts = PluginPlays::new();
    for row in rows {
        let Some(obj) = row.as_object() else { continue };
        let id = ["item_id", "ItemId", "itemId"]
            .iter()
            .find_map(|k| obj.get(*k))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        if let Some(id) = id {
            *counts.entry(id.to_string()).or_insert(0) += 1;
        }
    }
    (!counts.is_empty()).then_some(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(iso: &str) -> i64 {
        parse_iso(iso).unwrap()
    }

    fn song(
        id: &str,
        name: &str,
        artist: &str,
        album: &str,
        plays: i64,
        played: &str,
    ) -> BaseItemDto {
        serde_json::from_value(json!({
            "Id": id, "Name": name, "Type": "Audio", "RunTimeTicks": 1_800_000_000i64,
            "AlbumId": format!("al-{album}"), "Album": album, "AlbumArtist": artist,
            "ArtistItems": [{"Id": format!("ar-{artist}"), "Name": artist}],
            "Genres": ["Rock"],
            "UserData": {"PlayCount": plays, "LastPlayedDate": played},
        }))
        .unwrap()
    }

    fn movie(id: &str, name: &str, plays: i64, played: &str) -> BaseItemDto {
        serde_json::from_value(json!({
            "Id": id, "Name": name, "Type": "Movie", "RunTimeTicks": 72_000_000_000i64,
            "ProductionYear": 1999,
            "UserData": {"PlayCount": plays, "LastPlayedDate": played},
        }))
        .unwrap()
    }

    fn episode(id: &str, series: &str, plays: i64, played: &str) -> BaseItemDto {
        serde_json::from_value(json!({
            "Id": id, "Name": "Ep", "Type": "Episode", "RunTimeTicks": 18_000_000_000i64,
            "SeriesId": format!("s-{series}"), "SeriesName": series, "SeriesPrimaryImageTag": "tag",
            "UserData": {"PlayCount": plays, "LastPlayedDate": played},
        }))
        .unwrap()
    }

    const NOW: &str = "2026-11-18T12:00:00Z";

    #[test]
    fn parses_iso_dates() {
        assert_eq!(at("1970-01-01T00:00:00Z"), 0);
        assert_eq!(at("2000-03-01T00:00:00.0000000Z"), 951_868_800);
        assert_eq!(at("2000-03-01T01:00:00+01:00"), 951_868_800);
        assert_eq!(year_month("2026-03-04T21:15:00Z"), Some((2026, 2)));
        assert_eq!(year_month("nope"), None);
    }

    #[test]
    fn plays_count_only_inside_the_year() {
        let s = song("a", "A", "X", "Al", 4, "2026-02-01T10:00:00Z");
        assert_eq!(plays_in_year(&s, 2026, None), 4);
        assert_eq!(plays_in_year(&s, 2025, None), 0);
        let never = song("b", "B", "X", "Al", 0, "2026-02-01T10:00:00Z");
        assert_eq!(plays_in_year(&never, 2026, None), 1, "at least one play");
        let plugin: PluginPlays = [("a".to_string(), 9)].into();
        assert_eq!(plays_in_year(&s, 2025, Some(&plugin)), 9);
        assert_eq!(plays_in_year(&never, 2026, Some(&plugin)), 0);
    }

    #[test]
    fn years_are_descending_and_include_now() {
        let items = vec![
            song("a", "A", "X", "Al", 1, "2023-02-01T10:00:00Z"),
            song("b", "B", "X", "Al", 1, "2025-02-01T10:00:00Z"),
        ];
        assert_eq!(available_years(&items, at(NOW)), vec![2026, 2025, 2023]);
    }

    #[test]
    fn aggregates_music_video_and_facts() {
        let items = vec![
            song("1", "One", "Daft", "Disc", 10, "2026-03-04T10:00:00Z"),
            song("2", "Two", "Daft", "Disc", 5, "2026-03-05T10:00:00Z"),
            song("3", "Three", "Other", "Rec", 5, "2026-07-05T10:00:00Z"),
            song("old", "Old", "Daft", "Disc", 50, "2024-03-05T10:00:00Z"),
            movie("m", "Matrix", 2, "2026-03-06T10:00:00Z"),
            episode("e1", "Show", 3, "2026-03-07T10:00:00Z"),
            episode("e2", "Show", 1, "2026-03-08T10:00:00Z"),
        ];
        let st = aggregate_year(
            &items,
            2026,
            &AggregateOptions {
                now: at(NOW),
                ..Default::default()
            },
        );
        assert!(!st.is_empty);
        assert_eq!(st.music.plays, 20);
        assert_eq!(st.music.distinct_songs, 3);
        assert_eq!(st.music.minutes, 60); // 20 plays of 3 minutes
        assert_eq!(st.music.artists[0].name, "Daft");
        assert_eq!(st.music.artists[0].plays, 15);
        assert_eq!(st.music.artists[0].target_id.as_deref(), Some("ar-Daft"));
        assert_eq!(st.music.songs[0].name, "One");
        assert_eq!(st.music.songs[0].target_id.as_deref(), Some("al-Disc"));
        assert_eq!(st.music.songs[0].subtitle.as_deref(), Some("Daft"));
        // Ties are broken by name: Three (5) and Two (5).
        assert_eq!(st.music.songs[1].name, "Three");
        assert_eq!(st.music.albums[0].name, "Disc");
        assert_eq!(st.music.albums[0].plays, 15);
        assert_eq!(st.music.genres[0].name, "Rock");
        assert!((st.music.genres[0].share - 1.0).abs() < 1e-9);
        assert_eq!(st.video.movie_plays, 2);
        assert_eq!(st.video.episode_plays, 4);
        assert_eq!(st.video.minutes, 2 * 120 + 4 * 30);
        assert_eq!(st.video.movies[0].subtitle.as_deref(), Some("1999"));
        assert_eq!(st.video.series.len(), 1);
        assert_eq!(st.video.series[0].plays, 4);
        assert_eq!(st.video.series[0].target_id.as_deref(), Some("s-Show"));
        let art = st.video.series[0].art.as_ref().unwrap();
        assert_eq!(art.id, "s-Show");
        assert_eq!(
            art.image_tags.get("Primary").map(String::as_str),
            Some("tag")
        );
        assert_eq!(st.facts.total_minutes, 60 + 360);
        // March holds 10 + 5 + 2 + 3 + 1 plays, July 5.
        assert_eq!(st.facts.busiest_month, Some(2));
        assert!((st.facts.top_artist_share - 0.75).abs() < 1e-9);
        // Up to the 18th of November: 322 days since 1 January (ceil of 321.5).
        assert!((st.facts.minutes_per_day - 420.0 / 322.0).abs() < 1e-9);
    }

    #[test]
    fn past_years_use_the_full_length_and_empty_years_are_empty() {
        let items = vec![song("1", "One", "A", "Al", 1, "2024-03-04T10:00:00Z")];
        let st = aggregate_year(
            &items,
            2024,
            &AggregateOptions {
                now: at(NOW),
                ..Default::default()
            },
        );
        assert!((st.facts.minutes_per_day - 3.0 / 366.0).abs() < 1e-9);
        let empty = aggregate_year(
            &items,
            2025,
            &AggregateOptions {
                now: at(NOW),
                ..Default::default()
            },
        );
        assert!(empty.is_empty);
        assert_eq!(empty.facts.busiest_month, None);
        assert_eq!(empty.facts.top_artist_share, 0.0);
    }

    #[test]
    fn limit_and_plugin_counts_apply() {
        let items = vec![
            song("1", "One", "A", "Al", 1, "2020-01-01T00:00:00Z"),
            song("2", "Two", "B", "Al", 1, "2020-01-01T00:00:00Z"),
            song("3", "Three", "C", "Al", 1, "2020-01-01T00:00:00Z"),
        ];
        let plugin: PluginPlays = [
            ("1".to_string(), 3),
            ("2".to_string(), 7),
            ("3".to_string(), 1),
        ]
        .into();
        let st = aggregate_year(
            &items,
            2026,
            &AggregateOptions {
                plugin_plays: Some(&plugin),
                now: at(NOW),
                limit: Some(2),
            },
        );
        assert_eq!(st.music.songs.len(), 2);
        assert_eq!(st.music.songs[0].name, "Two");
        assert_eq!(st.music.songs[0].plays, 7);
        assert_eq!(st.music.plays, 11);
    }

    #[test]
    fn top_songs_ranks_once_per_song() {
        let items = vec![
            song("1", "Beta", "A", "Al", 3, "2026-01-01T00:00:00Z"),
            song("1", "Beta", "A", "Al", 3, "2026-01-01T00:00:00Z"),
            song("2", "Alpha", "A", "Al", 3, "2026-01-01T00:00:00Z"),
            song("3", "Gamma", "A", "Al", 9, "2026-01-01T00:00:00Z"),
            song("4", "Old", "A", "Al", 9, "2020-01-01T00:00:00Z"),
            movie("m", "Matrix", 5, "2026-01-01T00:00:00Z"),
        ];
        let top = top_songs(&items, 2026, None, 50);
        let names: Vec<_> = top.iter().map(|s| s.name()).collect();
        assert_eq!(names, ["Gamma", "Alpha", "Beta"]);
        assert_eq!(top_songs(&items, 2026, None, 2).len(), 2);
    }

    #[test]
    fn counts_plugin_rows() {
        let rows = json!([{"item_id": "a"}, {"ItemId": "a"}, {"itemId": "b"}, {"x": 1}, 5, {"item_id": ""}]);
        let c = count_plugin_rows(&rows).unwrap();
        assert_eq!(c["a"], 2);
        assert_eq!(c["b"], 1);
        assert!(count_plugin_rows(&json!([{"x": 1}])).is_none());
        assert!(count_plugin_rows(&json!({"a": 1})).is_none());
    }
}
