//! Orphyn GPUI spike: album page + mini player + Now Playing sheet + libmpv video overlay.
//! Every number below is copied from the app's CSS (dark theme, default interface style,
//! "immersive" artwork backdrop); the source rule is noted next to the value.
mod backdrop;
mod jellyfin;
mod mpv;

use anyhow::Result;
use gpui::{
    canvas, div, img, linear_color_stop, linear_gradient, point, prelude::*, px, size, svg, App,
    Application, AssetSource, Bounds, BoxShadow, Context, FontWeight, Hsla, MouseButton,
    MouseDownEvent, ObjectFit, Pixels, RenderImage, Rgba, SharedString, Window, WindowBounds,
    WindowOptions,
};
use jellyfin::{Client, Item};
use std::{
    cell::Cell,
    io::Cursor,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------- assets (lucide icons)
struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let dir = std::env::var("ORPHYN_ASSETS").unwrap_or_else(|_| format!("{}/assets", env!("CARGO_MANIFEST_DIR")));
        Ok(std::fs::read(PathBuf::from(dir).join(path)).ok().map(Into::into))
    }
    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------- tokens (styles/tokens.css, dark)
fn rgba8(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba { r: r as f32 / 255., g: g as f32 / 255., b: b as f32 / 255., a }.into()
}
fn hex(v: u32) -> Hsla {
    gpui::rgb(v).into()
}
mod t {
    use super::*;
    pub fn window() -> Hsla { hex(0x1d1d1f) } // --surface-window-base
    pub fn sidebar() -> Hsla { hex(0x262628) } // --surface-sidebar-base
    pub fn label() -> Hsla { hex(0xffffff) }
    pub fn secondary() -> Hsla { rgba8(235, 235, 245, 0.66) } // --secondary-label
    pub fn separator() -> Hsla { rgba8(84, 84, 88, 0.6) }
    pub fn quaternary_fill() -> Hsla { rgba8(118, 118, 128, 0.18) }
    pub fn tertiary_fill() -> Hsla { rgba8(118, 118, 128, 0.24) } // table says 118/118/128 .24
    pub fn secondary_fill() -> Hsla { rgba8(120, 120, 128, 0.32) }
    pub fn system_fill() -> Hsla { rgba8(120, 120, 128, 0.36) }
    pub fn hover() -> Hsla { rgba8(255, 255, 255, 0.06) } // --surface-hover
    pub fn accent() -> Hsla { hex(0xfc7b8c) } // --accent (dark)
    pub fn accent_fill() -> Hsla { hex(0xed0624) }
    pub fn accent_tint() -> Hsla { rgba8(0xfc, 0x7b, 0x8c, 0.14) }
    pub fn font() -> &'static str { if cfg!(windows) { "Segoe UI" } else { "Inter" } }
    pub const SIDEBAR_W: f32 = 248.;
    pub const TOOLBAR_H: f32 = 52.;
    pub const NOW_PLAYING_H: f32 = 72.;
    pub const GUTTER: f32 = 40.;
}

fn shadow(y: f32, blur: f32, a: f32) -> BoxShadow {
    BoxShadow { color: rgba8(0, 0, 0, a), offset: point(px(0.), px(y)), blur_radius: px(blur), spread_radius: px(0.) }
}

fn icon(name: &str, size_px: f32, color: Hsla) -> impl IntoElement {
    svg().path(format!("icons/{name}.svg")).size(px(size_px)).flex_none().text_color(color)
}

/// Critically-damped-ish spring from design/motion.ts `smooth` (stiffness 300, damping 30, mass 1): 0 -> 1.
fn spring(t: f32) -> f32 {
    let (k, c): (f32, f32) = (300., 30.);
    let w0 = k.sqrt();
    let z = c / (2. * k.sqrt());
    if z < 1. {
        let wd = w0 * (1. - z * z).sqrt();
        1. - (-z * w0 * t).exp() * ((wd * t).cos() + z * w0 / wd * (wd * t).sin())
    } else {
        1. - (-w0 * t).exp() * (1. + w0 * t)
    }
}

