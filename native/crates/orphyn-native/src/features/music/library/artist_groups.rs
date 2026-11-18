//! `library/artistGroups.ts`: client-side artist grouping (a pure view, nothing is written to the
//! server). Spelling variants collide on `artist_key`; credits are cut by `split_artist_credit`.
use std::collections::{HashMap, HashSet};

use orphyn_jellyfin::BaseItemDto;

/// Lowercase, accents stripped, apostrophes unified, spaces collapsed, leading "the" removed.
pub fn artist_key(name: &str) -> String {
    let folded: String = name
        .chars()
        .map(fold_accent)
        .flat_map(|c| c.to_lowercase())
        .map(|c| {
            if matches!(c, '’' | '‘' | '`' | '´') {
                '\''
            } else {
                c
            }
        })
        .collect();
    let collapsed = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.strip_prefix("the ") {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => collapsed,
    }
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// Position and length of a `feat` / `ft` / `featuring` marker (with its surrounding spaces and
/// opening bracket), found on word boundaries, case-insensitively.
fn find_feat(s: &str) -> Option<(usize, usize)> {
    let lower = s.to_lowercase();
    if lower.len() != s.len() {
        return None;
    }
    let bytes = lower.as_bytes();
    for word in ["featuring", "feat", "ft"] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(word) {
            let start = from + i;
            let end = start + word.len();
            let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
            let mut after = end;
            if bytes.get(after) == Some(&b'.') {
                after += 1;
            }
            let space = after < bytes.len() && bytes[after].is_ascii_whitespace();
            if before_ok && space && bytes.get(end).is_none_or(|b| !b.is_ascii_alphanumeric()) {
                // extend over leading spaces / bracket and trailing spaces
                let mut a = start;
                while a > 0 && bytes[a - 1].is_ascii_whitespace() {
                    a -= 1;
                }
                if a > 0 && matches!(bytes[a - 1], b'(' | b'[') {
                    a -= 1;
                }
                let mut b = after;
                while b < bytes.len() && bytes[b].is_ascii_whitespace() {
                    b += 1;
                }
                return Some((a, b));
            }
            from = end;
        }
    }
    None
}

fn split_joiners(s: &str) -> Vec<String> {
    let mut parts = vec![s.to_string()];
    for sep in [";", ",", "&", "+", "/", " × ", " x ", " with ", " and "] {
        parts = parts
            .into_iter()
            .flat_map(|p| {
                if sep.starts_with(' ') {
                    p.split(sep).map(str::to_string).collect::<Vec<_>>()
                } else {
                    p.split(sep).map(str::to_string).collect()
                }
            })
            .collect();
    }
    parts
}

pub struct SplitArtists {
    pub names: Vec<String>,
    /// The credit used feat./ft./featuring: its head is certainly the main act.
    pub featured: bool,
}

pub fn split_artist_credit(credit: &str) -> SplitArtists {
    let mut chunks = Vec::new();
    let mut rest = credit;
    let mut featured = false;
    while let Some((a, b)) = find_feat(rest) {
        featured = true;
        chunks.push(&rest[..a]);
        rest = &rest[b..];
    }
    chunks.push(rest);
    let mut names = Vec::new();
    for chunk in chunks {
        for p in split_joiners(chunk) {
            let clean = p.trim().trim_end_matches([')', ']']).trim().to_string();
            if !clean.is_empty() {
                names.push(clean);
            }
        }
    }
    if names.is_empty() {
        names.push(credit.trim().to_string());
    }
    SplitArtists { names, featured }
}

pub struct ArtistGroup {
    pub item: BaseItemDto,
    pub ids: Vec<String>,
}

