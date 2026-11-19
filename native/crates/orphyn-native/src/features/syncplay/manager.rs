//! `features/syncplay/manager.ts`: SyncPlay group membership and playback following.
//!
//! - Connects the server socket while in a group, keeps the server clock estimate fresh.
//! - Applies the group's queue (opens the video player or loads the music queue) and its commands
//!   at their `When` time; corrects drift while playing; reports Buffering / Ready so the group
//!   waits for its slowest member.
//! - Turns local play / pause / seek / next / new queue into group requests (see `bridge.rs`),
//!   and survives a network loss by rejoining.
//!
//! UI state is published through a `watch` channel; one-off events (toasts, navigation) go
//! through an unbounded channel of [`UiEvent`].
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;

use orphyn_jellyfin::syncplay::{
    self as sp, ClockOffsetEstimator, CommandKind, GroupState, JellyfinSocket, Member, PlayQueue,
    QueueEntry, SocketStatus, SyncPlayCommand, SyncPlayErrorKind, SyncPlayEvent, SyncPlayGroup,
};
use orphyn_jellyfin::endpoints::Session;
use orphyn_jellyfin::{BaseItemDto, Client, TICKS_PER_SECOND};

mod events;
pub use events::{item_title, playable_of};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Off,
    Connecting,
    Joined,
    Reconnecting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncPlayState {
    pub phase: Phase,
    pub group: Option<SyncPlayGroup>,
    pub group_state: GroupState,
    pub self_name: String,
    pub self_ready: bool,
    pub members: Vec<Member>,
    pub queue: Option<PlayQueue>,
    /// The group's current item, as this user sees it (`None`: none, or not available to us).
    pub current_item: Option<BaseItemDto>,
    pub current_kind: Option<MediaKind>,
    pub sheet_open: bool,
    pub sheet_kind: MediaKind,
}

impl Default for SyncPlayState {
    fn default() -> Self {
        Self {
            phase: Phase::Off,
            group: None,
            group_state: GroupState::Idle,
            self_name: String::new(),
            self_ready: false,
            members: Vec::new(),
            queue: None,
            current_item: None,
            current_kind: None,
            sheet_open: false,
            sheet_kind: MediaKind::Video,
        }
    }
}

/// One-off events for the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    UserJoined(String),
    UserLeft(String),
    Paused(Option<String>),
    Resumed(Option<String>),
    Seeked(Option<String>),
    NowPlaying { title: String, by: Option<String> },
    Waiting(Option<String>),
    Joined(String),
    Left,
    GroupClosed,
    ConnectionLost,
    Reconnected,
    ItemUnavailable,
    LibraryDenied,
    JoinDenied,
    CreateDenied,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    Notice(Notice),
    /// Open this route (the video player on the group's item, or `/music`).
    Navigate(String),
}

/// What SyncPlay needs from a player (the video and the music players each provide one).
pub trait SyncPlayer: Send + Sync {
    /// Jellyfin id of the item loaded (normalised, no dashes), or `None`.
    fn item_id(&self) -> Option<String>;
    /// Current position (s).
    fn position(&self) -> f64;
    /// Audio / picture is moving.
    fn is_playing(&self) -> bool;
    /// Enough data to start or keep playing at the current position.
    fn is_ready(&self) -> bool;
    fn play(&self);
    fn pause(&self);
    fn seek(&self, seconds: f64);
    /// Speed control for gentle drift correction; players without it only seek.
    fn set_rate(&self, _rate: f64) -> bool {
        false
    }
}

/// Local intents of the music player, forwarded to the group.
pub trait MusicIntents {
    fn play(&self);
    fn pause(&self);
    fn seek(&self, seconds: f64);
    fn next(&self);
    fn previous(&self);
    fn jump(&self, item_id: &str);
    fn new_queue(&self, item_ids: Vec<String>, index: usize, start_seconds: f64);
    fn queue(&self, item_ids: Vec<String>, next: bool);
}

/// What the manager needs from the music player.
pub trait MusicBridge: Send + Sync {
    fn player(&self) -> Arc<dyn SyncPlayer>;
    /// Item ids of the local queue, the current index and whether it plays.
    fn local_queue(&self) -> (Vec<String>, i64, bool);
    /// Replaces the local queue (paused).
    fn load(&self, tracks: Vec<BaseItemDto>, index: usize);
    /// Moves to another entry of the same queue (paused).
    fn jump(&self, index: usize);
    /// The group's queue grew around the current entry: add the new tracks locally if they fit.
    fn extend(&self, tracks: Vec<BaseItemDto>);
}