fn fmt_dur(s: f32) -> String {
    if !s.is_finite() || s < 0. {
        return "--:--".into();
    }
    let s = s as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

// ---------------------------------------------------------------- audio
struct Audio {
    _stream: rodio::OutputStream,
    _handle: rodio::OutputStreamHandle,
    sink: rodio::Sink,
}
impl Audio {
    fn new() -> Option<Self> {
        let (stream, handle) = rodio::OutputStream::try_default().ok()?;
        let sink = rodio::Sink::try_new(&handle).ok()?;
        Some(Self { _stream: stream, _handle: handle, sink })
    }
}

// ---------------------------------------------------------------- app state
struct OrphynApp {
    client: Arc<Client>,
    album: Item,
    tracks: Vec<Item>,
    art: PathBuf,
    backdrop: PathBuf,
    np_backdrop: PathBuf,
    last_fps_log: Instant,
    audio: Option<Audio>,
    current: Option<usize>,
    loading: bool,
    paused: bool,
    shuffle: bool,
    prog_bounds: Rc<Cell<Bounds<Pixels>>>,
    np_prog_bounds: Rc<Cell<Bounds<Pixels>>>,
    video_prog_bounds: Rc<Cell<Bounds<Pixels>>>,
    // Now Playing sheet: spring progress 0 (hidden) .. 1 (shown)
    np_target: f32,
    np_from: f32,
    np_start: Instant,
    // video
    video: Option<mpv::Player>,
    video_frame: Option<Arc<RenderImage>>,
    video_title: String,
    video_item: Option<Item>,
    // frame stats
    frames: std::collections::VecDeque<Instant>,
    started: Instant,
}

impl OrphynApp {
    fn np_value(&self) -> f32 {
        let t = self.np_start.elapsed().as_secs_f32();
        self.np_from + (self.np_target - self.np_from) * spring(t)
    }
    fn np_animating(&self) -> bool {
        self.np_start.elapsed().as_secs_f32() < 1.2
    }
    fn set_np(&mut self, open: bool) {
        self.np_from = self.np_value();
        self.np_target = if open { 1. } else { 0. };
        self.np_start = Instant::now();
    }

    fn play_index(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(tr) = self.tracks.get(i).cloned() else { return };
        self.current = Some(i);
        self.loading = true;
        self.paused = false;
        if let Some(a) = &self.audio {
            a.sink.stop();
        }
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let bytes = cx.background_spawn(async move { client.download_audio(&tr.id) }).await;
            this.update(cx, |this, cx| {
                this.loading = false;
                if this.current != Some(i) {
                    return;
                }
                match bytes {
                    Ok(b) => {
                        if let Some(a) = &mut this.audio {
                            // a stopped Sink cannot be reused: make a fresh one
                            if let Ok(s) = rodio::Sink::try_new(&a._handle) {
                                a.sink = s;
                            }
                            if let Ok(dec) = rodio::Decoder::new(Cursor::new(b)) {
                                a.sink.append(dec);
                                a.sink.play();
                            } else {
                                eprintln!("audio decode failed");
                            }
                        }
                    }
                    Err(e) => eprintln!("audio download failed: {e}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        match (&self.audio, self.current) {
            (Some(a), Some(_)) => {
                if self.paused {
                    a.sink.play();
                } else {
                    a.sink.pause();
                }
                self.paused = !self.paused;
            }
            _ => self.play_index(0, cx),
        }
        cx.notify();
    }
    fn step(&mut self, d: i32, cx: &mut Context<Self>) {
        let n = self.tracks.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.current.map(|c| c as i32).unwrap_or(-1);
        let next = (cur + d).rem_euclid(n);
        self.play_index(next as usize, cx);
    }
    fn position(&self) -> f32 {
        self.audio.as_ref().map(|a| a.sink.get_pos().as_secs_f32()).unwrap_or(0.)
    }
    fn duration(&self) -> f32 {
        self.current.and_then(|c| self.tracks.get(c)).map(|t| t.seconds()).unwrap_or(0.)
    }
    fn seek_frac(&mut self, f: f32) {
        let d = self.duration();
        if let (Some(a), true) = (&self.audio, d > 0.) {
            let _ = a.sink.try_seek(Duration::from_secs_f32((f.clamp(0., 1.)) * d));
        }
    }

    fn open_video(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.video_item.clone() else {
            eprintln!("no video item on this server");
            return;
        };
        let url = self.client.video_url(&item.id);
        self.video_title = item.name.clone();
        match mpv::Player::start(&url, 1280, 720) {
            Ok(p) => self.video = Some(p),
            Err(e) => {
                eprintln!("libmpv: {e}");
                return;
            }
        }
        cx.notify();
    }
}

// ---------------------------------------------------------------- views
impl OrphynApp {
    fn sidebar(&self) -> impl IntoElement {
        let item = |ic: &'static str, label: &'static str, active: bool| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .w_full()
                .h(px(30.))
                .px(px(8.))
                .rounded(px(6.))
                .when(active, |d| d.bg(rgba8(255, 255, 255, 0.11)))
                .hover(|d| d.bg(t::hover()))
                .child(icon(ic, 18., t::accent_fill()))
                .child(div().flex_1().text_size(px(14.)).line_height(px(19.)).font_weight(FontWeight::MEDIUM).child(label))
        };
        let title = |s: &'static str| {
            div().px(px(8.)).pb(px(4.)).text_size(px(12.)).line_height(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(t::secondary()).child(s)
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(t::SIDEBAR_W))
            .h_full()
            .bg(t::sidebar())
            .border_r(px(0.5))
            .border_color(t::separator())
            .child(div().h(px(t::TOOLBAR_H)))
            .child(
                div()
                    .px(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(title("Musique"))
                    .child(item("house", "Accueil", false))
                    .child(item("disc-3", "Albums", true))
                    .child(item("mic-vocal", "Artistes", false))
                    .child(item("list-music", "Titres", false))
                    .child(div().h(px(16.)))
                    .child(title("Bibliothèque"))
                    .child(item("film", "Films", false))
                    .child(item("tv", "Séries", false))
                    .child(item("radio", "Radio", false)),
            )
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // .jrm-detail: flex, align flex-end, gap 32, margin-bottom 32; art 232 w/ --shadow-artwork
        let subtitle = self.album.album_artist.clone().unwrap_or_default();
        let meta = [self.album.genres.first().cloned(), self.album.production_year.map(|y| y.to_string())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        let btn = |ic: &'static str, label: &'static str, filled: bool| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .h(px(34.)) // .jr-button--medium
                .px(px(16.))
                .rounded(px(8.)) // --radius-md
                .bg(if filled { t::accent_fill() } else { t::accent_tint() })
                .text_color(if filled { hex(0xffffff) } else { t::accent() })
                .text_size(px(14.))
                .line_height(px(19.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(icon(ic, 16., if filled { hex(0xffffff) } else { t::accent() }))
                .child(label)
        };
        let round = |ic: &'static str| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(32.))
                .rounded_full()
                .bg(t::tertiary_fill())
                .child(icon(ic, 16., t::label()))
        };
        div()
            .flex()
            .items_end()
            .gap(px(32.))
            .mb(px(32.))
            .child(
                div().flex_none().size(px(232.)).rounded(px(12.)).shadow(vec![shadow(2., 6., 0.3), shadow(8., 24., 0.35)]).child(
                    img(self.art.clone()).size_full().rounded(px(12.)).object_fit(ObjectFit::Cover),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .min_w_0()
                    .child(div().text_size(px(26.)).line_height(px(32.)).font_weight(FontWeight::BOLD).child(self.album.name.clone()))
                    .child(div().text_size(px(19.)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(t::accent()).child(subtitle))
                    .child(div().text_size(px(12.)).line_height(px(16.)).text_color(t::secondary()).child(meta))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .mt(px(16.))
                            .child(btn("play", "Lecture", true).id("play").on_click(cx.listener(|this, _, _, cx| this.play_index(0, cx))))
                            .child(btn("shuffle", "Aléatoire", false).id("shuffle").on_click(cx.listener(|this, _, _, cx| {
                                this.shuffle = true;
                                let i = (Instant::now().elapsed().subsec_nanos() as usize + this.started.elapsed().subsec_nanos() as usize) % this.tracks.len().max(1);
                                this.play_index(i, cx)
                            })))
                            .child(round("heart"))
                            .child(round("ellipsis")),
                    ),
            )
    }

    fn rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // .jr-track: min-height 48 (.jrm-songs .jr-track: 52?), padding 6 8, radius-md 8, gap 12.
        // Zebra on even rows (nth-child(even) => index 1,3,...), hover --surface-hover.
        let hide_artist = self.album.album_artist.clone();
        let multi_disc = self.tracks.iter().any(|t| t.parent_index_number.unwrap_or(1) != 1);
        let mut list = div().flex().flex_col();
        for (i, tr) in self.tracks.iter().enumerate() {
            let is_cur = self.current == Some(i);
            let playing_now = is_cur && !self.paused && !self.loading;
            let artist = tr.artists.first().cloned().filter(|a| Some(a) != hide_artist.as_ref());
            let _ = multi_disc;
            let number = tr.index_number.unwrap_or(i as i32 + 1);
            let row = div()
                .id(("row", i))
                .group("row")
                .relative()
                .flex()
                .items_center()
                .gap(px(12.))
                .h(px(52.))
                .px(px(8.))
                .py(px(6.))
                .rounded(px(8.))
                .text_size(px(13.))
                .line_height(px(18.))
                .when(i % 2 == 1, |d| d.bg(t::quaternary_fill()))
                .hover(|d| d.bg(t::hover()))
                .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, _, cx| {
                    if ev.click_count() >= 2 {
                        this.play_index(i, cx);
                    }
                }))
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .w(px(36.))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .group_hover("row", |d| d.opacity(0.))
                                .text_color(if is_cur { t::accent() } else { t::secondary() })
                                .child(if playing_now { "♪".to_string() } else { number.to_string() }),
                        )
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.)
                                .group_hover("row", |d| d.opacity(1.))
                                .child(icon(if playing_now { "pause" } else { "play" }, 14., t::label()))
                                .id(("rowplay", i))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.current == Some(i) {
                                        this.toggle_pause(cx)
                                    } else {
                                        this.play_index(i, cx)
                                    }
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_cur { t::accent() } else { t::label() })
                                .truncate()
                                .child(tr.name.clone()),
                        )
                        .when_some(artist, |d, a| d.child(div().text_size(px(12.)).line_height(px(16.)).text_color(t::secondary()).truncate().child(a))),
                )
                .child(div().flex_none().w(px(56.)).flex().justify_end().text_color(t::secondary()).child(fmt_dur(tr.seconds())))
                .child(
                    div().flex_none().w(px(32.)).flex().items_center().justify_center().child(
                        div()
                            .size(px(28.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(0.)
                            .group_hover("row", |d| d.opacity(1.))
                            .child(icon("ellipsis", 16., t::accent())),
                    ),
                );
            list = list.child(row);
        }
        list
    }

    fn page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total: f32 = self.tracks.iter().map(|t| t.seconds()).sum();
        div()
            .id("page-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(t::GUTTER))
                    .pt(px(t::TOOLBAR_H))
                    .pb(px(48.))
                    .child(self.header(cx))
                    .child(self.rows(cx))
                    .child(
                        div()
                            .mt(px(16.))
                            .px(px(8.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(t::secondary())
                            .child(format!("{} titres · {} minutes", self.tracks.len(), (total / 60.).round() as u32)),
                    ),
            )
    }

    /// Slider like `.jrm-mini__progress` (3px track, label fill, 15% label background).
    fn progress(&self, frac: f32, bounds: Rc<Cell<Bounds<Pixels>>>, id: &'static str, cx: &mut Context<Self>, which: u8) -> impl IntoElement {
        div()
            .id(id)
            .relative()
            .w_full()
            .h(px(10.))
            .flex()
            .items_end()
            .child(canvas({ let b = bounds.clone(); move |bb, _, _| b.set(bb) }, |_, _, _, _| {}).absolute().size_full())
            .child(
                div()
                    .w_full()
                    .h(px(3.))
                    .mb(px(3.))
                    .rounded_full()
                    .bg(rgba8(255, 255, 255, 0.15))
                    .child(div().h_full().w(gpui::relative(frac.clamp(0., 1.))).rounded_full().bg(t::label())),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    let b = match which {
                        0 => this.prog_bounds.get(),
                        1 => this.np_prog_bounds.get(),
                        _ => this.video_prog_bounds.get(),
                    };
                    let f = ((ev.position.x - b.origin.x) / b.size.width).clamp(0., 1.);
                    if which == 2 {
                        if let Some(v) = &this.video {
                            v.seek_frac(f);
                        }
                    } else {
                        this.seek_frac(f);
                    }
                    cx.notify();
                }),
            )
    }

    fn mini(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = self.current.and_then(|c| self.tracks.get(c)).cloned();
        let pos = self.position();
        let dur = self.duration();
        let frac = if dur > 0. { pos / dur } else { 0. };
        let ib = |name: &'static str, size_px: f32, glyph: f32| {
            div().flex().items_center().justify_center().size(px(size_px)).rounded_full().hover(|d| d.bg(t::hover())).child(icon(name, glyph, t::secondary()))
        };
        let subtitle = tr.as_ref().map(|t| [t.artists.first().cloned(), t.album.clone()].into_iter().flatten().collect::<Vec<_>>().join(" — ")).unwrap_or_default();
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(t::NOW_PLAYING_H))
            .px(px(16.))
            .border_t(px(0.5))
            .border_color(t::separator())
            .gap(px(24.))
            .overflow_hidden()
            // left transport: gap 5, buttons 30, play 40
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .flex_none()
                    .child(ib("shuffle", 30., 17.))
                    .child(ib("skip-back", 30., 17.).id("prev").on_click(cx.listener(|t, _, _, cx| t.step(-1, cx))))
                    .child(
                        div()
                            .id("playpause")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(40.))
                            .rounded_full()
                            .hover(|d| d.bg(t::hover()))
                            .child(icon(if self.current.is_some() && !self.paused { "pause" } else { "play" }, 22., t::label()))
                            .on_click(cx.listener(|t, _, _, cx| t.toggle_pause(cx))),
                    )
                    .child(ib("skip-forward", 30., 17.).id("next").on_click(cx.listener(|t, _, _, cx| t.step(1, cx))))
                    .child(ib("repeat", 30., 17.)),
            )
            // capsule: flex 1, max 560, h 52, radius 10, tertiary fill + inset hairline
            .child(
                div()
                    .group("capsule")
                    .relative()
                    .flex()
                    .flex_1()
                    .items_center()
                    .min_w_0()
                    .max_w(px(560.))
                    .mx_auto()
                    .h(px(52.))
                    .overflow_hidden()
                    .rounded(px(10.))
                    .bg(t::tertiary_fill())
                    .border(px(0.5))
                    .border_color(t::separator())
                    .child(
                        div()
                            .id("capsule-info")
                            .flex()
                            .flex_1()
                            .items_center()
                            .min_w_0()
                            .h_full()
                            .pl(px(6.))
                            .pr(px(6.))
                            .pb(px(8.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.current.is_some() {
                                    this.set_np(true);
                                    cx.notify();
                                }
                            }))
                            .child(img(self.art.clone()).flex_none().size(px(40.)).mt(px(8.)).rounded(px(5.)).object_fit(ObjectFit::Cover))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(1.))
                                    .min_w_0()
                                    .px(px(12.))
                                    .child(div().max_w_full().text_size(px(13.)).line_height(px(16.)).font_weight(FontWeight::SEMIBOLD).truncate().child(
                                        tr.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "Aucune lecture".into()),
                                    ))
                                    .child(div().max_w_full().text_size(px(12.)).line_height(px(16.)).text_color(t::secondary()).truncate().child(subtitle)),
                            ),
                    )
                    // elapsed / remaining, visible on capsule hover
                    .child(
                        div()
                            .absolute()
                            .left(px(54.))
                            .right(px(10.))
                            .bottom(px(8.))
                            .flex()
                            .justify_between()
                            .text_size(px(11.))
                            .line_height(px(11.))
                            .text_color(t::secondary())
                            .opacity(0.)
                            .group_hover("capsule", |d| d.opacity(1.))
                            .child(fmt_dur(pos))
                            .child(if dur > 0. { format!("-{}", fmt_dur((dur - pos).max(0.))) } else { "--:--".into() }),
                    )
                    .child(div().absolute().left(px(8.)).right(px(8.)).bottom_0().child(self.progress(frac, self.prog_bounds.clone(), "mini-prog", cx, 0))),
            )
            // right: volume, lyrics, queue (gap 14)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .flex_none()
                    .child(
                        div().flex().items_center().gap(px(2.)).child(ib("volume-2", 30., 17.)).child(
                            div().w(px(90.)).h(px(4.)).rounded_full().bg(rgba8(255, 255, 255, 0.15)).child(div().h_full().w(px(60.)).rounded_full().bg(t::label())),
                        ),
                    )
                    .child(ib("film", 30., 17.).id("video-btn").on_click(cx.listener(|this, _, _, cx| this.open_video(cx))))
                    .child(ib("message-square-quote", 30., 17.))
                    .child(ib("list-music", 30., 17.)),
            )
    }

    fn now_playing(&self, win_h: f32, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let v = self.np_value();
        if v < 0.002 && self.np_target == 0. {
            return None;
        }
        let tr = self.current.and_then(|c| self.tracks.get(c)).cloned();
        let pos = self.position();
        let dur = self.duration();
        let frac = if dur > 0. { pos / dur } else { 0. };
        // .jrm-np__blob: blurred artwork, drifting (translate only here), opacity .75; scrim 45% system-background
        let drift = self.started.elapsed().as_secs_f32() / 38.;
        let art_side = (win_h * 0.52).min(520.);
        Some(
            div()
                .absolute()
                .left_0()
                .w_full()
                .h(px(win_h))
                .top(px((1. - v) * win_h)) // framer-motion y: 100% -> 0 with the `smooth` spring
                .overflow_hidden()
                .bg(hex(0x000000))
                .text_color(t::label())
                .child(
                    img(self.np_backdrop.clone())
                        .absolute()
                        .left(px(-300. + 150. * (drift * 6.28).sin()))
                        .top(px(-300. + 100. * (drift * 6.28).cos()))
                        .w(px(1700.))
                        .h(px(1400.))
                        .object_fit(ObjectFit::Cover)
                        .opacity(0.75),
                )
                .child(div().absolute().inset_0().bg(rgba8(0, 0, 0, 0.45)))
                // header
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .h(px(52.))
                        .px(px(t::GUTTER))
                        .child(
                            div()
                                .id("np-close")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(32.))
                                .rounded_full()
                                .bg(t::tertiary_fill())
                                .child(icon("chevron-down", 18., t::label()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_np(false);
                                    cx.notify();
                                })),
                        ),
                )
                // body: artwork + meta + seek + transport, column max-width min(52vh, 520)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(52.))
                        .bottom(px(40.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(art_side))
                                .flex()
                                .flex_col()
                                .gap(px(16.))
                                .child(div().w(px(art_side)).h(px(art_side)).rounded(px(12.)).shadow(vec![BoxShadow { color: rgba8(0, 0, 0, 0.55), offset: point(px(0.), px(12.)), blur_radius: px(40.), spread_radius: px(0.) }]).child(
                                    img(self.art.clone()).size_full().rounded(px(12.)).object_fit(ObjectFit::Cover),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .child(div().text_size(px(22.)).line_height(px(28.)).font_weight(FontWeight::BOLD).truncate().child(tr.as_ref().map(|t| t.name.clone()).unwrap_or_default()))
                                        .child(div().text_size(px(15.)).line_height(px(20.)).text_color(t::secondary()).truncate().child(
                                            tr.as_ref().and_then(|t| t.artists.first().cloned()).unwrap_or_default(),
                                        )),
                                )
                                .child(self.progress(frac, self.np_prog_bounds.clone(), "np-prog", cx, 1))
                                .child(
                                    div().flex().justify_between().text_size(px(11.)).line_height(px(14.)).text_color(t::secondary()).child(fmt_dur(pos)).child(format!("-{}", fmt_dur((dur - pos).max(0.)))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(24.))
                                        .child(icon("skip-back", 28., t::label()))
                                        .child(div().id("np-play").child(icon(if self.paused { "play" } else { "pause" }, 40., t::label())).on_click(cx.listener(|t, _, _, cx| t.toggle_pause(cx))))
                                        .child(icon("skip-forward", 28., t::label())),
                                ),
                        ),
                ),
        )
    }

    fn video_overlay(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let v = self.video.as_ref()?;
        let st = v.state();
        let frac = if st.duration > 0. { (st.position / st.duration) as f32 } else { 0. };
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(hex(0x000000))
                .group("video")
                .child(match &self.video_frame {
                    Some(f) => img(f.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
                    None => div().size_full().flex().items_center().justify_center().text_color(t::secondary()).child("Chargement…").into_any_element(),
                })
                // overlay control bar (GPUI elements painted over the mpv frame), shown on hover
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(120.))
                        .opacity(0.)
                        .group_hover("video", |d| d.opacity(1.))
                        .bg(linear_gradient(180., linear_color_stop(rgba8(0, 0, 0, 0.), 0.), linear_color_stop(rgba8(0, 0, 0, 0.75), 1.)))
                        .flex()
                        .flex_col()
                        .justify_end()
                        .px(px(32.))
                        .pb(px(24.))
                        .gap(px(10.))
                        .child(self.progress(frac, self.video_prog_bounds.clone(), "video-prog", cx, 2))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(16.))
                                .child(div().id("v-play").child(icon(if st.paused { "play" } else { "pause" }, 28., t::label())).on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(v) = &this.video {
                                        v.toggle_pause();
                                    }
                                    cx.notify();
                                })))
                                .child(div().text_size(px(13.)).child(format!("{} / {}", fmt_dur(st.position as f32), fmt_dur(st.duration as f32))))
                                .child(div().flex_1().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(self.video_title.clone()))
                                .child(div().id("v-close").child(icon("x", 24., t::label())).on_click(cx.listener(|this, _, _, cx| {
                                    this.video = None;
                                    this.video_frame = None;
                                    cx.notify();
                                }))),
                        ),
                ),
        )
    }
}

