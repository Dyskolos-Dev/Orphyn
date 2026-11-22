//! The application layout: port of `src/renderer/src/shell` (`AppShell`, `ShellPage`,
//! `ShellSidebar`, `ShellAmbient`, `StubPage`). Same DOM and classes as the React shell.
mod page;
mod sidebar;

use std::rc::Rc;

use dioxus::prelude::*;

pub use page::{use_page, PageHeader, ShellPage, StubPage};
pub use sidebar::ShellSidebar;

use crate::design::OverlayHost;
use crate::features;
use crate::i18n::use_i18n;
use crate::router::use_router;
use crate::state::use_app;

/// What the shell shares with pages (`ShellContext` of `shell/context.tsx`).
#[derive(Clone, Copy, PartialEq)]
pub struct ShellContext {
    /// Artwork behind the interface (`ShellAmbient`): pages set the artwork they show.
    pub ambient_art: Signal<Option<String>>,
    /// Pages that take the whole window (video player) hide the sidebar and the bar.
    pub immersive: Signal<u32>,
    /// The main scroller, for scroll restoration.
    pub scroller: Signal<Option<Rc<MountedData>>>,
}

pub fn use_shell() -> ShellContext {
    use_context::<ShellContext>()
}

#[component]
pub fn AppShell() -> Element {
    let t = use_i18n();
    let app = use_app();
    let router = use_router();
    let shell = use_context_provider(|| ShellContext {
        ambient_art: Signal::new(None),
        immersive: Signal::new(0),
        scroller: Signal::new(None),
    });
    let immersive = (shell.immersive)() > 0;
    features::watchlist::use_watchlist_provider();

    // Scroll restoration: a new page starts at the top, back / forward return to where it was.
    let nav_id = (router.nav_id)();
    use_effect(use_reactive!(|nav_id| {
        let _ = nav_id;
        let y = router.history.peek().scroll();
        if let Some(el) = shell.scroller.peek().clone() {
            spawn(async move {
                // Let the new page lay out before scrolling it.
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                let _ = el
                    .scroll(
                        dioxus::html::geometry::PixelsVector2D::new(0.0, y),
                        dioxus::html::ScrollBehavior::Instant,
                    )
                    .await;
            });
        }
    }));

    let backdrop = app.interface.read().appearance().backdrop != "off";
    let ambient = if backdrop {
        (shell.ambient_art)()
    } else {
        None
    };
    let shell_cls = if immersive {
        "jr-shell jr-shell--immersive"
    } else {
        "jr-shell"
    };
    let mut scroll_y = router.scroll_y;

    rsx! {
        div { class: "{shell_cls}",
            div { class: "jr-shell__ambient", "aria-hidden": "true", hidden: immersive,
                if let Some(src) = ambient {
                    div { key: "{src}", class: "jr-shell__ambient-art", style: "--ambient-image: url(\"{src}\");" }
                }
            }
            div { class: "jr-shell__sidebar", hidden: immersive, ShellSidebar {} }
            div { class: "jr-shell__main",
                main {
                    class: "jr-shell__scroll",
                    onmounted: move |e| shell.scroller.clone().set(Some(e.data())),
                    onscroll: move |e| scroll_y.set(e.scroll_top()),
                    features::RoutedPage {}
                }
                section {
                    class: "jr-shell__now-playing jr-material-thick",
                    "aria-label": t.t("shell:slots.nowPlaying"),
                    hidden: immersive,
                    features::music::MiniPlayer {}
                }
            }
            features::music::NowPlaying {}
            features::watchlist::WatchlistHost {}
            OverlayHost {}
        }
    }
}
