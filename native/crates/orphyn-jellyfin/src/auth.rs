//! Sign-in (port of `auth.ts`): `POST /Users/AuthenticateByName` as client "Orphyn" with this
//! install's device id, with the same error kinds as the React app.
use reqwest::Method;
use serde::Deserialize;

use crate::client::{Client, JfError};

#[derive(Clone, Debug, PartialEq)]
pub struct LoginSuccess {
    pub access_token: String,
    pub user_id: String,
    pub user_name: String,
    pub server_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginErrorKind {
    InvalidCredentials,
    Unreachable,
    ServerError,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoginError {
    pub kind: LoginErrorKind,
    pub message: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct AuthenticationResult {
    access_token: Option<String>,
    server_id: Option<String>,
    user: Option<AuthUser>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct AuthUser {
    id: Option<String>,
    name: Option<String>,
}

/// Signs in. `url` is the server's base URL; the returned token is not stored anywhere yet.
pub async fn login(
    url: &str,
    username: &str,
    password: &str,
    device_id: &str,
) -> Result<LoginSuccess, LoginError> {
    let client = Client::new(url, device_id, None);
    let res: Result<AuthenticationResult, JfError> = client
        .request(
            Method::POST,
            "/Users/AuthenticateByName",
            &[],
            Some(serde_json::json!({ "Username": username, "Pw": password })),
        )
        .await;
    match res {
        Ok(r) => match (r.access_token, r.user.as_ref().and_then(|u| u.id.clone())) {
            (Some(access_token), Some(user_id)) => Ok(LoginSuccess {
                access_token,
                user_id,
                user_name: r
                    .user
                    .and_then(|u| u.name)
                    .unwrap_or_else(|| username.to_string()),
                server_id: r.server_id.unwrap_or_default(),
            }),
            _ => Err(LoginError {
                kind: LoginErrorKind::ServerError,
                message: "Unexpected server response.".into(),
            }),
        },
        Err(JfError::Http(400 | 401)) => Err(LoginError {
            kind: LoginErrorKind::InvalidCredentials,
            message: "Wrong username or password.".into(),
        }),
        Err(JfError::Network(_)) => Err(LoginError {
            kind: LoginErrorKind::Unreachable,
            message: "The server could not be reached.".into(),
        }),
        Err(JfError::Http(status)) => Err(LoginError {
            kind: LoginErrorKind::ServerError,
            message: format!("Server error ({status})."),
        }),
        Err(JfError::Decode(_)) => Err(LoginError {
            kind: LoginErrorKind::ServerError,
            message: "Unexpected server response.".into(),
        }),
    }
}

/// `GET /System/Info/Public`: name, version and id of a server (no sign-in needed).
#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct PublicSystemInfo {
    pub server_name: String,
    pub version: String,
    pub id: String,
}

pub async fn public_info(url: &str, device_id: &str) -> Result<PublicSystemInfo, JfError> {
    Client::new(url, device_id, None)
        .get("/System/Info/Public", &[])
        .await
}

/// `POST /Sessions/Logout`: revokes the token on the server (best effort, like the React app).
pub async fn logout(client: &Client) {
    let _: Result<serde_json::Value, _> = client
        .request(Method::POST, "/Sessions/Logout", &[], None)
        .await;
}
