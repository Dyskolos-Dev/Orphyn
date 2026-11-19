//! Command palette (`features/palette`): Ctrl/Cmd+K overlay with commands, fuzzy matching and a
//! live search of the library.
//!
//! Mount `PaletteHost {}` once, last in the app root (see the task comment): it provides
//! [`use_palette`] (open / close / toggle from anywhere), listens for the shortcut and draws the
//! panel over the app.
mod fuzzy;
mod model;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use dioxus::html::Key as DKey;
use dioxus::html::NamedKey;
use dioxus::prelude::*;
use dioxus_native::winit::event::{ElementState, WindowEvent};
use dioxus_native::winit::keyboard::{Key as WKey, ModifiersState};
use orphyn_jellyfin::images::{build_image_url, ImageKind, ImageOptions};
use orphyn_jellyfin::keys::{self, stale};
use orphyn_jellyfin::{BaseItemDto, ItemsParams};
use orphyn_player::music::RepeatMode;

use crate::design::{Artwork, Icon, Spinner};
use crate::features::music::{toggle_favorite, use_base_url, use_music, NowPlayingView};
use crate::features::search::{group_results, use_search, ResultKind};
use crate::i18n::{translate, use_i18n, Lang as UiLang};
use crate::motion::{easings, use_tween};
use crate::query::use_query;
use crate::router::use_router;
use crate::state::{use_app, use_services, AppState, Services};
use crate::theme::{Resolved, ThemeMode, INTERFACE_STYLES};
use fuzzy::highlight_runs;
use model::{
    build_model, commands, highlight_of, nav_reduce, open_route, play_action, read_recent_commands,
    record_recent_command, search_commands, AccountInfo, Action, BuildInput, Context, Entry,
    LibraryKind, LibrarySection, Lang, NavAction, NavState, PlayAction, Repeat, Source,
};

const DEBOUNCE_MS: u64 = 180;

/// `usePaletteUi`: whether the palette is showing. Anything can open it with `use_palette().show()`.
#[derive(Clone, Copy, PartialEq)]
pub struct PaletteUi {
    pub open: Signal<bool>,
}

impl PaletteUi {
    pub fn show(mut self) {
        self.open.set(true);
    }
    pub fn hide(mut self) {
        self.open.set(false);
    }
    pub fn toggle(mut self) {
        let v = *self.open.peek();
        self.open.set(!v);
    }
}

pub fn use_palette() -> PaletteUi {
    use_context::<PaletteUi>()
}

/// True for Cmd+K on macOS, Ctrl+K elsewhere.
pub fn is_palette_shortcut(key: &WKey, mods: ModifiersState) -> bool {
    let is_k = matches!(key, WKey::Character(c) if c.eq_ignore_ascii_case("k"));
    if !is_k || mods.shift_key() || mods.alt_key() {
        return false;
    }
    if cfg!(target_os = "macos") {
        mods.meta_key() && !mods.control_key()
    } else {
        mods.control_key() && !mods.meta_key()
    }
}

/// Always mounted by the app root: listens for the shortcut and shows the palette over the app.
#[component]
pub fn PaletteHost() -> Element {
    let ui = use_context_provider(|| PaletteUi { open: Signal::new(false) });
    let mut modifiers = use_signal(ModifiersState::default);
    dioxus_native::use_window_event(move |event, _| match event {
        WindowEvent::ModifiersChanged(m) => modifiers.set(m.state()),
        WindowEvent::KeyboardInput { event, .. }
            if event.state == ElementState::Pressed
                && is_palette_shortcut(&event.logical_key, *modifiers.peek()) =>
        {
            ui.toggle();
        }
        _ => {}
    });
    if (ui.open)() {
        rsx! { PalettePanel {} }
    } else {
        rsx! {}
    }
}

