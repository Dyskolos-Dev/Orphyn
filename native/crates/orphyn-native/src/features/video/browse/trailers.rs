//! `trailers.ts`: which trailers a detail page offers, and how each one plays.
use orphyn_jellyfin::api::video::RemoteTrailer;
use orphyn_jellyfin::BaseItemDto;

/// A trailer the detail page can offer.
#[derive(Clone, Debug, PartialEq)]
pub enum TrailerSource {
    /// A file on the server, played in Orphyn's own player.
    Local {
        key: String,
        name: String,
        item_id: String,
    },
    /// A YouTube video. There is no web engine here: it opens in the default browser.
    YouTube {
        key: String,
        name: String,
        video_id: String,
    },
    /// Any other web page: opened in the default browser.
    External {
        key: String,
        name: String,
        url: String,
    },
}

impl TrailerSource {
    pub fn name(&self) -> &str {
        match self {
            Self::Local { name, .. } | Self::YouTube { name, .. } | Self::External { name, .. } => {
                name
            }
        }
    }
}

/// `https://www.youtube.com/watch?v=<id>` for a video id.
pub fn youtube_watch_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

fn is_video_id(s: &str) -> bool {
    s.len() == 11
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The video id of a youtube.com / youtu.be / embed / shorts link, else `None` (`youtubeId`).
pub fn youtube_id(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host).to_lowercase();
    let host = ["www.", "m.", "music."]
        .iter()
        .find_map(|p| host.strip_prefix(p))
        .unwrap_or(&host);
    let tail = tail.split('#').next().unwrap_or("");
    let (path, query) = tail.split_once('?').unwrap_or((tail, ""));
    let id = if host == "youtu.be" {
        path.split('/').nth(1).map(str::to_string)
    } else if host == "youtube.com" || host == "youtube-nocookie.com" {
        if path == "/watch" {
            query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == "v")
                .map(|(_, v)| v.to_string())
        } else {
            let mut parts = path.split('/').skip(1);
            match (parts.next(), parts.next()) {
                (Some("embed" | "shorts" | "v"), Some(id)) => Some(id.to_string()),
                _ => None,
            }
        }
    } else {
        None
    };
    id.filter(|i| is_video_id(i))
}

fn is_web_url(url: &str) -> bool {
    let l = url.to_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// Every trailer of an item, best first: files on the server, then YouTube links, then other
/// links; the same video listed twice is kept once (`pickTrailerSources`).
pub fn pick_trailer_sources(local: &[BaseItemDto], remote: &[RemoteTrailer]) -> Vec<TrailerSource> {
    let mut out: Vec<TrailerSource> = local
        .iter()
        .filter(|t| !t.id.is_empty())
        .map(|t| TrailerSource::Local {
            key: format!("local:{}", t.id),
            name: t.name().to_string(),
            item_id: t.id.clone(),
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut others = Vec::new();
    for r in remote {
        let Some(url) = r.url.as_deref().map(str::trim).filter(|u| is_web_url(u)) else {
            continue;
        };
        let video_id = youtube_id(url);
        let key = match &video_id {
            Some(id) => format!("youtube:{id}"),
            None => format!("url:{url}"),
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        let name = r.name.clone().unwrap_or_default();
        match video_id {
            Some(video_id) => out.push(TrailerSource::YouTube {
                key,
                name,
                video_id,
            }),
            None => others.push(TrailerSource::External {
                key,
                name,
                url: url.to_string(),
            }),
        }
    }
    out.extend(others);
    out
}

/// Opens a web page in the default browser (`window.open(url, '_blank')` of the React app).
/// Only http(s) links are ever opened.
pub fn open_external(url: &str) {
    if !is_web_url(url) {
        return;
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = result {
        log::warn!("[trailers] could not open {url}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_links_of_every_shape_give_their_id() {
        let id = Some("dQw4w9WgXcQ".to_string());
        for u in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?t=3",
            "https://m.youtube.com/watch?feature=x&v=dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "https://youtube.com/shorts/dQw4w9WgXcQ",
        ] {
            assert_eq!(youtube_id(u), id, "{u}");
        }
        assert_eq!(youtube_id("https://vimeo.com/123"), None);
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=short"), None);
        assert_eq!(youtube_id("not a url"), None);
    }

    #[test]
    fn sources_are_ordered_and_deduplicated() {
        let local: Vec<BaseItemDto> =
            vec![serde_json::from_value(serde_json::json!({"Id":"t1","Name":"Local"})).unwrap()];
        let remote = vec![
            RemoteTrailer {
                url: Some("https://vimeo.com/1".into()),
                name: Some("V".into()),
            },
            RemoteTrailer {
                url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ".into()),
                name: Some("Y".into()),
            },
            RemoteTrailer {
                url: Some("https://youtu.be/dQw4w9WgXcQ".into()),
                name: Some("Y again".into()),
            },
            RemoteTrailer {
                url: Some("ftp://nope".into()),
                name: None,
            },
        ];
        let s = pick_trailer_sources(&local, &remote);
        assert_eq!(s.len(), 3);
        assert!(matches!(s[0], TrailerSource::Local { .. }));
        assert!(matches!(s[1], TrailerSource::YouTube { .. }));
        assert!(matches!(s[2], TrailerSource::External { .. }));
    }
}
