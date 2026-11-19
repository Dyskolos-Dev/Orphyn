//! SyncPlay messages and REST requests (shapes checked against Jellyfin 10.11).
use reqwest::Method;
use serde_json::{json, Value};

use crate::client::{Client, JfError, JfResult};
use crate::syncplay::socket::SocketMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupState {
    Idle,
    Waiting,
    Paused,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Unpause,
    Pause,
    Stop,
    Seek,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncPlayGroup {
    pub id: String,
    pub name: String,
    pub state: GroupState,
    /// User names (the server gives no ids).
    pub participants: Vec<String>,
    pub last_updated_at: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncPlayCommand {
    pub group_id: String,
    pub playlist_item_id: String,
    /// Server time (ms since epoch) at which the command takes effect.
    pub when: f64,
    pub position_ticks: f64,
    pub command: CommandKind,
    pub emitted_at: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub item_id: String,
    pub playlist_item_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayQueue {
    pub reason: String,
    pub last_update: f64,
    pub playlist: Vec<QueueEntry>,
    pub playing_item_index: i64,
    pub start_position_ticks: f64,
    pub is_playing: bool,
    pub shuffle_mode: String,
    pub repeat_mode: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPlayErrorKind {
    NotInGroup,
    GroupDoesNotExist,
    CreateGroupDenied,
    JoinGroupDenied,
    LibraryAccessDenied,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncPlayEvent {
    Command(SyncPlayCommand),
    GroupJoined(SyncPlayGroup),
    GroupLeft {
        group_id: String,
    },
    UserJoined {
        group_id: String,
        user: String,
    },
    UserLeft {
        group_id: String,
        user: String,
    },
    StateUpdate {
        group_id: String,
        state: GroupState,
        reason: String,
    },
    PlayQueue {
        group_id: String,
        queue: PlayQueue,
    },
    Error {
        group_id: String,
        kind: SyncPlayErrorKind,
    },
}

pub const EMPTY_GUID: &str = "00000000000000000000000000000000";

/// Guids come with and without dashes depending on the message: compare them normalised.
pub fn normalize_guid(id: &Value) -> String {
    id.as_str()
        .map(|s| s.replace('-', "").to_lowercase())
        .unwrap_or_default()
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parses a .NET date (`2026-11-17T18:27:41.0429711Z`, 7 fractional digits) to ms since epoch.
pub fn parse_server_date(value: &Value) -> Option<f64> {
    let s = value.as_str()?.trim();
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let mo: i64 = d.next()?.parse().ok()?;
    let da: i64 = d.next()?.parse().ok()?;
    // The server always sends UTC (`Z`).
    let time = time.trim_end_matches('Z');
    let (hms, frac) = time.split_once('.').unwrap_or((time, ""));
    let mut t = hms.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let mi: i64 = t.next()?.parse().ok()?;
    let se: i64 = t.next()?.parse().ok()?;
    let ms: f64 = if frac.is_empty() {
        0.0
    } else {
        let digits: String = frac.chars().take(3).collect();
        format!("{digits:0<3}").parse::<f64>().ok()?
    };
    let days = days_from_civil(y, mo, da);
    Some(((days * 86_400 + h * 3600 + mi * 60 + se) as f64) * 1000.0 + ms)
}

/// ms since epoch to the ISO string the server expects (`2026-11-17T18:27:41.042Z`).
pub fn to_server_date(ms: f64) -> String {
    let total = ms.floor() as i64;
    let millis = total.rem_euclid(1000);
    let secs = total.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn num(v: Option<&Value>, fallback: f64) -> f64 {
    v.and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .unwrap_or(fallback)
}
fn text(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or_default().to_string()
}
fn guid(v: Option<&Value>) -> String {
    v.map(normalize_guid).unwrap_or_default()
}

fn to_state(v: Option<&Value>) -> GroupState {
    match v.and_then(Value::as_str) {
        Some("Waiting") => GroupState::Waiting,
        Some("Paused") => GroupState::Paused,
        Some("Playing") => GroupState::Playing,
        _ => GroupState::Idle,
    }
}

pub fn parse_group(raw: &Value) -> Option<SyncPlayGroup> {
    let obj = raw.as_object()?;
    let gid = obj
        .get("GroupId")
        .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))?;
    Some(SyncPlayGroup {
        id: normalize_guid(gid),
        name: text(obj.get("GroupName")),
        state: to_state(obj.get("State")),
        participants: obj
            .get("Participants")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        last_updated_at: obj
            .get("LastUpdatedAt")
            .and_then(parse_server_date)
            .unwrap_or(0.0),
    })
}

pub fn parse_command(raw: &Value) -> Option<SyncPlayCommand> {
    let obj = raw.as_object()?;
    let command = match obj.get("Command")?.as_str()? {
        "Unpause" => CommandKind::Unpause,
        "Pause" => CommandKind::Pause,
        "Stop" => CommandKind::Stop,
        "Seek" => CommandKind::Seek,
        _ => return None,
    };
    let when = parse_server_date(obj.get("When")?)?;
    Some(SyncPlayCommand {
        group_id: guid(obj.get("GroupId")),
        playlist_item_id: guid(obj.get("PlaylistItemId")),
        when,
        position_ticks: num(obj.get("PositionTicks"), 0.0),
        command,
        emitted_at: obj
            .get("EmittedAt")
            .and_then(parse_server_date)
            .unwrap_or(when),
    })
}

pub fn parse_play_queue(raw: &Value) -> Option<PlayQueue> {
    let obj = raw.as_object()?;
    let list = obj.get("Playlist")?.as_array()?;
    let non_empty = |v: Option<&Value>| {
        v.filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
            .cloned()
    };
    let playlist = list
        .iter()
        .filter_map(|e| {
            let item = non_empty(e.get("ItemId"))?;
            let pid = non_empty(e.get("PlaylistItemId"))?;
            Some(QueueEntry {
                item_id: normalize_guid(&item),
                playlist_item_id: normalize_guid(&pid),
            })
        })
        .collect();
    Some(PlayQueue {
        reason: text(obj.get("Reason")),
        last_update: obj
            .get("LastUpdate")
            .and_then(parse_server_date)
            .unwrap_or(0.0),
        playlist,
        playing_item_index: num(obj.get("PlayingItemIndex"), -1.0) as i64,
        start_position_ticks: num(obj.get("StartPositionTicks"), 0.0),
        is_playing: obj.get("IsPlaying").and_then(Value::as_bool) == Some(true),
        shuffle_mode: text(obj.get("ShuffleMode")),
        repeat_mode: text(obj.get("RepeatMode")),
    })
}

/// One socket message to a SyncPlay event; `None` for anything else (keep-alives, other features).
pub fn parse_syncplay_message(msg: &SocketMessage) -> Option<SyncPlayEvent> {
    if msg.message_type == "SyncPlayCommand" {
        return parse_command(msg.data.as_ref()?).map(SyncPlayEvent::Command);
    }
    if msg.message_type != "SyncPlayGroupUpdate" {
        return None;
    }
    let update = msg.data.as_ref()?.as_object()?;
    let group_id = guid(update.get("GroupId"));
    let data = update.get("Data").unwrap_or(&Value::Null);
    match update.get("Type")?.as_str()? {
        "GroupJoined" => parse_group(data).map(SyncPlayEvent::GroupJoined),
        "GroupLeft" => {
            let g = normalize_guid(data);
            Some(SyncPlayEvent::GroupLeft {
                group_id: if g.is_empty() { group_id } else { g },
            })
        }
        "UserJoined" => Some(SyncPlayEvent::UserJoined {
            group_id,
            user: text(Some(data)),
        }),
        "UserLeft" => Some(SyncPlayEvent::UserLeft {
            group_id,
            user: text(Some(data)),
        }),
        "StateUpdate" => {
            let o = data.as_object()?;
            Some(SyncPlayEvent::StateUpdate {
                group_id,
                state: to_state(o.get("State")),
                reason: text(o.get("Reason")),
            })
        }
        "PlayQueue" => {
            parse_play_queue(data).map(|queue| SyncPlayEvent::PlayQueue { group_id, queue })
        }
        other => {
            let kind = match other {
                "NotInGroup" => SyncPlayErrorKind::NotInGroup,
                "GroupDoesNotExist" => SyncPlayErrorKind::GroupDoesNotExist,
                "CreateGroupDenied" => SyncPlayErrorKind::CreateGroupDenied,
                "JoinGroupDenied" => SyncPlayErrorKind::JoinGroupDenied,
                "LibraryAccessDenied" => SyncPlayErrorKind::LibraryAccessDenied,
                _ => return None,
            };
            Some(SyncPlayEvent::Error { group_id, kind })
        }
    }
}

// ---------------------------------------------------------------------------------- REST

async fn post(client: &Client, path: &str, data: Value) -> JfResult<()> {
    client
        .request::<Value>(Method::POST, &format!("/SyncPlay/{path}"), &[], Some(data))
        .await
        .map(|_| ())
}

pub async fn list_groups(client: &Client) -> JfResult<Vec<SyncPlayGroup>> {
    let raw: Value = client.get("/SyncPlay/List", &[]).await?;
    Ok(raw
        .as_array()
        .map(|a| a.iter().filter_map(parse_group).collect())
        .unwrap_or_default())
}

pub async fn create_group(client: &Client, name: &str) -> JfResult<()> {
    post(client, "New", json!({ "GroupName": name })).await
}
pub async fn join_group(client: &Client, group_id: &str) -> JfResult<()> {
    post(client, "Join", json!({ "GroupId": group_id })).await
}
pub async fn leave_group(client: &Client) -> JfResult<()> {
    post(client, "Leave", json!({})).await
}

pub async fn set_new_queue(
    client: &Client,
    item_ids: &[String],
    playing_index: usize,
    start_position_ticks: f64,
) -> JfResult<()> {
    post(
        client,
        "SetNewQueue",
        json!({
            "PlayingQueue": item_ids,
            "PlayingItemPosition": playing_index,
            "StartPositionTicks": start_position_ticks.max(0.0).round() as i64,
        }),
    )
    .await
}

pub async fn queue_items(client: &Client, item_ids: &[String], next: bool) -> JfResult<()> {
    let mode = if next { "QueueNext" } else { "Queue" };
    post(
        client,
        "Queue",
        json!({ "ItemIds": item_ids, "Mode": mode }),
    )
    .await
}
pub async fn set_playlist_item(client: &Client, playlist_item_id: &str) -> JfResult<()> {
    post(
        client,
        "SetPlaylistItem",
        json!({ "PlaylistItemId": playlist_item_id }),
    )
    .await
}
/// The server ignores it unless `current` is still the playing entry: duplicates are harmless.
pub async fn next_item(client: &Client, current: &str) -> JfResult<()> {
    post(client, "NextItem", json!({ "PlaylistItemId": current })).await
}
pub async fn previous_item(client: &Client, current: &str) -> JfResult<()> {
    post(client, "PreviousItem", json!({ "PlaylistItemId": current })).await
}
/// With `true` the group stops waiting for this member (it cannot play the item).
pub async fn set_ignore_wait(client: &Client, ignore: bool) -> JfResult<()> {
    post(client, "SetIgnoreWait", json!({ "IgnoreWait": ignore })).await
}
pub async fn request_unpause(client: &Client) -> JfResult<()> {
    post(client, "Unpause", json!({})).await
}
pub async fn request_pause(client: &Client) -> JfResult<()> {
    post(client, "Pause", json!({})).await
}
pub async fn request_stop(client: &Client) -> JfResult<()> {
    post(client, "Stop", json!({})).await
}
pub async fn request_seek(client: &Client, position_ticks: f64) -> JfResult<()> {
    post(
        client,
        "Seek",
        json!({ "PositionTicks": position_ticks.max(0.0).round() as i64 }),
    )
    .await
}

#[derive(Debug, Clone)]
pub struct BufferingReport {
    /// Server time (ms).
    pub when: f64,
    pub position_ticks: f64,
    pub is_playing: bool,
    pub playlist_item_id: String,
}

fn buffering_body(r: &BufferingReport) -> Value {
    json!({
        "When": to_server_date(r.when),
        "PositionTicks": r.position_ticks.max(0.0).round() as i64,
        "IsPlaying": r.is_playing,
        "PlaylistItemId": r.playlist_item_id,
    })
}

pub async fn report_buffering(client: &Client, r: &BufferingReport) -> JfResult<()> {
    post(client, "Buffering", buffering_body(r)).await
}
pub async fn report_ready(client: &Client, r: &BufferingReport) -> JfResult<()> {
    post(client, "Ready", buffering_body(r)).await
}
/// Tells the server our round-trip time (ms); it uses it to place `When` far enough ahead.
pub async fn send_ping(client: &Client, ping_ms: f64) -> JfResult<()> {
    post(
        client,
        "Ping",
        json!({ "Ping": ping_ms.max(0.0).round() as i64 }),
    )
    .await
}

/// `GET /GetUtcTime`: when the server received the request and sent the answer (ms).
pub async fn get_utc_time(client: &Client) -> JfResult<(f64, f64)> {
    let raw: Value = client.get("/GetUtcTime", &[]).await?;
    let bad = || JfError::Decode("Bad /GetUtcTime answer".into());
    let received = raw
        .get("RequestReceptionTime")
        .and_then(parse_server_date)
        .ok_or_else(bad)?;
    let sent = raw
        .get("ResponseTransmissionTime")
        .and_then(parse_server_date)
        .ok_or_else(bad)?;
    Ok((received, sent))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(t: &str, data: Value) -> SocketMessage {
        SocketMessage {
            message_type: t.into(),
            message_id: None,
            data: Some(data),
        }
    }

    #[test]
    fn dates_round_trip_and_cut_fraction() {
        let ms = parse_server_date(&json!("2026-11-17T18:27:41.0429711Z")).unwrap();
        assert_eq!(to_server_date(ms), "2026-11-17T18:27:41.042Z");
        assert_eq!(
            parse_server_date(&json!("1970-01-01T00:00:01Z")),
            Some(1000.0)
        );
        assert!(parse_server_date(&json!("nope")).is_none());
        assert!(parse_server_date(&json!(3)).is_none());
    }

    #[test]
    fn guids_are_normalised() {
        assert_eq!(normalize_guid(&json!("AB-cd-01")), "abcd01");
        assert_eq!(normalize_guid(&json!(4)), "");
    }

    #[test]
    fn parses_a_command() {
        let e = parse_syncplay_message(&msg(
            "SyncPlayCommand",
            json!({"GroupId":"AA-BB","PlaylistItemId":"CC","When":"2026-11-17T18:27:41.5Z",
                   "PositionTicks":50000000,"Command":"Unpause","EmittedAt":"2026-11-17T18:27:40Z"}),
        ));
        match e {
            Some(SyncPlayEvent::Command(c)) => {
                assert_eq!(c.group_id, "aabb");
                assert_eq!(c.command, CommandKind::Unpause);
                assert_eq!(c.position_ticks, 5e7);
                assert!(c.emitted_at < c.when);
            }
            other => panic!("{other:?}"),
        }
        let bad = msg(
            "SyncPlayCommand",
            json!({"Command":"Boom","When":"2026-11-17T18:27:41Z"}),
        );
        assert!(parse_syncplay_message(&bad).is_none());
        assert!(parse_syncplay_message(&msg("KeepAlive", json!(null))).is_none());
    }

    #[test]
    fn parses_group_updates() {
        let joined = parse_syncplay_message(&msg(
            "SyncPlayGroupUpdate",
            json!({"GroupId":"g","Type":"GroupJoined","Data":{"GroupId":"G-1","GroupName":"Room",
                   "State":"Waiting","Participants":["a","b"],"LastUpdatedAt":"2026-11-17T18:27:41Z"}}),
        ));
        match joined {
            Some(SyncPlayEvent::GroupJoined(g)) => {
                assert_eq!(g.id, "g1");
                assert_eq!(g.state, GroupState::Waiting);
                assert_eq!(g.participants.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        let q = parse_syncplay_message(&msg(
            "SyncPlayGroupUpdate",
            json!({"GroupId":"g","Type":"PlayQueue","Data":{"Reason":"NewPlaylist",
                   "Playlist":[{"ItemId":"I-1","PlaylistItemId":"P-1"},{"ItemId":""}],
                   "PlayingItemIndex":0,"IsPlaying":true}}),
        ));
        match q {
            Some(SyncPlayEvent::PlayQueue { queue, .. }) => {
                assert_eq!(
                    queue.playlist,
                    vec![QueueEntry {
                        item_id: "i1".into(),
                        playlist_item_id: "p1".into()
                    }]
                );
                assert!(queue.is_playing);
            }
            other => panic!("{other:?}"),
        }
        let err = parse_syncplay_message(&msg(
            "SyncPlayGroupUpdate",
            json!({"GroupId":"g","Type":"JoinGroupDenied","Data":null}),
        ));
        assert!(matches!(
            err,
            Some(SyncPlayEvent::Error {
                kind: SyncPlayErrorKind::JoinGroupDenied,
                ..
            })
        ));
        let left = parse_syncplay_message(&msg(
            "SyncPlayGroupUpdate",
            json!({"GroupId":"G-2","Type":"GroupLeft","Data":null}),
        ));
        assert_eq!(
            left,
            Some(SyncPlayEvent::GroupLeft {
                group_id: "g2".into()
            })
        );
        let weird = msg(
            "SyncPlayGroupUpdate",
            json!({"GroupId":"g","Type":"Weird","Data":null}),
        );
        assert!(parse_syncplay_message(&weird).is_none());
    }
}
