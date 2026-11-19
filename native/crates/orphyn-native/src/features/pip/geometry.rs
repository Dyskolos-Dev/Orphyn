//! Size rules and remembered placement of the PiP windows (`src-tauri/src/pip/mod.rs`, same
//! `pip.json`): fixed width / height ratio, minimum and maximum width, the double-click size
//! switch of `windowHandling.tsx`, and "is the remembered position still on a screen".
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Music,
    Video,
}

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::Music => "music",
            Kind::Video => "video",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Music => "Orphyn - mini player",
            Kind::Video => "Orphyn - Picture in Picture",
        }
    }

    /// Width / height of the window.
    pub fn ratio(self) -> f64 {
        match self {
            Kind::Music => 320.0 / 380.0,
            Kind::Video => 16.0 / 9.0,
        }
    }

    /// Logical width the window opens at.
    pub fn default_width(self) -> f64 {
        match self {
            Kind::Music => 320.0,
            Kind::Video => 480.0,
        }
    }

    pub fn min_width(self) -> f64 {
        match self {
            Kind::Music => 240.0,
            Kind::Video => 320.0,
        }
    }

    pub fn max_width(self) -> f64 {
        match self {
            Kind::Music => 640.0,
            Kind::Video => 960.0,
        }
    }
}

/// Height that keeps `ratio` (width / height) for a width.
pub fn height_for(width: u32, ratio: f64) -> u32 {
    (f64::from(width) / ratio).round() as u32
}

/// Width (px) the music design is drawn at: lengths are multiples of `width / DESIGN_WIDTH`.
pub const DESIGN_WIDTH: f64 = 320.0;

/// Compact and large widths (logical px) the double-click switches between.
pub const COMPACT_WIDTH: f64 = 260.0;
pub const LARGE_WIDTH: f64 = 420.0;

/// Width the double-click goes to: large from anything up to the middle, else compact.
pub fn toggled_width(current: f64) -> f64 {
    if current > (COMPACT_WIDTH + LARGE_WIDTH) / 2.0 {
        COMPACT_WIDTH
    } else {
        LARGE_WIDTH
    }
}

/// Pixels the pointer travels before a press becomes a window drag.
pub const DRAG_THRESHOLD: f64 = 4.0;

pub fn past_threshold(dx: f64, dy: f64) -> bool {
    dx.hypot(dy) >= DRAG_THRESHOLD
}

/// Artwork widths (px) asked from the server.
pub const ARTWORK_STEPS: [u32; 4] = [320, 480, 640, 960];

/// Smallest step covering `pixels` (so a bitmap is never upscaled), capped at the largest.
pub fn artwork_step(pixels: f64) -> u32 {
    ARTWORK_STEPS
        .into_iter()
        .find(|s| f64::from(*s) >= pixels)
        .unwrap_or(ARTWORK_STEPS[ARTWORK_STEPS.len() - 1])
}

/// Remembered window placement, physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
}

/// Whether a window of `size` at `(x, y)` overlaps one of the `monitors` (`x, y, w, h`).
pub fn is_visible(x: i32, y: i32, size: (u32, u32), monitors: &[(i32, i32, u32, u32)]) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        x + size.0 as i32 > mx + 40
            && x < mx + mw as i32 - 40
            && y + size.1 as i32 > my + 40
            && y < my + mh as i32 - 40
    })
}

/// Bottom-right corner of a monitor with a margin (the default place of a first PiP).
pub fn default_position(
    monitor: (i32, i32, u32, u32),
    size: (u32, u32),
    margin: i32,
) -> (i32, i32) {
    (
        monitor.0 + monitor.2 as i32 - size.0 as i32 - margin,
        monitor.1 + monitor.3 as i32 - size.1 as i32 - margin,
    )
}

fn path() -> Option<std::path::PathBuf> {
    if orphyn_core::profile::is_test() {
        return None;
    }
    orphyn_core::profile::config_dir()
        .ok()
        .map(|d| d.join("pip.json"))
}

fn load_all() -> HashMap<String, Placement> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load(kind: Kind) -> Option<Placement> {
    load_all().get(kind.key()).copied()
}

pub fn save(kind: Kind, placement: Placement) {
    let Some(path) = path() else {
        return;
    };
    let mut all = load_all();
    all.insert(kind.key().to_string(), placement);
    if let Ok(text) = serde_json::to_string_pretty(&all) {
        let _ = orphyn_core::accounts::store::write_atomic(&path, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_follows_the_ratio() {
        assert_eq!(height_for(320, Kind::Music.ratio()), 380);
        assert_eq!(height_for(480, Kind::Video.ratio()), 270);
    }

    #[test]
    fn double_click_switches_between_compact_and_large() {
        assert_eq!(toggled_width(320.0), LARGE_WIDTH);
        assert_eq!(toggled_width(260.0), LARGE_WIDTH);
        assert_eq!(toggled_width(420.0), COMPACT_WIDTH);
        assert_eq!(toggled_width(341.0), COMPACT_WIDTH);
    }

    #[test]
    fn a_press_becomes_a_drag_past_the_threshold() {
        assert!(!past_threshold(2.0, 2.0));
        assert!(past_threshold(3.0, 3.0));
    }

    #[test]
    fn artwork_never_upscales_and_is_capped() {
        assert_eq!(artwork_step(300.0), 320);
        assert_eq!(artwork_step(321.0), 480);
        assert_eq!(artwork_step(2000.0), 960);
    }

    #[test]
    fn remembered_position_must_be_reachable() {
        let screen = [(0, 0, 1920, 1080)];
        assert!(is_visible(100, 100, (320, 380), &screen));
        assert!(!is_visible(3000, 100, (320, 380), &screen));
        assert!(!is_visible(100, -500, (320, 380), &screen));
    }

    #[test]
    fn default_position_is_bottom_right() {
        assert_eq!(
            default_position((0, 0, 1920, 1080), (320, 380), 24),
            (1576, 676)
        );
    }
}
