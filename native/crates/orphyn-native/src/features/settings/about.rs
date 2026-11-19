//! Settings > About (`AboutPane.tsx`): logo and version, what's new, updates, the connected
//! server, links and the credit.
use dioxus::prelude::*;

use super::ui::{List, ListRow};
use crate::design::Icon;
use crate::features::updater::UpdatesSection;
use crate::features::whatsnew::{bundled_notes, WhatsNewSheet};
use crate::i18n::use_i18n;
use crate::state::use_services;

const JELLYFIN_URL: &str = "https://jellyfin.org";
const JELLYFIN_DOCS_URL: &str = "https://jellyfin.org/docs/";
const AUTHOR_URL: &str = "https://dyskolos.fr";

/// Opens `url` in the system browser (`window.open(url, '_blank')`).
pub fn open_external(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn value(text: String) -> Element {
    rsx! { span { class: "jr-text-callout jr-color-secondary", "{text}" } }
}

fn link_icon() -> Element {
    rsx! { span { class: "jr-settings__link-icon", "aria-hidden": "true", Icon { name: "external-link", size: 16 } } }
}

#[component]
pub fn AboutPane() -> Element {
    let t = use_i18n();
    let svc = use_services();
    let version = crate::features::system::app_version();
    let mut show_notes = use_signal(|| false);
    let notes: Vec<_> = bundled_notes()
        .into_iter()
        .filter(|n| n.version == version)
        .collect();
    let (account, encrypted) = match svc.accounts.as_ref() {
        Some(store) => {
            let s = store.lock().unwrap_or_else(|p| p.into_inner());
            (s.active(), Some(s.status().encrypted))
        }
        None => (None, None),
    };
    let linux = cfg!(target_os = "linux");
    rsx! {
        div { class: "jr-settings__about",
            img {
                class: "jr-brand-logo jr-settings__about-logo",
                src: crate::assets::logo_url(),
                width: "72",
                height: "72",
                alt: "",
            }
            h1 { class: "jr-text-title-1 jr-color-primary", {t.t("settings:about.app")} }
            span { class: "jr-text-callout jr-color-secondary", {t.tf("settings:about.version", &[("version", version.clone())])} }
        }
        if !notes.is_empty() {
            List {
                ListRow {
                    title: rsx! { {t.t("settings:about.whatsNew")} },
                    subtitle: Some(t.tf("settings:about.whatsNewDescription", &[("version", version.clone())])),
                    trailing: Some(rsx! { span { class: "jr-settings__link-icon", "aria-hidden": "true", Icon { name: "sparkles", size: 16 } } }),
                    onclick: move |_| show_notes.set(true),
                }
            }
        }
        if show_notes() {
            WhatsNewSheet { notes: notes.clone(), on_close: move |_| show_notes.set(false) }
        }
        UpdatesSection {}
        List { header: t.t("settings:about.server"),
            if let Some(a) = account {
                ListRow { title: rsx! { {t.t("settings:about.serverName")} }, trailing: Some(value(a.server_name.clone())) }
                ListRow { title: rsx! { {t.t("settings:about.serverVersion")} }, trailing: Some(value(a.version.clone())) }
                ListRow { title: rsx! { {t.t("settings:about.serverUrl")} }, trailing: Some(value(a.url.clone())) }
                ListRow { title: rsx! { {t.t("settings:about.user")} }, trailing: Some(value(a.user_name.clone())) }
            } else {
                ListRow { title: rsx! { {t.t("settings:about.noServer")} } }
            }
            if let Some(enc) = encrypted {
                ListRow {
                    title: rsx! { {t.t("settings:about.tokenStorage")} },
                    trailing: Some(value(t.t(if enc {
                        "settings:about.encrypted"
                    } else if linux {
                        "settings:about.notEncryptedLinux"
                    } else {
                        "settings:about.notEncrypted"
                    }))),
                }
            }
        }
        List { header: t.t("settings:about.links"),
            ListRow {
                title: rsx! { {t.t("settings:about.jellyfin")} },
                subtitle: Some(t.t("settings:about.jellyfinDescription")),
                trailing: Some(link_icon()),
                onclick: move |_| open_external(JELLYFIN_URL),
            }
            ListRow {
                title: rsx! { {t.t("settings:about.docs")} },
                trailing: Some(link_icon()),
                onclick: move |_| open_external(JELLYFIN_DOCS_URL),
            }
        }
        p { class: "jr-text-footnote jr-color-secondary jr-settings__credit",
            {t.t("settings:about.credit.before")}
            a {
                href: AUTHOR_URL,
                onclick: move |e| {
                    e.prevent_default();
                    open_external(AUTHOR_URL);
                },
                {t.t("settings:about.credit.author")}
            }
        }
    }
}
