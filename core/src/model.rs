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
    /// the art is a picture someone chose for an album that had none
    #[serde(default, rename = "customCover")]
    pub custom_cover: bool,
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

/// A playlist as the website keeps it in your app data (and, with `shared`, as the server keeps a
/// collaborative one). Inside your own data you are `"u_me"`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Playlist {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub collaborative: bool,
    #[serde(default)]
    pub collaborators: Vec<String>,
    #[serde(default)]
    pub tracks: Vec<PlaylistEntry>,
    /// the built-in Liked Songs
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub liked: bool,
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

impl Playlist {
    pub fn track_ids(&self) -> Vec<String> {
        self.tracks.iter().map(|t| t.id.clone()).collect()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlaylistEntry {
    pub id: String,
    #[serde(default, rename = "addedBy")]
    pub added_by: String,
    #[serde(default)]
    pub at: u64,
}

/// How a drawn cover looks: a background, optionally its two colours, a font and the text colour (the website's
/// `look`). Colours are CSS hex strings; absent = the background's own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub bg: String,
    pub font: String,
    #[serde(default)]
    pub c1: Option<String>,
    #[serde(default)]
    pub c2: Option<String>,
    #[serde(default)]
    pub ink: Option<String>,
}

impl Look {
    pub fn from_value(v: &serde_json::Value) -> Option<Look> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from);
        Some(Look { bg: s("bg")?, font: s("font")?, c1: s("c1"), c2: s("c2"), ink: s("ink") })
    }
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!({ "bg": self.bg, "font": self.font, "c1": self.c1, "c2": self.c2, "ink": self.ink })
    }
}

/// A collaborative playlist as `/api/shared` sends it (people by their server ids).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SharedPlaylist {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub collaborators: Vec<String>,
    #[serde(default)]
    pub invited: Vec<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub tracks: Vec<PlaylistEntry>,
    #[serde(default)]
    pub rev: u64,
}

/// An invite to someone else's collaborative playlist.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SharedInvite {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default, rename = "ownerName")]
    pub owner_name: String,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SharedList {
    #[serde(default)]
    pub playlists: Vec<SharedPlaylist>,
    #[serde(default)]
    pub invites: Vec<SharedInvite>,
}

/// `/api/shared/{id}/op`'s answer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SharedOpResult {
    pub playlist: SharedPlaylist,
    #[serde(default)]
    pub gone: bool,
}

/// Something for you: a playlist invite, a party invite, requested music that's ready…
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub at: u64,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub done: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Notes {
    #[serde(default)]
    pub items: Vec<Note>,
    #[serde(default)]
    pub unread: usize,
}

/// Someone else in the household: what they're playing and their public playlists.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// the song they're on (a track, plus `listening` = playing in the last minute)
    #[serde(default)]
    pub now: Option<serde_json::Value>,
    #[serde(default)]
    pub playlists: Vec<SharedPlaylist>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Household {
    #[serde(default)]
    pub members: Vec<Member>,
}

/// `/api/lyrics/{id}`: synced lines (with optional word timing) or plain text.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LyricsAnswer {
    #[serde(default)]
    pub lyrics: Option<Lyrics>,
    /// the server may look lyrics up online
    #[serde(default)]
    pub online: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Lyrics {
    #[serde(default)]
    pub synced: Vec<LyricLine>,
    #[serde(default)]
    pub plain: String,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LyricLine {
    pub ms: i64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub words: Vec<LyricWord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LyricWord {
    pub ms: i64,
    #[serde(default)]
    pub text: String,
}

impl Lyrics {
    /// The line being sung at `ms` (shown a touch early, as the eye is faster than the ear).
    pub fn line_at(&self, ms: i64) -> Option<usize> {
        let ms = ms + 250;
        let n = self.synced.partition_point(|l| l.ms <= ms);
        n.checked_sub(1)
    }
}

/// Music you don't have yet, found through MusicBrainz (`/api/lidarr/search`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FindResults {
    #[serde(default)]
    pub artists: Vec<FoundItem>,
    #[serde(default)]
    pub albums: Vec<FoundItem>,
    #[serde(default)]
    pub songs: Vec<FoundItem>,
    #[serde(default, rename = "canRequest")]
    pub can_request: bool,
    #[serde(default)]
    pub configured: bool,
}

/// One artist, album or song found. Albums and artists have `foreign_id`; songs point at the album
/// (or single) to get.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FoundItem {
    #[serde(default, rename = "foreignId")]
    pub foreign_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default, rename = "albumId")]
    pub album_id: Option<String>,
    #[serde(default, rename = "singleId")]
    pub single_id: Option<String>,
    /// the server's address for its picture ("/api/discover/image/…")
    #[serde(default)]
    pub image: Option<String>,
    /// already in your library: the album id (albums) / artist name (artists) / true (songs)
    #[serde(default, rename = "onServer")]
    pub on_server: serde_json::Value,
    #[serde(default)]
    pub requested: bool,
}

