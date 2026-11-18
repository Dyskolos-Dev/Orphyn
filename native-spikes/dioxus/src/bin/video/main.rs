//! Spike A2: libmpv video inside the Dioxus Native (Blitz) window, controls are plain RSX/CSS on top.
//! The video is a Blitz `Widget` (`<object data=...>`): mpv renders BGRA on a worker thread, the widget
//! uploads the newest frame into a wgpu texture and paints it through the renderer's resource API.
#[path = "../../jellyfin.rs"]
mod jellyfin;
mod mpv;

use anyrender::{Paint, PaintScene, RenderContext, Scene};
use blitz_dom::node::ComputedStyles;
use dioxus::prelude::*;
use dioxus_native::{CustomWidgetAttr, DeviceHandle, Widget};
use mpv::Player;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

static PLAYER: OnceLock<Arc<Player>> = OnceLock::new();
static PAINTS: AtomicU64 = AtomicU64::new(0);

struct VideoWidget {
    player: Arc<Player>,
    dev: Option<DeviceHandle>,
    tex: Option<(wgpu::Texture, u32, u32)>,
    res: Option<anyrender::ResourceId>,
}

impl Widget for VideoWidget {
    fn can_create_surfaces(&mut self, ctx: &mut dyn RenderContext) {
        self.dev = ctx.renderer_specific_context().and_then(|c| c.downcast::<DeviceHandle>().ok()).map(|b| *b);
        if let Some(d) = &self.dev { let i = d.adapter.get_info(); eprintln!("[video] wgpu adapter: {} ({:?}, {:?})", i.name, i.backend, i.device_type); }
    }
    fn destroy_surfaces(&mut self) {
        self.tex = None;
        self.res = None;
        self.dev = None;
    }
    fn requires_redraw(&self) -> bool {
        true
    }
    fn paint(&mut self, ctx: &mut dyn RenderContext, _s: &ComputedStyles, width: u32, height: u32, _scale: f64) -> Scene {
        let mut scene = Scene::new();
        if PAINTS.load(Relaxed) == 0 { eprintln!("[video] first paint {width}x{height} dev={}", self.dev.is_some()); }
        let Some(dev) = &self.dev else { return scene };
        self.player.target.0.store(width, Relaxed);
        self.player.target.1.store(height, Relaxed);
        {
            let mut f = self.player.frame.lock().unwrap();
            if f.fresh && f.w > 0 {
                let (w, h) = (f.w, f.h);
                if self.tex.as_ref().map(|t| (t.1, t.2)) != Some((w, h)) {
                    if let Some(id) = self.res.take() {
                        ctx.unregister_resource(id);
                    }
                    let tex = dev.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("mpv-frame"),
                        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    eprintln!("[video] texture {w}x{h}");
                    self.res = ctx.try_register_custom_resource(Box::new(tex.clone())).ok();
                    self.tex = Some((tex, w, h));
                }
                let (tex, _, _) = self.tex.as_ref().unwrap();
                dev.queue.write_texture(
                    tex.as_image_copy(),
                    &f.data,
                    wgpu::TexelCopyBufferLayout { offset: None.unwrap_or(0), bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                f.fresh = false;
                PAINTS.fetch_add(1, Relaxed);
            }
        }
        if let (Some(id), Some((_, w, h))) = (self.res, &self.tex) {
            let brush = Paint::Resource(peniko::ImageBrush { image: id, sampler: Default::default() });
            // texture is rendered at the widget size, so it maps 1:1
            let _ = (w, h);
            scene.fill(peniko::Fill::NonZero, kurbo::Affine::IDENTITY, &brush, None, &kurbo::Rect::new(0., 0., width as f64, height as f64));
        }
        scene
    }
}

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

fn fmt(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(unix)]
extern "C" fn on_segv(_: libc::c_int, info: *mut libc::siginfo_t, uc: *mut libc::c_void) {
    unsafe {
        let rip = (*(uc as *mut libc::ucontext_t)).uc_mcontext.gregs[libc::REG_RIP as usize] as u64;
        let addr = (*info).si_addr() as u64;
        eprintln!("[segv] rip={rip:#x} fault_addr={addr:#x}");
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            for l in maps.lines() {
                if let Some((range, _)) = l.split_once(' ') {
                    if let Some((a, b)) = range.split_once('-') {
                        let (a, b) = (u64::from_str_radix(a, 16).unwrap_or(0), u64::from_str_radix(b, 16).unwrap_or(0));
                        if rip >= a && rip < b {
                            eprintln!("[segv] in {l} (offset in map {:#x})", rip - a);
                        }
                    }
                }
            }
        }
    }
    std::process::exit(139);
}

fn main() {
    #[cfg(unix)]
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_segv as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        libc::sigaction(libc::SIGSEGV, &sa, std::ptr::null_mut());
    }
    load_dotenv();
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let url = match std::env::var("ORPHYN_VIDEO_URL") {
        Ok(u) => u,
        Err(_) => {
            let s = jellyfin::Session::login(&var("ORPHYN_E2E_URL"), &var("ORPHYN_E2E_USER"), &var("ORPHYN_E2E_PASSWORD")).expect("login");
            let id = var("ORPHYN_VIDEO_ID");
            format!("{}/Videos/{id}/stream?static=true&api_key={}", s.base, s.token)
        }
    };
    eprintln!("[video] starting mpv");
    let player = Arc::new(Player::start(&url).expect("mpv"));
    eprintln!("[video] mpv ready");
    if let Some(s) = std::env::var("ORPHYN_DELAY").ok().and_then(|v| v.parse::<u64>().ok()) { std::thread::sleep(Duration::from_secs(s)); eprintln!("[video] delay over, state {:?}", player.state.lock().unwrap().position); }
    let _ = PLAYER.set(player.clone());
    // stats + scripted seek test, once per second on stderr
    let secs: u64 = std::env::var("ORPHYN_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    std::thread::spawn({
        let p = player.clone();
        move || {
            let t0 = Instant::now();
            let (mut r0, mut q0) = (0, 0);
            let mut seek: Option<(Instant, f64)> = None;
            let mut seeked = false;
            loop {
                std::thread::sleep(Duration::from_millis(250));
                if let Some((t, target)) = seek {
                    let st = p.state.lock().unwrap().clone();
                    if (st.position - target).abs() < 1.5 {
                        eprintln!("[video] seek latency {:?}", t.elapsed());
                        seek = None;
                    }
                }
                let el = t0.elapsed().as_secs();
                if !seeked && el >= 25 && std::env::var("ORPHYN_SEEK_TEST").is_ok() {
                    seeked = true;
                    let d = p.state.lock().unwrap().duration;
                    seek = Some((Instant::now(), d * 0.6));
                    p.seek_frac(0.6);
                }
                if t0.elapsed().as_millis() % 1000 < 250 {
                    let (r, q) = (p.renders.load(Relaxed), PAINTS.load(Relaxed));
                    let st = p.state.lock().unwrap().clone();
                    eprintln!(
                        "[video] t={}s pos={:.1} renders/s={} uploads/s={} hwdec={} src={}x{} drop(vo)={} drop(dec)={} delayed={}",
                        el, st.position, r - r0, q - q0, st.hwdec, st.video_w, st.video_h, st.dropped, st.decoder_dropped, st.vo_delayed
                    );
                    (r0, q0) = (r, q);
                }
                if secs > 0 && el >= secs {
                    std::process::exit(0);
                }
            }
        }
    });
    let size = std::env::var("ORPHYN_WIN").ok().and_then(|v| v.split_once('x').map(|(w, h)| (w.parse().unwrap_or(1280.0), h.parse().unwrap_or(720.0)))).unwrap_or((1280.0, 720.0));
    let window = winit::window::WindowAttributes::default().with_title("Orphyn video spike").with_surface_size(winit::dpi::LogicalSize::new(size.0, size.1));
    dioxus_native::launch_cfg(app, vec![], vec![Box::new(window)]);
}

