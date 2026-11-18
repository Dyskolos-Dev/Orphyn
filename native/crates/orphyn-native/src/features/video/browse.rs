//! Video browsing: the home ("Watch Now"), the movie / series libraries and the item pages
//! (`src/renderer/src/features/video`). Playing goes through `/video/play/<id>`.
mod cards;
mod data;
mod format;
mod home;
mod item;
pub mod layout;
mod library;
mod trailer;
mod trailers;
mod ui;

pub use home::VideoHomePage;
pub use item::VideoItemPage;
pub use library::VideoLibraryPage;
