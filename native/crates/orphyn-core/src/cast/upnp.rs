//! UPnP/DLNA transport: AVTransport + RenderingControl over SOAP, with position polling and
//! ended detection (also drives WiiM / LinkPlay streamers).

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use super::runtime::{spawn, JoinHandle};

use super::didl::{build_didl, format_upnp_time, parse_upnp_time};
use super::model::{CastMedia, CastStatus};
use super::soap::{
    arg, default_soap, SoapArg, SoapCall, SoapFn, SoapResult, AVTRANSPORT_SERVICE,
    RENDERING_CONTROL_SERVICE,
};
use super::transport::{BoxFut, CastTransport, TransportHandlers, TransportUpdate};

#[derive(Debug, Clone)]
pub struct UpnpEndpoints {
    pub av_transport_control_url: String,
    pub rendering_control_url: Option<String>,
}

pub struct UpnpOptions {
    pub soap: SoapFn,
    pub poll_interval_ms: u64,
    /// Consecutive poll failures before the link is considered dead.
    pub max_poll_failures: u32,
}

impl Default for UpnpOptions {
    fn default() -> Self {
        Self {
            soap: default_soap(),
            poll_interval_ms: 1000,
            max_poll_failures: 5,
        }
    }
}

/// A UPnP TransportState as the session sees it ('stopped' = STOPPED / NO_MEDIA_PRESENT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapped {
    Status(CastStatus),
    Stopped,
}

pub fn map_transport_state(state: &str) -> Option<Mapped> {
    match state.to_uppercase().as_str() {
        "PLAYING" => Some(Mapped::Status(CastStatus::Playing)),
        "PAUSED_PLAYBACK" | "PAUSED_RECORDING" => Some(Mapped::Status(CastStatus::Paused)),
        "TRANSITIONING" => Some(Mapped::Status(CastStatus::Buffering)),
        "STOPPED" | "NO_MEDIA_PRESENT" => Some(Mapped::Stopped),
        _ => None,
    }
}

#[derive(Default)]
struct Flags {
    handlers: Option<TransportHandlers>,
    timer: Option<JoinHandle<()>>,
    polling: bool,
    failures: u32,
    closed: bool,
    /// True once the device reported PLAYING for the current media: only then STOPPED means "ended".
    saw_playing: bool,
    ended: bool,
    tick: u64,
}

pub struct UpnpTransport {
    endpoints: UpnpEndpoints,
    soap: SoapFn,
    poll_ms: u64,
    max_failures: u32,
    flags: Mutex<Flags>,
    me: Weak<UpnpTransport>,
}

