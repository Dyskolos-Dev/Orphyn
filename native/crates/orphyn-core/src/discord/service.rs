//! Rich Presence state machine (port of `src/main/discord/service.ts` + `scheduler.ts`).
//!
//! It is synchronous and driven by explicit timestamps, so it is unit-tested without Discord:
//! the worker thread in `mod.rs` feeds it messages and calls `tick` at `next_deadline`.
//! It connects lazily (first presence, or when the state is read), retries every 30 s while
//! Discord is not running, rate-limits updates (last write wins) and never fails loudly.

use serde::{Deserialize, Serialize};

use super::activity::{build_activity, Activity, Presence};

/// Discord allows 5 updates / 20 s: stay well under it.
pub const MIN_UPDATE_INTERVAL_MS: i64 = 2000;
pub const RECONNECT_INTERVAL_MS: i64 = 30_000;

/// Placeholder application id: while it is set, Rich Presence stays inactive.
pub const CLIENT_ID_PLACEHOLDER: &str = "000000000000000000";
/// Public application id of the Discord application named Orphyn (see `docs/DISCORD.md`).
pub const CLIENT_ID: &str = "1570792208716537866";

pub fn is_configured(id: &str) -> bool {
    (17..=20).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_digit())
        && id != CLIENT_ID_PLACEHOLDER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub show_artwork: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_artwork: true,
        }
    }
}

/// `Partial<DiscordSettings>` of the contract.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub enabled: Option<bool>,
    pub show_artwork: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    NotConfigured,
    Disabled,
    Idle,
    Connected,
    Unavailable,
}

/// `DiscordState` of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub enabled: bool,
    pub show_artwork: bool,
    pub status: Status,
}

/// The slice of the RPC client the service needs.
pub trait Rpc {
    fn connect(&mut self) -> Result<(), String>;
    fn set_activity(&mut self, activity: &Activity) -> Result<(), String>;
    fn clear_activity(&mut self) -> Result<(), String>;
    fn close(&mut self);
}

pub struct Core<C: Rpc> {
    configured: bool,
    settings: Settings,
    make_client: Box<dyn Fn() -> C + Send>,
    client: Option<C>,
    attempted: bool,
    presence: Option<Presence>,
    /// `Some(None)` = a clear is waiting, `Some(Some(a))` = an update is waiting.
    pending: Option<Option<Activity>>,
    last_send: Option<i64>,
    next_retry: Option<i64>,
    /// A connection attempt is wanted at the next tick (lazy connect).
    want_connect: bool,
}

impl<C: Rpc> Core<C> {
    pub fn new(
        configured: bool,
        settings: Settings,
        make_client: Box<dyn Fn() -> C + Send>,
    ) -> Self {
        Self {
            configured,
            settings,
            make_client,
            client: None,
            attempted: false,
            presence: None,
            pending: None,
            last_send: None,
            next_retry: None,
            want_connect: false,
        }
    }

    fn active(&self) -> bool {
        self.configured && self.settings.enabled
    }

    pub fn status(&self) -> Status {
        if !self.configured {
            Status::NotConfigured
        } else if !self.settings.enabled {
            Status::Disabled
        } else if self.client.is_some() {
            Status::Connected
        } else if self.attempted {
            Status::Unavailable
        } else {
            Status::Idle
        }
    }

