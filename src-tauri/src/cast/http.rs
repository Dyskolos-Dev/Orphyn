//! Shared HTTP client and the bounded text fetch used for UPnP device descriptions.

use std::sync::OnceLock;
use std::time::Duration;

const MAX_DESCRIPTION_BYTES: usize = 512 * 1024;

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .build()
            .unwrap_or_default()
    })
}

/// GET a small text document (device description) with a timeout and a size cap.
pub async fn fetch_text(url: &str, timeout: Duration) -> Result<String, String> {
    let mut resp = client()
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() >= 400 {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let mut data: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if data.len() + chunk.len() > MAX_DESCRIPTION_BYTES {
            return Err("Description too large".into());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&data).into_owned())
}
