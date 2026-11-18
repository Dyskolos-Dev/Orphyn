//! Online artist images (Deezer, then TheAudioDB). The renderer asks for a name and gets back an
//! https image URL or `None`; nothing is ever written to the Jellyfin server. The APIs send no
//! CORS headers, which is why the request is made from here. Results are cached in memory per
//! normalised name and at most `MAX_CONCURRENT` requests run at once.

mod matching;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::error::{Error, Result};
use matching::{normalize, pick_audiodb, pick_deezer, AudioDbArtist, DeezerArtist};

const MAX_CONCURRENT: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(8);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .user_agent("Orphyn")
            .build()
            .unwrap_or_default()
    })
}

fn permits() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(MAX_CONCURRENT))
}

/// `None` = looked up and nothing reliable found (a miss is cached too).
fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[derive(Deserialize)]
struct DeezerSearch {
    #[serde(default)]
    data: Vec<DeezerArtist>,
}

#[derive(Deserialize)]
struct AudioDbSearch {
    artists: Option<Vec<AudioDbArtist>>,
}

async fn get_json<T: serde::de::DeserializeOwned>(url: reqwest::Url) -> Result<T> {
    let resp = client()
        .get(url)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!("HTTP {}", resp.status().as_u16())));
    }
    resp.json::<T>()
        .await
        .map_err(|e| Error::Other(e.to_string()))
}

async fn lookup(name: &str) -> Result<Option<String>> {
    let url = reqwest::Url::parse_with_params(
        "https://api.deezer.com/search/artist",
        &[("q", name), ("limit", "5")],
    )
    .map_err(|e| Error::Other(e.to_string()))?;
    let deezer: DeezerSearch = get_json(url).await?;
    if let Some(found) = pick_deezer(name, &deezer.data) {
        return Ok(Some(found));
    }
    // Second source: a failure here is a plain miss, Deezer already answered.
    let url = reqwest::Url::parse_with_params(
        "https://www.theaudiodb.com/api/v1/json/2/search.php",
        &[("s", name)],
    )
    .map_err(|e| Error::Other(e.to_string()))?;
    let found = match get_json::<AudioDbSearch>(url).await {
        Ok(r) => pick_audiodb(name, &r.artists.unwrap_or_default()),
        Err(_) => None,
    };
    Ok(found)
}

/// An image URL for the artist `name`, or `None`. Network errors are returned (and not cached)
/// so the renderer can retry later.
#[tauri::command]
pub async fn artwork_artist_image(name: String) -> Result<Option<String>> {
    let key = normalize(&name);
    if key.is_empty() {
        return Ok(None);
    }
    if let Some(hit) = cache()
        .lock()
        .map_err(|e| Error::Other(e.to_string()))?
        .get(&key)
    {
        return Ok(hit.clone());
    }
    let _permit = permits()
        .acquire()
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    let result = lookup(name.trim()).await?;
    if let Ok(mut c) = cache().lock() {
        c.insert(key, result.clone());
    }
    Ok(result)
}
