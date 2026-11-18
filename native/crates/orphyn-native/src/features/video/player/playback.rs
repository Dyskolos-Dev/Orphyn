//! Playback resolution and reporting for the player: the Rust twin of `jellyfin/playback/info.ts`,
//! `jellyfin/playback/reporting.ts` and `jellyfin/segments.ts`.
//!
//! The device profile differs from the React one on purpose: mpv decodes every container and
//! codec, so the server is asked to direct play anything within the bitrate cap, and only falls
//! back to HLS (h264/aac) when it must (bitrate, or a failed direct play, see
//! [`transcode_fallback`]). Embedded subtitles stay in the file (mpv shows them, image ones
//! included); external ones come as WebVTT like in the React app.
//!
//! These endpoints live here, not in `orphyn-jellyfin/src/api/video.rs`, because the player port
//! does not own that file; they can move there unchanged.
use orphyn_jellyfin::client::Q;
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{Client, JfError, JfResult, TICKS_PER_SECOND};
use serde_json::{json, Value};

pub fn secs_to_ticks(s: f64) -> i64 {
    (s * TICKS_PER_SECOND).round() as i64
}

pub fn ticks_to_secs(t: i64) -> f64 {
    t as f64 / TICKS_PER_SECOND
}

/// `PlayMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMethod {
    DirectPlay,
    DirectStream,
    Transcode,
}

impl PlayMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayMethod::DirectPlay => "DirectPlay",
            PlayMethod::DirectStream => "DirectStream",
            PlayMethod::Transcode => "Transcode",
        }
    }
}

/// `MediaSegmentType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentType {
    Intro,
    Outro,
    Recap,
    Preview,
    Commercial,
}

impl SegmentType {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "Intro" => SegmentType::Intro,
            "Outro" => SegmentType::Outro,
            "Recap" => SegmentType::Recap,
            "Preview" => SegmentType::Preview,
            "Commercial" => SegmentType::Commercial,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SegmentType::Intro => "Intro",
            SegmentType::Outro => "Outro",
            SegmentType::Recap => "Recap",
            SegmentType::Preview => "Preview",
            SegmentType::Commercial => "Commercial",
        }
    }
}

