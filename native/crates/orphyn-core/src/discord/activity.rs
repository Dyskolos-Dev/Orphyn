//! Builds the Discord activity for what is playing (1:1 port of `src/main/discord/activity.ts`).

use reqwest::Url;
use serde::Deserialize;

pub const ACTIVITY_LISTENING: u8 = 2;
pub const ACTIVITY_WATCHING: u8 = 3;

/// Art asset keys uploaded to the Discord application.
pub const ASSET_LOGO: &str = "orphyn";
pub const ASSET_PAUSE: &str = "pause";
/// Where a click on the presence's large or small image leads.
pub const ORPHYN_WEBSITE_URL: &str = "https://orphyn.dyskolos.fr";

const SECRET_PARAMS: [&str; 5] = [
    "api_key",
    "apikey",
    "x-emby-token",
    "x-mediabrowser-token",
    "token",
];

/// `DiscordPresence` of the TypeScript contract (`src/shared/ipc/discord.ts`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Presence {
    #[serde(rename_all = "camelCase")]
    Music {
        paused: bool,
        title: String,
        position_sec: f64,
        duration_sec: f64,
        artwork_url: Option<String>,
        artist: Option<String>,
        album: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Video {
        paused: bool,
        title: String,
        position_sec: f64,
        duration_sec: f64,
        artwork_url: Option<String>,
        series: Option<String>,
        season: Option<u32>,
        episode: Option<u32>,
        year: Option<u32>,
    },
}

impl Presence {
    fn title(&self) -> &str {
        match self {
            Presence::Music { title, .. } | Presence::Video { title, .. } => title,
        }
    }
}

/// The subset of the RPC activity payload Orphyn uses. Timestamps are Unix milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub kind: u8,
    pub details: String,
    pub state: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub large_image: String,
    pub large_text: String,
    pub small_image: String,
    pub small_text: String,
    pub link: &'static str,
}

/// Hosts a Discord client cannot fetch from the internet: localhost, LAN, link-local, .local.
pub fn is_public_host(hostname: &str) -> bool {
    let lower = hostname.to_lowercase();
    let host = lower.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() || host == "localhost" || host.ends_with(".localhost") {
        return false;
    }
    if host.ends_with(".local") || host.ends_with(".lan") || host.ends_with(".internal") {
        return false;
    }
    if !host.contains('.') && !host.contains(':') {
        return false; // bare machine name
    }
    let octets: Vec<&str> = host.split('.').collect();
    let is_v4 = octets.len() == 4
        && octets
            .iter()
            .all(|o| !o.is_empty() && o.len() <= 3 && o.bytes().all(|b| b.is_ascii_digit()));
    if is_v4 {
        let n: Vec<u32> = octets.iter().map(|o| o.parse().unwrap_or(999)).collect();
        let (a, b) = (n[0], n[1]);
        if a == 10 || a == 127 || a == 0 || a >= 224 {
            return false;
        }
        if a == 169 && b == 254 {
            return false;
        }
        if a == 172 && (16..=31).contains(&b) {
            return false;
        }
        if a == 192 && b == 168 {
            return false;
        }
        if a == 100 && (64..=127).contains(&b) {
            return false; // CGNAT / Tailscale
        }
        return true;
    }
    if host.contains(':') {
        // IPv6: only global unicast (2000::/3) counts as public.
        return host.starts_with('2') || host.starts_with('3');
    }
    true
}

/// The artwork URL that may be sent to Discord, or `None`. It must be https on a public host
/// (Discord's servers fetch it) and never carries credentials: secret query params are dropped,
/// and a URL with user info is refused.
pub fn safe_artwork_url(raw: Option<&str>, show_artwork: bool) -> Option<String> {
    let raw = raw?;
    if !show_artwork || raw.is_empty() {
        return None;
    }
    let mut url = Url::parse(raw).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    if !is_public_host(url.host_str()?) {
        return None;
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !SECRET_PARAMS.contains(&k.to_lowercase().as_str()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(kept);
    }
    url.set_fragment(None);
    let out = url.to_string();
    // Discord limits image strings to 256 characters.
    (out.chars().count() <= 256).then_some(out)
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max - 1).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

fn episode_code(season: Option<u32>, episode: Option<u32>) -> String {
    format!(
        "{}{}",
        season.map(|s| format!("S{s}")).unwrap_or_default(),
        episode.map(|e| format!("E{e}")).unwrap_or_default()
    )
}

/// What `build_activity` derives from a presence before the shared part.
struct Parts<'a> {
    details: String,
    state: Option<String>,
    large_text: String,
    kind: u8,
    paused: bool,
    position: f64,
    duration: f64,
    artwork: Option<&'a str>,
}

