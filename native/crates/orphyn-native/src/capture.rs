//! Capture mode (development only, no UI). The window stays hidden and nothing is grabbed from the
//! screen: the document is laid out at the capture size and painted with Vello's CPU renderer into
//! an image, so a capture has no taskbar and no other window and does not depend on the monitor.
//!
//! * `ORPHYN_SHOT=<file.png>`: waits until the route's data is in, every image is decoded and the
//!   screen has been still for `ORPHYN_SHOT_DELAY_MS` (2000), writes the PNG and exits.
//! * `ORPHYN_TOUR=<dir>` + `ORPHYN_TOUR_SCRIPT=<steps.json>`: a scripted tour for a demo video.
//!   Each step `{"nav": "/video", "play": "<album>:<track>:<seconds>", "sheet":
//!   "player|lyrics|queue|close", "hold": 3.5}` runs its actions, waits for the screen to load,
//!   then writes `hold` seconds of frames (`frame_00001.png`...) at 30 fps on a virtual clock, so
//!   transitions and animations play smoothly however long a frame takes to paint.
//!
//! Common variables: `ORPHYN_WIN=1440x900` (logical size), `ORPHYN_SCALE` (pixel ratio, 2 for a
//! shot so 2880x1800 px, 1 for a tour), `ORPHYN_ROUTE`, `ORPHYN_LANG`, `ORPHYN_THEME=dark|light`,
//! `ORPHYN_STYLE`, `ORPHYN_SHOT_PLAY=<album>[:<track>[:<seconds>]]` (plays muted),
//! `ORPHYN_SHOT_SHEET=player|lyrics|queue` (opens Now Playing), `ORPHYN_SHOT_TIMEOUT_S` (120).
//! What's new, the update notice and toasts are not shown in this mode.
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyrender::ImageRenderer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::BaseDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus::prelude::*;
use serde::Deserialize;

use crate::features::music::{MusicUi, NowPlayingView};
use crate::theme::ThemeMode;

/// Nothing is captured before this (the sign-in and the first fetches have to start).
const MIN_WAIT: Duration = Duration::from_secs(4);
/// What the app needs after an action to start changing the screen.
const ACTION_WAIT: Duration = Duration::from_millis(700);
const FPS: f64 = 30.0;

#[derive(Debug)]
pub struct Settings {
    pub shot: Option<PathBuf>,
    pub tour: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub settle: Duration,
    pub theme: Option<ThemeMode>,
}

