//! Cached queries of the video screens: `use_query` bound to the signed-in session, with the
//! `enabled` flag of TanStack (a disabled query fetches nothing and answers with an empty value).
use std::future::Future;
use std::time::Duration;

use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::keys::{self, Key};
use orphyn_jellyfin::{BaseItemDto, ItemsPage, ItemsParams, JfResult, QueryState};

use crate::query::use_query;
use crate::state::use_app;

/// `use_query` for a fetch that needs the session. `key` receives the account id. While
/// `enabled` is false the query is not fetched and `T::default()` is the answer.
pub fn use_session_query<T, F, Fut>(
    key: impl FnOnce(&str) -> Key,
    stale: Duration,
    enabled: bool,
    fetch: F,
) -> QueryState<T>
where
    T: Default + Send + Sync + 'static,
    F: Fn(Session) -> Fut + 'static,
    Fut: Future<Output = JfResult<T>> + Send + 'static,
{
    let app = use_app();
    let session = (app.session)();
    let acc = session
        .as_ref()
        .map(|s| s.account_id.clone())
        .unwrap_or_default();
    let mut k = key(&acc);
    if !enabled {
        k.push("off".into());
    }
    use_query(k, stale, move || {
        let session = session.clone().filter(|_| enabled);
        let pending = session.map(&fetch);
        async move {
            match pending {
                Some(f) => f.await,
                None => Ok(T::default()),
            }
        }
    })
}

/// `useItems` (first page): `GET /Items` for `params`, optionally released titles only.
pub fn use_items(params: ItemsParams, released_only: bool, enabled: bool) -> QueryState<ItemsPage> {
    let key_params = params.clone();
    use_session_query(
        move |acc| orphyn_jellyfin::api::video::keys::video_items(acc, &key_params, released_only),
        keys::stale::CATALOG,
        enabled,
        move |s| {
            let p = params.clone();
            async move { s.video_items_page(&p, 0, released_only).await }
        },
    )
}

/// The items of the first page, empty while loading.
pub fn items_of(q: &QueryState<ItemsPage>) -> Vec<BaseItemDto> {
    q.data.as_ref().map(|p| p.items.clone()).unwrap_or_default()
}

pub fn list_of(q: &QueryState<Vec<BaseItemDto>>) -> Vec<BaseItemDto> {
    q.data.as_deref().cloned().unwrap_or_default()
}

/// `useUserViews`.
pub fn use_views() -> QueryState<Vec<BaseItemDto>> {
    use_session_query(keys::views, keys::stale::VIEWS, true, |s| async move {
        s.user_views().await
    })
}

/// `useItem(id)`.
pub fn use_item(id: &str, enabled: bool) -> QueryState<BaseItemDto> {
    let id = id.to_string();
    let key_id = id.clone();
    use_session_query(
        move |acc| keys::item(acc, &key_id),
        keys::stale::ITEM,
        enabled && !id.is_empty(),
        move |s| {
            let id = id.clone();
            async move { s.item(&id).await }
        },
    )
}

/// `useResumeItems('video', limit)`.
pub fn use_resume(limit: usize) -> QueryState<Vec<BaseItemDto>> {
    use_session_query(
        move |acc| {
            let mut k = keys::resume(acc, "video");
            k.push(limit.to_string());
            k
        },
        keys::stale::PROGRESS,
        true,
        move |s| async move { s.resume("Video", limit as u32).await },
    )
}

/// `useSimilar(id, limit)`.
pub fn use_similar(id: Option<String>, limit: usize) -> QueryState<Vec<BaseItemDto>> {
    let key_id = id.clone().unwrap_or_default();
    let enabled = id.is_some();
    use_session_query(
        move |acc| {
            let mut k = keys::similar(acc, &key_id);
            k.push(limit.to_string());
            k
        },
        keys::stale::CATALOG,
        enabled,
        move |s| {
            let id = id.clone().unwrap_or_default();
            async move { s.similar(&id, limit as u32).await }
        },
    )
}

/// `useSeasons(seriesId)`.
pub fn use_seasons(series_id: Option<String>) -> QueryState<Vec<BaseItemDto>> {
    let key_id = series_id.clone().unwrap_or_default();
    let enabled = series_id.is_some();
    use_session_query(
        move |acc| orphyn_jellyfin::api::video::keys::seasons(acc, &key_id),
        keys::stale::CATALOG,
        enabled,
        move |s| {
            let id = series_id.clone().unwrap_or_default();
            async move { s.seasons(&id).await }
        },
    )
}

/// `useEpisodes(seriesId)`: every episode of a series.
pub fn use_episodes(series_id: Option<String>) -> QueryState<Vec<BaseItemDto>> {
    let key_id = series_id.clone().unwrap_or_default();
    let enabled = series_id.is_some();
    use_session_query(
        move |acc| orphyn_jellyfin::api::video::keys::episodes(acc, &key_id, None),
        keys::stale::ITEM,
        enabled,
        move |s| {
            let id = series_id.clone().unwrap_or_default();
            async move { s.episodes(&id, None).await }
        },
    )
}

/// `useLocalTrailers(id, count)`: no request for items that have none.
pub fn use_local_trailers(id: &str, count: Option<i64>) -> QueryState<Vec<BaseItemDto>> {
    let id = id.to_string();
    let key_id = id.clone();
    let enabled = !id.is_empty() && count != Some(0);
    use_session_query(
        move |acc| orphyn_jellyfin::api::video::keys::local_trailers(acc, &key_id),
        keys::stale::CATALOG,
        enabled,
        move |s| {
            let id = id.clone();
            async move { s.local_trailers(&id).await }
        },
    )
}
