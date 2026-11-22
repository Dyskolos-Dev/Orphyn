//! The watchlist ("À regarder"): films and series the person marked to watch later. It lives in
//! the server's DisplayPreferences (client `orphyn`, id `watchlist`, one `CustomPrefs` entry), so
//! it follows the person across devices without touching favourites or any item data.
//!
//! This file holds the storage format, the new-episode detection (both pure, unit tested) and
//! the three requests.
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::{JfResult, Q};
use crate::endpoints::Session;
use crate::types::{BaseItemDto, QueryResult};

pub const PREFS_ID: &str = "watchlist";
pub const PREFS_CLIENT: &str = "orphyn";
/// Key of the `CustomPrefs` entry that holds the JSON list.
pub const PREFS_KEY: &str = "orphynWatchlist";
const FORMAT_VERSION: u32 = 1;

/// One marked title: its id, `Movie` or `Series`, and when it was added (epoch milliseconds).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchEntry {
    pub id: String,
    #[serde(rename = "t")]
    pub kind: String,
    #[serde(rename = "a")]
    pub added_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watchlist {
    #[serde(rename = "v", default)]
    pub version: u32,
    #[serde(default)]
    pub items: Vec<WatchEntry>,
}

impl Watchlist {
    pub fn contains(&self, id: &str) -> bool {
        self.items.iter().any(|e| e.id == id)
    }

    /// Adds a title (newest last); a title already there keeps its date.
    pub fn add(&mut self, id: &str, kind: &str, now_ms: i64) {
        if !self.contains(id) {
            self.items.push(WatchEntry {
                id: id.to_string(),
                kind: kind.to_string(),
                added_ms: now_ms,
            });
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.items.retain(|e| e.id != id);
    }

    pub fn entry(&self, id: &str) -> Option<&WatchEntry> {
        self.items.iter().find(|e| e.id == id)
    }

    /// The `CustomPrefs` string: `{"v":1,"items":[{"id":..,"t":"Movie","a":ms}]}`.
    pub fn encode(&self) -> String {
        let mut v = self.clone();
        v.version = FORMAT_VERSION;
        serde_json::to_string(&v).unwrap_or_else(|_| "{}".into())
    }

    /// Reads the `CustomPrefs` string; anything unusable is an empty list. Entries without an id,
    /// of a kind other than `Movie` / `Series`, or repeated, are dropped.
    pub fn decode(raw: &str) -> Self {
        let mut list: Watchlist = serde_json::from_str(raw).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        list.items.retain(|e| {
            !e.id.is_empty()
                && (e.kind == "Movie" || e.kind == "Series")
                && seen.insert(e.id.clone())
        });
        list.version = FORMAT_VERSION;
        list
    }
}

/// An episode as far as the detection needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpisodeInfo {
    pub id: String,
    pub series_id: String,
    pub series_name: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub created_ms: i64,
    pub played: bool,
}

impl EpisodeInfo {
    pub fn from_item(i: &BaseItemDto) -> Option<Self> {
        Some(Self {
            id: i.id.clone(),
            series_id: i.series_id.clone()?,
            series_name: i.series_name.clone().unwrap_or_default(),
            season: i.parent_index_number,
            episode: i.index_number,
            created_ms: parse_iso_ms(i.date_created.as_deref()?)?,
            played: i.user_data.as_ref().is_some_and(|u| u.played),
        })
    }
}

