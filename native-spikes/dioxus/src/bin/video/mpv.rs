//! libmpv through the software render API, loaded at run time. A worker thread owns the handle,
//! renders BGRA frames of the size the UI asked for and swaps them into a shared slot; the Blitz
//! widget uploads the newest one into a wgpu texture. Hardware decoding (`*-copy`) feeds the sw renderer.
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

type Handle = *mut c_void;
#[repr(C)]
struct Param {
    ty: c_int,
    data: *mut c_void,
}
const FMT_STRING: c_int = 1;
const FMT_FLAG: c_int = 3;
const FMT_INT64: c_int = 4;
const FMT_DOUBLE: c_int = 5;

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
unsafe impl Send for Api {}

impl Api {
    fn load() -> Result<Self, String> {
        let names: &[&str] = if cfg!(windows) { &["libmpv-2.dll", "mpv-2.dll"] } else { &["libmpv.so.2", "libmpv.so.1", "libmpv.so"] };
        let dir = std::env::var("ORPHYN_MPV_DIR").ok();
        let mut last = String::new();
        for n in names {
            let cand = dir.as_ref().map(|d| format!("{d}/{n}")).unwrap_or_else(|| n.to_string());
            match unsafe { Library::new(&cand) } {
                Ok(lib) => unsafe {
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
                },
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("libmpv not found ({last}); set ORPHYN_MPV_DIR"))
    }
}

#[derive(Clone, Default)]
pub struct State {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub hwdec: String,
    pub dropped: i64,
    pub decoder_dropped: i64,
    pub vo_delayed: i64,
    pub video_w: i64,
    pub video_h: i64,
}

pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
    pub fresh: bool,
}

enum Cmd {
    Pause,
    SeekAbs(f64),
}

