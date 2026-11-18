//! App-wide state, provided once at the root.
//!
//! - [`Services`] (non-reactive, cheap to clone): local storage, the account store, the query
//!   cache, the music player and the video controller.
//! - [`AppState`] (`Copy`, reactive signals): theme, interface style, language, preferences,
//!   window facts, accounts and the signed-in session.
//!
//! Components read them with `use_services()` / `use_app()`.
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use orphyn_core::accounts::{AccountStore, NewServerAccount, ServerAccount};
use orphyn_core::storage::LocalStorage;
use orphyn_jellyfin::auth::{self, LoginError, LoginErrorKind};
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{Client, QueryCache};
use orphyn_player::music::MusicPlayer;
use orphyn_player::video::VideoController;

use crate::i18n::LanguageChoice;
use crate::preferences::{AudioQuality, PlaybackPreferences};
use crate::theme::{InterfaceState, ThemeState, WindowFacts};

#[derive(Clone)]
pub struct Services {
    pub storage: LocalStorage,
    /// `None` when the data directory is unusable (the app still runs, nothing is saved).
    pub accounts: Option<Arc<Mutex<AccountStore>>>,
    pub cache: QueryCache,
    pub music: MusicPlayer,
    pub video: VideoController,
}

impl Services {
    pub fn open() -> Self {
        let accounts = match orphyn_core::accounts::open_default() {
            Ok(s) => Some(Arc::new(Mutex::new(s))),
            Err(e) => {
                log::warn!("[accounts] {e}");
                None
            }
        };
        Self {
            storage: LocalStorage::open_default(),
            accounts,
            cache: QueryCache::new(),
            music: MusicPlayer::new(),
            video: VideoController::new(),
        }
    }

    fn with_accounts<T>(&self, f: impl FnOnce(&mut AccountStore) -> T) -> Option<T> {
        let store = self.accounts.as_ref()?;
        let mut guard = store.lock().unwrap_or_else(|p| p.into_inner());
        Some(f(&mut guard))
    }

    pub fn list_accounts(&self) -> Vec<ServerAccount> {
        self.with_accounts(|s| s.list()).unwrap_or_default()
    }

    pub fn device_id(&self) -> String {
        self.with_accounts(|s| s.device_id().to_string())
            .unwrap_or_else(|| "orphyn-native".into())
    }

    /// The session of the active account, when it has a token.
    pub fn active_session(&self) -> Option<Session> {
        let device = self.device_id();
        self.with_accounts(|s| {
            let a = s.active()?;
            let token = s.token(&a.id)?;
            Some(Session {
                account_id: a.id.clone(),
                user_id: a.user_id.clone(),
                client: Client::new(&a.url, &device, Some(&token)),
            })
        })
        .flatten()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct AppState {
    pub theme: Signal<ThemeState>,
    pub interface: Signal<InterfaceState>,
    /// The OS appearance (followed while the theme mode is `system`).
    pub os_dark: Signal<bool>,
    pub window: Signal<WindowFacts>,
    pub language: Signal<LanguageChoice>,
    pub preferences: Signal<PlaybackPreferences>,
    pub accounts: Signal<Vec<ServerAccount>>,
    pub session: Signal<Option<Session>>,
}

pub fn use_services() -> Services {
    use_context::<Services>()
}

pub fn use_app() -> AppState {
    use_context::<AppState>()
}

/// The signed-in session; screens that need one are only mounted when it exists.
pub fn use_session() -> Option<Session> {
    (use_app().session)()
}

impl AppState {
    pub fn set_theme(mut self, svc: &Services, f: impl FnOnce(&mut ThemeState)) {
        let mut t = self.theme.peek().clone();
        f(&mut t);
        svc.storage.set(crate::theme::keys::MODE, t.mode.as_str());
        svc.storage.set(crate::theme::keys::ACCENT, &t.accent);
        svc.storage.set(
            crate::theme::keys::FOLLOW_ARTWORK,
            if t.follow_artwork { "true" } else { "false" },
        );
        self.theme.set(t);
    }

    pub fn set_style(mut self, svc: &Services, style: &'static str) {
        let mut s = self.interface.peek().clone();
        s.style = style;
        s.save(&svc.storage);
        self.interface.set(s);
    }

    pub fn set_language(
        mut self,
        svc: &Services,
        choice: LanguageChoice,
        mut lang: Signal<crate::i18n::Lang>,
    ) {
        svc.storage
            .set(crate::i18n::LANGUAGE_STORAGE_KEY, choice.as_str());
        self.language.set(choice);
        lang.set(choice.resolve());
    }

    pub fn set_preferences(mut self, svc: &Services, f: impl FnOnce(&mut PlaybackPreferences)) {
        let mut p = self.preferences.peek().clone();
        f(&mut p);
        p.save(&svc.storage);
        self.preferences.set(p);
    }

    /// Switches the active session: music URLs follow it, the old server's cache is dropped.
    pub fn activate(mut self, svc: &Services, session: Option<Session>) {
        if let Some(old) = self.session.peek().as_ref() {
            svc.cache
                .remove(&orphyn_jellyfin::keys::all(&old.account_id));
        }
        if let Some(s) = &session {
            let s = s.clone();
            let quality = self.preferences.peek().audio_quality;
            svc.music.set_resolver(Arc::new(move |t| {
                Some(match quality {
                    AudioQuality::Original => s.audio_url(&t.id),
                    AudioQuality::Bitrate(b) => s.audio_transcode_url(&t.id, b as u32),
                })
            }));
        }
        self.accounts.set(svc.list_accounts());
        self.session.set(session);
    }

    /// Signs in to `url` and makes that account active (the onboarding / add-server flow).
    pub async fn sign_in(
        self,
        svc: Services,
        url: String,
        user: String,
        password: String,
    ) -> Result<(), LoginError> {
        let url = url.trim().trim_end_matches('/').to_string();
        let device = svc.device_id();
        let info = auth::public_info(&url, &device)
            .await
            .map_err(|e| LoginError {
                kind: if matches!(e, orphyn_jellyfin::JfError::Network(_)) {
                    LoginErrorKind::Unreachable
                } else {
                    LoginErrorKind::ServerError
                },
                message: e.to_string(),
            })?;
        let ok = auth::login(&url, &user, &password, &device).await?;
        let added = svc.with_accounts(|s| {
            let a = s.add(NewServerAccount {
                name: info.server_name.clone(),
                url: url.clone(),
                server_id: if ok.server_id.is_empty() {
                    info.id.clone()
                } else {
                    ok.server_id.clone()
                },
                server_name: info.server_name.clone(),
                version: info.version.clone(),
                user_id: ok.user_id.clone(),
                user_name: ok.user_name.clone(),
                access_token: ok.access_token.clone(),
            })?;
            s.set_active(Some(&a.id))?;
            Ok::<_, orphyn_core::error::Error>(a)
        });
        let account_id = match added {
            Some(Ok(a)) => a.id,
            Some(Err(e)) => {
                return Err(LoginError {
                    kind: LoginErrorKind::ServerError,
                    message: e.to_string(),
                })
            }
            None => "memory".to_string(),
        };
        self.activate(
            &svc,
            Some(Session {
                account_id,
                user_id: ok.user_id,
                client: Client::new(&url, &device, Some(&ok.access_token)),
            }),
        );
        Ok(())
    }
}
