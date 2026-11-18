//! `OnboardingPage.tsx`: the aurora, the logo and the connect flow, over the whole window.
use dioxus::prelude::*;

use super::ConnectFlow;
use crate::router::use_router;
use crate::shell::use_shell;

#[component]
pub fn OnboardingPage() -> Element {
    let router = use_router();
    let mut shell = use_shell();
    // ShellImmersive: the onboarding takes the whole window.
    use_hook(move || shell.immersive += 1);
    use_drop(move || shell.immersive -= 1);
    rsx! {
        div { class: "jr-onboarding",
            div { class: "jr-aurora", "aria-hidden": "true",
                span { class: "jr-aurora__blob jr-aurora__blob--indigo" }
                span { class: "jr-aurora__blob jr-aurora__blob--cyan" }
                span { class: "jr-aurora__blob jr-aurora__blob--lilac" }
            }
            div { class: "jr-onboarding__content",
                div { class: "jr-onboarding__brand",
                    span { class: "jr-onboarding__glow" }
                    img {
                        class: "jr-onboarding__logo",
                        src: crate::assets::logo_url(),
                        width: "96",
                        height: "96",
                        alt: "",
                    }
                }
                div { class: "jr-onboarding__flow",
                    ConnectFlow { variant: "page", on_done: move |_| router.replace("/") }
                }
            }
        }
    }
}
