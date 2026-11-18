//! Pure rules of the player, ported 1:1 from the React app: `skip.ts`, `upNext.ts`, `tracks.ts`,
//! `segmentAt` (`jellyfin/segments.ts`), `formatClock` / `nextEpisode` (`format.ts`).
use orphyn_jellyfin::types::BaseItemDto;

use super::playback::{AudioTrack, MediaSegment, PlaybackTracks, SegmentType, SubtitleTrack};

/// `formatClock`: `m:ss` or `h:mm:ss`, never negative.
pub fn format_clock(seconds: f64) -> String {
    let s = if seconds.is_finite() {
        seconds.max(0.0).floor() as u64
    } else {
        0
    };
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// `sortEpisodes`: by season (specials last), episode number, then name.
pub fn sort_episodes(episodes: &[BaseItemDto]) -> Vec<BaseItemDto> {
    let season = |e: &BaseItemDto| match e.parent_index_number.unwrap_or(0) {
        0 => i64::MAX,
        s => s as i64,
    };
    let num = |e: &BaseItemDto| e.index_number.map(i64::from).unwrap_or(i64::MAX);
    let name = |e: &BaseItemDto| {
        e.extra
            .get("SortName")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| e.name.clone())
            .unwrap_or_default()
    };
    let mut v = episodes.to_vec();
    v.sort_by(|a, b| {
        season(a)
            .cmp(&season(b))
            .then(num(a).cmp(&num(b)))
            .then_with(|| name(a).cmp(&name(b)))
    });
    v
}

/// `nextEpisode`: the episode after `current_id` in series order.
pub fn next_episode(episodes: &[BaseItemDto], current_id: &str) -> Option<BaseItemDto> {
    let sorted = sort_episodes(episodes);
    let i = sorted.iter().position(|e| e.id == current_id)?;
    sorted.get(i + 1).cloned()
}

/// `segmentAt`: the first segment of `types` containing `seconds` (end minus `tail` excluded).
pub fn segment_at<'a>(
    segments: &'a [MediaSegment],
    seconds: f64,
    types: &[SegmentType],
    tail: f64,
) -> Option<&'a MediaSegment> {
    segments
        .iter()
        .find(|s| types.contains(&s.kind) && seconds >= s.start && seconds < s.end - tail)
}

/// `SkipKind` of `skip.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipKind {
    Intro,
    Recap,
    Credits,
    NextEpisode,
}