/// One group per real artist: `album_artists` root the groups, `everyone` attaches variants.
pub fn group_artists(album_artists: &[BaseItemDto], everyone: &[BaseItemDto]) -> Vec<ArtistGroup> {
    let known: HashSet<String> = album_artists
        .iter()
        .chain(everyone)
        .filter_map(|a| a.name.as_deref().filter(|n| !n.is_empty()).map(artist_key))
        .collect();
    let group_key_of = |name: &str| -> String {
        let own = artist_key(name);
        let split = split_artist_credit(name);
        let Some(head) = split.names.first() else {
            return own;
        };
        if split.names.len() < 2 {
            return own;
        }
        let head_key = artist_key(head);
        if split.featured || known.contains(&head_key) {
            head_key
        } else {
            own
        }
    };
    let rank = |a: &BaseItemDto| -> u8 {
        let composite = split_artist_credit(a.name()).names.len() > 1;
        (if composite { 2 } else { 0 }) + u8::from(!a.image_tags.contains_key("Primary"))
    };
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, ArtistGroup> = HashMap::new();
    let mut add = |a: &BaseItemDto, root: bool| {
        let Some(name) = a.name.as_deref().filter(|n| !n.is_empty()) else {
            return;
        };
        if a.id.is_empty() {
            return;
        }
        let key = group_key_of(name);
        match groups.get_mut(&key) {
            None => {
                if root {
                    order.push(key.clone());
                    groups.insert(
                        key,
                        ArtistGroup {
                            item: a.clone(),
                            ids: vec![a.id.clone()],
                        },
                    );
                }
            }
            Some(g) => {
                if !g.ids.contains(&a.id) {
                    g.ids.push(a.id.clone());
                }
                if root && rank(a) < rank(&g.item) {
                    g.item = a.clone();
                    let mut ids = vec![a.id.clone()];
                    ids.extend(g.ids.iter().filter(|i| **i != a.id).cloned());
                    g.ids = ids;
                }
            }
        }
    };
    for a in album_artists {
        add(a, true);
    }
    for a in everyone {
        add(a, false);
    }
    order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .collect()
}

/// The two full artist lists (`useArtistLists`), shared through the query cache.
pub struct ArtistLists {
    pub album_artists: Option<std::sync::Arc<Vec<BaseItemDto>>>,
    pub everyone: Option<std::sync::Arc<Vec<BaseItemDto>>>,
    pub error: bool,
}

pub fn use_artist_lists(
    session: &orphyn_jellyfin::endpoints::Session,
    enabled: bool,
) -> ArtistLists {
    use orphyn_jellyfin::api::music::ArtistList;
    use orphyn_jellyfin::keys;
    use orphyn_jellyfin::ItemsParams;

    let load = |kind: ArtistList, tag: &str| {
        let mut key = keys::items(
            &session.account_id,
            &ItemsParams {
                page_size: Some(20000),
                ..ItemsParams::default()
            },
        );
        key.insert(2, tag.to_string());
        let s = session.clone();
        crate::query::use_query(key, keys::stale::CATALOG, move || {
            let s = s.clone();
            async move {
                if !enabled {
                    return Ok(Vec::new());
                }
                let p = ItemsParams {
                    sort_by: vec!["SortName".into()],
                    page_size: Some(20000),
                    ..ItemsParams::default()
                };
                Ok(s.artists_page(kind, &p, 0).await?.items)
            }
        })
    };
    let a = load(ArtistList::Album, "album-artists-all");
    let e = load(ArtistList::Everyone, "artists-all");
    ArtistLists {
        error: a.error.is_some(),
        album_artists: a.data,
        everyone: e.data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artist(id: &str, name: &str) -> BaseItemDto {
        BaseItemDto {
            id: id.into(),
            name: Some(name.into()),
            ..Default::default()
        }
    }

    #[test]
    fn keys_ignore_case_accents_and_the() {
        assert_eq!(artist_key("The Béatles"), "beatles");
        assert_eq!(artist_key("  AC/DC "), "ac/dc");
    }

    #[test]
    fn credits_are_split() {
        let s = split_artist_credit("Daft Punk feat. Pharrell Williams & Nile Rodgers");
        assert!(s.featured);
        assert_eq!(s.names, ["Daft Punk", "Pharrell Williams", "Nile Rodgers"]);
        assert_eq!(split_artist_credit("Solo").names, ["Solo"]);
    }

    #[test]
    fn variants_merge_into_their_root() {
        let roots = [artist("1", "Daft Punk")];
        let all = [
            artist("1", "Daft Punk"),
            artist("2", "daft punk"),
            artist("3", "Daft Punk feat. Julian Casablancas"),
        ];
        let g = group_artists(&roots, &all);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].ids, ["1", "2", "3"]);
    }
}
