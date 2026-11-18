//! What is playing as a Discord presence (port of `features/discord/presence.ts`).
use orphyn_core::discord::activity::Presence;
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::BaseItemDto;
use orphyn_player::music::PlayerStatus;
use orphyn_player::video::PlaybackStatus;

const TICKS_PER_SECOND: f64 = 10_000_000.0;
/// A progress tick that drifts from where the last update predicted is a seek.
const SEEK_DRIFT_SEC: f64 = 3.0;

/// Bare URL: the Jellyfin image endpoint needs no key and must never carry one.
pub fn artwork(base: &str, item: &BaseItemDto) -> Option<String> {
    build_image_url(
        base,
        item,
        ImageKind::Primary,
        &ImageOptions {
            max_width: Some(512),
            max_height: Some(512),
            quality: None,
            tag: None,
        },
    )
}

/// The poster of a video item: the series poster for an episode, its own otherwise.
pub fn video_poster(base: &str, item: &BaseItemDto) -> Option<String> {
    match (&item.series_id, is_episode(item)) {
        (Some(series), true) => {
            let mut s = BaseItemDto {
                id: series.clone(),
                item_type: Some("Series".into()),
                ..BaseItemDto::default()
            };
            if let Some(tag) = &item.series_primary_image_tag {
                s.image_tags.insert("Primary".into(), tag.clone());
            }
            artwork(base, &s)
        }
        _ => artwork(base, item),
    }
}

fn is_episode(item: &BaseItemDto) -> bool {
    item.item_type.as_deref() == Some("Episode") && item.series_name.is_some()
}

fn duration_of(measured: f64, item: &BaseItemDto) -> f64 {
    if measured > 0.0 {
        measured
    } else {
        item.run_time_ticks.unwrap_or(0) as f64 / TICKS_PER_SECOND
    }
}

pub fn music_presence(
    base: &str,
    track: Option<&BaseItemDto>,
    status: PlayerStatus,
    position: f64,
    duration: f64,
) -> Option<Presence> {
    let t = track?;
    if !matches!(
        status,
        PlayerStatus::Playing | PlayerStatus::Paused | PlayerStatus::Loading
    ) {
        return None;
    }
    Some(Presence::Music {
        paused: status == PlayerStatus::Paused,
        title: t.name.clone().unwrap_or_default(),
        artist: if t.artists.is_empty() {
            t.album_artist.clone()
        } else {
            Some(t.artists.join(", "))
        },
        album: t.album.clone(),
        position_sec: position,
        duration_sec: duration_of(duration, t),
        artwork_url: artwork(base, t),
    })
}

pub fn video_presence(
    base: &str,
    item_id: Option<&str>,
    item: Option<&BaseItemDto>,
    status: PlaybackStatus,
    position: f64,
    duration: f64,
) -> Option<Presence> {
    let (id, item) = (item_id?, item?);
    if item.id != id
        || !matches!(
            status,
            PlaybackStatus::Playing | PlaybackStatus::Paused | PlaybackStatus::Loading
        )
    {
        return None;
    }
    let episode = is_episode(item);
    Some(Presence::Video {
        paused: status == PlaybackStatus::Paused,
        title: item.name.clone().unwrap_or_default(),
        series: if episode {
            item.series_name.clone()
        } else {
            None
        },
        season: if episode {
            item.parent_index_number.map(|n| n as u32)
        } else {
            None
        },
        episode: if episode {
            item.index_number.map(|n| n as u32)
        } else {
            None
        },
        year: item.production_year.map(|n| n as u32),
        position_sec: position,
        duration_sec: duration_of(duration, item),
        artwork_url: video_poster(base, item),
    })
}

type Parts<'a> = (u8, &'a str, bool, Option<&'a str>, f64, f64);

