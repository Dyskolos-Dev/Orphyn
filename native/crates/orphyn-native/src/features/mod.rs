//! Feature screens, routed like `app/routes.tsx`. A screen not ported yet renders the `StubPage`
//! of the React app (its nav label as the name); porting one = replacing its arm below.
pub mod cast;
pub mod discord;
pub mod music;
pub mod palette;
pub mod pip;
pub mod search;
pub mod servers;
pub mod settings;
pub mod stats;
pub mod syncplay;
pub mod system;
pub mod updater;
pub mod whatsnew;
pub mod video;

use dioxus::prelude::*;

use crate::router::{use_router, Route};
use crate::shell::StubPage;
use crate::state::use_app;

#[component]
pub fn RoutedPage() -> Element {
    let router = use_router();
    let app = use_app();
    let route = router.route();
    // ServerGuard: without a signed-in server every route shows the onboarding.
    if (app.session)().is_none() {
        return rsx! { servers::OnboardingPage {} };
    }
    match route {
        Route::MusicAlbum { id } => rsx! { music::AlbumPage { key: "{id}", id } },
        Route::Onboarding => rsx! { servers::OnboardingPage {} },
        Route::VideoHome => rsx! { video::browse::VideoHomePage {} },
        Route::VideoLibrary { library_id } => {
            rsx! { video::browse::VideoLibraryPage { key: "{library_id}", library_id } }
        }
        Route::VideoItem { item_id } => {
            rsx! { video::browse::VideoItemPage { key: "{item_id}", item_id } }
        }
        Route::VideoPlay {
            item_id,
            start,
            trailer,
        } => {
            // A trailer never resumes (start 0).
            let start = if trailer { Some(0) } else { start };
            rsx! { video::player::VideoPlayerPage { key: "{item_id}", item_id, start } }
        }
        Route::MusicHome => rsx! { music::library::MusicHomePage {} },
        Route::MusicAlbums => rsx! { music::library::AlbumsPage {} },
        Route::MusicArtists => rsx! { music::library::ArtistsPage {} },
        Route::MusicArtist { id } => rsx! { music::library::ArtistPage { key: "{id}", id } },
        Route::MusicPlaylists => rsx! { music::library::PlaylistsPage {} },
        Route::MusicPlaylist { id } => rsx! { music::library::PlaylistPage { key: "{id}", id } },
        Route::MusicFavorites => rsx! { music::library::FavoritesPage {} },
        Route::MusicSongs => rsx! { music::library::SongsPage {} },
        Route::Search { q } => rsx! { search::SearchPage { q } },
        Route::Settings => rsx! { settings::SettingsPage {} },
        Route::Cast => rsx! { cast::CastPage {} },
        Route::Stats => rsx! { stats::StatsPage {} },
        Route::NotFound { path } => rsx! { StubPage { name: path } },
    }
}