/// A command reported by `Ready` is played this long after (ms) without any drift fighting.
const GRACE_MS: f64 = 1500.0;
const BUFFERING_DEBOUNCE_MS: f64 = 500.0;
const MINE_WINDOW_MS: f64 = 5000.0;
const WAITING_NOTICE_GAP_MS: f64 = 6000.0;
/// Changes of a player this soon after the manager touched it are not the person's.
const QUIET_MS: f64 = 800.0;

/// Queue updates that change what plays (the others only edit the list around it).
const QUEUE_RESETS: [&str; 4] = ["NewPlaylist", "SetCurrentItem", "NextItem", "PreviousItem"];

pub fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn runtime() -> Handle {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("orphyn-syncplay")
            .enable_all()
            .build()
            .expect("syncplay runtime")
    })
    .handle()
    .clone()
}

/// Runs `future` on the SyncPlay runtime.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

fn playing_entry(q: &Option<PlayQueue>) -> Option<QueueEntry> {
    let q = q.as_ref()?;
    usize::try_from(q.playing_item_index)
        .ok()
        .and_then(|i| q.playlist.get(i))
        .cloned()
}

fn normalize(id: &str) -> String {
    id.replace('-', "").to_lowercase()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    Pause,
    Unpause,
    Seek,
    Queue,
}

struct Conn {
    client: Client,
    user_id: String,
    socket: JellyfinSocket,
    time: Arc<Mutex<ClockOffsetEstimator>>,
    tasks: Vec<JoinHandle<()>>,
}

#[derive(Default)]
struct Inner {
    ui: SyncPlayState,
    conn: Option<Conn>,
    video: Option<Arc<dyn SyncPlayer>>,
    music: Option<Arc<dyn MusicBridge>>,
    seen: Vec<String>,
    command_task: Option<JoinHandle<()>>,
    buffer_task: Option<JoinHandle<()>>,
    /// Playback may run (an Unpause applied); otherwise the player is held paused.
    allow_play: bool,
    last_unpause: Option<(f64, f64)>,
    /// A Ready is owed to the server (after a new queue, a seek, joining).
    awaiting_ready: bool,
    reported_ready: Option<bool>,
    grace_until: f64,
    rate: f64,
    rate_kind: Option<MediaKind>,
    last_local: Option<(LocalKind, f64)>,
    items: HashMap<String, BaseItemDto>,
    queue_key: String,
    pending_command: Option<SyncPlayCommand>,
    share_on_join: bool,
    leaving: bool,
    rejoining: bool,
    ignoring_wait: bool,
    last_waiting_notice: f64,
    others_seen: usize,
    quiet_until: f64,
}

pub struct SyncPlayManager {
    inner: Mutex<Inner>,
    state_tx: watch::Sender<Arc<SyncPlayState>>,
    ui_tx: mpsc::UnboundedSender<UiEvent>,
    ui_rx: Mutex<Option<mpsc::UnboundedReceiver<UiEvent>>>,
    me: Weak<SyncPlayManager>,
}

static MANAGER: OnceLock<Arc<SyncPlayManager>> = OnceLock::new();

/// The one SyncPlay manager of the app.
pub fn manager() -> Arc<SyncPlayManager> {
    MANAGER
        .get_or_init(|| {
            let (ui_tx, ui_rx) = mpsc::unbounded_channel();
            let (state_tx, _) = watch::channel(Arc::new(SyncPlayState::default()));
            Arc::new_cyclic(|me| SyncPlayManager {
                inner: Mutex::new(Inner {
                    rate: 1.0,
                    ..Inner::default()
                }),
                state_tx,
                ui_tx,
                ui_rx: Mutex::new(Some(ui_rx)),
                me: me.clone(),
            })
        })
        .clone()
}