/// The capture settings, `None` outside capture mode.
pub fn settings() -> Option<&'static Settings> {
    static SETTINGS: OnceLock<Option<Settings>> = OnceLock::new();
    let path = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    SETTINGS
        .get_or_init(|| {
            let (shot, tour) = (path("ORPHYN_SHOT"), path("ORPHYN_TOUR"));
            if shot.is_none() && tour.is_none() {
                return None;
            }
            let (width, height) = std::env::var("ORPHYN_WIN")
                .ok()
                .and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
                .unwrap_or((1440, 900));
            let scale = std::env::var("ORPHYN_SCALE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|s| *s >= 1.0)
                .unwrap_or(if tour.is_some() { 1.0 } else { 2.0 });
            let settle = std::env::var("ORPHYN_SHOT_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .map_or(Duration::from_millis(2000), Duration::from_millis);
            let theme = match std::env::var("ORPHYN_THEME").as_deref() {
                Ok("dark") => Some(ThemeMode::Dark),
                Ok("light") => Some(ThemeMode::Light),
                _ => None,
            };
            Some(Settings {
                shot,
                tour,
                width,
                height,
                scale,
                settle,
                theme,
            })
        })
        .as_ref()
}

pub fn active() -> bool {
    settings().is_some()
}

fn tour_active() -> bool {
    settings().is_some_and(|s| s.tour.is_some())
}

// ---------------------------------------------------------------- painting

struct Painter {
    renderer: VelloCpuImageRenderer,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl Painter {
    fn new(s: &Settings) -> Self {
        let (width, height) = (
            (s.width as f64 * s.scale).round() as u32,
            (s.height as f64 * s.scale).round() as u32,
        );
        Self {
            renderer: VelloCpuImageRenderer::new(width, height),
            pixels: Vec::new(),
            width,
            height,
        }
    }

    /// Lays the document out at the capture size and paints it at animation clock `time`.
    fn paint(&mut self, doc: &mut BaseDocument, s: &Settings, time: f64) -> &[u8] {
        let scheme = match s.theme {
            Some(ThemeMode::Dark) => ColorScheme::Dark,
            _ => ColorScheme::Light,
        };
        doc.set_viewport(Viewport::new(
            self.width,
            self.height,
            s.scale as f32,
            scheme,
        ));
        doc.resolve(time);
        doc.resolve(time);
        // Draw commands of the previous frame must not be replayed (their images may be evicted).
        self.renderer.reset();
        let (w, h) = (self.width, self.height);
        self.renderer.render_to_vec(
            |scene| blitz_paint::paint_scene(scene, doc, s.scale, w, h, 0, 0),
            &mut self.pixels,
        );
        // The pixmap is premultiplied; the root paints an opaque background, so un-premultiplying
        // only matters on anti-aliased edges against it.
        for px in self.pixels.as_chunks_mut::<4>().0.iter_mut() {
            let a = px[3] as u32;
            if a != 0 && a != 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
                px[3] = 255;
            }
        }
        &self.pixels
    }
}

fn write_png(
    path: &Path,
    width: u32,
    height: u32,
    pixels: &[u8],
    fast: bool,
) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if fast {
        encoder.set_compression(png::Compression::Fast);
    }
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| e.to_string())
}

const SKELETONS: &str = ".jr-skeleton, .jr-spinner, [aria-busy='true']";

/// No skeleton or spinner left and no image still being fetched or decoded.
fn is_loaded(doc: &BaseDocument) -> bool {
    doc.pending_image_count() == 0
        && !doc.has_pending_critical_resources()
        && doc
            .query_selector_all(SKELETONS)
            .map(|n| n.is_empty())
            .unwrap_or(true)
}

fn fail(message: &str) -> ! {
    eprintln!("[capture] {message}");
    std::process::exit(1);
}

// ---------------------------------------------------------------- the hooks

/// Registers the hook that paints the screen (or the tour) and exits.
pub fn install() {
    let Some(s) = settings() else { return };
    let start = Instant::now();
    let timeout = std::env::var("ORPHYN_SHOT_TIMEOUT_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(
            Duration::from_secs(if s.tour.is_some() { 900 } else { 120 }),
            Duration::from_secs,
        );
    let mut painter = Painter::new(s);
    if let Some(dir) = &s.tour {
        let mut tour = Tour::new(load_tour(), dir.clone());
        dioxus_native::set_frame_hook(Box::new(move |doc, event_loop| {
            if start.elapsed() > timeout {
                fail("timed out");
            }
            tour.tick(doc, event_loop, &mut painter, s, start)
        }));
        return;
    }
    let out = s.shot.clone().unwrap_or_default();
    let mut settled_since: Option<Instant> = None;
    dioxus_native::set_frame_hook(Box::new(move |doc, event_loop| {
        let now = Instant::now();
        let time = now.duration_since(start).as_secs_f64();
        doc.handle_messages();
        doc.resolve(time);
        if now.duration_since(start) > timeout {
            fail(&format!(
                "timed out: {} images pending, {} loading markers",
                doc.pending_image_count(),
                doc.query_selector_all(SKELETONS).map_or(0, |n| n.len())
            ));
        }
        if !is_loaded(doc) || now.duration_since(start) < MIN_WAIT {
            settled_since = None;
            return false;
        }
        let since = *settled_since.get_or_insert(now);
        if now.duration_since(since) < s.settle {
            return false;
        }
        let (pw, ph) = (painter.width, painter.height);
        let pixels = painter.paint(doc, s, time);
        match write_png(&out, pw, ph, pixels, false) {
            Ok(()) => {
                eprintln!("[capture] wrote {}", out.display());
                event_loop.exit();
            }
            Err(e) => fail(&e),
        }
        false
    }));
}