impl SkipKind {
    pub fn label_key(self) -> &'static str {
        match self {
            SkipKind::Intro => "video:skip.intro",
            SkipKind::Recap => "video:skip.recap",
            SkipKind::Credits => "video:skip.credits",
            SkipKind::NextEpisode => "video:skip.nextEpisode",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkipPill {
    pub kind: SkipKind,
    pub segment: MediaSegment,
}

/// `SKIP_TAIL_SECONDS`.
pub const SKIP_TAIL_SECONDS: f64 = 1.0;

/// `skipPill`: Intro / Recap → skip them; Outro → "Next Episode" when one follows (until the Up
/// Next card takes over), else "Skip Credits". Preview and Commercial never show a pill.
pub fn skip_pill(
    segments: &[MediaSegment],
    position: f64,
    has_next: bool,
    up_next_visible: bool,
    trailer: bool,
) -> Option<SkipPill> {
    if trailer {
        return None;
    }
    let segment = segment_at(
        segments,
        position,
        &[SegmentType::Intro, SegmentType::Recap, SegmentType::Outro],
        SKIP_TAIL_SECONDS,
    )?
    .clone();
    let kind = match segment.kind {
        SegmentType::Intro => SkipKind::Intro,
        SegmentType::Recap => SkipKind::Recap,
        _ if has_next => {
            if up_next_visible {
                return None;
            }
            SkipKind::NextEpisode
        }
        _ => SkipKind::Credits,
    };
    Some(SkipPill { kind, segment })
}

/// `skipTarget`: where skipping a segment seeks to (never past the end).
pub fn skip_target(segment: &MediaSegment, duration: f64) -> f64 {
    if duration > 0.0 {
        segment.end.min(duration)
    } else {
        segment.end
    }
}

/// `isAutoSkippable`: intros and recaps only.
pub fn is_auto_skippable(p: &SkipPill) -> bool {
    matches!(p.kind, SkipKind::Intro | SkipKind::Recap)
}

/// `segmentKey`: to skip a segment automatically only once.
pub fn segment_key(item_id: &str, s: &MediaSegment) -> String {
    format!("{item_id}:{}:{}", s.kind.as_str(), s.start)
}

/// `UP_NEXT_WINDOW_SECONDS`.
pub const UP_NEXT_WINDOW_SECONDS: f64 = 15.0;

/// `upNextState`: (visible, whole seconds left).
pub fn up_next_state(position: f64, duration: f64, has_next: bool, dismissed: bool) -> (bool, i64) {
    if !has_next || dismissed || duration.is_nan() || duration <= UP_NEXT_WINDOW_SECONDS {
        return (false, 0);
    }
    let remaining = duration - position;
    let visible = remaining <= UP_NEXT_WINDOW_SECONDS && remaining > 0.0;
    (visible, if visible { remaining.ceil() as i64 } else { 0 })
}

fn norm_lang(l: Option<&str>) -> Option<String> {
    let l = l?.to_lowercase();
    let k = l.split(['-', '_']).next().unwrap_or("").to_string();
    if k.is_empty() {
        return None;
    }
    let alias = match k.as_str() {
        "fr" | "fre" | "fra" => "fra",
        "en" | "eng" => "eng",
        "de" | "ger" | "deu" => "deu",
        "es" | "spa" => "spa",
        "it" | "ita" => "ita",
        "pt" | "por" => "por",
        "ja" | "jpn" => "jpn",
        "ko" | "kor" => "kor",
        "zh" | "chi" | "zho" => "zho",
        "ru" | "rus" => "rus",
        "nl" | "dut" | "nld" => "nld",
        _ => return Some(k),
    };
    Some(alias.to_string())
}

/// `sameLanguage`.
pub fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((norm_lang(a), norm_lang(b)), (Some(x), Some(y)) if x == y)
}

/// `pickInitialAudio`: preferred language, else the server default, else the default flag,
/// else the first.
pub fn pick_initial_audio<'a>(t: &'a PlaybackTracks, pref: Option<&str>) -> Option<&'a AudioTrack> {
    pref.and_then(|p| {
        t.audio
            .iter()
            .find(|a| same_language(a.language.as_deref(), Some(p)))
    })
    .or_else(|| {
        t.audio
            .iter()
            .find(|a| Some(a.index) == t.default_audio_index)
    })
    .or_else(|| t.audio.iter().find(|a| a.is_default))
    .or_else(|| t.audio.first())
}

/// `pickInitialSubtitle`: a preferred language wins (non-forced first); otherwise only the
/// server's default selection; `None` = off.
pub fn pick_initial_subtitle<'a>(
    t: &'a PlaybackTracks,
    pref: Option<&str>,
) -> Option<&'a SubtitleTrack> {
    if let Some(p) = pref {
        let matches: Vec<&SubtitleTrack> = t
            .subtitles
            .iter()
            .filter(|s| same_language(s.language.as_deref(), Some(p)))
            .collect();
        if let Some(pick) = matches.iter().find(|s| !s.is_forced).or(matches.first()) {
            return Some(pick);
        }
    }
    t.subtitles
        .iter()
        .find(|s| Some(s.index) == t.default_subtitle_index)
}

