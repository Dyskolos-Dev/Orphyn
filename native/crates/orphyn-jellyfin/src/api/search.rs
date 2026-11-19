//! Grouped search (`fetchSearch` of `endpoints.ts`, `useSearch` of `hooks.ts`): one small
//! request per type so every group is relevant and bounded.
use crate::api::music::ArtistList;
use crate::client::JfResult;
use crate::endpoints::Session;
use crate::keys::Key;
use crate::types::{BaseItemDto, ItemsParams};

/// Items found per type.
pub const LIMIT_PER_TYPE: u32 = 12;

/// `SearchResults`: one list per kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchResults {
    pub movies: Vec<BaseItemDto>,
    pub series: Vec<BaseItemDto>,
    pub episodes: Vec<BaseItemDto>,
    pub albums: Vec<BaseItemDto>,
    pub artists: Vec<BaseItemDto>,
    pub tracks: Vec<BaseItemDto>,
}

/// `jfKeys.search`.
pub fn key(acc: &str, term: &str) -> Key {
    vec![
        "jf".to_string(),
        acc.to_string(),
        "search".to_string(),
        term.to_string(),
    ]
}

impl Session {
    async fn search_type(&self, term: &str, kind: &str, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        let p = ItemsParams {
            search_term: Some(term.to_string()),
            include_item_types: vec![kind.to_string()],
            page_size: Some(limit),
            ..ItemsParams::default()
        };
        Ok(self.items_page(&p, 0).await?.items)
    }

    /// `fetchSearch`: movies, series, episodes, albums, songs and artists, in parallel.
    pub async fn search(&self, term: &str, limit: u32) -> JfResult<SearchResults> {
        let artist_params = ItemsParams {
            search_term: Some(term.to_string()),
            page_size: Some(limit),
            ..ItemsParams::default()
        };
        let (movies, series, episodes, albums, tracks, artists) = futures_util::try_join!(
            self.search_type(term, "Movie", limit),
            self.search_type(term, "Series", limit),
            self.search_type(term, "Episode", limit),
            self.search_type(term, "MusicAlbum", limit),
            self.search_type(term, "Audio", limit),
            async {
                Ok(self
                    .artists_page(ArtistList::Everyone, &artist_params, 0)
                    .await?
                    .items)
            },
        )?;
        Ok(SearchResults {
            movies,
            series,
            episodes,
            albums,
            artists,
            tracks,
        })
    }
}

// ------------------------------------------------------------------------------ year in review

/// Fields the stats page reads (`FIELDS` of `useStats.ts`).
const PLAYED_FIELDS: [&str; 5] = [
    "PrimaryImageAspectRatio",
    "Genres",
    "RunTimeTicks",
    "ProductionYear",
    "UserData",
];
const PLAYED_PAGE: usize = 500;
/// Safety cap per kind: the 4000 most recently played items.
const MAX_PER_KIND: usize = 4000;

/// `jfKeys` of the stats queries: `["jf", account, "stats", "played", user, ...kinds]`.
pub fn played_key(acc: &str, user_id: &str) -> Key {
    ["jf", acc, "stats", "played", user_id]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

pub fn plugin_key(acc: &str, user_id: &str, year: i32) -> Key {
    let mut k: Key = ["jf", acc, "stats", "plugin", user_id]
        .iter()
        .map(|s| s.to_string())
        .collect();
    k.push(year.to_string());
    k
}

impl Session {
    /// `playedOfType`: every played item of one type, most recently played first.
    pub async fn played_of_type(&self, kind: &str) -> JfResult<Vec<BaseItemDto>> {
        let params = ItemsParams {
            include_item_types: vec![kind.to_string()],
            filters: vec!["IsPlayed".to_string()],
            sort_by: vec!["DatePlayed".to_string()],
            sort_order: Some("Descending".to_string()),
            fields: Some(PLAYED_FIELDS.iter().map(|s| s.to_string()).collect()),
            page_size: Some(PLAYED_PAGE as u32),
            ..ItemsParams::default()
        };
        let mut out = Vec::new();
        let mut start = 0;
        while start < MAX_PER_KIND {
            let page = self.items_page(&params, start).await?;
            let got = page.items.len();
            out.extend(page.items);
            if start + got >= page.total || got == 0 {
                break;
            }
            start += PLAYED_PAGE;
        }
        Ok(out)
    }

    /// Every played song, film and episode of the user (`playedOfKinds`).
    pub async fn played_items(&self) -> JfResult<Vec<BaseItemDto>> {
        let (a, m, e) = futures_util::try_join!(
            self.played_of_type("Audio"),
            self.played_of_type("Movie"),
            self.played_of_type("Episode"),
        )?;
        Ok([a, m, e].concat())
    }

    /// Rows of the Playback Reporting plugin (`/user_usage_stats/UserPlaylist`), `None` when the
    /// plugin is missing or answers something unexpected.
    pub async fn plugin_playlist(&self, days: u32, end_date: &str) -> Option<serde_json::Value> {
        use crate::client::Q;
        self.client
            .get::<serde_json::Value>(
                "/user_usage_stats/UserPlaylist",
                &[
                    ("user_id", Some(Q::Str(self.user_id.clone()))),
                    ("aggregate_data", Some(Q::Str("false".into()))),
                    ("days", Some(Q::from(u64::from(days)))),
                    ("end_date", Some(Q::Str(end_date.to_string()))),
                ],
            )
            .await
            .ok()
    }

    /// `useSaveTopPlaylist`: creates the playlist `name`, or, when one with that name exists,
    /// replaces its songs with `ids`. Returns the playlist id.
    pub async fn save_playlist(&self, name: &str, ids: &[String]) -> JfResult<String> {
        use crate::client::Q;
        use reqwest::Method;
        let wanted = name.trim().to_lowercase();
        let params = ItemsParams {
            include_item_types: vec!["Playlist".to_string()],
            sort_by: vec!["SortName".to_string()],
            sort_order: Some("Ascending".to_string()),
            page_size: Some(500),
            ..ItemsParams::default()
        };
        let existing = self
            .items_page(&params, 0)
            .await?
            .items
            .into_iter()
            .filter(|p| p.media_type.as_deref().is_none_or(|m| m == "Audio"))
            .find(|p| p.name().trim().to_lowercase() == wanted);
        let Some(existing) = existing else {
            let r: serde_json::Value = self
                .client
                .request(
                    Method::POST,
                    "/Playlists",
                    &[],
                    Some(serde_json::json!({
                        "Name": name.trim(),
                        "Ids": ids,
                        "UserId": self.user_id,
                        "MediaType": "Audio",
                        "IsPublic": false,
                    })),
                )
                .await?;
            return Ok(r
                .get("Id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string());
        };
        let entries = self.playlist_items(&existing.id).await?;
        let entry_ids: Vec<String> = entries
            .iter()
            .filter_map(|e| {
                e.extra
                    .get("PlaylistItemId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        if !entry_ids.is_empty() {
            self.remove_from_playlist(&existing.id, &entry_ids).await?;
        }
        let _: serde_json::Value = self
            .client
            .request(
                Method::POST,
                &format!("/Playlists/{}/Items", existing.id),
                &[
                    ("ids", Some(Q::List(ids.to_vec()))),
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                ],
                None,
            )
            .await?;
        Ok(existing.id)
    }
}
