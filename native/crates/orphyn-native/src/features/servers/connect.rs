//! `ConnectFlow.tsx`: address (probe) -> credentials -> success beat -> name. The password lives
//! in a signal only until the login call returns. The account is saved at login, renamed and
//! activated at the last step.
use std::time::Duration;

use dioxus::prelude::*;
use orphyn_core::accounts::NewServerAccount;
use orphyn_jellyfin::auth::{self, LoginErrorKind, PublicSystemInfo};
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{Client, JfError};

use super::TextField;
use crate::design::{Button, Icon, IconButton};
use crate::i18n::use_i18n;
use crate::state::{use_app, use_services};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Address,
    Login,
    Success,
    Name,
}

const DOT_STEPS: [Step; 3] = [Step::Address, Step::Login, Step::Name];
const SUCCESS_MS: u64 = 1300;

#[derive(Clone, PartialEq)]
struct Found {
    typed: String,
    url: String,
    info: PublicSystemInfo,
}

#[derive(Clone)]
struct Saved {
    account_id: String,
    name: String,
    session: Session,
}

/// Probes `typed`: as written when it has a scheme, else https then http.
async fn probe(typed: &str, device: &str) -> Result<(String, PublicSystemInfo), &'static str> {
    let base = typed.trim().trim_end_matches('/');
    let candidates: Vec<String> = if base.contains("://") {
        vec![base.to_string()]
    } else {
        vec![format!("https://{base}"), format!("http://{base}")]
    };
    let mut last = "invalid_url";
    for url in candidates {
        match auth::public_info(&url, device).await {
            Ok(info) => return Ok((url, info)),
            Err(JfError::Network(_)) => last = "unreachable",
            Err(_) => last = "not_jellyfin",
        }
    }
    Err(last)
}

