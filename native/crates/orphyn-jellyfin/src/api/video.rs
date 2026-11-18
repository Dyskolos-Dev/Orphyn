//! Video endpoints: what the browsing screens need beyond `endpoints.rs` (seasons, episodes,
//! trailers, genres, a released-only items page), their cache keys, and typed readers for the
//! video fields `BaseItemDto` keeps in `extra` (people, trailers, ratings, streams).
use reqwest::Method;
use serde::Deserialize;

use crate::client::{JfResult, Q};
use crate::endpoints::{Session, DEFAULT_FIELDS, DEFAULT_PAGE_SIZE};
use crate::keys::Key;
use crate::types::{BaseItemDto, ItemsPage, ItemsParams, QueryResult};

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

/// Cache keys of the video endpoints (`keys.ts`), same `["jf", account, ...]` prefix as `keys`.
pub mod keys {
    use super::Key;
    use crate::types::ItemsParams;

    fn k(acc: &str, parts: &[&str]) -> Key {
        let mut v = vec!["jf".to_string(), acc.to_string()];
        v.extend(parts.iter().map(|s| s.to_string()));
        v
    }

    pub fn seasons(acc: &str, series_id: &str) -> Key {
        k(acc, &["seasons", series_id])
    }
    pub fn episodes(acc: &str, series_id: &str, season_id: Option<&str>) -> Key {
        k(acc, &["episodes", series_id, season_id.unwrap_or("null")])
    }
    pub fn local_trailers(acc: &str, id: &str) -> Key {
        k(acc, &["localTrailers", id])
    }
    pub fn genres(acc: &str) -> Key {
        k(acc, &["genres", "video"])
    }
    /// A `GET /Items` page that may be restricted to released titles (`releasedOnly`).
    pub fn video_items(acc: &str, params: &ItemsParams, released_only: bool) -> Key {
        let p = serde_json::to_string(params).unwrap_or_default();
        k(
            acc,
            &["items", &p, if released_only { "released" } else { "all" }],
        )
    }
}

