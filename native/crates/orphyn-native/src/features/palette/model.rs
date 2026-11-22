//! The palette without UI: the command list (`commands.ts` + `registry.ts`), command search, the
//! layout of sections (`model.ts`), keyboard navigation (`navReducer.ts`) and recent commands
//! (`recents.ts`).
use orphyn_core::storage::LocalStorage;
use orphyn_jellyfin::BaseItemDto;

use super::fuzzy::{fuzzy_match, rank};

// ------------------------------------------------------------------------------------ commands

/// Section a command is listed under; the order is the display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Playback,
    Navigate,
    Actions,
    Appearance,
    Servers,
}

pub const COMMAND_GROUPS: [Group; 5] = [
    Group::Playback,
    Group::Navigate,
    Group::Actions,
    Group::Appearance,
    Group::Servers,
];

impl Group {
    pub fn key(self) -> &'static str {
        match self {
            Group::Playback => "playback",
            Group::Navigate => "navigate",
            Group::Actions => "actions",
            Group::Appearance => "appearance",
            Group::Servers => "servers",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    fn key(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub active: bool,
}

/// What commands may read (`PaletteContext`), built from the live app state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub has_track: bool,
    pub playing: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub favorite_available: bool,
    pub favorite: bool,
    pub dark: bool,
    pub style: String,
    pub styles: Vec<String>,
    pub accounts: Vec<AccountInfo>,
    pub signed_in: bool,
}

/// What running a command does; the host maps each to the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Navigate(String),
    TogglePlay,
    Next,
    Previous,
    ToggleShuffle,
    CycleRepeat,
    NowPlaying,
    ToggleFavorite,
    Refresh,
    SignOut,
    SetDark(bool),
    SetStyle(String),
    SwitchServer(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Stable id (kebab-case); also the i18n key under `commands.` and the recents key.
    pub id: String,
    pub group: Group,
    /// Key under `commands.` in palette.json (may depend on the state: "Pause" vs "Play").
    pub label_key: String,
    /// Literal name of commands generated from data (a server's name).
    pub title: Option<String>,
    pub icon: &'static str,
    /// Extra search terms shared by both languages.
    pub keywords: Vec<&'static str>,
    pub shortcut: Option<&'static str>,
    pub action: Action,
}

fn cmd(
    id: &str,
    group: Group,
    label_key: &str,
    icon: &'static str,
    keywords: &[&'static str],
    action: Action,
) -> Command {
    Command {
        id: id.into(),
        group,
        label_key: label_key.into(),
        title: None,
        icon,
        keywords: keywords.to_vec(),
        shortcut: None,
        action,
    }
}

fn page(id: &str, icon: &'static str, to: &str, keywords: &[&'static str]) -> Command {
    let key = format!("go-{id}");
    cmd(
        &key,
        Group::Navigate,
        &key,
        icon,
        keywords,
        Action::Navigate(to.into()),
    )
}

/// The commands available in `ctx`, in display order. Not offered natively: Picture in Picture,
/// update check (no such feature in the native app) and "add to playlist" (no picker yet).
pub fn commands(ctx: &Context) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    if ctx.has_track {
        let mut play = cmd(
            "play-pause",
            Group::Playback,
            if ctx.playing { "pause" } else { "play" },
            if ctx.playing { "pause" } else { "play" },
            &["play", "pause", "lecture", "toggle"],
            Action::TogglePlay,
        );
        play.shortcut = Some("Space");
        out.push(play);
        out.push(cmd(
            "next",
            Group::Playback,
            "next",
            "skip-forward",
            &["skip", "suivant"],
            Action::Next,
        ));
        out.push(cmd(
            "previous",
            Group::Playback,
            "previous",
            "skip-back",
            &["back", "precedent"],
            Action::Previous,
        ));
        out.push(cmd(
            "shuffle",
            Group::Playback,
            if ctx.shuffle {
                "shuffle-off"
            } else {
                "shuffle-on"
            },
            "shuffle",
            &["random", "aleatoire"],
            Action::ToggleShuffle,
        ));
        out.push(cmd(
            "repeat",
            Group::Playback,
            &format!("repeat-{}", ctx.repeat.key()),
            "repeat",
            &["loop", "boucle"],
            Action::CycleRepeat,
        ));
        out.push(cmd(
            "now-playing",
            Group::Playback,
            "now-playing",
            "disc-3",
            &["player", "lecteur", "lyrics", "paroles", "queue", "file"],
            Action::NowPlaying,
        ));
    }
    if ctx.favorite_available {
        out.push(cmd(
            "favorite",
            Group::Playback,
            if ctx.favorite {
                "unfavorite"
            } else {
                "favorite"
            },
            "heart",
            &["like", "heart", "coeur", "favori", "favourite"],
            Action::ToggleFavorite,
        ));
    }

