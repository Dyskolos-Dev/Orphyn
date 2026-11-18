//! Player core of Orphyn native, with APIs mirroring the React app so screens port 1:1:
//!
//! - [`music::MusicPlayer`]: `musicPlayer` + the player store (`features/music/player`).
//! - [`video::VideoController`]: `PlaybackController` (`features/video/controller.ts`), on libmpv,
//!   and [`video::VideoWidget`] to paint its frames in the Blitz tree.
pub mod music;
pub mod video;
