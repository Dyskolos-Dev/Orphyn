//! Router: the routes of `src/renderer/src/app/routes.tsx` (and each feature's `routes.tsx`), a
//! browser-like history (back / forward, `replace`), and scroll restoration: each history entry
//! remembers the scroll offset of the main scroller and gets it back on back / forward, while a
//! new page starts at the top (like `AppShell`).
//!
//! In components: `let nav = use_router(); nav.push("/music/album/123")`, `nav.back()`,
//! `nav.route()` (reactive).
use dioxus::prelude::*;

/// Every page of the app. Paths and parameters are the React ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    // features/video/routes.tsx
    VideoHome,
    VideoLibrary { library_id: String },
    VideoItem { item_id: String },
    /// `?start=ticks` resumes there; `?trailer=1` plays a trailer (no resume, nothing reported).
    VideoPlay {
        item_id: String,
        start: Option<i64>,
        trailer: bool,
    },
    // features/music/routes.tsx
    MusicHome,
    MusicAlbums,
    MusicArtists,
    MusicPlaylists,
    MusicFavorites,
    MusicSongs,
    MusicAlbum { id: String },
    MusicArtist { id: String },
    MusicPlaylist { id: String },
    // search, settings, servers, cast, stats
    Search { q: String },
    Settings,
    Onboarding,
    Cast,
    Stats,
    NotFound { path: String },
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl Route {
    /// Parses a path like `/music/album/123` or `/search?q=abc`. `/` redirects to `/video`, like
    /// the index route of `routes.tsx`.
    pub fn parse(location: &str) -> Route {
        let (path, query) = location.split_once('?').unwrap_or((location, ""));
        let param = |name: &str| {
            query
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == name)
                .map(|(_, v)| decode(v))
                .unwrap_or_default()
        };
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').skip(1).collect();
        let id = |s: &&str| decode(s);
        match parts.as_slice() {
            [] | [""] => Route::VideoHome,
            ["video"] => Route::VideoHome,
            ["video", "library", x] => Route::VideoLibrary { library_id: id(x) },
            ["video", "item", x] => Route::VideoItem { item_id: id(x) },
            ["video", "play", x] => Route::VideoPlay {
                item_id: id(x),
                start: param("start").parse().ok().filter(|t: &i64| *t >= 0),
                trailer: param("trailer") == "1",
            },
            ["music"] => Route::MusicHome,
            ["music", "albums"] => Route::MusicAlbums,
            ["music", "artists"] => Route::MusicArtists,
            ["music", "playlists"] => Route::MusicPlaylists,
            ["music", "favorites"] => Route::MusicFavorites,
            ["music", "songs"] => Route::MusicSongs,
            ["music", "album", x] => Route::MusicAlbum { id: id(x) },
            ["music", "artist", x] => Route::MusicArtist { id: id(x) },
            ["music", "playlist", x] => Route::MusicPlaylist { id: id(x) },
            ["search"] => Route::Search { q: param("q") },
            ["settings"] => Route::Settings,
            ["onboarding"] => Route::Onboarding,
            ["cast"] => Route::Cast,
            ["stats"] => Route::Stats,
            _ => Route::NotFound {
                path: location.to_string(),
            },
        }
    }

    pub fn path(&self) -> String {
        match self {
            Route::VideoHome => "/video".into(),
            Route::VideoLibrary { library_id } => format!("/video/library/{}", encode(library_id)),
            Route::VideoItem { item_id } => format!("/video/item/{}", encode(item_id)),
            Route::VideoPlay {
                item_id,
                start,
                trailer,
            } => {
                let base = format!("/video/play/{}", encode(item_id));
                match (start, trailer) {
                    (_, true) => format!("{base}?trailer=1"),
                    (Some(t), false) if *t > 0 => format!("{base}?start={t}"),
                    _ => base,
                }
            }
            Route::MusicHome => "/music".into(),
            Route::MusicAlbums => "/music/albums".into(),
            Route::MusicArtists => "/music/artists".into(),
            Route::MusicPlaylists => "/music/playlists".into(),
            Route::MusicFavorites => "/music/favorites".into(),
            Route::MusicSongs => "/music/songs".into(),
            Route::MusicAlbum { id } => format!("/music/album/{}", encode(id)),
            Route::MusicArtist { id } => format!("/music/artist/{}", encode(id)),
            Route::MusicPlaylist { id } => format!("/music/playlist/{}", encode(id)),
            Route::Search { q } => format!("/search?q={}", encode(q)),
            Route::Settings => "/settings".into(),
            Route::Onboarding => "/onboarding".into(),
            Route::Cast => "/cast".into(),
            Route::Stats => "/stats".into(),
            Route::NotFound { path } => path.clone(),
        }
    }

    /// `NavLink` matching of the sidebar items (`end` is on by default there: exact path).
    pub fn is_active_link(&self, to: &str) -> bool {
        let here = self.path();
        here.split('?').next().unwrap_or("") == to
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    route: Route,
    scroll: f64,
}

/// The history stack.
#[derive(Clone, Debug, PartialEq)]
pub struct History {
    entries: Vec<Entry>,
    index: usize,
}

impl History {
    pub fn new(start: Route) -> Self {
        Self {
            entries: vec![Entry {
                route: start,
                scroll: 0.0,
            }],
            index: 0,
        }
    }

    pub fn current(&self) -> &Route {
        &self.entries[self.index].route
    }