/// `MediaSegment` (seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct MediaSegment {
    pub kind: SegmentType,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioTrack {
    pub index: i64,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub title: String,
    pub is_default: bool,
    pub is_external: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleTrack {
    pub index: i64,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub title: String,
    pub is_default: bool,
    pub is_forced: bool,
    pub is_external: bool,
    /// WebVTT URL; `None` for image subtitles (PGS / DVD).
    pub vtt_url: Option<String>,
    pub burn_in_required: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaybackTracks {
    pub audio: Vec<AudioTrack>,
    pub subtitles: Vec<SubtitleTrack>,
    pub default_audio_index: Option<i64>,
    pub default_subtitle_index: Option<i64>,
}

/// `PlaybackInfo` (what the player needs of it).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackInfo {
    pub item_id: String,
    pub url: String,
    pub is_hls: bool,
    pub method: PlayMethod,
    pub play_session_id: String,
    pub media_source_id: String,
    pub run_time_ticks: Option<i64>,
    pub tracks: PlaybackTracks,
    pub segments: Vec<MediaSegment>,
}

/// `PlaybackOptions`.
#[derive(Clone, Debug, Default)]
pub struct InfoOptions {
    pub start_ticks: i64,
    pub audio_index: Option<i64>,
    /// `-1` turns subtitles off; an image subtitle is burned in when transcoding.
    pub subtitle_index: Option<i64>,
    pub max_bitrate: Option<f64>,
}

/// Default cap when the person set none: high enough that any local file direct plays.
const DEFAULT_MAX_BITRATE: f64 = 120_000_000.0;

/// The device profile of mpv: direct play of every container / codec, HLS h264/aac otherwise.
pub fn device_profile(max_bitrate: f64) -> Value {
    let embed: Vec<Value> = [
        "srt", "subrip", "ass", "ssa", "vtt", "webvtt", "mov_text", "pgssub", "pgs", "dvdsub",
        "dvbsub", "sub", "idx",
    ]
    .iter()
    .map(|f| json!({ "Format": f, "Method": "Embed" }))
    .collect();
    let external: Vec<Value> = ["vtt", "srt", "ass", "ssa"]
        .iter()
        .map(|f| json!({ "Format": f, "Method": "External" }))
        .collect();
    json!({
        "Name": "Orphyn native (mpv)",
        "MaxStreamingBitrate": max_bitrate as i64,
        "MaxStaticBitrate": max_bitrate as i64,
        "DirectPlayProfiles": [{ "Type": "Video" }, { "Type": "Audio" }],
        "TranscodingProfiles": [{
            "Container": "ts",
            "Type": "Video",
            "Protocol": "hls",
            "Context": "Streaming",
            "VideoCodec": "h264",
            "AudioCodec": "aac,mp3,ac3",
            "MaxAudioChannels": "6",
            "BreakOnNonKeyFrames": true,
            "MinSegments": 1,
        }],
        "ContainerProfiles": [],
        "CodecProfiles": [],
        "SubtitleProfiles": embed.into_iter().chain(external).collect::<Vec<_>>(),
    })
}

async fn send(
    c: &Client,
    method: &str,
    path: &str,
    params: &[(&str, Option<Q>)],
    body: Option<Value>,
) -> JfResult<Value> {
    let m = method
        .parse()
        .map_err(|_| JfError::Decode(format!("bad method {method}")))?;
    c.request(m, path, params, body).await
}

fn subtitle_url(c: &Client, item_id: &str, source_id: &str, index: i64) -> String {
    c.authed_url(
        &format!("/Videos/{item_id}/{source_id}/Subtitles/{index}/0/Stream.vtt"),
        &[],
    )
}

fn is_image_codec(codec: &str) -> bool {
    matches!(
        codec.to_lowercase().as_str(),
        "pgs" | "pgssub" | "dvdsub" | "dvbsub" | "vobsub" | "xsub"
    )
}

fn describe(s: &Value, fallback: String) -> String {
    let display = s["DisplayTitle"].as_str().unwrap_or("");
    if !display.is_empty() {
        return display.to_string();
    }
    let parts: Vec<String> = [
        s["Language"].as_str().map(str::to_string),
        s["Codec"].as_str().map(str::to_uppercase),
    ]
    .into_iter()
    .flatten()
    .filter(|p| !p.is_empty())
    .collect();
    if parts.is_empty() {
        fallback
    } else {
        parts.join(" · ")
    }
}

/// `buildTracks`.
pub fn build_tracks(c: &Client, item_id: &str, source: &Value) -> PlaybackTracks {
    let source_id = source["Id"].as_str().unwrap_or(item_id);
    let streams = source["MediaStreams"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut t = PlaybackTracks {
        default_audio_index: source["DefaultAudioStreamIndex"].as_i64(),
        default_subtitle_index: source["DefaultSubtitleStreamIndex"].as_i64(),
        ..Default::default()
    };
    for s in &streams {
        let index = s["Index"].as_i64().unwrap_or(0);
        let lang = s["Language"].as_str().map(str::to_string);
        let codec = s["Codec"].as_str().map(str::to_string);
        let external = s["IsExternal"].as_bool().unwrap_or(false);
        match s["Type"].as_str() {
            Some("Audio") => t.audio.push(AudioTrack {
                index,
                language: lang,
                codec,
                title: describe(s, format!("Audio {index}")),
                is_default: s["IsDefault"].as_bool().unwrap_or(false),
                is_external: external,
            }),
            Some("Subtitle") => {
                let image = codec.as_deref().map(is_image_codec).unwrap_or(false)
                    || s["IsTextSubtitleStream"].as_bool() == Some(false);
                t.subtitles.push(SubtitleTrack {
                    index,
                    language: lang,
                    title: describe(s, format!("Subtitle {index}")),
                    codec,
                    is_default: s["IsDefault"].as_bool().unwrap_or(false),
                    is_forced: s["IsForced"].as_bool().unwrap_or(false),
                    is_external: external,
                    vtt_url: (!image).then(|| subtitle_url(c, item_id, source_id, index)),
                    burn_in_required: image,
                });
            }
            _ => {}
        }
    }
    t
}

/// `normalizeSegments`: known types with a valid range, sorted by start.
pub fn normalize_segments(items: &Value) -> Vec<MediaSegment> {
    let mut out: Vec<MediaSegment> = items
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|s| {
            let kind = SegmentType::parse(s["Type"].as_str()?)?;
            let start = s["StartTicks"].as_f64().unwrap_or(0.0) / TICKS_PER_SECOND;
            let end = s["EndTicks"].as_f64().unwrap_or(0.0) / TICKS_PER_SECOND;
            (start.is_finite() && end.is_finite() && end > start).then_some(MediaSegment {
                kind,
                start,
                end,
            })
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// `fetchMediaSegments`: no segments (old server, feature off, error) is a valid answer.
pub async fn media_segments(s: &Session, item_id: &str) -> Vec<MediaSegment> {
    match send(
        &s.client,
        "GET",
        &format!("/MediaSegments/{item_id}"),
        &[],
        None,
    )
    .await
    {
        Ok(v) => normalize_segments(&v["Items"]),
        Err(_) => Vec::new(),
    }
}

/// `getPlaybackInfo` for video: POST /Items/{id}/PlaybackInfo, then the URL to load.
pub async fn playback_info(
    s: &Session,
    item_id: &str,
    o: &InfoOptions,
) -> Result<PlaybackInfo, String> {
    let max = o.max_bitrate.unwrap_or(DEFAULT_MAX_BITRATE);
    let start = (o.start_ticks > 0).then_some(o.start_ticks);
    let params: Vec<(&str, Option<Q>)> = vec![
        ("userId", Some(Q::Str(s.user_id.clone()))),
        ("startTimeTicks", start.map(|v| Q::Str(v.to_string()))),
        (
            "audioStreamIndex",
            o.audio_index.map(|v| Q::Str(v.to_string())),
        ),
        (
            "subtitleStreamIndex",
            o.subtitle_index.map(|v| Q::Str(v.to_string())),
        ),
        (
            "maxStreamingBitrate",
            Some(Q::Str((max as i64).to_string())),
        ),
        ("autoOpenLiveStream", Some(true.into())),
    ];
    let body = json!({
        "DeviceProfile": device_profile(max),
        "UserId": s.user_id,
        "StartTimeTicks": start,
        "AudioStreamIndex": o.audio_index,
        "SubtitleStreamIndex": o.subtitle_index,
        "MaxStreamingBitrate": max as i64,
    });
    let path = format!("/Items/{item_id}/PlaybackInfo");
    let (res, segments) = tokio::join!(
        send(&s.client, "POST", &path, &params, Some(body)),
        media_segments(s, item_id)
    );
    let res = res.map_err(|e| e.to_string())?;
    if let Some(code) = res["ErrorCode"].as_str() {
        return Err(format!("Playback refused by the server: {code}"));
    }
    let source = res["MediaSources"]
        .get(0)
        .cloned()
        .ok_or_else(|| "No media source for this item.".to_string())?;
    let play_session_id = res["PlaySessionId"].as_str().unwrap_or("").to_string();
    let media_source_id = source["Id"].as_str().unwrap_or(item_id).to_string();
    let c = &s.client;
    let (url, method) = if source["SupportsDirectPlay"].as_bool() == Some(true) {
        let url = c.authed_url(
            &format!("/Videos/{item_id}/stream"),
            &[
                ("static", Some(true.into())),
                ("mediaSourceId", Some(Q::Str(media_source_id.clone()))),
                ("deviceId", Some(Q::Str(c.device_id().to_string()))),
                ("playSessionId", Some(Q::Str(play_session_id.clone()))),
                (
                    "tag",
                    source["ETag"].as_str().map(|t| Q::Str(t.to_string())),
                ),
            ],
        );
        (url, PlayMethod::DirectPlay)
    } else if let Some(tu) = source["TranscodingUrl"].as_str() {
        let lower = tu.to_lowercase();
        let has_key = lower.contains("api_key=") || lower.contains("apikey=");
        let sep = if tu.contains('?') { '&' } else { '?' };
        let key = if has_key {
            String::new()
        } else {
            format!(
                "{sep}ApiKey={}",
                orphyn_jellyfin::client::encode_component(c.token().unwrap_or(""))
            )
        };
        let method = if source["SupportsDirectStream"].as_bool() == Some(true) {
            PlayMethod::DirectStream
        } else {
            PlayMethod::Transcode
        };
        (format!("{}{tu}{key}", c.base_url()), method)
    } else {
        return Err("The server offers no playable stream for this item.".into());
    };
    Ok(PlaybackInfo {
        item_id: item_id.to_string(),
        is_hls: url.to_lowercase().contains(".m3u8")
            || source["TranscodingSubProtocol"].as_str() == Some("hls"),
        url,
        method,
        play_session_id,
        tracks: build_tracks(c, item_id, &source),
        media_source_id,
        run_time_ticks: source["RunTimeTicks"].as_i64(),
        segments,
    })
}

/// `transcodeFallback` (`streamUrlForCast(..., { transcode: true })`): HLS h264/aac.
pub fn transcode_fallback(
    s: &Session,
    info: &PlaybackInfo,
    max_bitrate: Option<f64>,
) -> PlaybackInfo {
    let c = &s.client;
    let url = c.authed_url(
        &format!("/Videos/{}/master.m3u8", info.item_id),
        &[
            ("mediaSourceId", Some(Q::Str(info.media_source_id.clone()))),
            ("deviceId", Some(Q::Str(c.device_id().to_string()))),
            ("playSessionId", Some(Q::Str(info.play_session_id.clone()))),
            ("videoCodec", Some("h264".into())),
            ("audioCodec", Some("aac".into())),
            ("segmentContainer", Some("ts".into())),
            ("maxAudioChannels", Some(2u32.into())),
            ("minSegments", Some(1u32.into())),
            ("breakOnNonKeyFrames", Some(true.into())),
            (
                "videoBitRate",
                max_bitrate.map(|b| Q::Str((b as i64).to_string())),
            ),
        ],
    );
    PlaybackInfo {
        url,
        is_hls: true,
        method: PlayMethod::Transcode,
        ..info.clone()
    }
}

/// `ReportTarget` + `PlaybackState`.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub item_id: String,
    pub media_source_id: String,
    pub play_session_id: String,
    pub method: PlayMethod,
    pub position_ticks: i64,
    pub is_paused: bool,
    pub is_muted: bool,
    pub volume_level: i64,
    pub audio_index: Option<i64>,
    pub subtitle_index: Option<i64>,
}

/// `ReportEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportKind {
    Start,
    /// Progress with an event name (`timeupdate`, `pause`, `unpause`).
    Progress(&'static str),
    Stopped,
}

pub fn report_body(r: &Report, event: Option<&str>) -> Value {
    json!({
        "ItemId": r.item_id,
        "MediaSourceId": r.media_source_id,
        "PlaySessionId": (!r.play_session_id.is_empty()).then_some(&r.play_session_id),
        "PlayMethod": r.method.as_str(),
        "PositionTicks": r.position_ticks.max(0),
        "IsPaused": r.is_paused,
        "IsMuted": r.is_muted,
        "VolumeLevel": r.volume_level,
        "AudioStreamIndex": r.audio_index,
        "SubtitleStreamIndex": r.subtitle_index,
        "CanSeek": true,
        "EventName": event,
    })
}

/// POST /Sessions/Playing[/Progress|/Stopped]; failures are logged, never raised to the player.
pub async fn report(c: &Client, kind: ReportKind, r: &Report) {
    let (path, event) = match kind {
        ReportKind::Start => ("/Sessions/Playing", None),
        ReportKind::Progress(e) => ("/Sessions/Playing/Progress", Some(e)),
        ReportKind::Stopped => ("/Sessions/Playing/Stopped", None),
    };
    match send(c, "POST", path, &[], Some(report_body(r, event))).await {
        Ok(_) => log::info!(
            "[video] reported {path} {:?} at {}",
            event,
            r.position_ticks
        ),
        Err(e) => log::warn!("[video] report {path}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_filtered_and_sorted() {
        let v = json!([
            { "Type": "Outro", "StartTicks": 12_000_000_000i64, "EndTicks": 13_000_000_000i64 },
            { "Type": "Intro", "StartTicks": 100_000_000, "EndTicks": 700_000_000 },
            { "Type": "Unknown", "StartTicks": 0, "EndTicks": 10 },
            { "Type": "Recap", "StartTicks": 50, "EndTicks": 50 },
        ]);
        let s = normalize_segments(&v);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, SegmentType::Intro);
        assert_eq!((s[0].start, s[0].end), (10.0, 70.0));
        assert_eq!(s[1].kind, SegmentType::Outro);
    }

    #[test]
    fn tracks_mark_image_subtitles_for_burn_in() {
        let c = Client::new("https://jf.example", "dev", Some("tok"));
        let source = json!({
            "Id": "src",
            "DefaultAudioStreamIndex": 1,
            "MediaStreams": [
                { "Type": "Video", "Index": 0 },
                { "Type": "Audio", "Index": 1, "Language": "eng", "Codec": "aac", "IsDefault": true },
                { "Type": "Subtitle", "Index": 2, "Codec": "pgssub", "Language": "fre" },
                { "Type": "Subtitle", "Index": 3, "Codec": "subrip", "DisplayTitle": "English", "IsExternal": true },
            ]
        });
        let t = build_tracks(&c, "it", &source);
        assert_eq!(t.audio.len(), 1);
        assert_eq!(t.audio[0].title, "eng · AAC");
        assert_eq!(t.default_audio_index, Some(1));
        assert!(t.subtitles[0].burn_in_required && t.subtitles[0].vtt_url.is_none());
        assert_eq!(
            t.subtitles[1].vtt_url.as_deref(),
            Some("https://jf.example/Videos/it/src/Subtitles/3/0/Stream.vtt?api_key=tok")
        );
        assert!(t.subtitles[1].is_external);
    }

    #[test]
    fn report_body_matches_reporting_ts() {
        let r = Report {
            item_id: "i".into(),
            media_source_id: "m".into(),
            play_session_id: String::new(),
            method: PlayMethod::DirectPlay,
            position_ticks: -5,
            is_paused: true,
            is_muted: false,
            volume_level: 80,
            audio_index: Some(1),
            subtitle_index: None,
        };
        let b = report_body(&r, Some("pause"));
        assert_eq!(b["PositionTicks"], 0);
        assert_eq!(b["PlayMethod"], "DirectPlay");
        assert!(b["PlaySessionId"].is_null());
        assert_eq!(b["EventName"], "pause");
        assert_eq!(secs_to_ticks(1.5), 15_000_000);
        assert_eq!(ticks_to_secs(25_000_000), 2.5);
    }
}
