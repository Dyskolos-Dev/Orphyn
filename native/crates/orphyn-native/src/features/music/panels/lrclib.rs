//! Lyrics model, LRC parsing and the online fallback: LRCLIB (https://lrclib.net), a free, open
//! lyrics database (port of `jellyfin/lyrics/lrclib.ts` and `lyrics/lines.ts`). Synced lyrics (LRC)
//! are preferred over plain text. Results, misses included (for a day), are cached in memory and in
//! local storage (same key and shape as the React app) so a track is looked up once.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use orphyn_core::storage::LocalStorage;
use serde_json::{json, Map, Value};

pub const LRCLIB_URL: &str = "https://lrclib.net/api";
pub const LRCLIB_SOURCE: &str = "LRCLIB";
pub const LRCLIB_CACHE_KEY: &str = "orphyn.lyrics.lrclib";
pub const MISS_TTL_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_STORED: usize = 150;
/// LRCLIB asks for matches within a couple of seconds of the real duration.
const DURATION_TOLERANCE_S: f64 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct LyricLine {
    pub text: String,
    /// `None` for unsynced lyrics.
    pub start_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<LyricLine>,
    /// Where the lyrics come from when not the server (`"LRCLIB"`).
    pub source: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
}

// ---------------------------------------------------------------------------------- parsing

/// Parses one `[mm:ss.xx]` stamp at the start of `s`: `(milliseconds, rest)`.
fn stamp(s: &str) -> Option<(i64, &str)> {
    let rest = s.strip_prefix('[')?;
    let end = rest.find(']')?;
    let (body, after) = (&rest[..end], &rest[end + 1..]);
    let (min, tail) = body.split_once(':')?;
    if min.is_empty() || min.len() > 3 || !min.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (sec, frac) = match tail.find(['.', ':']) {
        Some(i) => (&tail[..i], Some(&tail[i + 1..])),
        None => (tail, None),
    };
    if sec.len() != 2 || !sec.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let frac_ms = match frac {
        None => 0,
        Some(f) if (1..=3).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{f}00")[..3].parse::<i64>().ok()?
        }
        Some(_) => return None,
    };
    Some((
        min.parse::<i64>().ok()? * 60_000 + sec.parse::<i64>().ok()? * 1000 + frac_ms,
        after,
    ))
}

/// Parses LRC text. `None` when it holds no timestamped line. Several stamps share a line.
pub fn parse_lrc(text: &str) -> Option<Lyrics> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut stamps = Vec::new();
        let mut rest = raw;
        // Stamps may also sit mid-line (enhanced LRC): scan the whole line for them.
        let mut content = String::new();
        while !rest.is_empty() {
            if let Some((ms, after)) = stamp(rest) {
                stamps.push(ms);
                rest = after;
            } else {
                let ch = rest.chars().next().expect("not empty");
                content.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
        let content = content.trim().to_string();
        for ms in stamps {
            lines.push(LyricLine {
                text: content.clone(),
                start_ms: Some(ms),
            });
        }
    }
    if lines.is_empty() {
        return None;
    }
    lines.sort_by_key(|l| l.start_ms);
    Some(Lyrics {
        synced: true,
        lines,
        source: None,
    })
}

