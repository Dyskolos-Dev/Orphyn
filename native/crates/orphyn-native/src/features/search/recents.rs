//! `recents.ts`: recent searches, newest first, in localStorage (`orphyn.search.recent`).
use orphyn_core::storage::LocalStorage;

pub const RECENT_SEARCHES_KEY: &str = "orphyn.search.recent";
pub const MAX_RECENT_SEARCHES: usize = 8;

pub fn read(storage: &LocalStorage) -> Vec<String> {
    storage
        .get_json::<serde_json::Value>(RECENT_SEARCHES_KEY)
        .and_then(|v| match v {
            serde_json::Value::Array(a) => Some(
                a.into_iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .take(MAX_RECENT_SEARCHES)
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

/// `addRecentSearch` on a list: case-insensitive de-duplication, newest first, capped. `None`
/// when the term is too short to be remembered.
pub fn with_term(current: &[String], term: &str) -> Option<Vec<String>> {
    let t = term.trim();
    if t.chars().count() < 2 {
        return None;
    }
    let mut next = vec![t.to_string()];
    next.extend(
        current
            .iter()
            .filter(|s| s.to_lowercase() != t.to_lowercase())
            .cloned(),
    );
    next.truncate(MAX_RECENT_SEARCHES);
    Some(next)
}

pub fn write(storage: &LocalStorage, list: &[String]) {
    storage.set_json(RECENT_SEARCHES_KEY, &list);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_case_insensitively_and_caps() {
        let cur: Vec<String> = ["b", "alien", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(with_term(&cur, " Alien ").unwrap(), ["Alien", "b", "c"]);
        assert!(with_term(&cur, "a").is_none());
        let many: Vec<String> = (0..8).map(|i| format!("term{i}")).collect();
        let next = with_term(&many, "new").unwrap();
        assert_eq!(next.len(), MAX_RECENT_SEARCHES);
        assert_eq!(next[0], "new");
    }

    #[test]
    fn round_trips_through_storage() {
        let s = LocalStorage::in_memory();
        assert!(read(&s).is_empty());
        write(&s, &["a b".to_string()]);
        assert_eq!(read(&s), ["a b"]);
    }
}