pub struct Player {
    cmd: mpsc::Sender<Cmd>,
    pub state: Arc<Mutex<State>>,
    pub frame: Arc<Mutex<Frame>>,
    /// Size (physical px) the UI wants the next frame rendered at.
    pub target: Arc<(AtomicU32, AtomicU32)>,
    pub renders: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Player {
    pub fn start(url: &str) -> Result<Self, String> {
        let api = Api::load()?;
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(State::default()));
        let frame = Arc::new(Mutex::new(Frame { w: 0, h: 0, data: Vec::new(), fresh: false }));
        let target = Arc::new((AtomicU32::new(1280), AtomicU32::new(720)));
        let renders = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (st, fr, tg, rn, sp, url) = (state.clone(), frame.clone(), target.clone(), renders.clone(), stop.clone(), url.to_string());
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        std::thread::Builder::new().stack_size(64 << 20).spawn(move || unsafe {
            let api = api; // capture the whole struct: keeps the Library loaded for the thread lifetime
            let hnd = (api.create)();
            let set = |k: &str, v: &str| {
                let (k, v) = (CString::new(k).unwrap(), CString::new(v).unwrap());
                (api.set_option_string)(hnd, k.as_ptr(), v.as_ptr());
            };
            set("vo", "libmpv");
            set("hwdec", &std::env::var("ORPHYN_HWDEC").unwrap_or_else(|_| "auto-copy".into()));
            set("audio-display", "no");
            set("terminal", "no");
            set("keep-open", "yes");
            if let Ok(v) = std::env::var("ORPHYN_MPV_OPTS") {
                for kv in v.split(',').filter(|s| !s.is_empty()) {
                    if let Some((k, v)) = kv.split_once('=') {
                        set(k, v);
                    }
                }
            }
            if (api.initialize)(hnd) < 0 {
                let _ = ready_tx.send(Err("mpv_initialize failed".into()));
                return;
            }
            let mut ctx: *mut c_void = std::ptr::null_mut();
            let sw = CString::new("sw").unwrap();
            let mut params = [Param { ty: 1, data: sw.as_ptr() as *mut c_void }, Param { ty: 0, data: std::ptr::null_mut() }];
            if (api.render_create)(&mut ctx, hnd, params.as_mut_ptr()) < 0 {
                let _ = ready_tx.send(Err("mpv_render_context_create(sw) failed".into()));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            let cmd = |a: &[&str]| {
                let a: Vec<CString> = a.iter().map(|s| CString::new(*s).unwrap()).collect();
                let mut v: Vec<*const c_char> = a.iter().map(|x| x.as_ptr()).collect();
                v.push(std::ptr::null());
                (api.command)(hnd, v.as_mut_ptr());
            };
            if std::env::var("ORPHYN_NOLOAD").is_err() { cmd(&["loadfile", url.as_str()]); }
            let fmt = CString::new("bgr0").unwrap();
            let mut buf: Vec<u8> = Vec::new();
            let getd = |name: &str| -> f64 {
                let n = CString::new(name).unwrap();
                let mut v: f64 = 0.;
                if (api.get_property)(hnd, n.as_ptr(), FMT_DOUBLE, &mut v as *mut f64 as *mut c_void) < 0 { 0. } else { v }
            };
            let geti = |name: &str| -> i64 {
                let n = CString::new(name).unwrap();
                let mut v: i64 = 0;
                if (api.get_property)(hnd, n.as_ptr(), FMT_INT64, &mut v as *mut i64 as *mut c_void) < 0 { 0 } else { v }
            };
            let gets = |name: &str| -> String {
                let n = CString::new(name).unwrap();
                let mut p: *mut c_char = std::ptr::null_mut();
                if (api.get_property)(hnd, n.as_ptr(), FMT_STRING, &mut p as *mut *mut c_char as *mut c_void) < 0 || p.is_null() {
                    return String::new();
                }
                let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
                (api.free)(p as *mut c_void);
                s
            };
            let mut last_stats = std::time::Instant::now() - Duration::from_secs(1);
            while !sp.load(Relaxed) {
                while let Ok(c) = rx.try_recv() {
                    match c {
                        Cmd::Pause => cmd(&["cycle", "pause"]),
                        Cmd::SeekAbs(s) => cmd(&["seek", &format!("{s}"), "absolute"]),
                    }
                }
                loop {
                    let ev = (api.wait_event)(hnd, 0.) as *const c_int;
                    if ev.is_null() || *ev == 0 {
                        break;
                    }
                }
                let flags = if std::env::var("ORPHYN_NOUPDATE").is_err() { (api.render_update)(ctx) } else { 0 };
                if flags & 1 != 0 {
                    let (w, h) = (tg.0.load(Relaxed).clamp(16, 3840), tg.1.load(Relaxed).clamp(16, 2160));
                    buf.resize((w * h * 4) as usize, 0);
                    let mut size = [w as c_int, h as c_int];
                    let mut stride: usize = (w * 4) as usize;
                    let mut p = [
                        Param { ty: 17, data: size.as_mut_ptr() as *mut c_void },
                        Param { ty: 18, data: fmt.as_ptr() as *mut c_void },
                        Param { ty: 19, data: &mut stride as *mut usize as *mut c_void },
                        Param { ty: 20, data: buf.as_mut_ptr() as *mut c_void },
                        Param { ty: 0, data: std::ptr::null_mut() },
                    ];
                    if (api.render)(ctx, p.as_mut_ptr()) >= 0 {
                        // bgr0 leaves alpha undefined: force opaque
                        for px in buf.chunks_exact_mut(4) {
                            px[3] = 255;
                        }
                        let mut f = fr.lock().unwrap();
                        std::mem::swap(&mut f.data, &mut buf);
                        f.w = w;
                        f.h = h;
                        f.fresh = true;
                        rn.fetch_add(1, Relaxed);
                    }
                }
                if last_stats.elapsed() >= Duration::from_millis(200) && std::env::var("ORPHYN_NOSTATS").is_err() {
                    last_stats = std::time::Instant::now();
                    let mut pf: c_int = 0;
                    let n = CString::new("pause").unwrap();
                    (api.get_property)(hnd, n.as_ptr(), FMT_FLAG, &mut pf as *mut c_int as *mut c_void);
                    *st.lock().unwrap() = State {
                        position: getd("time-pos"),
                        duration: getd("duration"),
                        paused: pf != 0,
                        hwdec: gets("hwdec-current"),
                        dropped: geti("frame-drop-count"),
                        decoder_dropped: geti("decoder-frame-drop-count"),
                        vo_delayed: geti("vo-delayed-frame-count"),
                        video_w: geti("width"),
                        video_h: geti("height"),
                    };
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            (api.render_free)(ctx);
            (api.terminate_destroy)(hnd);
        }).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())??;
        Ok(Self { cmd: tx, state, frame, target, renders, stop })
    }
    pub fn toggle_pause(&self) {
        let _ = self.cmd.send(Cmd::Pause);
    }
    pub fn seek_frac(&self, f: f64) {
        let d = self.state.lock().unwrap().duration;
        if d > 0. {
            let _ = self.cmd.send(Cmd::SeekAbs(d * f));
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
    }
}
