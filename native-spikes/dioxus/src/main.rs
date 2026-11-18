//! Orphyn spike A: the music album page, the mini player and the Now Playing sheet, rendered
//! natively by Dioxus (Blitz: Stylo + Taffy + Vello) from the app's real stylesheets.
mod audio;
mod icons;
mod jellyfin;

use audio::{Cmd, Player};
use dioxus::prelude::*;
use jellyfin::{Item, Session};
use std::sync::OnceLock;
use std::time::Duration;

const CSS: &str = include_str!("../assets/orphyn.css");
const OVERRIDES: &str = include_str!("../assets/spike.css");

struct Data {
    session: Session,
    album: Item,
    tracks: Vec<Item>,
}
static DATA: OnceLock<Data> = OnceLock::new();
static PLAYER: OnceLock<Player> = OnceLock::new();

fn load_dotenv() {
    if let Ok(text) = std::fs::read_to_string(".env") {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if std::env::var(k.trim()).is_err() {
                    std::env::set_var(k.trim(), v.trim());
                }
            }
        }
    }
}

fn main() {
    load_dotenv();
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let t0 = std::time::Instant::now();
    let session = Session::login(&var("ORPHYN_E2E_URL"), &var("ORPHYN_E2E_USER"), &var("ORPHYN_E2E_PASSWORD"))
        .expect("jellyfin login");
    let (album, tracks) = session.album(std::env::var("ORPHYN_ALBUM_ID").ok().as_deref()).expect("album");
    eprintln!("[spike] logged in, album '{}' ({} tracks) in {:?}", album.name, tracks.len(), t0.elapsed());
    let _ = DATA.set(Data { session, album, tracks });
    let _ = PLAYER.set(Player::spawn());
    let size = std::env::var("ORPHYN_WIN").ok().and_then(|v| v.split_once('x').map(|(w, h)| (w.parse().unwrap_or(1180.0), h.parse().unwrap_or(740.0)))).unwrap_or((1180.0, 740.0));
    let window = winit::window::WindowAttributes::default()
        .with_title("Orphyn")
        .with_surface_size(winit::dpi::LogicalSize::new(size.0, size.1));
    dioxus_native::launch_cfg(app, vec![], vec![Box::new(window)]);
}

fn fmt(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--:--".into();
    }
    let s = secs as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}

#[component]
fn Icon(name: &'static str, #[props(default = 24)] size: u32, #[props(default = false)] fill: bool) -> Element {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\" fill=\"{}\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
        if fill { "currentColor" } else { "none" },
        icons::icon_markup(name)
    );
    rsx! { span { style: "display:inline-flex", dangerous_inner_html: svg } }
}

#[component]
fn IconButton(
    icon: &'static str,
    label: &'static str,
    #[props(default = "plain")] variant: &'static str,
    #[props(default = "medium")] size: &'static str,
    #[props(default = false)] pressed: bool,
    #[props(default = false)] fill: bool,
    #[props(default = "")] class: &'static str,
    onclick: Option<EventHandler<MouseEvent>>,
) -> Element {
    let pressed_cls = if pressed { " jr-icon-button--pressed" } else { "" };
    rsx! {
        button {
            r#type: "button",
            title: label,
            "aria-label": label,
            "aria-pressed": if pressed { "true" } else { "false" },
            class: "jr-icon-button jr-icon-button--{variant} jr-icon-button--{size}{pressed_cls} {class}",
            onclick: move |e| if let Some(h) = &onclick { h.call(e) },
            span { class: "jr-icon-button__icon", Icon { name: icon, size: 18, fill } }
        }
    }
}