impl UpnpTransport {
    pub fn new(endpoints: UpnpEndpoints, opts: UpnpOptions) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            endpoints,
            soap: opts.soap,
            poll_ms: opts.poll_interval_ms.max(1),
            max_failures: opts.max_poll_failures.max(1),
            flags: Mutex::new(Flags::default()),
            me: me.clone(),
        })
    }

    fn flags(&self) -> MutexGuard<'_, Flags> {
        self.flags.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn handlers(&self) -> Option<TransportHandlers> {
        self.flags().handlers.clone()
    }

    fn av(
        &self,
        action: &'static str,
        args: Vec<SoapArg>,
    ) -> impl std::future::Future<Output = SoapResult> {
        let mut all = vec![arg("InstanceID", 0)];
        all.extend(args);
        (self.soap)(SoapCall {
            control_url: self.endpoints.av_transport_control_url.clone(),
            service_type: AVTRANSPORT_SERVICE,
            action,
            args: all,
        })
    }

    fn rc(
        &self,
        action: &'static str,
        args: Vec<SoapArg>,
    ) -> impl std::future::Future<Output = SoapResult> {
        let url = self.endpoints.rendering_control_url.clone();
        let soap = self.soap.clone();
        async move {
            let Some(control_url) = url else {
                return Err("Device has no RenderingControl service".to_string());
            };
            let mut all = vec![arg("InstanceID", 0)];
            all.extend(args);
            soap(SoapCall {
                control_url,
                service_type: RENDERING_CONTROL_SERVICE,
                action,
                args: all,
            })
            .await
        }
    }

    fn start_polling(&self) {
        self.stop_polling();
        let Some(me) = self.me.upgrade() else { return };
        let every = Duration::from_millis(self.poll_ms);
        let handle = spawn(async move {
            let mut ticker = tokio::time::interval(every);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ticker.tick().await; // the first tick fires immediately
            loop {
                ticker.tick().await;
                me.poll().await;
            }
        });
        let mut f = self.flags();
        f.failures = 0;
        f.tick = 0;
        f.timer = Some(handle);
    }

    fn stop_polling(&self) {
        if let Some(h) = self.flags().timer.take() {
            h.abort();
        }
    }

    /// One poll cycle; public for tests (drives the ended/closed logic without timers).
    pub async fn poll(&self) {
        {
            let mut f = self.flags();
            if f.polling || f.closed {
                return;
            }
            f.polling = true;
        }
        let result = async {
            let info = self.av("GetTransportInfo", vec![]).await?;
            let pos = self.av("GetPositionInfo", vec![]).await?;
            Ok::<_, String>((info, pos))
        }
        .await;
        match result {
            Ok((info, pos)) => self.on_poll_ok(&info, &pos).await,
            Err(err) => {
                let (closed, failures) = {
                    let mut f = self.flags();
                    f.polling = false;
                    if f.closed {
                        return;
                    }
                    f.failures += 1;
                    (f.failures >= self.max_failures, f.failures)
                };
                log::warn!(
                    "[cast] poll failed ({failures}/{}): {err}",
                    self.max_failures
                );
                if closed {
                    self.stop_polling();
                    if let Some(h) = self.handlers() {
                        (h.on_closed)(Some(err));
                    }
                }
            }
        }
    }

    async fn on_poll_ok(
        &self,
        info: &std::collections::HashMap<String, String>,
        pos: &std::collections::HashMap<String, String>,
    ) {
        let mapped =
            map_transport_state(info.get("CurrentTransportState").map_or("", String::as_str));
        let mut update = TransportUpdate {
            position_sec: Some(parse_upnp_time(pos.get("RelTime").map(String::as_str))),
            duration_sec: Some(parse_upnp_time(
                pos.get("TrackDuration").map(String::as_str),
            )),
            ..Default::default()
        };
        let mut ended_now = false;
        let (handlers, volume_due) = {
            let mut f = self.flags();
            f.polling = false;
            if f.closed {
                return;
            }
            f.failures = 0;
            match mapped {
                Some(Mapped::Stopped) => {
                    if f.saw_playing && !f.ended {
                        f.ended = true;
                        ended_now = true;
                    }
                }
                Some(Mapped::Status(s)) => {
                    if s == CastStatus::Playing {
                        f.saw_playing = true;
                    }
                    update.status = Some(s);
                }
                None => {}
            }
            let mut volume_due = false;
            if !ended_now {
                f.tick += 1;
                volume_due = f.tick.is_multiple_of(5);
            }
            (f.handlers.clone(), volume_due)
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
        if volume_due {
            self.poll_volume().await;
        }
    }

    async fn poll_volume(&self) {
        if self.endpoints.rendering_control_url.is_none() {
            return;
        }
        // Volume control is optional on some renderers: failures are ignored.
        let Ok(vol) = self.rc("GetVolume", vec![arg("Channel", "Master")]).await else {
            return;
        };
        let Ok(mute) = self.rc("GetMute", vec![arg("Channel", "Master")]).await else {
            return;
        };
        let (closed, handlers) = {
            let f = self.flags();
            (f.closed, f.handlers.clone())
        };
        if closed {
            return;
        }
        let level = vol
            .get("CurrentVolume")
            .and_then(|v| v.trim().parse::<f64>().ok());
        let muted = mute
            .get("CurrentMute")
            .map(|m| m == "1" || m.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if let Some(h) = handlers {
            (h.on_update)(TransportUpdate {
                volume: level
                    .filter(|l| l.is_finite())
                    .map(|l| (l / 100.0).clamp(0.0, 1.0)),
                muted: Some(muted),
                ..Default::default()
            });
        }
    }
}

impl CastTransport for UpnpTransport {
    fn set_handlers(&self, handlers: TransportHandlers) {
        self.flags().handlers = Some(handlers);
    }

    fn connect(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.av("GetTransportInfo", vec![]).await?;
            self.flags().closed = false;
            // Initial volume, best effort.
            if let Some(me) = self.me.upgrade() {
                spawn(async move { me.poll_volume().await });
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
            // Some renderers refuse a new URI while playing; stop first and ignore a failure.
            let _ = self.av("Stop", vec![]).await;
            self.av(
                "SetAVTransportURI",
                vec![
                    arg("CurrentURI", &media.url),
                    arg("CurrentURIMetaData", build_didl(&media)),
                ],
            )
            .await?;
            self.av("Play", vec![arg("Speed", 1)]).await?;
            if let Some(start) = media.start_position_sec.filter(|s| *s > 1.0) {
                // Renderers accept Seek only once playing; failure is not fatal.
                let r = self
                    .av(
                        "Seek",
                        vec![
                            arg("Unit", "REL_TIME"),
                            arg("Target", format_upnp_time(start)),
                        ],
                    )
                    .await;
                if let Err(e) = r {
                    log::warn!("[cast] initial seek failed: {e}");
                }
            }
            self.start_polling();
            Ok(())
        })
    }

    fn play(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.av("Play", vec![arg("Speed", 1)]).await?;
            Ok(())
        })
    }

    fn pause(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.av("Pause", vec![]).await?;
            Ok(())
        })
    }

    fn seek(&self, position_sec: f64) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.av(
                "Seek",
                vec![
                    arg("Unit", "REL_TIME"),
                    arg("Target", format_upnp_time(position_sec)),
                ],
            )
            .await?;
            Ok(())
        })
    }

    fn stop(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            // A user stop is not an "ended": flag it before the device reports STOPPED.
            {
                let mut f = self.flags();
                f.saw_playing = false;
                f.ended = true;
            }
            self.stop_polling();
            self.av("Stop", vec![]).await?;
            Ok(())
        })
    }

    fn set_volume(&self, volume: f64) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.rc(
                "SetVolume",
                vec![
                    arg("Channel", "Master"),
                    arg("DesiredVolume", (volume * 100.0).round() as i64),
                ],
            )
            .await?;
            Ok(())
        })
    }

    fn set_muted(&self, muted: bool) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.rc(
                "SetMute",
                vec![
                    arg("Channel", "Master"),
                    arg("DesiredMute", u8::from(muted)),
                ],
            )
            .await?;
            Ok(())
        })
    }

    fn close(&self) -> BoxFut<'_, ()> {
        Box::pin(async move {
            {
                let mut f = self.flags();
                f.closed = true;
                f.handlers = None;
            }
            self.stop_polling();
            Ok::<(), String>(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cast::model::test_media;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type Calls = Arc<Mutex<Vec<SoapCall>>>;
    type Updates = Arc<Mutex<Vec<TransportUpdate>>>;
    type Made = (Arc<UpnpTransport>, Calls, Updates, Arc<Mutex<u32>>);

    fn mock_soap(states: Vec<&'static str>, fail: bool) -> (SoapFn, Calls) {
        let calls: Calls = Arc::default();
        let log = calls.clone();
        let i = Arc::new(AtomicUsize::new(0));
        let f: SoapFn = Arc::new(move |c: SoapCall| {
            log.lock().unwrap().push(c.clone());
            let states = states.clone();
            let i = i.clone();
            Box::pin(async move {
                if fail {
                    return Err("ECONNREFUSED".to_string());
                }
                let mut out = HashMap::new();
                match c.action {
                    "GetTransportInfo" => {
                        let n = i
                            .fetch_add(1, Ordering::SeqCst)
                            .min(states.len().saturating_sub(1));
                        out.insert("CurrentTransportState".into(), states[n].to_string());
                    }
                    "GetPositionInfo" => {
                        out.insert("RelTime".into(), "0:00:07".into());
                        out.insert("TrackDuration".into(), "0:03:00".into());
                    }
                    _ => {}
                }
                Ok(out)
            })
        });
        (f, calls)
    }

    fn endpoints() -> UpnpEndpoints {
        UpnpEndpoints {
            av_transport_control_url: "http://x/av".into(),
            rendering_control_url: Some("http://x/rc".into()),
        }
    }

    fn make(states: Vec<&'static str>) -> Made {
        let (soap, calls) = mock_soap(states, false);
        let t = UpnpTransport::new(
            endpoints(),
            UpnpOptions {
                soap,
                poll_interval_ms: 1000,
                max_poll_failures: 2,
            },
        );
        let updates: Arc<Mutex<Vec<TransportUpdate>>> = Arc::default();
        let closed: Arc<Mutex<u32>> = Arc::default();
        let (u, c) = (updates.clone(), closed.clone());
        t.set_handlers(TransportHandlers {
            on_update: Arc::new(move |x| u.lock().unwrap().push(x)),
            on_closed: Arc::new(move |_| *c.lock().unwrap() += 1),
        });
        (t, calls, updates, closed)
    }

    #[tokio::test]
    async fn sends_set_av_transport_uri_with_didl_then_play_on_load() {
        let (t, calls, _, _) = make(vec!["PLAYING"]);
        t.load(test_media()).await.unwrap();
        t.close().await.unwrap();
        let calls = calls.lock().unwrap();
        let actions: Vec<_> = calls.iter().map(|c| c.action).collect();
        assert_eq!(actions, vec!["Stop", "SetAVTransportURI", "Play"]);
        let meta = &calls[1]
            .args
            .iter()
            .find(|(k, _)| *k == "CurrentURIMetaData")
            .unwrap()
            .1;
        assert!(meta.contains("<DIDL-Lite"));
    }

    #[tokio::test]
    async fn reports_ended_only_when_stopped_follows_playing() {
        let (t, _, updates, _) = make(vec!["STOPPED", "PLAYING", "STOPPED"]);
        t.poll().await; // STOPPED before ever playing: initial state, not an end
        assert!(!updates.lock().unwrap().iter().any(|u| u.ended));
        t.poll().await; // PLAYING
        let last = updates.lock().unwrap().last().cloned().unwrap();
        assert_eq!(
            (last.status, last.position_sec, last.duration_sec),
            (Some(CastStatus::Playing), Some(7.0), Some(180.0))
        );
        t.poll().await; // STOPPED after playing
        assert_eq!(
            updates.lock().unwrap().last().cloned().unwrap(),
            TransportUpdate::ended()
        );
        t.poll().await;
        assert_eq!(
            updates.lock().unwrap().iter().filter(|u| u.ended).count(),
            1
        );
    }

    #[tokio::test]
    async fn does_not_report_ended_after_a_user_stop() {
        let (t, _, updates, _) = make(vec!["PLAYING", "STOPPED"]);
        t.poll().await;
        t.stop().await.unwrap();
        t.poll().await;
        assert!(!updates.lock().unwrap().iter().any(|u| u.ended));
    }

    #[tokio::test]
    async fn reports_a_closed_link_after_repeated_poll_failures() {
        let (soap, _) = mock_soap(vec![], true);
        let t = UpnpTransport::new(
            endpoints(),
            UpnpOptions {
                soap,
                poll_interval_ms: 1000,
                max_poll_failures: 2,
            },
        );
        let closed: Arc<Mutex<u32>> = Arc::default();
        let c = closed.clone();
        t.set_handlers(TransportHandlers {
            on_update: Arc::new(|_| {}),
            on_closed: Arc::new(move |_| *c.lock().unwrap() += 1),
        });
        t.poll().await;
        assert_eq!(*closed.lock().unwrap(), 0);
        t.poll().await;
        assert_eq!(*closed.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn maps_volume_to_0_100_and_mute_to_0_1() {
        let (t, calls, _, _) = make(vec!["PLAYING"]);
        t.set_volume(0.5).await.unwrap();
        t.set_muted(true).await.unwrap();
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls[0].args,
            vec![
                arg("InstanceID", 0),
                arg("Channel", "Master"),
                arg("DesiredVolume", 50)
            ]
        );
        assert_eq!(
            calls[1].args,
            vec![
                arg("InstanceID", 0),
                arg("Channel", "Master"),
                arg("DesiredMute", 1)
            ]
        );
    }

    #[tokio::test]
    async fn volume_calls_fail_cleanly_without_a_rendering_control() {
        let (soap, _) = mock_soap(vec![], false);
        let t = UpnpTransport::new(
            UpnpEndpoints {
                av_transport_control_url: "http://x/av".into(),
                rendering_control_url: None,
            },
            UpnpOptions {
                soap,
                ..Default::default()
            },
        );
        assert!(t
            .set_volume(0.5)
            .await
            .unwrap_err()
            .contains("RenderingControl"));
    }

    #[test]
    fn maps_transport_states() {
        assert_eq!(
            map_transport_state("playing"),
            Some(Mapped::Status(CastStatus::Playing))
        );
        assert_eq!(
            map_transport_state("TRANSITIONING"),
            Some(Mapped::Status(CastStatus::Buffering))
        );
        assert_eq!(
            map_transport_state("NO_MEDIA_PRESENT"),
            Some(Mapped::Stopped)
        );
        assert_eq!(map_transport_state("???"), None);
    }
}