const CSS: &str = "
html,body{margin:0;background:#000;overflow:hidden}
.stage{position:relative;width:100vw;height:100vh;background:#000;font-family:sans-serif;color:#fff}
.video{position:absolute;left:0;top:0;width:100%;height:100%}
.bar{position:absolute;left:0;right:0;bottom:0;padding:48px 24px 20px;background:linear-gradient(to top,rgba(0,0,0,.85),rgba(0,0,0,0));display:flex;align-items:center;gap:16px}
.btn{width:44px;height:44px;border-radius:22px;background:rgba(255,255,255,.18);display:flex;align-items:center;justify-content:center;font-size:18px}
.seek{flex:1;height:6px;border-radius:3px;background:rgba(255,255,255,.25);position:relative}
.seek-fill{height:6px;border-radius:3px;background:#fff}
.time{font-size:13px;min-width:90px;text-align:right}
.title{position:absolute;left:24px;top:20px;font-size:20px;font-weight:600;text-shadow:0 1px 6px rgba(0,0,0,.8)}
.hud{position:absolute;right:24px;top:20px;font-size:12px;background:rgba(0,0,0,.55);padding:6px 10px;border-radius:8px}
";

fn app() -> Element {
    let p = PLAYER.get().unwrap().clone();
    let mut pos = use_signal(|| 0.0f64);
    let mut dur = use_signal(|| 0.0f64);
    let mut paused = use_signal(|| false);
    let mut hud = use_signal(String::new);
    let widget = use_hook({
        let p = p.clone();
        move || CustomWidgetAttr::new(VideoWidget { player: p, dev: None, tex: None, res: None })
    });
    use_future({
        let p = p.clone();
        move || {
            let p = p.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let st = p.state.lock().unwrap().clone();
                    pos.set(st.position);
                    dur.set(st.duration);
                    paused.set(st.paused);
                    hud.set(format!("{}x{} {}", st.video_w, st.video_h, st.hwdec));
                }
            }
        }
    });
    let pct = if dur() > 0.0 { pos() / dur() * 100.0 } else { 0.0 };
    let (pp, ps) = (p.clone(), p.clone());
    rsx! {
        style { {CSS} }
        div { class: "stage",
            object { class: "video", data: widget }
            div { class: "title", "Orphyn video spike (libmpv in Blitz)" }
            div { class: "hud", "{hud}" }
            div { class: "bar",
                div { class: "btn", onclick: move |_| pp.toggle_pause(), if paused() { "▶" } else { "❚❚" } }
                div { class: "seek",
                    onmousedown: move |e| {
                        let w = 1.0f64.max(e.element_coordinates().x);
                        let _ = w;
                    },
                    onclick: move |e| {
                        // fraction along the bar; width is read from the full-width layout (bar minus paddings)
                        let x = e.element_coordinates().x;
                        let width = (e.client_coordinates().x - x).max(0.0);
                        let _ = width;
                        ps.seek_frac((x / SEEK_W.load(Relaxed).max(1) as f64).clamp(0.0, 1.0));
                    },
                    div { class: "seek-fill", style: "width:{pct}%" }
                }
                div { class: "time", "{fmt(pos())} / {fmt(dur())}" }
            }
        }
    }
}

static SEEK_W: AtomicU64 = AtomicU64::new(1000);
