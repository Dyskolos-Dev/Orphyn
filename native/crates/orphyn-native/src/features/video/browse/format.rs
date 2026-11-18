//! `format.ts` and the path helpers of `cards.tsx`: pure functions of the video screens.
use orphyn_jellyfin::api::video::{MediaStream, VideoItemExt};
use orphyn_jellyfin::BaseItemDto;

const TICKS_PER_MINUTE: i64 = 600_000_000;

pub fn item_path(id: &str) -> String {
    format!("/video/item/{id}")
}

/// `playPath`: the player route, with `?start=<ticks>` when resuming.
pub fn play_path(id: &str, start_ticks: i64) -> String {
    if start_ticks > 0 {
        format!("/video/play/{id}?start={start_ticks}")
    } else {
        format!("/video/play/{id}")
    }
}

/// `trailerPath`: a trailer file of the server in the player (no resume, nothing reported).
pub fn trailer_path(id: &str) -> String {
    format!("/video/play/{id}?trailer=1")
}

pub fn position_ticks(item: &BaseItemDto) -> i64 {
    item.user_data
        .as_ref()
        .and_then(|u| u.playback_position_ticks)
        .unwrap_or(0)
}

pub fn is_played(item: &BaseItemDto) -> bool {
    item.user_data.as_ref().is_some_and(|u| u.played)
}

/// "2 h 10 min", "45 min", "" when unknown (`formatRuntime`).
pub fn format_runtime(ticks: Option<i64>) -> String {
    let Some(t) = ticks.filter(|t| *t > 0) else {
        return String::new();
    };
    let total = ((t as f64 / TICKS_PER_MINUTE as f64).round() as i64).max(1);
    let (h, m) = (total / 60, total % 60);
    if h == 0 {
        format!("{m} min")
    } else if m == 0 {
        format!("{h} h")
    } else {
        format!("{h} h {m} min")
    }
}

/// Minutes shown in "Resume at 42 min" (rounded, at least 1).
pub fn resume_minutes(position_ticks: i64) -> i64 {
    ((position_ticks as f64 / TICKS_PER_MINUTE as f64).round() as i64).max(1)
}

/// 0..1 watched progress, `None` when not started (`progressOf`).
pub fn progress_of(item: &BaseItemDto) -> Option<f64> {
    let pos = position_ticks(item);
    let total = item.run_time_ticks.unwrap_or(0);
    if pos > 0 && total > 0 {
        return Some((pos as f64 / total as f64).min(1.0));
    }
    item.user_data
        .as_ref()
        .and_then(|u| u.played_percentage)
        .filter(|p| *p > 0.0)
        .map(|p| (p / 100.0).min(1.0))
}

/// Minutes left for "24 min left" captions; 0 when unknown (`remainingMinutes`).
pub fn remaining_minutes(item: &BaseItemDto) -> i64 {
    let total = item.run_time_ticks.unwrap_or(0);
    if total > 0 {
        (((total - position_ticks(item)) as f64 / TICKS_PER_MINUTE as f64).round() as i64).max(1)
    } else {
        0
    }
}

