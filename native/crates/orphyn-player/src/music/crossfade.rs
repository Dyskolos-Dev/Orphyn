//! Crossfade between songs: the pure rules (should we fade, for how long) and the equal-power
//! curves (port of `features/music/player/crossfade.ts`). The audio thread applies them.
use super::types::{RepeatMode, Track};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossfadeSettings {
    pub enabled: bool,
    /// Wanted fade length, seconds.
    pub seconds: f64,
    /// Gapless albums: do not blend two consecutive tracks of the same album.
    pub skip_same_album: bool,
}

pub const CROSSFADE_OFF: CrossfadeSettings = CrossfadeSettings {
    enabled: false,
    seconds: 0.0,
    skip_same_album: true,
};
pub const CROSSFADE_MIN: f64 = 1.0;
pub const CROSSFADE_MAX: f64 = 12.0;
/// A fade shorter than this is inaudible: play the plain (gapless) switch instead.
pub const MIN_FADE_SECONDS: f64 = 0.5;

impl Default for CrossfadeSettings {
    fn default() -> Self {
        CROSSFADE_OFF
    }
}

pub struct FadePlanInput<'a> {
    pub settings: CrossfadeSettings,
    pub current: &'a Track,
    pub next: &'a Track,
    pub repeat: RepeatMode,
    /// Real duration of the current track when the output knows it (seconds).
    pub current_duration: Option<f64>,
}

/// True when `next` follows `current` on the same album, as in a gapless album playthrough.
pub fn is_album_continuation(current: &Track, next: &Track) -> bool {
    let (Some(a), Some(b)) = (&current.album_id, &next.album_id) else {
        return false;
    };
    if a != b {
        return false;
    }
    match (current.index_number, next.index_number) {
        (Some(x), Some(y)) => {
            current.parent_index_number.unwrap_or(1) == next.parent_index_number.unwrap_or(1)
                && y == x + 1
        }
        _ => true,
    }
}

/// Seconds of crossfade to use from `current` into `next`; 0 means no crossfade (plain switch).
/// Tracks shorter than twice the wanted length get a proportionally shorter fade.
pub fn plan_fade(i: &FadePlanInput) -> f64 {
    let s = &i.settings;
    if !s.enabled || i.repeat == RepeatMode::One {
        return 0.0;
    }
    if s.skip_same_album && is_album_continuation(i.current, i.next) {
        return 0.0;
    }
    let mut seconds = s.seconds.clamp(CROSSFADE_MIN, CROSSFADE_MAX);
    let current = i
        .current_duration
        .filter(|d| *d > 0.0)
        .unwrap_or_else(|| i.current.duration_seconds());
    for d in [current, i.next.duration_seconds()] {
        if d > 0.0 {
            seconds = seconds.min(d / 2.0);
        }
    }
    if seconds >= MIN_FADE_SECONDS {
        seconds
    } else {
        0.0
    }
}

/// Equal-power gains `(incoming, outgoing)` at progress `p` (0..1): `in² + out² = 1`, so the
/// loudness stays constant.
pub fn fade_gains(p: f64) -> (f32, f32) {
    let x = p.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2;
    (x.sin() as f32, x.cos() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, album: &str, n: i32, secs: i64) -> Track {
        Track {
            id: id.into(),
            album_id: Some(album.into()),
            index_number: Some(n),
            run_time_ticks: Some(secs * 10_000_000),
            ..Track::default()
        }
    }

    fn on(seconds: f64) -> CrossfadeSettings {
        CrossfadeSettings {
            enabled: true,
            seconds,
            skip_same_album: true,
        }
    }

    fn plan(s: CrossfadeSettings, a: &Track, b: &Track, repeat: RepeatMode) -> f64 {
        plan_fade(&FadePlanInput {
            settings: s,
            current: a,
            next: b,
            repeat,
            current_duration: None,
        })
    }

    #[test]
    fn off_or_repeat_one_never_fades() {
        let (a, b) = (track("a", "x", 1, 200), track("b", "y", 1, 200));
        assert_eq!(plan(CROSSFADE_OFF, &a, &b, RepeatMode::Off), 0.0);
        assert_eq!(plan(on(6.0), &a, &b, RepeatMode::One), 0.0);
        assert_eq!(plan(on(6.0), &a, &b, RepeatMode::Off), 6.0);
    }

    #[test]
    fn album_continuation_stays_gapless_when_asked() {
        let (a, b) = (track("a", "x", 1, 200), track("b", "x", 2, 200));
        assert!(is_album_continuation(&a, &b));
        assert_eq!(plan(on(6.0), &a, &b, RepeatMode::Off), 0.0);
        let mut s = on(6.0);
        s.skip_same_album = false;
        assert_eq!(plan(s, &a, &b, RepeatMode::Off), 6.0);
        // Same album but not consecutive (shuffle): not a continuation.
        let c = track("c", "x", 5, 200);
        assert!(!is_album_continuation(&a, &c));
        assert_eq!(plan(on(6.0), &a, &c, RepeatMode::Off), 6.0);
        // Other disc.
        let mut d = track("d", "x", 2, 200);
        d.parent_index_number = Some(2);
        assert!(!is_album_continuation(&a, &d));
    }

    #[test]
    fn length_is_clamped_and_limited_by_short_tracks() {
        let (a, b) = (track("a", "x", 1, 200), track("b", "y", 1, 200));
        assert_eq!(plan(on(40.0), &a, &b, RepeatMode::Off), CROSSFADE_MAX);
        assert_eq!(plan(on(0.2), &a, &b, RepeatMode::Off), CROSSFADE_MIN);
        let short = track("s", "y", 1, 8);
        assert_eq!(plan(on(6.0), &a, &short, RepeatMode::Off), 4.0);
        // Real duration of the playing track wins over the metadata.
        let p = plan_fade(&FadePlanInput {
            settings: on(6.0),
            current: &a,
            next: &b,
            repeat: RepeatMode::Off,
            current_duration: Some(5.0),
        });
        assert_eq!(p, 2.5);
        // Under the minimum audible length: plain switch.
        let tiny = track("t", "y", 1, 0);
        let tiny = Track {
            run_time_ticks: Some(8_000_000),
            ..tiny
        };
        assert_eq!(plan(on(6.0), &a, &tiny, RepeatMode::Off), 0.0);
    }

    #[test]
    fn curves_are_equal_power() {
        for i in 0..=10 {
            let (inn, out) = fade_gains(i as f64 / 10.0);
            assert!((inn * inn + out * out - 1.0).abs() < 1e-5);
        }
        assert_eq!(fade_gains(0.0), (0.0, 1.0));
        let (i, o) = fade_gains(1.0);
        assert!((i - 1.0).abs() < 1e-6 && o.abs() < 1e-6);
        assert_eq!(fade_gains(-3.0), fade_gains(0.0));
    }
}
