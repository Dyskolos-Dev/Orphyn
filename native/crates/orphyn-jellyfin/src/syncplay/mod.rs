//! Jellyfin SyncPlay (server 10.9+): the Rust counterpart of `src/renderer/src/jellyfin/syncplay`
//! (+ the pure decisions of `features/syncplay/logic.ts`). REST requests, WebSocket messages,
//! server clock estimation, command scheduling and drift correction.
pub mod logic;
pub mod protocol;
pub mod socket;
pub mod time_sync;

pub use logic::*;
pub use protocol::*;
pub use socket::{backoff_delay, socket_url, JellyfinSocket, SocketMessage, SocketStatus};
pub use time_sync::{median, time_sample, ClockOffsetEstimator, TimeSample};
