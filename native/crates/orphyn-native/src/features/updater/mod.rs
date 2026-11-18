//! Updater UI: the "ready / available" notice (`shell/UpdateNotice.tsx`) and the Updates section
//! of Settings > About (`AboutPane.tsx`). The logic is `orphyn_core::updater`.
use dioxus::prelude::*;
use orphyn_core::updater::state::{release_url, UpdaterState, UpdaterStatus};

use crate::design::Button;
use crate::features::discord::Switch;
use crate::features::system::use_system;
use crate::i18n::use_i18n;

/// The updater state as a signal, refreshed when the service publishes.
pub fn use_updater_state() -> Signal<UpdaterState> {
    let updater = use_system().updater;
    let mut state = use_signal(|| updater.state());
    use_hook(move || {
        let mut rx = updater.subscribe();
        spawn(async move {
            while rx.changed().await.is_ok() {
                let next = rx.borrow_and_update().clone();
                state.set(next);
            }
        });
    });
    state
}

fn open_release(version: Option<&str>) {
    let url = release_url(version);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}

fn status_key(s: UpdaterStatus) -> &'static str {
    match s {
        UpdaterStatus::Idle => "idle",
        UpdaterStatus::Checking => "checking",
        UpdaterStatus::UpToDate => "up-to-date",
        UpdaterStatus::Available => "available",
        UpdaterStatus::Downloading => "downloading",
        UpdaterStatus::Ready => "ready",
        UpdaterStatus::Error => "error",
    }
}

/// Installs the downloaded update, then quits (the installer restarts the app).
fn install_and_exit(updater: &orphyn_core::updater::UpdaterService) {
    match updater.install() {
        Ok(true) => std::process::exit(0),
        Ok(false) => {}
        Err(e) => log::warn!("[updater] install failed: {e}"),
    }
}

/// Toast-like notice, shown while an update is ready (restart) or available (download page).
#[component]
pub fn UpdateNotice() -> Element {
    let t = use_i18n();
    let updater = use_system().updater;
    let state = use_updater_state();
    let mut dismissed = use_signal(String::new);
    let s = state();
    let Some(version) = s.version.clone() else {
        return rsx! {};
    };
    let key = format!("{}:{version}", status_key(s.status));
    if dismissed() == key || !matches!(s.status, UpdaterStatus::Ready | UpdaterStatus::Available) {
        return rsx! {};
    }
    let ready = s.status == UpdaterStatus::Ready;
    let args = [("version", version.clone())];
    let title = t.tf(
        if ready {
            "shell:update.ready"
        } else {
            "shell:update.available"
        },
        &args,
    );
    let action = t.t(if ready {
        "shell:update.restart"
    } else {
        "shell:update.download"
    });
    rsx! {
        div { class: "jr-toast-viewport",
            div { class: "jr-toast-stack", role: "status", "aria-live": "polite",
                div { class: "jr-toast jr-material-thick",
                    span { class: "jr-toast__icon", "aria-hidden": "true",
                        img { src: crate::assets::logo_url(), width: "28", height: "28", alt: "" }
                    }
                    span { class: "jr-toast__text",
                        span { class: "jr-toast__title", "{title}" }
                        if ready {
                            span { class: "jr-toast__description", {t.t("shell:update.readyHint")} }
                        }
                    }
                    button {
                        r#type: "button",
                        class: "jr-toast__action",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: {
                            let updater = updater.clone();
                            let version = version.clone();
                            move |_| if ready { install_and_exit(&updater) } else { open_release(Some(&version)) }
                        },
                        "{action}"
                    }
                    button {
                        r#type: "button",
                        class: "jr-toast__action",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |_| dismissed.set(key.clone()),
                        {t.t("shell:update.later")}
                    }
                }
            }
        }
    }
}

/// The "Updates" list of the About pane: check / download / restart and the automatic toggle.
#[component]
pub fn UpdatesSection() -> Element {
    let t = use_i18n();
    let updater = use_system().updater;
    let state = use_updater_state();
    let s = state();
    let text = if s.supported {
        t.tf(
            &format!("settings:about.status.{}", status_key(s.status)),
            &[
                ("version", s.version.clone().unwrap_or_default()),
                ("percent", s.percent.to_string()),
                ("error", s.error.clone().unwrap_or_default()),
            ],
        )
    } else {
        t.t("settings:about.status.unsupported")
    };
    let busy = matches!(
        s.status,
        UpdaterStatus::Checking | UpdaterStatus::Downloading
    );
    let (up_check, up_install, up_auto) = (updater.clone(), updater.clone(), updater);
    let version = s.version.clone();
    rsx! {
        div { class: "jr-list-group jr-list-group--inset",
            div { class: "jr-list-group__header", {t.t("settings:about.updates")} }
            ul { class: "jr-list",
                li { class: "jr-list__item",
                    div { class: "jr-row",
                        span { class: "jr-row__text",
                            span { class: "jr-row__title", {t.t("settings:about.checkForUpdates")} }
                            span { class: "jr-row__subtitle", "{text}" }
                        }
                        span { class: "jr-row__trailing",
                            match s.status {
                                UpdaterStatus::Ready => rsx! {
                                    Button { size: "small", onclick: move |_| install_and_exit(&up_install), {t.t("settings:about.restart")} }
                                },
                                UpdaterStatus::Available => rsx! {
                                    Button { size: "small", onclick: move |_| open_release(version.as_deref()), {t.t("settings:about.download")} }
                                },
                                _ => rsx! {
                                    Button {
                                        size: "small",
                                        variant: "gray",
                                        disabled: !s.supported || busy,
                                        onclick: move |_| {
                                            let u = up_check.clone();
                                            spawn(async move { u.check().await });
                                        },
                                        {t.t("settings:about.checkForUpdates")}
                                    }
                                },
                            }
                        }
                    }
                }
                li { class: "jr-list__item",
                    div { class: "jr-row",
                        span { class: "jr-row__text",
                            span { class: "jr-row__title", {t.t("settings:about.autoUpdate")} }
                            span { class: "jr-row__subtitle", {t.t("settings:about.autoUpdateDescription")} }
                        }
                        span { class: "jr-row__trailing",
                            Switch {
                                checked: s.auto_update,
                                label: t.t("settings:about.autoUpdate"),
                                on_change: move |on| up_auto.set_auto_update(on),
                            }
                        }
                    }
                }
            }
        }
    }
}
