//! Music player types (port of `features/music/player/types.ts`).
use std::sync::Arc;

/// A playable track: a Jellyfin `Audio` item, like `Track = BaseItemDto` in the React app.
pub type Track = orphyn_jellyfin::BaseItemDto;

/// One entry of the queue. `uid` is unique per entry so the same track can be queued twice.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    pub uid: String,
    pub track: Arc<Track>,
}

impl QueueItem {
    pub fn new(uid: String, track: Track) -> Self {
        Self {
            uid,
            track: Arc::new(track),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// The i18n suffix of `player.repeat.<mode>`.
    pub fn as_str(self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Error,
}

/// Pure queue model shared by the controller and its tests.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueModel {
    /// Playback order (already shuffled when `shuffle` is on).
    pub queue: Vec<QueueItem>,
    /// Original order, kept only while shuffle is on, to restore it when shuffle is turned off.
    pub original: Option<Vec<QueueItem>>,
    /// Index of the current entry in `queue`; -1 when the queue is empty.
    pub index: isize,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Default for QueueModel {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            original: None,
            index: -1,
            shuffle: false,
            repeat: RepeatMode::Off,
        }
    }
}

/// What the UI reads (`PlayerState` of `store.ts` without the engine plumbing). Position and
/// duration change several times a second and live in [`Progress`] instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerState {
    pub model: QueueModel,
    pub status: PlayerStatus,
    pub volume: f32,
    pub muted: bool,
    /// The user wants audio (play pressed / autoplay) as opposed to paused.
    pub want_playing: bool,
}

impl PlayerState {
    pub fn current(&self) -> Option<&QueueItem> {
        super::queue::current_item(&self.model)
    }

    /// Same rule as `useRowPlayback`: playing for the UI.
    pub fn is_playing(&self) -> bool {
        self.want_playing
            && self.status != PlayerStatus::Paused
            && self.status != PlayerStatus::Error
    }

    pub fn has_queue(&self) -> bool {
        !self.model.queue.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub position: f64,
    pub duration: f64,
}

/// How one track hands over to the next. Only `Gapless` is implemented by the engine so far; the
/// crossfade settings (`crossfade`, `crossfadeSeconds`, `crossfadeSkipAlbum` preferences) are
/// stored here for the music port to implement (see README, "Player core").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Transition {
    #[default]
    Gapless,
    Crossfade { seconds: f32, skip_same_album: bool },
}
