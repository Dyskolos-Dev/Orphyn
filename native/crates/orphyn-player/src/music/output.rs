//! The audio thread: owns the output device and one rodio `Sink`. Tracks are downloaded on a
//! helper thread, decoded by symphonia and appended to the sink. The hand-over to the next track
//! is either gapless (the preloaded source is appended to the same sink) or a crossfade (the
//! preloaded source starts on a second sink while the first fades out, with equal-power gains).
//! Reports through `on_event`.
use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, Sink, Source};

use super::crossfade::fade_gains;
use super::devices::open_stream;

pub(crate) enum Msg {
    /// Replace whatever plays with `url` (generation `gen`).
    Load {
        gen: u64,
        url: String,
        start_at: f64,
        autoplay: bool,
    },
    /// Download `url` and hand over to it after the current track: gapless when `fade` is 0,
    /// else a crossfade of `fade` seconds. `current_duration` is the playing track's duration.
    Preload {
        gen: u64,
        url: String,
        fade: f64,
        current_duration: f64,
    },
    Play,
    Pause,
    Seek(f64),
    Volume(f32),
    /// The output device to use (`None` = system default); playback moves there at once.
    Device(Option<String>),
    Stop,
    Fetched {
        gen: u64,
        next: bool,
        result: Result<Vec<u8>, String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Loaded {
        gen: u64,
        duration: Option<f64>,
    },
    /// The preloaded track `gen` started (the previous one ended, or began fading out).
    Advanced {
        gen: u64,
        duration: Option<f64>,
    },
    Position(f64),
    Ended,
    Failed {
        gen: u64,
        message: String,
    },
}

pub(crate) fn spawn(on_event: impl Fn(Event) + Send + 'static) -> Sender<Msg> {
    let (tx, rx) = mpsc::channel();
    let fetch_tx = tx.clone();
    std::thread::Builder::new()
        .name("orphyn-audio".into())
        .spawn(move || run(rx, fetch_tx, on_event))
        .expect("spawn the audio thread");
    tx
}

fn fetch(url: String, gen: u64, next: bool, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let result = reqwest::blocking::Client::new()
            .get(&url)
            .timeout(Duration::from_secs(120))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map(|b| b.to_vec())
            .map_err(|e| e.without_url().to_string());
        let _ = tx.send(Msg::Fetched { gen, next, result });
    });
}

type Bytes = Arc<[u8]>;
type Source0 = Decoder<Cursor<Bytes>>;

fn decode(bytes: Bytes) -> Result<(Source0, Option<f64>), String> {
    let d = Decoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let duration = d.total_duration().map(|t| t.as_secs_f64());
    Ok((d, duration))
}

/// A preloaded track waiting for its crossfade.
struct Staged {
    gen: u64,
    bytes: Bytes,
    source: Source0,
    duration: Option<f64>,
    fade: f64,
    current_duration: f64,
}

/// The previous track fading out on its own sink.
struct Fading {
    out: Sink,
    /// Seconds of fade done (does not advance while paused).
    done: f64,
    secs: f64,
    last: Instant,
}

struct Thread {
    stream: Option<OutputStream>,
    device: Option<String>,
    sink: Option<Sink>,
    volume: f32,
    current: u64,
    current_bytes: Option<Bytes>,
    /// Duration of the playing source (seconds), 0 when unknown.
    current_duration: f64,
    pending: Option<(u64, f64, bool)>,
    /// Generation, fade and playing duration of the preload asked for.
    preload: Option<(u64, f64, f64)>,
    /// Gapless: generation, duration and bytes of the source queued after the current one.
    queued: Option<(u64, Option<f64>, Bytes)>,
    staged: Option<Staged>,
    fading: Option<Fading>,
    ended_sent: bool,
    last_position: Instant,
}