    out.push(page(
        "home",
        "home",
        "/video",
        &["accueil", "watch", "video"],
    ));
    out.push(page(
        "movies",
        "film",
        "/video/library/movies",
        &["films", "cinema"],
    ));
    out.push(page(
        "series",
        "tv",
        "/video/library/series",
        &["tv", "shows", "series tv", "emissions"],
    ));
    out.push(page(
        "watchlist",
        "clock",
        "/video/watchlist",
        &["a regarder", "watchlist", "later", "plus tard", "signet", "bookmark"],
    ));
    out.push(page("music", "music", "/music", &["musique"]));
    out.push(page("albums", "disc", "/music/albums", &[]));
    out.push(page("artists", "user", "/music/artists", &["artistes"]));
    out.push(page(
        "playlists",
        "list-music",
        "/music/playlists",
        &["listes de lecture"],
    ));
    out.push(page(
        "songs",
        "music",
        "/music/songs",
        &["titres", "tracks", "morceaux", "chansons"],
    ));
    out.push(page(
        "favorites",
        "star",
        "/music/favorites",
        &["favoris", "likes", "liked"],
    ));
    out.push(page(
        "search",
        "search",
        "/search",
        &["recherche", "find", "chercher"],
    ));
    out.push(page(
        "stats",
        "bar-chart-3",
        "/stats",
        &[
            "stats",
            "statistiques",
            "wrapped",
            "year",
            "annee",
            "bilan",
            "recap",
        ],
    ));
    out.push(cmd(
        "customise-home",
        Group::Navigate,
        "customise-home",
        "layout-dashboard",
        &[
            "home screen",
            "personnaliser accueil",
            "rows",
            "rangees",
            "layout",
            "edit home",
        ],
        Action::Navigate("/settings?pane=home".into()),
    ));
    out.push(page(
        "settings",
        "settings",
        "/settings",
        &["preferences", "reglages", "options", "parametres"],
    ));

    let mut refresh = cmd(
        "refresh",
        Group::Actions,
        "refresh",
        "refresh-cw",
        &["reload", "recharger", "actualiser", "sync"],
        Action::Refresh,
    );
    refresh.shortcut = Some("R");
    out.push(refresh);
    if ctx.signed_in {
        out.push(cmd(
            "sign-out",
            Group::Actions,
            "sign-out",
            "log-out",
            &["logout", "log out", "deconnexion", "se deconnecter"],
            Action::SignOut,
        ));
    }

