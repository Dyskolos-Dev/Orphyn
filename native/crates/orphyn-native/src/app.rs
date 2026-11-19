//! The root component: opens the services, provides the app-wide contexts, follows the window
//! (OS theme, focus, bounds, back / forward buttons) and renders `.jr-root` with the theme and
//! interface-style tokens, the stylesheets and the shell.
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use dioxus_native::winit::event::{ElementState, WindowEvent};
use dioxus_native::winit::keyboard::{Key as WKey, ModifiersState, NamedKey};
use dioxus_native::winit::window::Theme;

use crate::design::Overlay;
use crate::i18n::{I18n, LanguageChoice, LANGUAGE_STORAGE_KEY};
use crate::preferences::PlaybackPreferences;
use crate::router::{Route, Router};
use crate::shell::AppShell;
use crate::state::{AppState, Services};
use crate::theme::{root_tokens, InterfaceState, ThemeState, WindowFacts};
use crate::window::{self, CaptionButtons};

pub fn app() -> Element {
    let svc = use_context_provider(Services::open);
    let win = dioxus_native::use_window();
    let storage = svc.storage.clone();

    let language =
        use_signal(|| LanguageChoice::parse(storage.get(LANGUAGE_STORAGE_KEY).as_deref()));
    let i18n = use_context_provider(|| I18n {
        lang: Signal::new(language.peek().resolve()),
    });
    let app = use_context_provider(|| AppState {
        theme: Signal::new(ThemeState::load(&storage)),
        interface: Signal::new(InterfaceState::load(&storage)),
        os_dark: Signal::new(win.theme() == Some(Theme::Dark)),
        window: Signal::new(WindowFacts {
            platform: window::platform(),
            material: "none",
            fullscreen: false,
            focused: true,
        }),
        language,
        preferences: Signal::new(PlaybackPreferences::load(&storage)),
        accounts: Signal::new(svc.list_accounts()),
        session: Signal::new(None),
    });
    let router = use_context_provider(|| {
        Router::new(Route::parse(
            &std::env::var("ORPHYN_ROUTE").unwrap_or_else(|_| "/".into()),
        ))
    });
    use_context_provider(|| Overlay {
        menu: Signal::new(None),
    });
    crate::features::music::use_music_provider(&svc);
    crate::features::system::use_system_provider(&svc);
    crate::design::toast::use_toast_provider();
    crate::features::cast::use_cast_provider();
    crate::features::pip::use_pip_provider();
    crate::features::syncplay::use_syncplay_provider();

    // The saved session, or (development) the test server of ORPHYN_E2E_* when there is none.
    use_hook({
        let svc = svc.clone();
        move || {
            let session = svc.active_session();
            let has = session.is_some();
            app.activate(&svc, session);
            if let (false, Ok(url), Ok(user), Ok(pw)) = (
                has,
                std::env::var("ORPHYN_E2E_URL"),
                std::env::var("ORPHYN_E2E_USER"),
                std::env::var("ORPHYN_E2E_PASSWORD"),
            ) {
                let svc = svc.clone();
                spawn(async move {
                    if let Err(e) = app.sign_in(svc, url, user, pw).await {
                        log::warn!("[app] test server sign-in failed: {}", e.message);
                    }
                });
            }
        }
    });

    // Window events: OS theme, focus, saved bounds, mouse back / forward, Alt+arrows.
    let last_save = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(Instant::now())));
    let mut modifiers = use_signal(ModifiersState::default);
    let ev_win = win.clone();
    dioxus_native::use_window_event(move |event, _| {
        let (mut os_dark, mut facts) = (app.os_dark, app.window);
        match event {
            WindowEvent::ThemeChanged(t) => os_dark.set(*t == Theme::Dark),
            WindowEvent::Focused(f) => facts.with_mut(|w| w.focused = *f),
            WindowEvent::SurfaceResized(_) | WindowEvent::Moved(_) => {
                let fs = ev_win.fullscreen().is_some();
                if facts.peek().fullscreen != fs {
                    facts.with_mut(|w| w.fullscreen = fs);
                }
                if last_save.get().elapsed() > Duration::from_millis(400) {
                    last_save.set(Instant::now());
                    if let Some(b) = window::bounds_of(ev_win.as_ref()) {
                        window::save_bounds(&b);
                    }
                }
            }
            WindowEvent::CloseRequested => {
                if let Some(b) = window::bounds_of(ev_win.as_ref()) {
                    window::save_bounds(&b);
                }
            }
            WindowEvent::ModifiersChanged(m) => modifiers.set(m.state()),
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                use dioxus_native::winit::event::MouseButton;
                match button.clone().mouse_button() {
                    Some(MouseButton::Back) => router.back(),
                    Some(MouseButton::Forward) => router.forward(),
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && modifiers.peek().alt_key() =>
            {
                match event.logical_key {
                    WKey::Named(NamedKey::ArrowLeft) => router.back(),
                    WKey::Named(NamedKey::ArrowRight) => router.forward(),
                    _ => {}
                }
            }
            _ => {}
        }
    });

    let theme = app.theme.read().clone();
    let resolved = theme.resolve((app.os_dark)());
    let iface = app.interface.read().clone();
    let facts = (app.window)();
    let tokens = root_tokens(&theme, resolved, &iface, facts);
    let attrs: Vec<Attribute> = tokens
        .attributes
        .iter()
        .map(|(k, v)| Attribute::new(k, v.clone(), None, false))
        .collect();
    let _ = i18n;
    let maximized = win.is_maximized();
    rsx! {
        style { {crate::assets::APP_CSS} }
        style { {crate::assets::OVERRIDES_CSS} }
        div { class: "jr-root", style: "{tokens.style}", ..attrs,
            div { class: "jr-body",
                div { class: "jr-app", AppShell {} }
                crate::features::whatsnew::WhatsNewHost {}
                crate::features::updater::UpdateNotice {}
                crate::features::palette::PaletteHost {}
                crate::features::pip::PipHost {}
                crate::design::toast::ToastHost {}
                crate::features::syncplay::SyncPlayHost {}
                CaptionButtons {
                    fullscreen: facts.fullscreen,
                    focused: facts.focused,
                    maximized,
                    dark: resolved == crate::theme::Resolved::Dark,
                }
            }
        }
    }
}