impl SyncPlayManager {
    fn arc(&self) -> Arc<SyncPlayManager> {
        self.me.upgrade().expect("manager alive")
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<SyncPlayState>> {
        self.state_tx.subscribe()
    }

    pub fn state(&self) -> Arc<SyncPlayState> {
        self.state_tx.borrow().clone()
    }

    /// The UI side of the event channel; only the first caller gets it.
    pub fn take_events(&self) -> Option<mpsc::UnboundedReceiver<UiEvent>> {
        self.ui_rx.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    fn notify(&self, n: Notice) {
        let _ = self.ui_tx.send(UiEvent::Notice(n));
    }

    fn update<R>(&self, f: impl FnOnce(&mut SyncPlayState) -> R) -> R {
        let mut g = self.lock();
        let r = f(&mut g.ui);
        self.state_tx.send_replace(Arc::new(g.ui.clone()));
        r
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state().phase, Phase::Joined | Phase::Reconnecting)
    }

    pub fn set_music_bridge(&self, bridge: Arc<dyn MusicBridge>) {
        self.lock().music = Some(bridge);
    }

    pub fn open_sheet(&self, kind: MediaKind) {
        self.update(|s| {
            s.sheet_open = true;
            s.sheet_kind = kind;
        });
    }

    pub fn close_sheet(&self) {
        self.update(|s| s.sheet_open = false);
    }

    // ----------------------------------------------------------------------- groups

    pub async fn list_groups(&self, session: &Session) -> Result<Vec<SyncPlayGroup>, String> {
        sp::list_groups(&session.client).await.map_err(|e| e.to_string())
    }

    pub fn create(&self, session: Session, user_name: String, name: String) {
        let me = self.arc();
        runtime().spawn(async move {
            match me.ensure_connected(&session, &user_name).await {
                Ok(client) => {
                    me.lock().share_on_join = true;
                    if sp::create_group(&client, &name).await.is_err() {
                        me.fail_to_connect();
                    }
                }
                Err(_) => me.fail_to_connect(),
            }
        });
    }

    pub fn join(&self, session: Session, user_name: String, group_id: String) {
        let me = self.arc();
        runtime().spawn(async move {
            match me.ensure_connected(&session, &user_name).await {
                Ok(client) => {
                    me.lock().share_on_join = false;
                    if sp::join_group(&client, &group_id).await.is_err() {
                        me.fail_to_connect();
                    }
                }
                Err(_) => me.fail_to_connect(),
            }
        });
    }

    /// Leaves the group; local playback goes on, on its own.
    pub fn leave(&self) {
        let me = self.arc();
        let client = {
            let mut g = self.lock();
            g.leaving = true;
            g.conn.as_ref().map(|c| c.client.clone())
        };
        runtime().spawn(async move {
            if let Some(client) = client {
                // Not in a group any more (403) or offline: leaving locally is enough.
                let _ = sp::leave_group(&client).await;
            }
            me.teardown();
            me.notify(Notice::Left);
        });
    }

    async fn ensure_connected(&self, session: &Session, user_name: &str) -> Result<Client, String> {
        let existing = self.lock().conn.as_ref().map(|c| c.client.clone());
        if let Some(client) = existing {
            self.when_open().await?;
            return Ok(client);
        }
        let client = session.client.clone();
        let token = client.token().ok_or("No session")?.to_string();
        let url = sp::socket_url(client.base_url(), &token, client.device_id());
        self.update(|s| {
            s.phase = Phase::Connecting;
            s.self_name = user_name.to_string();
        });
        let me = self.arc();
        let me2 = self.arc();
        let socket = JellyfinSocket::connect(
            &runtime(),
            url,
            move |m| {
                if let Some(e) = sp::parse_syncplay_message(&m) {
                    me.on_event(e);
                }
            },
            move |s| me2.on_socket_status(s),
        );
        let time = Arc::new(Mutex::new(ClockOffsetEstimator::default()));
        self.lock().conn = Some(Conn {
            client: client.clone(),
            user_id: session.user_id.clone(),
            socket,
            time: time.clone(),
            tasks: Vec::new(),
        });
        self.when_open().await?;
        let tasks = vec![self.spawn_time_sync(client.clone(), time), self.spawn_drift()];
        if let Some(c) = self.lock().conn.as_mut() {
            c.tasks = tasks;
        }
        Ok(client)
    }

    async fn when_open(&self) -> Result<(), String> {
        for _ in 0..200 {
            let status = self.lock().conn.as_ref().map(|c| c.socket.status());
            match status {
                Some(SocketStatus::Open) => return Ok(()),
                Some(SocketStatus::Closed) | None => return Err("Socket closed".into()),
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
        Err("Socket did not open".into())
    }

    /// A burst of samples, then one now and then; pings the server with the round trip.
    fn spawn_time_sync(&self, client: Client, time: Arc<Mutex<ClockOffsetEstimator>>) -> JoinHandle<()> {
        runtime().spawn(async move {
            let mut taken = 0usize;
            loop {
                let t0 = now_ms();
                if let Ok((received, sent)) = sp::get_utc_time(&client).await {
                    let t3 = now_ms();
                    let (count, delay) = {
                        let mut est = time.lock().unwrap_or_else(|p| p.into_inner());
                        est.add(sp::time_sample(t0, received, sent, t3));
                        (est.count(), est.delay())
                    };
                    // The server places `When` far enough ahead for the slowest member's ping.
                    if count == 1 || count % 4 == 0 {
                        let _ = sp::send_ping(&client, delay).await;
                    }
                }
                taken += 1;
                let gap = if taken < 8 { 300 } else { 30_000 };
                tokio::time::sleep(Duration::from_millis(gap)).await;
            }
        })
    }

    fn spawn_drift(&self) -> JoinHandle<()> {
        let me = self.arc();
        runtime().spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                me.correct_drift();
            }
        })
    }

