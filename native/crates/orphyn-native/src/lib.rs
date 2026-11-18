//! Orphyn native: the desktop Jellyfin client in Rust, no web engine. Dioxus Native renders the
//! React app's own stylesheets with Blitz (Stylo + Taffy + Vello on wgpu). See native/README.md.
//!
//! Development variables: `ORPHYN_E2E_URL` / `_USER` / `_PASSWORD` (signs in when no account is
//! saved; also read from a git-ignored `native/.env`), `ORPHYN_ROUTE=/music/album/<id>` (first
//! page), `ORPHYN_WIN=1280x800` (window size), `ORPHYN_LANG=fr` (OS language),
//! `ORPHYN_USER_DATA_DIR` (debug: separate profile), `ORPHYN_MPV_DIR` (libmpv folder).
pub mod app;
pub mod assets;
pub mod design;
pub mod features;
pub mod i18n;
pub mod motion;
pub mod preferences;
pub mod query;
pub mod router;
pub mod shell;
pub mod state;
pub mod theme;
pub mod window;

/// `formatShortcut`: `⌘R` on macOS, `Ctrl+R` elsewhere.
pub fn platform_shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

/// Loads `KEY=value` lines of a `.env` next to the working directory (never overrides the env).
pub fn load_dotenv() {
    for path in [".env", "native/.env"] {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines().map(str::trim) {
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if std::env::var_os(k.trim()).is_none() {
                    // SAFETY: single-threaded at this point (before the UI and audio threads).
                    unsafe { std::env::set_var(k.trim(), v.trim()) };
                }
            }
        }
    }
}

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info && m.target().starts_with("orphyn")
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("{} {}", r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

/// Opens the window and runs the app until it closes.
pub fn run() {
    load_dotenv();
    let _ = log::set_logger(&StderrLogger).map(|()| log::set_max_level(log::LevelFilter::Info));
    let config = dioxus_native::Config::new()
        .with_window_attributes(window::attributes())
        .with_font_ctx(assets::font_context());
    dioxus_native::launch_cfg(app::app, vec![], vec![Box::new(config)]);
}
