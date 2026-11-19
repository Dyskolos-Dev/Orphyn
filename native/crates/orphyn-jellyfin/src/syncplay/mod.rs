//! Jellyfin SyncPlay (server 10.9+): the Rust counterpart of `src/renderer/src/jellyfin/syncplay`
//! (+ the pure decisions of `features/syncplay/logic.ts`). REST requests, WebSocket messages,
//! server clock estimation, command scheduling and drift correction.
#[cfg(test)]
mod live;
pub mod logic;
pub mod protocol;
pub mod socket;
pub mod time_sync;

pub use logic::*;
pub use protocol::*;
pub use socket::{backoff_delay, socket_url, JellyfinSocket, SocketMessage, SocketStatus};
pub use time_sync::{median, time_sample, ClockOffsetEstimator, TimeSample};

use std::collections::HashMap;

use crate::client::{Client, JfResult, Q};
use crate::endpoints::DEFAULT_FIELDS;
use crate::BaseItemDto;

/// The items of a group's queue, as this user sees them, keyed by normalised id. An id missing
/// from the result is an item the user cannot access (another library, parental rating).
pub async fn fetch_items_by_ids(
    client: &Client,
    user_id: &str,
    ids: &[String],
) -> JfResult<HashMap<String, BaseItemDto>> {
    let mut found = HashMap::new();
    let fields: Vec<String> = DEFAULT_FIELDS
        .iter()
        .chain(&["MediaSources"])
        .map(|s| s.to_string())
        .collect();
    // Long queues (a whole artist) go in chunks so the URL stays reasonable.
    for chunk in ids.chunks(100) {
        let raw: serde_json::Value = client
            .get(
                "/Items",
                &[
                    ("userId", Some(Q::from(user_id))),
                    ("ids", Some(Q::List(chunk.to_vec()))),
                    ("fields", Some(Q::List(fields.clone()))),
                    ("enableUserData", Some(true.into())),
                ],
            )
            .await?;
        for item in raw
            .get("Items")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            if let Ok(dto) = serde_json::from_value::<BaseItemDto>(item.clone()) {
                found.insert(dto.id.replace('-', "").to_lowercase(), dto);
            }
        }
    }
    Ok(found)
}
