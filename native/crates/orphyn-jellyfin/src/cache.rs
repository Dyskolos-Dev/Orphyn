//! Query cache: the TanStack Query equivalent of the React app.
//!
//! - **Dedup**: concurrent `fetch`es of one key share a single request.
//! - **Stale-while-revalidate**: `peek` returns cached data at once with a `stale` flag; the UI
//!   shows it and calls `fetch`, which refetches only when the entry is stale or invalidated.
//! - **Invalidation**: `invalidate(prefix)` marks every key starting with `prefix` stale and bumps
//!   the generation, so mounted screens refetch (`["jf", acc]` = everything of one server).
//! - **Updates**: `set` writes data directly (optimistic favourites, mutations).
//!
//! Values are stored type-erased (`Arc<dyn Any>`); a key always holds one type.
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures_util::future::{BoxFuture, FutureExt, Shared};
use tokio::sync::watch;

use crate::client::{JfError, JfResult};
use crate::keys::Key;

type Value = Arc<dyn Any + Send + Sync>;
type Inflight = Shared<BoxFuture<'static, Result<Value, JfError>>>;

#[derive(Default)]
struct Entry {
    data: Option<Value>,
    updated: Option<Instant>,
    stale_time: Duration,
    invalidated: bool,
    error: Option<JfError>,
    inflight: Option<Inflight>,
}

impl Entry {
    fn is_stale(&self) -> bool {
        self.invalidated
            || match self.updated {
                Some(t) => t.elapsed() >= self.stale_time,
                None => true,
            }
    }
}

/// What a screen renders for one key.
#[derive(Debug)]
pub struct QueryState<T> {
    pub data: Option<Arc<T>>,
    /// The data is older than its stale time or was invalidated: a refetch is due.
    pub stale: bool,
    /// A request for this key is running.
    pub fetching: bool,
    /// The last request failed (data, if any, is the previous good value).
    pub error: Option<JfError>,
}

impl<T> Clone for QueryState<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            stale: self.stale,
            fetching: self.fetching,
            error: self.error.clone(),
        }
    }
}

impl<T> QueryState<T> {
    /// No data yet and a request running or due: show the spinner.
    pub fn is_loading(&self) -> bool {
        self.data.is_none() && self.error.is_none()
    }
}

#[derive(Clone)]
pub struct QueryCache {
    entries: Arc<Mutex<HashMap<Key, Entry>>>,
    generation: Arc<watch::Sender<u64>>,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

fn starts_with(key: &Key, prefix: &[String]) -> bool {
    key.len() >= prefix.len() && key[..prefix.len()] == *prefix
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::default(),
            generation: Arc::new(watch::channel(0).0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Key, Entry>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn bump(&self) {
        self.generation.send_modify(|g| *g += 1);
    }

    /// Changes whenever any entry changes: UI hooks re-`peek` when it moves.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    /// The cached state of `key`, without fetching.
    pub fn peek<T: Send + Sync + 'static>(&self, key: &Key) -> QueryState<T> {
        let map = self.lock();
        match map.get(key) {
            None => QueryState {
                data: None,
                stale: true,
                fetching: false,
                error: None,
            },
            Some(e) => QueryState {
                data: e.data.clone().and_then(|d| d.downcast::<T>().ok()),
                stale: e.is_stale(),
                fetching: e.inflight.is_some(),
                error: e.error.clone(),
            },
        }
    }

    /// Returns fresh data from the cache, or runs `fetcher` (once for all concurrent callers),
    /// stores its result and returns it.
    pub async fn fetch<T, F, Fut>(
        &self,
        key: Key,
        stale_time: Duration,
        fetcher: F,
    ) -> JfResult<Arc<T>>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = JfResult<T>> + Send + 'static,
    {
        let shared = {
            let mut map = self.lock();
            let entry = map.entry(key.clone()).or_default();
            entry.stale_time = stale_time;
            if !entry.is_stale() {
                if let Some(d) = entry.data.clone().and_then(|d| d.downcast::<T>().ok()) {
                    return Ok(d);
                }
            }
            match &entry.inflight {
                Some(f) => f.clone(),
                None => {
                    let fut = fetcher();
                    let cache = self.clone();
                    let k = key.clone();
                    let shared: Inflight = async move {
                        let result = fut.await.map(|v| Arc::new(v) as Value);
                        cache.complete(&k, &result);
                        result
                    }
                    .boxed()
                    .shared();
                    entry.inflight = Some(shared.clone());
                    // Run it to completion even if every caller goes away (a screen unmounted),
                    // otherwise the entry would stay "fetching" forever.
                    if let Ok(rt) = tokio::runtime::Handle::try_current() {
                        let detached = shared.clone();
                        rt.spawn(async move {
                            let _ = detached.await;
                        });
                    }
                    shared
                }
            }
        };
        self.bump();
        let value = shared.await?;
        value
            .downcast::<T>()
            .map_err(|_| JfError::Decode("cache key holds another type".into()))
    }

    fn complete(&self, key: &Key, result: &Result<Value, JfError>) {
        {
            let mut map = self.lock();
            let entry = map.entry(key.clone()).or_default();
            entry.inflight = None;
            match result {
                Ok(v) => {
                    entry.data = Some(v.clone());
                    entry.updated = Some(Instant::now());
                    entry.invalidated = false;
                    entry.error = None;
                }
                Err(e) => entry.error = Some(e.clone()),
            }
        }
        self.bump();
    }

    /// Writes `value` as fresh data for `key` (mutations, optimistic updates).
    pub fn set<T: Send + Sync + 'static>(&self, key: Key, value: T) {
        {
            let mut map = self.lock();
            let entry = map.entry(key).or_default();
            if entry.stale_time.is_zero() {
                // Never fetched yet: the first `fetch` sets the real stale time.
                entry.stale_time = crate::keys::stale::ITEM;
            }
            entry.data = Some(Arc::new(value));
            entry.updated = Some(Instant::now());
            entry.invalidated = false;
            entry.error = None;
        }
        self.bump();
    }

