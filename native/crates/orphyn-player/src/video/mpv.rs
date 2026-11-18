//! libmpv through its software render API, loaded at run time (from spike A2). A worker thread
//! owns the handle, renders frames and swaps them into a shared slot; the widget uploads the
//! newest one into a wgpu texture. Hardware decoding (`auto-copy`) feeds the software renderer.
//!
//! Frame pipeline (see [`Pipeline`]):
//! - `Yuv` (default): mpv renders packed YUV 4:2:2 (`yuyv422`, 2 bytes per pixel) at the video's
//!   own size; the widget converts it to RGB in a wgpu shader and the GPU scales it into the box.
//!   The software render API takes a single plane, so two-plane NV12 cannot come out of it:
//!   packed 4:2:2 is the closest format it accepts. When this libmpv refuses `yuyv422`, frames
//!   fall back to `bgr0` through the same shader (still at video size, no CPU alpha pass).
//! - `Bgra` (`ORPHYN_VIDEO_PIPELINE=bgra`): the spike's path, BGRA at the box's pixel size with
//!   a CPU alpha pass, uploaded as is. Kept to compare (README "Video").
//!
//! Lookup: `ORPHYN_MPV_DIR` first, then the folder of the executable, then the system paths.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libloading::Library;

type Handle = *mut c_void;

#[repr(C)]
struct Param {
    ty: c_int,
    data: *mut c_void,
}

/// `mpv_event`.
#[repr(C)]
struct Event {
    id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

/// The start of `mpv_event_end_file`.
#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}

const FMT_STRING: c_int = 1;
const FMT_FLAG: c_int = 3;
const FMT_DOUBLE: c_int = 5;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const END_FILE_REASON_EOF: c_int = 0;
const END_FILE_REASON_ERROR: c_int = 4;
const RENDER_PARAM_API_TYPE: c_int = 1;
const RENDER_PARAM_SW_SIZE: c_int = 17;
const RENDER_PARAM_SW_FORMAT: c_int = 18;
const RENDER_PARAM_SW_STRIDE: c_int = 19;
const RENDER_PARAM_SW_POINTER: c_int = 20;
const RENDER_UPDATE_FRAME: u64 = 1;
/// Largest frame rendered (4K UHD).
const MAX_W: u32 = 3840;
const MAX_H: u32 = 2160;

struct Api {
    _lib: Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    get_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *mut c_void,
    terminate_destroy: unsafe extern "C" fn(Handle),
    render_create: unsafe extern "C" fn(*mut *mut c_void, Handle, *mut Param) -> c_int,
    render_update: unsafe extern "C" fn(*mut c_void) -> u64,
    render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    render_free: unsafe extern "C" fn(*mut c_void),
}

// SAFETY: the function pointers and the library handle are plain data; libmpv itself is thread
// safe for these calls, and the handle is only used from the worker thread.
unsafe impl Send for Api {}

fn candidates() -> Vec<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["libmpv-2.dll", "mpv-2.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib"]
    } else {
        &["libmpv.so.2", "libmpv.so.1", "libmpv.so"]
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("ORPHYN_MPV_DIR") {
        dirs.push(d.into());
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
    {
        dirs.push(d);
    }
    let mut out = Vec::new();
    for n in names {
        for d in &dirs {
            out.push(d.join(n));
        }
        out.push(PathBuf::from(n));
    }
    out
}

impl Api {
    fn load() -> Result<Self, String> {
        let mut last = String::new();
        for cand in candidates() {
            // SAFETY: loading libmpv runs its initialisers, which have no preconditions.
            match unsafe { Library::new(&cand) } {
                Ok(lib) => {
                    // SAFETY: the symbol types match the libmpv 2.x C API.
                    unsafe {
                        macro_rules! sym {
                            ($n:literal) => {
                                *lib.get($n).map_err(|e| e.to_string())?
                            };
                        }
                        return Ok(Api {
                            create: sym!(b"mpv_create"),
                            initialize: sym!(b"mpv_initialize"),
                            set_option_string: sym!(b"mpv_set_option_string"),
                            command: sym!(b"mpv_command"),
                            get_property: sym!(b"mpv_get_property"),
                            free: sym!(b"mpv_free"),
                            error_string: sym!(b"mpv_error_string"),
                            wait_event: sym!(b"mpv_wait_event"),
                            terminate_destroy: sym!(b"mpv_terminate_destroy"),
                            render_create: sym!(b"mpv_render_context_create"),
                            render_update: sym!(b"mpv_render_context_update"),
                            render: sym!(b"mpv_render_context_render"),
                            render_free: sym!(b"mpv_render_context_free"),
                            _lib: lib,
                        });
                    }
                }
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("libmpv not found ({last}); set ORPHYN_MPV_DIR"))
    }
}

/// What mpv reports, refreshed about 5 times a second.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MpvState {
    pub position: f64,
    pub duration: f64,
    pub buffered: f64,
    pub paused: bool,
    pub idle: bool,
    pub ended: bool,
    pub loaded: bool,
    /// Waiting for data (`paused-for-cache`) or seeking.
    pub buffering: bool,
    /// Number of `loadfile` commands run so far (tells states of an older file apart).
    pub loads: u64,
    /// Why the last file failed to load or play (end-file with an error).
    pub error: Option<String>,
    pub hwdec: String,
}

/// How the frames travel from mpv to the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pipeline {
    /// Packed YUV (or `bgr0` fallback) at video size, converted and scaled on the GPU.
    Yuv,
    /// BGRA at the box's pixel size, CPU alpha pass (spike A2).
    Bgra,
}