    pub fn state(&self) -> State {
        State {
            enabled: self.settings.enabled,
            show_artwork: self.settings.show_artwork,
            status: self.status(),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Reading the state is a reason to find out whether Discord is there.
    pub fn poke(&mut self) {
        if self.active() && !self.attempted {
            self.want_connect = true;
        }
    }

    pub fn set_presence(&mut self, presence: Option<Presence>, now: i64) {
        self.presence = presence;
        if !self.active() {
            return;
        }
        match build_activity(self.presence.as_ref(), self.settings.show_artwork, now) {
            // Clearing is not rate limited: do it now so it feels instant.
            None => {
                self.pending = None;
                self.apply(None, now);
            }
            Some(a) => {
                self.pending = Some(Some(a));
                self.want_connect = true;
            }
        }
    }

    pub fn set_settings(&mut self, patch: SettingsPatch, now: i64) {
        let was_enabled = self.settings.enabled;
        self.settings = Settings {
            enabled: patch.enabled.unwrap_or(self.settings.enabled),
            show_artwork: patch.show_artwork.unwrap_or(self.settings.show_artwork),
        };
        if !self.configured {
            return;
        }
        if !self.settings.enabled {
            self.pending = None;
            self.apply(None, now);
            self.next_retry = None;
        } else if !was_enabled || patch.show_artwork.is_some() {
            // Rebuild the activity with the new settings, immediately.
            self.pending = None;
            if let Some(a) = build_activity(self.presence.as_ref(), self.settings.show_artwork, now)
            {
                if self.client.is_none() {
                    self.try_connect(now);
                }
                self.apply(Some(a), now);
            }
        }
    }

    /// Clears the activity and closes the connection (app exit).
    pub fn dispose(&mut self) {
        self.pending = None;
        if let Some(mut c) = self.client.take() {
            let _ = c.clear_activity();
            c.close();
        }
    }

    /// When `tick` next has something to do.
    pub fn next_deadline(&self, now: i64) -> Option<i64> {
        let mut best: Option<i64> = None;
        let mut consider = |t: i64| best = Some(best.map_or(t, |b| b.min(t)));
        if self.want_connect {
            consider(now);
        }
        if self.pending.is_some() && self.client.is_some() {
            consider(self.last_send.map_or(now, |l| l + MIN_UPDATE_INTERVAL_MS));
        }
        if let Some(t) = self.next_retry {
            consider(t);
        }
        best
    }

    pub fn tick(&mut self, now: i64) {
        if !self.active() {
            self.want_connect = false;
            self.next_retry = None;
            return;
        }
        let retry_due = self.next_retry.is_some_and(|t| t <= now);
        if (self.want_connect || retry_due) && self.client.is_none() {
            self.try_connect(now);
        }
        self.want_connect = false;
        if self.client.is_some() {
            let due = self
                .last_send
                .is_none_or(|l| now - l >= MIN_UPDATE_INTERVAL_MS);
            if due {
                if let Some(p) = self.pending.take() {
                    self.apply(p, now);
                }
            }
        }
    }

    fn try_connect(&mut self, now: i64) {
        self.attempted = true;
        let mut client = (self.make_client)();
        match client.connect() {
            Ok(()) => {
                self.client = Some(client);
                self.next_retry = None;
                // Show whatever is playing now.
                if let Some(a) =
                    build_activity(self.presence.as_ref(), self.settings.show_artwork, now)
                {
                    self.pending = None;
                    self.apply(Some(a), now);
                }
            }
            Err(_) => {
                // Discord is not running (or refused): stay quiet and try again later.
                client.close();
                self.next_retry = Some(now + RECONNECT_INTERVAL_MS);
            }
        }
    }

    fn apply(&mut self, activity: Option<Activity>, now: i64) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        let result = match &activity {
            Some(a) => client.set_activity(a),
            None => client.clear_activity(),
        };
        self.last_send = Some(now);
        if result.is_err() {
            if let Some(mut c) = self.client.take() {
                c.close();
            }
            if self.active() {
                self.next_retry = Some(now + RECONNECT_INTERVAL_MS);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log {
        connects: u32,
        sent: Vec<Option<Activity>>,
        discord_up: bool,
    }

    struct Fake(Arc<Mutex<Log>>);

    impl Rpc for Fake {
        fn connect(&mut self) -> Result<(), String> {
            let mut l = self.0.lock().unwrap();
            l.connects += 1;
            if l.discord_up {
                Ok(())
            } else {
                Err("down".into())
            }
        }
        fn set_activity(&mut self, a: &Activity) -> Result<(), String> {
            self.0.lock().unwrap().sent.push(Some(a.clone()));
            Ok(())
        }
        fn clear_activity(&mut self) -> Result<(), String> {
            self.0.lock().unwrap().sent.push(None);
            Ok(())
        }
        fn close(&mut self) {}
    }

    fn core(configured: bool, up: bool) -> (Core<Fake>, Arc<Mutex<Log>>) {
        let log = Arc::new(Mutex::new(Log {
            discord_up: up,
            ..Log::default()
        }));
        let l = log.clone();
        let c = Core::new(
            configured,
            Settings::default(),
            Box::new(move || Fake(l.clone())),
        );
        (c, log)
    }

    fn song(title: &str) -> Option<Presence> {
        Some(Presence::Music {
            paused: false,
            title: title.into(),
            position_sec: 0.0,
            duration_sec: 100.0,
            artwork_url: None,
            artist: None,
            album: None,
        })
    }

    #[test]
    fn not_configured_never_connects() {
        let (mut c, log) = core(false, true);
        c.poke();
        c.set_presence(song("A"), 0);
        c.tick(0);
        assert_eq!(c.status(), Status::NotConfigured);
        assert_eq!(log.lock().unwrap().connects, 0);
    }

    #[test]
    fn connects_lazily_on_the_first_presence_and_sends_it() {
        let (mut c, log) = core(true, true);
        assert_eq!(c.status(), Status::Idle);
        assert_eq!(log.lock().unwrap().connects, 0);
        c.set_presence(song("A"), 1000);
        c.tick(1000);
        assert_eq!(c.status(), Status::Connected);
        assert_eq!(log.lock().unwrap().sent.len(), 1);
    }

    #[test]
    fn updates_are_rate_limited_and_the_last_write_wins() {
        let (mut c, log) = core(true, true);
        c.set_presence(song("A"), 0);
        c.tick(0);
        c.set_presence(song("B"), 100);
        c.set_presence(song("C"), 200);
        c.tick(300);
        assert_eq!(log.lock().unwrap().sent.len(), 1, "too early");
        assert_eq!(c.next_deadline(300), Some(MIN_UPDATE_INTERVAL_MS));
        c.tick(MIN_UPDATE_INTERVAL_MS);
        let l = log.lock().unwrap();
        assert_eq!(l.sent.len(), 2);
        assert_eq!(l.sent[1].as_ref().unwrap().details, "C");
    }

    #[test]
    fn clearing_is_immediate() {
        let (mut c, log) = core(true, true);
        c.set_presence(song("A"), 0);
        c.tick(0);
        c.set_presence(None, 10);
        assert_eq!(log.lock().unwrap().sent.last(), Some(&None));
    }

    #[test]
    fn retries_quietly_every_30_seconds_while_discord_is_absent() {
        let (mut c, log) = core(true, false);
        c.set_presence(song("A"), 0);
        c.tick(0);
        assert_eq!(c.status(), Status::Unavailable);
        assert_eq!(c.next_deadline(1), Some(RECONNECT_INTERVAL_MS));
        c.tick(RECONNECT_INTERVAL_MS - 1);
        assert_eq!(log.lock().unwrap().connects, 1);
        log.lock().unwrap().discord_up = true;
        c.tick(RECONNECT_INTERVAL_MS);
        assert_eq!(c.status(), Status::Connected);
        assert_eq!(
            log.lock().unwrap().sent.len(),
            1,
            "the current presence is shown on connect"
        );
    }

    #[test]
    fn disabling_clears_and_stops_retrying() {
        let (mut c, log) = core(true, true);
        c.set_presence(song("A"), 0);
        c.tick(0);
        c.set_settings(
            SettingsPatch {
                enabled: Some(false),
                show_artwork: None,
            },
            5,
        );
        assert_eq!(c.status(), Status::Disabled);
        assert_eq!(log.lock().unwrap().sent.last(), Some(&None));
        c.set_presence(song("B"), 6);
        c.tick(10_000);
        assert_eq!(
            log.lock().unwrap().sent.len(),
            2,
            "nothing sent while disabled"
        );
    }

    #[test]
    fn toggling_artwork_rebuilds_the_activity_at_once() {
        let (mut c, log) = core(true, true);
        c.set_presence(song("A"), 0);
        c.tick(0);
        c.set_settings(
            SettingsPatch {
                enabled: None,
                show_artwork: Some(false),
            },
            50,
        );
        assert_eq!(log.lock().unwrap().sent.len(), 2);
        assert!(!c.settings().show_artwork);
    }

    #[test]
    fn configured_ids() {
        assert!(is_configured(CLIENT_ID));
        assert!(!is_configured(CLIENT_ID_PLACEHOLDER));
        assert!(!is_configured(""));
        assert!(!is_configured("abc"));
    }
}