fn parts(p: &Presence) -> Parts<'_> {
    match p {
        Presence::Music {
            paused,
            title,
            position_sec,
            duration_sec,
            artwork_url,
            ..
        } => (
            0,
            title,
            *paused,
            artwork_url.as_deref(),
            *position_sec,
            *duration_sec,
        ),
        Presence::Video {
            paused,
            title,
            position_sec,
            duration_sec,
            artwork_url,
            ..
        } => (
            1,
            title,
            *paused,
            artwork_url.as_deref(),
            *position_sec,
            *duration_sec,
        ),
    }
}

/// Whether `next` must be sent: the item, play/pause state or metadata changed, or the position
/// jumped away from the one extrapolated from the last update (a seek). Plain progress ticks are
/// not updates: Discord animates the progress bar by itself. `elapsed` = seconds since `prev`.
pub fn should_send(prev: Option<&Presence>, next: Option<&Presence>, elapsed: f64) -> bool {
    let (Some(before), Some(next)) = (prev, next) else {
        return prev.is_some() != next.is_some();
    };
    let (bk, bt, bp, ba, bpos, bdur) = parts(before);
    let (nk, nt, np, na, npos, ndur) = parts(next);
    if bk != nk || bt != nt || bp != np || ba != na {
        return true;
    }
    if bdur != ndur && ndur > 0.0 {
        return true;
    }
    let expected = if bp { bpos } else { bpos + elapsed };
    (npos - expected).abs() > SEEK_DRIFT_SEC
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, paused: bool, pos: f64) -> Presence {
        Presence::Music {
            paused,
            title: title.into(),
            position_sec: pos,
            duration_sec: 200.0,
            artwork_url: None,
            artist: None,
            album: None,
        }
    }

    #[test]
    fn plain_ticks_are_not_sent_but_seeks_and_changes_are() {
        let a = song("A", false, 10.0);
        assert!(!should_send(Some(&a), Some(&song("A", false, 11.0)), 1.0));
        assert!(should_send(Some(&a), Some(&song("A", false, 60.0)), 1.0));
        assert!(should_send(Some(&a), Some(&song("A", true, 10.0)), 1.0));
        assert!(should_send(Some(&a), Some(&song("B", false, 10.0)), 1.0));
        assert!(should_send(None, Some(&a), 0.0));
        assert!(should_send(Some(&a), None, 0.0));
        assert!(!should_send(None, None, 0.0));
    }

    #[test]
    fn music_presence_needs_a_playing_track() {
        let t = BaseItemDto {
            id: "t1".into(),
            name: Some("Song".into()),
            artists: vec!["A".into(), "B".into()],
            ..BaseItemDto::default()
        };
        assert!(music_presence("http://x", Some(&t), PlayerStatus::Idle, 0.0, 0.0).is_none());
        let p = music_presence("http://x", Some(&t), PlayerStatus::Paused, 5.0, 100.0).unwrap();
        match p {
            Presence::Music {
                paused,
                artist,
                title,
                ..
            } => {
                assert!(paused);
                assert_eq!(artist.as_deref(), Some("A, B"));
                assert_eq!(title, "Song");
            }
            _ => panic!("music expected"),
        }
    }

    #[test]
    fn an_episode_carries_series_season_and_episode() {
        let item = BaseItemDto {
            id: "e1".into(),
            name: Some("Pilot".into()),
            item_type: Some("Episode".into()),
            series_name: Some("Show".into()),
            parent_index_number: Some(1),
            index_number: Some(2),
            ..BaseItemDto::default()
        };
        let p = video_presence(
            "http://x",
            Some("e1"),
            Some(&item),
            PlaybackStatus::Playing,
            1.0,
            50.0,
        )
        .unwrap();
        assert!(matches!(
            p,
            Presence::Video { series: Some(ref s), season: Some(1), episode: Some(2), .. } if s == "Show"
        ));
        assert!(video_presence(
            "http://x",
            Some("other"),
            Some(&item),
            PlaybackStatus::Playing,
            0.0,
            0.0
        )
        .is_none());
    }
}
