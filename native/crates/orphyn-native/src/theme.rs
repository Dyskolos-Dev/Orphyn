//! Theme and interface style: ports of `design/theme/{color,theme,appearance,interfaceStyle}.ts`.
//! Same storage keys and defaults, same accessible accent maths. The React app writes these as
//! attributes and CSS variables on `<html>`; here they go on the `.jr-root` element (see
//! [`RootTokens`]), which `build.rs` made the anchor of every `html[...]` selector.
use orphyn_core::storage::LocalStorage;
use serde_json::Value;

// ---------------------------------------------------------------- color.ts

pub type Rgb = [f64; 3];

pub fn parse_hex(hex: &str) -> Option<Rgb> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([((n >> 16) & 255) as f64, ((n >> 8) & 255) as f64, (n & 255) as f64])
}

pub fn to_hex(c: Rgb) -> String {
    let ch = |v: f64| v.clamp(0.0, 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", ch(c[0]), ch(c[1]), ch(c[2]))
}

/// WCAG 2 relative luminance.
pub fn luminance(c: Rgb) -> f64 {
    let lin = |v: f64| {
        let s = v / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn rgb_to_hsl(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c[0] / 255.0, c[1] / 255.0, c[2] / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    if s == 0.0 {
        return [l * 255.0; 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |t: f64| {
        let mut x = t;
        if x < 0.0 {
            x += 1.0;
        }
        if x > 1.0 {
            x -= 1.0;
        }
        if x < 1.0 / 6.0 {
            p + (q - p) * 6.0 * x
        } else if x < 0.5 {
            q
        } else if x < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - x) * 6.0
        } else {
            p
        }
    };
    let hn = h / 360.0;
    [
        hue(hn + 1.0 / 3.0) * 255.0,
        hue(hn) * 255.0,
        hue(hn - 1.0 / 3.0) * 255.0,
    ]
}

fn with_lightness(c: Rgb, l: f64) -> Rgb {
    let (h, s, _) = rgb_to_hsl(c);
    let [r, g, b] = hsl_to_rgb(h, s, l.clamp(0.0, 1.0));
    [r.round(), g.round(), b.round()]
}

/// The closest colour of the same hue whose contrast with `against` reaches `target`.
pub fn ensure_contrast(c: Rgb, against: Rgb, target: f64, darken: bool) -> Rgb {
    if contrast(c, against) >= target {
        return c;
    }
    let start = rgb_to_hsl(c).2;
    let (mut lo, mut hi) = if darken { (0.0, start) } else { (start, 1.0) };
    let mut best: Rgb = if darken { [0.0; 3] } else { [255.0; 3] };
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        let cand = with_lightness(c, mid);
        if contrast(cand, against) >= target {
            best = cand;
            if darken {
                lo = mid;
            } else {
                hi = mid;
            }
        } else if darken {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    best
}

// ---------------------------------------------------------------- theme.ts

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn parse(v: Option<&str>) -> Self {
        match v {
            Some("light") => ThemeMode::Light,
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::System,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    Light,
    Dark,
}

impl Resolved {
    pub fn as_str(self) -> &'static str {
        match self {
            Resolved::Light => "light",
            Resolved::Dark => "dark",
        }
    }
}

pub const ACCENT_PRESETS: [(&str, &str, &str); 10] = [
    ("red", "#fa2d48", "#fa2d48"),
    ("pink", "#ff2d55", "#ff375f"),
    ("orange", "#ff9500", "#ff9f0a"),
    ("yellow", "#ffcc00", "#ffd60a"),
    ("green", "#34c759", "#30d158"),
    ("teal", "#30b0c7", "#40c8e0"),
    ("blue", "#007aff", "#0a84ff"),
    ("indigo", "#5856d6", "#5e5ce6"),
    ("purple", "#af52de", "#bf5af2"),
    ("graphite", "#8e8e93", "#8e8e93"),
];
pub const DEFAULT_ACCENT: &str = "red";

pub mod keys {
    pub const MODE: &str = "orphyn.theme.mode";
    pub const ACCENT: &str = "orphyn.theme.accent";
    pub const FOLLOW_ARTWORK: &str = "orphyn.theme.followArtwork";
    pub const ARTWORK_COLOR: &str = "orphyn.theme.artworkColor";
    pub const STYLE: &str = "orphyn.interface.style";
    pub const REDUCE_TRANSPARENCY: &str = "orphyn.interface.reduceTransparency";
    pub const APPEARANCE: &str = "orphyn.interface.appearance";
}

const AA: f64 = 4.5;

fn is_hex(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && parse_hex(v).is_some()
}

pub fn is_accent(v: &str) -> bool {
    ACCENT_PRESETS.iter().any(|(n, _, _)| *n == v) || is_hex(v)
}

pub fn accent_hex(accent: &str, theme: Resolved) -> String {
    if is_hex(accent) {
        return accent.to_string();
    }
    let (_, light, dark) = ACCENT_PRESETS
        .iter()
        .find(|(n, _, _)| *n == accent)
        .unwrap_or(&ACCENT_PRESETS[0]);
    match theme {
        Resolved::Light => light,
        Resolved::Dark => dark,
    }
    .to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccentTokens {
    pub accent: String,
    pub accent_contrast: String,
    pub fill: String,
    pub fill_contrast: String,
}

/// `accentTokens`: accessible tokens for a colour in a theme (hue kept, lightness moved for AA).
pub fn accent_tokens(color: &str, theme: Resolved) -> AccentTokens {
    let rgb = parse_hex(color).unwrap_or_else(|| parse_hex(&accent_hex("red", theme)).unwrap());
    let (surface, window): (Rgb, Rgb) = match theme {
        Resolved::Light => ([229.0, 229.0, 234.0], [255.0; 3]),
        Resolved::Dark => ([58.0, 58.0, 60.0], [29.0, 29.0, 31.0]),
    };
    let (white, black) = ([255.0; 3], [0.0; 3]);
    let light = theme == Resolved::Light;
    let text = ensure_contrast(rgb, surface, AA, light);
    let text_contrast = if contrast(text, white) >= contrast(text, black) {
        white
    } else {
        black
    };
    let visible = ensure_contrast(rgb, window, 3.0, light);
    let bright = luminance(rgb) > 0.5;
    let fill = if bright {
        ensure_contrast(visible, black, AA, false)
    } else {
        ensure_contrast(visible, white, AA, true)
    };
    AccentTokens {
        accent: to_hex(text),
        accent_contrast: to_hex(text_contrast),
        fill: to_hex(fill),
        fill_contrast: if bright { "#000000" } else { "#ffffff" }.into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeState {
    pub mode: ThemeMode,
    pub accent: String,
    pub follow_artwork: bool,
    pub artwork_color: Option<String>,
}

impl ThemeState {
    pub fn load(storage: &LocalStorage) -> Self {
        let accent = storage
            .get(keys::ACCENT)
            .filter(|a| is_accent(a))
            .unwrap_or_else(|| DEFAULT_ACCENT.into());
        Self {
            mode: ThemeMode::parse(storage.get(keys::MODE).as_deref()),
            accent,
            follow_artwork: storage.get(keys::FOLLOW_ARTWORK).as_deref() == Some("true"),
            artwork_color: storage
                .get(keys::ARTWORK_COLOR)
                .filter(|c| is_hex(c))
                .map(|c| c.to_lowercase()),
        }
    }

    pub fn resolve(&self, os_dark: bool) -> Resolved {
        match self.mode {
            ThemeMode::Light => Resolved::Light,
            ThemeMode::Dark => Resolved::Dark,
            ThemeMode::System if os_dark => Resolved::Dark,
            ThemeMode::System => Resolved::Light,
        }
    }
}

// ---------------------------------------------------------------- appearance.ts / interfaceStyle.ts

pub const INTERFACE_STYLES: [&str; 5] = ["classic", "glass", "cinema", "minimal", "compact"];
pub const DEFAULT_INTERFACE_STYLE: &str = "classic";

const CORNERS: [&str; 3] = ["sharp", "standard", "round"];
const DENSITY: [&str; 3] = ["comfortable", "standard", "compact"];
const TEXT_SIZE: [&str; 4] = ["s", "m", "l", "xl"];
const SIDEBAR: [&str; 3] = ["expanded", "icons", "auto-hide"];
const BACKDROP: [&str; 3] = ["off", "subtle", "immersive"];
const MOTION: [&str; 2] = ["full", "reduced"];
const VIDEO_CARDS: [&str; 2] = ["poster", "landscape"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub transparency: u8,
    pub corners: &'static str,
    pub density: &'static str,
    pub text_size: &'static str,
    pub sidebar: &'static str,
    pub backdrop: &'static str,
    pub motion: &'static str,
    pub video_cards: &'static str,
}

const BASE: Appearance = Appearance {
    transparency: 100,
    corners: "standard",
    density: "standard",
    text_size: "m",
    sidebar: "expanded",
    backdrop: "off",
    motion: "full",
    video_cards: "poster",
};

/// `STYLE_DEFAULTS`: what each style looks like before the user changes anything.
pub fn style_defaults(style: &str) -> Appearance {
    match style {
        "cinema" => Appearance {
            sidebar: "icons",
            backdrop: "immersive",
            ..BASE
        },
        "minimal" => Appearance {
            transparency: 0,
            ..BASE
        },
        _ => BASE,
    }
}

fn pick(o: &serde_json::Map<String, Value>, key: &str, choices: &[&'static str]) -> Option<&'static str> {
    let v = o.get(key)?.as_str()?;
    choices.iter().copied().find(|c| *c == v)
}

/// The interface style plus the appearance options the user changed (`sanitizeOverrides`).
#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceState {
    pub style: &'static str,
    /// The stored overrides object, kept as is so unknown fields survive a round trip.
    pub overrides: serde_json::Map<String, Value>,
}

impl InterfaceState {
    pub fn load(storage: &LocalStorage) -> Self {
        let style = storage
            .get(keys::STYLE)
            .and_then(|s| INTERFACE_STYLES.iter().copied().find(|x| *x == s))
            .unwrap_or(DEFAULT_INTERFACE_STYLE);
        let mut overrides = storage
            .get_json::<Value>(keys::APPEARANCE)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        if !overrides.contains_key("transparency")
            && storage.get(keys::REDUCE_TRANSPARENCY).as_deref() == Some("true")
        {
            overrides.insert("transparency".into(), Value::from(0));
        }
        Self { style, overrides }
    }

    pub fn appearance(&self) -> Appearance {
        let d = style_defaults(self.style);
        let o = &self.overrides;
        Appearance {
            transparency: o
                .get("transparency")
                .and_then(Value::as_f64)
                .filter(|t| t.is_finite())
                .map(|t| t.clamp(0.0, 100.0).round() as u8)
                .unwrap_or(d.transparency),
            corners: pick(o, "corners", &CORNERS).unwrap_or(d.corners),
            density: pick(o, "density", &DENSITY).unwrap_or(d.density),
            text_size: pick(o, "textSize", &TEXT_SIZE).unwrap_or(d.text_size),
            sidebar: pick(o, "sidebar", &SIDEBAR).unwrap_or(d.sidebar),
            backdrop: pick(o, "backdrop", &BACKDROP).unwrap_or(d.backdrop),
            motion: pick(o, "motion", &MOTION).unwrap_or(d.motion),
            video_cards: pick(o, "videoCards", &VIDEO_CARDS).unwrap_or(d.video_cards),
        }
    }

    pub fn save(&self, storage: &LocalStorage) {
        storage.set(keys::STYLE, self.style);
        storage.set_json(keys::APPEARANCE, &Value::Object(self.overrides.clone()));
        storage.remove(keys::REDUCE_TRANSPARENCY);
    }
}

// ---------------------------------------------------------------- the root element

/// Window facts the CSS reads (`applyWindowStateToDocument`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowFacts {
    pub platform: &'static str,
    pub material: &'static str,
    pub fullscreen: bool,
    pub focused: bool,
}

/// Everything `.jr-root` carries: `data-*` attributes and the inline CSS variables.
#[derive(Clone, Debug, PartialEq)]
pub struct RootTokens {
    pub attributes: Vec<(&'static str, String)>,
    pub style: String,
}

pub fn root_tokens(
    theme: &ThemeState,
    resolved: Resolved,
    iface: &InterfaceState,
    win: WindowFacts,
) -> RootTokens {
    let a = iface.appearance();
    let solid = a.transparency == 0;
    let mut attributes: Vec<(&'static str, String)> = vec![
        ("data-theme", resolved.as_str().into()),
        ("data-style", iface.style.into()),
        ("data-corners", a.corners.into()),
        ("data-density", a.density.into()),
        ("data-text-size", a.text_size.into()),
        ("data-sidebar", a.sidebar.into()),
        ("data-backdrop", a.backdrop.into()),
        ("data-motion", a.motion.into()),
        ("data-video-cards", a.video_cards.into()),
        (
            "data-transparency",
            if solid {
                "solid"
            } else if a.transparency >= 100 {
                "full"
            } else {
                "partial"
            }
            .into(),
        ),
        ("data-platform", win.platform.into()),
        ("data-material", win.material.into()),
        ("data-fullscreen", win.fullscreen.to_string()),
        ("data-window-focused", win.focused.to_string()),
    ];
    if solid {
        attributes.push(("data-reduce-transparency", "true".into()));
    }
    if !is_hex(&theme.accent) {
        attributes.push(("data-accent", theme.accent.clone()));
    }
    let artwork = theme
        .artwork_color
        .as_ref()
        .filter(|_| theme.follow_artwork);
    if artwork.is_some() {
        attributes.push(("data-accent-source", "artwork".into()));
    }
    let (radius, text) = (
        match a.corners {
            "sharp" => 0.35,
            "round" => 1.45,
            _ => 1.0,
        },
        match a.text_size {
            "s" => 0.92,
            "l" => 1.1,
            "xl" => 1.22,
            _ => 1.0,
        },
    );
    let (space, rows, cards, items) = match a.density {
        "comfortable" => (1.25, 1.15, 1.12, -1),
        "compact" => (0.75, 0.85, 0.86, 1),
        _ => (1.0, 1.0, 1.0, 0),
    };
    let mut style = format!(
        "--transparency: {}; --radius-scale: {radius}; --text-scale: {text}; --density: {space}; \
         --density-rows: {rows}; --density-cards: {cards}; --density-items: {items};",
        a.transparency as f64 / 100.0
    );
    for t in [Resolved::Light, Resolved::Dark] {
        let color = artwork
            .cloned()
            .unwrap_or_else(|| accent_hex(&theme.accent, t));
        let k = accent_tokens(&color, t);
        let n = t.as_str();
        style.push_str(&format!(
            " --accent-{n}: {}; --accent-contrast-{n}: {}; --accent-fill-{n}: {}; --accent-fill-contrast-{n}: {};",
            k.accent, k.accent_contrast, k.fill, k.fill_contrast
        ));
    }
    RootTokens { attributes, style }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_tokens_meet_aa() {
        for (name, _, _) in ACCENT_PRESETS {
            for t in [Resolved::Light, Resolved::Dark] {
                let k = accent_tokens(&accent_hex(name, t), t);
                let fill = parse_hex(&k.fill).unwrap();
                let on = parse_hex(&k.fill_contrast).unwrap();
                assert!(contrast(fill, on) >= AA - 0.01, "{name} {t:?}");
            }
        }
        // The default red keeps its hue and reads on the dark surface.
        let k = accent_tokens("#fa2d48", Resolved::Dark);
        assert!(contrast(parse_hex(&k.accent).unwrap(), [58.0, 58.0, 60.0]) >= AA - 0.01);
    }

    #[test]
    fn stored_values_are_validated_like_the_react_stores() {
        let s = LocalStorage::in_memory();
        s.set(keys::MODE, "dark");
        s.set(keys::ACCENT, "#12ab34");
        s.set(keys::STYLE, "cinema");
        s.set(keys::APPEARANCE, r#"{"corners":"round","density":"nope","transparency":140}"#);
        let theme = ThemeState::load(&s);
        assert_eq!(theme.mode, ThemeMode::Dark);
        assert_eq!(theme.accent, "#12ab34");
        let iface = InterfaceState::load(&s);
        let a = iface.appearance();
        assert_eq!((a.corners, a.density, a.transparency), ("round", "standard", 100));
        assert_eq!((a.sidebar, a.backdrop), ("icons", "immersive"));
        let tokens = root_tokens(
            &theme,
            Resolved::Dark,
            &iface,
            WindowFacts {
                platform: "win32",
                material: "none",
                fullscreen: false,
                focused: true,
            },
        );
        assert!(tokens.attributes.contains(&("data-style", "cinema".into())));
        assert!(!tokens.attributes.iter().any(|(k, _)| *k == "data-accent"));
        assert!(tokens.style.contains("--radius-scale: 1.45"));
    }
}
