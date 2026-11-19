//! The server WebSocket (`/socket?api_key=…&deviceId=…`): keep-alive as the server asks
//! (`ForceKeepAlive`), a watchdog that notices a silent dead connection, and reconnection with an
//! exponential backoff. SyncPlay is its only user for now.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::client::encode_component;

#[derive(Debug, Clone, PartialEq)]
pub struct SocketMessage {
    pub message_type: String,
    pub message_id: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketStatus {
    Idle,
    Connecting,
    Open,
    Reconnecting,
    Closed,
}

/// `wss://host/base/socket?api_key=TOKEN&deviceId=ID` from the server's http(s) address.
pub fn socket_url(base_url: &str, token: &str, device_id: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("ws://{base}")
    };
    // `api_key` is what the socket endpoint reads (checked on 10.11), `ApiKey` is not accepted.
    format!(
        "{ws}/socket?api_key={}&deviceId={}",
        encode_component(token),
        encode_component(device_id)
    )
}

/// 1 s, 2 s, 4 s … capped at 30 s, with ±20 % jitter so clients do not reconnect in lockstep.
/// `random` is in [0, 1).
pub fn backoff_delay(attempt: u32, random: f64) -> Duration {
    let base = (1000.0 * 2f64.powi(attempt.min(10) as i32)).min(30_000.0);
    Duration::from_millis((base * (0.8 + random * 0.4)).round() as u64)
}

fn jitter() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(n % 1000) / 1000.0
}

type MessageCb = Arc<dyn Fn(SocketMessage) + Send + Sync>;
type StatusCb = Arc<dyn Fn(SocketStatus) + Send + Sync>;

enum Command {
    Send(String),
    Close,
}

/// Handle to the socket task. Dropping it closes the socket.
pub struct JellyfinSocket {
    tx: mpsc::UnboundedSender<Command>,
    status: Arc<Mutex<SocketStatus>>,
    task: JoinHandle<()>,
}

impl JellyfinSocket {
    /// Connects (and keeps reconnecting) on `runtime`; callbacks are invoked from its threads.
    pub fn connect(
        runtime: &Handle,
        url: String,
        on_message: impl Fn(SocketMessage) + Send + Sync + 'static,
        on_status: impl Fn(SocketStatus) + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let status = Arc::new(Mutex::new(SocketStatus::Idle));
        let task = runtime.spawn(run(url, rx, Arc::new(on_message), Arc::new(on_status), status.clone()));
        Self { tx, status, task }
    }

    pub fn status(&self) -> SocketStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues a message; `false` when the socket is not open.
    pub fn send(&self, message_type: &str, data: Option<Value>) -> bool {
        if self.status() != SocketStatus::Open {
            return false;
        }
        let body = match data {
            Some(d) => json!({ "MessageType": message_type, "Data": d }),
            None => json!({ "MessageType": message_type }),
        };
        self.tx.send(Command::Send(body.to_string())).is_ok()
    }

    /// Closes for good (no reconnection).
    pub fn close(&self) {
        let _ = self.tx.send(Command::Close);
    }
}

impl Drop for JellyfinSocket {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Close);
        // Give the task a moment to say goodbye, then make sure it is gone.
        self.task.abort();
    }
}

fn parse_message(text: &str) -> Option<SocketMessage> {
    let v: Value = serde_json::from_str(text).ok()?;
    let message_type = v.get("MessageType")?.as_str()?.to_string();
    Some(SocketMessage {
        message_type,
        message_id: v.get("MessageId").and_then(Value::as_str).map(String::from),
        data: v.get("Data").cloned(),
    })
}

enum Ended {
    /// `close()` was called.
    Closed,
    /// The link dropped or went silent: reconnect.
    Lost,
}

async fn run(
    url: String,
    mut rx: mpsc::UnboundedReceiver<Command>,
    on_message: MessageCb,
    on_status: StatusCb,
    shared: Arc<Mutex<SocketStatus>>,
) {
    let set = |s: SocketStatus| {
        let mut g = shared.lock().unwrap_or_else(|e| e.into_inner());
        if *g != s {
            *g = s;
            drop(g);
            on_status(s);
        }
    };
    let mut attempt: u32 = 0;
    set(SocketStatus::Connecting);
    loop {
        match session(&url, &mut rx, &on_message, &set, &mut attempt).await {
            Ended::Closed => {
                set(SocketStatus::Closed);
                return;
            }
            Ended::Lost => {
                set(SocketStatus::Reconnecting);
                let delay = backoff_delay(attempt, jitter());
                attempt += 1;
                // A close request during the wait still wins.
                let sleep = tokio::time::sleep(delay);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        () = &mut sleep => break,
                        cmd = rx.recv() => match cmd {
                            Some(Command::Send(_)) => {}
                            Some(Command::Close) | None => {
                                set(SocketStatus::Closed);
                                return;
                            }
                        },
                    }
                }
            }
        }
    }
}