    pub fn scroll(&self) -> f64 {
        self.entries[self.index].scroll
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }

    /// `location.key !== 'default'`: the first page has no back button.
    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    fn save_scroll(&mut self, y: f64) {
        self.entries[self.index].scroll = y;
    }

    pub fn push(&mut self, route: Route, scroll_now: f64) {
        if *self.current() == route {
            return;
        }
        self.save_scroll(scroll_now);
        self.entries.truncate(self.index + 1);
        self.entries.push(Entry { route, scroll: 0.0 });
        self.index += 1;
    }

    pub fn replace(&mut self, route: Route) {
        self.entries[self.index] = Entry { route, scroll: 0.0 };
    }

    pub fn back(&mut self, scroll_now: f64) -> bool {
        if !self.can_go_back() {
            return false;
        }
        self.save_scroll(scroll_now);
        self.index -= 1;
        true
    }

    pub fn forward(&mut self, scroll_now: f64) -> bool {
        if !self.can_go_forward() {
            return false;
        }
        self.save_scroll(scroll_now);
        self.index += 1;
        true
    }
}

/// The router handle, provided at the root (`Copy`, reactive through its signals).
#[derive(Clone, Copy, PartialEq)]
pub struct Router {
    pub history: Signal<History>,
    /// Scroll offset of the main scroller, kept up to date by the shell's `onscroll`.
    pub scroll_y: Signal<f64>,
    /// Bumped on every navigation; the shell scrolls to `history.scroll()` when it changes.
    pub nav_id: Signal<u64>,
}

impl Router {
    pub fn new(start: Route) -> Self {
        Self {
            history: Signal::new(History::new(start)),
            scroll_y: Signal::new(0.0),
            nav_id: Signal::new(0),
        }
    }

    pub fn route(&self) -> Route {
        self.history.read().current().clone()
    }

    pub fn navigate(&self, route: Route) {
        let y = *self.scroll_y.peek();
        let mut h = self.history;
        h.write().push(route, y);
        self.bump();
    }

    pub fn push(&self, path: &str) {
        self.navigate(Route::parse(path));
    }

    pub fn replace(&self, path: &str) {
        let mut h = self.history;
        h.write().replace(Route::parse(path));
        self.bump();
    }

    pub fn back(&self) {
        let y = *self.scroll_y.peek();
        let mut h = self.history;
        if h.write().back(y) {
            self.bump();
        }
    }

    pub fn forward(&self) {
        let y = *self.scroll_y.peek();
        let mut h = self.history;
        if h.write().forward(y) {
            self.bump();
        }
    }

    fn bump(&self) {
        let mut n = self.nav_id;
        n += 1;
    }
}

pub fn use_router() -> Router {
    use_context::<Router>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_route_keeps_start_and_trailer() {
        let r = Route::parse("/video/play/abc?start=600");
        assert_eq!(
            r,
            Route::VideoPlay { item_id: "abc".into(), start: Some(600), trailer: false }
        );
        assert_eq!(r.path(), "/video/play/abc?start=600");
        let t = Route::parse("/video/play/abc?trailer=1");
        assert_eq!(t.path(), "/video/play/abc?trailer=1");
        assert_eq!(Route::parse("/video/play/abc").path(), "/video/play/abc");
    }

    #[test]
    fn parses_every_react_route() {
        for path in [
            "/video",
            "/video/library/movies",
            "/video/item/abc",
            "/video/play/abc",
            "/music",
            "/music/albums",
            "/music/artists",
            "/music/playlists",
            "/music/favorites",
            "/music/songs",
            "/music/album/a1",
            "/music/artist/b2",
            "/music/playlist/c3",
            "/search?q=hello%20world",
            "/settings",
            "/onboarding",
            "/cast",
            "/stats",
        ] {
            let r = Route::parse(path);
            assert!(!matches!(r, Route::NotFound { .. }), "{path}");
            assert_eq!(r.path(), path, "{path}");
        }
        assert_eq!(Route::parse("/"), Route::VideoHome);
        assert!(matches!(Route::parse("/nope"), Route::NotFound { .. }));
        assert_eq!(
            Route::parse("/search?q=a+b%C3%A9"),
            Route::Search { q: "a bé".into() }
        );
    }

    #[test]
    fn history_restores_scroll_on_back_and_forward() {
        let mut h = History::new(Route::MusicAlbums);
        h.push(Route::MusicAlbum { id: "1".into() }, 840.0);
        assert_eq!(h.scroll(), 0.0);
        assert!(h.back(120.0));
        assert_eq!(*h.current(), Route::MusicAlbums);
        assert_eq!(h.scroll(), 840.0);
        assert!(h.forward(0.0));
        assert_eq!(h.scroll(), 120.0);
        // A new page drops the forward entries.
        h.back(0.0);
        h.push(Route::Settings, 0.0);
        assert!(!h.can_go_forward());
        assert!(!h.is_first());
    }

    #[test]
    fn sidebar_links_match_like_navlink() {
        let album = Route::MusicAlbum { id: "x".into() };
        assert!(!album.is_active_link("/music"));
        assert!(Route::MusicAlbums.is_active_link("/music/albums"));
        assert!(Route::VideoLibrary {
            library_id: "movies".into()
        }
        .is_active_link("/video/library/movies"));
        assert!(!Route::VideoLibrary {
            library_id: "movies".into()
        }
        .is_active_link("/video"));
    }
}
