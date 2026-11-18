//! Music engine: queue model (port of `queue.ts`), controller (`store.ts` + `api.ts`) and the
//! audio output thread (rodio + symphonia, gapless hand-over).
mod output;
pub mod player;
pub mod queue;
pub mod types;

pub use player::{MusicPlayer, Resolver};
pub use types::*;
