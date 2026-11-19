//! What both PiP windows share: the window handle, the logical-pixel unit of the card, the
//! ratio lock, remembered placement, drag from anywhere, resize from every edge
//! (`windowHandling.tsx`).
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use dioxus_native::winit::dpi::PhysicalSize;
use dioxus_native::winit::event::WindowEvent;
use dioxus_native::winit::window::{ResizeDirection, Window};
use dioxus_native::{use_extra_windows, use_window, ExtraWindows};

use super::geometry::{self, Kind, Placement};
use crate::features::music::{use_music, MusicUi, NowPlayingView};

#[derive(Clone)]
pub struct PipChrome {
    pub kind: Kind,
    pub win: Arc<dyn Window>,
    main: Arc<dyn Window>,
    windows: ExtraWindows,
    music: MusicUi,
    /// Logical width of the window.
    pub width: Signal<f64>,
    /// Where a press started (a drag begins once the pointer has moved a few pixels).
    press: Signal<Option<(f64, f64)>>,
}

impl PartialEq for PipChrome {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && Arc::ptr_eq(&self.win, &other.win) && self.width == other.width
    }
}

impl PipChrome {
    pub fn close(&self) {
        self.windows.close_window(self.win.id());
    }

    /// Brings the main window back (it may be minimised) and closes the PiP. The music button
    /// also opens Now Playing; the video one leaves the player page, still mounted, in front.
    pub fn back_to_main(&self) {
        if self.kind == Kind::Music {
            self.music.show(NowPlayingView::Player);
        }
        self.main.set_visible(true);
        self.main.set_minimized(false);
        self.main.focus_window();
        self.close();
    }

    /// One design pixel of the music card: the window width / 320, in CSS pixels.
    pub fn unit(&self) -> f64 {
        (self.width)() / geometry::DESIGN_WIDTH
    }

    /// Double-click: switches between the compact and the large size.
    pub fn toggle_compact(&self) {
        let scale = self.win.scale_factor();
        let width = geometry::toggled_width((self.width)()) * scale;
        let w = width.round() as u32;
        let _ = self
            .win
            .request_surface_size(PhysicalSize::new(w, geometry::height_for(w, self.kind.ratio())).into());
    }

    pub fn press(mut self, x: f64, y: f64) {
        self.press.set(Some((x, y)));
    }

    pub fn release(mut self) {
        self.press.set(None);
    }

    /// Pointer move: starts the OS drag once past the threshold.
    pub fn moved(mut self, x: f64, y: f64) {
        let Some((sx, sy)) = *self.press.peek() else {
            return;
        };
        if geometry::past_threshold(x - sx, y - sy) {
            self.press.set(None);
            let _ = self.win.drag_window();
        }
    }

    pub fn resize(&self, dir: ResizeDirection) {
        let _ = self.win.drag_resize_window(dir);
    }
}

/// Sets up the window facts: width signal, ratio lock, placement saving.
pub fn use_pip_chrome(kind: Kind, main: Arc<dyn Window>) -> PipChrome {
    let win = use_window();
    let windows = use_extra_windows();
    let music = use_music();
    let mut width = use_signal(|| win.surface_size().width as f64 / win.scale_factor());
    let press = use_signal(|| None);
    let last = use_hook(|| Rc::new(Cell::new(win.surface_size())));
    let saved_at = use_hook(|| Rc::new(Cell::new(Instant::now())));
    let ev_win = win.clone();
    dioxus_native::use_window_event(move |event, _| match event {
        WindowEvent::SurfaceResized(size) => {
            let scale = ev_win.scale_factor();
            let ratio = kind.ratio();
            let prev = last.get();
            // The edge that moved decides: a vertical drag drives the width.
            let target = if size.width == prev.width && size.height != prev.height {
                let w = (f64::from(size.height) * ratio).round() as u32;
                PhysicalSize::new(w, size.height)
            } else {
                PhysicalSize::new(size.width, geometry::height_for(size.width, ratio))
            };
            let target = clamp(target, kind, scale);
            if target != *size {
                last.set(target);
                let _ = ev_win.request_surface_size(target.into());
            } else {
                last.set(*size);
                width.set(f64::from(size.width) / scale);
            }
            remember(&ev_win, kind, &saved_at);
        }
        WindowEvent::Moved(_) => remember(&ev_win, kind, &saved_at),
        _ => {}
    });
    PipChrome {
        kind,
        win,
        main,
        windows,
        music,
        width,
        press,
    }
}

fn clamp(size: PhysicalSize<u32>, kind: Kind, scale: f64) -> PhysicalSize<u32> {
    let w = size.width.clamp(
        (kind.min_width() * scale).round() as u32,
        (kind.max_width() * scale).round() as u32,
    );
    PhysicalSize::new(w, geometry::height_for(w, kind.ratio()))
}

fn remember(win: &Arc<dyn Window>, kind: Kind, at: &Cell<Instant>) {
    if at.get().elapsed() < Duration::from_millis(400) {
        return;
    }
    at.set(Instant::now());
    if let Ok(p) = win.outer_position() {
        geometry::save(
            kind,
            Placement {
                x: p.x,
                y: p.y,
                width: win.surface_size().width,
            },
        );
    }
}

const HANDLES: [(&str, ResizeDirection); 8] = [
    ("n", ResizeDirection::North),
    ("s", ResizeDirection::South),
    ("e", ResizeDirection::East),
    ("w", ResizeDirection::West),
    ("ne", ResizeDirection::NorthEast),
    ("nw", ResizeDirection::NorthWest),
    ("se", ResizeDirection::SouthEast),
    ("sw", ResizeDirection::SouthWest),
];

const GRIP: &str = "<svg viewBox=\"0 0 12 12\" width=\"12\" height=\"12\" aria-hidden=\"true\" class=\"pip__grip\"><path d=\"M11 4 4 11M11 8 8 11\" fill=\"none\" stroke=\"rgba(255,255,255,0.55)\" stroke-width=\"1.2\" stroke-linecap=\"round\"/></svg>";

/// Invisible hit areas on every edge and corner (a frameless window has no border), plus a
/// grip in the bottom-right corner on hover.
#[component]
pub fn ResizeHandles(chrome: PipChrome) -> Element {
    rsx! {
        for (class, dir) in HANDLES {
            div {
                key: "{class}",
                class: "pip__resize pip__resize--{class}",
                onmousedown: {
                    let chrome = chrome.clone();
                    move |e: MouseEvent| {
                        e.stop_propagation();
                        chrome.resize(dir);
                    }
                },
            }
        }
        span { dangerous_inner_html: GRIP }
    }
}