fn parts(presence: &Presence) -> Parts<'_> {
    match presence {
        Presence::Music {
            title,
            artist,
            album,
            paused,
            position_sec,
            duration_sec,
            artwork_url,
        } => {
            let joined = [artist.as_deref(), album.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            let large = album
                .clone()
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| title.clone());
            Parts {
                details: title.clone(),
                state: (!joined.is_empty()).then_some(joined),
                large_text: large,
                kind: ACTIVITY_LISTENING,
                paused: *paused,
                position: *position_sec,
                duration: *duration_sec,
                artwork: artwork_url.as_deref(),
            }
        }
        Presence::Video {
            title,
            series,
            season,
            episode,
            year,
            paused,
            position_sec,
            duration_sec,
            artwork_url,
        } => {
            let (details, state, large) = match series.as_deref().filter(|s| !s.is_empty()) {
                Some(series) => {
                    let code = episode_code(*season, *episode);
                    let prefix = if code.is_empty() {
                        String::new()
                    } else {
                        format!("{code} — ")
                    };
                    (
                        format!("{series} · {prefix}{title}"),
                        Some(series.to_string()),
                        series.to_string(),
                    )
                }
                None => (
                    title.clone(),
                    year.filter(|y| *y != 0).map(|y| y.to_string()),
                    title.clone(),
                ),
            };
            Parts {
                details,
                state,
                large_text: large,
                kind: ACTIVITY_WATCHING,
                paused: *paused,
                position: *position_sec,
                duration: *duration_sec,
                artwork: artwork_url.as_deref(),
            }
        }
    }
}

