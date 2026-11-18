//! Year in review ("Ton année").
use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::shell::StubPage;

#[component]
pub fn StatsPage() -> Element {
    let t = use_i18n();
    rsx! { StubPage { name: t.t("stats:nav") } }
}
