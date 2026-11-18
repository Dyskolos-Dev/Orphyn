//! Video browsing: home, movie/series libraries, item detail. Replace the stubs when porting.
use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::shell::StubPage;

#[component]
pub fn VideoHomePage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("shell:nav.home") } }
}

#[component]
pub fn VideoLibraryPage(library_id: String) -> Element {
    let t = use_i18n();
    let key = match library_id.as_str() {
        "movies" => "shell:nav.movies",
        "series" => "shell:nav.series",
        _ => "common:nav.video",
    };
    rsx! { StubPage { name: t.t(key) } }
}

#[component]
pub fn VideoItemPage(item_id: String) -> Element {
    let _ = item_id;
    let t = use_i18n();
    rsx! { StubPage { name: t.t("common:nav.video") } }
}
