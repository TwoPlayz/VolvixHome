//! The Volvix Home server's JSON API.
use std::io::Read;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::error::{Error, Result};
use crate::model::*;

/// One server, optionally signed in. Cheap to clone and safe to share between threads.
#[derive(Clone)]
pub struct Client {
    base: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never print the token
        f.debug_struct("Client").field("base", &self.base).field("signed_in", &self.token.is_some()).finish()
    }
}

/// Signed in: who, and the token to keep.
#[derive(Debug, Clone)]
pub struct SignedIn {
    pub user: User,
    pub token: String,
}

impl Client {
    /// `base` is the server's address as normalised by [`normalize_address`] ("https://music.example.com").
    pub fn new(base: &str, token: Option<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(8))
            .timeout_read(Duration::from_secs(30))
            .user_agent(concat!("VolvixHomeNative/", env!("CARGO_PKG_VERSION")))
            .build();
        Client { base: base.trim_end_matches('/').to_string(), token, agent }
    }

    /// Find the server someone typed ("nas.local:4747", "music.example.com", "https://…"): checks it
    /// answers as a Volvix Home server. Without a scheme it tries https first, then http.
    pub fn connect(typed: &str) -> Result<(Client, ServerStatus)> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Err(Error::Unreachable("no address".into()));
        }
        let tries: Vec<String> = if typed.contains("://") {
            vec![normalize_address(typed)]
        } else {
            let first = normalize_address(typed);
            let other = if first.starts_with("https://") { first.replacen("https://", "http://", 1) } else { first.replacen("http://", "https://", 1) };
            vec![first, other]
        };
        let mut last = Error::Unreachable("no address".into());
        for base in tries {
            let c = Client::new(&base, None);
            match c.status() {
                Ok(s) if s.name.contains("Volvix") => return Ok((c, s)),
                Ok(_) => last = Error::BadAnswer("it isn't a Volvix Home server".into()),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn base(&self) -> &str {
        &self.base
    }
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
    pub fn with_token(&self, token: Option<String>) -> Client {
        Client { base: self.base.clone(), token, agent: self.agent.clone() }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let mut r = self.agent.request(method, &self.url(path));
        if let Some(t) = &self.token {
            r = r.set("Authorization", &format!("Bearer {t}"));
        }
        r
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let res = self.request("GET", path).call()?;
        res.into_json::<T>().map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /* ---------- before signing in ---------- */

    pub fn status(&self) -> Result<ServerStatus> {
        self.get_json("/api/status")
    }

    /// Sign in with a name and password. The returned client carries the token.
    pub fn login(&self, name: &str, password: &str) -> Result<(Client, SignedIn)> {
        let res = self.request("POST", "/api/login").send_json(json!({ "name": name, "password": password }))?;
        self.finish_sign_in(res)
    }

    /// First run of a new server only: create the admin account.
    pub fn setup(&self, name: &str, password: &str) -> Result<(Client, SignedIn)> {
        let res = self.request("POST", "/api/setup").send_json(json!({ "name": name, "password": password }))?;
        self.finish_sign_in(res)
    }

    fn finish_sign_in(&self, res: ureq::Response) -> Result<(Client, SignedIn)> {
        #[derive(serde::Deserialize)]
        struct Answer {
            user: User,
            token: String,
        }
        let a: Answer = res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))?;
        let signed = SignedIn { user: a.user, token: a.token };
        Ok((self.with_token(Some(signed.token.clone())), signed))
    }

    /// Ends this sign-in on the server too.
    pub fn logout(&self) -> Result<()> {
        self.request("POST", "/api/logout").call()?;
        Ok(())
    }

    /* ---------- signed in ---------- */

    pub fn me(&self) -> Result<Me> {
        self.get_json("/api/me")
    }

    pub fn library(&self) -> Result<Catalog> {
        self.get_json("/api/library")
    }

    /// Your playlists, recently played and settings (shared with the website). `None` for a
    /// person who has never saved anything.
    pub fn user_data(&self) -> Result<Option<UserData>> {
        let v: serde_json::Value = self.get_json("/api/me/data")?;
        if v.is_null() {
            return Ok(None);
        }
        serde_json::from_value(v).map(Some).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    pub fn save_user_data(&self, data: &UserData) -> Result<()> {
        let mut data = data.clone();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        data.rest.insert("savedAt".into(), json!(now));
        self.request("PUT", "/api/me/data").send_json(serde_json::to_value(&data).map_err(|e| Error::BadAnswer(e.to_string()))?)?;
        Ok(())
    }

    /// Lyrics for a song: a .lrc next to it, its tags, or LRCLIB (the server decides).
    pub fn lyrics(&self, track_id: &str) -> Result<LyricsAnswer> {
        self.get_json(&format!("/api/lyrics/{}", enc(track_id)))
    }

    /* ---------- account ---------- */

    /// Change your own password. The server signs you in again with a fresh token (returned client).
    pub fn change_password(&self, current: &str, new: &str) -> Result<(Client, SignedIn)> {
        let res = self.request("POST", "/api/me/password").send_json(json!({ "current": current, "new": new }))?;
        self.finish_sign_in(res)
    }

    /// Change your display name.
    pub fn rename_me(&self, name: &str) -> Result<User> {
        #[derive(serde::Deserialize)]
        struct A {
            user: User,
        }
        let res = self.request("PUT", "/api/me/profile").send_json(json!({ "name": name }))?;
        res.into_json::<A>().map(|a| a.user).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /* ---------- household ---------- */

    /// Collaborative playlists you're on, and invites waiting for you.
    pub fn shared(&self) -> Result<SharedList> {
        self.get_json("/api/shared")
    }

    /// Make a playlist collaborative (it moves to the server; you stay the owner).
    pub fn create_shared(&self, name: &str, description: &str, public: bool, track_ids: &[String]) -> Result<SharedOpResult> {
        let res = self.request("POST", "/api/shared").send_json(json!({ "name": name, "description": description, "public": public, "tracks": track_ids }))?;
        res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// One change to a collaborative playlist: `{"op":"add","ids":[…]}`, `remove` (index, trackId), `move`
    /// (from, to), `update` (name, description, public), `invite` (userIds), `uninvite`, `accept`,
    /// `decline`, `leave`, `unshare`, `delete`.
    pub fn shared_op(&self, playlist_id: &str, op: serde_json::Value) -> Result<SharedOpResult> {
        let res = self.request("POST", &format!("/api/shared/{}/op", enc(playlist_id))).send_json(op)?;
        res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Everyone else: what they're playing and their public playlists.
    pub fn household(&self) -> Result<Household> {
        self.get_json("/api/household")
    }

    /// Tell the household what you're playing (`None` = nothing / private).
    pub fn set_now_playing(&self, track_id: Option<&str>, playing: bool) -> Result<()> {
        self.request("PUT", "/api/me/now").send_json(json!({ "trackId": track_id, "playing": playing }))?;
        Ok(())
    }

    pub fn notifications(&self) -> Result<Notes> {
        self.get_json("/api/notifications")
    }

    /// Mark some (or, with `None`, all) notifications read.
    pub fn mark_read(&self, ids: Option<&[String]>) -> Result<()> {
        let body = match ids {
            Some(ids) => json!({ "ids": ids }),
            None => json!({}),
        };
        self.request("POST", "/api/notifications/read").send_json(body)?;
        Ok(())
    }

    /* ---------- getting more music (Lidarr) ---------- */

    /// Look for music you don't have yet (MusicBrainz, through the server).
    pub fn find_music(&self, term: &str) -> Result<FindResults> {
        let q: String = term.trim().bytes().map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        }).collect();
        // MusicBrainz can be slow: give it longer than ordinary calls
        let res = self.request("GET", &format!("/api/lidarr/search?term={q}")).timeout(Duration::from_secs(45)).call()?;
        res.into_json().map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Ask Lidarr to get an album or everything by an artist.
    pub fn request_music(&self, kind: &str, foreign_id: &str) -> Result<()> {
        self.request("POST", "/api/lidarr/request").timeout(Duration::from_secs(60)).send_json(json!({ "kind": kind, "foreignId": foreign_id }))?;
        Ok(())
    }

    /// Take a request back (only whoever asked, or someone who manages members).
    pub fn cancel_request(&self, foreign_id: &str) -> Result<()> {
        self.request("DELETE", &format!("/api/lidarr/request/{}", enc(foreign_id))).call()?;
        Ok(())
    }

    pub fn requests(&self) -> Result<Vec<Request>> {
        #[derive(serde::Deserialize)]
        struct A {
            #[serde(default)]
            requests: Vec<Request>,
        }
        self.get_json::<A>("/api/lidarr/requests").map(|a| a.requests)
    }

    /// Downloads in progress in Lidarr (`items`: title, artist, progress 0..1, state).
    pub fn lidarr_queue(&self) -> Result<serde_json::Value> {
        self.get_json("/api/lidarr/queue")
    }

    /// A server-relative picture (e.g. a found album's "/api/discover/image/…").
    pub fn image_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let res = self.request("GET", path).call()?;
        let mut buf = Vec::new();
        res.into_reader().take(20_000_000).read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Where a song streams from. Players that can send headers use [`Client::auth_header`] with it.
    pub fn stream_url(&self, track_id: &str) -> String {
        self.url(&format!("/api/stream/{}", enc(track_id)))
    }

    /// The `Authorization` header value for requests made outside this client (e.g. AVPlayer).
    pub fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {t}"))
    }

    /// Part of a song: bytes from `start` (to the end). Returns the reader, the total size if the
    /// server said, and whether the server honoured the range.
    pub fn open_stream(&self, track_id: &str, start: u64) -> Result<(Box<dyn Read + Send + Sync>, Option<u64>, bool)> {
        // no overall timeout (a song can take a while to listen through); each read still times out
        let mut req = self.request("GET", &format!("/api/stream/{}", enc(track_id)));
        if start > 0 {
            req = req.set("Range", &format!("bytes={start}-"));
        }
        let res = req.call()?;
        let partial = res.status() == 206;
        let total = if partial {
            // Content-Range: bytes 100-999/1000
            res.header("Content-Range").and_then(|v| v.rsplit('/').next()).and_then(|n| n.parse().ok())
        } else {
            res.header("Content-Length").and_then(|n| n.parse().ok())
        };
        Ok((Box::new(res.into_reader()), total, partial))
    }

    /// Cover address for an album. `size` is 640 or 1200 (server-made smaller copies) or 0 for
    /// the original. With the cover's version in it, the address always means this exact picture.
    pub fn cover_path(album_id: &str, size: u32, version: u64) -> String {
        let mut p = format!("/api/cover/{}", enc(album_id));
        let mut q = vec![];
        if size == 640 || size == 1200 {
            q.push(format!("s={size}"));
        }
        if version > 0 {
            q.push(format!("v={version}"));
        }
        if !q.is_empty() {
            p.push('?');
            p.push_str(&q.join("&"));
        }
        p
    }

    /// The cover's bytes, straight from the server (see [`crate::covers::CoverCache`] for the cached way).
    pub fn cover_bytes(&self, album_id: &str, size: u32, version: u64) -> Result<Vec<u8>> {
        let res = self.request("GET", &Client::cover_path(album_id, size, version)).call()?;
        let mut buf = Vec::new();
        res.into_reader().take(30_000_000).read_to_end(&mut buf)?;
        Ok(buf)
    }
}

/// "nas.local:4747" -> "http://nas.local:4747"; "music.example.com/" -> "https://music.example.com".
/// Addresses on this computer or the home network default to http, everything else to https.
pub fn normalize_address(typed: &str) -> String {
    let t = typed.trim().trim_end_matches('/');
    if t.contains("://") {
        return t.to_string();
    }
    let host = t.split(['/', ':']).next().unwrap_or("").to_lowercase();
    let local = host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host.ends_with(".home")
        || !host.contains('.')
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || (host.starts_with("172.") && host.split('.').nth(1).and_then(|n| n.parse::<u8>().ok()).map(|n| (16..32).contains(&n)).unwrap_or(false));
    format!("{}://{t}", if local { "http" } else { "https" })
}

/// Percent-encode one path segment (ids are plain, but be safe).
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(normalize_address("nas.local:4747"), "http://nas.local:4747");
        assert_eq!(normalize_address("localhost:4747/"), "http://localhost:4747");
        assert_eq!(normalize_address("192.168.1.20:4747"), "http://192.168.1.20:4747");
        assert_eq!(normalize_address("172.20.0.5"), "http://172.20.0.5");
        assert_eq!(normalize_address("music.example.com"), "https://music.example.com");
        assert_eq!(normalize_address("https://music.example.com/"), "https://music.example.com");
        assert_eq!(normalize_address("http://x.example.com"), "http://x.example.com");
    }

    #[test]
    fn cover_paths() {
        assert_eq!(Client::cover_path("a1", 640, 17), "/api/cover/a1?s=640&v=17");
        assert_eq!(Client::cover_path("a1", 0, 0), "/api/cover/a1");
        assert_eq!(Client::cover_path("a1", 300, 5), "/api/cover/a1?v=5");
    }

    #[test]
    fn encoding() {
        assert_eq!(enc("abc123"), "abc123");
        assert_eq!(enc("a b/c"), "a%20b%2Fc");
    }
}