impl Render for OrphynApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        self.frames.push_back(now);
        while self.frames.front().map_or(false, |f| now.duration_since(*f) > Duration::from_secs(1)) {
            self.frames.pop_front();
        }
        let fps = self.frames.len();
        if (self.np_animating() || self.video.is_some()) && self.last_fps_log.elapsed() > Duration::from_secs(1) {
            eprintln!("[fps] {fps} renders in the last second (animating)");
            self.last_fps_log = Instant::now();
        }
        if self.np_animating() || self.video.is_some() {
            window.request_animation_frame();
        }
        let win_h: f32 = window.viewport_size().height.into();

        // .jr-shell__ambient-art: inset -12%, blurred artwork, opacity .5 (dark/immersive), + veil gradient
        let ambient = div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(img(self.backdrop.clone()).absolute().left(px(-150.)).top(px(-100.)).w(px(1600.)).h(px(1100.)).object_fit(ObjectFit::Cover).opacity(0.5))
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(rgba8(0x1d, 0x1d, 0x1f, 0.10), 0.),
                linear_color_stop(rgba8(0x1d, 0x1d, 0x1f, 0.85), 1.),
            )))
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(rgba8(0x1d, 0x1d, 0x1f, 0.0), 0.0),
                linear_color_stop(rgba8(0x1d, 0x1d, 0x1f, 0.0), 1.0),
            )));

        let show_fps = std::env::var("ORPHYN_FPS").is_ok();
        let np = self.now_playing(win_h, cx);
        let video = self.video_overlay(cx);
        div()
            .relative()
            .size_full()
            .flex()
            .bg(t::window())
            .text_color(t::label())
            .font_family(t::font())
            .text_size(px(15.))
            .child(self.sidebar())
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(ambient)
                    .child(self.page(cx))
                    .child(self.mini(cx)),
            )
            .children(np)
            .children(video)
            .when(show_fps, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(6.))
                        .right(px(8.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(rgba8(0, 0, 0, 0.7))
                        .text_size(px(11.))
                        .child(format!("{fps} renders/s")),
                )
            })
    }
}

