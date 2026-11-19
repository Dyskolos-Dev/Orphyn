//! Opt-in check against a real Jellyfin server (`ORPHYN_E2E_URL`, `_USER`, `_PASSWORD`): one
//! member signs in, opens the socket, creates a room, sees itself in the group list, measures the
//! server clock, and leaves. Run with `cargo test -p orphyn-jellyfin live -- --ignored`.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::auth;
use crate::Client;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a Jellyfin server: ORPHYN_E2E_URL / _USER / _PASSWORD"]
async fn create_join_clock_and_leave_on_a_real_server() {
    let (Ok(url), Ok(user), Ok(pw)) = (
        std::env::var("ORPHYN_E2E_URL"),
        std::env::var("ORPHYN_E2E_USER"),
        std::env::var("ORPHYN_E2E_PASSWORD"),
    ) else {
        return;
    };
    let device = "orphyn-syncplay-live-test";
    let ok = auth::login(&url, &user, &pw, device).await.expect("login");
    let client = Client::new(&url, device, Some(&ok.access_token));

    let events: Arc<Mutex<Vec<SyncPlayEvent>>> = Arc::default();
    let sink = events.clone();
    let socket = JellyfinSocket::connect(
        &tokio::runtime::Handle::current(),
        socket_url(&url, &ok.access_token, device),
        move |m| {
            if let Some(e) = parse_syncplay_message(&m) {
                sink.lock().unwrap().push(e);
            }
        },
        |_| {},
    );
    for _ in 0..100 {
        if socket.status() == SocketStatus::Open {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(socket.status(), SocketStatus::Open, "socket did not open");

    // Server clock: a burst of samples gives a sane offset and delay.
    let mut est = ClockOffsetEstimator::default();
    for _ in 0..4 {
        let t0 = now_ms();
        let (received, sent) = get_utc_time(&client).await.expect("GetUtcTime");
        est.add(time_sample(t0, received, sent, now_ms()));
    }
    assert!(est.delay() < 5_000.0, "delay {}", est.delay());
    assert!(est.offset().abs() < 3_600_000.0, "offset {}", est.offset());

    create_group(&client, "orphyn live test")
        .await
        .expect("create");
    let mut group = None;
    for _ in 0..50 {
        let found = events.lock().unwrap().iter().find_map(|e| match e {
            SyncPlayEvent::GroupJoined(g) => Some(g.clone()),
            _ => None,
        });
        if found.is_some() {
            group = found;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let group = group.expect("GroupJoined after create");
    assert_eq!(group.name, "orphyn live test");
    let listed = list_groups(&client).await.expect("list");
    assert!(listed.iter().any(|g| g.id == group.id), "own room listed");

    leave_group(&client).await.expect("leave");
    socket.close();
}

fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