pub fn parse_plain(text: &str) -> Option<Lyrics> {
    let mut lines: Vec<LyricLine> = text
        .lines()
        .map(|t| LyricLine {
            text: t.trim_end().to_string(),
            start_ms: None,
        })
        .collect();
    // Blank lines inside the text are stanza breaks; drop them at both ends.
    while lines.first().is_some_and(|l| l.text.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    (!lines.is_empty()).then_some(Lyrics {
        synced: false,
        lines,
        source: None,
    })
}

/// Synced first, plain otherwise; `None` for instrumentals or empty records.
pub fn lyrics_from_record(r: &Value) -> Option<Lyrics> {
    if r.get("instrumental").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let s = |k: &str| r.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
    let parsed = s("syncedLyrics")
        .and_then(parse_lrc)
        .or_else(|| s("plainLyrics").and_then(parse_plain))?;
    Some(Lyrics {
        source: Some(LRCLIB_SOURCE),
        ..parsed
    })
}

/// Best search hit: synced beats plain, then the closest duration within tolerance.
pub fn pick_best(records: &[Value], q: &TrackQuery) -> Option<Lyrics> {
    let want = q.duration.unwrap_or(0.0);
    let gap = |r: &Value| (r.get("duration").and_then(Value::as_f64).unwrap_or(want) - want).abs();
    let synced = |r: &Value| {
        r.get("syncedLyrics")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
    };
    let mut usable: Vec<&Value> = records
        .iter()
        .filter(|r| {
            lyrics_from_record(r).is_some() && (q.duration.is_none() || gap(r) <= DURATION_TOLERANCE_S)
        })
        .collect();
    usable.sort_by(|a, b| {
        synced(b)
            .cmp(&synced(a))
            .then(gap(a).partial_cmp(&gap(b)).unwrap_or(std::cmp::Ordering::Equal))
    });
    usable.first().and_then(|r| lyrics_from_record(r))
}

/// Index of the lyric line being sung at `time_ms`: the last line whose start is <= `time_ms`, or
/// -1 before the first line. Unsynced lines are never "current" (`findLineIndex`).
pub fn find_line_index(lines: &[LyricLine], time_ms: f64) -> isize {
    let (mut lo, mut hi) = (0isize, lines.len() as isize - 1);
    let mut found = -1;
    while lo <= hi {
        let mid = (lo + hi) >> 1;
        let Some(start) = lines[mid as usize].start_ms else {
            return -1;
        };
        if start as f64 <= time_ms {
            found = mid;
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    found
}

// ---------------------------------------------------------------------------------- Jellyfin

/// The server's `/Audio/{id}/Lyrics` answer (start in ticks of 100 ns) as lyrics.
pub fn from_server(dto: &Value) -> Option<Lyrics> {
    let raw = dto.get("Lyrics")?.as_array()?;
    if raw.is_empty() {
        return None;
    }
    let lines: Vec<LyricLine> = raw
        .iter()
        .map(|l| LyricLine {
            text: l
                .get("Text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            start_ms: l
                .get("Start")
                .and_then(Value::as_f64)
                .map(|t| (t / 10_000.0).round() as i64),
        })
        .collect();
    Some(Lyrics {
        synced: lines.iter().all(|l| l.start_ms.is_some()),
        lines,
        source: None,
    })
}

// ---------------------------------------------------------------------------------- cache

pub fn cache_key(q: &TrackQuery) -> String {
    let n = |s: &str| s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{}|{}|{}|{}",
        n(&q.artist),
        n(&q.title),
        n(q.album.as_deref().unwrap_or("")),
        q.duration.unwrap_or(0.0).round() as i64
    )
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// Lyrics, or `None` for a miss.
    v: Option<Lyrics>,
    /// Stored at (ms).
    t: i64,
}

#[derive(Default)]
pub struct LrclibCache {
    entries: HashMap<String, Entry>,
}

fn lyrics_to_json(l: &Lyrics) -> Value {
    json!({
        "synced": l.synced,
        "lines": l.lines.iter().map(|x| json!({"text": x.text, "startMs": x.start_ms})).collect::<Vec<_>>(),
        "source": l.source.unwrap_or(LRCLIB_SOURCE),
    })
}

fn lyrics_from_json(v: &Value) -> Option<Lyrics> {
    let lines = v
        .get("lines")?
        .as_array()?
        .iter()
        .map(|l| {
            Some(LyricLine {
                text: l.get("text")?.as_str()?.to_string(),
                start_ms: l.get("startMs").and_then(Value::as_i64),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Lyrics {
        synced: v.get("synced").and_then(Value::as_bool).unwrap_or(false),
        lines,
        source: Some(LRCLIB_SOURCE),
    })
}

impl LrclibCache {
    /// Reads the stored JSON (corrupt storage starts empty).
    pub fn from_json(raw: &str) -> Self {
        let mut c = Self::default();
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(raw) else {
            return c;
        };
        for (k, e) in map {
            let Some(t) = e.get("t").and_then(Value::as_i64) else {
                continue;
            };
            let v = match e.get("v") {
                Some(Value::Null) | None => None,
                Some(v) => match lyrics_from_json(v) {
                    Some(l) => Some(l),
                    None => continue,
                },
            };
            c.entries.insert(k, Entry { v, t });
        }
        c
    }

    /// The most recent entries only, so storage stays small.
    pub fn to_json(&self) -> String {
        let mut all: Vec<(&String, &Entry)> = self.entries.iter().collect();
        all.sort_by_key(|(_, e)| e.t);
        let skip = all.len().saturating_sub(MAX_STORED);
        let mut out = Map::new();
        for (k, e) in all.into_iter().skip(skip) {
            out.insert(
                k.clone(),
                json!({"v": e.v.as_ref().map(lyrics_to_json), "t": e.t}),
            );
        }
        Value::Object(out).to_string()
    }

    /// `None` = unknown (go online), `Some(None)` = known miss, `Some(Some(l))` = the lyrics.
    pub fn read(&self, q: &TrackQuery, now_ms: i64) -> Option<Option<Lyrics>> {
        let e = self.entries.get(&cache_key(q))?;
        if e.v.is_none() && now_ms - e.t > MISS_TTL_MS {
            return None;
        }
        Some(e.v.clone())
    }

    pub fn write(&mut self, q: &TrackQuery, v: Option<Lyrics>, now_ms: i64) {
        self.entries.insert(cache_key(q), Entry { v, t: now_ms });
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn memory(storage: &LocalStorage) -> &'static Mutex<LrclibCache> {
    static CACHE: OnceLock<Mutex<LrclibCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(
            storage
                .get(LRCLIB_CACHE_KEY)
                .map(|raw| LrclibCache::from_json(&raw))
                .unwrap_or_default(),
        )
    })
}

/// Cached lookup, no network (`readCache`).
pub fn read_cache(storage: &LocalStorage, q: &TrackQuery) -> Option<Option<Lyrics>> {
    memory(storage)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .read(q, now_ms())
}

fn encode(s: &str) -> String {
    orphyn_jellyfin::client::encode_component(s)
}

fn query_string(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Looks lyrics up for a track (blocking: run it on a blocking thread). `Ok(None)` when LRCLIB
/// has none. Network failures are errors and are not cached, so a flaky connection is retried.
pub fn fetch_lrclib(storage: &LocalStorage, q: &TrackQuery) -> Result<Option<Lyrics>, String> {
    use orphyn_player::music::http::get_text;
    if q.title.trim().is_empty() || q.artist.trim().is_empty() {
        return Ok(None);
    }
    if let Some(hit) = read_cache(storage, q) {
        return Ok(hit);
    }
    let timeout = Duration::from_secs(15);
    let mut exact = vec![
        ("artist_name", q.artist.clone()),
        ("track_name", q.title.clone()),
    ];
    if let Some(a) = &q.album {
        exact.push(("album_name", a.clone()));
    }
    if let Some(d) = q.duration.filter(|d| *d > 0.0) {
        exact.push(("duration", (d.round() as i64).to_string()));
    }
    let get_json = |url: String| -> Result<Option<Value>, String> {
        get_text(&url, timeout)?
            .map(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()))
            .transpose()
    };
    let mut found = get_json(format!("{LRCLIB_URL}/get?{}", query_string(&exact)))?
        .and_then(|r| lyrics_from_record(&r));
    if found.is_none() {
        let search = [
            ("artist_name", q.artist.clone()),
            ("track_name", q.title.clone()),
        ];
        let hits = get_json(format!("{LRCLIB_URL}/search?{}", query_string(&search)))?;
        let list = hits
            .as_ref()
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        found = pick_best(&list, q);
    }
    let mut cache = memory(storage).lock().unwrap_or_else(|p| p.into_inner());
    cache.write(q, found.clone(), now_ms());
    storage.set(LRCLIB_CACHE_KEY, &cache.to_json());
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(l: &Lyrics) -> Vec<i64> {
        l.lines.iter().map(|x| x.start_ms.unwrap()).collect()
    }

    #[test]
    fn lrc_parses_stamps_sorts_and_shares_text() {
        let l = parse_lrc("[ar:Someone]\n[00:12.50]Second\n[00:01.5][01:02.003]Shared\r\n[00:05]Plain stamp\n[00:07:25]Colon").unwrap();
        assert!(l.synced);
        assert_eq!(starts(&l), [1500, 5000, 7250, 12500, 62003]);
        let texts: Vec<&str> = l.lines.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["Shared", "Plain stamp", "Colon", "Second", "Shared"]);
    }

    #[test]
    fn lrc_fraction_is_milliseconds_whatever_its_width() {
        assert_eq!(starts(&parse_lrc("[00:01.5]a").unwrap()), [1500]);
        assert_eq!(starts(&parse_lrc("[00:01.05]a").unwrap()), [1050]);
        assert_eq!(starts(&parse_lrc("[00:01.123]a").unwrap()), [1123]);
        assert_eq!(starts(&parse_lrc("[100:00.00]a").unwrap()), [6_000_000]);
    }

    #[test]
    fn lrc_without_stamps_is_not_synced() {
        assert!(parse_lrc("just words\nmore words").is_none());
        assert!(parse_lrc("[ti:Title]\n[by:me]").is_none());
        // A blank text line keeps its stamp (an instrumental gap).
        let l = parse_lrc("[00:01.00]\n[00:02.00]x").unwrap();
        assert_eq!(l.lines[0].text, "");
    }

    #[test]
    fn plain_trims_the_blank_ends_only() {
        let l = parse_plain("\n\nA  \n\nB\n\n").unwrap();
        assert!(!l.synced);
        let t: Vec<&str> = l.lines.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(t, ["A", "", "B"]);
        assert!(l.lines.iter().all(|x| x.start_ms.is_none()));
        assert!(parse_plain(" \n\n").is_none() || parse_plain("\n\n").is_none());
    }

    #[test]
    fn record_prefers_synced_and_skips_instrumentals() {
        let r = json!({"syncedLyrics": "[00:01.00]a", "plainLyrics": "a"});
        let l = lyrics_from_record(&r).unwrap();
        assert!(l.synced);
        assert_eq!(l.source, Some("LRCLIB"));
        let r = json!({"syncedLyrics": null, "plainLyrics": "a\nb"});
        assert!(!lyrics_from_record(&r).unwrap().synced);
        assert!(lyrics_from_record(&json!({"instrumental": true, "plainLyrics": "a"})).is_none());
        assert!(lyrics_from_record(&json!({})).is_none());
    }

    #[test]
    fn best_hit_is_synced_then_closest_within_tolerance() {
        let q = TrackQuery {
            title: "t".into(),
            artist: "a".into(),
            album: None,
            duration: Some(200.0),
        };
        let recs = vec![
            json!({"duration": 200, "plainLyrics": "plain"}),
            json!({"duration": 203.5, "syncedLyrics": "[00:01.00]far"}),
            json!({"duration": 202, "syncedLyrics": "[00:01.00]near"}),
            json!({"duration": 199, "syncedLyrics": "[00:01.00]nearer"}),
        ];
        let best = pick_best(&recs, &q).unwrap();
        assert_eq!(best.lines[0].text, "nearer");
        assert!(pick_best(&recs[..2], &q).unwrap().lines[0].text == "plain");
        let far = vec![json!({"duration": 260, "syncedLyrics": "[00:01.00]x"})];
        assert!(pick_best(&far, &q).is_none());
        // No wanted duration: any usable record will do.
        let q2 = TrackQuery { duration: None, ..q };
        assert!(pick_best(&far, &q2).is_some());
    }

    #[test]
    fn line_index_follows_the_playback_time() {
        let l = parse_lrc("[00:01.00]a\n[00:02.00]b\n[00:04.00]c").unwrap();
        assert_eq!(find_line_index(&l.lines, 500.0), -1);
        assert_eq!(find_line_index(&l.lines, 1000.0), 0);
        assert_eq!(find_line_index(&l.lines, 3999.0), 1);
        assert_eq!(find_line_index(&l.lines, 9e6), 2);
        let plain = parse_plain("x\ny").unwrap();
        assert_eq!(find_line_index(&plain.lines, 5000.0), -1);
        assert_eq!(find_line_index(&[], 5.0), -1);
    }

    #[test]
    fn server_lyrics_convert_ticks_to_ms() {
        let dto = json!({"Lyrics": [{"Text": "a", "Start": 15_000_000}, {"Text": "b", "Start": 30_000_000}]});
        let l = from_server(&dto).unwrap();
        assert!(l.synced);
        assert_eq!(starts(&l), [1500, 3000]);
        let plain = json!({"Lyrics": [{"Text": "a"}, {"Text": "b", "Start": 1}]});
        assert!(!from_server(&plain).unwrap().synced);
        assert!(from_server(&json!({"Lyrics": []})).is_none());
        assert!(from_server(&Value::Null).is_none());
    }

    #[test]
    fn cache_keys_are_normalised_and_misses_expire() {
        let q = TrackQuery {
            title: "  Hello   World ".into(),
            artist: "ARTIST".into(),
            album: None,
            duration: Some(200.4),
        };
        assert_eq!(cache_key(&q), "artist|hello world||200");
        let mut c = LrclibCache::default();
        assert_eq!(c.read(&q, 0), None);
        c.write(&q, None, 1000);
        assert_eq!(c.read(&q, 1000 + MISS_TTL_MS), Some(None));
        assert_eq!(c.read(&q, 1001 + MISS_TTL_MS), None);
        let l = parse_lrc("[00:01.00]a").unwrap();
        let l = Lyrics {
            source: Some(LRCLIB_SOURCE),
            ..l
        };
        c.write(&q, Some(l.clone()), 5);
        // Hits never expire; the storage round trip keeps them.
        assert_eq!(c.read(&q, 5 + 10 * MISS_TTL_MS), Some(Some(l.clone())));
        let back = LrclibCache::from_json(&c.to_json());
        assert_eq!(back.read(&q, 6), Some(Some(l)));
        assert!(LrclibCache::from_json("not json").entries.is_empty());
    }
}