impl Session {
    /// `fetchItemsPage` with `releasedOnly` (`maxPremiereDate` = now) for the video screens.
    pub async fn video_items_page(
        &self,
        p: &ItemsParams,
        start_index: usize,
        released_only: bool,
    ) -> JfResult<ItemsPage> {
        let fields_q = match &p.fields {
            Some(f) => Q::List(f.clone()),
            None => fields(&[]),
        };
        let now = released_now();
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
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    ("recursive", Some(Q::from(p.recursive.unwrap_or(true)))),
                    ("includeItemTypes", list(&p.include_item_types)),
                    ("filters", list(&p.filters)),
                    ("mediaTypes", list(&p.media_types)),
                    ("maxPremiereDate", released_only.then_some(Q::Str(now))),
                ],
            )
            .await?;
        Ok(r.into_page(start_index))
    }

    /// `GET /Shows/{id}/Seasons` (`fetchSeasons`).
    pub async fn seasons(&self, series_id: &str) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Shows/{series_id}/Seasons"),
                &[
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    ("fields", Some(fields(&[]))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /Shows/{id}/Episodes` (`fetchEpisodes`), optionally of one season.
    pub async fn episodes(
        &self,
        series_id: &str,
        season_id: Option<&str>,
    ) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                &format!("/Shows/{series_id}/Episodes"),
                &[
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    ("seasonId", season_id.map(Q::from)),
                    ("fields", Some(fields(&["MediaSources"]))),
                ],
            )
            .await?;
        Ok(r.items)
    }

    /// `GET /Items/{id}/LocalTrailers` (`fetchLocalTrailers`): trailer files next to the item.
    /// A 404 (no trailers) is an empty list.
    pub async fn local_trailers(&self, id: &str) -> JfResult<Vec<BaseItemDto>> {
        let r: JfResult<Option<Vec<BaseItemDto>>> = self
            .client
            .request(
                Method::GET,
                &format!("/Items/{id}/LocalTrailers"),
                &[("userId", Some(Q::Str(self.user_id.clone())))],
                None,
            )
            .await;
        match r {
            Ok(v) => Ok(v.unwrap_or_default()),
            Err(crate::JfError::Http(404)) => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// `GET /Genres` for movies and series (`fetchGenres('video')`), by name.
    pub async fn video_genres(&self) -> JfResult<Vec<BaseItemDto>> {
        let r: QueryResult = self
            .client
            .get(
                "/Genres",
                &[
                    ("userId", Some(Q::Str(self.user_id.clone()))),
                    (
                        "includeItemTypes",
                        Some(Q::List(vec!["Movie".into(), "Series".into()])),
                    ),
                    ("sortBy", Some("SortName".into())),
                    ("sortOrder", Some("Ascending".into())),
                    ("enableTotalRecordCount", Some(false.into())),
                ],
            )
            .await?;
        Ok(r.items)
    }
}

/// Now as an ISO 8601 UTC timestamp (`new Date().toISOString()`).
fn released_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    iso_from_secs(secs)
}

fn iso_from_secs(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// A cast or crew member (`People` of an item).
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct Person {
    pub id: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    #[serde(rename = "Type")]
    pub kind: Option<String>,
    pub primary_image_tag: Option<String>,
}

/// A remote trailer link (`RemoteTrailers`).
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct RemoteTrailer {
    pub url: Option<String>,
    pub name: Option<String>,
}

/// One media stream (`MediaStreams` of the first source), the fields the quality badges read.
#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaStream {
    #[serde(rename = "Type")]
    pub kind: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_range: Option<String>,
    pub video_range_type: Option<String>,
    pub channels: Option<i64>,
    pub profile: Option<String>,
    pub display_title: Option<String>,
}

/// Typed readers of the video fields that live in `BaseItemDto::extra`.
pub trait VideoItemExt {
    fn people(&self) -> Vec<Person>;
    fn remote_trailers(&self) -> Vec<RemoteTrailer>;
    fn local_trailer_count(&self) -> Option<i64>;
    fn official_rating(&self) -> Option<String>;
    fn community_rating(&self) -> Option<f64>;
    fn sort_name(&self) -> Option<String>;
    fn media_streams(&self) -> Vec<MediaStream>;
}

fn typed<T: serde::de::DeserializeOwned + Default>(item: &BaseItemDto, key: &str) -> T {
    item.extra
        .get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

impl VideoItemExt for BaseItemDto {
    fn people(&self) -> Vec<Person> {
        typed(self, "People")
    }
    fn remote_trailers(&self) -> Vec<RemoteTrailer> {
        typed(self, "RemoteTrailers")
    }
    fn local_trailer_count(&self) -> Option<i64> {
        self.extra.get("LocalTrailerCount").and_then(|v| v.as_i64())
    }
    fn official_rating(&self) -> Option<String> {
        self.extra
            .get("OfficialRating")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
    fn community_rating(&self) -> Option<f64> {
        self.extra.get("CommunityRating").and_then(|v| v.as_f64())
    }
    fn sort_name(&self) -> Option<String> {
        self.extra
            .get("SortName")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }
    /// `MediaSources[0].MediaStreams`, else the item's own `MediaStreams`.
    fn media_streams(&self) -> Vec<MediaStream> {
        let from_source = self
            .extra
            .get("MediaSources")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.get("MediaStreams"))
            .and_then(|v| serde_json::from_value::<Vec<MediaStream>>(v.clone()).ok());
        from_source.unwrap_or_else(|| typed(self, "MediaStreams"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_timestamp_matches_known_instants() {
        assert_eq!(iso_from_secs(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_secs(1_709_210_096), "2024-02-29T12:34:56.000Z");
        assert_eq!(released_now().len(), 24);
    }

    #[test]
    fn video_fields_are_read_from_extra() {
        let item: BaseItemDto = serde_json::from_value(serde_json::json!({
            "Id": "m", "Type": "Movie", "OfficialRating": "PG-13", "CommunityRating": 7.84,
            "LocalTrailerCount": 1,
            "People": [{"Id": "p1", "Name": "A", "Role": "Hero", "Type": "Actor", "PrimaryImageTag": "t"}],
            "RemoteTrailers": [{"Url": "https://youtu.be/abcdefghijk", "Name": "T"}],
            "MediaSources": [{"MediaStreams": [{"Type": "Video", "Width": 3840, "Height": 1600}]}]
        }))
        .unwrap();
        assert_eq!(item.official_rating().as_deref(), Some("PG-13"));
        assert_eq!(item.community_rating(), Some(7.84));
        assert_eq!(item.local_trailer_count(), Some(1));
        assert_eq!(item.people()[0].role.as_deref(), Some("Hero"));
        assert_eq!(item.remote_trailers().len(), 1);
        assert_eq!(item.media_streams()[0].width, Some(3840));
    }
}
