//! Name matching and placeholder filtering, kept free of I/O so it is unit tested.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct DeezerArtist {
    pub name: Option<String>,
    pub picture_xl: Option<String>,
    pub picture_big: Option<String>,
}

#[derive(Deserialize)]
pub struct AudioDbArtist {
    #[serde(rename = "strArtist")]
    pub name: Option<String>,
    #[serde(rename = "strArtistThumb")]
    pub thumb: Option<String>,
}

fn fold(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => "i",
        'ł' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        _ => return None,
    })
}

/// Same idea as `artistKey` in the renderer (lowercase, accents folded, leading "the" dropped),
/// plus every non-alphanumeric character and space removed so "AC/DC" equals "AC-DC".
pub fn normalize(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if let Some(f) = fold(c) {
            out.push_str(f);
        } else if c.is_alphanumeric() {
            out.push(c);
        } else if c.is_whitespace() {
            out.push(' ');
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = match collapsed.strip_prefix("the ") {
        Some(rest) if !rest.is_empty() => rest,
        _ => collapsed.as_str(),
    };
    trimmed.replace(' ', "")
}

/// Deezer serves a grey default picture whose URL has an empty md5 (`.../artist//1000x1000-...`).
pub fn is_placeholder(url: &str) -> bool {
    url.trim().is_empty() || url.contains("/artist//")
}

fn good_https(url: &str) -> Option<String> {
    let u = url.trim();
    if u.is_empty() || is_placeholder(u) {
        return None;
    }
    let u = u.replacen("http://", "https://", 1);
    u.starts_with("https://").then_some(u)
}

/// The picture of the first Deezer result whose name equals `wanted` once normalised.
pub fn pick_deezer(wanted: &str, results: &[DeezerArtist]) -> Option<String> {
    let key = normalize(wanted);
    results
        .iter()
        .filter(|a| a.name.as_deref().map(normalize).as_deref() == Some(key.as_str()))
        .find_map(|a| {
            a.picture_xl
                .as_deref()
                .and_then(good_https)
                .or_else(|| a.picture_big.as_deref().and_then(good_https))
        })
}

pub fn pick_audiodb(wanted: &str, results: &[AudioDbArtist]) -> Option<String> {
    let key = normalize(wanted);
    results
        .iter()
        .filter(|a| a.name.as_deref().map(normalize).as_deref() == Some(key.as_str()))
        .find_map(|a| a.thumb.as_deref().and_then(good_https))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dz(name: &str, xl: &str) -> DeezerArtist {
        DeezerArtist {
            name: Some(name.into()),
            picture_xl: Some(xl.into()),
            picture_big: None,
        }
    }

    const REAL: &str =
        "https://cdn-images.dzcdn.net/images/artist/abc123/1000x1000-000000-80-0-0.jpg";
    const GOOD: &str =
        "https://cdn-images.dzcdn.net/images/artist/good/1000x1000-000000-80-0-0.jpg";
    const EMPTY: &str = "https://cdn-images.dzcdn.net/images/artist//1000x1000-000000-80-0-0.jpg";

    #[test]
    fn normalizes_case_accents_punctuation_and_the() {
        assert_eq!(normalize("Beyoncé"), "beyonce");
        assert_eq!(normalize("The  Beatles"), "beatles");
        assert_eq!(normalize("AC/DC"), normalize("ac-dc"));
        assert_eq!(normalize("   "), "");
    }

    #[test]
    fn placeholder_urls_are_detected() {
        assert!(is_placeholder(EMPTY));
        assert!(!is_placeholder(REAL));
    }

    #[test]
    fn prefers_the_exact_name_over_a_closer_ranked_result() {
        let results = vec![dz("Daft Punk Tribute", REAL), dz("Daft Punk", GOOD)];
        assert_eq!(pick_deezer("daft punk", &results).as_deref(), Some(GOOD));
    }

    #[test]
    fn rejects_non_exact_matches() {
        assert_eq!(pick_deezer("Muse", &[dz("Muse Tribute Band", REAL)]), None);
    }

    #[test]
    fn skips_placeholder_pictures() {
        assert_eq!(pick_deezer("Muse", &[dz("Muse", EMPTY)]), None);
    }

    #[test]
    fn falls_back_to_picture_big() {
        let a = DeezerArtist {
            name: Some("Muse".into()),
            picture_xl: Some(EMPTY.into()),
            picture_big: Some(REAL.into()),
        };
        assert_eq!(pick_deezer("Muse", &[a]).as_deref(), Some(REAL));
    }

    #[test]
    fn audiodb_needs_exact_name_and_a_thumb() {
        let a = |n: &str, t: Option<&str>| AudioDbArtist {
            name: Some(n.into()),
            thumb: t.map(Into::into),
        };
        assert_eq!(pick_audiodb("Muse", &[a("Muse", None)]), None);
        assert_eq!(pick_audiodb("Muse", &[a("Mute", Some(REAL))]), None);
        assert_eq!(
            pick_audiodb("Muse", &[a("Muse", Some(REAL))]).as_deref(),
            Some(REAL)
        );
    }
}
