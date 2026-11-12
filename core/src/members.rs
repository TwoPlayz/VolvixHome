//! The people who can sign in to this server (the website's Account → Members).
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::{enc, Client};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Member {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub perms: Vec<String>,
    #[serde(default, rename = "hideExplicit")]
    pub hide_explicit: bool,
    /// profile picture version; 0 = none
    #[serde(default)]
    pub avatar: u64,
}

/// All the things a member can be allowed to do (web/js/views/account.js PERMS).
pub const ALL_PERMISSIONS: usize = 6;

impl Member {
    /// "Admin", "Member", or "Member · custom permissions", as the website words it.
    pub fn role(&self) -> &'static str {
        if self.admin || self.perms.len() == ALL_PERMISSIONS {
            "Admin"
        } else if self.perms.len() == 2 && self.perms.iter().any(|p| p == "request") && self.perms.iter().any(|p| p == "parties") {
            "Member"
        } else {
            "Member · custom permissions"
        }
    }
}

impl Client {
    pub fn members(&self) -> Result<Vec<Member>> {
        let v: Value = self.get_json("/api/users")?;
        Ok(serde_json::from_value(v["users"].clone()).unwrap_or_default())
    }

    /// Change what a member may do (`{"perms": [..]}`) or their explicit-music setting (`{"hideExplicit": bool}`).
    /// Answers with the member as the server now has them.
    pub fn edit_member(&self, id: &str, body: Value) -> Result<Member> {
        let res = self.request("PUT", &format!("/api/users/{}", enc(id))).send_json(body)?;
        let v: Value = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        serde_json::from_value(v["user"].clone()).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Your profile picture: a JPEG (the website saves a 256 px square). Answers with its new version.
    pub fn set_avatar(&self, jpeg: &[u8]) -> Result<u64> {
        let res = self.request("PUT", "/api/me/avatar").set("Content-Type", "image/jpeg").send_bytes(jpeg)?;
        let v: Value = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        Ok(v["avatar"].as_u64().unwrap_or(1))
    }

    /// Choose how an album without cover art is drawn (for everyone on the server).
    pub fn set_album_look(&self, album: &str, look: &crate::model::Look) -> Result<()> {
        self.request("PUT", &format!("/api/album/{}/art", enc(album))).send_json(look.to_value())?;
        Ok(())
    }

    /// A picture for an album that has no cover art (a JPEG, a square). Answers with the album's new cover version.
    pub fn set_album_picture(&self, album: &str, jpeg: &[u8]) -> Result<u64> {
        let res = self.request("PUT", &format!("/api/album/{}/picture", enc(album))).set("Content-Type", "image/jpeg").send_bytes(jpeg)?;
        let v: Value = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        Ok(v["coverV"].as_u64().unwrap_or(1))
    }

    pub fn clear_album_picture(&self, album: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/album/{}/picture", enc(album))).call()?;
        Ok(())
    }

    pub fn clear_album_look(&self, album: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/album/{}/art", enc(album))).call()?;
        Ok(())
    }

    pub fn remove_avatar(&self) -> Result<()> {
        self.request("DELETE", "/api/me/avatar").call()?;
        Ok(())
    }

    /// The picture for one of your playlists (a JPEG, a 512 px square). Answers with its new version.
    pub fn set_playlist_cover(&self, playlist: &str, jpeg: &[u8]) -> Result<u64> {
        let res = self.request("PUT", &format!("/api/playlists/{}/cover", enc(playlist))).set("Content-Type", "image/jpeg").send_bytes(jpeg)?;
        let v: Value = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        Ok(v["cover"].as_u64().unwrap_or(1))
    }

    pub fn remove_playlist_cover(&self, playlist: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/playlists/{}/cover", enc(playlist))).call()?;
        Ok(())
    }

    /// A playlist's chosen picture (`owner` is the server id of whoever made the playlist).
    pub fn playlist_cover_bytes(&self, owner: &str, playlist: &str, version: u64) -> Result<Vec<u8>> {
        self.image_bytes(&format!("/api/playlist-cover/{}/{}?v={version}", enc(owner), enc(playlist)))
    }

    /// Someone's picture as the server has it (`version` keeps a changed picture from coming from a cache).
    pub fn avatar_bytes(&self, id: &str, version: u64) -> Result<Vec<u8>> {
        self.image_bytes(&format!("/api/avatar/{}?v={version}", enc(id)))
    }

    /// Set a new password for a member (they're signed out everywhere).
    pub fn set_member_password(&self, id: &str, password: &str) -> Result<()> {
        self.request("POST", &format!("/api/users/{}/password", enc(id))).send_json(json!({ "password": password }))?;
        Ok(())
    }

    pub fn remove_member(&self, id: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/users/{}", enc(id))).call()?;
        Ok(())
    }

    /// Add a member (they get the basics; permissions are changed afterwards).
    pub fn add_member(&self, name: &str, password: &str) -> Result<()> {
        self.request("POST", "/api/users").send_json(json!({ "name": name, "password": password }))?;
        Ok(())
    }
}
