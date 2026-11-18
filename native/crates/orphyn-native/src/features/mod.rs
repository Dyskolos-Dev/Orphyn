//! Feature screens, routed like `app/routes.tsx`. A screen not ported yet renders the `StubPage`
//! of the React app (its nav label as the name); porting one = replacing its arm below.
pub mod music;
pub mod servers;

use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::router::{use_router, Route};
use crate::shell::StubPage;
use crate::state::use_app;

#[component]
pub fn RoutedPage() -> Element {
    let t = use_i18n();
    let router = use_router();
    let app = use_app();
    let route = router.route();
    // ServerGuard: without a signed-in server every route shows the onboarding.
    if (app.session)().is_none() {
        return rsx! { servers::OnboardingPage {} };
    }
    let stub = |key: &str| rsx! { StubPage { name: t.t(key) } };
    match route {
        Route::MusicAlbum { id } => rsx! { music::AlbumPage { key: "{id}", id } },
        Route::Onboarding => rsx! { servers::OnboardingPage {} },
        Route::VideoHome => stub("shell:nav.home"),
        Route::VideoLibrary { library_id } => match library_id.as_str() {
            "movies" => stub("shell:nav.movies"),
            "series" => stub("shell:nav.series"),
            _ => stub("common:nav.video"),
        },
        Route::VideoItem { .. } | Route::VideoPlay { .. } => stub("common:nav.video"),
        Route::MusicHome => stub("shell:nav.music"),
        Route::MusicAlbums => stub("shell:nav.albums"),
        Route::MusicArtists | Route::MusicArtist { .. } => stub("shell:nav.artists"),
        Route::MusicPlaylists | Route::MusicPlaylist { .. } => stub("shell:nav.playlists"),
        Route::MusicFavorites => stub("music:nav.favorites"),
        Route::MusicSongs => stub("shell:nav.songs"),
        Route::Search { .. } => stub("common:nav.search"),
        Route::Settings => rsx! { servers::QuickSettings {} },
        Route::Cast => stub("common:nav.cast"),
        Route::Stats => stub("stats:nav"),
        Route::NotFound { path } => rsx! { StubPage { name: path } },
    }
}
