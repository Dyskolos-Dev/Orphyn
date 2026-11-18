//! Music engine: queue model (port of `queue.ts`), controller (`store.ts` + `api.ts`) and the
//! audio output thread (rodio + symphonia, gapless hand-over and crossfade),
//! output device choice, radios.
pub mod crossfade;
pub mod devices;
pub mod http;
mod output;
pub mod player;
pub mod queue;
pub mod radio;
pub mod types;

pub use player::{MusicPlayer, RemoteEvent, RemoteOutput, Resolver};
pub use types::*;