impl Pipeline {
    pub fn from_env() -> Self {
        match std::env::var("ORPHYN_VIDEO_PIPELINE").as_deref() {
            Ok("bgra") => Pipeline::Bgra,
            _ => Pipeline::Yuv,
        }
    }
}

/// Byte layout of a [`Frame`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PixelFormat {
    /// `bgr0`: 4 bytes per pixel, alpha undefined unless the CPU pass forced it.
    #[default]
    Bgrx,
    /// `yuyv422`: 4 bytes per 2 pixels (Y0 U Y1 V).
    Yuyv,
}

impl PixelFormat {
    pub fn bytes_per_row(self, w: u32) -> u32 {
        match self {
            PixelFormat::Bgrx => w * 4,
            PixelFormat::Yuyv => w * 2,
        }
    }

    fn mpv_name(self) -> &'static str {
        match self {
            PixelFormat::Bgrx => "bgr0",
            PixelFormat::Yuyv => "yuyv422",
        }
    }
}

/// The newest frame.
#[derive(Default)]
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub format: PixelFormat,
    /// YUV matrix of a `Yuyv` frame: BT.709 (else BT.601), limited range. mpv tags its YUV
    /// output the way `mp_image_params_guess_csp` does: BT.709 from 1280 wide or 577 high.
    pub bt709: bool,
    pub data: Vec<u8>,
    pub fresh: bool,
}

/// One entry of mpv's `track-list`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MpvTrack {
    pub id: i64,
    /// `video`, `audio` or `sub`.
    pub kind: String,
    /// Stream index in the container (`ff-index`); `None` for external files.
    pub ff_index: Option<i64>,
    pub external: bool,
    /// The file or URL of an external track.
    pub external_filename: String,
}

/// The size mpv renders at: the video's display size in the YUV pipeline (the GPU scales it),
/// the box's size in the BGRA one. Always within 4K and even wide (4:2:2 pairs pixels).
pub fn render_size(pipeline: Pipeline, video: (u32, u32), widget: (u32, u32)) -> (u32, u32) {
    let (w, h) = match pipeline {
        Pipeline::Yuv if video.0 > 0 && video.1 > 0 => {
            // Keep the aspect ratio when a huge video is brought within 4K.
            let s = (MAX_W as f64 / video.0 as f64)
                .min(MAX_H as f64 / video.1 as f64)
                .min(1.0);
            (
                (video.0 as f64 * s).round() as u32,
                (video.1 as f64 * s).round() as u32,
            )
        }
        _ => widget,
    };
    (w.clamp(16, MAX_W) & !1, h.clamp(16, MAX_H))
}

pub(crate) enum Cmd {
    Command(Vec<String>),
    /// Reads string properties (empty string when unavailable).
    Get(Vec<String>, tokio::sync::oneshot::Sender<Vec<String>>),
}

/// The running mpv instance. Dropping it stops the worker thread.
pub struct Mpv {
    cmd: mpsc::Sender<Cmd>,
    pub state: Arc<Mutex<MpvState>>,
    pub frame: Arc<Mutex<Frame>>,
    /// Size (physical pixels) of the widget's box.
    pub target: Arc<(AtomicU32, AtomicU32)>,
    pub renders: Arc<AtomicU64>,
    pub pipeline: Pipeline,
    /// Called when `state` changes.
    pub on_state: Arc<dyn Fn(MpvState) + Send + Sync>,
    stop: Arc<AtomicBool>,
}