fn item_subtitle(kind: LibraryKind, item: &BaseItemDto) -> Option<String> {
    let joined = |parts: [String; 2]| {
        let s = parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ");
        (!s.is_empty()).then_some(s)
    };
    match kind {
        LibraryKind::Movie | LibraryKind::Series => item.production_year.map(|y| y.to_string()),
        LibraryKind::Episode => item.series_name.clone(),
        LibraryKind::Album => joined([
            item.album_artist.clone().unwrap_or_default(),
            item.production_year.map(|y| y.to_string()).unwrap_or_default(),
        ]),
        LibraryKind::Song => joined([
            item.album_artist.clone().or_else(|| item.artists.first().cloned()).unwrap_or_default(),
            item.album.clone().unwrap_or_default(),
        ]),
        _ => None,
    }
}

fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    let first = parts.first().and_then(|p| p.chars().next());
    let last = if parts.len() > 1 { parts.last().and_then(|p| p.chars().next()) } else { None };
    first.into_iter().chain(last).flat_map(char::to_uppercase).collect()
}

fn library_kind(k: ResultKind) -> LibraryKind {
    match k {
        ResultKind::Movie => LibraryKind::Movie,
        ResultKind::Series => LibraryKind::Series,
        ResultKind::Episode => LibraryKind::Episode,
        ResultKind::Artist => LibraryKind::Artist,
        ResultKind::Album => LibraryKind::Album,
        ResultKind::Song => LibraryKind::Song,
    }
}

fn model_lang(l: UiLang) -> Lang {
    match l {
        UiLang::Fr => Lang::Fr,
        UiLang::En => Lang::En,
    }
}

fn ui_lang(l: Lang) -> UiLang {
    match l {
        Lang::Fr => UiLang::Fr,
        Lang::En => UiLang::En,
    }
}

/// `translate` of the panel: the palette string, else the owning feature's (Settings > Home),
/// else "".
fn palette_string(lang: Lang, key: &str) -> String {
    let l = ui_lang(lang);
    let own = translate(l, &format!("palette:{key}"), None, &[]);
    if own != key {
        return own;
    }
    let owned_key = key.replacen("commands.", "paletteCommands.", 1);
    let other = translate(l, &format!("settings:{owned_key}"), None, &[]);
    if other == owned_key {
        String::new()
    } else {
        other
    }
}

/// Signs the active account out (forgets it), like Settings > Servers.
fn sign_out(app: AppState, svc: &Services) {
    let Some(id) = app.session.peek().as_ref().map(|s| s.account_id.clone()) else {
        return;
    };
    if let Some(store) = svc.accounts.as_ref() {
        let mut s = store.lock().unwrap_or_else(|p| p.into_inner());
        let _ = s.remove(&id);
    }
    svc.cache.remove(&keys::all(&id));
    let next = svc.active_session();
    app.activate(svc, next);
}

