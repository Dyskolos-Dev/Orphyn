//! `fuzzy.ts`: fuzzy matching for the command palette. An accent- and case-insensitive
//! subsequence match that rewards consecutive letters, word starts and early matches, and returns
//! the matched positions (char indices into the ORIGINAL text) so the UI can highlight them.
use crate::features::search::normalize;

#[derive(Clone, Debug, PartialEq)]
pub struct FuzzyMatch {
    pub score: f64,
    /// Indices of the matched chars in the original text, ascending.
    pub indices: Vec<usize>,
}

const MAX_TEXT: usize = 120;
const MATCH: f64 = 10.0;
const START_BONUS: f64 = 12.0;
const WORD_BONUS: f64 = 8.0;
const CONSECUTIVE_BONUS: f64 = 9.0;
const GAP_PENALTY: f64 = 1.5;
const LEADING_PENALTY: f64 = 0.5;

fn is_separator(c: char) -> bool {
    c.is_whitespace() || "-_.:/,–—·".contains(c)
}

/// One normalized char per kept original index, so highlights map back onto the original text.
fn fold(text: &str) -> (Vec<char>, Vec<usize>) {
    let mut chars = Vec::new();
    let mut orig = Vec::new();
    for (i, c) in text.chars().take(MAX_TEXT).enumerate() {
        if let Some(n) = normalize(&c.to_string()).chars().next() {
            chars.push(n);
            orig.push(i);
        }
    }
    (chars, orig)
}