impl Mpv {
    pub fn start(on_state: Arc<dyn Fn(MpvState) + Send + Sync>) -> Result<Self, String> {
        let api = Api::load()?;
        let pipeline = Pipeline::from_env();
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(MpvState::default()));
        let frame = Arc::new(Mutex::new(Frame::default()));
        let target = Arc::new((AtomicU32::new(1280), AtomicU32::new(720)));
        let renders = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Shared {
            state: state.clone(),
            frame: frame.clone(),
            target: target.clone(),
            renders: renders.clone(),
            stop: stop.clone(),
            on_state: on_state.clone(),
            pipeline,
        };
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        std::thread::Builder::new()
            .name("orphyn-mpv".into())
            .stack_size(64 << 20)
            .spawn(move || {
                // Capture the whole struct: it keeps the Library loaded for the thread's lifetime
                // (capturing only some fields dropped it and unmapped libmpv: spike A2 trap).
                let api = api;
                worker(api, rx, shared, ready_tx);
            })
            .map_err(|e| e.to_string())?;
        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())??;
        Ok(Self {
            cmd: tx,
            state,
            frame,
            target,
            renders,
            pipeline,
            on_state,
            stop,
        })
    }

    pub fn command(&self, args: &[&str]) {
        let _ = self
            .cmd
            .send(Cmd::Command(args.iter().map(|s| s.to_string()).collect()));
    }

    /// Reads properties as strings, in order (empty when unavailable).
    pub async fn get(&self, names: &[&str]) -> Vec<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let n = names.len();
        if self.cmd.send(Cmd::Get(names, tx)).is_err() {
            return vec![String::new(); n];
        }
        rx.await.unwrap_or_else(|_| vec![String::new(); n])
    }

    /// mpv's `track-list` (ids to select with `aid` / `sid`).
    pub async fn tracks(&self) -> Vec<MpvTrack> {
        let count = self.get(&["track-list/count"]).await[0]
            .parse::<usize>()
            .unwrap_or(0);
        let mut names = Vec::new();
        for i in 0..count {
            for f in ["id", "type", "ff-index", "external", "external-filename"] {
                names.push(format!("track-list/{i}/{f}"));
            }
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let v = self.get(&refs).await;
        v.chunks(5)
            .filter_map(|c| {
                Some(MpvTrack {
                    id: c[0].parse().ok()?,
                    kind: c[1].clone(),
                    ff_index: c[2].parse().ok(),
                    external: c[3] == "yes",
                    external_filename: c[4].clone(),
                })
            })
            .collect()
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
    }
}

struct Shared {
    state: Arc<Mutex<MpvState>>,
    frame: Arc<Mutex<Frame>>,
    target: Arc<(AtomicU32, AtomicU32)>,
    renders: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    on_state: Arc<dyn Fn(MpvState) + Send + Sync>,
    pipeline: Pipeline,
}

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

