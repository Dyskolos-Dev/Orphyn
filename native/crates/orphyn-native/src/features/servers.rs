//! Sign-in, enough for the foundation: the onboarding frame of `OnboardingPage.tsx` with the
//! address + sign-in steps of `ConnectFlow.tsx` on one form (same classes and strings). The full
//! multi-step flow, server switcher and relogin come with the servers port.
//!
//! `QuickSettings` stands in for `/settings` until the settings port: the appearance, interface
//! style and language switches, wired to the real stores.
use dioxus::prelude::*;
use orphyn_jellyfin::auth::LoginErrorKind;

use crate::design::{Button, Segment, SegmentedControl};
use crate::i18n::{use_i18n, Lang, LanguageChoice};
use crate::shell::{use_shell, ShellPage};
use crate::state::{use_app, use_services};
use crate::theme::{ThemeMode, INTERFACE_STYLES};

#[component]
fn Field(label: String, value: Signal<String>, #[props(default = "text")] kind: &'static str, #[props(default)] placeholder: String) -> Element {
    let mut value = value;
    rsx! {
        div { class: "jr-field jr-field--large",
            label { class: "jr-field__label", "{label}" }
            div { class: "jr-field__box",
                input {
                    class: "jr-field__input",
                    r#type: kind,
                    placeholder,
                    value: "{value}",
                    oninput: move |e| value.set(e.value()),
                }
            }
        }
    }
}

#[component]
pub fn OnboardingPage() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let mut shell = use_shell();
    // ShellImmersive: the onboarding takes the whole window.
    use_hook(move || shell.immersive += 1);
    use_drop(move || shell.immersive -= 1);
    let url = use_signal(|| std::env::var("ORPHYN_E2E_URL").unwrap_or_default());
    let user = use_signal(String::new);
    let password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let submit = move |_| {
        if busy() {
            return;
        }
        busy.set(true);
        error.set(None);
        let svc = svc.clone();
        spawn(async move {
            let r = app.sign_in(svc, url(), user(), password()).await;
            busy.set(false);
            if let Err(e) = r {
                let key = match e.kind {
                    LoginErrorKind::InvalidCredentials => "servers:errors.invalid_credentials",
                    LoginErrorKind::Unreachable => "servers:errors.unreachable",
                    LoginErrorKind::ServerError => "servers:errors.server_error",
                };
                error.set(Some(t.t(key)));
            }
        });
    };
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
                    img { class: "jr-onboarding__logo", src: crate::assets::logo_url(), width: "96", height: "96", alt: "" }
                }
                div { class: "jr-onboarding__flow",
                    div { class: "jr-connect jr-connect--onboarding",
                        div { class: "jr-connect__stage",
                            div { class: "jr-connect__step",
                                h1 { class: "jr-connect__title", {t.t("servers:address.title")} }
                                p { class: "jr-connect__subtitle", {t.t("servers:address.subtitle")} }
                                div { class: "jr-connect__fields",
                                    Field { label: t.t("servers:address.label"), value: url, placeholder: t.t("servers:address.placeholder") }
                                    Field { label: t.t("servers:login.username"), value: user }
                                    Field { label: t.t("servers:login.password"), value: password, kind: "password" }
                                    if let Some(e) = error() {
                                        span { class: "jr-field__error", role: "alert", "{e}" }
                                    }
                                }
                                div { class: "jr-connect__actions",
                                    Button { size: "large", full_width: true, disabled: busy(), onclick: submit,
                                        {t.t("servers:login.signIn")}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn QuickSettings() -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let lang = use_context::<crate::i18n::I18n>().lang;
    let mode = app.theme.read().mode.as_str();
    let style = app.interface.read().style;
    let language = (app.language)().as_str();
    let (s1, s2, s3) = (svc.clone(), svc.clone(), svc);
    rsx! {
        ShellPage { title: t.t("settings:title"), gutter: true,
            div { class: "jr-settings-quick", style: "display: flex; flex-direction: column; gap: 20px; max-width: 560px;",
                p { class: "jr-text-headline", {t.t("settings:appearance.mode.header")} }
                SegmentedControl {
                    label: t.t("settings:appearance.mode.label"),
                    value: mode,
                    options: ["system", "light", "dark"].iter().map(|m| Segment { value: m, label: t.t(&format!("settings:appearance.mode.{m}")) }).collect::<Vec<_>>(),
                    on_change: move |v: &'static str| app.set_theme(&s1, |th| th.mode = ThemeMode::parse(Some(v))),
                }
                p { class: "jr-text-headline", {t.t("settings:appearance.style.header")} }
                SegmentedControl {
                    label: t.t("settings:appearance.style.label"),
                    value: style,
                    options: INTERFACE_STYLES.iter().map(|s| Segment { value: s, label: capitalize(s) }).collect::<Vec<_>>(),
                    on_change: move |v: &'static str| app.set_style(&s2, v),
                }
                p { class: "jr-text-headline", {t.t("settings:general.language.header")} }
                SegmentedControl {
                    label: t.t("settings:general.language.label"),
                    value: language,
                    options: ["system", "fr", "en"].iter().map(|l| Segment { value: l, label: t.t(&format!("settings:general.language.{l}")) }).collect::<Vec<_>>(),
                    on_change: move |v: &'static str| {
                        let choice = match v {
                            "fr" => LanguageChoice::Fixed(Lang::Fr),
                            "en" => LanguageChoice::Fixed(Lang::En),
                            _ => LanguageChoice::System,
                        };
                        app.set_language(&s3, choice, lang);
                    },
                }
            }
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}