// ---------------------------------------------------------------- the tour

#[derive(Deserialize, Default)]
#[serde(default)]
struct Step {
    nav: Option<String>,
    play: Option<String>,
    sheet: Option<String>,
    hold: f64,
}

fn load_tour() -> Vec<Step> {
    let Some(path) = std::env::var_os("ORPHYN_TOUR_SCRIPT") else {
        fail("ORPHYN_TOUR_SCRIPT is not set");
    };
    std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        .unwrap_or_else(|e| fail(&format!("tour script: {e}")))
}

enum Action {
    Nav(String),
    Play(String),
    Sheet(String),
}

static ACTIONS: Mutex<VecDeque<Action>> = Mutex::new(VecDeque::new());

#[derive(Clone, Copy)]
enum Phase {
    Issue,
    Wait {
        issued: Instant,
        stable: Option<Instant>,
    },
    Render {
        left: u32,
    },
}

struct Tour {
    steps: Vec<Step>,
    dir: PathBuf,
    step: usize,
    phase: Phase,
    frame: u32,
    clock: f64,
}

impl Tour {
    fn new(steps: Vec<Step>, dir: PathBuf) -> Self {
        Self {
            steps,
            dir,
            step: 0,
            phase: Phase::Issue,
            frame: 0,
            clock: 0.0,
        }
    }

    /// One hook call; `true` asks for the next one at once (frames are being written).
    fn tick(
        &mut self,
        doc: &mut BaseDocument,
        event_loop: &dyn dioxus_native::winit::event_loop::ActiveEventLoop,
        painter: &mut Painter,
        s: &Settings,
        start: Instant,
    ) -> bool {
        let now = Instant::now();
        doc.handle_messages();
        doc.resolve(self.clock);
        match self.phase {
            Phase::Issue => {
                let Some(step) = self.steps.get(self.step) else {
                    eprintln!("[capture] {} frames in {}", self.frame, self.dir.display());
                    event_loop.exit();
                    return false;
                };
                let mut queue = ACTIONS.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(p) = &step.play {
                    queue.push_back(Action::Play(p.clone()));
                }
                if let Some(n) = &step.nav {
                    queue.push_back(Action::Nav(n.clone()));
                }
                if let Some(v) = &step.sheet {
                    queue.push_back(Action::Sheet(v.clone()));
                }
                self.phase = Phase::Wait {
                    issued: now,
                    stable: None,
                };
                false
            }
            Phase::Wait { issued, stable } => {
                let warmed = now.duration_since(issued) >= ACTION_WAIT
                    && (self.step > 0 || now.duration_since(start) >= MIN_WAIT);
                if !warmed || !is_loaded(doc) {
                    self.phase = Phase::Wait {
                        issued,
                        stable: None,
                    };
                    return false;
                }
                let since = stable.unwrap_or(now);
                self.phase = if now.duration_since(since) >= Duration::from_millis(600) {
                    let hold = self.steps[self.step].hold.max(0.0);
                    Phase::Render {
                        left: (hold * FPS).round() as u32,
                    }
                } else {
                    Phase::Wait {
                        issued,
                        stable: Some(since),
                    }
                };
                false
            }
            Phase::Render { left } => {
                if left == 0 {
                    self.step += 1;
                    self.phase = Phase::Issue;
                    return true;
                }
                self.clock += 1.0 / FPS;
                self.frame += 1;
                let path = self.dir.join(format!("frame_{:05}.png", self.frame));
                let (pw, ph) = (painter.width, painter.height);
                let pixels = painter.paint(doc, s, self.clock);
                if let Err(e) = write_png(&path, pw, ph, pixels, true) {
                    fail(&e);
                }
                self.phase = Phase::Render { left: left - 1 };
                true
            }
        }
    }
}

