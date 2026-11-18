//! Cache keys (port of `keys.ts`). Every key starts with `["jf", account]`, so data of two servers
//! never mix and `invalidate(&all(acc))` refreshes everything for one server.
use std::time::Duration;

use crate::types::ItemsParams;

pub type Key = Vec<String>;

fn k(acc: &str, parts: &[&str]) -> Key {
    let mut v = vec!["jf".to_string(), acc.to_string()];
    v.extend(parts.iter().map(|s| s.to_string()));
    v
}

pub fn all(acc: &str) -> Key {
    k(acc, &[])
}
pub fn views(acc: &str) -> Key {
    k(acc, &["views"])
}
pub fn items(acc: &str, params: &ItemsParams) -> Key {
    let p = serde_json::to_string(params).unwrap_or_default();
    k(acc, &["items", &p])
}
pub fn item(acc: &str, id: &str) -> Key {
    k(acc, &["item", id])
}
pub fn resume(acc: &str, kind: &str) -> Key {
    k(acc, &["resume", kind])
}
pub fn next_up(acc: &str, parent_id: Option<&str>) -> Key {
    k(acc, &["nextUp", parent_id.unwrap_or("null")])
}
pub fn latest(acc: &str, parent_id: &str) -> Key {
    k(acc, &["latest", parent_id])
}
pub fn similar(acc: &str, id: &str) -> Key {
    k(acc, &["similar", id])
}
pub fn artist_albums(acc: &str, artist_id: &str) -> Key {
    k(acc, &["artistAlbums", artist_id])
}
pub fn album_tracks(acc: &str, album_id: &str) -> Key {
    k(acc, &["albumTracks", album_id])
}
pub fn playlist_items(acc: &str, playlist_id: &str) -> Key {
    k(acc, &["playlistItems", playlist_id])
}
pub fn instant_mix(acc: &str, id: &str) -> Key {
    k(acc, &["instantMix", id])
}

/// Stale times (`STALE` of `keys.ts`): libraries change rarely, resume / next-up must feel fresh.
pub mod stale {
    use super::Duration;
    pub const VIEWS: Duration = Duration::from_secs(10 * 60);
    pub const CATALOG: Duration = Duration::from_secs(5 * 60);
    pub const ITEM: Duration = Duration::from_secs(2 * 60);
    pub const PROGRESS: Duration = Duration::from_secs(15);
    pub const LYRICS: Duration = Duration::from_secs(60 * 60);
    pub const SEARCH: Duration = Duration::from_secs(60);
}