    fn time_offset(&self) -> (f64, bool) {
        let g = self.lock();
        match &g.conn {
            Some(c) => {
                let est = c.time.lock().unwrap_or_else(|p| p.into_inner());
                (est.offset(), est.count() > 0)
            }
            None => (0.0, false),
        }
    }

    fn server_now(&self) -> f64 {
        now_ms() + self.time_offset().0
    }

    fn fail_to_connect(&self) {
        if self.state().phase == Phase::Connecting {
            self.teardown();
            self.notify(Notice::Error);
        }
    }

    fn teardown(&self) {
        let (conn, command, buffer, music) = {
            let mut g = self.lock();
            let conn = g.conn.take();
            let command = g.command_task.take();
            let buffer = g.buffer_task.take();
            let music = g.music.clone();
            (conn, command, buffer, music)
        };
        if let Some(c) = conn {
            c.socket.close();
            c.tasks.iter().for_each(JoinHandle::abort);
        }
        command.iter().for_each(JoinHandle::abort);
        buffer.iter().for_each(JoinHandle::abort);
        self.set_rate(1.0);
        let _ = music;
        let (sheet_open, sheet_kind) = {
            let g = self.lock();
            (g.ui.sheet_open, g.ui.sheet_kind)
        };
        {
            let mut g = self.lock();
            let video = g.video.clone();
            let music = g.music.clone();
            *g = Inner {
                rate: 1.0,
                video,
                music,
                ..Inner::default()
            };
        }
        self.update(|s| {
            *s = SyncPlayState {
                sheet_open,
                sheet_kind,
                ..SyncPlayState::default()
            }
        });
    }

    // ----------------------------------------------------------------------- players

    /// Registers the video player (it reports no item while nothing is open).
    pub fn set_video(&self, player: Arc<dyn SyncPlayer>) {
        self.lock().video = Some(player);
    }

    /// The video player loaded an item: when it is the group's, wait on the group's position.
    pub fn video_loaded(&self) {
        let s = self.state();
        let video = self.lock().video.clone();
        let (Some(entry), Some(v)) = (playing_entry(&s.queue), video) else { return };
        if s.current_kind == Some(MediaKind::Video) && v.item_id().as_deref() == Some(entry.item_id.as_str()) {
            let start = s.queue.as_ref().map_or(0.0, |q| q.start_position_ticks / TICKS_PER_SECOND);
            self.align(Some(start));
        }
    }

    /// The player the group's current item plays in.
    fn player(&self) -> Option<Arc<dyn SyncPlayer>> {
        let kind = self.state().current_kind;
        let g = self.lock();
        match kind? {
            MediaKind::Video => g.video.clone(),
            MediaKind::Audio => g.music.as_ref().map(|m| m.player()),
        }
    }

    /// Marks that the manager is about to act on a player (its changes are not the person's).
    pub fn quiet_now(&self) {
        self.quiet();
    }

    fn quiet(&self) {
        self.lock().quiet_until = now_ms() + QUIET_MS;
    }

    /// Whether a change of a player right now is the manager's own doing.
    pub fn is_quiet(&self) -> bool {
        now_ms() < self.lock().quiet_until
    }