// ---------------------------------------------------------------- app side

/// Plays `<album>[:<track>[:<seconds>]]` muted, once the account is signed in.
async fn play_spec(svc: &crate::state::Services, app: crate::state::AppState, spec: &str) {
    use orphyn_jellyfin::keys::{self, stale};
    let mut parts = spec.split(':');
    let album = parts.next().unwrap_or_default().to_string();
    let index: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let seek: f64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let (player, cache) = (svc.music.clone(), svc.cache.clone());
    let session = loop {
        if let Some(s) = app.session.peek().clone() {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let key = keys::album_tracks(&session.account_id, &album);
    let s2 = session.clone();
    let Ok(list) = cache
        .fetch::<Vec<orphyn_jellyfin::BaseItemDto>, _, _>(
            key,
            stale::CATALOG,
            move || async move { s2.album_tracks(&album).await },
        )
        .await
    else {
        eprintln!("[capture] the album's tracks could not be fetched");
        return;
    };
    player.set_volume(0.0);
    player.play(&list, Some(index), Some(false));
    if seek > 0.0 {
        tokio::time::sleep(Duration::from_millis(2500)).await;
        player.seek(seek);
    }
}

fn show_sheet(ui: MusicUi, view: &str) {
    let mut open = ui.open;
    match view {
        "player" => ui.show(NowPlayingView::Player),
        "lyrics" => ui.show(NowPlayingView::Lyrics),
        "queue" => ui.show(NowPlayingView::Queue),
        _ => open.set(false),
    }
}

/// Capture scenes driven from the app: `ORPHYN_SHOT_PLAY` / `ORPHYN_SHOT_SHEET`, and the tour's
/// actions (navigation, playback, Now Playing).
pub fn spawn_scene(
    svc: &crate::state::Services,
    app: crate::state::AppState,
    ui: MusicUi,
    router: crate::router::Router,
) {
    if !active() {
        return;
    }
    let svc = svc.clone();
    spawn(async move {
        if let Ok(spec) = std::env::var("ORPHYN_SHOT_PLAY") {
            play_spec(&svc, app, &spec).await;
        }
        if let Ok(view) = std::env::var("ORPHYN_SHOT_SHEET") {
            show_sheet(ui, &view);
        }
        if !tour_active() {
            return;
        }
        loop {
            let next = ACTIONS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_front();
            match next {
                Some(Action::Nav(path)) => router.push(&path),
                Some(Action::Play(spec)) => play_spec(&svc, app, &spec).await,
                Some(Action::Sheet(view)) => show_sheet(ui, &view),
                None => tokio::time::sleep(Duration::from_millis(40)).await,
            }
        }
    });
}

/// `ORPHYN_STYLE=classic|glass|cinema|minimal|compact` (capture mode only).
pub fn interface(mut state: crate::theme::InterfaceState) -> crate::theme::InterfaceState {
    if active() {
        let wanted = std::env::var("ORPHYN_STYLE").unwrap_or_default();
        if let Some(style) = crate::theme::INTERFACE_STYLES
            .iter()
            .copied()
            .find(|s| *s == wanted)
        {
            state.style = style;
        }
    }
    state
}

/// `ORPHYN_SHOT_WHATSNEW=1` shows What's new as after an update (needs a profile that already has
/// the account, the sheet only opens for signed-in profiles).
pub fn shows_whats_new() -> bool {
    active() && std::env::var_os("ORPHYN_SHOT_WHATSNEW").is_some()
}

/// Capture mode setup that has to happen before the first screen is built.
pub fn prepare(svc: &crate::state::Services) {
    if shows_whats_new() {
        svc.storage.set("orphyn.lastSeenVersion", "1.0.2");
    }
}
