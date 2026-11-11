//! Server settings the website shows under Settings → Library and Share links: the music folder, checking it
//! for new songs, and the share links that are open.
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::{enc, Client};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct LibraryFolder {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default, rename = "isDefault")]
    pub is_default: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Dir {
    pub name: String,
    pub path: String,
}

/// One folder on the server, with the folders inside it (the folder picker).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Browse {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub dirs: Vec<Dir>,
    #[serde(default, rename = "songsHere")]
    pub songs_here: u64,
    /// the server isn't allowed to read it
    #[serde(default)]
    pub denied: bool,
    /// it doesn't exist (this is the closest one that does)
    #[serde(default)]
    pub missing: bool,
    /// the server runs in Docker, so it only sees what was mounted into it
    #[serde(default)]
    pub docker: bool,
}

/// A share link that is open.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Share {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub by: String,
    /// when it stops working (unix seconds)
    #[serde(default)]
    pub expires: f64,
    #[serde(default)]
    pub path: String,
}

impl Share {
    /// "3 days left" / "5 hours left", as the website words it.
    pub fn time_left(&self, now: u64) -> String {
        let d = (self.expires - now as f64).max(0.0);
        let days = if (d / 86400.0 * 10.0).round() / 10.0 >= 1.0 { (d / 86400.0).round() as u64 } else { 0 };
        if days >= 1 {
            format!("{days} day{} left", if days > 1 { "s" } else { "" })
        } else {
            format!("{} hours left", ((d / 3600.0).round() as u64).max(1))
        }
    }
}

fn json_of(res: ureq::Response) -> Result<Value> {
    res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))
}

impl Client {
    pub fn library_folder(&self) -> Result<LibraryFolder> {
        let v: Value = self.get_json("/api/library/folder")?;
        serde_json::from_value(v).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    pub fn set_library_folder(&self, path: &str) -> Result<()> {
        self.request("PUT", "/api/library/folder").send_json(json!({ "path": path }))?;
        Ok(())
    }

    /// The folders inside `path` ("" = where the server starts).
    pub fn browse_folders(&self, path: &str) -> Result<Browse> {
        let url = if path.is_empty() { "/api/library/browse".to_string() } else { format!("/api/library/browse?path={}", enc(path)) };
        let v: Value = self.get_json(&url)?;
        serde_json::from_value(v).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Look in the music folder for added or removed songs now.
    pub fn rescan(&self) -> Result<()> {
        self.request("POST", "/api/library/rescan").send_json(json!({}))?;
        Ok(())
    }

    /// The Lidarr connection as the server has it: `{ configured, error, settings{..}, lidarr{ version, rootFolders, qualityProfiles, metadataProfiles } }`.
    pub fn lidarr_settings(&self) -> Result<Value> {
        self.get_json("/api/lidarr")
    }

    /// Save the Lidarr connection (an empty `url` disconnects). Answers with the same shape as [`Client::lidarr_settings`].
    pub fn save_lidarr(&self, body: Value) -> Result<Value> {
        json_of(self.request("PUT", "/api/lidarr").send_json(body)?)
    }

    pub fn shares(&self) -> Result<Vec<Share>> {
        let v = json_of(self.request("GET", "/api/shares").call()?)?;
        Ok(serde_json::from_value(v["shares"].clone()).unwrap_or_default())
    }

    pub fn cancel_share(&self, token: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/shares/{}", enc(token))).call()?;
        Ok(())
    }
}
