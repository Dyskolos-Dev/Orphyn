//! Typed fetchers, one per endpoint (port of `endpoints.ts`, same parameters). Screens call them
//! through the query cache (`QueryCache::fetch` with a key from `keys`), never directly in render.
//! Endpoints a later screen needs (search, lyrics, playlists editing...) are added here, next to
//! their TypeScript twin.
use reqwest::Method;

use crate::client::{Client, JfResult, Q};
use crate::types::{BaseItemDto, ItemsPage, ItemsParams, QueryResult};

pub const DEFAULT_FIELDS: &[&str] = &[
    "PrimaryImageAspectRatio",
    "Overview",
    "Genres",
    "ProductionYear",
    "RunTimeTicks",
    "ParentId",
    "DateCreated",
];
pub const DEFAULT_PAGE_SIZE: u32 = 50;

fn fields(extra: &[&str]) -> Q {
    Q::List(
        DEFAULT_FIELDS
            .iter()
            .chain(extra)
            .map(|s| s.to_string())
            .collect(),
    )
}

fn list(v: &[String]) -> Option<Q> {
    (!v.is_empty()).then(|| Q::List(v.to_vec()))
}

/// The signed-in user of a server: every endpoint needs the client (with its token) and the user.
#[derive(Clone, Debug)]
pub struct Session {
    /// Account id of the store (`ServerAccount::id`), the first segment of every cache key.
    pub account_id: String,
    pub user_id: String,
    pub client: Client,
}

impl Session {
    fn user(&self) -> Option<Q> {
        Some(Q::Str(self.user_id.clone()))
    }

    /// `GET /Items` (`fetchItemsPage`).
    pub async fn items_page(&self, p: &ItemsParams, start_index: usize) -> JfResult<ItemsPage> {
        let fields_q = match &p.fields {
            Some(f) => Q::List(f.clone()),
            None => fields(&[]),
        };
        let r: QueryResult = self
            .client
            .get(
                "/Items",
                &[
                    ("parentId", p.parent_id.clone().map(Q::Str)),
                    ("searchTerm", p.search_term.clone().map(Q::Str)),
                    ("sortBy", list(&p.sort_by)),
                    ("sortOrder", p.sort_order.clone().map(Q::Str)),
                    ("isFavorite", p.is_favorite.map(Q::from)),
                    ("genres", list(&p.genres)),
                    ("startIndex", Some(Q::from(start_index as u64))),
                    (
                        "limit",
                        Some(Q::from(p.page_size.unwrap_or(DEFAULT_PAGE_SIZE))),
                    ),
                    ("enableTotalRecordCount", Some(true.into())),
                    ("fields", Some(fields_q)),
                    ("userId", self.user()),
                    ("recursive", Some(Q::from(p.recursive.unwrap_or(true)))),
                    ("includeItemTypes", list(&p.include_item_types)),
                    ("filters", list(&p.filters)),
                    ("artistIds", list(&p.artist_ids)),
                    ("albumArtistIds", list(&p.album_artist_ids)),
                    (
                        "years",
                        (!p.years.is_empty())
                            .then(|| Q::List(p.years.iter().map(|y| y.to_string()).collect())),
                    ),
                    ("mediaTypes", list(&p.media_types)),
                ],
            )
            .await?;
        Ok(r.into_page(start_index))
    }

    /// `GET /Users/{user}/Items/{id}` (`fetchItem`).
    pub async fn item(&self, id: &str) -> JfResult<BaseItemDto> {
        self.client
            .get(&format!("/Users/{}/Items/{id}", self.user_id), &[])
            .await
    }

