//! Music player panels: lyrics (server and LRCLIB), Up Next queue, radio, the track "…" menu, and
//! the wiring of the crossfade / audio output preferences to the engine.
pub mod host;
pub mod lrclib;
pub mod lyrics;
pub mod menus;
pub mod queue;
pub mod radio;

pub use host::PanelsHost;
pub use lyrics::LyricsView;
pub use menus::{use_track_menu, TrackMenu};
pub use queue::QueueView;
pub use radio::{start_radio, RadioIndicator};
