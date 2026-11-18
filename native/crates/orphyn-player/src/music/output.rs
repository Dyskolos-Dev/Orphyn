//! The audio thread: owns the output device and one rodio `Sink`. Tracks are downloaded on a
//! helper thread, decoded by symphonia and appended to the sink; the next track is appended
//! before the current one ends, which makes the hand-over gapless. Reports through `on_event`.
use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

pub(crate) enum Msg {
    /// Replace whatever plays with `url` (generation `gen`).
    Load {
        gen: u64,
        url: String,
        start_at: f64,
        autoplay: bool,
    },
    /// Download `url` and queue it after the current track (gapless).
    Preload { gen: u64, url: String },
    Play,
    Pause,
    Seek(f64),
    Volume(f32),
    Stop,
    Fetched {
        gen: u64,
        next: bool,
        result: Result<Vec<u8>, String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Loaded { gen: u64, duration: Option<f64> },
    /// The preloaded track `gen` started (the previous one ended).
    Advanced { gen: u64, duration: Option<f64> },
    Position(f64),
    Ended,
    Failed { gen: u64, message: String },
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

type Decoded = (Decoder<Cursor<Vec<u8>>>, Option<f64>);

fn decode(bytes: Vec<u8>) -> Result<Decoded, String> {
    let d = Decoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let duration = d.total_duration().map(|t| t.as_secs_f64());
    Ok((d, duration))
}

struct Thread {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
    volume: f32,
    current: u64,
    pending: Option<(u64, f64, bool)>,
    preload: Option<u64>,
    /// Generation and duration of the preloaded source queued after the current one.
    queued: Option<(u64, Option<f64>)>,
    ended_sent: bool,
    last_position: Instant,
}

fn run(rx: Receiver<Msg>, tx: Sender<Msg>, on_event: impl Fn(Event)) {
    let mut t = Thread {
        stream: None,
        sink: None,
        volume: 1.0,
        current: 0,
        pending: None,
        preload: None,
        queued: None,
        ended_sent: true,
        last_position: Instant::now(),
    };
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
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
            let mut s = OutputStreamBuilder::open_default_stream().map_err(|e| e.to_string())?;
            s.log_on_drop(false);
            self.stream = Some(s);
        }
        Ok(self.stream.as_ref().expect("stream just opened"))
    }

    fn handle(&mut self, msg: Msg, tx: &Sender<Msg>, on_event: &impl Fn(Event)) {
        match msg {
            Msg::Load {
                gen,
                url,
                start_at,
                autoplay,
            } => {
                if let Some(s) = self.sink.take() {
                    s.stop();
                }
                self.current = gen;
                self.pending = Some((gen, start_at, autoplay));
                self.preload = None;
                self.queued = None;
                self.ended_sent = true;
                fetch(url, gen, false, tx.clone());
            }
            Msg::Preload { gen, url } => {
                self.preload = Some(gen);
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
                if let Some(p) = &mut self.pending {
                    p.2 = true;
                }
            }
            Msg::Pause => {
                if let Some(s) = &self.sink {
                    s.pause();
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
                if let Some(s) = &self.sink {
                    s.set_volume(v);
                }
            }
            Msg::Stop => {
                if let Some(s) = self.sink.take() {
                    s.stop();
                }
                self.current += 1;
                self.pending = None;
                self.preload = None;
                self.queued = None;
                self.ended_sent = true;
            }
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
        let decoded = result.and_then(decode);
        let (source, duration) = match decoded {
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
        self.ended_sent = false;
        on_event(Event::Loaded { gen, duration });
    }

    fn on_preloaded(&mut self, gen: u64, result: Result<Vec<u8>, String>) {
        if self.preload != Some(gen) {
            return;
        }
        self.preload = None;
        let Some(sink) = &self.sink else { return };
        match result.and_then(decode) {
            Ok((source, duration)) => {
                sink.append(source);
                self.queued = Some((gen, duration));
            }
            // The controller loads it normally when the current track ends.
            Err(e) => log::warn!("[audio] preload failed: {e}"),
        }
    }

    fn tick(&mut self, on_event: &impl Fn(Event)) {
        let Some(sink) = &self.sink else { return };
        if let Some((gen, duration)) = self.queued {
            if sink.len() <= 1 {
                // The current source finished and the queued one took over.
                self.queued = None;
                self.current = gen;
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
