//! Real audio playback: the track is downloaded, decoded by rodio (symphonia) and played on the
//! default output device. Position / state are shared with the UI through atomics.
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub enum Cmd {
    Load(String),
    Toggle,
    Seek(f64),
    Volume(f32),
}

#[derive(Default)]
pub struct Shared {
    pub position_ms: AtomicU64,
    pub playing: AtomicBool,
    pub loading: AtomicBool,
    pub finished: AtomicBool,
}

#[derive(Clone)]
pub struct Player {
    tx: mpsc::Sender<Cmd>,
    pub shared: Arc<Shared>,
}

impl Player {
    pub fn spawn() -> Player {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let s = shared.clone();
        std::thread::spawn(move || run(rx, s));
        Player { tx, shared }
    }
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
    pub fn position(&self) -> f64 {
        self.shared.position_ms.load(Ordering::Relaxed) as f64 / 1000.0
    }
    pub fn playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }
    pub fn loading(&self) -> bool {
        self.shared.loading.load(Ordering::Relaxed)
    }
    pub fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Relaxed)
    }
}

fn run(rx: mpsc::Receiver<Cmd>, shared: Arc<Shared>) {
    let stream: OutputStream = match OutputStreamBuilder::open_default_stream() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[audio] no output device: {e}");
            return;
        }
    };
    let mut sink: Option<Sink> = None;
    let mut volume = 1.0f32;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Cmd::Load(url)) => {
                shared.loading.store(true, Ordering::Relaxed);
                shared.finished.store(false, Ordering::Relaxed);
                if let Some(s) = sink.take() {
                    s.stop();
                }
                let bytes = reqwest::blocking::get(&url)
                    .and_then(|r| r.error_for_status())
                    .and_then(|r| r.bytes());
                match bytes
                    .map_err(|e| e.to_string())
                    .and_then(|b| Decoder::new(Cursor::new(b.to_vec())).map_err(|e| e.to_string()))
                {
                    Ok(dec) => {
                        let s = Sink::connect_new(stream.mixer());
                        s.set_volume(volume);
                        s.append(dec);
                        sink = Some(s);
                        shared.playing.store(true, Ordering::Relaxed);
                    }
                    Err(e) => eprintln!("[audio] load failed: {e}"),
                }
                shared.loading.store(false, Ordering::Relaxed);
            }
            Ok(Cmd::Toggle) => {
                if let Some(s) = &sink {
                    if s.is_paused() {
                        s.play();
                    } else {
                        s.pause();
                    }
                    shared.playing.store(!s.is_paused(), Ordering::Relaxed);
                }
            }
            Ok(Cmd::Seek(t)) => {
                if let Some(s) = &sink {
                    let _ = s.try_seek(Duration::from_secs_f64(t.max(0.0)));
                }
            }
            Ok(Cmd::Volume(v)) => {
                volume = v;
                if let Some(s) = &sink {
                    s.set_volume(v);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
        if let Some(s) = &sink {
            shared.position_ms.store(s.get_pos().as_millis() as u64, Ordering::Relaxed);
            if s.empty() {
                shared.finished.store(true, Ordering::Relaxed);
                shared.playing.store(false, Ordering::Relaxed);
            }
        }
    }
}
