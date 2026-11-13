//! Legacy AirPlay (HTTP video, port 7000): `POST /play` with `Content-Location`,
//! `GET /playback-info`, `POST /rate`, `/scrub`, `/stop`. Works on many Apple TVs (and some
//! AirPlay-video TVs/receivers) that do not require a pairing PIN. There is no remote volume
//! control in this protocol. AirPlay 2 audio (HomePod, AirPlay 2 speakers) needs HomeKit pairing
//! and encrypted RTSP and is NOT supported.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use tauri::async_runtime::{spawn, JoinHandle};

use super::model::{CastMedia, CastStatus};
use super::transport::{BoxFut, CastTransport, TResult, TransportHandlers, TransportUpdate};

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackInfo {
    pub rate: f64,
    pub position: f64,
    pub duration: f64,
    pub ready_to_play: bool,
}

/// Extracts the few fields we need from the `/playback-info` XML plist (no plist dependency).
pub fn parse_playback_info(plist: &str) -> PlaybackInfo {
    let num = |key: &str| -> f64 {
        let needle = format!("<key>{key}</key>");
        let Some(at) = plist.find(&needle) else {
            return 0.0;
        };
        let rest = plist[at + needle.len()..].trim_start();
        let value = ["<real>", "<integer>"]
            .iter()
            .find_map(|open| rest.strip_prefix(open))
            .and_then(|v| v.split('<').next());
        value
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let ready = plist.find("<key>readyToPlay</key>").is_some_and(|at| {
        let rest = plist[at + "<key>readyToPlay</key>".len()..].trim_start();
        rest.starts_with("<true/>") || rest.starts_with("<true />")
    });
    PlaybackInfo {
        rate: num("rate"),
        position: num("position"),
        duration: num("duration"),
        ready_to_play: ready,
    }
}

/// Body of `POST /play` (text/parameters).
pub fn build_play_body(url: &str, start_fraction: f64) -> String {
    format!("Content-Location: {url}\nStart-Position: {start_fraction}\n")
}

struct HttpResult {
    status: u16,
    body: String,
}

#[derive(Default)]
struct Flags {
    handlers: Option<TransportHandlers>,
    timer: Option<JoinHandle<()>>,
    polling: bool,
    failures: u32,
    closed: bool,
    saw_playing: bool,
    ended: bool,
    last_rate: f64,
}

pub struct AirplayTransport {
    host: String,
    port: u16,
    session: String,
    flags: Mutex<Flags>,
    me: Weak<AirplayTransport>,
}

impl AirplayTransport {
    pub fn new(host: &str, port: u16) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            host: host.to_string(),
            port,
            session: uuid::Uuid::new_v4().to_string(),
            flags: Mutex::new(Flags {
                last_rate: 1.0,
                ..Default::default()
            }),
            me: me.clone(),
        })
    }

    fn flags(&self) -> MutexGuard<'_, Flags> {
        self.flags.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn request(&self, method: &str, path: &str, body: Option<String>) -> TResult<HttpResult> {
        let host = if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let url = format!("http://{host}:{}{path}", self.port);
        let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
        let mut req = super::http::client()
            .request(method, url)
            .timeout(Duration::from_secs(5))
            .header("User-Agent", "MediaControl/1.0")
            .header("X-Apple-Session-ID", &self.session);
        if let Some(b) = body {
            req = req.header("Content-Type", "text/parameters").body(b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("AirPlay {path} failed: {e}"))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("AirPlay {path} failed: {e}"))?;
        Ok(HttpResult { status, body })
    }

    async fn ok(&self, method: &str, path: &str, body: Option<String>) -> TResult<HttpResult> {
        let res = self.request(method, path, body).await?;
        if res.status >= 400 {
            return Err(format!("AirPlay {path} failed with HTTP {}", res.status));
        }
        Ok(res)
    }

    fn start_polling(&self) {
        self.stop_polling();
        let Some(me) = self.me.upgrade() else { return };
        let handle = spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                me.poll().await;
            }
        });
        self.flags().timer = Some(handle);
    }

    fn stop_polling(&self) {
        if let Some(h) = self.flags().timer.take() {
            h.abort();
        }
    }

    pub async fn poll(&self) {
        {
            let mut f = self.flags();
            if f.polling || f.closed {
                return;
            }
            f.polling = true;
        }
        let res = self.request("GET", "/playback-info", None).await;
        match res {
            Ok(res) => {
                let info = parse_playback_info(&res.body);
                let mut update = TransportUpdate {
                    position_sec: Some(info.position),
                    duration_sec: Some(info.duration),
                    ..Default::default()
                };
                let mut ended_now = false;
                let handlers = {
                    let mut f = self.flags();
                    f.polling = false;
                    if f.closed {
                        return;
                    }
                    f.failures = 0;
                    if info.rate > 0.0 {
                        f.saw_playing = true;
                        f.last_rate = info.rate;
                        update.status = Some(CastStatus::Playing);
                    } else if info.ready_to_play || info.duration > 0.0 {
                        update.status = Some(CastStatus::Paused);
                    } else if f.saw_playing && !f.ended {
                        // playback-info has no item any more: the item finished.
                        f.ended = true;
                        ended_now = true;
                    }
                    f.handlers.clone()
                };
                if ended_now {
                    self.stop_polling();
                    if let Some(h) = handlers {
                        (h.on_update)(TransportUpdate::ended());
                    }
                    return;
                }
                if let Some(h) = handlers {
                    (h.on_update)(update);
                }
            }
            Err(err) => {
                let (dead, failures) = {
                    let mut f = self.flags();
                    f.polling = false;
                    if f.closed {
                        return;
                    }
                    f.failures += 1;
                    (f.failures >= 5, f.failures)
                };
                log::warn!("[cast] airplay poll failed ({failures}/5): {err}");
                if dead {
                    self.stop_polling();
                    let handlers = self.flags().handlers.clone();
                    if let Some(h) = handlers {
                        (h.on_closed)(Some(err));
                    }
                }
            }
        }
    }
}

