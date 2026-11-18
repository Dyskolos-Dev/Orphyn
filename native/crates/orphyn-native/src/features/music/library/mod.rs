//! Music library screens: home, albums, artists, artist page, playlists, favourites, songs.
use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::shell::StubPage;

#[component]
pub fn MusicHomePage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.music") } }
}

#[component]
pub fn AlbumsPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.albums") } }
}

#[component]
pub fn ArtistsPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.artists") } }
}

#[component]
pub fn PlaylistsPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.playlists") } }
}

#[component]
pub fn FavoritesPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("music:nav.favorites") } }
}

#[component]
pub fn SongsPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.songs") } }
}

#[component]
pub fn ArtistPage(id: String) -> Element {
    let _ = id;
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.artists") } }
}

#[component]
pub fn PlaylistPage(id: String) -> Element {
    let _ = id;
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.playlists") } }
}