/// Click / drag slider (same DOM as the app's Slider).
#[component]
fn Slider(value: f64, max: f64, onseek: EventHandler<f64>, #[props(default = "")] class: &'static str) -> Element {
    let mut width = use_signal(|| 0.0f64);
    let pct = if max > 0.0 { (value / max * 100.0).clamp(0.0, 100.0) } else { 0.0 };
    rsx! {
        div {
            role: "slider",
            class: "jr-slider jr-slider--default {class}",
            style: "--jr-slider-value: {pct}%; --jr-slider-buffered: 0%;",
            onmounted: move |e| async move {
                if let Ok(r) = e.data().get_client_rect().await { width.set(r.size.width); }
            },
            onmousedown: move |e| {
                let w = width();
                if w > 0.0 { onseek.call((e.element_coordinates().x / w).clamp(0.0, 1.0) * max); }
            },
            span { class: "jr-slider__track", span { class: "jr-slider__fill" } }
            span { class: "jr-slider__thumb" }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Ui {
    current: Signal<Option<usize>>,
    playing: Signal<bool>,
    position: Signal<f64>,
    sheet_open: Signal<bool>,
    sheet_y: Signal<f64>,
}

fn play_index(mut ui: Ui, i: usize) {
    let d = DATA.get().unwrap();
    if let Some(t) = d.tracks.get(i) {
        PLAYER.get().unwrap().send(Cmd::Load(d.session.audio_url(&t.id)));
        ui.current.set(Some(i));
        ui.playing.set(true);
        ui.position.set(0.0);
    }
}

fn app() -> Element {
    let ui = Ui {
        current: use_signal(|| None),
        playing: use_signal(|| false),
        position: use_signal(|| 0.0),
        sheet_open: use_signal(|| false),
        sheet_y: use_signal(|| 100.0),
    };
    use_context_provider(|| ui);

    // Poll the audio thread (progress bar), and advance to the next track at the end.
    use_future(move || {
        let mut ui = ui;
        async move {
            let p = PLAYER.get().unwrap();
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if ui.current.peek().is_some() {
                    ui.position.set(p.position());
                    ui.playing.set(p.playing() || p.loading());
                    if p.finished() {
                        let next = ui.current.peek().unwrap() + 1;
                        if next < DATA.get().unwrap().tracks.len() {
                            play_index(ui, next);
                        }
                    }
                }
            }
        }
    });

    // Sheet slide-in/out: the app uses framer-motion `springs.smooth` (stiffness 300, damping 30,
    // mass 1); integrated here with the same constants at ~120 Hz.
    use_future(move || {
        let mut ui = ui;
        async move {
            let (mut y, mut v) = (100.0f64, 0.0f64);
            loop {
                tokio::time::sleep(Duration::from_millis(8)).await;
                let target = if *ui.sheet_open.peek() { 0.0 } else { 100.0 };
                if (y - target).abs() < 0.01 && v.abs() < 0.01 {
                    if *ui.sheet_y.peek() != target {
                        ui.sheet_y.set(target);
                    }
                    continue;
                }
                let dt = 0.008;
                let a = -300.0 * (y - target) - 30.0 * v;
                v += a * dt;
                y += v * dt;
                ui.sheet_y.set(y);
            }
        }
    });

    // Debug / screenshot helpers.
    use_hook(move || {
        let mut ui = ui;
        if std::env::var("ORPHYN_AUTOPLAY").is_ok() {
            play_index(ui, 0);
        }
        if std::env::var("ORPHYN_OPEN_SHEET").is_ok() {
            ui.sheet_open.set(true);
        }
    });

    let d = DATA.get().unwrap();
    let art = d.session.image(&d.album.id, 464);
    rsx! {
        style { {CSS} }
        style { {OVERRIDES} }
        div { class: "jr-root", "data-theme": "dark", "data-backdrop": "immersive", "data-style": "classic",
            div { class: "jr-shell",
                div { class: "jr-shell__ambient", "aria-hidden": "true",
                    div { class: "jr-shell__ambient-art", style: "--ambient-image: url('{art}')" }
                }
                div { class: "jr-shell__sidebar", Sidebar {} }
                div { class: "jr-shell__main",
                    main { class: "jr-shell__scroll", AlbumPage {} }
                    section { class: "jr-shell__now-playing jr-material-thick", MiniPlayer {} }
                }
                NowPlayingSheet {}
            }
        }
    }
}

#[component]
fn SidebarItem(icon: &'static str, label: &'static str, #[props(default = false)] active: bool) -> Element {
    let cls = if active { "jr-sidebar-item__link jr-sidebar-item__link--active" } else { "jr-sidebar-item__link" };
    rsx! {
        li { class: "jr-sidebar-item",
            button { r#type: "button", class: "{cls}",
                span { class: "jr-sidebar-item__icon", Icon { name: icon, size: 18 } }
                span { class: "jr-sidebar-item__label", "{label}" }
            }
        }
    }
}

#[component]
fn Section(title: &'static str, children: Element) -> Element {
    rsx! {
        section { class: "jr-sidebar-section",
            h2 { class: "jr-sidebar-section__title", "{title}" }
            ul { class: "jr-sidebar-section__list", {children} }
        }
    }
}

fn Sidebar() -> Element {
    rsx! {
        nav { class: "jr-sidebar",
            div { class: "jr-sidebar__titlebar jr-drag" }
            div { class: "jr-sidebar__header",
                div { class: "jr-field jr-field--medium jr-search-field",
                    div { class: "jr-field__box",
                        span { class: "jr-field__leading", Icon { name: "search", size: 16 } }
                        input { class: "jr-field__input", placeholder: "Search", r#type: "text" }
                    }
                }
            }
            div { class: "jr-sidebar__scroll",
                Section { title: "Watch",
                    SidebarItem { icon: "circle-play", label: "Home" }
                    SidebarItem { icon: "clapperboard", label: "Movies" }
                    SidebarItem { icon: "tv", label: "TV Shows" }
                }
                Section { title: "Music",
                    SidebarItem { icon: "house", label: "Home" }
                    SidebarItem { icon: "disc-3", label: "Albums", active: true }
                    SidebarItem { icon: "mic-vocal", label: "Artists" }
                    SidebarItem { icon: "list-music", label: "Playlists" }
                }
            }
            div { class: "jr-sidebar__footer",
                ul { class: "jr-sidebar-section__list", SidebarItem { icon: "settings", label: "Settings" } }
            }
        }
    }
}

fn AlbumPage() -> Element {
    let ui = use_context::<Ui>();
    let d = DATA.get().unwrap();
    let album = &d.album;
    let total: f64 = d.tracks.iter().map(|t| t.seconds()).sum();
    let minutes = ((total / 60.0).round() as u64).max(1);
    let total_text = if minutes >= 60 { format!("{} hr {} min", minutes / 60, minutes % 60) } else { format!("{minutes} min") };
    let meta = [album.genres.first().cloned(), album.production_year.map(|y| y.to_string())]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let art = d.session.image(&album.id, 464);
    let artist = album.album_artist.clone().unwrap_or_default();
    let fav = album.user_data.as_ref().map(|u| u.is_favorite).unwrap_or(false);
    let n = d.tracks.len();
    rsx! {
        div { class: "jr-page jr-page--inline",
            div { class: "jr-page__toolbar",
                div { class: "jr-toolbar jr-toolbar--transparent jr-drag",
                    div { class: "jr-toolbar__leading",
                        IconButton { icon: "chevron-left", label: "Back" }
                    }
                    div { class: "jr-toolbar__title" }
                    div { class: "jr-toolbar__trailing" }
                }
            }
            div { class: "jr-page__content jr-page__content--gutter",
                header { class: "jrm-detail",
                    div { class: "jr-artwork jr-artwork--rounded jr-artwork--loaded jr-artwork--shadow jrm-detail__art", style: "aspect-ratio: 1 / 1;",
                        img { class: "jr-artwork__img", src: "{art}" }
                    }
                    div { class: "jrm-detail__info",
                        h1 { class: "jr-text-title-1 jr-color-primary jrm-detail__title", "{album.name}" }
                        h3 { class: "jr-text-title-3 jr-color-primary jrm-detail__subtitle",
                            span { class: "jrm-link jrm-link--accent", "{artist}" }
                        }
                        p { class: "jr-text-footnote jr-color-secondary jrm-detail__meta", "{meta}" }
                        div { class: "jrm-detail__actions",
                            button { r#type: "button", class: "jr-button jr-button--filled jr-button--medium",
                                onclick: move |_| play_index(ui, 0),
                                span { class: "jr-button__icon", Icon { name: "play", size: 16, fill: true } }
                                span { class: "jr-button__label", "Play" }
                            }
                            button { r#type: "button", class: "jr-button jr-button--tinted jr-button--medium",
                                onclick: move |_| play_index(ui, (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() as usize) % n.max(1)),
                                span { class: "jr-button__icon", Icon { name: "shuffle", size: 16 } }
                                span { class: "jr-button__label", "Shuffle" }
                            }
                            IconButton { icon: "heart", label: "Favorite", variant: "gray", fill: fav, pressed: fav }
                            IconButton { icon: "ellipsis", label: "More", variant: "gray" }
                        }
                    }
                }
                ol { class: "jr-track-list jrm-tracklist", "aria-label": "{album.name}",
                    for (i, t) in d.tracks.iter().enumerate() {
                        TrackRow { key: "{t.id}", i, track: t.clone(), hide_artist: artist.clone() }
                    }
                }
                p { class: "jr-text-footnote jr-color-secondary jrm-footer", "{n} songs · {total_text}" }
            }
        }
    }
}

#[component]
fn TrackRow(i: usize, track: Item, hide_artist: String) -> Element {
    let ui = use_context::<Ui>();
    let current = (ui.current)() == Some(i);
    let playing = current && (ui.playing)();
    let pp = if playing { "pause" } else { "play" };
    let cls = if current { "jr-track jr-track--current" } else { "jr-track" };
    let number = track.index_number.map(|n| n as usize).unwrap_or(i + 1);
    let artist = track.artist();
    let show_artist = !artist.is_empty() && artist != hide_artist;
    rsx! {
        li { class: "{cls}", tabindex: "0",
            ondoubleclick: move |_| play_index(ui, i),
            span { class: "jr-track__lead",
                span { class: "jr-track__number", if playing { "♪" } else { "{number}" } }
                button { r#type: "button", class: "jr-track__play", tabindex: "-1",
                    onclick: move |e| { e.stop_propagation(); play_index(ui, i); },
                    Icon { name: pp, size: 14, fill: true }
                }
            }
            span { class: "jr-track__main",
                span { class: "jr-track__title", span { class: "jr-track__title-text", "{track.name}" } }
                if show_artist { span { class: "jr-track__artist", "{artist}" } }
            }
            span { class: "jr-track__accessory",
                button { r#type: "button", tabindex: "-1", class: "jrm-heart jrm-heart--off",
                    Icon { name: "heart", size: 14 }
                }
            }
            span { class: "jr-track__duration", "{fmt(track.seconds())}" }
            span { class: "jr-track__more-slot",
                button { r#type: "button", tabindex: "-1", class: "jr-track__more", Icon { name: "ellipsis", size: 16 } }
            }
        }
    }
}

fn MiniPlayer() -> Element {
    let mut ui = use_context::<Ui>();
    let d = DATA.get().unwrap();
    let track = (ui.current)().and_then(|i| d.tracks.get(i));
    let playing = (ui.playing)();
    let pp = if playing { "pause" } else { "play" };
    let pos = (ui.position)();
    let dur = track.map(|t| t.seconds()).unwrap_or(0.0);
    let toggle = move |_| {
        PLAYER.get().unwrap().send(Cmd::Toggle);
        let p = *ui.playing.peek();
        ui.playing.set(!p);
    };
    let step = move |delta: isize| {
        if let Some(i) = *ui.current.peek() {
            let n = DATA.get().unwrap().tracks.len() as isize;
            let j = i as isize + delta;
            if (0..n).contains(&j) {
                play_index(ui, j as usize);
            }
        }
    };
    rsx! {
        div { class: "jrm-mini", role: "region",
            div { class: "jrm-mini__left",
                div { class: "jrm-transport", role: "group",
                    IconButton { icon: "shuffle", label: "Shuffle", class: "jrm-toggle" }
                    IconButton { icon: "skip-back", label: "Previous", fill: true, onclick: move |_| step(-1) }
                    IconButton { icon: pp, label: "Play", fill: true, onclick: toggle }
                    IconButton { icon: "skip-forward", label: "Next", fill: true, onclick: move |_| step(1) }
                    IconButton { icon: "repeat", label: "Repeat", class: "jrm-toggle" }
                }
            }
            div { class: "jrm-mini__capsule",
                if let Some(t) = track {
                    button { r#type: "button", class: "jrm-mini__info", onclick: move |_| ui.sheet_open.set(true),
                        div { class: "jr-artwork jr-artwork--rounded jr-artwork--loaded jrm-mini__art", style: "aspect-ratio: 1 / 1;",
                            img { class: "jr-artwork__img", src: "{d.session.image(&d.album.id, 112)}" }
                        }
                        span { class: "jrm-mini__text",
                            span { class: "jr-text-subheadline jr-color-primary jr-truncate jrm-mini__title", "{t.name}" }
                            span { class: "jr-text-footnote jr-color-secondary jr-truncate jrm-mini__subtitle",
                                "{t.artist()} — {d.album.name}"
                            }
                        }
                    }
                    div { class: "jrm-mini__times jr-tabular",
                        span { "{fmt(pos)}" }
                        span { "-{fmt((dur - pos).max(0.0))}" }
                    }
                    div { class: "jrm-mini__progress",
                        Slider { value: pos, max: dur, onseek: move |t: f64| {
                            PLAYER.get().unwrap().send(Cmd::Seek(t));
                            ui.position.set(t);
                        } }
                    }
                } else {
                    div { class: "jrm-mini__info",
                        img { class: "jrm-mini__logo", src: logo_url(), width: "32", height: "32" }
                        span { class: "jrm-mini__text",
                            span { class: "jr-text-subheadline jr-color-secondary jr-truncate jrm-mini__title", "Not Playing" }
                        }
                    }
                }
            }
            div { class: "jrm-mini__right",
                div { class: "jrm-volume",
                    Icon { name: "volume-2", size: 16 }
                    Slider { value: 100.0, max: 100.0, onseek: move |v: f64| PLAYER.get().unwrap().send(Cmd::Volume((v / 100.0) as f32)) }
                }
                IconButton { icon: "message-square-quote", label: "Lyrics" }
                IconButton { icon: "list-music", label: "Queue" }
            }
        }
    }
}

fn NowPlayingSheet() -> Element {
    let mut ui = use_context::<Ui>();
    let d = DATA.get().unwrap();
    let y = (ui.sheet_y)();
    if y >= 99.9 && !(ui.sheet_open)() {
        return rsx! {};
    }
    let Some(t) = (ui.current)().and_then(|i| d.tracks.get(i)) else { return rsx! {} };
    let bg = d.session.image(&d.album.id, 160);
    let art = d.session.image(&d.album.id, 640);
    let pos = (ui.position)();
    let dur = t.seconds();
    let playing = (ui.playing)();
    let pp = if playing { "pause" } else { "play" };
    rsx! {
        div { class: "jrm-np", "data-theme": "dark", "data-view": "player", role: "dialog",
            style: "transform: translateY({y}%);",
            div { class: "jrm-np__bg", "aria-hidden": "true",
                div { class: "jrm-np__blob jrm-np__blob--a", style: "background-image: url('{bg}')" }
                div { class: "jrm-np__blob jrm-np__blob--b", style: "background-image: url('{bg}')" }
                div { class: "jrm-np__scrim" }
            }
            header { class: "jrm-np__header jr-drag",
                IconButton { icon: "chevron-down", label: "Close", variant: "glass", onclick: move |_| ui.sheet_open.set(false) }
                div { class: "jr-segmented jrm-np__tabs", role: "radiogroup",
                    button { class: "jr-segmented__item jr-segmented__item--selected", "Now Playing" }
                    button { class: "jr-segmented__item", "Lyrics" }
                    button { class: "jr-segmented__item", "Up Next" }
                }
                div { class: "jrm-np__header-end" }
            }
            div { class: "jrm-np__body",
                section { class: "jrm-np__main",
                    div { class: "jr-artwork jr-artwork--rounded jr-artwork--loaded jr-artwork--shadow jrm-np__art", style: "aspect-ratio: 1 / 1;",
                        img { class: "jr-artwork__img", src: "{art}" }
                    }
                    div { class: "jrm-np__meta",
                        span { class: "jr-text-title-2 jr-color-primary jr-truncate jrm-np__title", "{t.name}" }
                        span { class: "jr-text-body jr-color-secondary jr-truncate", "{t.artist()} — {d.album.name}" }
                    }
                    div { class: "jrm-seek",
                        span { class: "jrm-seek__time jr-tabular", "{fmt(pos)}" }
                        Slider { class: "jr-slider--on-media", value: pos, max: dur, onseek: move |s: f64| {
                            PLAYER.get().unwrap().send(Cmd::Seek(s));
                            ui.position.set(s);
                        } }
                        span { class: "jrm-seek__time jr-tabular", "-{fmt((dur - pos).max(0.0))}" }
                    }
                    div { class: "jrm-transport",
                        IconButton { icon: "shuffle", label: "Shuffle" }
                        IconButton { icon: "skip-back", label: "Previous", size: "large", fill: true }
                        IconButton { icon: pp, label: "Play", size: "large", fill: true,
                            onclick: move |_| { PLAYER.get().unwrap().send(Cmd::Toggle); let p = *ui.playing.peek(); ui.playing.set(!p); } }
                        IconButton { icon: "skip-forward", label: "Next", size: "large", fill: true }
                        IconButton { icon: "repeat", label: "Repeat" }
                    }
                }
            }
        }
    }
}

fn logo_url() -> String {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/orphyn-logo.png").replace('\\', "/");
    format!("file:///{}", p.trim_start_matches('/'))
}
