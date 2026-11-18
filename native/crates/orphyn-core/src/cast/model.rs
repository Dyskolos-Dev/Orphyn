//! Device-agnostic casting model (mirrors `src/shared/cast/model.ts`; JSON is camelCase).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CastKind {
    Chromecast,
    Upnp,
    Airplay,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastCapabilities {
    pub audio: bool,
    pub video: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CastDevice {
    pub id: String,
    pub name: String,
    pub kind: CastKind,
    pub model: String,
    pub manufacturer: String,
    pub capabilities: CastCapabilities,
    pub address: String,
    pub port: u16,
    pub wiim: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CastMediaKind {
    Audio,
    Video,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CastMedia {
    pub url: String,
    pub mime_type: String,
    pub kind: CastMediaKind,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_position_sec: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CastStatus {
    Idle,
    Connecting,
    Playing,
    Paused,
    Buffering,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CastSessionState {
    pub device_id: String,
    pub status: CastStatus,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub volume: f64,
    pub muted: bool,
    pub media: Option<CastMedia>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CastSessionState {
    pub fn idle(device_id: &str) -> Self {
        Self {
            device_id: device_id.to_string(),
            status: CastStatus::Idle,
            position_sec: 0.0,
            duration_sec: 0.0,
            volume: 1.0,
            muted: false,
            media: None,
            error: None,
        }
    }
}

/// Payload of `cast:ended`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CastEndedEvent {
    pub device_id: String,
    pub media: Option<CastMedia>,
}

/// Payload of `cast:error`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CastErrorEvent {
    pub device_id: Option<String>,
    pub message: String,
}

#[cfg(test)]
pub fn test_media() -> CastMedia {
    CastMedia {
        url: "http://h/a.mp3".into(),
        mime_type: "audio/mpeg".into(),
        kind: CastMediaKind::Audio,
        title: "T".into(),
        artist: None,
        album: None,
        artwork_url: None,
        duration_sec: None,
        start_position_sec: None,
    }
}
