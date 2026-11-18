//! Music library screens: home, albums, artists, artist page, playlists, favourites, songs.
mod artist;
mod artist_groups;
mod cards;
mod detail;
mod home;
mod lists;
mod paged;
mod playlist;
mod songs;

pub use artist::ArtistPage;
pub use home::MusicHomePage;
pub use lists::{AlbumsPage, ArtistsPage, FavoritesPage, PlaylistsPage};
pub use playlist::PlaylistPage;
pub use songs::SongsPage;
