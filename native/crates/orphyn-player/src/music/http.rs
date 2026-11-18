//! A tiny blocking HTTP getter for the app's lyrics lookups (LRCLIB): the app crate has no HTTP
//! client of its own, and the JSON is parsed on its side.
use std::time::Duration;

/// `GET url` as text. `Ok(None)` for a 404; network errors and other statuses are `Err`.
pub fn get_text(url: &str, timeout: Duration) -> Result<Option<String>, String> {
    let resp = reqwest::blocking::Client::new()
        .get(url)
        .header("Accept", "application/json")
        .timeout(timeout)
        .send()
        .map_err(|e| e.without_url().to_string())?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    resp.text()
        .map(Some)
        .map_err(|e| e.without_url().to_string())
}

/// `POST url` with an empty body; any non-2xx status is an error.
pub fn post_empty(url: &str, timeout: Duration) -> Result<(), String> {
    let resp = reqwest::blocking::Client::new()
        .post(url)
        .header("Content-Length", "0")
        .timeout(timeout)
        .send()
        .map_err(|e| e.without_url().to_string())?;
    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", status.as_u16()))
    }
}
