//! Music library endpoints: paginated artist lists (`fetchListPage` for `artists` /
//! `albumArtists`) and the favourites lists (`fetchFavorites`).
use crate::client::{JfResult, Q};
use crate::endpoints::{Session, DEFAULT_FIELDS, DEFAULT_PAGE_SIZE};
use crate::types::{BaseItemDto, ItemsPage, ItemsParams, QueryResult};

/// Which Jellyfin artist list to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtistList {
    /// `GET /Artists/AlbumArtists`: artists that own albums.
    Album,
    /// `GET /Artists`: every credited artist.
    Everyone,
}

impl ArtistList {
    fn path(self) -> &'static str {
        match self {
            ArtistList::Album => "/Artists/AlbumArtists",
            ArtistList::Everyone => "/Artists",
        }
    }
}

/// Favourites tab (`fetchFavorites` kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FavoriteKind {
    Songs,
    Albums,
    Artists,
}

impl Session {
    /// `fetchListPage` for the artist endpoints (`listParams` of `endpoints.ts`).
    pub async fn artists_page(
        &self,
        list: ArtistList,
        p: &ItemsParams,
        start_index: usize,
    ) -> JfResult<ItemsPage> {
        let sort = (!p.sort_by.is_empty()).then(|| Q::List(p.sort_by.clone()));
        let r: QueryResult = self
            .client
            .get(
                list.path(),
                &[
                    ("searchTerm", p.search_term.clone().map(Q::Str)),
                    ("sortBy", sort),
                    ("sortOrder", p.sort_order.clone().map(Q::Str)),
                    ("isFavorite", p.is_favorite.map(Q::from)),
                    ("startIndex", Some(Q::from(start_index as u64))),
                    (
                        "limit",
                        Some(Q::from(p.page_size.unwrap_or(DEFAULT_PAGE_SIZE))),
                    ),
                    ("enableTotalRecordCount", Some(true.into())),
                    (
                        "fields",
                        Some(Q::List(
                            DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect(),
                        )),
                    ),
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                ],
            )
            .await?;
        Ok(r.into_page(start_index))
    }

    /// Every favourite of one kind (`fetchFavorites`): up to 2000 items.
    pub async fn favorites(&self, kind: FavoriteKind) -> JfResult<Vec<BaseItemDto>> {
        let p = ItemsParams {
            is_favorite: Some(true),
            page_size: Some(2000),
            sort_by: vec!["SortName".into()],
            sort_order: Some("Ascending".into()),
            ..ItemsParams::default()
        };
        match kind {
            FavoriteKind::Artists => Ok(self
                .artists_page(ArtistList::Everyone, &p, 0)
                .await?
                .items),
            FavoriteKind::Songs | FavoriteKind::Albums => {
                let p = ItemsParams {
                    include_item_types: vec![if kind == FavoriteKind::Songs {
                        "Audio".into()
                    } else {
                        "MusicAlbum".into()
                    }],
                    ..p
                };
                Ok(self.items_page(&p, 0).await?.items)
            }
        }
    }
}

/// Playlist editing (`renamePlaylist`, `deletePlaylist`, `removeFromPlaylist`, `movePlaylistItem`).
impl Session {
    /// `POST /Playlists/{id}` (Jellyfin 10.9+).
    pub async fn rename_playlist(&self, id: &str, name: &str) -> JfResult<()> {
        let _: serde_json::Value = self
            .client
            .request(
                reqwest::Method::POST,
                &format!("/Playlists/{id}"),
                &[],
                Some(serde_json::json!({ "Name": name })),
            )
            .await?;
        Ok(())
    }

    /// A playlist is a library item: `DELETE /Items/{id}`.
    pub async fn delete_playlist(&self, id: &str) -> JfResult<()> {
        let _: serde_json::Value = self
            .client
            .request(reqwest::Method::DELETE, &format!("/Items/{id}"), &[], None)
            .await?;
        Ok(())
    }

    /// `DELETE /Playlists/{id}/Items`; `entry_ids` are PlaylistItemIds, not track ids.
    pub async fn remove_from_playlist(&self, id: &str, entry_ids: &[String]) -> JfResult<()> {
        let _: serde_json::Value = self
            .client
            .request(
                reqwest::Method::DELETE,
                &format!("/Playlists/{id}/Items"),
                &[("entryIds", Some(Q::List(entry_ids.to_vec())))],
                None,
            )
            .await?;
        Ok(())
    }

    /// Moves one entry (PlaylistItemId) to a 0-based position.
    pub async fn move_playlist_item(&self, id: &str, entry_id: &str, new_index: usize) -> JfResult<()> {
        let _: serde_json::Value = self
            .client
            .request(
                reqwest::Method::POST,
                &format!("/Playlists/{id}/Items/{entry_id}/Move/{new_index}"),
                &[],
                None,
            )
            .await?;
        Ok(())
    }
}