    /// `GET /UserViews`: the libraries.
    pub async fn user_views(&self) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get("/UserViews", &[("userId", self.user())])
            .await?;
        Ok(r.items)
    }

    /// The tracks of an album, in disc / track order (`fetchAlbumTracks`).
    pub async fn album_tracks(&self, album_id: &str) -> JfResult<Vec<BaseItemDto>> {
        let p = ItemsParams {
            parent_id: Some(album_id.to_string()),
            include_item_types: vec!["Audio".into()],
            sort_by: vec![
                "ParentIndexNumber".into(),
                "IndexNumber".into(),
                "SortName".into(),
            ],
            sort_order: Some("Ascending".into()),
            page_size: Some(1000),
            fields: Some(
                DEFAULT_FIELDS
                    .iter()
                    .chain(&["MediaSources"])
                    .map(|s| s.to_string())
                    .collect(),
            ),
            ..ItemsParams::default()
        };
        Ok(self.items_page(&p, 0).await?.items)
    }

    /// An artist's albums, newest first (`fetchArtistAlbums`).
    pub async fn artist_albums(&self, artist_id: &str) -> JfResult<Vec<BaseItemDto>> {
        let p = ItemsParams {
            include_item_types: vec!["MusicAlbum".into()],
            artist_ids: vec![artist_id.to_string()],
            sort_by: vec!["ProductionYear".into(), "SortName".into()],
            sort_order: Some("Descending".into()),
            page_size: Some(500),
            ..ItemsParams::default()
        };
        Ok(self.items_page(&p, 0).await?.items)
    }

    /// `GET /Playlists/{id}/Items` (`fetchPlaylistItems`).
    pub async fn playlist_items(&self, playlist_id: &str) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Playlists/{playlist_id}/Items"),
                &[
                    ("userId", self.user()),
                    ("fields", Some(fields(&["MediaSources"]))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /Items/{id}/InstantMix` (`fetchInstantMix`).
    pub async fn instant_mix(&self, id: &str, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Items/{id}/InstantMix"),
                &[
                    ("userId", self.user()),
                    ("limit", Some(limit.into())),
                    ("fields", Some(fields(&[]))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /UserItems/Resume` for `Video` or `Audio` (`fetchResume`).
    pub async fn resume(&self, media_type: &str, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                "/UserItems/Resume",
                &[
                    ("userId", self.user()),
                    ("mediaTypes", Some(media_type.into())),
                    ("limit", Some(limit.into())),
                    ("fields", Some(fields(&[]))),
                    ("enableTotalRecordCount", Some(false.into())),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /Shows/NextUp` (`fetchNextUp`).
    pub async fn next_up(&self, parent_id: Option<&str>, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                "/Shows/NextUp",
                &[
                    ("userId", self.user()),
                    ("parentId", parent_id.map(Q::from)),
                    ("limit", Some(limit.into())),
                    ("fields", Some(fields(&[]))),
                    ("enableTotalRecordCount", Some(false.into())),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /Items/Latest` (`fetchLatest`).
    pub async fn latest(&self, parent_id: &str, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        self.client
            .get(
                "/Items/Latest",
                &[
                    ("userId", self.user()),
                    ("parentId", Some(parent_id.into())),
                    ("limit", Some(limit.into())),
                    ("fields", Some(fields(&[]))),
                ],
            )
            .await
    }

    /// `GET /Items/{id}/Similar` (`fetchSimilar`).
    pub async fn similar(&self, id: &str, limit: u32) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Items/{id}/Similar"),
                &[
                    ("userId", self.user()),
                    ("limit", Some(limit.into())),
                    ("fields", Some(fields(&[]))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// Favourite on / off (`setFavorite`).
    pub async fn set_favorite(&self, id: &str, favorite: bool) -> JfResult<()> {
        let method = if favorite {
            Method::POST
        } else {
            Method::DELETE
        };
        let _: serde_json::Value = self
            .client
            .request(
                method,
                &format!("/UserFavoriteItems/{id}"),
                &[("userId", self.user())],
                None,
            )
            .await?;
        Ok(())
    }

    /// Played on / off (`setPlayed`).
    pub async fn set_played(&self, id: &str, played: bool) -> JfResult<()> {
        let method = if played { Method::POST } else { Method::DELETE };
        let _: serde_json::Value = self
            .client
            .request(
                method,
                &format!("/UserPlayedItems/{id}"),
                &[("userId", self.user())],
                None,
            )
            .await?;
        Ok(())
    }

    /// Direct-play URL of a track (what `musicPlayer` loads with `audioQuality: 'original'`).
    pub fn audio_url(&self, id: &str) -> String {
        self.client.authed_url(
            &format!("/Audio/{id}/stream"),
            &[("static", Some(true.into()))],
        )
    }

    /// Transcoded audio URL capped at `max_bitrate` bits/s (`audioQuality` set to a number).
    pub fn audio_transcode_url(&self, id: &str, max_bitrate: u32) -> String {
        self.client.authed_url(
            &format!("/Audio/{id}/universal"),
            &[
                ("userId", self.user()),
                ("deviceId", Some(self.client.device_id().into())),
                ("maxStreamingBitrate", Some(max_bitrate.into())),
                (
                    "container",
                    Some("opus,webm|opus,mp3,aac,m4a|aac,flac".into()),
                ),
                ("transcodingContainer", Some("mp3".into())),
                ("audioCodec", Some("mp3".into())),
            ],
        )
    }

    /// Direct-play URL of a video file (handed to libmpv).
    pub fn video_url(&self, id: &str) -> String {
        self.client.authed_url(
            &format!("/Videos/{id}/stream"),
            &[("static", Some(true.into()))],
        )
    }
}