fn worker(api: Api, rx: Receiver<Cmd>, sh: Shared, ready: mpsc::Sender<Result<(), String>>) {
    // SAFETY: every call below follows the libmpv C API contract: the handle is created and
    // destroyed on this thread, strings are NUL-terminated and outlive the calls, and the render
    // parameters point at live locals.
    unsafe {
        let hnd = (api.create)();
        if hnd.is_null() {
            let _ = ready.send(Err("mpv_create failed".into()));
            return;
        }
        let set = |k: &str, v: &str| {
            let (k, v) = (cstr(k), cstr(v));
            (api.set_option_string)(hnd, k.as_ptr(), v.as_ptr());
        };
        set("vo", "libmpv");
        set(
            "hwdec",
            &std::env::var("ORPHYN_HWDEC").unwrap_or_else(|_| "auto-copy".into()),
        );
        set("audio-display", "no");
        set("terminal", "no");
        set("keep-open", "yes");
        set("idle", "yes");
        // The controls are the app's own: no mpv OSD drawn into the frames.
        set("osd-level", "0");
        set("osd-on-seek", "no");
        // Subtitles like `.jr-video-player__video::cue`: title-3 text on a dark box.
        set("sub-font", "Inter");
        set("sub-font-size", "44");
        set("sub-border-style", "background-box");
        set("sub-back-color", "0.0/0.0/0.0/0.55");
        set("sub-margin-y", "48");
        if (api.initialize)(hnd) < 0 {
            let _ = ready.send(Err("mpv_initialize failed".into()));
            return;
        }
        let mut ctx: *mut c_void = std::ptr::null_mut();
        let sw = cstr("sw");
        let mut params = [
            Param {
                ty: RENDER_PARAM_API_TYPE,
                data: sw.as_ptr() as *mut c_void,
            },
            Param {
                ty: 0,
                data: std::ptr::null_mut(),
            },
        ];
        if (api.render_create)(&mut ctx, hnd, params.as_mut_ptr()) < 0 {
            let _ = ready.send(Err("mpv_render_context_create(sw) failed".into()));
            (api.terminate_destroy)(hnd);
            return;
        }
        let _ = ready.send(Ok(()));
        let command = |a: &[String]| {
            let owned: Vec<CString> = a.iter().map(|s| cstr(s)).collect();
            let mut v: Vec<*const c_char> = owned.iter().map(|x| x.as_ptr()).collect();
            v.push(std::ptr::null());
            (api.command)(hnd, v.as_mut_ptr());
        };
        let get_f64 = |name: &str| -> f64 {
            let n = cstr(name);
            let mut v: f64 = 0.0;
            if (api.get_property)(
                hnd,
                n.as_ptr(),
                FMT_DOUBLE,
                &mut v as *mut f64 as *mut c_void,
            ) < 0
            {
                0.0
            } else {
                v
            }
        };
        let get_flag = |name: &str| -> bool {
            let n = cstr(name);
            let mut v: c_int = 0;
            (api.get_property)(
                hnd,
                n.as_ptr(),
                FMT_FLAG,
                &mut v as *mut c_int as *mut c_void,
            );
            v != 0
        };
        let get_str = |name: &str| -> String {
            let n = cstr(name);
            let mut p: *mut c_char = std::ptr::null_mut();
            if (api.get_property)(
                hnd,
                n.as_ptr(),
                FMT_STRING,
                &mut p as *mut *mut c_char as *mut c_void,
            ) < 0
                || p.is_null()
            {
                return String::new();
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            (api.free)(p as *mut c_void);
            s
        };
        let error_text = |code: c_int| -> String {
            let p = (api.error_string)(code);
            if p.is_null() {
                format!("mpv error {code}")
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        };
        let mut format = match sh.pipeline {
            Pipeline::Yuv => PixelFormat::Yuyv,
            Pipeline::Bgra => PixelFormat::Bgrx,
        };
        let mut fmt = cstr(format.mpv_name());
        let mut buf: Vec<u8> = Vec::new();
        let mut last_stats = Instant::now() - Duration::from_secs(1);
        let (mut loaded, mut ended) = (false, false);
        let mut error: Option<String> = None;
        let mut video = (0u32, 0u32);
        let mut loads = 0u64;
        while !sh.stop.load(Relaxed) {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Cmd::Command(a) => {
                        if a.first().map(String::as_str) == Some("loadfile") {
                            loaded = false;
                            ended = false;
                            error = None;
                            video = (0, 0);
                            loads += 1;
                            // Force a state push right after the load.
                            last_stats = Instant::now() - Duration::from_secs(1);
                        }
                        command(&a);
                    }
                    Cmd::Get(names, reply) => {
                        let _ = reply.send(names.iter().map(|n| get_str(n)).collect());
                    }
                }
            }
            loop {
                let ev = (api.wait_event)(hnd, 0.0) as *const Event;
                if ev.is_null() || (*ev).id == 0 {
                    break;
                }
                match (*ev).id {
                    EVENT_FILE_LOADED => loaded = true,
                    EVENT_END_FILE if !(*ev).data.is_null() => {
                        let end = &*((*ev).data as *const EndFile);
                        match end.reason {
                            END_FILE_REASON_EOF => ended = true,
                            END_FILE_REASON_ERROR => error = Some(error_text(end.error)),
                            // Stopped / replaced by the next loadfile: not an end.
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            if (api.render_update)(ctx) & RENDER_UPDATE_FRAME != 0 {
                if sh.pipeline == Pipeline::Yuv && video == (0, 0) {
                    video = (
                        get_f64("dwidth").max(0.0) as u32,
                        get_f64("dheight").max(0.0) as u32,
                    );
                }
                let widget = (sh.target.0.load(Relaxed), sh.target.1.load(Relaxed));
                let (w, h) = render_size(sh.pipeline, video, widget);
                let row = format.bytes_per_row(w) as usize;
                buf.resize(row * h as usize, 0);
                let mut size = [w as c_int, h as c_int];
                let mut stride: usize = row;
                let mut p = [
                    Param {
                        ty: RENDER_PARAM_SW_SIZE,
                        data: size.as_mut_ptr() as *mut c_void,
                    },
                    Param {
                        ty: RENDER_PARAM_SW_FORMAT,
                        data: fmt.as_ptr() as *mut c_void,
                    },
                    Param {
                        ty: RENDER_PARAM_SW_STRIDE,
                        data: &mut stride as *mut usize as *mut c_void,
                    },
                    Param {
                        ty: RENDER_PARAM_SW_POINTER,
                        data: buf.as_mut_ptr() as *mut c_void,
                    },
                    Param {
                        ty: 0,
                        data: std::ptr::null_mut(),
                    },
                ];
                let rc = (api.render)(ctx, p.as_mut_ptr());
                if rc >= 0 {
                    if sh.pipeline == Pipeline::Bgra {
                        // bgr0 leaves alpha undefined: force opaque (the YUV pipeline's shader
                        // ignores it instead).
                        for px in buf.as_chunks_mut::<4>().0 {
                            px[3] = 255;
                        }
                    }
                    let mut f = sh.frame.lock().unwrap_or_else(|p| p.into_inner());
                    std::mem::swap(&mut f.data, &mut buf);
                    f.w = w;
                    f.h = h;
                    f.format = format;
                    f.bt709 = w >= 1280 || h > 576;
                    f.fresh = true;
                    sh.renders.fetch_add(1, Relaxed);
                } else if format == PixelFormat::Yuyv {
                    log::warn!(
                        "[mpv] yuyv422 output refused ({}), using bgr0",
                        error_text(rc)
                    );
                    format = PixelFormat::Bgrx;
                    fmt = cstr(format.mpv_name());
                }
            }
            if last_stats.elapsed() >= Duration::from_millis(200) {
                last_stats = Instant::now();
                let next = MpvState {
                    position: get_f64("time-pos"),
                    duration: get_f64("duration"),
                    buffered: get_f64("demuxer-cache-time"),
                    paused: get_flag("pause"),
                    idle: get_flag("idle-active"),
                    ended: ended || (loaded && get_flag("eof-reached")),
                    loaded,
                    buffering: loaded && (get_flag("paused-for-cache") || get_flag("seeking")),
                    loads,
                    error: error.clone(),
                    hwdec: get_str("hwdec-current"),
                };
                if loaded && sh.pipeline == Pipeline::Yuv {
                    // The display size can change mid-file (new aspect, resolution switch).
                    video = (
                        get_f64("dwidth").max(0.0) as u32,
                        get_f64("dheight").max(0.0) as u32,
                    );
                }
                let changed = {
                    let mut s = sh.state.lock().unwrap_or_else(|p| p.into_inner());
                    let changed = *s != next;
                    *s = next.clone();
                    changed
                };
                if changed {
                    (sh.on_state)(next);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        (api.render_free)(ctx);
        (api.terminate_destroy)(hnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yuv_pipeline_renders_at_video_size_even_wide() {
        assert_eq!(
            render_size(Pipeline::Yuv, (1920, 1080), (2560, 1440)),
            (1920, 1080)
        );
        assert_eq!(
            render_size(Pipeline::Yuv, (1279, 534), (800, 600)),
            (1278, 534)
        );
        // Unknown video size yet: the box.
        assert_eq!(render_size(Pipeline::Yuv, (0, 0), (800, 600)), (800, 600));
    }

    #[test]
    fn huge_videos_keep_their_aspect_within_4k() {
        assert_eq!(
            render_size(Pipeline::Yuv, (7680, 4320), (800, 600)),
            (3840, 2160)
        );
        assert_eq!(
            render_size(Pipeline::Yuv, (4096, 1716), (800, 600)),
            (3840, 1609)
        );
    }

    #[test]
    fn bgra_pipeline_renders_at_box_size() {
        assert_eq!(
            render_size(Pipeline::Bgra, (1920, 1080), (2560, 1440)),
            (2560, 1440)
        );
        assert_eq!(render_size(Pipeline::Bgra, (1920, 1080), (5, 5)), (16, 16));
    }

    #[test]
    fn yuyv_rows_are_half_the_bgra_ones() {
        assert_eq!(PixelFormat::Yuyv.bytes_per_row(1920), 3840);
        assert_eq!(PixelFormat::Bgrx.bytes_per_row(1920), 7680);
    }
}