    /// Updates the cached value of `key` in place, if there is one of type `T`.
    pub fn update<T: Clone + Send + Sync + 'static>(&self, key: &Key, f: impl FnOnce(&mut T)) {
        let changed = {
            let mut map = self.lock();
            match map.get_mut(key) {
                Some(entry) => match entry.data.clone().and_then(|d| d.downcast::<T>().ok()) {
                    Some(old) => {
                        let mut next = (*old).clone();
                        f(&mut next);
                        entry.data = Some(Arc::new(next));
                        true
                    }
                    None => false,
                },
                None => false,
            }
        };
        if changed {
            self.bump();
        }
    }

    /// Marks every key starting with `prefix` stale (data is kept and still shown).
    pub fn invalidate(&self, prefix: &[String]) {
        {
            let mut map = self.lock();
            for (k, e) in map.iter_mut() {
                if starts_with(k, prefix) {
                    e.invalidated = true;
                }
            }
        }
        self.bump();
    }

    /// Drops every key starting with `prefix` (sign-out of a server).
    pub fn remove(&self, prefix: &[String]) {
        self.lock().retain(|k, _| !starts_with(k, prefix));
        self.bump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(s: &str) -> Key {
        vec!["jf".into(), "acc".into(), s.into()]
    }

    #[tokio::test]
    async fn concurrent_fetches_share_one_request() {
        let cache = QueryCache::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let run = || {
            let calls = calls.clone();
            cache.fetch(key("a"), Duration::from_secs(60), move || async move {
                calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok::<_, JfError>(42u32)
            })
        };
        let (a, b) = tokio::join!(run(), run());
        assert_eq!((*a.unwrap(), *b.unwrap()), (42, 42));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Fresh: served from the cache.
        let c = run().await.unwrap();
        assert_eq!(*c, 42);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalidation_keeps_data_and_refetches() {
        let cache = QueryCache::new();
        cache.set(key("a"), 1u32);
        cache.set(vec!["jf".into(), "other".into(), "a".into()], 9u32);
        cache.invalidate(&["jf".to_string(), "acc".to_string()]);
        let s = cache.peek::<u32>(&key("a"));
        assert_eq!(s.data.as_deref(), Some(&1));
        assert!(s.stale);
        assert!(
            !cache
                .peek::<u32>(&vec!["jf".into(), "other".into(), "a".into()])
                .stale
        );
        let v = cache
            .fetch(key("a"), Duration::from_secs(60), || async {
                Ok::<_, JfError>(2u32)
            })
            .await
            .unwrap();
        assert_eq!(*v, 2);
        assert!(!cache.peek::<u32>(&key("a")).stale);
    }

    #[tokio::test]
    async fn errors_keep_the_previous_data() {
        let cache = QueryCache::new();
        cache.set(key("a"), 1u32);
        cache.invalidate(&key("a"));
        let r = cache
            .fetch::<u32, _, _>(key("a"), Duration::ZERO, || async {
                Err(JfError::Http(500))
            })
            .await;
        assert_eq!(r.unwrap_err(), JfError::Http(500));
        let s = cache.peek::<u32>(&key("a"));
        assert_eq!(s.data.as_deref(), Some(&1));
        assert_eq!(s.error, Some(JfError::Http(500)));
    }

    #[tokio::test]
    async fn update_and_remove() {
        let cache = QueryCache::new();
        let mut rx = cache.subscribe();
        cache.set(key("a"), vec![1u32]);
        cache.update::<Vec<u32>>(&key("a"), |v| v.push(2));
        assert_eq!(
            cache.peek::<Vec<u32>>(&key("a")).data.as_deref(),
            Some(&vec![1, 2])
        );
        assert!(rx.has_changed().unwrap());
        rx.mark_unchanged();
        cache.remove(&["jf".to_string()]);
        assert!(cache.peek::<Vec<u32>>(&key("a")).data.is_none());
        assert!(rx.has_changed().unwrap());
    }
}
