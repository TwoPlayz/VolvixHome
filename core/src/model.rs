//! What the server sends, as Rust types. Every field the server may leave out has a default, so an
//! older or newer server still loads.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Quality {
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub lossless: bool,
    #[serde(default)]
    pub bits: Option<u8>,
    #[serde(default)]
    pub khz: Option<f64>,
    #[serde(default)]
    pub kbps: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default, rename = "catalogNumber")]
    pub catalog_number: Option<String>,
    #[serde(default, rename = "trackIds")]
    pub track_ids: Vec<String>,
    /// the album has real cover art (otherwise the app draws one)
    #[serde(default)]
    pub cover: bool,
    /// the cover's version: changes whenever the picture does, so covers are cached for good under it
    #[serde(default, rename = "coverV")]
    pub cover_v: u64,
    #[serde(default)]
    pub quality: Quality,
    /// newest file time (seconds), for "Recently added"
    #[serde(default)]
    pub added: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Credit {
    pub role: String,
    pub names: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct ReplayGain {
    #[serde(default)]
    pub track: Option<f32>,
    #[serde(default)]
    pub album: Option<f32>,
    #[serde(default)]
    pub peak: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(rename = "albumId")]
    pub album_id: String,
    #[serde(default)]
    pub no: u32,
    #[serde(default = "one")]
    pub disc: u32,
    /// seconds
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub credits: Vec<Credit>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default, rename = "replayGain")]
    pub replay_gain: Option<ReplayGain>,
}

fn one() -> u32 {
    1
}

/// `/api/library` as sent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// the drawn look chosen for albums without art: album id -> { bg, font }
    #[serde(default)]
    pub covers: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub scanning: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub admin: bool,
    #[serde(default, rename = "hideExplicit")]
    pub hide_explicit: bool,
    /// profile picture version; 0 = none
    #[serde(default)]
    pub avatar: u64,
    #[serde(default)]
    pub perms: Vec<String>,
}

impl User {
    pub fn can(&self, perm: &str) -> bool {
        self.admin || self.perms.iter().any(|p| p == perm)
    }
}

/// `/api/status`: who the server is, before signing in.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerStatus {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "setupNeeded")]
    pub setup_needed: bool,
    #[serde(default)]
    pub scanning: bool,
    #[serde(default)]
    pub albums: u64,
    #[serde(default)]
    pub tracks: u64,
}

/// `/api/me`: the signed-in person and what the server can do for them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Me {
    pub user: User,
    #[serde(default)]
    pub lidarr: LidarrAccess,
    #[serde(default)]
    pub transcoding: bool,
    #[serde(default, rename = "artistInfo")]
    pub artist_info: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LidarrAccess {
    #[serde(default)]
    pub configured: bool,
    #[serde(default, rename = "canRequest")]
    pub can_request: bool,
}

/// The per-person app data the website keeps in `/api/me/data` (playlists, recently played,
/// settings). Only the parts the native apps use are typed; everything else is kept as it is, so
/// saving it back never loses what the website stored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserData {
    /// album ids, most recent first
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "trackIds")]
    pub track_ids: Vec<String>,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// "3:07", "1:02:45"
pub fn format_duration(seconds: u32) -> String {
    let (h, m, s) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(187), "3:07");
        assert_eq!(format_duration(3765), "1:02:45");
    }

    #[test]
    fn track_defaults_when_fields_missing() {
        let t: Track = serde_json::from_str(r#"{"id":"t1","title":"A","artist":"B","albumId":"a1"}"#).unwrap();
        assert_eq!(t.disc, 1);
        assert_eq!(t.no, 0);
        assert!(t.replay_gain.is_none());
    }

    #[test]
    fn user_data_keeps_unknown_fields() {
        let src = r#"{"v":1,"theme":"dark","recent":["a1"],"playlists":[{"id":"p1","name":"Mine","trackIds":["t1"],"owner":"u1"}],"settings":{"gapless":true}}"#;
        let d: UserData = serde_json::from_str(src).unwrap();
        assert_eq!(d.recent, vec!["a1"]);
        assert_eq!(d.playlists[0].track_ids, vec!["t1"]);
        let back: serde_json::Value = serde_json::to_value(&d).unwrap();
        assert_eq!(back["theme"], "dark");
        assert_eq!(back["settings"]["gapless"], true);
        assert_eq!(back["playlists"][0]["owner"], "u1");
    }
}
