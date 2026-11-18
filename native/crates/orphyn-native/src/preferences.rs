//! Playback and music preferences: port of `features/settings/preferences.ts`. Same storage key
//! (`orphyn.playback.preferences`, one JSON object), same field names, defaults and field-by-field
//! validation, so the file written by either app reads the same in the other.
use orphyn_core::storage::LocalStorage;
use serde::Serialize;
use serde_json::Value;

pub const PLAYBACK_PREFERENCES_KEY: &str = "orphyn.playback.preferences";
pub const CROSSFADE_SECONDS_MIN: i64 = 1;
pub const CROSSFADE_SECONDS_MAX: i64 = 12;
/// Video streaming ceilings offered in Settings (bits per second).
pub const VIDEO_BITRATES: [u64; 7] = [
    120_000_000,
    60_000_000,
    40_000_000,
    20_000_000,
    8_000_000,
    4_000_000,
    2_000_000,
];
/// Music transcoding ceilings offered in Settings (bits per second).
pub const AUDIO_BITRATES: [u64; 4] = [320_000, 256_000, 192_000, 128_000];
/// Languages offered in Settings (ISO 639-2).
pub const LANGUAGES: [&str; 10] = [
    "eng", "fre", "spa", "ger", "ita", "por", "jpn", "kor", "chi", "rus",
];

/// `'original'` or a bits-per-second ceiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioQuality {
    Original,
    Bitrate(f64),
}

impl Serialize for AudioQuality {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            AudioQuality::Original => s.serialize_str("original"),
            AudioQuality::Bitrate(b) => s.serialize_f64(*b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackPreferences {
    pub max_video_bitrate: Option<f64>,
    pub preferred_audio_language: Option<String>,
    pub preferred_subtitle_language: Option<String>,
    pub auto_play_next: bool,
    pub audio_quality: AudioQuality,
    pub crossfade: bool,
    pub crossfade_seconds: i64,
    pub crossfade_skip_album: bool,
    pub group_artists: bool,
    pub online_lyrics: bool,
    pub artist_images: bool,
    pub audio_output_id: Option<String>,
}

impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            max_video_bitrate: None,
            preferred_audio_language: None,
            preferred_subtitle_language: None,
            auto_play_next: true,
            audio_quality: AudioQuality::Original,
            crossfade: false,
            crossfade_seconds: 6,
            crossfade_skip_album: true,
            group_artists: true,
            online_lyrics: true,
            artist_images: true,
            audio_output_id: None,
        }
    }
}

impl PlaybackPreferences {
    /// `sanitize`: unknown or invalid stored values fall back to the defaults field by field.
    pub fn sanitize(raw: &Value) -> Self {
        let d = Self::default();
        let empty = serde_json::Map::new();
        let o = raw.as_object().unwrap_or(&empty);
        let pos = |k: &str| {
            o.get(k)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite() && *v > 0.0)
        };
        let lang = |k: &str| {
            o.get(k)
                .and_then(Value::as_str)
                .filter(|v| v.len() == 3 && v.chars().all(|c| c.is_ascii_alphabetic()))
                .map(|v| v.to_lowercase())
        };
        let flag = |k: &str, def: bool| o.get(k).and_then(Value::as_bool).unwrap_or(def);
        Self {
            max_video_bitrate: pos("maxVideoBitrate"),
            preferred_audio_language: lang("preferredAudioLanguage"),
            preferred_subtitle_language: lang("preferredSubtitleLanguage"),
            auto_play_next: flag("autoPlayNext", d.auto_play_next),
            audio_quality: match o.get("audioQuality") {
                Some(Value::String(s)) if s == "original" => AudioQuality::Original,
                _ => pos("audioQuality")
                    .map(AudioQuality::Bitrate)
                    .unwrap_or(d.audio_quality),
            },
            crossfade: flag("crossfade", d.crossfade),
            crossfade_seconds: o
                .get("crossfadeSeconds")
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .map(|v| (v.round() as i64).clamp(CROSSFADE_SECONDS_MIN, CROSSFADE_SECONDS_MAX))
                .unwrap_or(d.crossfade_seconds),
            crossfade_skip_album: flag("crossfadeSkipAlbum", d.crossfade_skip_album),
            group_artists: flag("groupArtists", d.group_artists),
            online_lyrics: flag("onlineLyrics", d.online_lyrics),
            artist_images: flag("artistImages", d.artist_images),
            audio_output_id: o
                .get("audioOutputId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from),
        }
    }

    pub fn load(storage: &LocalStorage) -> Self {
        Self::sanitize(
            &storage
                .get_json::<Value>(PLAYBACK_PREFERENCES_KEY)
                .unwrap_or(Value::Null),
        )
    }

    pub fn save(&self, storage: &LocalStorage) {
        storage.set_json(PLAYBACK_PREFERENCES_KEY, self);
    }
}

/// `languageMatches`: two ISO 639-2 codes name the same language (`fra` = `fre`).
pub fn language_matches(a: Option<&str>, b: Option<&str>) -> bool {
    const ALIASES: [(&str, &str); 19] = [
        ("fra", "fre"),
        ("deu", "ger"),
        ("zho", "chi"),
        ("nld", "dut"),
        ("ces", "cze"),
        ("ell", "gre"),
        ("fas", "per"),
        ("isl", "ice"),
        ("mkd", "mac"),
        ("ron", "rum"),
        ("slk", "slo"),
        ("sqi", "alb"),
        ("hye", "arm"),
        ("kat", "geo"),
        ("eus", "baq"),
        ("mya", "bur"),
        ("msa", "may"),
        ("bod", "tib"),
        ("cym", "wel"),
    ];
    let norm = |c: &str| {
        let c = c.to_lowercase();
        ALIASES
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, v)| v.to_string())
            .unwrap_or(c)
    };
    match (a, b) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => norm(a) == norm(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_fields_fall_back_one_by_one() {
        let p = PlaybackPreferences::sanitize(&serde_json::json!({
            "maxVideoBitrate": 8000000,
            "preferredAudioLanguage": "FRE",
            "preferredSubtitleLanguage": "french",
            "audioQuality": 320000,
            "crossfadeSeconds": 40,
            "groupArtists": "yes",
        }));
        assert_eq!(p.max_video_bitrate, Some(8_000_000.0));
        assert_eq!(p.preferred_audio_language.as_deref(), Some("fre"));
        assert_eq!(p.preferred_subtitle_language, None);
        assert_eq!(p.audio_quality, AudioQuality::Bitrate(320_000.0));
        assert_eq!(p.crossfade_seconds, 12);
        assert!(p.group_artists);
        assert_eq!(
            PlaybackPreferences::sanitize(&Value::Null),
            PlaybackPreferences::default()
        );
    }

    #[test]
    fn saved_json_uses_the_react_field_names() {
        let s = LocalStorage::in_memory();
        PlaybackPreferences::default().save(&s);
        let v: Value = s.get_json(PLAYBACK_PREFERENCES_KEY).unwrap();
        assert_eq!(v["audioQuality"], "original");
        assert_eq!(v["autoPlayNext"], true);
        assert_eq!(v["maxVideoBitrate"], Value::Null);
        assert_eq!(
            PlaybackPreferences::load(&s),
            PlaybackPreferences::default()
        );
    }

    #[test]
    fn language_aliases() {
        assert!(language_matches(Some("fra"), Some("FRE")));
        assert!(!language_matches(Some("eng"), None));
    }
}
