//! Video player screen (controls, tracks, skip intro/credits, next episode). Replace the stub when porting.
use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::shell::StubPage;

#[component]
pub fn VideoPlayerPage(item_id: String) -> Element {
    let _ = item_id;
    let t = use_i18n();
    rsx! { StubPage { name: t.t("common:nav.video") } }
}
