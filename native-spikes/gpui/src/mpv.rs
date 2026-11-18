//! libmpv through its software render API, loaded at run time (libloading: no link-time dependency).
//! A worker thread owns the mpv handle, renders each new frame into a BGRA buffer and publishes it;
//! the UI turns it into a GPUI `RenderImage`, so the video is an ordinary GPUI element and the
//! control bar is painted *over* it by GPUI itself.
use anyhow::{anyhow, Result};
use gpui::RenderImage;
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

type Handle = *mut c_void;
#[repr(C)]
struct Param {
    ty: c_int,
    data: *mut c_void,
}
const FMT_FLAG: c_int = 3;
const FMT_DOUBLE: c_int = 5;
const P_API_TYPE: c_int = 1;
const P_SW_SIZE: c_int = 17;
const P_SW_FORMAT: c_int = 18;
const P_SW_STRIDE: c_int = 19;
const P_SW_POINTER: c_int = 20;

struct Api {
    _lib: Library,
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    get_property: unsafe extern "C" fn(Handle, *const c_char, c_int, *mut c_void) -> c_int,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *mut c_void,
    terminate_destroy: unsafe extern "C" fn(Handle),
    render_create: unsafe extern "C" fn(*mut *mut c_void, Handle, *mut Param) -> c_int,
    render_update: unsafe extern "C" fn(*mut c_void) -> u64,
    render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    render_free: unsafe extern "C" fn(*mut c_void),
}
unsafe impl Send for Api {}

impl Api {
    fn load() -> Result<Self> {
        let names: &[&str] = if cfg!(windows) { &["libmpv-2.dll", "mpv-2.dll", "mpv-1.dll"] } else { &["libmpv.so.2", "libmpv.so.1", "libmpv.so"] };
        let dir = std::env::var("ORPHYN_MPV_DIR").ok();
        let mut last = None;
        for n in names {
            let cand = dir.as_ref().map(|d| format!("{d}/{n}")).unwrap_or_else(|| n.to_string());
            match unsafe { Library::new(&cand) } {
                Ok(lib) => unsafe {
                    macro_rules! sym {
                        ($n:literal) => {
                            *lib.get($n).map_err(|e| anyhow!("{e}"))?
                        };
                    }
                    return Ok(Api {
                        create: sym!(b"mpv_create"),
                        initialize: sym!(b"mpv_initialize"),
                        set_option_string: sym!(b"mpv_set_option_string"),
                        command: sym!(b"mpv_command"),
                        get_property: sym!(b"mpv_get_property"),
                        wait_event: sym!(b"mpv_wait_event"),
                        terminate_destroy: sym!(b"mpv_terminate_destroy"),
                        render_create: sym!(b"mpv_render_context_create"),
                        render_update: sym!(b"mpv_render_context_update"),
                        render: sym!(b"mpv_render_context_render"),
                        render_free: sym!(b"mpv_render_context_free"),
                        _lib: lib,
                    });
                },
                Err(e) => last = Some(e),
            }
        }
        Err(anyhow!("libmpv not found ({:?}); set ORPHYN_MPV_DIR", last))
    }
}

#[derive(Clone, Copy, Default)]
pub struct State {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
}

enum Cmd {
    Pause,
    SeekAbs(f64),
}

pub struct Player {
    cmd: mpsc::Sender<Cmd>,
    state: Arc<Mutex<State>>,
    frame: Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>,
    stop: Arc<AtomicBool>,
}

