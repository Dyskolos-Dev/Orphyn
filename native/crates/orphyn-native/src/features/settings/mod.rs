//! Settings page (every pane of the React app). Replace the stub when porting.
use dioxus::prelude::*;

#[component]
pub fn SettingsPage() -> Element {
    rsx! { crate::features::servers::QuickSettings {} }
}
