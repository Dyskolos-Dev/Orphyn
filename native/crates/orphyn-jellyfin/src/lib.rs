//! Jellyfin data layer of Orphyn native: the Rust counterpart of `src/renderer/src/jellyfin`.
//!
//! - [`client`]: HTTP access with the Orphyn client identity (one place for headers, queries, 401).
//! - [`auth`]: `AuthenticateByName`, logout.
//! - [`endpoints`]: one typed fetcher per endpoint the app uses.
//! - [`images`]: image URLs (same source resolution as `images.ts`) with device-pixel sizing.
//! - [`cache`] + [`keys`]: the TanStack Query equivalent (dedup, stale-while-revalidate,
//!   invalidation by key prefix), keyed like `keys.ts`.
pub mod api;
pub mod auth;
pub mod cache;
pub mod client;
pub mod endpoints;
pub mod images;
pub mod keys;
pub mod syncplay;
pub mod types;

pub use cache::{QueryCache, QueryState};
pub use client::{Client, JfError, JfResult};
pub use types::{BaseItemDto, ItemsPage, ItemsParams, NameIdPair, UserItemData};

/// Same identity as the Tauri app (`src/shared/jellyfin`): the server lists Orphyn sessions alike.
pub const CLIENT_NAME: &str = "Orphyn";
pub const DEVICE_NAME: &str = "Orphyn Desktop";
/// Version string sent to the server (the React client sends its own `APP_VERSION`).
pub const APP_VERSION: &str = "0.1.0";

/// Ticks (100 ns) per second, the unit of `RunTimeTicks` and positions.
pub const TICKS_PER_SECOND: f64 = 10_000_000.0;
