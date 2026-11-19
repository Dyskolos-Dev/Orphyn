//! Picture in Picture: the music mini window and the video PiP window (`features/pip` of the
//! React app, `src-tauri/src/pip`). Each is a second, borderless, always-on-top, transparent
//! window with its own `VirtualDom` (the `ExtraWindows` patch of vendor/dioxus-native, see
//! native/README.md). Both windows share the app's services: the music window reads the player's
//! signals and calls the player directly; the video window draws the *same* mpv instance as the
//! player page (a second `VideoWidget`), so playback cannot drift and nothing is handed over.
mod chrome;
pub mod geometry;
mod music;
mod video;

use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_native::winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use dioxus_native::winit::window::{Window, WindowAttributes, WindowLevel};
use dioxus_native::{use_extra_windows, use_window, ExtraWindow, ExtraWindows};

pub use geometry::Kind;

use crate::features::music::{use_music, MusicUi};
use crate::i18n::{use_i18n, I18n};
use crate::state::{use_app, AppState, Services};
use crate::theme::{root_tokens, Resolved, WindowFacts};

/// What a PiP window needs from the main window; its root provides these as contexts.
struct Deps {
    svc: Services,
    app: AppState,
    music: MusicUi,
    i18n: I18n,
    main: Arc<dyn Window>,
}

#[derive(Clone)]
struct PipProps {
    deps: Rc<Deps>,
    kind: Kind,
    title: String,
}

impl PartialEq for PipProps {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.deps, &other.deps) && self.kind == other.kind && self.title == other.title
    }
}

/// The PiP windows of the app (a context of the main window): which are open, and how to open
/// and close them.
#[derive(Clone)]
pub struct Pip {
    music_window: Signal<Option<ExtraWindow>>,
    video_window: Signal<Option<ExtraWindow>>,
    deps: Rc<Deps>,
    windows: ExtraWindows,
}

pub fn use_pip() -> Pip {
    use_context::<Pip>()
}

/// Provides [`Pip`] at the root (after the music, i18n and app contexts).
pub fn use_pip_provider() -> Pip {
    let svc = use_context::<Services>();
    let app = use_app();
    let music = use_music();
    let i18n = use_i18n();
    let main = use_window();
    let windows = use_extra_windows();
    use_context_provider(|| Pip {
        music_window: Signal::new(None),
        video_window: Signal::new(None),
        deps: Rc::new(Deps {
            svc,
            app,
            music,
            i18n,
            main,
        }),
        windows,
    })
}

impl Pip {
    fn slot(&self, kind: Kind) -> Signal<Option<ExtraWindow>> {
        match kind {
            Kind::Music => self.music_window,
            Kind::Video => self.video_window,
        }
    }

    pub fn is_open(&self, kind: Kind) -> bool {
        self.slot(kind).peek().is_some()
    }

    /// Reactive `is_open` (for a pressed state of a button).
    pub fn is_open_signal(&self, kind: Kind) -> bool {
        self.slot(kind).read().is_some()
    }

    pub fn close(&self, kind: Kind) {
        let slot = self.slot(kind);
        let open = *slot.peek();
        if let Some(w) = open {
            self.windows.close(w);
        }
    }

    pub fn toggle(&self, kind: Kind, title: String) {
        if self.is_open(kind) {
            self.close(kind);
        } else {
            self.open(kind, title);
        }
    }

    /// Opens the PiP (one at a time, like the Tauri app: the other kind is closed first).
    /// `title` is the video's title line; the music window shows the player's track.
    pub fn open(&self, kind: Kind, title: String) {
        if self.is_open(kind) {
            return;
        }
        for other in [Kind::Music, Kind::Video] {
            if other != kind {
                self.close(other);
            }
        }
        let attributes = self.attributes(kind);
        let props = PipProps {
            deps: self.deps.clone(),
            kind,
            title,
        };
        let mut slot = self.slot(kind);
        let opened = self.windows.open(
            attributes,
            move || VirtualDom::new_with_props(pip_root, props),
            move || slot.set(None),
        );
        slot.set(Some(opened));
    }