#[component]
pub fn ConnectFlow(
    #[props(default = "page")] variant: &'static str,
    on_done: EventHandler<()>,
) -> Element {
    let t = use_i18n();
    let app = use_app();
    let svc = use_services();
    let mut step = use_signal(|| Step::Address);
    let mut forward = use_signal(|| true);
    let mut go = move |next: Step| {
        let order = |s: Step| DOT_STEPS.iter().position(|d| *d == s).unwrap_or(1);
        forward.set(order(next) >= order(step()));
        step.set(next);
    };

    let address = use_signal(|| std::env::var("ORPHYN_E2E_URL").unwrap_or_default());
    let mut probing = use_signal(|| false);
    let mut probe_error = use_signal(|| None::<String>);
    let mut found = use_signal(|| None::<Found>);

    let username = use_signal(|| std::env::var("ORPHYN_E2E_USER").unwrap_or_default());
    let mut password = use_signal(|| std::env::var("ORPHYN_E2E_PASSWORD").unwrap_or_default());
    let mut show_password = use_signal(|| false);
    let mut logging_in = use_signal(|| false);
    let mut login_error = use_signal(|| None::<String>);

    let mut saved = use_signal(|| None::<Saved>);
    let mut name = use_signal(String::new);
    let mut finishing = use_signal(|| false);
    let mut finish_error = use_signal(|| None::<String>);

    let found_now = move || found().filter(|f| f.typed == address().trim());

    let submit_address = {
        let svc = svc.clone();
        move |_: ()| {
            if found_now().is_some() {
                return go(Step::Login);
            }
            let typed = address().trim().to_string();
            if typed.is_empty() || probing() {
                return;
            }
            probing.set(true);
            probe_error.set(None);
            found.set(None);
            let device = svc.device_id();
            spawn(async move {
                match probe(&typed, &device).await {
                    Ok((url, info)) => found.set(Some(Found { typed, url, info })),
                    Err(kind) => probe_error.set(Some(t.t(&format!("servers:errors.{kind}")))),
                }
                probing.set(false);
            });
        }
    };

    let submit_login = {
        let svc = svc.clone();
        move |_: ()| {
            let Some(f) = found() else { return };
            let (user, pass) = (username().trim().to_string(), password());
            if user.is_empty() || pass.is_empty() || logging_in() {
                return;
            }
            logging_in.set(true);
            login_error.set(None);
            let svc = svc.clone();
            spawn(async move {
                let device = svc.device_id();
                let res = auth::login(&f.url, &user, &pass, &device).await;
                // The password never outlives the login call.
                password.set(String::new());
                show_password.set(false);
                match res {
                    Ok(ok) => {
                        let added = svc.accounts.as_ref().map(|s| {
                            let mut s = s.lock().unwrap_or_else(|p| p.into_inner());
                            s.add(NewServerAccount {
                                name: f.info.server_name.clone(),
                                url: f.url.clone(),
                                server_id: if ok.server_id.is_empty() {
                                    f.info.id.clone()
                                } else {
                                    ok.server_id.clone()
                                },
                                server_name: f.info.server_name.clone(),
                                version: f.info.version.clone(),
                                user_id: ok.user_id.clone(),
                                user_name: ok.user_name.clone(),
                                access_token: ok.access_token.clone(),
                            })
                        });
                        let account_id = match added {
                            Some(Ok(a)) => a.id,
                            Some(Err(_)) => {
                                login_error.set(Some(t.t("servers:errors.generic")));
                                logging_in.set(false);
                                return;
                            }
                            None => "memory".to_string(),
                        };
                        let session = Session {
                            account_id: account_id.clone(),
                            user_id: ok.user_id,
                            client: Client::new(&f.url, &device, Some(&ok.access_token)),
                        };
                        let mut accounts = app.accounts;
                        accounts.set(svc.list_accounts());
                        name.set(f.info.server_name.clone());
                        saved.set(Some(Saved {
                            account_id,
                            name: f.info.server_name.clone(),
                            session,
                        }));
                        go(Step::Success);
                        tokio::time::sleep(Duration::from_millis(SUCCESS_MS)).await;
                        if step() == Step::Success {
                            go(Step::Name);
                        }
                    }
                    Err(e) => {
                        let key = match e.kind {
                            LoginErrorKind::InvalidCredentials => "invalid_credentials",
                            LoginErrorKind::Unreachable => "unreachable",
                            LoginErrorKind::ServerError => "server_error",
                        };
                        login_error.set(Some(t.t(&format!("servers:errors.{key}"))));
                    }
                }
                logging_in.set(false);
            });
        }
    };

    let submit_name = {
        let svc = svc.clone();
        move |_: ()| {
            let Some(s) = saved() else { return };
            if finishing() {
                return;
            }
            finishing.set(true);
            finish_error.set(None);
            let trimmed = name().trim().to_string();
            let result = svc.accounts.as_ref().map(|store| {
                let mut store = store.lock().unwrap_or_else(|p| p.into_inner());
                if !trimmed.is_empty() && trimmed != s.name {
                    store.rename(&s.account_id, &trimmed)?;
                }
                store.set_active(Some(&s.account_id))?;
                Ok::<_, orphyn_core::error::Error>(())
            });
            if matches!(result, Some(Err(_))) {
                finish_error.set(Some(t.t("servers:errors.generic")));
                finishing.set(false);
                return;
            }
            app.activate(&svc, Some(s.session));
            on_done.call(());
        }
    };

    let dot = if step() == Step::Success {
        Step::Login
    } else {
        step()
    };
    let dot_index = DOT_STEPS.iter().position(|d| *d == dot).unwrap_or(0);
    let enter = if forward() {
        "jr-step-enter"
    } else {
        "jr-step-enter-back"
    };
    let steps_label = t.tf(
        "servers:steps.label",
        &[
            ("current", (dot_index + 1).to_string()),
            ("total", DOT_STEPS.len().to_string()),
        ],
    );
    let pw_kind = if show_password() { "text" } else { "password" };
    let (eye, eye_label) = if show_password() {
        ("eye-off", t.t("servers:login.hidePassword"))
    } else {
        ("eye", t.t("servers:login.showPassword"))
    };
    let (submit_address, submit_login, submit_name) = (
        Callback::new(submit_address),
        Callback::new(submit_login),
        Callback::new(submit_name),
    );
    let connect_label = if probing() {
        t.t("servers:address.checking")
    } else if found_now().is_some() {
        t.t("servers:address.continue")
    } else {
        t.t("servers:address.connect")
    };

    rsx! {
        div { class: "jr-connect jr-connect--{variant}",
            div { class: "jr-connect__stage",
                match step() {
                    Step::Address => rsx! {
                        div { key: "address-{enter}", class: "jr-connect__step {enter}",
                            h1 { class: "jr-connect__title", {t.t("servers:address.title")} }
                            p { class: "jr-connect__subtitle", {t.t("servers:address.subtitle")} }
                            div { class: "jr-connect__fields",
                                super::TextField {
                                    label: t.t("servers:address.label"),
                                    placeholder: t.t("servers:address.placeholder"),
                                    hint: t.t("servers:address.hint"),
                                    error: probe_error(),
                                    value: address,
                                    on_input: move |_| probe_error.set(None),
                                    on_enter: move |_| submit_address.call(()),
                                }
                            }
                            if let Some(f) = found_now() {
                                div { class: "jr-connect__found", role: "status",
                                    Icon { name: "circle-check", size: 20, class: "jr-connect__found-icon" }
                                    span {
                                        span { class: "jr-connect__found-title", {t.t("servers:address.found")} }
                                        span { class: "jr-connect__found-detail",
                                            {t.tf("servers:address.foundDetail", &[("name", f.info.server_name.clone()), ("version", f.info.version.clone())])}
                                        }
                                    }
                                }
                            }
                            div { class: "jr-connect__actions",
                                Button {
                                    size: "large",
                                    full_width: true,
                                    disabled: probing() || address().trim().is_empty(),
                                    onclick: move |_| submit_address.call(()),
                                    "{connect_label}"
                                }
                            }
                        }
                    },
                    Step::Login => rsx! {
                        div { key: "login-{enter}", class: "jr-connect__step {enter}",
                            h1 { class: "jr-connect__title", {t.t("servers:login.title")} }
                            p { class: "jr-connect__subtitle",
                                {t.tf("servers:login.subtitle", &[("name", found().map(|f| f.info.server_name).unwrap_or_default())])}
                            }
                            div { class: "jr-connect__fields",
                                TextField {
                                    label: t.t("servers:login.username"),
                                    value: username,
                                    on_input: move |_| login_error.set(None),
                                    on_enter: move |_| submit_login.call(()),
                                }
                                TextField {
                                    label: t.t("servers:login.password"),
                                    kind: pw_kind,
                                    value: password,
                                    error: login_error(),
                                    on_input: move |_| login_error.set(None),
                                    on_enter: move |_| submit_login.call(()),
                                    trailing: rsx! {
                                        IconButton {
                                            size: "small",
                                            icon: eye,
                                            label: eye_label,
                                            pressed: show_password(),
                                            onclick: move |_| show_password.set(!show_password()),
                                        }
                                    },
                                }
                            }
                            div { class: "jr-connect__actions",
                                Button {
                                    size: "large",
                                    full_width: true,
                                    disabled: logging_in() || username().trim().is_empty() || password().is_empty(),
                                    onclick: move |_| submit_login.call(()),
                                    {t.t("servers:login.signIn")}
                                }
                                Button {
                                    variant: "plain",
                                    full_width: true,
                                    disabled: logging_in(),
                                    onclick: move |_| go(Step::Address),
                                    {t.t("servers:login.back")}
                                }
                            }
                        }
                    },
                    Step::Success => rsx! {
                        div { key: "success-{enter}", class: "jr-connect__step jr-connect__success {enter}", role: "status",
                            svg { class: "jr-connect__check jr-check-pop", view_box: "0 0 52 52", "aria-hidden": "true",
                                circle { cx: "26", cy: "26", r: "24", class: "jr-connect__check-bg" }
                                path { d: "M15 27 L23 35 L38 19", class: "jr-connect__check-mark" }
                            }
                            h1 { class: "jr-connect__title", {t.t("servers:success.title")} }
                            p { class: "jr-connect__subtitle", {t.t("servers:success.subtitle")} }
                        }
                    },
                    Step::Name => rsx! {
                        div { key: "name-{enter}", class: "jr-connect__step {enter}",
                            h1 { class: "jr-connect__title", {t.t("servers:name.title")} }
                            p { class: "jr-connect__subtitle", {t.t("servers:name.subtitle")} }
                            div { class: "jr-connect__fields",
                                TextField {
                                    label: t.t("servers:name.label"),
                                    hint: t.t("servers:name.hint"),
                                    error: finish_error(),
                                    value: name,
                                    on_enter: move |_| submit_name.call(()),
                                }
                            }
                            div { class: "jr-connect__actions",
                                Button {
                                    size: "large",
                                    full_width: true,
                                    disabled: finishing(),
                                    onclick: move |_| submit_name.call(()),
                                    {t.t("servers:name.finish")}
                                }
                            }
                        }
                    },
                }
            }
            div { class: "jr-connect__dots", role: "img", "aria-label": "{steps_label}",
                for (i, _) in DOT_STEPS.iter().enumerate() {
                    span {
                        key: "{i}",
                        class: if i == dot_index { "jr-connect__dot jr-connect__dot--active" } else { "jr-connect__dot" },
                    }
                }
            }
        }
    }
}