    out.push(cmd(
        "toggle-theme",
        Group::Appearance,
        if ctx.dark {
            "theme-light"
        } else {
            "theme-dark"
        },
        if ctx.dark { "sun" } else { "moon" },
        &[
            "dark",
            "light",
            "sombre",
            "clair",
            "theme",
            "mode",
            "apparence",
        ],
        Action::SetDark(!ctx.dark),
    ));
    for style in ctx.styles.iter().filter(|s| **s != ctx.style) {
        out.push(cmd(
            &format!("style-{style}"),
            Group::Appearance,
            &format!("style-{style}"),
            "palette",
            &["style", "interface", "apparence", "skin"],
            Action::SetStyle(style.clone()),
        ));
    }
    for a in ctx.accounts.iter().filter(|a| !a.active) {
        let mut c = cmd(
            &format!("server-{}", a.id),
            Group::Servers,
            "",
            "server",
            &[
                "switch server",
                "server",
                "changer de serveur",
                "serveur",
                "compte",
                "account",
            ],
            Action::SwitchServer(a.id.clone()),
        );
        c.title = Some(a.name.clone());
        out.push(c);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    fn other(self) -> Lang {
        match self {
            Lang::Fr => Lang::En,
            Lang::En => Lang::Fr,
        }
    }
}

/// `translate(lang, key)`: the palette string for a language, or "" when it has none.
pub type Translate<'a> = &'a dyn Fn(Lang, &str) -> String;

/// (title shown, aliases that also match).
pub fn command_names(c: &Command, lang: Lang, translate: Translate) -> (String, Vec<String>) {
    let keywords = || c.keywords.iter().map(|k| k.to_string());
    if let Some(title) = &c.title {
        return (title.clone(), keywords().collect());
    }
    let key = format!("commands.{}", c.label_key);
    let title = Some(translate(lang, &key))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| c.id.clone());
    let aliases = std::iter::once(translate(lang.other(), &key))
        .chain(keywords())
        .filter(|s| !s.is_empty())
        .collect();
    (title, aliases)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandMatch {
    pub command: Command,
    pub title: String,
    /// Highlighted positions in `title`.
    pub indices: Vec<usize>,
    pub score: f64,
}

/// Commands matching `query`, best first. An empty query returns `recent_ids` first (most recent
/// first), then every other command in registration order.
pub fn search_commands(
    commands: &[Command],
    query: &str,
    lang: Lang,
    translate: Translate,
    recent_ids: &[String],
) -> Vec<CommandMatch> {
    #[derive(Clone)]
    struct Named {
        command: Command,
        title: String,
        aliases: Vec<String>,
    }
    let named: Vec<Named> = commands
        .iter()
        .map(|c| {
            let (title, aliases) = command_names(c, lang, translate);
            Named {
                command: c.clone(),
                title,
                aliases,
            }
        })
        .collect();
    if query.trim().is_empty() {
        let pos = |id: &str| {
            recent_ids
                .iter()
                .position(|r| r == id)
                .unwrap_or(usize::MAX)
        };
        let mut v: Vec<(usize, Named)> = named.into_iter().enumerate().collect();
        v.sort_by(|a, b| {
            pos(&a.1.command.id)
                .cmp(&pos(&b.1.command.id))
                .then(a.0.cmp(&b.0))
        });
        return v
            .into_iter()
            .map(|(_, n)| CommandMatch {
                command: n.command,
                title: n.title,
                indices: Vec::new(),
                score: 0.0,
            })
            .collect();
    }
    rank(&named, query, |n| {
        std::iter::once(n.title.clone())
            .chain(n.aliases.iter().cloned())
            .collect()
    })
    .into_iter()
    .map(|r| CommandMatch {
        command: r.item.command,
        title: r.item.title,
        indices: r.indices,
        score: r.score,
    })
    .collect()
}

// -------------------------------------------------------------------------------------- model

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryKind {
    Movie,
    Series,
    Episode,
    Artist,
    Album,
    Song,
    Playlist,
}

impl LibraryKind {
    pub fn key(self) -> &'static str {
        match self {
            LibraryKind::Movie => "movie",
            LibraryKind::Series => "series",
            LibraryKind::Episode => "episode",
            LibraryKind::Artist => "artist",
            LibraryKind::Album => "album",
            LibraryKind::Song => "song",
            LibraryKind::Playlist => "playlist",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            LibraryKind::Movie => "film",
            LibraryKind::Series | LibraryKind::Episode => "tv",
            LibraryKind::Artist => "user",
            LibraryKind::Album => "disc-3",
            LibraryKind::Song => "music",
            LibraryKind::Playlist => "list-music",
        }
    }

