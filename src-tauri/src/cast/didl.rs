//! DIDL-Lite metadata and UPnP time formats.

use super::model::{CastMedia, CastMediaKind};
use super::xml::escape_xml;

/// DLNA.ORG_FLAGS: streaming transfer mode, background transfer, connection stall, DLNA v1.5.
const DLNA_FLAGS: &str = "01700000000000000000000000000000";

pub fn dlna_profile_for(mime_type: &str) -> Option<&'static str> {
    match mime_type.to_lowercase().as_str() {
        "audio/mpeg" | "audio/mp3" => Some("MP3"),
        "audio/mp4" | "audio/x-m4a" | "audio/aac" => Some("AAC_ISO_320"),
        _ => None,
    }
}

/// protocolInfo `<protocol>:<network>:<contentFormat>:<additionalInfo>` for an http-get resource.
pub fn build_protocol_info(mime_type: &str) -> String {
    let pn = dlna_profile_for(mime_type)
        .map(|p| format!("DLNA.ORG_PN={p};"))
        .unwrap_or_default();
    format!("http-get:*:{mime_type}:{pn}DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS={DLNA_FLAGS}")
}

/// `H:MM:SS` (UPnP REL_TIME / duration format).
pub fn format_upnp_time(total_sec: f64) -> String {
    let s = if total_sec.is_finite() {
        total_sec.round().max(0.0) as u64
    } else {
        0
    };
    format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Parses `H:MM:SS(.fff)` into seconds; `NOT_IMPLEMENTED` and garbage give 0.
pub fn parse_upnp_time(value: Option<&str>) -> f64 {
    let Some(v) = value else { return 0.0 };
    let mut parts = v.trim().split(':');
    let (Some(h), Some(m), Some(s), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return 0.0;
    };
    let digits = |x: &str, max: usize| {
        !x.is_empty() && x.len() <= max && x.bytes().all(|b| b.is_ascii_digit())
    };
    let (sec, frac) = s.split_once('.').map_or((s, None), |(a, b)| (a, Some(b)));
    if !digits(h, 9) || !digits(m, 2) || !digits(sec, 2) || frac.is_some_and(|f| !digits(f, 9)) {
        return 0.0;
    }
    let base = h.parse::<f64>().unwrap_or(0.0) * 3600.0
        + m.parse::<f64>().unwrap_or(0.0) * 60.0
        + sec.parse::<f64>().unwrap_or(0.0);
    base + frac
        .and_then(|f| format!("0.{f}").parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// DIDL-Lite document for one item, with title/artist/album/albumArtURI and a protocolInfo resource.
pub fn build_didl(media: &CastMedia) -> String {
    let class = if media.kind == CastMediaKind::Video {
        "object.item.videoItem.movie"
    } else {
        "object.item.audioItem.musicTrack"
    };
    let mut parts = vec![
        format!("<dc:title>{}</dc:title>", escape_xml(&media.title)),
        format!("<upnp:class>{class}</upnp:class>"),
    ];
    if let Some(artist) = media.artist.as_deref().filter(|a| !a.is_empty()) {
        parts.push(format!("<dc:creator>{}</dc:creator>", escape_xml(artist)));
        parts.push(format!("<upnp:artist>{}</upnp:artist>", escape_xml(artist)));
    }
    if let Some(album) = media.album.as_deref().filter(|a| !a.is_empty()) {
        parts.push(format!("<upnp:album>{}</upnp:album>", escape_xml(album)));
    }
    if let Some(art) = media.artwork_url.as_deref().filter(|a| !a.is_empty()) {
        parts.push(format!(
            "<upnp:albumArtURI>{}</upnp:albumArtURI>",
            escape_xml(art)
        ));
    }
    let duration = media
        .duration_sec
        .filter(|d| *d > 0.0)
        .map(|d| format!(" duration=\"{}\"", format_upnp_time(d)))
        .unwrap_or_default();
    parts.push(format!(
        "<res protocolInfo=\"{}\"{duration}>{}</res>",
        escape_xml(&build_protocol_info(&media.mime_type)),
        escape_xml(&media.url)
    ));
    format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" \
         xmlns:dlna=\"urn:schemas-dlna-org:metadata-1-0/\">\
         <item id=\"0\" parentID=\"-1\" restricted=\"1\">{}</item></DIDL-Lite>",
        parts.join("")
    )
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::cast::xml::{find_node, parse_xml};

    pub fn rich_media() -> CastMedia {
        CastMedia {
            url: "http://jelly.local:8096/Audio/1/stream?static=true&api_key=abc".into(),
            mime_type: "audio/flac".into(),
            kind: CastMediaKind::Audio,
            title: "Tom & Jerry <live> \"2024\"".into(),
            artist: Some("Guns N' Roses".into()),
            album: Some("A > B".into()),
            artwork_url: Some(
                "http://jelly.local:8096/Items/1/Images/Primary?tag=x&maxWidth=300".into(),
            ),
            duration_sec: Some(3725.0),
            start_position_sec: None,
        }
    }

    #[test]
    fn builds_metadata_with_escaped_fields_art_protocol_info_and_duration() {
        let didl = build_didl(&rich_media());
        assert!(didl.contains("<dc:title>Tom &amp; Jerry &lt;live&gt; &quot;2024&quot;</dc:title>"));
        assert!(didl.contains("<upnp:artist>Guns N&apos; Roses</upnp:artist>"));
        assert!(didl.contains("<upnp:album>A &gt; B</upnp:album>"));
        assert!(didl.contains(
            "albumArtURI>http://jelly.local:8096/Items/1/Images/Primary?tag=x&amp;maxWidth=300<"
        ));
        assert!(didl.contains("object.item.audioItem.musicTrack"));
        assert!(didl.contains("protocolInfo=\"http-get:*:audio/flac:"));
        assert!(didl.contains("duration=\"1:02:05\""));
        assert!(didl.contains("stream?static=true&amp;api_key=abc</res>"));
    }

    #[test]
    fn is_well_formed_xml_that_round_trips_the_title() {
        let media = rich_media();
        let root = parse_xml(&build_didl(&media));
        assert_eq!(find_node(&root, "title").unwrap().text, media.title);
    }

    #[test]
    fn uses_the_movie_class_for_video_and_adds_a_dlna_profile_for_mp3() {
        let m = CastMedia {
            kind: CastMediaKind::Video,
            mime_type: "audio/mpeg".into(),
            ..rich_media()
        };
        let d = build_didl(&m);
        assert!(d.contains("object.item.videoItem.movie"));
        assert!(d.contains("DLNA.ORG_PN=MP3"));
    }

    #[test]
    fn formats_and_parses_upnp_times() {
        assert_eq!(format_upnp_time(65.4), "0:01:05");
        assert_eq!(parse_upnp_time(Some("1:02:05")), 3725.0);
        assert_eq!(parse_upnp_time(Some("0:00:01.500")), 1.5);
        assert_eq!(parse_upnp_time(Some("NOT_IMPLEMENTED")), 0.0);
        assert_eq!(parse_upnp_time(Some("-0:00:05")), 0.0);
        assert_eq!(parse_upnp_time(None), 0.0);
    }
}