impl FoundItem {
    pub fn label(&self) -> String {
        self.title.clone().or_else(|| self.name.clone()).unwrap_or_default()
    }
    pub fn is_on_server(&self) -> bool {
        !(self.on_server.is_null() || self.on_server == serde_json::Value::Bool(false))
    }
    /// What to hand Lidarr to get this: ("album" | "artist", id).
    pub fn request_target(&self, is_artist: bool) -> Option<(&'static str, String)> {
        if is_artist {
            return self.foreign_id.clone().map(|id| ("artist", id));
        }
        self.foreign_id.clone().or_else(|| self.album_id.clone()).or_else(|| self.single_id.clone()).map(|id| ("album", id))
    }
}

/// Music someone asked Lidarr for (`/api/lidarr/requests`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub kind: String,
    /// who asked
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub at: u64,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    /// something went wrong after Lidarr accepted it
    #[serde(default)]
    pub problem: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default, rename = "foreignId")]
    pub foreign_id: String,
    #[serde(default, rename = "onServer")]
    pub on_server: serde_json::Value,
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
        let src = r#"{"v":1,"theme":"dark","recent":["a1"],"playlists":[{"id":"p1","name":"Mine","tracks":[{"id":"t1","addedBy":"u_me","at":5}],"owner":"u_me","cover":"x"}],"settings":{"gapless":true}}"#;
        let d: UserData = serde_json::from_str(src).unwrap();
        assert_eq!(d.recent, vec!["a1"]);
        assert_eq!(d.playlists[0].track_ids(), vec!["t1"]);
        let back: serde_json::Value = serde_json::to_value(&d).unwrap();
        assert_eq!(back["theme"], "dark");
        assert_eq!(back["settings"]["gapless"], true);
        assert_eq!(back["playlists"][0]["owner"], "u_me");
        assert_eq!(back["playlists"][0]["cover"], "x");
        assert_eq!(back["playlists"][0]["tracks"][0]["addedBy"], "u_me");
        assert!(back["playlists"][0].get("liked").is_none(), "liked isn't written for ordinary playlists");
    }

    #[test]
    fn lyric_lines() {
        let ly = Lyrics { synced: [0, 1000, 5000].iter().map(|&ms| LyricLine { ms, ..Default::default() }).collect(), ..Default::default() };
        assert_eq!(ly.line_at(-500), None);
        assert_eq!(ly.line_at(800), Some(1), "a touch early");
        assert_eq!(ly.line_at(4000), Some(1));
        assert_eq!(ly.line_at(99_000), Some(2));
    }

    #[test]
    fn found_items() {
        let song: FoundItem = serde_json::from_str(r#"{"title":"North Star","artist":"X","albumId":null,"singleId":"s1","onServer":false}"#).unwrap();
        assert_eq!(song.request_target(false), Some(("album", "s1".into())));
        assert!(!song.is_on_server());
        let album: FoundItem = serde_json::from_str(r#"{"foreignId":"rg1","title":"A","onServer":"a9"}"#).unwrap();
        assert!(album.is_on_server());
        assert_eq!(album.request_target(false), Some(("album", "rg1".into())));
    }
}