    /// CSS ratio suffix of the thumbnail (`jr-palette__thumb--2x3`) and the aspect ratio.
    pub fn ratio(self) -> (&'static str, &'static str) {
        match self {
            LibraryKind::Movie | LibraryKind::Series => ("2x3", "2 / 3"),
            LibraryKind::Episode => ("16x9", "16 / 9"),
            _ => ("1x1", "1 / 1"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibrarySection {
    pub id: &'static str,
    pub kind: LibraryKind,
    pub items: Vec<BaseItemDto>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Command {
        key: String,
        matched: CommandMatch,
    },
    Item {
        key: String,
        kind: LibraryKind,
        item: BaseItemDto,
        /// Highlighted positions in the item's name.
        indices: Vec<usize>,
    },
}

impl Entry {
    pub fn key(&self) -> &str {
        match self {
            Entry::Command { key, .. } | Entry::Item { key, .. } => key,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// `recent`, `group:<group>`, `commands` or `library:<id>`.
    pub id: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub sections: Vec<Section>,
    /// Every entry in display order (the keyboard moves through this list).
    pub flat: Vec<Entry>,
    /// Index in `flat` where each section starts (Tab jumps between them).
    pub group_starts: Vec<usize>,
}

const MAX_COMMANDS: usize = 6;
const MAX_PER_LIBRARY_SECTION: usize = 4;
/// A command scoring at least this well (a clear name match) is listed above library results.
pub const STRONG_COMMAND_SCORE: f64 = 60.0;

pub struct BuildInput<'a> {
    pub query: &'a str,
    pub commands: &'a [CommandMatch],
    pub recent_ids: &'a [String],
    pub library: &'a [LibrarySection],
    /// Highlights for library names.
    pub highlight: &'a dyn Fn(&str) -> Vec<usize>,
}

/// Lays the palette out. Empty query: recently used commands, then every command by group. With a
/// query: the matching commands and the library hits; the commands lead when one matches clearly.
pub fn build_model(input: &BuildInput) -> Model {
    let command_entry = |m: &CommandMatch| Entry::Command {
        key: format!("cmd:{}", m.command.id),
        matched: m.clone(),
    };
    let library_sections: Vec<Section> = input
        .library
        .iter()
        .filter(|s| !s.items.is_empty())
        .map(|s| Section {
            id: format!("library:{}", s.id),
            entries: s
                .items
                .iter()
                .take(MAX_PER_LIBRARY_SECTION)
                .map(|item| Entry::Item {
                    key: format!(
                        "item:{}:{}",
                        s.kind.key(),
                        if item.id.is_empty() {
                            item.name()
                        } else {
                            &item.id
                        }
                    ),
                    kind: s.kind,
                    item: item.clone(),
                    indices: (input.highlight)(item.name()),
                })
                .collect(),
        })
        .collect();

    let mut sections: Vec<Section> = Vec::new();
    if !input.query.trim().is_empty() {
        let top: Vec<&CommandMatch> = input.commands.iter().take(MAX_COMMANDS).collect();
        let command_section = (!top.is_empty()).then(|| Section {
            id: "commands".into(),
            entries: top.iter().map(|m| command_entry(m)).collect(),
        });
        let leads = library_sections.is_empty()
            || top.first().map(|m| m.score).unwrap_or(0.0) >= STRONG_COMMAND_SCORE;
        if leads {
            sections.extend(command_section);
            sections.extend(library_sections);
        } else {
            sections.extend(library_sections);
            sections.extend(command_section);
        }
    } else {
        let recent: Vec<&CommandMatch> = input
            .recent_ids
            .iter()
            .filter_map(|id| input.commands.iter().find(|m| &m.command.id == id))
            .collect();
        if !recent.is_empty() {
            sections.push(Section {
                id: "recent".into(),
                entries: recent.iter().map(|m| command_entry(m)).collect(),
            });
        }
        for group in COMMAND_GROUPS {
            let entries: Vec<Entry> = input
                .commands
                .iter()
                .filter(|m| {
                    m.command.group == group && !recent.iter().any(|r| r.command.id == m.command.id)
                })
                .map(command_entry)
                .collect();
            if !entries.is_empty() {
                sections.push(Section {
                    id: format!("group:{}", group.key()),
                    entries,
                });
            }
        }
    }

    let flat: Vec<Entry> = sections.iter().flat_map(|s| s.entries.clone()).collect();
    let mut group_starts = Vec::new();
    let mut at = 0;
    for s in &sections {
        group_starts.push(at);
        at += s.entries.len();
    }
    Model {
        sections,
        flat,
        group_starts,
    }
}

/// App route that opens a library result.
pub fn open_route(kind: LibraryKind, item: &BaseItemDto) -> String {
    let id = &item.id;
    match kind {
        LibraryKind::Movie | LibraryKind::Series | LibraryKind::Episode => {
            format!("/video/item/{id}")
        }
        LibraryKind::Artist => format!("/music/artist/{id}"),
        LibraryKind::Album => format!("/music/album/{id}"),
        LibraryKind::Playlist => format!("/music/playlist/{id}"),
        LibraryKind::Song => format!(
            "/music/album/{}",
            item.album_id
                .as_deref()
                .filter(|a| !a.is_empty())
                .unwrap_or(id)
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayAction {
    Video(String),
    Audio(String),
}

/// What Ctrl+Enter does on a library result: play it. A series has no single thing to play.
pub fn play_action(kind: LibraryKind, item: &BaseItemDto) -> Option<PlayAction> {
    if item.id.is_empty() {
        return None;
    }
    match kind {
        LibraryKind::Movie | LibraryKind::Episode => {
            Some(PlayAction::Video(format!("/video/play/{}", item.id)))
        }
        LibraryKind::Series => None,
        _ => Some(PlayAction::Audio(item.id.clone())),
    }
}

/// Highlight positions of `term` in a library name.
pub fn highlight_of(term: &str, name: &str) -> Vec<usize> {
    fuzzy_match(term, name)
        .map(|m| m.indices)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------- navigation

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Keyboard,
    Pointer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavState {
    pub query: String,
    /// Highlighted row.
    pub index: usize,
    /// Who moved the highlight last: the keyboard scrolls it into view, the pointer does not.
    pub source: Source,
}

impl Default for NavState {
    fn default() -> Self {
        NavState {
            query: String::new(),
            index: 0,
            source: Source::Keyboard,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NavAction {
    SetQuery(String),
    Move {
        delta: isize,
        count: usize,
    },
    CycleGroup {
        direction: isize,
        group_starts: Vec<usize>,
    },
    Hover(usize),
    Clamp(usize),
}

/// Index of the group containing `index` (last start <= index).
pub fn group_of(index: usize, group_starts: &[usize]) -> usize {
    let mut g = 0;
    for (i, start) in group_starts.iter().enumerate() {
        if *start <= index {
            g = i;
        }
    }
    g
}

pub fn nav_reduce(state: &NavState, action: NavAction) -> NavState {
    let mut s = state.clone();
    match action {
        NavAction::SetQuery(q) => {
            if q != state.query {
                s = NavState {
                    query: q,
                    index: 0,
                    source: Source::Keyboard,
                };
            }
        }
        NavAction::Move { delta, count } => {
            s.source = Source::Keyboard;
            if count == 0 {
                s.index = 0;
            } else {
                // Wraps around: Down on the last row goes back to the first.
                s.index = (state.index as isize + delta).rem_euclid(count as isize) as usize;
            }
        }
        NavAction::CycleGroup {
            direction,
            group_starts,
        } => {
            if !group_starts.is_empty() {
                let current = group_of(state.index, &group_starts) as isize;
                let n = group_starts.len() as isize;
                s.index = group_starts[(current + direction + n).rem_euclid(n) as usize];
                s.source = Source::Keyboard;
            }
        }
        NavAction::Hover(i) => {
            if !(i == state.index && state.source == Source::Pointer) {
                s.index = i;
                s.source = Source::Pointer;
            }
        }
        NavAction::Clamp(count) => {
            if state.index >= count.max(1) {
                s.index = count.saturating_sub(1);
            }
        }
    }
    s
}

// ------------------------------------------------------------------------------------ recents

/// Recently used commands (ids), most recent first, in localStorage.
pub const RECENT_COMMANDS_KEY: &str = "orphyn.palette.recent";
pub const MAX_RECENT_COMMANDS: usize = 6;

pub fn push_recent(list: &[String], id: &str) -> Vec<String> {
    let mut v = vec![id.to_string()];
    v.extend(list.iter().filter(|x| *x != id).cloned());
    v.truncate(MAX_RECENT_COMMANDS);
    v
}

pub fn read_recent_commands(storage: &LocalStorage) -> Vec<String> {
    storage
        .get_json::<Vec<serde_json::Value>>(RECENT_COMMANDS_KEY)
        .map(|a| {
            a.into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .take(MAX_RECENT_COMMANDS)
                .collect()
        })
        .unwrap_or_default()
}

pub fn record_recent_command(storage: &LocalStorage, id: &str) -> Vec<String> {
    let next = push_recent(&read_recent_commands(storage), id);
    storage.set_json(RECENT_COMMANDS_KEY, &next);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Context {
        Context {
            dark: true,
            style: "classic".into(),
            styles: ["classic", "glass", "cinema"].map(String::from).to_vec(),
            accounts: vec![
                AccountInfo {
                    id: "a".into(),
                    name: "Home".into(),
                    active: true,
                },
                AccountInfo {
                    id: "b".into(),
                    name: "Friends".into(),
                    active: false,
                },
            ],
            signed_in: true,
            ..Context::default()
        }
    }

    /// A tiny catalogue standing in for the locale files.
    fn translate(lang: Lang, key: &str) -> String {
        let (en, fr) = match key {
            "commands.go-settings" => ("Open Settings", "Ouvrir les réglages"),
            "commands.go-stats" => ("Go to Your year", "Aller à Ton année"),
            "commands.go-home" => ("Go to Home", "Aller à l'accueil"),
            "commands.theme-light" => ("Switch to light theme", "Passer au thème clair"),
            "commands.refresh" => ("Refresh library", "Actualiser la bibliothèque"),
            _ => return String::new(),
        };
        if lang == Lang::Fr { fr } else { en }.to_string()
    }

    fn item(id: &str, name: &str) -> BaseItemDto {
        serde_json::from_value(json!({"Id": id, "Name": name})).unwrap()
    }

    #[test]
    fn commands_follow_the_state() {
        let ids = |c: &Context| commands(c).into_iter().map(|x| x.id).collect::<Vec<_>>();
        let idle = ids(&ctx());
        assert!(!idle.contains(&"play-pause".to_string()));
        assert!(!idle.contains(&"favorite".to_string()));
        assert!(idle.contains(&"go-stats".to_string()));
        assert!(idle.contains(&"server-b".to_string()));
        assert!(
            !idle.contains(&"server-a".to_string()),
            "the active server is not offered"
        );
        assert!(
            !idle.contains(&"style-classic".to_string()),
            "the style in use is not offered"
        );
        assert!(idle.contains(&"style-glass".to_string()));
        let mut playing = ctx();
        playing.has_track = true;
        playing.playing = true;
        playing.favorite_available = true;
        let cmds = commands(&playing);
        let pp = cmds.iter().find(|c| c.id == "play-pause").unwrap();
        assert_eq!((pp.label_key.as_str(), pp.icon), ("pause", "pause"));
        assert_eq!(
            cmds.iter().find(|c| c.id == "toggle-theme").unwrap().action,
            Action::SetDark(false)
        );
        let mut out = ctx();
        out.signed_in = false;
        assert!(!ids(&out).contains(&"sign-out".to_string()));
    }

    #[test]
    fn command_ids_are_unique() {
        let mut c = ctx();
        c.has_track = true;
        c.favorite_available = true;
        let ids: Vec<_> = commands(&c).into_iter().map(|x| x.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len());
    }

    #[test]
    fn searches_in_both_languages_and_by_keyword() {
        let cmds = commands(&ctx());
        let find = |q: &str, lang| {
            search_commands(&cmds, q, lang, &translate, &[])
                .into_iter()
                .map(|m| m.command.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(find("settings", Lang::En)[0], "go-settings");
        // French name while the interface is English, and an English keyword in French.
        assert_eq!(find("reglages", Lang::En)[0], "go-settings");
        assert_eq!(find("preferences", Lang::Fr)[0], "go-settings");
        // Keyword shared by both languages; the server is found by its literal name.
        assert!(find("wrapped", Lang::En).contains(&"go-stats".to_string()));
        assert_eq!(find("friends", Lang::En)[0], "server-b");
        assert!(find("zzzzqq", Lang::En).is_empty());
    }

    #[test]
    fn empty_query_lists_recents_first() {
        let cmds = commands(&ctx());
        let recent = vec!["go-stats".to_string(), "refresh".to_string()];
        let r = search_commands(&cmds, "  ", Lang::En, &translate, &recent);
        assert_eq!(r.len(), cmds.len());
        assert_eq!(r[0].command.id, "go-stats");
        assert_eq!(r[1].command.id, "refresh");
        assert_eq!(
            r[2].command.id, "go-home",
            "others keep their registration order"
        );
    }

    fn model_for(query: &str, library: &[LibrarySection], recent: &[String]) -> Model {
        let cmds = commands(&ctx());
        let matches = search_commands(&cmds, query, Lang::En, &translate, recent);
        build_model(&BuildInput {
            query,
            commands: &matches,
            recent_ids: recent,
            library,
            highlight: &|name| highlight_of(query, name),
        })
    }

    #[test]
    fn empty_query_groups_commands_after_recents() {
        let recent = vec!["go-stats".to_string()];
        let m = model_for("", &[], &recent);
        let ids: Vec<_> = m.sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "recent",
                "group:navigate",
                "group:actions",
                "group:appearance",
                "group:servers"
            ]
        );
        // A recent command is not repeated in its group.
        let nav = &m.sections[1];
        assert!(nav.entries.iter().all(|e| e.key() != "cmd:go-stats"));
        assert_eq!(
            m.group_starts,
            vec![
                0,
                1,
                1 + nav.entries.len(),
                m.group_starts[3],
                m.group_starts[4]
            ]
        );
        assert_eq!(
            m.flat.len(),
            m.sections.iter().map(|s| s.entries.len()).sum::<usize>()
        );
    }

    #[test]
    fn library_hits_lead_unless_a_command_matches_clearly() {
        let library = vec![LibrarySection {
            id: "movies",
            kind: LibraryKind::Movie,
            items: (0..6)
                .map(|i| item(&format!("m{i}"), "Settings of the Heart"))
                .collect(),
        }];
        // "stats" is a keyword of a command, but no title matches strongly: library first.
        let m = model_for("hear", &library, &[]);
        assert_eq!(m.sections[0].id, "library:movies");
        assert_eq!(m.sections[0].entries.len(), MAX_PER_LIBRARY_SECTION);
        // A clear command match goes above the library.
        let m = model_for("open settings", &library, &[]);
        assert_eq!(m.sections[0].id, "commands");
        assert_eq!(m.sections[0].entries[0].key(), "cmd:go-settings");
        // With no library result the commands are the only section.
        let m = model_for("home", &[], &[]);
        assert_eq!(m.sections.len(), 1);
        assert!(m.sections[0].entries.len() <= MAX_COMMANDS);
    }

    #[test]
    fn routes_and_play_actions() {
        let song: BaseItemDto =
            serde_json::from_value(json!({"Id": "s", "AlbumId": "al"})).unwrap();
        assert_eq!(open_route(LibraryKind::Song, &song), "/music/album/al");
        assert_eq!(
            open_route(LibraryKind::Playlist, &song),
            "/music/playlist/s"
        );
        assert_eq!(open_route(LibraryKind::Episode, &song), "/video/item/s");
        assert_eq!(
            play_action(LibraryKind::Movie, &song),
            Some(PlayAction::Video("/video/play/s".into()))
        );
        assert_eq!(play_action(LibraryKind::Series, &song), None);
        assert_eq!(
            play_action(LibraryKind::Album, &song),
            Some(PlayAction::Audio("s".into()))
        );
        assert_eq!(play_action(LibraryKind::Album, &item("", "x")), None);
    }

    #[test]
    fn navigation_wraps_jumps_and_clamps() {
        let s = NavState::default();
        let s = nav_reduce(
            &s,
            NavAction::Move {
                delta: -1,
                count: 5,
            },
        );
        assert_eq!(s.index, 4, "Up on the first row wraps to the last");
        let s = nav_reduce(&s, NavAction::Move { delta: 1, count: 5 });
        assert_eq!(s.index, 0);
        let s = nav_reduce(&s, NavAction::Move { delta: 1, count: 0 });
        assert_eq!(s.index, 0);
        let starts = vec![0, 2, 5];
        let s = nav_reduce(
            &s,
            NavAction::CycleGroup {
                direction: 1,
                group_starts: starts.clone(),
            },
        );
        assert_eq!(s.index, 2);
        let s = nav_reduce(&s, NavAction::Move { delta: 1, count: 8 });
        let s = nav_reduce(
            &s,
            NavAction::CycleGroup {
                direction: 1,
                group_starts: starts.clone(),
            },
        );
        assert_eq!(
            s.index, 5,
            "Tab from inside the second group goes to the third"
        );
        let s = nav_reduce(
            &s,
            NavAction::CycleGroup {
                direction: 1,
                group_starts: starts.clone(),
            },
        );
        assert_eq!(s.index, 0, "and wraps");
        let s = nav_reduce(
            &s,
            NavAction::CycleGroup {
                direction: -1,
                group_starts: starts,
            },
        );
        assert_eq!(s.index, 5);
        let s = nav_reduce(
            &s,
            NavAction::Move {
                delta: -1,
                count: 8,
            },
        );
        assert_eq!(s.index, 4);
        let s = nav_reduce(&s, NavAction::Move { delta: 3, count: 8 });
        assert_eq!(s.index, 7);
        let s = nav_reduce(&s, NavAction::Clamp(3));
        assert_eq!(s.index, 2);
        let s = nav_reduce(&s, NavAction::Hover(1));
        assert_eq!((s.index, s.source), (1, Source::Pointer));
        let s = nav_reduce(&s, NavAction::SetQuery("abc".into()));
        assert_eq!(
            (s.index, s.source, s.query.as_str()),
            (0, Source::Keyboard, "abc")
        );
        let again = nav_reduce(&s, NavAction::SetQuery("abc".into()));
        assert_eq!(again, s);
    }

    #[test]
    fn recents_are_deduplicated_capped_and_persisted() {
        let l = push_recent(&["a".into(), "b".into(), "c".into()], "b");
        assert_eq!(l, ["b", "a", "c"]);
        let many: Vec<String> = (0..6).map(|i| format!("c{i}")).collect();
        let l = push_recent(&many, "new");
        assert_eq!(l.len(), MAX_RECENT_COMMANDS);
        assert_eq!(l[0], "new");
        let storage = LocalStorage::in_memory();
        assert!(read_recent_commands(&storage).is_empty());
        record_recent_command(&storage, "go-stats");
        record_recent_command(&storage, "refresh");
        assert_eq!(read_recent_commands(&storage), ["refresh", "go-stats"]);
    }
}