fn run(rx: Receiver<Msg>, tx: Sender<Msg>, on_event: impl Fn(Event)) {
    let mut t = Thread {
        stream: None,
        device: None,
        sink: None,
        volume: 1.0,
        current: 0,
        current_bytes: None,
        current_duration: 0.0,
        pending: None,
        preload: None,
        queued: None,
        staged: None,
        fading: None,
        ended_sent: true,
        last_position: Instant::now(),
    };
    loop {
        // Short steps while fading keep the gain ramp smooth.
        let wait = if t.fading.is_some() { 15 } else { 50 };
        match rx.recv_timeout(Duration::from_millis(wait)) {
            Ok(msg) => t.handle(msg, &tx, &on_event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        t.tick(&on_event);
    }
}

impl Thread {
    fn output(&mut self) -> Result<&OutputStream, String> {
        if self.stream.is_none() {
            self.stream = Some(open_stream(self.device.as_deref())?);
        }
        Ok(self.stream.as_ref().expect("stream just opened"))
    }

    fn drop_sinks(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        if let Some(f) = self.fading.take() {
            f.out.stop();
        }
        self.staged = None;
    }

    fn handle(&mut self, msg: Msg, tx: &Sender<Msg>, on_event: &impl Fn(Event)) {
        match msg {
            Msg::Load {
                gen,
                url,
                start_at,
                autoplay,
            } => {
                self.drop_sinks();
                self.current = gen;
                self.pending = Some((gen, start_at, autoplay));
                self.preload = None;
                self.queued = None;
                self.ended_sent = true;
                fetch(url, gen, false, tx.clone());
            }
            Msg::Preload {
                gen,
                url,
                fade,
                current_duration,
            } => {
                self.preload = Some((gen, fade, current_duration));
                fetch(url, gen, true, tx.clone());
            }
            Msg::Fetched { gen, next, result } => {
                if next {
                    self.on_preloaded(gen, result);
                } else {
                    self.on_loaded(gen, result, on_event);
                }
            }
            Msg::Play => {
                if let Some(s) = &self.sink {
                    s.play();
                }
                if let Some(f) = &mut self.fading {
                    f.out.play();
                    f.last = Instant::now();
                }
                if let Some(p) = &mut self.pending {
                    p.2 = true;
                }
            }
            Msg::Pause => {
                if let Some(s) = &self.sink {
                    s.pause();
                }
                if let Some(f) = &self.fading {
                    f.out.pause();
                }
                if let Some(p) = &mut self.pending {
                    p.2 = false;
                }
            }
            Msg::Seek(secs) => {
                if let Some(s) = &self.sink {
                    let _ = s.try_seek(Duration::from_secs_f64(secs.max(0.0)));
                    on_event(Event::Position(s.get_pos().as_secs_f64()));
                } else if let Some(p) = &mut self.pending {
                    p.1 = secs;
                }
            }
            Msg::Volume(v) => {
                self.volume = v;
                if self.fading.is_none() {
                    if let Some(s) = &self.sink {
                        s.set_volume(v);
                    }
                }
            }
            Msg::Device(name) => {
                if self.device != name {
                    self.device = name;
                    self.move_to_device(on_event);
                }
            }
            Msg::Stop => {
                self.drop_sinks();
                self.current += 1;
                self.current_bytes = None;
                self.pending = None;
                self.preload = None;
                self.queued = None;
                self.ended_sent = true;
            }
        }
    }

    /// Reopens the stream on the new device and resumes the playing track where it was. A
    /// gapless-queued next track is dropped: the controller loads it when this one ends.
    fn move_to_device(&mut self, on_event: &impl Fn(Event)) {
        let state = self
            .sink
            .as_ref()
            .map(|s| (s.get_pos().as_secs_f64(), s.is_paused()));
        self.drop_sinks();
        self.queued = None;
        self.stream = None;
        let (Some((pos, paused)), Some(bytes)) = (state, self.current_bytes.clone()) else {
            return;
        };
        let gen = self.current;
        let result = decode(bytes).and_then(|(source, _)| {
            self.output()
                .map(|s| (source, Sink::connect_new(s.mixer())))
        });
        match result {
            Ok((source, sink)) => {
                sink.set_volume(self.volume);
                sink.append(source);
                let _ = sink.try_seek(Duration::from_secs_f64(pos));
                if paused {
                    sink.pause();
                }
                self.sink = Some(sink);
            }
            Err(message) => on_event(Event::Failed { gen, message }),
        }
    }

    fn on_loaded(&mut self, gen: u64, result: Result<Vec<u8>, String>, on_event: &impl Fn(Event)) {
        let Some((pending_gen, start_at, autoplay)) = self.pending else {
            return;
        };
        if gen != pending_gen || gen != self.current {
            return;
        }
        self.pending = None;
        let bytes: Bytes = match result {
            Ok(b) => b.into(),
            Err(message) => return on_event(Event::Failed { gen, message }),
        };
        let (source, duration) = match decode(bytes.clone()) {
            Ok(d) => d,
            Err(message) => return on_event(Event::Failed { gen, message }),
        };
        let volume = self.volume;
        let sink = match self.output() {
            Ok(stream) => Sink::connect_new(stream.mixer()),
            Err(message) => return on_event(Event::Failed { gen, message }),
        };
        sink.set_volume(volume);
        sink.append(source);
        if start_at > 0.0 {
            let _ = sink.try_seek(Duration::from_secs_f64(start_at));
        }
        if !autoplay {
            sink.pause();
        }
        self.sink = Some(sink);
        self.current_bytes = Some(bytes);
        self.current_duration = duration.unwrap_or(0.0);
        self.ended_sent = false;
        on_event(Event::Loaded { gen, duration });
    }

    fn on_preloaded(&mut self, gen: u64, result: Result<Vec<u8>, String>) {
        let Some((want, fade, current_duration)) = self.preload else {
            return;
        };
        if want != gen {
            return;
        }
        self.preload = None;
        let Some(sink) = &self.sink else { return };
        let bytes: Bytes = match result {
            Ok(b) => b.into(),
            // The controller loads it normally when the current track ends.
            Err(e) => return log::warn!("[audio] preload failed: {e}"),
        };
        match decode(bytes.clone()) {
            Ok((source, duration)) if fade > 0.0 => {
                self.staged = Some(Staged {
                    gen,
                    bytes,
                    source,
                    duration,
                    fade,
                    current_duration,
                });
            }
            Ok((source, duration)) => {
                sink.append(source);
                self.queued = Some((gen, duration, bytes));
            }
            Err(e) => log::warn!("[audio] preload failed: {e}"),
        }
    }

    /// Starts the staged track on a second sink; the playing one fades out beside it.
    fn start_staged(&mut self, on_event: &impl Fn(Event)) {
        let Some(st) = self.staged.take() else { return };
        let Ok(stream) = self.output() else { return };
        let new = Sink::connect_new(stream.mixer());
        new.set_volume(0.0);
        new.append(st.source);
        let old = self.sink.replace(new);
        if let Some(old) = old {
            if old.is_paused() {
                if let Some(n) = &self.sink {
                    n.pause();
                }
            }
            if let Some(prev) = self.fading.take() {
                prev.out.stop();
            }
            self.fading = Some(Fading {
                out: old,
                done: 0.0,
                secs: st.fade,
                last: Instant::now(),
            });
        }
        self.current = st.gen;
        self.current_bytes = Some(st.bytes);
        self.current_duration = st.duration.unwrap_or(0.0);
        self.ended_sent = false;
        on_event(Event::Advanced {
            gen: st.gen,
            duration: st.duration,
        });
        on_event(Event::Position(0.0));
        self.last_position = Instant::now();
    }

    fn tick_fade(&mut self) {
        let volume = self.volume;
        let Some(f) = &mut self.fading else { return };
        let now = Instant::now();
        if !f.out.is_paused() {
            f.done += now.duration_since(f.last).as_secs_f64();
        }
        f.last = now;
        let p = f.done / f.secs;
        if p >= 1.0 {
            if let Some(f) = self.fading.take() {
                f.out.stop();
            }
            if let Some(s) = &self.sink {
                s.set_volume(volume);
            }
        } else {
            let (inn, out) = fade_gains(p);
            f.out.set_volume(out * volume);
            if let Some(s) = &self.sink {
                s.set_volume(inn * volume);
            }
        }
    }

    fn tick(&mut self, on_event: &impl Fn(Event)) {
        self.tick_fade();
        let Some(sink) = &self.sink else { return };
        if let Some(st) = &self.staged {
            let dur = if self.current_duration > 0.0 {
                self.current_duration
            } else {
                st.current_duration
            };
            let left = dur - sink.get_pos().as_secs_f64();
            // Also when the playing source ended early (unknown duration).
            if sink.empty() || (dur > 0.0 && left <= st.fade) {
                self.start_staged(on_event);
            }
        }
        let Some(sink) = &self.sink else { return };
        if let Some((gen, duration, _)) = self.queued {
            if sink.len() <= 1 {
                // The current source finished and the queued one took over.
                if let Some((_, _, bytes)) = self.queued.take() {
                    self.current_bytes = Some(bytes);
                }
                self.current = gen;
                self.current_duration = duration.unwrap_or(0.0);
                on_event(Event::Advanced { gen, duration });
            }
        }
        if sink.empty() {
            if !self.ended_sent {
                self.ended_sent = true;
                on_event(Event::Ended);
            }
            return;
        }
        if !sink.is_paused() && self.last_position.elapsed() >= Duration::from_millis(250) {
            self.last_position = Instant::now();
            on_event(Event::Position(sink.get_pos().as_secs_f64()));
        }
    }
}