    /// Pauses on the group's position and owes the server a Ready once loaded there.
    fn align(&self, start_seconds: Option<f64>) {
        let Some(p) = self.player() else { return };
        {
            let mut g = self.lock();
            g.allow_play = false;
            g.last_unpause = None;
        }
        self.set_rate(1.0);
        self.quiet();
        p.pause();
        if let Some(s) = start_seconds {
            if (p.position() - s).abs() > 0.25 {
                p.seek(s);
            }
        }
        {
            let mut g = self.lock();
            g.awaiting_ready = true;
            g.reported_ready = None;
            g.grace_until = now_ms() + GRACE_MS;
        }
        self.player_changed();
    }

    /// A player changed (called by the bridges for every state change of the current player).
    pub fn player_changed(&self) {
        let Some(p) = self.player() else { return };
        let s = self.state();
        let Some(entry) = playing_entry(&s.queue) else { return };
        if p.item_id().as_deref() != Some(entry.item_id.as_str()) {
            // Another item than the group's (being switched): it waits for the group.
            if p.is_playing() {
                self.quiet();
                p.pause();
            }
            self.set_self_ready(false);
            return;
        }
        let ready = p.is_ready();
        self.set_self_ready(ready);
        let allow = self.lock().allow_play;
        if !allow && p.is_playing() {
            self.quiet();
            p.pause();
        }
        let (awaiting, reported, buffer_pending, grace_until) = {
            let g = self.lock();
            (g.awaiting_ready, g.reported_ready, g.buffer_task.is_some(), g.grace_until)
        };
        if awaiting {
            if ready {
                self.lock().awaiting_ready = false;
                self.report(true);
            }
            return;
        }
        if ready {
            if let Some(t) = self.lock().buffer_task.take() {
                t.abort();
            }
            if reported == Some(false) {
                self.report(true);
            }
            return;
        }
        // Stalled while the group plays: tell it to wait for us (unless it is our own seek).
        if reported == Some(false) || buffer_pending {
            return;
        }
        if s.group_state != GroupState::Playing || !allow {
            return;
        }
        let wait = BUFFERING_DEBOUNCE_MS.max(grace_until - now_ms());
        let me = self.arc();
        let task = runtime().spawn(async move {
            tokio::time::sleep(Duration::from_millis(wait as u64)).await;
            me.lock().buffer_task = None;
            if let Some(cur) = me.player() {
                if !cur.is_ready() && me.lock().allow_play {
                    me.report(false);
                }
            }
        });
        self.lock().buffer_task = Some(task);
    }

    fn set_self_ready(&self, ready: bool) {
        if self.state().self_ready == ready {
            return;
        }
        self.update(|s| s.self_ready = ready);
        self.refresh_members();
    }

    fn report(&self, ready: bool) {
        let (client, p, entry) = {
            let mut g = self.lock();
            let client = g.conn.as_ref().map(|c| c.client.clone());
            let entry = playing_entry(&g.ui.queue);
            let p = match g.ui.current_kind {
                Some(MediaKind::Video) => g.video.clone(),
                Some(MediaKind::Audio) => g.music.as_ref().map(|m| m.player()),
                None => None,
            };
            if client.is_some() {
                g.reported_ready = Some(ready);
            }
            (client, p, entry)
        };
        let (Some(client), Some(p), Some(entry)) = (client, p, entry) else { return };
        let body = sp::BufferingReport {
            when: self.server_now(),
            position_ticks: p.position() * TICKS_PER_SECOND,
            is_playing: p.is_playing(),
            playlist_item_id: entry.playlist_item_id,
        };
        runtime().spawn(async move {
            let _ = if ready {
                sp::report_ready(&client, &body).await
            } else {
                sp::report_buffering(&client, &body).await
            };
        });
    }

    fn set_rate(&self, rate: f64) {
        let current = self.player();
        let kind = self.state().current_kind;
        let previous = {
            let mut g = self.lock();
            let prev = if g.rate_kind.is_some() && g.rate_kind != kind {
                match g.rate_kind {
                    Some(MediaKind::Video) => g.video.clone(),
                    Some(MediaKind::Audio) => g.music.as_ref().map(|m| m.player()),
                    None => None,
                }
            } else {
                None
            };
            g.rate_kind = kind;
            g.rate = rate;
            prev
        };
        if let Some(p) = previous {
            p.set_rate(1.0);
        }
        if let Some(p) = current {
            p.set_rate(rate);
        }
    }