impl CastTransport for AirplayTransport {
    fn set_handlers(&self, handlers: TransportHandlers) {
        self.flags().handlers = Some(handlers);
    }

    fn connect(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.flags().closed = false;
            let res = self.request("GET", "/server-info", None).await?;
            if res.status >= 400 {
                return Err(format!(
                    "AirPlay server-info failed with HTTP {}",
                    res.status
                ));
            }
            Ok(())
        })
    }

    fn load(&self, media: CastMedia) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.stop_polling();
            {
                let mut f = self.flags();
                f.saw_playing = false;
                f.ended = false;
            }
            // Start-Position is a fraction (0..1) of the duration.
            let frac = match (media.start_position_sec, media.duration_sec) {
                (Some(s), Some(d)) if s > 0.0 && d > 0.0 => (s / d).min(0.999),
                _ => 0.0,
            };
            self.ok("POST", "/play", Some(build_play_body(&media.url, frac)))
                .await?;
            self.start_polling();
            Ok(())
        })
    }

    fn play(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            let rate = self.flags().last_rate;
            let rate = if rate > 0.0 { rate } else { 1.0 };
            self.ok("POST", &format!("/rate?value={rate}"), None)
                .await?;
            Ok(())
        })
    }

    fn pause(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.ok("POST", "/rate?value=0", None).await?;
            Ok(())
        })
    }

    fn seek(&self, position_sec: f64) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.ok("POST", &format!("/scrub?position={position_sec}"), None)
                .await?;
            Ok(())
        })
    }

    fn stop(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.flags().ended = true;
            self.stop_polling();
            self.ok("POST", "/stop", None).await?;
            Ok(())
        })
    }

    fn set_volume(&self, _volume: f64) -> BoxFut<'_, ()> {
        Box::pin(async { Err("Volume control is not available over legacy AirPlay".to_string()) })
    }

    fn set_muted(&self, _muted: bool) -> BoxFut<'_, ()> {
        Box::pin(async { Err("Mute is not available over legacy AirPlay".to_string()) })
    }

    fn close(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            {
                let mut f = self.flags();
                f.closed = true;
                f.handlers = None;
            }
            self.stop_polling();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_playback_info_plists() {
        let plist = "<plist><dict><key>duration</key><real>201.5</real><key>position</key><real>12.25</real><key>rate</key><real>1.0</real><key>readyToPlay</key><true/></dict></plist>";
        assert_eq!(
            parse_playback_info(plist),
            PlaybackInfo {
                rate: 1.0,
                position: 12.25,
                duration: 201.5,
                ready_to_play: true
            }
        );
        assert_eq!(
            parse_playback_info("<plist><dict/></plist>"),
            PlaybackInfo {
                rate: 0.0,
                position: 0.0,
                duration: 0.0,
                ready_to_play: false
            }
        );
    }

    #[test]
    fn builds_the_play_body() {
        assert_eq!(
            build_play_body("http://h/a.mp4", 0.5),
            "Content-Location: http://h/a.mp4\nStart-Position: 0.5\n"
        );
    }

    #[tokio::test]
    async fn volume_and_mute_are_rejected_with_a_clear_message() {
        let t = AirplayTransport::new("127.0.0.1", 1);
        assert!(t
            .set_volume(0.5)
            .await
            .unwrap_err()
            .contains("legacy AirPlay"));
        assert!(t
            .set_muted(true)
            .await
            .unwrap_err()
            .contains("legacy AirPlay"));
    }
}
