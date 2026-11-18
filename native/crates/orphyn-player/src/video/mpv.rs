//! libmpv through its software render API, loaded at run time (from spike A2). A worker thread
//! owns the handle, renders BGRA frames at the size the widget asks for and swaps them into a
//! shared slot; the widget uploads the newest one into a wgpu texture. Hardware decoding
//! (`auto-copy`) feeds the software renderer.
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

const FMT_STRING: c_int = 1;
const FMT_FLAG: c_int = 3;
const FMT_DOUBLE: c_int = 5;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const RENDER_PARAM_API_TYPE: c_int = 1;
const RENDER_PARAM_SW_SIZE: c_int = 17;
const RENDER_PARAM_SW_FORMAT: c_int = 18;
const RENDER_PARAM_SW_STRIDE: c_int = 19;
const RENDER_PARAM_SW_POINTER: c_int = 20;
const RENDER_UPDATE_FRAME: u64 = 1;

struct Api {
    _lib: Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    get_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
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
    pub hwdec: String,
}

/// The newest frame, BGRA, `w * h * 4` bytes.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
    pub fresh: bool,
}

pub(crate) enum Cmd {
    Command(Vec<String>),
}

/// The running mpv instance. Dropping it stops the worker thread.
pub struct Mpv {
    cmd: mpsc::Sender<Cmd>,
    pub state: Arc<Mutex<MpvState>>,
    pub frame: Arc<Mutex<Frame>>,
    /// Size (physical pixels) the widget wants the next frame rendered at.
    pub target: Arc<(AtomicU32, AtomicU32)>,
    pub renders: Arc<AtomicU64>,
    /// Bumped when `state` changes, for listeners.
    pub on_state: Arc<dyn Fn(MpvState) + Send + Sync>,
    stop: Arc<AtomicBool>,
}

impl Mpv {
    pub fn start(on_state: Arc<dyn Fn(MpvState) + Send + Sync>) -> Result<Self, String> {
        let api = Api::load()?;
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(MpvState::default()));
        let frame = Arc::new(Mutex::new(Frame {
            w: 0,
            h: 0,
            data: Vec::new(),
            fresh: false,
        }));
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
            on_state,
            stop,
        })
    }

    pub fn command(&self, args: &[&str]) {
        let _ = self
            .cmd
            .send(Cmd::Command(args.iter().map(|s| s.to_string()).collect()));
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
        let fmt = cstr("bgr0");
        let mut buf: Vec<u8> = Vec::new();
        let mut last_stats = Instant::now() - Duration::from_secs(1);
        let (mut loaded, mut ended) = (false, false);
        while !sh.stop.load(Relaxed) {
            while let Ok(Cmd::Command(a)) = rx.try_recv() {
                if a.first().map(String::as_str) == Some("loadfile") {
                    loaded = false;
                    ended = false;
                }
                command(&a);
            }
            loop {
                let ev = (api.wait_event)(hnd, 0.0) as *const c_int;
                if ev.is_null() || *ev == 0 {
                    break;
                }
                match *ev {
                    EVENT_FILE_LOADED => loaded = true,
                    EVENT_END_FILE => ended = true,
                    _ => {}
                }
            }
            if (api.render_update)(ctx) & RENDER_UPDATE_FRAME != 0 {
                let w = sh.target.0.load(Relaxed).clamp(16, 3840);
                let h = sh.target.1.load(Relaxed).clamp(16, 2160);
                buf.resize((w * h * 4) as usize, 0);
                let mut size = [w as c_int, h as c_int];
                let mut stride: usize = (w * 4) as usize;
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
                if (api.render)(ctx, p.as_mut_ptr()) >= 0 {
                    // bgr0 leaves alpha undefined: force opaque.
                    for px in buf.as_chunks_mut::<4>().0 {
                        px[3] = 255;
                    }
                    let mut f = sh.frame.lock().unwrap_or_else(|p| p.into_inner());
                    std::mem::swap(&mut f.data, &mut buf);
                    f.w = w;
                    f.h = h;
                    f.fresh = true;
                    sh.renders.fetch_add(1, Relaxed);
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
                    ended: ended || get_flag("eof-reached"),
                    loaded,
                    hwdec: get_str("hwdec-current"),
                };
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
