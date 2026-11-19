//! `grouping.ts`: groups a search response into ordered shelves, picks the best match as the
//! "Top Result" and gives the route of a result.
use orphyn_jellyfin::api::search::SearchResults;
use orphyn_jellyfin::BaseItemDto;

use super::fold::normalize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultKind {
    Movie,
    Series,
    Episode,
    Artist,
    Album,
    Song,
}

impl ResultKind {
    /// Key under `search:kinds.`.
    pub fn key(self) -> &'static str {
        match self {
            ResultKind::Movie => "movie",
            ResultKind::Series => "series",
            ResultKind::Episode => "episode",
            ResultKind::Artist => "artist",
            ResultKind::Album => "album",
            ResultKind::Song => "song",
        }
    }

    /// Fallback icon of the artwork (`KIND_ICON`).
    pub fn icon(self) -> &'static str {
        match self {
            ResultKind::Movie => "film",
            ResultKind::Series | ResultKind::Episode => "tv",
            ResultKind::Artist => "user",
            ResultKind::Album => "disc-3",
            ResultKind::Song => "music",
        }
    }

    /// When two candidates match equally well, the lower rank wins the top result.
    fn rank(self) -> usize {
        match self {
            ResultKind::Movie => 0,
            ResultKind::Series => 1,
            ResultKind::Artist => 2,
            ResultKind::Album => 3,
            ResultKind::Song => 4,
            ResultKind::Episode => 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultSection {
    /// Key under `search:sections.`.
    pub id: &'static str,
    pub kind: ResultKind,
    pub items: Vec<BaseItemDto>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopResult {
    pub kind: ResultKind,
    pub item: BaseItemDto,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupedResults {
    pub top: Option<TopResult>,
    /// Non-empty sections in display order (Movies, Series, Episodes, Artists, Albums, Songs).
    pub sections: Vec<ResultSection>,
    pub total: usize,
}

/// 3 = exact name, 2 = name starts with the term, 1 = a word starts with it, 0 = contains it,
/// -1 = no match.
pub fn match_score(name: Option<&str>, term: &str) -> i32 {
    let n = normalize(name.unwrap_or("")).trim().to_string();
    let q = normalize(term).trim().to_string();
    if n.is_empty() || q.is_empty() {
        return -1;
    }
    if n == q {
        return 3;
    }
    if n.starts_with(&q) {
        return 2;
    }
    if n
        .split(|c: char| c.is_whitespace() || "-–:._".contains(c))
        .any(|w| w.starts_with(&q))
    {
        return 1;
    }
    if n.contains(&q) {
        0
    } else {
        -1
    }
}

pub fn group_results(results: &SearchResults, term: &str) -> GroupedResults {
    let defs: [(&'static str, ResultKind, &Vec<BaseItemDto>); 6] = [
        ("movies", ResultKind::Movie, &results.movies),
        ("series", ResultKind::Series, &results.series),
        ("episodes", ResultKind::Episode, &results.episodes),
        ("artists", ResultKind::Artist, &results.artists),
        ("albums", ResultKind::Album, &results.albums),
        ("songs", ResultKind::Song, &results.tracks),
    ];
    let mut sections = Vec::new();
    let mut best: Option<(ResultKind, &BaseItemDto, i32)> = None;
    let mut total = 0;
    for (id, kind, items) in defs {
        if items.is_empty() {
            continue;
        }
        sections.push(ResultSection {
            id,
            kind,
            items: items.clone(),
        });
        total += items.len();
        for item in items {
            let score = match_score(item.name.as_deref(), term);
            let better = match &best {
                None => true,
                Some((bk, _, bs)) => score > *bs || (score == *bs && kind.rank() < bk.rank()),
            };
            if better {
                best = Some((kind, item, score));
            }
        }
    }
    GroupedResults {
        top: best.map(|(kind, item, _)| TopResult {
            kind,
            item: item.clone(),
        }),
        sections,
        total,
    }
}

/// Documented app route for a result. A song opens its album.
pub fn route_for(kind: ResultKind, item: &BaseItemDto) -> String {
    let id = &item.id;
    match kind {
        ResultKind::Movie | ResultKind::Series | ResultKind::Episode => format!("/video/item/{id}"),
        ResultKind::Artist => format!("/music/artist/{id}"),
        ResultKind::Album => format!("/music/album/{id}"),
        ResultKind::Song => format!(
            "/music/album/{}",
            item.album_id
                .as_deref()
                .filter(|a| !a.is_empty())
                .unwrap_or(id)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(id: &str, name: &str) -> BaseItemDto {
        serde_json::from_value(json!({"Id": id, "Name": name})).unwrap()
    }

    #[test]
    fn scores_exact_prefix_word_and_substring() {
        assert_eq!(match_score(Some("Alien"), "alien"), 3);
        assert_eq!(match_score(Some("Alien: Covenant"), "alien"), 2);
        assert_eq!(match_score(Some("The Alien"), "alien"), 1);
        assert_eq!(match_score(Some("Malienx"), "alien"), 0);
        assert_eq!(match_score(Some("Matrix"), "alien"), -1);
        assert_eq!(match_score(None, "alien"), -1);
        assert_eq!(match_score(Some("Amélie"), "amelie"), 3);
    }

    #[test]
    fn top_result_is_best_score_then_priority() {
        let r = SearchResults {
            movies: vec![item("m", "Alien Resurrection")],
            artists: vec![item("a", "Alien")],
            albums: vec![item("b", "Alien")],
            ..SearchResults::default()
        };
        let g = group_results(&r, "alien");
        let top = g.top.unwrap();
        // The exact matches beat the prefix match; the artist ranks before the album.
        assert_eq!(top.kind, ResultKind::Artist);
        assert_eq!(top.item.id, "a");
        assert_eq!(g.total, 3);
        let ids: Vec<_> = g.sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["movies", "artists", "albums"]);
    }

    #[test]
    fn empty_results_have_no_top() {
        let g = group_results(&SearchResults::default(), "x");
        assert!(g.top.is_none());
        assert_eq!(g.total, 0);
        assert!(g.sections.is_empty());
    }

    #[test]
    fn routes() {
        let song: BaseItemDto = serde_json::from_value(json!({"Id": "s", "AlbumId": "al"})).unwrap();
        assert_eq!(route_for(ResultKind::Song, &song), "/music/album/al");
        assert_eq!(route_for(ResultKind::Movie, &song), "/video/item/s");
        assert_eq!(route_for(ResultKind::Artist, &song), "/music/artist/s");
    }
}