/// Unplayed episodes of watchlisted series added after `seen_ms` (and after the series was added
/// to the list), newest first.
pub fn new_episodes(list: &Watchlist, episodes: &[EpisodeInfo], seen_ms: i64) -> Vec<EpisodeInfo> {
    let mut out: Vec<EpisodeInfo> = episodes
        .iter()
        .filter(|e| !e.played)
        .filter(|e| {
            list.entry(&e.series_id)
                .is_some_and(|w| w.kind == "Series" && e.created_ms > seen_ms.max(w.added_ms))
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms).then(a.id.cmp(&b.id)));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// `2024-05-01T12:30:00.1234567Z` (also with an offset) to epoch milliseconds.
pub fn parse_iso_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut frac_ms = 0;
    if let Some(r) = rest.strip_prefix('.') {
        let digits: String = r.chars().take_while(char::is_ascii_digit).collect();
        frac_ms = format!("{digits:0<3}")[..3].parse::<i64>().ok()?;
        rest = &r[digits.len()..];
    }
    let mut offset_min = 0;
    if rest.len() >= 6 && (rest.starts_with('+') || rest.starts_with('-')) {
        let sign = if rest.starts_with('-') { -1 } else { 1 };
        let oh = rest.get(1..3)?.parse::<i64>().ok()?;
        let om = rest.get(4..6)?.parse::<i64>().ok()?;
        offset_min = sign * (oh * 60 + om);
    }
    // Days since 1970-01-01 (civil calendar).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hh * 3600 + mm * 60 + ss - offset_min * 60) * 1000 + frac_ms)
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Session {
    /// The watchlist stored on the server (empty when never written).
    pub async fn watchlist_fetch(&self) -> JfResult<Watchlist> {
        let v: serde_json::Value = self
            .client
            .get(
                &format!("/DisplayPreferences/{PREFS_ID}"),
                &[
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    ("client", Some(Q::from(PREFS_CLIENT))),
                ],
            )
            .await?;
        Ok(v.get("CustomPrefs")
            .and_then(|c| c.get(PREFS_KEY))
            .and_then(|d| d.as_str())
            .map(Watchlist::decode)
            .unwrap_or_default())
    }

    /// Writes the watchlist to the server.
    pub async fn watchlist_store(&self, list: &Watchlist) -> JfResult<()> {
        let body = serde_json::json!({
            "Id": PREFS_ID,
            "Client": PREFS_CLIENT,
            "SortBy": "SortName",
            "SortOrder": "Ascending",
            "CustomPrefs": { PREFS_KEY: list.encode() },
        });
        let _: serde_json::Value = self
            .client
            .request(
                Method::POST,
                &format!("/DisplayPreferences/{PREFS_ID}"),
                &[
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    ("client", Some(Q::from(PREFS_CLIENT))),
                ],
                Some(body),
            )
            .await?;
        Ok(())
    }

    /// The items with these ids (films, series, episodes), in the server's order.
    pub async fn items_by_ids(&self, ids: &[String]) -> JfResult<Vec<BaseItemDto>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let fields: Vec<String> = crate::endpoints::DEFAULT_FIELDS
            .iter()
            .map(|s| s.to_string())
            .collect();
        let r: QueryResult = self
            .client
            .get(
                &format!("/Users/{}/Items", self.user_id),
                &[
                    ("ids", Some(Q::List(ids.to_vec()))),
                    ("fields", Some(Q::List(fields))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// The `limit` most recently added unplayed episodes of the library, for the new-episode check.
    pub async fn recent_unplayed_episodes(&self, limit: u32) -> JfResult<Vec<EpisodeInfo>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Users/{}/Items", self.user_id),
                &[
                    ("includeItemTypes", Some(Q::from("Episode"))),
                    ("recursive", Some(Q::from(true))),
                    ("isPlayed", Some(Q::from(false))),
                    ("sortBy", Some(Q::from("DateCreated"))),
                    ("sortOrder", Some(Q::from("Descending"))),
                    ("limit", Some(Q::from(limit))),
                    ("fields", Some(Q::from("DateCreated,SeriesName"))),
                ],
            )
            .await?;
        Ok(r.items.iter().filter_map(EpisodeInfo::from_item).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: &str, series: &str, created: i64, played: bool) -> EpisodeInfo {
        EpisodeInfo {
            id: id.into(),
            series_id: series.into(),
            series_name: "S".into(),
            season: Some(2),
            episode: Some(5),
            created_ms: created,
            played,
        }
    }

    #[test]
    fn storage_format_roundtrips() {
        let mut l = Watchlist::default();
        l.add("a", "Movie", 10);
        l.add("b", "Series", 20);
        l.add("a", "Movie", 99);
        assert_eq!(l.items.len(), 2);
        assert_eq!(l.entry("a").unwrap().added_ms, 10);
        let raw = l.encode();
        assert_eq!(
            raw,
            r#"{"v":1,"items":[{"id":"a","t":"Movie","a":10},{"id":"b","t":"Series","a":20}]}"#
        );
        assert_eq!(
            Watchlist::decode(&raw),
            Watchlist {
                version: 1,
                ..l.clone()
            }
        );
        l.remove("a");
        assert!(!l.contains("a") && l.contains("b"));
    }

    #[test]
    fn decode_survives_garbage() {
        assert!(Watchlist::decode("nope").items.is_empty());
        let l = Watchlist::decode(
            r#"{"items":[{"id":"","t":"Movie","a":1},{"id":"x","t":"Episode","a":1},{"id":"y","t":"Movie","a":1},{"id":"y","t":"Movie","a":2}]}"#,
        );
        assert_eq!(l.items.len(), 1);
        assert_eq!(l.items[0].id, "y");
    }

    #[test]
    fn iso_dates_parse() {
        assert_eq!(parse_iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_iso_ms("2000-03-01T00:00:01.5000000Z"),
            Some(951_868_801_500)
        );
        assert_eq!(
            parse_iso_ms("2000-03-01T02:00:00+02:00"),
            parse_iso_ms("2000-03-01T00:00:00Z")
        );
        assert_eq!(parse_iso_ms("garbage"), None);
    }

    #[test]
    fn detects_new_unplayed_episodes_of_listed_series() {
        let mut l = Watchlist::default();
        l.add("s1", "Series", 100);
        l.add("m1", "Movie", 100);
        let eps = vec![
            ep("old", "s1", 50, false),
            ep("new1", "s1", 300, false),
            ep("new2", "s1", 400, false),
            ep("seen", "s1", 150, false),
            ep("played", "s1", 500, true),
            ep("other", "s2", 600, false),
        ];
        let out = new_episodes(&l, &eps, 200);
        assert_eq!(
            out.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            ["new2", "new1"]
        );
        assert!(new_episodes(&l, &eps, 1000).is_empty());
    }
}