    fn correct_drift(&self) {
        let p = self.player();
        let (last, allow, grace, rate, entry) = {
            let g = self.lock();
            (g.last_unpause, g.allow_play, g.grace_until, g.rate, playing_entry(&g.ui.queue))
        };
        let (_, ready_clock) = self.time_offset();
        let (Some(p), Some((when, ticks)), Some(entry)) = (p, last, entry) else {
            if rate != 1.0 {
                self.set_rate(1.0);
            }
            return;
        };
        if !allow || !ready_clock {
            if rate != 1.0 {
                self.set_rate(1.0);
            }
            return;
        }
        if p.item_id().as_deref() != Some(entry.item_id.as_str()) || !p.is_playing() || !p.is_ready() {
            return;
        }
        if now_ms() < grace {
            return;
        }
        let expected = sp::expected_position(when, ticks, self.server_now());
        let can_nudge = self.state().current_kind == Some(MediaKind::Video);
        match sp::drift_action(p.position() - expected, can_nudge) {
            sp::DriftAction::Rate(r) => {
                if (r - rate).abs() > f64::EPSILON {
                    self.set_rate(r);
                }
            }
            sp::DriftAction::Seek => {
                self.set_rate(1.0);
                self.quiet();
                p.seek(expected);
                self.lock().grace_until = now_ms() + GRACE_MS;
            }
            sp::DriftAction::None => {}
        }
    }

    // ----------------------------------------------------------------------- local intents

    fn mark(&self, kind: LocalKind) {
        self.lock().last_local = Some((kind, now_ms()));
    }

    fn is_mine(&self, kind: LocalKind) -> bool {
        self.lock()
            .last_local
            .is_some_and(|(k, at)| k == kind && now_ms() - at < MINE_WINDOW_MS)
    }

    fn send<F, Fut>(&self, run: F)
    where
        F: FnOnce(Client) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = orphyn_jellyfin::JfResult<()>> + Send + 'static,
    {
        let Some(client) = self.lock().conn.as_ref().map(|c| c.client.clone()) else {
            return;
        };
        let me = self.arc();
        runtime().spawn(async move {
            if run(client).await.is_err() {
                me.notify(Notice::Error);
            }
        });
    }

    pub fn request_play(&self) {
        if playing_entry(&self.state().queue).is_none() {
            self.share_local(true);
            return;
        }
        self.mark(LocalKind::Unpause);
        self.send(|c| async move { sp::request_unpause(&c).await });
    }

    pub fn request_pause(&self) {
        self.mark(LocalKind::Pause);
        // Stop right away; the Pause command then puts everyone on the same frame.
        self.lock().allow_play = false;
        if let Some(p) = self.player() {
            self.quiet();
            p.pause();
        }
        self.send(|c| async move { sp::request_pause(&c).await });
    }

    pub fn request_seek(&self, seconds: f64) {
        self.mark(LocalKind::Seek);
        self.send(move |c| async move { sp::request_seek(&c, seconds * TICKS_PER_SECOND).await });
    }

    pub fn request_next(&self) {
        let Some(entry) = playing_entry(&self.state().queue) else { return };
        self.mark(LocalKind::Queue);
        self.send(move |c| async move { sp::next_item(&c, &entry.playlist_item_id).await });
    }

    pub fn request_previous(&self) {
        let Some(entry) = playing_entry(&self.state().queue) else { return };
        self.mark(LocalKind::Queue);
        self.send(move |c| async move { sp::previous_item(&c, &entry.playlist_item_id).await });
    }

    /// Adds to the group's queue without touching what plays.
    pub fn queue_items(&self, ids: Vec<String>, next: bool) {
        if ids.is_empty() {
            return;
        }
        self.mark(LocalKind::Queue);
        self.send(move |c| async move { sp::queue_items(&c, &ids, next).await });
    }

    /// The group's video ended here, with `next_id` (next episode) to follow: one request for all.
    pub fn advance_at_end(&self, next_id: &str) {
        let s = self.state();
        if let Some(q) = &s.queue {
            if q.playing_item_index >= 0 && (q.playing_item_index as usize) + 1 < q.playlist.len() {
                self.request_next(); // deduplicated by the server
                return;
            }
        }
        // Everyone gets here at once: the first member of the list asks, the others follow.
        if s.group.as_ref().and_then(|g| g.participants.first()) == Some(&s.self_name) {
            self.share_queue(vec![normalize(next_id)], 0, 0.0);
        }
    }

