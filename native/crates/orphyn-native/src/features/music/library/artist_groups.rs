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
    for sep in [
        ";", ", ", " & ", " + ", " / ", " × ", " x ", " with ", " and ",
    ] {
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
}

pub fn split_artist_credit(credit: &str) -> SplitArtists {
    let mut chunks = Vec::new();
    let mut rest = credit;
    while let Some((a, b)) = find_feat(rest) {
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
    SplitArtists { names }
}

pub struct ArtistGroup {
    pub item: BaseItemDto,
    pub ids: Vec<String>,
}

/// One group per real artist: `album_artists` root the groups, `everyone` attaches variants.
///
/// A multi-artist entity ("AaMo, yvnnis", "A feat. B") is split into its parts only when a part is
/// itself an existing artist entity, or appears in another multi-artist name; otherwise it stays
/// whole ("Simon & Garfunkel", "Tyler, The Creator"). A split entity is attributed to every part:
/// parts without an entity of their own become virtual artists backed by the entity ids.
pub fn group_artists(album_artists: &[BaseItemDto], everyone: &[BaseItemDto]) -> Vec<ArtistGroup> {
    let named =
        |a: &&BaseItemDto| a.name.as_deref().is_some_and(|n| !n.is_empty()) && !a.id.is_empty();
    let parts_of = |a: &BaseItemDto| -> Vec<String> {
        let mut seen = HashSet::new();
        split_artist_credit(a.name())
            .names
            .into_iter()
            .filter(|p| seen.insert(artist_key(p)))
            .collect()
    };
    let mut single: HashSet<String> = HashSet::new();
    let mut part_count: HashMap<String, HashSet<String>> = HashMap::new();
    for a in album_artists.iter().chain(everyone).filter(named) {
        let parts = parts_of(a);
        if parts.len() < 2 {
            single.insert(artist_key(a.name()));
        } else {
            for p in &parts {
                part_count
                    .entry(artist_key(p))
                    .or_default()
                    .insert(artist_key(a.name()));
            }
        }
    }
    let split_of = |a: &BaseItemDto| -> Option<Vec<String>> {
        let parts = parts_of(a);
        let matches = |p: &String| {
            let k = artist_key(p);
            single.contains(&k) || part_count.get(&k).is_some_and(|s| s.len() > 1)
        };
        (parts.len() > 1 && parts.iter().any(matches)).then_some(parts)
    };
    let rank = |a: &BaseItemDto| -> u8 {
        let composite = split_artist_credit(a.name()).names.len() > 1;
        (if composite { 2 } else { 0 }) + u8::from(!a.image_tags.contains_key("Primary"))
    };
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, ArtistGroup> = HashMap::new();
    let mut deferred: Vec<(&BaseItemDto, Vec<String>, bool)> = Vec::new();
    for (list, root) in [(album_artists, true), (everyone, false)] {
        for a in list.iter().filter(named) {
            if let Some(parts) = split_of(a) {
                deferred.push((a, parts, root));
                continue;
            }
            let key = artist_key(a.name());
            match groups.get_mut(&key) {
                None if root => {
                    order.push(key.clone());
                    groups.insert(
                        key,
                        ArtistGroup {
                            item: a.clone(),
                            ids: vec![a.id.clone()],
                        },
                    );
                }
                None => {}
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
        }
    }
    for (a, parts, root) in deferred {
        for part in parts {
            let key = artist_key(&part);
            match groups.get_mut(&key) {
                Some(g) => {
                    if !g.ids.contains(&a.id) {
                        g.ids.push(a.id.clone());
                    }
                }
                None if root => {
                    let mut item = a.clone();
                    item.name = Some(part);
                    item.image_tags.clear();
                    order.push(key.clone());
                    groups.insert(
                        key,
                        ArtistGroup {
                            item,
                            ids: vec![a.id.clone()],
                        },
                    );
                }
                None => {}
            }
        }
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
        assert_eq!(s.names, ["Daft Punk", "Pharrell Williams", "Nile Rodgers"]);
        assert_eq!(split_artist_credit("Solo").names, ["Solo"]);
    }

    #[test]
    fn names_with_ampersand_are_not_cut() {
        assert_eq!(
            split_artist_credit("C&C Music Factory").names,
            ["C&C Music Factory"]
        );
        assert_eq!(split_artist_credit("AC/DC").names, ["AC/DC"]);
        let roots = [
            artist("1", "Simon & Garfunkel"),
            artist("2", "C&C Music Factory"),
            artist("3", "AC/DC"),
            artist("4", "Tyler, The Creator"),
        ];
        let g = group_artists(&roots, &roots);
        let names: Vec<_> = g.iter().map(|x| x.item.name().to_string()).collect();
        assert_eq!(
            names,
            [
                "Simon & Garfunkel",
                "C&C Music Factory",
                "AC/DC",
                "Tyler, The Creator"
            ]
        );
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

    fn names_ids(g: &[ArtistGroup]) -> Vec<(String, Vec<String>)> {
        g.iter()
            .map(|x| (x.item.name().to_string(), x.ids.clone()))
            .collect()
    }

    #[test]
    fn multi_artist_entities_split_when_a_part_is_known() {
        let roots = [
            artist("1", "HOUDI"),
            artist("2", "34Murphy, Sobek, HOUDI"),
            artist("3", "77 degrés, HOUDI"),
            artist("4", "AaMo, yvnnis"),
        ];
        let g = names_ids(&group_artists(&roots, &roots));
        let ids = |n: &str| g.iter().find(|x| x.0 == n).map(|x| x.1.clone());
        assert_eq!(ids("HOUDI").unwrap(), ["1", "2", "3"]);
        assert_eq!(ids("34Murphy").unwrap(), ["2"]);
        assert_eq!(ids("Sobek").unwrap(), ["2"]);
        assert_eq!(ids("77 degrés").unwrap(), ["3"]);
        // No part of "AaMo, yvnnis" is another artist: it stays whole.
        assert_eq!(ids("AaMo, yvnnis").unwrap(), ["4"]);
        assert!(ids("AaMo").is_none() && ids("34Murphy, Sobek, HOUDI").is_none());
    }

    #[test]
    fn shared_part_between_multi_names_splits_both() {
        let roots = [
            artist("1", "34Murphy, Sobek, HOUDI"),
            artist("2", "77 degrés, HOUDI"),
        ];
        let g = names_ids(&group_artists(&roots, &roots));
        assert_eq!(g.len(), 4);
        assert_eq!(g.iter().find(|x| x.0 == "HOUDI").unwrap().1, ["1", "2"]);
    }

    #[test]
    fn existing_part_joins_its_entity() {
        let roots = [artist("1", "AaMo"), artist("2", "AaMo, yvnnis")];
        let g = names_ids(&group_artists(&roots, &roots));
        assert_eq!(g[0], ("AaMo".into(), vec!["1".into(), "2".into()]));
        assert_eq!(g[1], ("yvnnis".into(), vec!["2".into()]));
        let roots = [artist("1", "Simon"), artist("2", "Simon & Garfunkel")];
        assert_eq!(group_artists(&roots, &roots).len(), 2);
    }
}
