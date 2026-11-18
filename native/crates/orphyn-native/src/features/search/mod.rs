//! Global search page and the command palette.
use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::shell::StubPage;

#[component]
pub fn SearchPage(q: String) -> Element {
    let _ = q;
    let t = use_i18n();
    rsx! { StubPage { name: t.t("common:nav.search") } }
}