#[component]
fn PalettePanel() -> Element {
    let t = use_i18n();
    let svc = use_services();
    let app = use_app();
    let router = use_router();
    let music = use_music();
    let ui = use_palette();
    let base = use_base_url();
    let lang = model_lang(t.lang());

    // Enter animation: the panel scales and slides in, the backdrop fades.
    let mut shown = use_signal(|| false);
    let p = use_tween(if shown() { 1.0 } else { 0.0 }, 0.18, easings::OUT);
    let mut nav = use_signal(NavState::default);
    let recent_ids = use_signal({
        let storage = svc.storage.clone();
        move || read_recent_commands(&storage)
    });
    let rows: Rc<RefCell<HashMap<String, Rc<MountedData>>>> = use_hook(Default::default);

    // The app state commands read.
    let state = (music.state)();
    let track = state.current().map(|q| q.track.clone());
    let session = (app.session)();
    crate::query::use_cache_updates();
    let favorite = track.as_ref().map(|tr| {
        session
            .as_ref()
            .and_then(|s| svc.cache.peek::<BaseItemDto>(&keys::item(&s.account_id, &tr.id)).data)
            .map(|c| c.is_favorite())
            .unwrap_or_else(|| tr.is_favorite())
    });
    let active_id = session.as_ref().map(|s| s.account_id.clone());
    let theme = app.theme.read().clone();
    let ctx = Context {
        has_track: track.is_some(),
        playing: state.is_playing(),
        shuffle: state.model.shuffle,
        repeat: match state.model.repeat {
            RepeatMode::Off => Repeat::Off,
            RepeatMode::All => Repeat::All,
            RepeatMode::One => Repeat::One,
        },
        favorite_available: track.is_some(),
        favorite: favorite.unwrap_or(false),
        dark: theme.resolve((app.os_dark)()) == Resolved::Dark,
        style: app.interface.read().style.to_string(),
        styles: INTERFACE_STYLES.iter().map(|s| s.to_string()).collect(),
        accounts: (app.accounts)()
            .iter()
            .map(|a| AccountInfo {
                id: a.id.clone(),
                name: a.name.clone(),
                active: Some(&a.id) == active_id.as_ref(),
            })
            .collect(),
        signed_in: session.is_some(),
    };

    // The library search follows the field 180 ms after the last keystroke.
    let query_text = nav.read().query.clone();
    let trimmed = query_text.trim().to_string();
    let mut term = use_signal(String::new);
    let mut generation = use_signal(|| 0u64);
    use_effect(use_reactive!(|trimmed| {
        let g = *generation.peek() + 1;
        generation.set(g);
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
            if *generation.peek() == g {
                term.set(trimmed);
            }
        });
    }));
    let term_now = term();
    let settled = term_now == trimmed;
    let searching = term_now.chars().count() >= 2;
    let (search, _) = use_search(&term_now);
    let acc = active_id.clone().unwrap_or_default();
    let mut playlists_key = vec!["jf".to_string(), acc, "palette".into(), "playlists".into(), term_now.clone()];
    if !searching {
        playlists_key.push("off".into());
    }
    let playlists = use_query(playlists_key, stale::SEARCH, {
        let (session, term_now) = (session.clone(), term_now.clone());
        move || {
            let (session, term_now) = (session.clone().filter(|_| searching), term_now.clone());
            async move {
                match session {
                    Some(s) => {
                        let p = ItemsParams {
                            include_item_types: vec!["Playlist".into()],
                            search_term: Some(term_now),
                            page_size: Some(4),
                            ..ItemsParams::default()
                        };
                        Ok(s.items_page(&p, 0).await?.items)
                    }
                    None => Ok(Vec::new()),
                }
            }
        }
    });

    let mut library: Vec<LibrarySection> = Vec::new();
    if searching && settled {
        if let Some(data) = &search.data {
            library = group_results(data, &term_now)
                .sections
                .into_iter()
                .map(|s| LibrarySection { id: s.id, kind: library_kind(s.kind), items: s.items })
                .collect();
        }
        if let Some(lists) = playlists.data.as_ref().filter(|l| !l.is_empty()) {
            library.push(LibrarySection { id: "playlists", kind: LibraryKind::Playlist, items: (**lists).clone() });
        }
    }

    let all = commands(&ctx);
    let recents_now = recent_ids();
    let translate_fn = |l: Lang, k: &str| palette_string(l, k);
    let matches = search_commands(&all, &query_text, lang, &translate_fn, &recents_now);
    let model = build_model(&BuildInput {
        query: &query_text,
        commands: &matches,
        recent_ids: &recents_now,
        library: &library,
        highlight: &|name| highlight_of(&term_now, name),
    });

    let count = model.flat.len();
    if nav.peek().index >= count.max(1) {
        let next = nav_reduce(&nav.peek(), NavAction::Clamp(count));
        nav.set(next);
    }
    let nav_now = nav();
    let active_entry = model.flat.get(nav_now.index).cloned();
    let option_id = |key: &str| format!("jr-palette-opt-{key}");

    // The keyboard scrolls the highlighted row into view; the pointer never does.
    {
        let (rows, entry, source) = (rows.clone(), active_entry.clone(), nav_now.source);
        use_effect(use_reactive!(|entry, source| {
            if source != Source::Keyboard {
                return;
            }
            if let Some(el) = entry.and_then(|e| rows.borrow().get(e.key()).cloned()) {
                spawn(async move {
                    let _ = el.scroll_to(dioxus::html::ScrollBehavior::Instant).await;
                });
            }
        }));
    }

    let run = {
        let (svc, base_session) = (svc.clone(), session.clone());
        move |entry: Entry, secondary: bool| {
            match entry {
                Entry::Command { matched, .. } => {
                    let cmd = matched.command;
                    let mut recents = recent_ids;
                    recents.set(record_recent_command(&svc.storage, &cmd.id));
                    ui.hide();
                    match cmd.action {
                        Action::Navigate(to) => router.push(&to),
                        Action::TogglePlay => svc.music.toggle(),
                        Action::Next => svc.music.next(),
                        Action::Previous => svc.music.previous(),
                        Action::ToggleShuffle => svc.music.toggle_shuffle(),
                        Action::CycleRepeat => svc.music.cycle_repeat(),
                        Action::NowPlaying => music.show(NowPlayingView::Player),
                        Action::ToggleFavorite => {
                            if let Some(tr) = (music.state)().current().map(|q| q.track.clone()) {
                                let value = base_session
                                    .as_ref()
                                    .and_then(|s| svc.cache.peek::<BaseItemDto>(&keys::item(&s.account_id, &tr.id)).data)
                                    .map(|c| c.is_favorite())
                                    .unwrap_or_else(|| tr.is_favorite());
                                let mut item = (*tr).clone();
                                item.user_data.get_or_insert_with(Default::default).is_favorite = value;
                                toggle_favorite(&svc, base_session.clone(), &item, !value);
                            }
                        }
                        Action::Refresh => {
                            if let Some(s) = &base_session {
                                svc.cache.invalidate(&keys::all(&s.account_id));
                            }
                        }
                        Action::SignOut => sign_out(app, &svc),
                        Action::SetDark(dark) => app.set_theme(&svc, |t| {
                            t.mode = if dark { ThemeMode::Dark } else { ThemeMode::Light };
                        }),
                        Action::SetStyle(style) => {
                            if let Some(s) = INTERFACE_STYLES.iter().copied().find(|s| *s == style) {
                                app.set_style(&svc, s);
                            }
                        }
                        Action::SwitchServer(id) => crate::features::settings::switch_server(app, &svc, &id),
                    }
                }
                Entry::Item { kind, item, .. } => {
                    let play = if secondary { play_action(kind, &item) } else { None };
                    ui.hide();
                    match play {
                        Some(PlayAction::Video(route)) => router.push(&route),
                        Some(PlayAction::Audio(_)) => {
                            svc.music.play(std::slice::from_ref(&item), Some(0), Some(false))
                        }
                        None => router.push(&open_route(kind, &item)),
                    }
                }
            }
        }
    };

    let ctrl_down = use_signal(|| false);
    let run_key = run.clone();
    let on_keydown = {
        let (entry, starts) = (active_entry.clone(), model.group_starts.clone());
        move |e: KeyboardEvent| {
            let mods = e.modifiers();
            let secondary = if cfg!(target_os = "macos") { mods.meta() } else { mods.ctrl() };
            let mut reduce = |a: NavAction| {
                let next = nav_reduce(&nav.peek(), a);
                nav.set(next);
            };
            match e.key() {
                DKey::Named(NamedKey::ArrowDown) => {
                    e.prevent_default();
                    reduce(NavAction::Move { delta: 1, count });
                }
                DKey::Named(NamedKey::ArrowUp) => {
                    e.prevent_default();
                    reduce(NavAction::Move { delta: -1, count });
                }
                DKey::Named(NamedKey::Tab) => {
                    e.prevent_default();
                    reduce(NavAction::CycleGroup {
                        direction: if mods.shift() { -1 } else { 1 },
                        group_starts: starts.clone(),
                    });
                }
                DKey::Named(NamedKey::Enter) => {
                    e.prevent_default();
                    if let Some(en) = entry.clone() {
                        run_key(en, secondary);
                    }
                }
                DKey::Named(NamedKey::Escape) => {
                    e.prevent_default();
                    e.stop_propagation();
                    ui.hide();
                }
                _ => {}
            }
        }
    };
    let _ = ctrl_down;

    let loading = searching && (!settled || search.fetching) && library.is_empty();
    let empty = count == 0 && !loading;
    let can_play = matches!(&active_entry, Some(Entry::Item { kind, item, .. }) if play_action(*kind, item).is_some());
    let mod_key = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    let opacity = p();
    let panel_style = format!(
        "opacity: {opacity}; transform: translateY({}px) scale({});",
        -14.0 * (1.0 - opacity),
        0.95 + 0.05 * opacity
    );
    let backdrop_style = format!("opacity: {opacity};");
    let label = t.t("palette:label");
    let active_id_attr = active_entry.as_ref().map(|e| option_id(e.key()));

    rsx! {
        div { class: "jr-palette-layer", onkeydown: on_keydown,
            div {
                class: "jr-palette-backdrop",
                style: "{backdrop_style}",
                "aria-hidden": "true",
                onmousedown: move |_| ui.hide(),
            }
            div {
                class: "jr-palette jr-material-thick",
                style: "{panel_style}",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "{label}",
                div { class: "jr-palette__search",
                    if loading {
                        Spinner { size: "small" }
                    } else {
                        Icon { name: "search", size: 20 }
                    }
                    input {
                        class: "jr-palette__input",
                        r#type: "text",
                        role: "combobox",
                        "aria-expanded": "true",
                        "aria-controls": "jr-palette-list",
                        "aria-activedescendant": active_id_attr,
                        "aria-label": "{label}",
                        placeholder: t.t("palette:placeholder"),
                        value: "{query_text}",
                        autocomplete: "off",
                        spellcheck: "false",
                        onmounted: move |e| {
                            shown.set(true);
                            let el = e.data();
                            async move { let _ = el.set_focus(true).await; }
                        },
                        oninput: move |e| {
                            let next = nav_reduce(&nav.peek(), NavAction::SetQuery(e.value()));
                            nav.set(next);
                        },
                    }
                }
                div { class: "jr-palette__list", id: "jr-palette-list", role: "listbox",
                    for section in model.sections.iter() {
                        {
                            let (kind, name) = section.id.split_once(':').unwrap_or((section.id.as_str(), ""));
                            let title = if kind == "library" {
                                t.t(&format!("palette:library.{}", library_kind_of(name).key()))
                            } else {
                                t.t(&format!("palette:groups.{}", if kind == "group" { name } else { kind }))
                            };
                            rsx! {
                                div { key: "{section.id}", role: "group", "aria-label": "{title}",
                                    div { class: "jr-palette__header", "aria-hidden": "true", "{title}" }
                                    for entry in section.entries.iter() {
                                        {
                                            let index = model.flat.iter().position(|e| e.key() == entry.key()).unwrap_or(0);
                                            let active = index == nav_now.index;
                                            let row_cls = if active { "jr-palette__row jr-palette__row--active" } else { "jr-palette__row" };
                                            let key = entry.key().to_string();
                                            let (rows_ref, key_ref) = (rows.clone(), key.clone());
                                            let (run_click, entry_click) = (run.clone(), entry.clone());
                                            rsx! {
                                                div {
                                                    key: "{key}",
                                                    id: option_id(&key),
                                                    role: "option",
                                                    "aria-selected": "{active}",
                                                    class: "{row_cls}",
                                                    onmounted: move |e| { rows_ref.borrow_mut().insert(key_ref.clone(), e.data()); },
                                                    onmousemove: move |_| {
                                                        let next = nav_reduce(&nav.peek(), NavAction::Hover(index));
                                                        if next != *nav.peek() { nav.set(next); }
                                                    },
                                                    onclick: move |e: MouseEvent| {
                                                        let m = e.modifiers();
                                                        run_click(entry_click.clone(), m.meta() || m.ctrl());
                                                    },
                                                    match entry {
                                                        Entry::Command { matched, .. } => rsx! {
                                                            span { class: "jr-palette__icon", Icon { name: matched.command.icon, size: 18 } }
                                                            span { class: "jr-palette__title", Highlighted { text: matched.title.clone(), indices: matched.indices.clone() } }
                                                            if let (Some(sc), false) = (matched.command.shortcut, active) {
                                                                kbd { class: "jr-palette__kbd", "{sc}" }
                                                            }
                                                            if active { Icon { name: "corner-down-left", size: 16, class: "jr-palette__enter" } }
                                                        },
                                                        Entry::Item { kind, item, indices, .. } => {
                                                            let kind = *kind;
                                                            let (suffix, ratio) = kind.ratio();
                                                            let round = kind == LibraryKind::Artist;
                                                            let src = build_image_url(&base, item, ImageKind::Primary, &ImageOptions {
                                                                max_width: Some(240), max_height: None, quality: Some(80), tag: None,
                                                            });
                                                            let subtitle = item_subtitle(kind, item);
                                                            let art_shape = if round { "circle" } else { "rounded" };
                                                            rsx! {
                                                                span { class: "jr-palette__thumb jr-palette__thumb--{suffix}",
                                                                    if round && src.is_none() {
                                                                        Artwork { src: None::<String>, ratio, shape: "circle", fallback: kind.icon(), class: "jr-artwork--initials" }
                                                                        span { class: "jr-card__initials", {initials(item.name())} }
                                                                    } else {
                                                                        Artwork { src, ratio, shape: art_shape, fallback: kind.icon() }
                                                                    }
                                                                }
                                                                span { class: "jr-palette__text",
                                                                    span { class: "jr-palette__title", Highlighted { text: item.name().to_string(), indices: indices.clone() } }
                                                                    if let Some(s) = subtitle { span { class: "jr-palette__subtitle", "{s}" } }
                                                                }
                                                                span { class: "jr-palette__kind", {t.t(&format!("palette:kinds.{}", kind.key()))} }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if empty {
                        div { class: "jr-palette__empty",
                            strong { {t.tf("palette:noResults", &[("query", trimmed.clone())])} }
                            span { {t.t("palette:noResultsHint")} }
                        }
                    }
                }
                div { class: "jr-palette__footer", "aria-hidden": "true",
                    span { class: "jr-palette__hint",
                        kbd { Icon { name: "corner-down-left", size: 14 } }
                        {if matches!(active_entry, Some(Entry::Command { .. })) { t.t("palette:hints.run") } else { t.t("palette:hints.open") }}
                    }
                    if can_play {
                        span { class: "jr-palette__hint", kbd { "{mod_key}↵" } {t.t("palette:hints.play")} }
                    }
                    if model.sections.len() > 1 {
                        span { class: "jr-palette__hint", kbd { "Tab" } {t.t("palette:hints.group")} }
                    }
                    span { class: "jr-palette__hint", kbd { "Esc" } {t.t("palette:hints.close")} }
                }
            }
        }
    }
}

fn library_kind_of(section_id: &str) -> LibraryKind {
    match section_id {
        "movies" => LibraryKind::Movie,
        "series" => LibraryKind::Series,
        "episodes" => LibraryKind::Episode,
        "artists" => LibraryKind::Artist,
        "albums" => LibraryKind::Album,
        "songs" => LibraryKind::Song,
        _ => LibraryKind::Playlist,
    }
}

/// Text with the matched letters emphasised.
#[component]
fn Highlighted(text: String, indices: Vec<usize>) -> Element {
    rsx! {
        for (i, (run, hit)) in highlight_runs(&text, &indices).into_iter().enumerate() {
            if hit {
                mark { key: "{i}", class: "jr-palette__mark", "{run}" }
            } else {
                span { key: "{i}", "{run}" }
            }
        }
    }
}

