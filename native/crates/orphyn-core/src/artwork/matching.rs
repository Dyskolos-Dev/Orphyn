//! Name matching and placeholder filtering, kept free of I/O so it is unit tested.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct DeezerArtist {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub picture_xl: Option<String>,
    pub picture_big: Option<String>,
}

#[derive(Deserialize)]
pub struct DeezerTrack {
    pub title: Option<String>,
    pub title_short: Option<String>,
}

#[derive(Deserialize)]
pub struct AudioDbArtist {
    #[serde(rename = "strArtist")]
    pub name: Option<String>,
    #[serde(rename = "strArtistThumb")]
    pub thumb: Option<String>,
    #[serde(rename = "strBiographyEN")]
    pub bio_en: Option<String>,
    #[serde(rename = "strBiographyFR")]
    pub bio_fr: Option<String>,
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

/// Id of the first Deezer result whose name equals `wanted` once normalised (any picture).
pub fn pick_deezer_id(wanted: &str, results: &[DeezerArtist]) -> Option<u64> {
    let key = normalize(wanted);
    results
        .iter()
        .find(|a| a.name.as_deref().map(normalize).as_deref() == Some(key.as_str()))
        .and_then(|a| a.id)
}

/// Track titles in Deezer order, blanks and exact repeats dropped.
pub fn track_titles(tracks: &[DeezerTrack]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tracks {
        let title = t
            .title
            .as_deref()
            .or(t.title_short.as_deref())
            .unwrap_or("")
            .trim();
        if !title.is_empty() && !out.iter().any(|x| x == title) {
            out.push(title.to_string());
        }
    }
    out
}

/// Names of the related artists, blanks dropped.
pub fn related_names(artists: &[DeezerArtist]) -> Vec<String> {
    artists
        .iter()
        .filter_map(|a| a.name.as_deref())
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .collect()
}

/// TheAudioDB biography of the exact-name match in `lang` ("fr", anything else = English),
/// falling back to the other language when the wanted one is empty.
pub fn pick_audiodb_bio(wanted: &str, lang: &str, results: &[AudioDbArtist]) -> Option<String> {
    let key = normalize(wanted);
    let a = results
        .iter()
        .find(|a| a.name.as_deref().map(normalize).as_deref() == Some(key.as_str()))?;
    let (first, second) = if lang == "fr" {
        (&a.bio_fr, &a.bio_en)
    } else {
        (&a.bio_en, &a.bio_fr)
    };
    [first, second]
        .into_iter()
        .filter_map(|b| b.as_deref())
        .map(str::trim)
        .find(|b| !b.is_empty())
        .map(String::from)
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
            id: Some(7),
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
            id: None,
            name: Some("Muse".into()),
            picture_xl: Some(EMPTY.into()),
            picture_big: Some(REAL.into()),
        };
        assert_eq!(pick_deezer("Muse", &[a]).as_deref(), Some(REAL));
    }

    #[test]
    fn deezer_id_needs_the_exact_name() {
        let results = vec![dz("Muse Tribute", REAL), dz("Muse", EMPTY)];
        assert_eq!(pick_deezer_id("muse", &results), Some(7));
        assert_eq!(pick_deezer_id("Mute", &results), None);
    }

    #[test]
    fn track_titles_drop_blanks_and_repeats() {
        let t = |s: &str| DeezerTrack {
            title: Some(s.into()),
            title_short: None,
        };
        assert_eq!(
            track_titles(&[t("Hysteria"), t(" "), t("Hysteria"), t("Starlight")]),
            vec!["Hysteria", "Starlight"]
        );
    }

    #[test]
    fn related_names_skip_blanks() {
        let r = vec![dz("Placebo", REAL), dz("  ", REAL)];
        assert_eq!(related_names(&r), vec!["Placebo"]);
    }

    #[test]
    fn audiodb_bio_prefers_the_language_then_falls_back() {
        let a = AudioDbArtist {
            name: Some("Muse".into()),
            thumb: None,
            bio_en: Some("English bio".into()),
            bio_fr: Some("  ".into()),
        };
        assert_eq!(
            pick_audiodb_bio("Muse", "fr", std::slice::from_ref(&a)).as_deref(),
            Some("English bio")
        );
        assert_eq!(pick_audiodb_bio("Other", "en", &[a]), None);
    }

    #[test]
    fn audiodb_needs_exact_name_and_a_thumb() {
        let a = |n: &str, t: Option<&str>| AudioDbArtist {
            name: Some(n.into()),
            thumb: t.map(Into::into),
            bio_en: None,
            bio_fr: None,
        };
        assert_eq!(pick_audiodb("Muse", &[a("Muse", None)]), None);
        assert_eq!(pick_audiodb("Muse", &[a("Mute", Some(REAL))]), None);
        assert_eq!(
            pick_audiodb("Muse", &[a("Muse", Some(REAL))]).as_deref(),
            Some(REAL)
        );
    }
}