impl Player {
    pub fn start(url: &str, w: u32, h: u32) -> Result<Self> {
        let api = Api::load()?;
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(State::default()));
        let frame = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (st, fr, sp, url) = (state.clone(), frame.clone(), stop.clone(), url.to_string());
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        std::thread::spawn(move || unsafe {
            let hnd = (api.create)();
            let set = |k: &str, v: &str| {
                let (k, v) = (CString::new(k).unwrap(), CString::new(v).unwrap());
                (api.set_option_string)(hnd, k.as_ptr(), v.as_ptr());
            };
            set("vo", "libmpv");
            set("hwdec", "no");
            set("audio-display", "no");
            set("terminal", "no");
            if (api.initialize)(hnd) < 0 {
                let _ = ready_tx.send(Err("mpv_initialize failed".into()));
                return;
            }
            let mut ctx: *mut c_void = std::ptr::null_mut();
            let sw = CString::new("sw").unwrap();
            let mut params = [Param { ty: P_API_TYPE, data: sw.as_ptr() as *mut c_void }, Param { ty: 0, data: std::ptr::null_mut() }];
            if (api.render_create)(&mut ctx, hnd, params.as_mut_ptr()) < 0 {
                let _ = ready_tx.send(Err("mpv_render_context_create(sw) failed".into()));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            let args: Vec<CString> = ["loadfile", url.as_str()].iter().map(|s| CString::new(*s).unwrap()).collect();
            let mut argv: Vec<*const c_char> = args.iter().map(|a| a.as_ptr()).collect();
            argv.push(std::ptr::null());
            (api.command)(hnd, argv.as_mut_ptr());
            let fmt = CString::new("bgr0").unwrap();
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let getd = |name: &str| -> f64 {
                let n = CString::new(name).unwrap();
                let mut v: f64 = 0.;
                if (api.get_property)(hnd, n.as_ptr(), FMT_DOUBLE, &mut v as *mut f64 as *mut c_void) < 0 {
                    0.
                } else {
                    v
                }
            };
            while !sp.load(Ordering::Relaxed) {
                while let Ok(c) = rx.try_recv() {
                    let a: Vec<String> = match c {
                        Cmd::Pause => vec!["cycle".into(), "pause".into()],
                        Cmd::SeekAbs(s) => vec!["seek".into(), format!("{s}"), "absolute".into()],
                    };
                    let a: Vec<CString> = a.iter().map(|s| CString::new(s.as_str()).unwrap()).collect();
                    let mut v: Vec<*const c_char> = a.iter().map(|x| x.as_ptr()).collect();
                    v.push(std::ptr::null());
                    (api.command)(hnd, v.as_mut_ptr());
                }
                // drain the event queue (mpv expects it to be read)
                loop {
                    // MPV_EVENT_NONE = 0 ends the drain; the event struct starts with the id
                    let ev = (api.wait_event)(hnd, 0.) as *const c_int;
                    if ev.is_null() || *ev == 0 {
                        break;
                    }
                }
                let flags = (api.render_update)(ctx);
                if flags & 1 != 0 {
                    let mut size = [w as c_int, h as c_int];
                    let mut stride: usize = (w * 4) as usize;
                    let mut p = [
                        Param { ty: P_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
                        Param { ty: P_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
                        Param { ty: P_SW_STRIDE, data: &mut stride as *mut usize as *mut c_void },
                        Param { ty: P_SW_POINTER, data: buf.as_mut_ptr() as *mut c_void },
                        Param { ty: 0, data: std::ptr::null_mut() },
                    ];
                    if (api.render)(ctx, p.as_mut_ptr()) >= 0 {
                        for px in buf.chunks_exact_mut(4) {
                            px[3] = 255; // bgr0 leaves alpha undefined; GPUI wants BGRA
                        }
                        *fr.lock().unwrap() = Some((w, h, buf.clone()));
                    }
                }
                let mut s = st.lock().unwrap();
                s.position = getd("time-pos");
                s.duration = getd("duration");
                let mut pf: c_int = 0;
                let n = CString::new("pause").unwrap();
                (api.get_property)(hnd, n.as_ptr(), FMT_FLAG, &mut pf as *mut c_int as *mut c_void);
                s.paused = pf != 0;
                drop(s);
                std::thread::sleep(Duration::from_millis(4));
            }
            (api.render_free)(ctx);
            (api.terminate_destroy)(hnd);
        });
        ready_rx.recv_timeout(Duration::from_secs(10)).map_err(|e| anyhow!("{e}"))?.map_err(|e| anyhow!(e))?;
        Ok(Self { cmd: tx, state, frame, stop })
    }
    pub fn state(&self) -> State {
        *self.state.lock().unwrap()
    }
    pub fn toggle_pause(&self) {
        let _ = self.cmd.send(Cmd::Pause);
    }
    pub fn seek_frac(&self, f: f32) {
        let d = self.state().duration;
        if d > 0. {
            let _ = self.cmd.send(Cmd::SeekAbs(d * f as f64));
        }
    }
    /// Newest frame since the last call, as a GPUI image.
    pub fn take_frame(&self) -> Option<Arc<RenderImage>> {
        let (w, h, data) = self.frame.lock().unwrap().take()?;
        let img = image::RgbaImage::from_raw(w, h, data)?;
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(img)])))
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