/// Builds the Discord activity for what is playing; `None` when nothing is.
pub fn build_activity(
    presence: Option<&Presence>,
    show_artwork: bool,
    now_ms: i64,
) -> Option<Activity> {
    let presence = presence?;
    if presence.title().trim().is_empty() {
        return None;
    }
    let p = parts(presence);
    let art = safe_artwork_url(p.artwork, show_artwork);
    let mut activity = Activity {
        kind: p.kind,
        details: clip(&p.details, 128),
        state: None,
        start: None,
        end: None,
        large_image: art.unwrap_or_else(|| ASSET_LOGO.to_string()),
        large_text: clip(&p.large_text, 128),
        small_image: ASSET_LOGO.to_string(),
        small_text: "Orphyn".to_string(),
        link: ORPHYN_WEBSITE_URL,
    };
    let mut state = p.state;
    if p.paused {
        state = Some(match state {
            Some(s) => format!("Paused · {s}"),
            None => "Paused".to_string(),
        });
        activity.small_image = ASSET_PAUSE.to_string();
        activity.small_text = "Paused".to_string();
    } else {
        let start = now_ms as f64 - p.position.max(0.0) * 1000.0;
        activity.start = Some(start.round() as i64);
        if p.duration > 0.0 {
            activity.end = Some((start + p.duration * 1000.0).round() as i64);
        }
    }
    activity.state = state.map(|s| clip(&s, 128));
    Some(activity)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn music(paused: bool) -> Presence {
        Presence::Music {
            paused,
            title: "Song".into(),
            position_sec: 30.0,
            duration_sec: 200.0,
            artwork_url: Some(
                "https://media.example.com/Items/1/Images/Primary?api_key=SECRET&maxWidth=300"
                    .into(),
            ),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
        }
    }

    #[test]
    fn music_is_listening_with_timestamps() {
        let a = build_activity(Some(&music(false)), true, NOW).unwrap();
        assert_eq!(a.kind, ACTIVITY_LISTENING);
        assert_eq!(a.details, "Song");
        assert_eq!(a.state.as_deref(), Some("Artist — Album"));
        assert_eq!(a.start, Some(NOW - 30_000));
        assert_eq!(a.end, Some(NOW + 170_000));
        assert_eq!(a.small_image, ASSET_LOGO);
        assert_eq!(a.link, "https://orphyn.dyskolos.fr");
    }

    #[test]
    fn paused_has_no_timestamps_and_the_pause_icon() {
        let a = build_activity(Some(&music(true)), true, NOW).unwrap();
        assert_eq!(a.state.as_deref(), Some("Paused · Artist — Album"));
        assert_eq!((a.start, a.end), (None, None));
        assert_eq!(a.small_image, ASSET_PAUSE);
    }

    #[test]
    fn artwork_never_carries_the_api_key() {
        let a = build_activity(Some(&music(false)), true, NOW).unwrap();
        assert_eq!(
            a.large_image,
            "https://media.example.com/Items/1/Images/Primary?maxWidth=300"
        );
        assert!(!a.large_image.contains("SECRET"));
    }

    #[test]
    fn artwork_is_off_when_disabled_or_private() {
        let off = build_activity(Some(&music(false)), false, NOW).unwrap();
        assert_eq!(off.large_image, ASSET_LOGO);
        for raw in [
            "http://media.example.com/a.jpg",
            "https://192.168.1.4:8096/a.jpg",
            "https://localhost/a.jpg",
            "https://nas/a.jpg",
            "https://user:pw@media.example.com/a.jpg",
            "https://nas.local/a.jpg",
        ] {
            assert_eq!(safe_artwork_url(Some(raw), true), None, "{raw}");
        }
    }

    #[test]
    fn public_hosts() {
        assert!(is_public_host("jellyfin.example.com"));
        assert!(is_public_host("8.8.8.8"));
        assert!(!is_public_host("10.0.0.2"));
        assert!(!is_public_host("172.20.1.1"));
        assert!(!is_public_host("100.100.1.1"));
        assert!(!is_public_host("169.254.1.1"));
        assert!(is_public_host("2001:db8::1"));
        assert!(!is_public_host("fe80::1"));
    }

    #[test]
    fn episode_is_watching_with_series_and_code() {
        let p = Presence::Video {
            paused: false,
            title: "Pilot".into(),
            position_sec: 0.0,
            duration_sec: 0.0,
            artwork_url: None,
            series: Some("Show".into()),
            season: Some(1),
            episode: Some(2),
            year: None,
        };
        let a = build_activity(Some(&p), true, NOW).unwrap();
        assert_eq!(a.kind, ACTIVITY_WATCHING);
        assert_eq!(a.details, "Show · S1E2 — Pilot");
        assert_eq!(a.state.as_deref(), Some("Show"));
        assert_eq!(a.end, None); // unknown duration
    }

    #[test]
    fn movie_shows_the_year_and_blank_titles_clear() {
        let p = Presence::Video {
            paused: false,
            title: "Film".into(),
            position_sec: 5.0,
            duration_sec: 100.0,
            artwork_url: None,
            series: None,
            season: None,
            episode: None,
            year: Some(1999),
        };
        let a = build_activity(Some(&p), true, NOW).unwrap();
        assert_eq!(a.state.as_deref(), Some("1999"));
        assert!(build_activity(None, true, NOW).is_none());
        let blank = Presence::Music {
            paused: false,
            title: "  ".into(),
            position_sec: 0.0,
            duration_sec: 0.0,
            artwork_url: None,
            artist: None,
            album: None,
        };
        assert!(build_activity(Some(&blank), true, NOW).is_none());
    }

    #[test]
    fn long_text_is_clipped() {
        let p = Presence::Music {
            paused: false,
            title: "x".repeat(300),
            position_sec: 0.0,
            duration_sec: 0.0,
            artwork_url: None,
            artist: None,
            album: None,
        };
        let a = build_activity(Some(&p), true, NOW).unwrap();
        assert_eq!(a.details.chars().count(), 128);
        assert!(a.details.ends_with('…'));
    }
}
