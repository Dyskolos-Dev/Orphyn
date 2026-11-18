//! Low-level HTTP access (port of `client.ts`): auth header, query serialisation, error mapping.
use std::sync::Arc;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::{APP_VERSION, CLIENT_NAME, DEVICE_NAME};

/// `encodeURIComponent`: everything but `A-Z a-z 0-9 - _ . ! ~ * ' ( )` is escaped.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

pub fn encode_component(s: &str) -> String {
    utf8_percent_encode(s, COMPONENT).to_string()
}

/// A query value: Jellyfin wants arrays as comma lists; `None` and empty lists are dropped.
#[derive(Clone, Debug, PartialEq)]
pub enum Q {
    Str(String),
    List(Vec<String>),
}

impl From<&str> for Q {
    fn from(v: &str) -> Self {
        Q::Str(v.to_string())
    }
}
impl From<String> for Q {
    fn from(v: String) -> Self {
        Q::Str(v)
    }
}
impl From<bool> for Q {
    fn from(v: bool) -> Self {
        Q::Str(v.to_string())
    }
}
impl From<u32> for Q {
    fn from(v: u32) -> Self {
        Q::Str(v.to_string())
    }
}
impl From<i32> for Q {
    fn from(v: i32) -> Self {
        Q::Str(v.to_string())
    }
}
impl From<u64> for Q {
    fn from(v: u64) -> Self {
        Q::Str(v.to_string())
    }
}
impl From<Vec<String>> for Q {
    fn from(v: Vec<String>) -> Self {
        Q::List(v)
    }
}
impl From<&[&str]> for Q {
    fn from(v: &[&str]) -> Self {
        Q::List(v.iter().map(|s| s.to_string()).collect())
    }
}

pub type Params = Vec<(&'static str, Option<Q>)>;

/// Serialises params like `buildQuery`: arrays as comma lists, `None` / empty lists dropped.
pub fn build_query(params: &[(&str, Option<Q>)]) -> String {
    let mut parts = Vec::new();
    for (key, value) in params {
        let v = match value {
            None => continue,
            Some(Q::Str(s)) => s.clone(),
            Some(Q::List(l)) if l.is_empty() => continue,
            Some(Q::List(l)) => l.join(","),
        };
        parts.push(format!(
            "{}={}",
            encode_component(key),
            encode_component(&v)
        ));
    }
    parts.join("&")
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum JfError {
    /// The server answered with an error status (401 = the token is no longer valid).
    #[error("HTTP {0}")]
    Http(u16),
    /// The server could not be reached.
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Decode(String),
}

impl JfError {
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, JfError::Http(401))
    }
}

pub type JfResult<T> = Result<T, JfError>;

/// One server connection. Cheap to clone (the HTTP pool is shared).
#[derive(Clone)]
pub struct Client {
    base: Arc<str>,
    device_id: Arc<str>,
    token: Option<Arc<str>>,
    http: reqwest::Client,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("authenticated", &self.token.is_some())
            .finish()
    }
}

fn shared_http() -> reqwest::Client {
    static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    })
    .clone()
}

impl Client {
    pub fn new(base_url: &str, device_id: &str, token: Option<&str>) -> Self {
        Self {
            base: base_url.trim_end_matches('/').into(),
            device_id: device_id.into(),
            token: token.map(Into::into),
            http: shared_http(),
        }
    }

    pub fn with_token(&self, token: &str) -> Self {
        Self {
            token: Some(token.into()),
            ..self.clone()
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The `Authorization` header of the Jellyfin SDK (`MediaBrowser Client=..., Token=...`).
    pub fn authorization_header(&self) -> String {
        let q = |v: &str| encode_component(v);
        let mut h = format!(
            "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
            q(CLIENT_NAME),
            q(DEVICE_NAME),
            q(&self.device_id),
            q(APP_VERSION)
        );
        if let Some(t) = &self.token {
            h.push_str(&format!(", Token=\"{}\"", q(t)));
        }
        h
    }

    /// `base + path + ?query` with the token as `api_key` (for URLs handed to players / images).
    pub fn authed_url(&self, path: &str, params: &[(&str, Option<Q>)]) -> String {
        let mut all: Vec<(&str, Option<Q>)> = params.to_vec();
        if let Some(t) = &self.token {
            all.push(("api_key", Some(Q::Str(t.to_string()))));
        }
        let query = build_query(&all);
        if query.is_empty() {
            format!("{}{path}", self.base)
        } else {
            format!("{}{path}?{query}", self.base)
        }
    }

    pub async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        params: &[(&str, Option<Q>)],
        body: Option<serde_json::Value>,
    ) -> JfResult<T> {
        let query = build_query(params);
        let url = if query.is_empty() {
            format!("{}{path}", self.base)
        } else {
            format!("{}{path}?{query}", self.base)
        };
        let mut req = self
            .http
            .request(method, &url)
            .header("Authorization", self.authorization_header())
            .timeout(std::time::Duration::from_secs(30));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| JfError::Network(e.without_url().to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JfError::Http(status.as_u16()));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| JfError::Network(e.without_url().to_string()))?;
        // 204 / empty bodies decode as JSON null (so `request::<()>` and `Option<T>` work).
        let slice: &[u8] = if bytes.is_empty() { b"null" } else { &bytes };
        serde_json::from_slice(slice).map_err(|e| JfError::Decode(e.to_string()))
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, Option<Q>)],
    ) -> JfResult<T> {
        self.request(Method::GET, path, params, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_drops_empty_values_and_joins_lists() {
        let q = build_query(&[
            ("userId", Some("u 1".into())),
            ("limit", None),
            (
                "fields",
                Some(Q::List(vec!["Genres".into(), "Overview".into()])),
            ),
            ("ids", Some(Q::List(vec![]))),
            ("recursive", Some(true.into())),
        ]);
        assert_eq!(q, "userId=u%201&fields=Genres%2COverview&recursive=true");
    }

    #[test]
    fn auth_header_carries_identity_and_token() {
        let c = Client::new("https://jf.example/", "dev-1", Some("tok"));
        assert_eq!(c.base_url(), "https://jf.example");
        assert_eq!(
            c.authorization_header(),
            "MediaBrowser Client=\"Orphyn\", Device=\"Orphyn%20Desktop\", DeviceId=\"dev-1\", Version=\"0.1.0\", Token=\"tok\""
        );
        assert_eq!(
            c.authed_url("/Audio/a/stream", &[("static", Some(true.into()))]),
            "https://jf.example/Audio/a/stream?static=true&api_key=tok"
        );
    }
}