    fn attributes(&self, kind: Kind) -> WindowAttributes {
        let main = &self.deps.main;
        let monitors: Vec<(i32, i32, u32, u32)> = main
            .available_monitors()
            .filter_map(|m| {
                let p = m.position()?;
                let s = m.current_video_mode()?.size();
                Some((p.x, p.y, s.width, s.height))
            })
            .collect();
        let scale = main.scale_factor();
        let ratio = kind.ratio();
        let mut width = (kind.default_width() * scale).round() as u32;
        let mut place = None;
        if let Some(p) = geometry::load(kind) {
            let w = p.width.clamp(
                (kind.min_width() * scale) as u32,
                (kind.max_width() * scale) as u32,
            );
            if geometry::is_visible(p.x, p.y, (w, geometry::height_for(w, ratio)), &monitors) {
                width = w;
                place = Some((p.x, p.y));
            }
        }
        let size = (width, geometry::height_for(width, ratio));
        if place.is_none() {
            let screen = main
                .current_monitor()
                .or_else(|| main.primary_monitor())
                .and_then(|m| {
                    let p = m.position()?;
                    let s = m.current_video_mode()?.size();
                    Some((p.x, p.y, s.width, s.height))
                });
            place = screen.map(|s| geometry::default_position(s, size, (24.0 * scale) as i32));
        }
        let mut attrs = WindowAttributes::default()
            .with_title(kind.title())
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_surface_size(PhysicalSize::new(size.0, size.1))
            .with_min_surface_size(LogicalSize::new(
                kind.min_width(),
                (kind.min_width() / ratio).round(),
            ))
            .with_max_surface_size(LogicalSize::new(
                kind.max_width(),
                (kind.max_width() / ratio).round(),
            ));
        if let Some((x, y)) = place {
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        attrs
    }
}

/// `PipButton`: opens (or closes) the PiP window of `kind`. `title` is the video's title line.
#[component]
pub fn PipButton(
    kind: Kind,
    #[props(default = "plain")] variant: &'static str,
    #[props(default)] title: String,
) -> Element {
    let t = use_i18n();
    let pip = use_pip();
    let showing = pip.is_open_signal(kind);
    let label = if showing {
        t.t("pip:close")
    } else if kind == Kind::Music {
        t.t("pip:openMusic")
    } else {
        t.t("pip:openVideo")
    };
    rsx! {
        crate::design::IconButton {
            icon: "picture-in-picture-2",
            variant,
            label,
            pressed: showing,
            onclick: move |_| pip.toggle(kind, title.clone()),
        }
    }
}

/// Hides the player page's own video while the PiP shows the same mpv instance (two widgets
/// would take turns on the frames). Mounted once in the main window.
#[component]
pub fn PipHost() -> Element {
    let pip = use_pip();
    rsx! {
        if pip.is_open_signal(Kind::Video) {
            style { ".jr-video-player__video {{ display: none !important; }}" }
        }
    }
}

/// The PiP stylesheet: `pip.css` of the React app, its `html[data-...]` selectors anchored on
/// `.jr-root` (where the tokens live), plus what Blitz needs on top.
fn pip_css() -> &'static str {
    use std::sync::OnceLock;
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let react = include_str!("../../../../../../src/renderer/src/features/pip/pip.css")
            .replace("html[data-", ".jr-root[data-");
        format!("{react}\n{}", include_str!("pip_native.css"))
    })
}

/// Root component of a PiP window.
fn pip_root(props: PipProps) -> Element {
    let deps = props.deps.clone();
    let svc = use_context_provider(|| deps.svc.clone());
    use_context_provider(|| deps.app);
    use_context_provider(|| deps.music);
    use_context_provider(|| deps.i18n);
    let _ = svc;
    let app = deps.app;
    let theme = app.theme.read().clone();
    let resolved = theme.resolve((app.os_dark)());
    let iface = app.interface.read().clone();
    let facts = WindowFacts {
        platform: crate::window::platform(),
        material: "none",
        fullscreen: false,
        focused: true,
    };
    let tokens = root_tokens(&theme, resolved, &iface, facts);
    let attrs: Vec<Attribute> = tokens
        .attributes
        .iter()
        .map(|(k, v)| Attribute::new(k, v.clone(), None, false))
        .collect();
    // PiP cards are always dark (`data-theme="dark"` on the card), whatever the app theme.
    let _ = Resolved::Dark;
    let kind = props.kind;
    let title = props.title.clone();
    let pip = chrome::use_pip_chrome(kind, deps.main.clone());
    rsx! {
        style { {crate::assets::APP_CSS} }
        style { {crate::assets::OVERRIDES_CSS} }
        style { {pip_css()} }
        div { class: "jr-root", style: "{tokens.style}", ..attrs,
            div { class: "jr-body",
                match kind {
                    Kind::Music => rsx! { music::MusicPip { chrome: pip.clone() } },
                    Kind::Video => rsx! { video::VideoPip { chrome: pip.clone(), title } },
                }
            }
        }
    }
}