async fn session(
    url: &str,
    rx: &mut mpsc::UnboundedReceiver<Command>,
    on_message: &MessageCb,
    set: &impl Fn(SocketStatus),
    attempt: &mut u32,
) -> Ended {
    let connect = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(url));
    let ws = tokio::select! {
        r = connect => match r {
            Ok(Ok((ws, _))) => ws,
            Ok(Err(e)) => {
                log::warn!("[syncplay] socket connect failed: {e}");
                return Ended::Lost;
            }
            Err(_) => return Ended::Lost,
        },
        () = wait_close(rx) => return Ended::Closed,
    };
    *attempt = 0;
    set(SocketStatus::Open);
    let (mut sink, mut stream) = ws.split();
    // Jellyfin's own clients send a keep-alive at half the timeout the server announces.
    let mut keep_alive = Duration::from_secs(30);
    let mut next_ping = Instant::now() + keep_alive;
    let mut last_heard = Instant::now();
    loop {
        let wake = tokio::time::sleep_until(next_ping.into());
        tokio::select! {
            () = wake => {
                // Nothing heard for 2.5 keep-alive periods (the server answers each): dead link.
                if last_heard.elapsed() > keep_alive.mul_f64(2.5) {
                    let _ = sink.close().await;
                    return Ended::Lost;
                }
                let body = json!({ "MessageType": "KeepAlive" }).to_string();
                if sink.send(Message::text(body)).await.is_err() {
                    return Ended::Lost;
                }
                next_ping = Instant::now() + keep_alive;
            }
            cmd = rx.recv() => match cmd {
                Some(Command::Send(body)) => {
                    if sink.send(Message::text(body)).await.is_err() {
                        return Ended::Lost;
                    }
                }
                Some(Command::Close) | None => {
                    let _ = sink.close().await;
                    return Ended::Closed;
                }
            },
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    last_heard = Instant::now();
                    let Some(msg) = parse_message(text.as_str()) else { continue };
                    if msg.message_type == "ForceKeepAlive" {
                        let seconds = msg.data.as_ref().and_then(Value::as_f64).unwrap_or(0.0);
                        if seconds > 0.0 {
                            keep_alive = Duration::from_secs_f64(seconds / 2.0);
                        }
                        let body = json!({ "MessageType": "KeepAlive" }).to_string();
                        if sink.send(Message::text(body)).await.is_err() {
                            return Ended::Lost;
                        }
                        next_ping = Instant::now() + keep_alive;
                    }
                    on_message(msg);
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return Ended::Lost,
                Some(Ok(_)) => last_heard = Instant::now(),
            },
        }
    }
}

/// Resolves when a close is requested (sends are dropped while connecting).
async fn wait_close(rx: &mut mpsc::UnboundedReceiver<Command>) {
    loop {
        match rx.recv().await {
            Some(Command::Close) | None => return,
            Some(Command::Send(_)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_socket_url() {
        assert_eq!(
            socket_url("https://jf.example/base/", "tok en", "dev-1"),
            "wss://jf.example/base/socket?api_key=tok%20en&deviceId=dev-1"
        );
        assert_eq!(socket_url("http://10.0.0.2:8096", "t", "d"), "ws://10.0.0.2:8096/socket?api_key=t&deviceId=d");
    }

    #[test]
    fn backoff_doubles_to_a_cap_with_jitter() {
        assert_eq!(backoff_delay(0, 0.5), Duration::from_millis(1000));
        assert_eq!(backoff_delay(2, 0.5), Duration::from_millis(4000));
        assert_eq!(backoff_delay(9, 0.5), Duration::from_millis(30_000));
        assert_eq!(backoff_delay(0, 0.0), Duration::from_millis(800));
        assert_eq!(backoff_delay(0, 1.0), Duration::from_millis(1200));
    }

    #[test]
    fn parses_messages() {
        let m = parse_message(r#"{"MessageType":"ForceKeepAlive","Data":60}"#).unwrap();
        assert_eq!(m.message_type, "ForceKeepAlive");
        assert_eq!(m.data, Some(json!(60)));
        assert!(parse_message("not json").is_none());
        assert!(parse_message(r#"{"Data":1}"#).is_none());
    }

    #[tokio::test]
    async fn reports_closed_after_close() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let closed = Arc::new(AtomicBool::new(false));
        let flag = closed.clone();
        // Port 9 (discard) refuses the connection: the task goes to reconnecting, then close wins.
        let sock = JellyfinSocket::connect(
            &Handle::current(),
            "ws://127.0.0.1:9/socket".into(),
            |_| {},
            move |s| {
                if s == SocketStatus::Closed {
                    flag.store(true, Ordering::SeqCst);
                }
            },
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!sock.send("KeepAlive", None));
        sock.close();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(closed.load(Ordering::SeqCst));
    }
}
