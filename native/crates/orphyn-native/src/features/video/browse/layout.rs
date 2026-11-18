//! `features/home/layout.ts` for the video home: which rows the home shows, in which order,
//! hidden or not. Reads the layout Settings > Home saves under `orphyn.home.video.<server>.<user>`
//! (the same JSON as the React app) and adds the "Recently Added" row of each new library.
use serde_json::{json, Value};

/// Version the React app writes (`LAYOUT_VERSION`). The video scope has no introductions, so
/// migrating an older layout only raises the number.
pub const LAYOUT_VERSION: i64 = 2;

pub const VIDEO_ROW_TYPES: [(&str, bool, bool, usize); 10] = [
    // (type, unique, needs a param, default count)
    ("hero", true, false, 8),
    ("continue", true, false, 20),
    ("nextUp", true, false, 20),
    ("latest", false, true, 20),
    ("favourites", true, false, 30),
    ("genre", false, true, 20),
    ("collection", false, true, 20),
    ("because", true, false, 20),
    ("released", true, false, 20),
    ("random", true, false, 20),
];

/// Card size of a row: `auto` follows Settings > Appearance, the others override it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowSize {
    Auto,
    Small,
    Large,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HomeRow {
    pub id: String,
    pub kind: String,
    pub hidden: bool,
    /// Library / genre / collection the row is about (an id, or the genre name).
    pub param: Option<String>,
    /// Display name of `param`, so the row can be titled without loading anything.
    pub label: Option<String>,
    pub size: RowSize,
    pub count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HomeLayout {
    pub version: i64,
    pub rows: Vec<HomeRow>,
    /// Libraries whose "Recently Added" row was already created, so a removed one stays removed.
    pub known_libraries: Vec<String>,
}

fn def_of(kind: &str) -> Option<(bool, bool, usize)> {
    VIDEO_ROW_TYPES
        .iter()
        .find(|d| d.0 == kind)
        .map(|d| (d.1, d.2, d.3))
}

fn default_row(kind: &str) -> HomeRow {
    HomeRow {
        id: kind.to_string(),
        kind: kind.to_string(),
        hidden: false,
        param: None,
        label: None,
        size: RowSize::Auto,
        count: None,
    }
}

pub fn default_layout() -> HomeLayout {
    HomeLayout {
        version: LAYOUT_VERSION,
        rows: ["hero", "continue", "nextUp", "favourites"]
            .into_iter()
            .map(default_row)
            .collect(),
        known_libraries: Vec::new(),
    }
}

pub fn storage_key(server_id: &str, user_id: &str) -> String {
    format!("orphyn.home.video.{server_id}.{user_id}")
}

/// Number of items of a row (`countOf`).
pub fn count_of(row: &HomeRow) -> usize {
    row.count
        .or_else(|| def_of(&row.kind).map(|d| d.2))
        .unwrap_or(20)
}

fn clamp_count(n: f64) -> usize {
    n.round().clamp(5.0, 60.0) as usize
}

/// Keeps the valid rows of a stored object; anything unusable is dropped (`sanitizeLayout`).
fn sanitize(raw: &Value) -> Option<HomeLayout> {
    let rows_raw = raw.get("rows")?.as_array()?;
    let mut rows: Vec<HomeRow> = Vec::new();
    let mut uniques = std::collections::HashSet::new();
    for r in rows_raw {
        let (Some(id), Some(kind)) = (
            r.get("id").and_then(Value::as_str),
            r.get("type").and_then(Value::as_str),
        ) else {
            continue;
        };
        let Some((unique, needs_param, _)) = def_of(kind) else {
            continue;
        };
        if rows.iter().any(|x| x.id == id) {
            continue;
        }
        if unique && !uniques.insert(kind.to_string()) {
            continue;
        }
        let param = r
            .get("param")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .map(str::to_string);
        if needs_param && param.is_none() {
            continue;
        }
        let mut row = HomeRow {
            id: id.to_string(),
            kind: kind.to_string(),
            hidden: r.get("hidden").and_then(Value::as_bool) == Some(true),
            ..default_row(kind)
        };
        if needs_param {
            row.param = param;
            row.label = r.get("label").and_then(Value::as_str).map(str::to_string);
        }
        if kind != "hero" {
            row.size = match r.get("size").and_then(Value::as_str) {
                Some("small") => RowSize::Small,
                Some("large") => RowSize::Large,
                _ => RowSize::Auto,
            };
        }
        if let Some(c) = r.get("count").and_then(Value::as_f64) {
            row.count = Some(clamp_count(c));
        }
        rows.push(row);
    }
    let version = raw.get("version").and_then(Value::as_i64).unwrap_or(0);
    let known_libraries = raw
        .get("knownLibraries")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(HomeLayout {
        version: version.max(LAYOUT_VERSION),
        rows,
        known_libraries,
    })
}

/// Parses a stored JSON string: defaults when there is none or it is unusable (`parseLayout`).
pub fn parse_layout(json: Option<&str>) -> HomeLayout {
    json.and_then(|j| serde_json::from_str::<Value>(j).ok())
        .and_then(|v| sanitize(&v))
        .unwrap_or_else(default_layout)
}

/// Serialises a layout the way `saveLayout` does (for tests and for a future editor).
pub fn to_json(layout: &HomeLayout) -> Value {
    json!({
        "version": LAYOUT_VERSION,
        "knownLibraries": layout.known_libraries,
        "rows": layout.rows.iter().map(|r| {
            let mut o = json!({"id": r.id, "type": r.kind, "hidden": r.hidden});
            if let Some(p) = &r.param { o["param"] = json!(p); }
            if let Some(l) = &r.label { o["label"] = json!(l); }
            match r.size {
                RowSize::Small => o["size"] = json!("small"),
                RowSize::Large => o["size"] = json!("large"),
                RowSize::Auto => {}
            }
            if let Some(c) = r.count { o["count"] = json!(c); }
            o
        }).collect::<Vec<_>>(),
    })
}

/// The layout with the rows of every video library that has none yet, so new libraries appear on
/// their own (`syncLibraries`). `libraries` are `(id, name)`.
pub fn sync_libraries(layout: HomeLayout, libraries: &[(String, String)]) -> HomeLayout {
    let fresh: Vec<&(String, String)> = libraries
        .iter()
        .filter(|(id, _)| !layout.known_libraries.contains(id))
        .collect();
    if fresh.is_empty() {
        return layout;
    }
    let mut rows = layout.rows.clone();
    // New libraries follow the other "Recently Added" rows, else the "Next Up" one, else the end.
    let last = |k: &str| rows.iter().rposition(|r| r.kind == k);
    let at = last("latest")
        .or_else(|| last("nextUp"))
        .map(|i| i + 1)
        .unwrap_or(rows.len());
    let added: Vec<HomeRow> = fresh
        .iter()
        .filter(|(id, _)| {
            !rows
                .iter()
                .any(|r| r.kind == "latest" && r.param.as_deref() == Some(id))
        })
        .map(|(id, name)| HomeRow {
            id: format!("latest-{id}"),
            kind: "latest".into(),
            hidden: false,
            param: Some(id.clone()),
            label: Some(name.clone()),
            size: RowSize::Auto,
            count: None,
        })
        .collect();
    rows.splice(at..at, added);
    let mut known = layout.known_libraries;
    known.extend(fresh.iter().map(|(id, _)| id.clone()));
    HomeLayout {
        rows,
        known_libraries: known,
        ..layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_stored_layout_gives_the_default_rows() {
        let l = parse_layout(None);
        let kinds: Vec<_> = l.rows.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, ["hero", "continue", "nextUp", "favourites"]);
        assert_eq!(parse_layout(Some("not json")), default_layout());
    }

    #[test]
    fn invalid_rows_are_dropped_and_options_kept() {
        let l = parse_layout(Some(
            r#"{"version":2,"knownLibraries":["a"],"rows":[
                {"id":"hero","type":"hero","hidden":true},
                {"id":"hero2","type":"hero"},
                {"id":"g","type":"genre"},
                {"id":"g2","type":"genre","param":"Drama","label":"Drama","size":"large","count":500},
                {"id":"x","type":"unknown"},
                {"id":"c","type":"continue","size":"small","count":2}]}"#,
        ));
        let ids: Vec<_> = l.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["hero", "g2", "c"]);
        assert!(l.rows[0].hidden);
        assert_eq!(l.rows[1].size, RowSize::Large);
        assert_eq!(count_of(&l.rows[1]), 60);
        assert_eq!(l.rows[2].size, RowSize::Small);
        assert_eq!(count_of(&l.rows[2]), 5);
        assert_eq!(l.known_libraries, ["a"]);
    }

    #[test]
    fn default_counts_come_from_the_row_type() {
        let l = default_layout();
        assert_eq!(count_of(&l.rows[0]), 8);
        assert_eq!(count_of(&l.rows[3]), 30);
    }

    #[test]
    fn new_libraries_get_a_row_once() {
        let libs = vec![("m".to_string(), "Movies".to_string())];
        let l = sync_libraries(default_layout(), &libs);
        let kinds: Vec<_> = l.rows.iter().map(|r| r.kind.as_str()).collect();
        // After "nextUp", before "favourites".
        assert_eq!(
            kinds,
            ["hero", "continue", "nextUp", "latest", "favourites"]
        );
        assert_eq!(l.rows[3].param.as_deref(), Some("m"));
        assert_eq!(l.known_libraries, ["m"]);
        // A second pass changes nothing; a removed row stays removed.
        assert_eq!(sync_libraries(l.clone(), &libs), l);
        let mut removed = l.clone();
        removed.rows.remove(3);
        assert_eq!(sync_libraries(removed.clone(), &libs), removed);
    }

    #[test]
    fn layout_round_trips_through_its_json() {
        let l = sync_libraries(default_layout(), &[("m".to_string(), "Movies".to_string())]);
        let text = to_json(&l).to_string();
        assert_eq!(parse_layout(Some(&text)), l);
        assert_eq!(storage_key("srv", "usr"), "orphyn.home.video.srv.usr");
    }
}