#[allow(clippy::needless_range_loop)]
/// Best fuzzy alignment of `query` in `text`, or `None` when some query letter is missing.
pub fn fuzzy_match(query: &str, text: &str) -> Option<FuzzyMatch> {
    let q: Vec<char> = normalize(query)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return None;
    }
    let (t, orig) = fold(text);
    let (n, m) = (t.len(), q.len());
    if m > n {
        return None;
    }
    let base = |i: usize| -> f64 {
        let mut s = MATCH;
        if i == 0 {
            s += START_BONUS;
        } else if is_separator(t[i - 1]) {
            s += WORD_BONUS;
        }
        s
    };

    const NEG: f64 = f64::NEG_INFINITY;
    // score[j][i]: best score with q[j] matched at t[i]; from[j][i]: where q[j-1] was matched.
    let mut score = vec![vec![NEG; n]; m];
    let mut from = vec![vec![usize::MAX; n]; m];
    for i in 0..n {
        if t[i] == q[0] {
            score[0][i] = base(i) - i as f64 * LEADING_PENALTY;
        }
    }
    for j in 1..m {
        for i in j..n {
            if t[i] != q[j] {
                continue;
            }
            let mut best = NEG;
            let mut best_k = usize::MAX;
            for k in (j - 1)..i {
                let prev = score[j - 1][k];
                if prev == NEG {
                    continue;
                }
                let s = prev
                    + if k + 1 == i {
                        CONSECUTIVE_BONUS
                    } else {
                        -((i - k - 1) as f64) * GAP_PENALTY
                    };
                if s > best {
                    best = s;
                    best_k = k;
                }
            }
            if best_k != usize::MAX {
                score[j][i] = best + base(i);
                from[j][i] = best_k;
            }
        }
    }

    let mut end = usize::MAX;
    let mut top = NEG;
    for i in (m - 1)..n {
        let s = score[m - 1][i];
        if s > top {
            top = s;
            end = i;
        }
    }
    if end == usize::MAX {
        return None;
    }

    let mut indices = vec![0; m];
    let mut i = end;
    for j in (0..m).rev() {
        indices[j] = orig[i];
        i = from[j][i];
    }

    // A contiguous hit of the whole query is always the better answer: reward it explicitly.
    let flat: String = t.iter().collect();
    let needle: String = q.iter().collect();
    let mut result = top;
    if let Some(at) = flat.find(&needle) {
        let at_chars = flat[..at].chars().count();
        result += 40.0;
        if at_chars == 0 {
            result += 30.0;
        } else if flat[..at].chars().last().is_none_or(is_separator) {
            result += 15.0;
        }
        if flat == needle {
            result += 60.0;
        }
    }
    Some(FuzzyMatch {
        score: result,
        indices,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked<T> {
    pub item: T,
    pub score: f64,
    /// Highlight positions in the item's primary text (empty when an alias matched).
    pub indices: Vec<usize>,
    /// Position in the input list (stable ordering for equal scores).
    pub order: usize,
}

/// Below this a match is noise (a scattering of letters over a long text).
const MIN_SCORE: f64 = 18.0;
const ALIAS_PENALTY: f64 = 6.0;

/// Ranks `items` against `query`. `texts(item)` returns the primary text (highlighted) followed by
/// aliases (other-language name, synonyms) that also match but do not highlight.
pub fn rank<T: Clone>(
    items: &[T],
    query: &str,
    texts: impl Fn(&T) -> Vec<String>,
) -> Vec<Ranked<T>> {
    let mut out: Vec<Ranked<T>> = Vec::new();
    for (order, item) in items.iter().enumerate() {
        let mut best: Option<Ranked<T>> = None;
        for (k, text) in texts(item).iter().enumerate() {
            if text.is_empty() {
                continue;
            }
            let Some(m) = fuzzy_match(query, text) else {
                continue;
            };
            let score = m.score - if k == 0 { 0.0 } else { ALIAS_PENALTY };
            if best.as_ref().is_none_or(|b| score > b.score) {
                best = Some(Ranked {
                    item: item.clone(),
                    score,
                    indices: if k == 0 { m.indices } else { Vec::new() },
                    order,
                });
            }
        }
        if let Some(b) = best.filter(|b| b.score >= MIN_SCORE) {
            out.push(b);
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.order.cmp(&b.order))
    });
    out
}

/// Splits `text` into runs, flagging the ones covered by `indices` (for `<mark>` rendering).
pub fn highlight_runs(text: &str, indices: &[usize]) -> Vec<(String, bool)> {
    if indices.is_empty() {
        return vec![(text.to_string(), false)];
    }
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let hit = indices.contains(&i);
        match runs.last_mut() {
            Some((s, m)) if *m == hit => s.push(c),
            _ => runs.push((c.to_string(), hit)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(q: &str, t: &str) -> Option<FuzzyMatch> {
        fuzzy_match(q, t)
    }

    #[test]
    fn empty_or_missing_letters_do_not_match() {
        assert!(m("", "Settings").is_none());
        assert!(m("   ", "Settings").is_none());
        assert!(m("xyz", "Settings").is_none());
        assert!(m("settingsss", "Settings").is_none());
        // Letters must appear in order.
        assert!(m("gs", "Settings").is_some());
        assert!(m("sg", "Go").is_none());
    }

    #[test]
    fn ignores_case_and_accents_and_maps_back_to_the_original() {
        let r = m("elephant", "Éléphant").unwrap();
        assert_eq!(r.indices, (0..8).collect::<Vec<_>>());
        let r = m("ete", "Un été").unwrap();
        assert_eq!(r.indices, vec![3, 4, 5]);
    }

    #[test]
    fn contiguous_and_word_start_hits_win() {
        let exact = m("set", "Settings").unwrap();
        let scattered = m("set", "Search for episodes in titles").unwrap();
        assert!(exact.score > scattered.score);
        let word = m("fav", "Go to Favorites").unwrap();
        let inside = m("avo", "Go to Favorites").unwrap();
        assert!(word.score > inside.score);
        // The whole text matched exactly is the best possible answer.
        assert!(m("home", "home").unwrap().score > m("home", "Go to Home").unwrap().score);
    }

    #[test]
    fn indices_are_ascending_and_point_at_the_matched_letters() {
        let r = m("gth", "Go to Home").unwrap();
        let chars: Vec<char> = "Go to Home".chars().collect();
        assert!(r.indices.windows(2).all(|w| w[0] < w[1]));
        let hit: String = r
            .indices
            .iter()
            .map(|i| chars[*i].to_ascii_lowercase())
            .collect();
        assert_eq!(hit, "gth");
        // Word starts are preferred: G, t(o), H.
        assert_eq!(r.indices, vec![0, 3, 6]);
    }

    #[test]
    fn rank_orders_by_score_then_input_order_and_drops_noise() {
        let items = vec!["Go to Albums", "Go to Artists", "Open Settings", "Sign out"];
        let r = rank(&items, "art", |s| vec![s.to_string()]);
        assert_eq!(r[0].item, "Go to Artists");
        assert!(r.iter().all(|x| x.item != "Sign out"));
        // Equal scores keep the input order.
        let same = vec!["abc", "abc"];
        let r = rank(&same, "abc", |s| vec![s.to_string()]);
        assert_eq!(r.iter().map(|x| x.order).collect::<Vec<_>>(), vec![0, 1]);
    }

    #[test]
    fn aliases_match_without_highlight_and_with_a_penalty() {
        let items = vec![("Open Settings", "preferences")];
        let r = rank(&items, "preferences", |(a, b)| {
            vec![a.to_string(), b.to_string()]
        });
        assert_eq!(r.len(), 1);
        assert!(r[0].indices.is_empty());
        let direct = rank(&items, "settings", |(a, b)| {
            vec![a.to_string(), b.to_string()]
        });
        assert!(!direct[0].indices.is_empty());
    }

    #[test]
    fn highlight_runs_group_consecutive_chars() {
        assert_eq!(highlight_runs("abc", &[]), vec![("abc".to_string(), false)]);
        assert_eq!(
            highlight_runs("Settings", &[0, 1, 6]),
            vec![
                ("Se".to_string(), true),
                ("ttin".to_string(), false),
                ("g".to_string(), true),
                ("s".to_string(), false),
            ]
        );
    }
}