/// `PLAYBACK_RATES` of `PlayerControls.tsx`.
pub const PLAYBACK_RATES: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// `{rate}×` as JavaScript prints numbers (`1×`, `1.25×`).
pub fn rate_label(r: f64) -> String {
    format!("{r}×")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(kind: SegmentType, start: f64, end: f64) -> MediaSegment {
        MediaSegment { kind, start, end }
    }

    #[test]
    fn clock_matches_format_clock() {
        assert_eq!(format_clock(0.0), "0:00");
        assert_eq!(format_clock(65.9), "1:05");
        assert_eq!(format_clock(3725.0), "1:02:05");
        assert_eq!(format_clock(-3.0), "0:00");
        assert_eq!(format_clock(f64::NAN), "0:00");
    }

    #[test]
    fn pills_follow_skip_ts() {
        let s = vec![
            seg(SegmentType::Intro, 10.0, 70.0),
            seg(SegmentType::Outro, 1300.0, 1400.0),
            seg(SegmentType::Preview, 1400.0, 1420.0),
        ];
        assert_eq!(skip_pill(&s, 9.0, true, false, false), None);
        assert_eq!(
            skip_pill(&s, 10.0, true, false, false).map(|p| p.kind),
            Some(SkipKind::Intro)
        );
        // Within the last second of a segment the pill is gone.
        assert_eq!(skip_pill(&s, 69.5, true, false, false), None);
        assert_eq!(
            skip_pill(&s, 1350.0, true, false, false).map(|p| p.kind),
            Some(SkipKind::NextEpisode)
        );
        assert_eq!(skip_pill(&s, 1350.0, true, true, false), None);
        assert_eq!(
            skip_pill(&s, 1350.0, false, false, false).map(|p| p.kind),
            Some(SkipKind::Credits)
        );
        assert_eq!(skip_pill(&s, 1410.0, false, false, false), None);
        assert_eq!(skip_pill(&s, 20.0, true, false, true), None);
        assert_eq!(skip_target(&s[1], 1380.0), 1380.0);
        assert_eq!(skip_target(&s[0], 0.0), 70.0);
        assert_eq!(segment_key("i", &s[0]), "i:Intro:10");
    }

    #[test]
    fn up_next_shows_in_the_last_15_seconds() {
        assert_eq!(up_next_state(1000.0, 1200.0, true, false), (false, 0));
        assert_eq!(up_next_state(1186.2, 1200.0, true, false), (true, 14));
        assert_eq!(up_next_state(1186.2, 1200.0, true, true), (false, 0));
        assert_eq!(up_next_state(1186.2, 1200.0, false, false), (false, 0));
        assert_eq!(up_next_state(5.0, 10.0, true, false), (false, 0));
        assert_eq!(up_next_state(1200.0, 1200.0, true, false), (false, 0));
    }

    fn sub(index: i64, lang: &str, forced: bool) -> SubtitleTrack {
        SubtitleTrack {
            index,
            language: Some(lang.into()),
            codec: None,
            title: lang.into(),
            is_default: false,
            is_forced: forced,
            is_external: false,
            vtt_url: None,
            burn_in_required: false,
        }
    }

    #[test]
    fn initial_tracks_follow_tracks_ts() {
        let audio = |index, lang: &str, def| AudioTrack {
            index,
            language: Some(lang.into()),
            codec: None,
            title: lang.into(),
            is_default: def,
            is_external: false,
        };
        let t = PlaybackTracks {
            audio: vec![audio(1, "eng", true), audio(2, "fre", false)],
            subtitles: vec![
                sub(3, "fra", true),
                sub(4, "fr", false),
                sub(5, "eng", false),
            ],
            default_audio_index: Some(1),
            default_subtitle_index: None,
        };
        assert_eq!(pick_initial_audio(&t, Some("fr")).map(|a| a.index), Some(2));
        assert_eq!(pick_initial_audio(&t, Some("de")).map(|a| a.index), Some(1));
        assert_eq!(pick_initial_audio(&t, None).map(|a| a.index), Some(1));
        assert_eq!(
            pick_initial_subtitle(&t, Some("fra")).map(|s| s.index),
            Some(4)
        );
        assert_eq!(pick_initial_subtitle(&t, None), None);
        assert!(same_language(Some("fr-FR"), Some("fre")));
        assert!(!same_language(None, Some("fre")));
    }

    #[test]
    fn next_episode_uses_series_order() {
        let ep = |id: &str, s: i32, e: i32| BaseItemDto {
            id: id.into(),
            parent_index_number: Some(s),
            index_number: Some(e),
            ..Default::default()
        };
        let list = vec![
            ep("s2e1", 2, 1),
            ep("sp", 0, 1),
            ep("s1e2", 1, 2),
            ep("s1e1", 1, 1),
        ];
        assert_eq!(
            next_episode(&list, "s1e1").map(|e| e.id),
            Some("s1e2".into())
        );
        assert_eq!(
            next_episode(&list, "s1e2").map(|e| e.id),
            Some("s2e1".into())
        );
        assert_eq!(next_episode(&list, "s2e1").map(|e| e.id), Some("sp".into()));
        assert_eq!(next_episode(&list, "sp"), None);
        assert_eq!(rate_label(1.0), "1×");
        assert_eq!(rate_label(1.25), "1.25×");
    }
}