    /// Makes `item_ids` the group's queue (what this member just started).
    pub fn share_queue(&self, item_ids: Vec<String>, index: usize, start_seconds: f64) {
        if item_ids.is_empty() {
            return;
        }
        self.mark(LocalKind::Queue);
        self.send(move |c| async move {
            sp::set_new_queue(&c, &item_ids, index, start_seconds * TICKS_PER_SECOND).await
        });
    }

    /// The video player started `item_id`: the group follows unless it is the group's own item.
    pub fn video_item_started(&self, item_id: &str, start_seconds: f64) {
        if !self.is_active() {
            return;
        }
        let id = normalize(item_id);
        if playing_entry(&self.state().queue).is_some_and(|e| e.item_id == id) {
            return;
        }
        self.share_queue(vec![id], 0, start_seconds);
    }

    /// Shares what plays here (after creating a group, or "play" in an empty group).
    fn share_local(&self, force: bool) {
        let (video, music) = {
            let g = self.lock();
            (g.video.clone(), g.music.clone())
        };
        if let Some(v) = video {
            if let Some(id) = v.item_id() {
                self.share_queue(vec![id], 0, v.position());
                return;
            }
        }
        if let Some(m) = music {
            let (ids, index, playing) = m.local_queue();
            if index >= 0 && !ids.is_empty() && (playing || force) {
                self.share_queue(ids, index as usize, m.player().position());
            }
        }
    }

    /// Intent of the person on the music player: the jump to an entry of the group's queue.
    pub fn request_jump(&self, item_id: &str) {
        let entry = self
            .state()
            .queue
            .as_ref()
            .and_then(|q| q.playlist.iter().find(|e| e.item_id == item_id).cloned());
        let Some(entry) = entry else { return };
        self.mark(LocalKind::Queue);
        self.send(move |c| async move { sp::set_playlist_item(&c, &entry.playlist_item_id).await });
    }

    // ----------------------------------------------------------------------- server events

    fn on_socket_status(&self, status: SocketStatus) {
        let phase = self.state().phase;
        if status == SocketStatus::Reconnecting && phase == Phase::Joined {
            // The server drops a session from its group when its socket closes: rejoin once back.
            self.update(|s| s.phase = Phase::Reconnecting);
            self.notify(Notice::ConnectionLost);
        } else if status == SocketStatus::Open && phase == Phase::Reconnecting {
            self.rejoin();
        }
    }

    fn rejoin(&self) {
        let Some(group) = self.state().group.clone() else { return };
        {
            let mut g = self.lock();
            g.rejoining = true;
            // The room sends its queue again and waits for us: apply it even if it did not change.
            g.queue_key.clear();
        }
        self.send(move |c| async move { sp::join_group(&c, &group.id).await });
    }

    fn on_event(&self, e: SyncPlayEvent) {
        let group = self.state().group.clone();
        let mine = |id: &str| group.as_ref().is_none_or(|g| id.is_empty() || id == g.id);
        match e {
            SyncPlayEvent::GroupJoined(g) => self.on_group_joined(g),
            SyncPlayEvent::GroupLeft { group_id } => {
                if self.lock().leaving || !mine(&group_id) {
                    return;
                }
                self.teardown();
                self.notify(Notice::GroupClosed);
            }
            SyncPlayEvent::UserJoined { group_id, user } => {
                if mine(&group_id) {
                    self.on_user(user, true);
                }
            }
            SyncPlayEvent::UserLeft { group_id, user } => {
                if mine(&group_id) {
                    self.on_user(user, false);
                }
            }
            SyncPlayEvent::StateUpdate { group_id, state, reason } => {
                if mine(&group_id) {
                    self.on_state_update(state, &reason);
                }
            }
            SyncPlayEvent::PlayQueue { group_id, queue } => {
                if mine(&group_id) {
                    let me = self.arc();
                    runtime().spawn(async move { me.apply_queue(queue).await });
                }
            }
            SyncPlayEvent::Command(c) => {
                if mine(&c.group_id) {
                    self.apply_command(c);
                }
            }
            SyncPlayEvent::Error { kind, .. } => self.on_error(kind),
        }
    }
}
