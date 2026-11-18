//! Minimal Jellyfin client: password login, one album with its tracks, image and audio URLs.
use serde::Deserialize;

const AUTH: &str = "MediaBrowser Client=\"Orphyn Spike\", Device=\"Dioxus Native\", DeviceId=\"orphyn-spike-dioxus\", Version=\"0.0.0\"";

#[derive(Clone, Debug)]
pub struct Session {
    pub base: String,
    pub token: String,
    pub user_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthResult {
    access_token: String,
    user: AuthUser,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthUser {
    id: String,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_artist: Option<String>,
    pub artists: Vec<String>,
    pub genres: Vec<String>,
    pub production_year: Option<i32>,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub run_time_ticks: Option<i64>,
    pub image_tags: std::collections::HashMap<String, String>,
    pub user_data: Option<UserData>,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserData {
    pub is_favorite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Items {
    items: Vec<Item>,
}

impl Item {
    pub fn seconds(&self) -> f64 {
        self.run_time_ticks.unwrap_or(0) as f64 / 10_000_000.0
    }
    pub fn artist(&self) -> String {
        self.artists.first().cloned().or_else(|| self.album_artist.clone()).unwrap_or_default()
    }
}

impl Session {
    pub fn login(base: &str, user: &str, password: &str) -> Result<Session, String> {
        let base = base.trim_end_matches('/').to_string();
        let r: AuthResult = reqwest::blocking::Client::new()
            .post(format!("{base}/Users/AuthenticateByName"))
            .header("X-Emby-Authorization", AUTH)
            .json(&serde_json::json!({ "Username": user, "Pw": password }))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| format!("login failed: {e}"))?;
        Ok(Session { base, token: r.access_token, user_id: r.user.id })
    }

    fn items(&self, query: &str) -> Result<Vec<Item>, String> {
        let url = format!("{}/Items?userId={}&{query}", self.base, self.user_id);
        let r: Items = reqwest::blocking::Client::new()
            .get(url)
            .header("X-Emby-Authorization", format!("{AUTH}, Token=\"{}\"", self.token))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| format!("items failed: {e}"))?;
        Ok(r.items)
    }

    /// An album: `id` if given, otherwise the first album with artwork out of a random sample.
    pub fn album(&self, id: Option<&str>) -> Result<(Item, Vec<Item>), String> {
        let fields = "fields=Genres,ProductionYear";
        let candidates = match id {
            Some(id) => self.items(&format!("ids={id}&{fields}"))?,
            None => self.items(&format!(
                "includeItemTypes=MusicAlbum&recursive=true&sortBy=Random&limit=40&{fields}"
            ))?,
        };
        for album in candidates.into_iter().filter(|a| id.is_some() || a.image_tags.contains_key("Primary")) {
            let tracks = self.items(&format!(
                "parentId={}&includeItemTypes=Audio&sortBy=ParentIndexNumber,IndexNumber,SortName",
                album.id
            ))?;
            if id.is_some() || tracks.len() >= 8 {
                return Ok((album, tracks));
            }
        }
        Err("no album with enough tracks".into())
    }

    pub fn image(&self, id: &str, width: u32) -> String {
        format!("{}/Items/{id}/Images/Primary?maxWidth={width}&quality=90&api_key={}", self.base, self.token)
    }

    pub fn audio_url(&self, id: &str) -> String {
        format!("{}/Audio/{id}/stream?static=true&api_key={}", self.base, self.token)
    }
}
