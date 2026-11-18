//! Minimal blocking Jellyfin client (login, album + tracks, artwork download, stream URLs).
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Item {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub production_year: Option<i32>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub index_number: Option<i32>,
    #[serde(default)]
    pub parent_index_number: Option<i32>,
    /// 10_000_000 ticks per second.
    #[serde(default)]
    pub run_time_ticks: Option<u64>,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub image_tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub album_primary_image_tag: Option<String>,
    #[serde(default)]
    pub child_count: Option<u32>,
}

impl Item {
    pub fn seconds(&self) -> f32 {
        self.run_time_ticks.unwrap_or(0) as f32 / 10_000_000.0
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Items {
    items: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Auth {
    access_token: String,
    user: AuthUser,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUser {
    id: String,
}

pub struct Client {
    pub base: String,
    pub token: String,
    pub user_id: String,
    http: reqwest::blocking::Client,
    cache: PathBuf,
}

impl Client {
    /// Credentials come from ORPHYN_E2E_URL / ORPHYN_E2E_USER / ORPHYN_E2E_PASSWORD only.
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("ORPHYN_E2E_URL").context("ORPHYN_E2E_URL not set")?;
        let user = std::env::var("ORPHYN_E2E_USER").context("ORPHYN_E2E_USER not set")?;
        let pass = std::env::var("ORPHYN_E2E_PASSWORD").context("ORPHYN_E2E_PASSWORD not set")?;
        Self::login(url.trim_end_matches('/'), &user, &pass)
    }

    pub fn login(base: &str, user: &str, pass: &str) -> Result<Self> {
        let http = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(60)).build()?;
        let r: Auth = http
            .post(format!("{base}/Users/AuthenticateByName"))
            .header("Authorization", "MediaBrowser Client=\"Orphyn GPUI spike\", Device=\"spike\", DeviceId=\"orphyn-gpui-spike\", Version=\"0.0.1\"")
            .json(&serde_json::json!({ "Username": user, "Pw": pass }))
            .send()?
            .error_for_status()
            .map_err(|e| anyhow!("login failed: {e}"))?
            .json()?;
        let cache = std::env::temp_dir().join("orphyn-gpui-spike");
        std::fs::create_dir_all(&cache)?;
        Ok(Self { base: base.into(), token: r.access_token, user_id: r.user.id, http, cache })
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        Ok(self
            .http
            .get(format!("{}{}", self.base, path))
            .header("X-Emby-Token", &self.token)
            .send()?
            .error_for_status()?
            .json()?)
    }

    /// Picks an album (the one with most tracks among a random sample, or ORPHYN_ALBUM_ID).
    pub fn pick_album(&self) -> Result<Item> {
        if let Ok(id) = std::env::var("ORPHYN_ALBUM_ID") {
            return self.get(&format!("/Users/{}/Items/{id}", self.user_id));
        }
        let r: Items = self.get(&format!(
            "/Users/{}/Items?IncludeItemTypes=MusicAlbum&Recursive=true&SortBy=SortName&Limit=60&Fields=Genres,ChildCount",
            self.user_id
        ))?;
        r.items
            .into_iter()
            .filter(|a| a.image_tags.contains_key("Primary"))
            .max_by_key(|a| a.child_count.unwrap_or(0).min(14))
            .ok_or_else(|| anyhow!("no music album with artwork on this server"))
    }

    pub fn tracks(&self, album: &str) -> Result<Vec<Item>> {
        let r: Items = self.get(&format!(
            "/Users/{}/Items?ParentId={album}&IncludeItemTypes=Audio&SortBy=ParentIndexNumber,IndexNumber,SortName&Fields=Genres",
            self.user_id
        ))?;
        Ok(r.items)
    }

    /// First video item, if the server has one (for the libmpv video test).
    pub fn pick_video(&self) -> Result<Option<Item>> {
        let r: Items = self.get(&format!(
            "/Users/{}/Items?IncludeItemTypes=Movie,Episode,Video&Recursive=true&Limit=1&SortBy=Runtime&SortOrder=Ascending&Filters=IsNotFolder&MinRunTimeTicks=600000000",
            self.user_id
        ))?;
        Ok(r.items.into_iter().next())
    }

    /// Downloads `/Items/{id}/Images/Primary` to the temp cache and returns the local path.
    pub fn artwork(&self, id: &str, max_width: u32) -> Result<PathBuf> {
        let path = self.cache.join(format!("{id}-{max_width}.jpg"));
        if !path.exists() {
            let bytes = self
                .http
                .get(format!("{}/Items/{id}/Images/Primary?maxWidth={max_width}&quality=90", self.base))
                .header("X-Emby-Token", &self.token)
                .send()?
                .error_for_status()?
                .bytes()?;
            std::fs::write(&path, &bytes)?;
        }
        Ok(path)
    }

    pub fn audio_url(&self, id: &str) -> String {
        format!("{}/Audio/{id}/stream?static=true&api_key={}", self.base, self.token)
    }
    pub fn video_url(&self, id: &str) -> String {
        format!("{}/Videos/{id}/stream?static=true&api_key={}", self.base, self.token)
    }

    /// Downloads a track fully (rodio needs a seekable source).
    pub fn download_audio(&self, id: &str) -> Result<Vec<u8>> {
        Ok(self.http.get(self.audio_url(id)).send()?.error_for_status()?.bytes()?.to_vec())
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache
    }
}
