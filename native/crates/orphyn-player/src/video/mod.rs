//! Video engine: libmpv loaded at run time, software render into a wgpu texture (YUV converted
//! and scaled on the GPU, see `mpv`).
pub mod controller;
pub mod mpv;
#[cfg(feature = "widget")]
mod widget;

pub use controller::{PlayOptions, PlaybackSnapshot, PlaybackStatus, VideoController};
pub use mpv::{MpvTrack, Pipeline};
#[cfg(feature = "widget")]
pub use widget::{contain, VideoWidget};
