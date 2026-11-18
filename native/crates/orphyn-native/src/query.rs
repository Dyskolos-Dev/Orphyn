//! `use_query`: the TanStack `useQuery` of this app, over [`orphyn_jellyfin::QueryCache`].
//!
//! ```ignore
//! let album = use_query(keys::item(&s.account_id, &id), stale::ITEM, {
//!     let s = s.clone(); let id = id.clone();
//!     move || { let s = s.clone(); let id = id.clone(); async move { s.item(&id).await } }
//! });
//! if album.is_loading() { return rsx! { Spinner {} } }
//! ```
//!
//! Renders cached data at once (stale-while-revalidate), fetches when the key has no data or is
//! stale / invalidated, shares one request between every component asking for the same key, and
//! re-renders when the cache changes (a mutation, an invalidation, another screen's fetch).
use std::future::Future;
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_jellyfin::keys::Key;
use orphyn_jellyfin::{JfResult, QueryState};

use crate::state::use_services;

/// Re-renders the calling component whenever the query cache changes.
pub fn use_cache_updates() {
    let cache = use_services().cache;
    let mut tick = use_signal(|| 0u64);
    use_hook(move || {
        let mut rx = cache.subscribe();
        spawn(async move {
            while rx.changed().await.is_ok() {
                tick += 1;
            }
        });
    });
    let _ = tick();
}

pub fn use_query<T, F, Fut>(key: Key, stale: Duration, fetcher: F) -> QueryState<T>
where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = JfResult<T>> + Send + 'static,
{
    let cache = use_services().cache;
    use_cache_updates();
    let state = cache.peek::<T>(&key);
    let due = !state.fetching && state.error.is_none() && (state.data.is_none() || state.stale);
    if due {
        let fut = fetcher();
        let cache = cache.clone();
        spawn(async move {
            let _ = cache.fetch(key, stale, move || fut).await;
        });
    }
    state
}
