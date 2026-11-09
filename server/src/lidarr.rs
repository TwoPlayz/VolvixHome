//! Lidarr: find music you don't have yet and ask Lidarr to get it.
//!
//! Volvix Home talks to Lidarr's v1 API with the API key from Lidarr → Settings → General.
//! Lidarr does the finding and downloading with whatever sources you've set up there; when it imports
//! an album into the music folder, its webhook (Settings → Connect) tells Volvix Home to rescan, so the
//! album shows up within seconds.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn yes() -> bool {
    true
}

/// Stored in <data>/lidarr.json; VH_LIDARR_URL / VH_LIDARR_API_KEY override the file.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Settings {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub root_folder: String,
    #[serde(default)]
    pub quality_profile_id: i64,
    #[serde(default)]
    pub metadata_profile_id: i64,
    #[serde(default = "yes")]
    pub everyone_can_request: bool,
    /// secret in the webhook URL, so only your Lidarr can trigger rescans
    #[serde(default)]
    pub hook_token: String,
}

impl Settings {
    pub fn configured(&self) -> bool {
        !self.url.is_empty() && !self.api_key.is_empty()
    }

    pub fn load(dir: &Path) -> Settings {
        let mut s: Settings = std::fs::read_to_string(dir.join("lidarr.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| Settings { everyone_can_request: true, ..Default::default() });
        if let Ok(u) = std::env::var("VH_LIDARR_URL") {
            if !u.trim().is_empty() {
                s.url = u.trim().trim_end_matches('/').to_string();
            }
        }
        if let Ok(k) = std::env::var("VH_LIDARR_API_KEY") {
            if !k.trim().is_empty() {
                s.api_key = k.trim().to_string();
            }
        }
        if s.hook_token.is_empty() {
            s.hook_token = crate::auth::random_hex(16);
            s.save(dir);
        }
        s
    }

    pub fn save(&self, dir: &Path) {
        let tmp = dir.join("lidarr.json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join("lidarr.json"));
        }
    }
}

/// Who asked for what (shown on the Requests page).
#[derive(Serialize, Deserialize, Clone)]
pub struct Request {
    pub kind: String, // "artist" | "album"
    #[serde(rename = "foreignId")]
    pub foreign_id: String,
    pub title: String,
    pub artist: String,
    pub image: Option<String>,
    pub year: Option<String>,
    pub by: String,
    #[serde(rename = "byId")]
    pub by_id: String,
    pub at: u64,
    /// the requester has been notified that it arrived (old requests count as done)
    #[serde(default = "yes")]
    pub notified: bool,
}

pub fn load_requests(dir: &Path) -> Vec<Request> {
    std::fs::read_to_string(dir.join("requests.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_requests(dir: &Path, list: &[Request]) {
    let path: PathBuf = dir.join("requests.json");
    let tmp = dir.join("requests.json.tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/* ---------------- HTTP ---------------- */

pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Client {
        Client {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .user_agent(concat!("VolvixHome/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("http client"),
        }
    }

    async fn call(&self, s: &Settings, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value, String> {
        if !s.configured() {
            return Err("Lidarr isn't set up yet. An admin can add it under Settings → Lidarr.".into());
        }
        let url = format!("{}/api/v1/{}", s.url.trim_end_matches('/'), path.trim_start_matches('/'));
        let mut req = self.http.request(method, &url).header("X-Api-Key", &s.api_key);
        if let Some(b) = body {
            req = req.json(b);
        }
        let res = req.send().await.map_err(|e| {
            if e.is_timeout() {
                format!("Lidarr at {} didn't answer in time.", s.url)
            } else {
                format!("Can't reach Lidarr at {}. Check the address.", s.url)
            }
        })?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            return Err("Lidarr didn't accept the API key.".into());
        }
        if !status.is_success() {
            // Lidarr validation errors: [{ "errorMessage": "..." }] or { "message": "..." }
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let msg = v
                .get(0)
                .and_then(|e| e.get("errorMessage"))
                .or_else(|| v.get("message"))
                .and_then(|m| m.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("Lidarr said {}", status));
            return Err(msg);
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|_| "Lidarr sent something unexpected.".into())
    }

    pub async fn get(&self, s: &Settings, path: &str) -> Result<Value, String> {
        self.call(s, reqwest::Method::GET, path, None).await
    }
    pub async fn post(&self, s: &Settings, path: &str, body: &Value) -> Result<Value, String> {
        self.call(s, reqwest::Method::POST, path, Some(body)).await
    }
    pub async fn put(&self, s: &Settings, path: &str, body: &Value) -> Result<Value, String> {
        self.call(s, reqwest::Method::PUT, path, Some(body)).await
    }

    /// Version, root folders and profiles: what the settings screen needs.
    pub async fn overview(&self, s: &Settings) -> Result<Value, String> {
        let (status, roots, quality, metadata) = tokio::join!(
            self.get(s, "system/status"),
            self.get(s, "rootfolder"),
            self.get(s, "qualityprofile"),
            self.get(s, "metadataprofile")
        );
        let status = status?;
        let pick = |v: Result<Value, String>, f: &dyn Fn(&Value) -> Value| -> Vec<Value> {
            v.ok().and_then(|x| x.as_array().cloned()).unwrap_or_default().iter().map(f).collect()
        };
        Ok(json!({
            "version": status.get("version").cloned().unwrap_or(Value::Null),
            "rootFolders": pick(roots, &|r| json!({ "path": r["path"], "qualityProfileId": r["defaultQualityProfileId"], "metadataProfileId": r["defaultMetadataProfileId"] })),
            "qualityProfiles": pick(quality, &|p| json!({ "id": p["id"], "name": p["name"] })),
            "metadataProfiles": pick(metadata, &|p| json!({ "id": p["id"], "name": p["name"] })),
        }))
    }

    /// Fill in anything not chosen yet from Lidarr's first root folder / profiles.
    pub async fn defaults(&self, s: &Settings) -> Result<Settings, String> {
        let mut out = s.clone();
        if out.root_folder.is_empty() || out.quality_profile_id == 0 || out.metadata_profile_id == 0 {
            let o = self.overview(s).await?;
            let first = |k: &str| o[k].as_array().and_then(|a| a.first().cloned()).unwrap_or(Value::Null);
            if out.root_folder.is_empty() {
                out.root_folder = first("rootFolders")["path"].as_str().unwrap_or("").to_string();
            }
            if out.quality_profile_id == 0 {
                out.quality_profile_id = first("qualityProfiles")["id"].as_i64().unwrap_or(1);
            }
            if out.metadata_profile_id == 0 {
                out.metadata_profile_id = first("metadataProfiles")["id"].as_i64().unwrap_or(1);
            }
        }
        if out.root_folder.is_empty() {
            return Err("Lidarr has no root folder. Add your music folder in Lidarr → Settings → Media Management.".into());
        }
        Ok(out)
    }
}

/* ---------------- shaping Lidarr's objects for the app ---------------- */

pub fn image(v: &Value, kinds: &[&str]) -> Option<String> {
    let imgs = v.get("images")?.as_array()?;
    for k in kinds {
        if let Some(i) = imgs.iter().find(|i| i["coverType"].as_str() == Some(k)) {
            if let Some(u) = i["remoteUrl"].as_str().or_else(|| i["url"].as_str()) {
                if u.starts_with("http") {
                    return Some(u.to_string());
                }
            }
        }
    }
    None
}

pub fn year(v: &Value) -> Option<String> {
    v["releaseDate"].as_str().and_then(|d| d.get(0..4)).map(String::from)
}

pub fn in_lidarr(v: &Value) -> bool {
    v["id"].as_i64().unwrap_or(0) > 0
}

/// Profile / folder fields Lidarr needs when adding an artist.
pub fn artist_body(artist: &Value, s: &Settings, monitor_all: bool) -> Value {
    let mut a = artist.clone();
    a["qualityProfileId"] = json!(s.quality_profile_id);
    a["metadataProfileId"] = json!(s.metadata_profile_id);
    a["rootFolderPath"] = json!(s.root_folder);
    a["monitored"] = json!(true);
    a["monitorNewItems"] = json!(if monitor_all { "all" } else { "none" });
    a["addOptions"] = json!({ "monitor": if monitor_all { "all" } else { "none" }, "searchForMissingAlbums": monitor_all });
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_remote_images_and_years() {
        let v = json!({ "images": [{ "coverType": "disc", "url": "/x" }, { "coverType": "cover", "url": "/MediaCover/1.jpg", "remoteUrl": "https://img/1.jpg" }], "releaseDate": "2019-04-05T00:00:00Z", "id": 0 });
        assert_eq!(image(&v, &["cover"]).as_deref(), Some("https://img/1.jpg"));
        assert_eq!(image(&v, &["poster"]), None);
        assert_eq!(year(&v).as_deref(), Some("2019"));
        assert!(!in_lidarr(&v));
    }

    #[test]
    fn artist_body_sets_profiles() {
        let s = Settings { url: "x".into(), api_key: "k".into(), root_folder: "/music".into(), quality_profile_id: 2, metadata_profile_id: 3, everyone_can_request: true, hook_token: "t".into() };
        let b = artist_body(&json!({ "artistName": "A", "foreignArtistId": "mb1" }), &s, false);
        assert_eq!(b["rootFolderPath"], "/music");
        assert_eq!(b["qualityProfileId"], 2);
        assert_eq!(b["addOptions"]["monitor"], "none");
        assert_eq!(b["artistName"], "A");
    }
}
