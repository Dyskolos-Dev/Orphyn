//! Video engine: libmpv loaded at run time, software render into a wgpu texture.
pub mod controller;
pub mod mpv;
#[cfg(feature = "widget")]
mod widget;

pub use controller::{PlayOptions, PlaybackSnapshot, PlaybackStatus, VideoController};
#[cfg(feature = "widget")]
pub use widget::VideoWidget;