fn main() -> Result<()> {
    let client = Arc::new(Client::from_env()?);
    let album = client.pick_album()?;
    let tracks = client.tracks(&album.id)?;
    let art = client.artwork(&album.id, 464)?; // 232 css px @2x
    let backdrop_src = client.artwork(&album.id, 400)?;
    let backdrop = backdrop::blurred(&backdrop_src, client.cache_dir(), &album.id, 0.6, 1.3)?;
    let np_backdrop = backdrop::blurred_as(&backdrop_src, client.cache_dir(), &album.id, "np", 1.0, 1.6)?;
    let video_item = client.pick_video().ok().flatten();
    eprintln!("album: {} ({} tracks); video: {:?}", album.name, tracks.len(), video_item.as_ref().map(|v| &v.name));

    Application::new().with_assets(Assets).run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(680.)), cx);
        cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| {
            cx.activate(true);
            let view = cx.new(|cx| {
                // 20 Hz clock: progress bar, video frames, track-end advance
                cx.spawn(async move |this: gpui::WeakEntity<OrphynApp>, cx| loop {
                    cx.background_executor().timer(Duration::from_millis(33)).await;
                    if this
                        .update(cx, |this, cx| {
                            if let Some(v) = &this.video {
                                if let Some(frame) = v.take_frame() {
                                    if let Some(old) = this.video_frame.replace(frame) {
                                        cx.drop_image(old, None);
                                    }
                                }
                            }
                            if let (Some(a), Some(c)) = (&this.audio, this.current) {
                                if a.sink.empty() && !this.loading && !this.paused {
                                    let n = (c + 1) % this.tracks.len().max(1);
                                    this.play_index(n, cx);
                                }
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                })
                .detach();
                OrphynApp {
                    client,
                    album,
                    tracks,
                    art,
                    backdrop,
                    np_backdrop,
                    last_fps_log: Instant::now(),
                    audio: Audio::new(),
                    current: None,
                    loading: false,
                    paused: false,
                    shuffle: false,
                    prog_bounds: Default::default(),
                    np_prog_bounds: Default::default(),
                    video_prog_bounds: Default::default(),
                    np_target: 0.,
                    np_from: 0.,
                    np_start: Instant::now() - Duration::from_secs(10),
                    video: None,
                    video_frame: None,
                    video_title: String::new(),
                    video_item,
                    frames: Default::default(),
                    started: Instant::now(),
                }
            });
            // test hooks for scripted screenshots
            let v = view.clone();
            if std::env::var("ORPHYN_AUTOPLAY").is_ok() {
                cx.defer(move |cx| {
                    v.update(cx, |this, cx| this.play_index(0, cx));
                });
            }
            if std::env::var("ORPHYN_OPEN_NP").is_ok() {
                let v = view.clone();
                cx.defer(move |cx| {
                    v.update(cx, |this, cx| {
                        if this.current.is_none() {
                            this.play_index(0, cx);
                        }
                        this.set_np(true);
                    });
                });
            }
            if std::env::var("ORPHYN_OPEN_VIDEO").is_ok() {
                let v = view.clone();
                cx.defer(move |cx| {
                    v.update(cx, |this, cx| this.open_video(cx));
                });
            }
            view
        })
        .unwrap();
    });
    Ok(())
}
