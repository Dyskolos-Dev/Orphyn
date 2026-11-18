//! Jellyfin DTOs (PascalCase like the API). `BaseItemDto` keeps the fields the app reads; anything
//! else the server sends is kept in `extra`, so a screen port can read a field without touching
//! this file first (`item.extra.get("CriticRating")`).
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::TICKS_PER_SECOND;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct NameIdPair {
    pub name: Option<String>,
    pub id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserItemData {
    pub is_favorite: bool,
    pub played: bool,
    pub play_count: Option<i64>,
    pub playback_position_ticks: Option<i64>,
    pub played_percentage: Option<f64>,
    pub unplayed_item_count: Option<i64>,
    pub last_played_date: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct BaseItemDto {
    pub id: String,
    pub name: Option<String>,
    pub server_id: Option<String>,
    #[serde(rename = "Type")]
    pub item_type: Option<String>,
    pub media_type: Option<String>,
    pub collection_type: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub production_year: Option<i32>,
    pub premiere_date: Option<String>,
    pub date_created: Option<String>,
    pub run_time_ticks: Option<i64>,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub parent_id: Option<String>,
    pub child_count: Option<i32>,
    pub primary_image_aspect_ratio: Option<f64>,
    // Music.
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_artist: Option<String>,
    pub album_artists: Vec<NameIdPair>,
    pub artists: Vec<String>,
    pub artist_items: Vec<NameIdPair>,
    pub album_primary_image_tag: Option<String>,
    pub has_lyrics: Option<bool>,
    // Video.
    pub series_id: Option<String>,
    pub series_name: Option<String>,
    pub season_id: Option<String>,
    pub series_primary_image_tag: Option<String>,
    // Images.
    pub image_tags: HashMap<String, String>,
    pub backdrop_image_tags: Vec<String>,
    pub parent_backdrop_image_tags: Vec<String>,
    pub parent_backdrop_item_id: Option<String>,
    pub parent_logo_image_tag: Option<String>,
    pub parent_logo_item_id: Option<String>,
    pub parent_thumb_image_tag: Option<String>,
    pub parent_thumb_item_id: Option<String>,
    pub parent_primary_image_tag: Option<String>,
    pub parent_primary_image_item_id: Option<String>,
    pub user_data: Option<UserItemData>,
    /// Every other field of the server's JSON.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl BaseItemDto {
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }

    pub fn is_type(&self, t: &str) -> bool {
        self.item_type.as_deref() == Some(t)
    }

    /// Duration in seconds (`RunTimeTicks`), 0 when unknown.
    pub fn duration_seconds(&self) -> f64 {
        self.run_time_ticks.unwrap_or(0) as f64 / TICKS_PER_SECOND
    }

    pub fn is_favorite(&self) -> bool {
        self.user_data.as_ref().is_some_and(|u| u.is_favorite)
    }
}

/// One page of a list (`ItemsPage` of `types.ts`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemsPage {
    pub items: Vec<BaseItemDto>,
    pub total: usize,
    pub start_index: usize,
}

/// `QueryResult` of the server.
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub(crate) struct QueryResult {
    pub items: Vec<BaseItemDto>,
    pub total_record_count: Option<usize>,
    pub start_index: Option<usize>,
}

impl QueryResult {
    pub(crate) fn into_page(self, start_index: usize) -> ItemsPage {
        let total = self.total_record_count.unwrap_or(self.items.len());
        ItemsPage {
            total,
            start_index: self.start_index.unwrap_or(start_index),
            items: self.items,
        }
    }
}

/// Parameters of `GET /Items` (`ItemsParams` of `types.ts`). `Default` = every filter off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct ItemsParams {
    pub parent_id: Option<String>,
    pub include_item_types: Vec<String>,
    pub search_term: Option<String>,
    pub sort_by: Vec<String>,
    pub sort_order: Option<String>,
    pub is_favorite: Option<bool>,
    pub genres: Vec<String>,
    pub filters: Vec<String>,
    pub artist_ids: Vec<String>,
    pub album_artist_ids: Vec<String>,
    pub years: Vec<i32>,
    pub media_types: Vec<String>,
    /// `None` = recursive (the default of `fetchItemsPage`).
    pub recursive: Option<bool>,
    pub page_size: Option<u32>,
    /// Replaces `DEFAULT_FIELDS` when set.
    pub fields: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_are_kept() {
        let json = r#"{"Id":"a","Name":"Song","Type":"Audio","RunTimeTicks":1800000000,
            "UserData":{"IsFavorite":true},"CriticRating":87}"#;
        let item: BaseItemDto = serde_json::from_str(json).unwrap();
        assert!(item.is_type("Audio"));
        assert_eq!(item.duration_seconds(), 180.0);
        assert!(item.is_favorite());
        assert_eq!(item.extra.get("CriticRating"), Some(&serde_json::json!(87)));
    }
}