/// Episodes by season then number; specials (season 0) go last (`sortEpisodes`).
pub fn sort_episodes(episodes: &[BaseItemDto]) -> Vec<BaseItemDto> {
    let season = |e: &BaseItemDto| match e.parent_index_number.unwrap_or(0) {
        0 => i64::MAX,
        s => s as i64,
    };
    let num = |e: &BaseItemDto| e.index_number.map(|n| n as i64).unwrap_or(i64::MAX);
    let name = |e: &BaseItemDto| {
        e.sort_name()
            .unwrap_or_else(|| e.name().to_string())
            .to_lowercase()
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

/// The episode a series' primary button plays: in progress, else Next Up, else the first unplayed,
/// else the first (`pickSeriesEpisode`).
pub fn pick_series_episode(
    episodes: &[BaseItemDto],
    next_up: Option<&BaseItemDto>,
) -> Option<BaseItemDto> {
    let sorted = sort_episodes(episodes);
    if let Some(e) = sorted.iter().find(|e| position_ticks(e) > 0) {
        return Some(e.clone());
    }
    if let Some(n) = next_up.filter(|n| !n.id.is_empty()) {
        return Some(sorted.iter().find(|e| e.id == n.id).unwrap_or(n).clone());
    }
    sorted
        .iter()
        .find(|e| !is_played(e))
        .or(sorted.first())
        .cloned()
}

/// What the big primary button of a detail page does (`PrimaryAction`).
#[derive(Clone, Debug, PartialEq)]
pub enum PrimaryAction {
    Play,
    Resume {
        position_ticks: i64,
        minutes: i64,
    },
    Episode {
        episode_id: String,
        season: Option<i32>,
        episode: Option<i32>,
        position_ticks: i64,
    },
}

pub fn primary_action(item: &BaseItemDto, episode: Option<&BaseItemDto>) -> PrimaryAction {
    if item.is_type("Series") || item.is_type("Season") {
        return match episode.filter(|e| !e.id.is_empty()) {
            None => PrimaryAction::Play,
            Some(e) => PrimaryAction::Episode {
                episode_id: e.id.clone(),
                season: e.parent_index_number,
                episode: e.index_number,
                position_ticks: position_ticks(e),
            },
        };
    }
    let pos = position_ticks(item);
    if pos > 0 {
        PrimaryAction::Resume {
            position_ticks: pos,
            minutes: resume_minutes(pos),
        }
    } else {
        PrimaryAction::Play
    }
}

/// Badge strings in display order: 4K, HDR / Dolby Vision, audio layout (`badgeList`).
pub fn badge_list(streams: &[MediaStream]) -> Vec<String> {
    let video = streams.iter().find(|s| s.kind.as_deref() == Some("Video"));
    let mut out = Vec::new();
    if let Some(v) = video {
        if v.width.unwrap_or(0) >= 3800 || v.height.unwrap_or(0) >= 2100 {
            out.push("4K".to_string());
        }
        let range = format!(
            "{} {}",
            v.video_range_type.clone().unwrap_or_default(),
            v.video_range.clone().unwrap_or_default()
        )
        .to_lowercase();
        if range.contains("dovi") || range.contains("dolby") {
            out.push("Dolby Vision".into());
        } else if range.contains("hdr") {
            out.push("HDR".into());
        }
    }
    let audio: Vec<&MediaStream> = streams
        .iter()
        .filter(|s| s.kind.as_deref() == Some("Audio"))
        .collect();
    let best = audio
        .iter()
        .map(|s| s.channels.unwrap_or(0))
        .max()
        .unwrap_or(0);
    let atmos = audio.iter().any(|s| {
        format!(
            "{} {}",
            s.profile.clone().unwrap_or_default(),
            s.display_title.clone().unwrap_or_default()
        )
        .to_lowercase()
        .contains("atmos")
    });
    if atmos {
        out.push("Atmos".into());
    } else if best >= 8 {
        out.push("7.1".into());
    } else if best >= 6 {
        out.push("5.1".into());
    }
    out
}

/// Year, genres, runtime, rating; empty ones dropped (`metaParts`).
pub fn meta_parts(item: &BaseItemDto) -> Vec<String> {
    let genres = item
        .genres
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    [
        item.production_year.map(|y| y.to_string()),
        Some(genres),
        if item.is_type("Series") {
            None
        } else {
            Some(format_runtime(item.run_time_ticks))
        },
        item.official_rating(),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect()
}

/// The hero line of the home: year · genres · runtime · rating (`heroMeta`).
pub fn hero_meta(item: &BaseItemDto) -> String {
    meta_parts(item).join(" · ")
}

/// First letter of the sort name, accents stripped; anything else is "#" (`letterOf`).
pub fn letter_of(item: &BaseItemDto) -> char {
    let name = item.sort_name().unwrap_or_else(|| item.name().to_string());
    let first = name.chars().next().map(strip_accent).unwrap_or('#');
    let up = first.to_ascii_uppercase();
    if up.is_ascii_uppercase() {
        up
    } else {
        '#'
    }
}

fn strip_accent(c: char) -> char {
    match c {
        'à'..='å' | 'À'..='Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è'..='ë' | 'È'..='Ë' => 'e',
        'ì'..='ï' | 'Ì'..='Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò'..='ö' | 'Ò'..='Ö' | 'ø' | 'Ø' => 'o',
        'ù'..='ü' | 'Ù'..='Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        c => c,
    }
}

/// Index of the first loaded item whose letter is >= `letter` (items sorted A-Z)
/// (`firstIndexForLetter`).
pub fn first_index_for_letter(items: &[BaseItemDto], letter: char) -> Option<usize> {
    if letter == '#' {
        return (!items.is_empty()).then_some(0);
    }
    items.iter().position(|i| {
        let l = letter_of(i);
        l != '#' && l >= letter
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(v: serde_json::Value) -> BaseItemDto {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn runtime_is_formatted_like_the_react_helper() {
        assert_eq!(format_runtime(None), "");
        assert_eq!(format_runtime(Some(0)), "");
        assert_eq!(format_runtime(Some(45 * TICKS_PER_MINUTE)), "45 min");
        assert_eq!(format_runtime(Some(120 * TICKS_PER_MINUTE)), "2 h");
        assert_eq!(format_runtime(Some(130 * TICKS_PER_MINUTE)), "2 h 10 min");
        assert_eq!(format_runtime(Some(10)), "1 min");
    }

    #[test]
    fn progress_and_remaining() {
        let m = item(json!({"Id":"m","RunTimeTicks": 100 * TICKS_PER_MINUTE,
            "UserData": {"PlaybackPositionTicks": 25 * TICKS_PER_MINUTE}}));
        assert_eq!(progress_of(&m), Some(0.25));
        assert_eq!(remaining_minutes(&m), 75);
        let none = item(json!({"Id":"m","RunTimeTicks": 100 * TICKS_PER_MINUTE}));
        assert_eq!(progress_of(&none), None);
        let pct = item(json!({"Id":"m","UserData": {"PlayedPercentage": 40.0}}));
        assert_eq!(progress_of(&pct), Some(0.4));
    }

    #[test]
    fn episodes_sort_with_specials_last_and_pick_in_progress_first() {
        let eps = vec![
            item(json!({"Id":"s0","ParentIndexNumber":0,"IndexNumber":1})),
            item(json!({"Id":"b","ParentIndexNumber":2,"IndexNumber":1})),
            item(
                json!({"Id":"a2","ParentIndexNumber":1,"IndexNumber":2,"UserData":{"Played":true}}),
            ),
            item(
                json!({"Id":"a1","ParentIndexNumber":1,"IndexNumber":1,"UserData":{"Played":true}}),
            ),
        ];
        let ids: Vec<_> = sort_episodes(&eps).into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["a1", "a2", "b", "s0"]);
        // First unplayed when nothing is in progress and there is no next up.
        assert_eq!(pick_series_episode(&eps, None).unwrap().id, "b");
        let next = item(json!({"Id":"s0"}));
        assert_eq!(pick_series_episode(&eps, Some(&next)).unwrap().id, "s0");
        let mut eps2 = eps.clone();
        eps2[1] = item(json!({"Id":"b","ParentIndexNumber":2,"IndexNumber":1,
            "UserData":{"PlaybackPositionTicks": 5}}));
        assert_eq!(pick_series_episode(&eps2, Some(&next)).unwrap().id, "b");
    }

    #[test]
    fn primary_action_of_movie_and_series() {
        let m = item(json!({"Id":"m","Type":"Movie",
            "UserData":{"PlaybackPositionTicks": 42 * TICKS_PER_MINUTE}}));
        assert_eq!(
            primary_action(&m, None),
            PrimaryAction::Resume {
                position_ticks: 42 * TICKS_PER_MINUTE,
                minutes: 42
            }
        );
        let s = item(json!({"Id":"s","Type":"Series"}));
        assert_eq!(primary_action(&s, None), PrimaryAction::Play);
        let e = item(json!({"Id":"e","ParentIndexNumber":2,"IndexNumber":3}));
        assert_eq!(
            primary_action(&s, Some(&e)),
            PrimaryAction::Episode {
                episode_id: "e".into(),
                season: Some(2),
                episode: Some(3),
                position_ticks: 0
            }
        );
    }

    #[test]
    fn paths_match_the_react_routes() {
        assert_eq!(play_path("a", 0), "/video/play/a");
        assert_eq!(play_path("a", 600), "/video/play/a?start=600");
        assert_eq!(trailer_path("t"), "/video/play/t?trailer=1");
        assert_eq!(item_path("x"), "/video/item/x");
    }

    #[test]
    fn badges_and_meta() {
        let m = item(json!({"Id":"m","Type":"Movie","ProductionYear":2021,
            "Genres":["Drama","Crime","Other"],"RunTimeTicks": 130 * TICKS_PER_MINUTE,
            "OfficialRating":"R",
            "MediaSources":[{"MediaStreams":[
                {"Type":"Video","Width":3840,"Height":2160,"VideoRangeType":"DOVIWithHDR10"},
                {"Type":"Audio","Channels":8,"Profile":"TrueHD Atmos"}]}]}));
        assert_eq!(
            badge_list(&m.media_streams()),
            ["4K", "Dolby Vision", "Atmos"]
        );
        assert_eq!(hero_meta(&m), "2021 · Drama, Crime · 2 h 10 min · R");
        let series = item(json!({"Id":"s","Type":"Series","RunTimeTicks": 5}));
        assert_eq!(meta_parts(&series), Vec::<String>::new());
    }

    #[test]
    fn letters_jump_like_the_az_rail() {
        let items = vec![
            item(json!({"Id":"1","Name":"12 Angry Men"})),
            item(json!({"Id":"2","Name":"Åke"})),
            item(json!({"Id":"3","Name":"Béla"})),
            item(json!({"Id":"4","Name":"Zorro"})),
        ];
        assert_eq!(letter_of(&items[0]), '#');
        assert_eq!(letter_of(&items[1]), 'A');
        assert_eq!(letter_of(&items[2]), 'B');
        assert_eq!(first_index_for_letter(&items, '#'), Some(0));
        assert_eq!(first_index_for_letter(&items, 'B'), Some(2));
        assert_eq!(first_index_for_letter(&items, 'C'), Some(3));
        assert_eq!(first_index_for_letter(&items, 'Z'), Some(3));
        assert_eq!(first_index_for_letter(&[], 'A'), None);
    }
}
