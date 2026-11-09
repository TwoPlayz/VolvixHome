//! The household side of Volvix Home:
//!   - presence: what each person is listening to right now (only when they choose to share it)
//!   - notifications: playlist invites, party invites, "your request arrived"
//!   - shared playlists: playlists several people can edit. They live on the server (everyone needs the
//!     same copy), and every change is an operation applied here, so two people editing at once never
//!     overwrite each other.
//! Ordinary playlists stay in each person's own app data (see /api/me/data); a playlist the owner makes
//! public is visible on the Household page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::{now, random_hex};

fn write_json(path: &Path, value: &impl Serialize) {
    crate::vault::write(path, &serde_json::to_string_pretty(value).unwrap_or_default());
}

/* ---------------- notifications ---------------- */

#[derive(Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub data: Value,
    pub at: u64,
    #[serde(default)]
    pub read: bool,
    /// set once an invite has been answered: "accepted" | "declined" | "joined"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<String>,
}

/* ---------------- presence ---------------- */

#[derive(Clone)]
pub struct Presence {
    pub track_id: String,
    pub playing: bool,
    pub at: u64,
}

/// Someone counts as "listening now" if they reported playing in the last minute.
const LIVE_SECS: u64 = 60;

/* ---------------- shared playlists ---------------- */

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub id: String,
    #[serde(rename = "addedBy")]
    pub added_by: String,
    /// when it was added (seconds); 0 for songs added before this was tracked
    #[serde(default)]
    pub at: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Shared {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub owner: String,
    #[serde(default)]
    pub collaborators: Vec<String>,
    #[serde(default)]
    pub invited: Vec<String>,
    #[serde(default)]
    pub public: bool,
    pub tracks: Vec<Entry>,
    #[serde(default)]
    pub rev: u64,
}

impl Shared {
    fn can_edit(&self, uid: &str) -> bool {
        self.owner == uid || self.collaborators.iter().any(|c| c == uid)
    }
}

pub struct OpResult {
    pub playlist: Shared,
    /// the playlist no longer exists on the server (deleted, or handed back to its owner)
    pub gone: bool,
}

pub type Fail = (u16, String);

pub struct Social {
    dir: PathBuf,
    presence: Mutex<HashMap<String, Presence>>,
    notes: Mutex<HashMap<String, Vec<Note>>>,
    shared: Mutex<HashMap<String, Shared>>,
}

impl Social {
    pub fn load(dir: &Path) -> Social {
        let mut shared = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("shared")) {
            for e in rd.flatten() {
                if let Some(s) = crate::vault::read(&e.path()).and_then(|t| serde_json::from_str::<Shared>(&t).ok()) {
                    shared.insert(s.id.clone(), s);
                }
            }
        }
        Social { dir: dir.to_path_buf(), presence: Mutex::new(HashMap::new()), notes: Mutex::new(HashMap::new()), shared: Mutex::new(shared) }
    }

    /* ----- presence ----- */

    pub fn set_presence(&self, uid: &str, p: Option<Presence>) {
        let mut map = self.presence.lock().unwrap();
        match p {
            Some(p) => { map.insert(uid.to_string(), p); }
            None => { map.remove(uid); }
        }
    }

    /// (trackId, playing right now, last update) for a person, if they share.
    pub fn presence_of(&self, uid: &str) -> Option<(String, bool, u64)> {
        let p = self.presence.lock().unwrap().get(uid).cloned()?;
        let live = p.playing && p.at + LIVE_SECS >= now();
        Some((p.track_id, live, p.at))
    }

    /* ----- notifications ----- */

    fn notes_path(&self, uid: &str) -> PathBuf {
        self.dir.join("notifications").join(format!("{uid}.json"))
    }

    fn load_notes(&self, uid: &str) -> Vec<Note> {
        crate::vault::read(&self.notes_path(uid)).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Returns the new notification's id.
    pub fn add_note(&self, uid: &str, kind: &str, text: String, data: Value) -> String {
        let id = format!("n{}", random_hex(5));
        let mut map = self.notes.lock().unwrap();
        let list = map.entry(uid.to_string()).or_insert_with(|| self.load_notes(uid));
        list.insert(0, Note { id: id.clone(), kind: kind.to_string(), text, data, at: now(), read: false, done: None });
        list.truncate(100);
        write_json(&self.notes_path(uid), list);
        id
    }

    pub fn notes_for(&self, uid: &str) -> Vec<Note> {
        let mut map = self.notes.lock().unwrap();
        map.entry(uid.to_string()).or_insert_with(|| self.load_notes(uid)).clone()
    }

    /// Mark some (or all, with None) notifications read.
    pub fn mark_read(&self, uid: &str, ids: Option<&[String]>) {
        let mut map = self.notes.lock().unwrap();
        let list = map.entry(uid.to_string()).or_insert_with(|| self.load_notes(uid));
        for n in list.iter_mut() {
            if ids.map(|ids| ids.iter().any(|i| *i == n.id)).unwrap_or(true) {
                n.read = true;
            }
        }
        write_json(&self.notes_path(uid), list);
    }

    /// An invite was answered: mark its notification done (and read).
    pub fn set_done(&self, uid: &str, kind: &str, field: &str, value: &str, status: &str) {
        let mut map = self.notes.lock().unwrap();
        let list = map.entry(uid.to_string()).or_insert_with(|| self.load_notes(uid));
        for n in list.iter_mut().filter(|n| n.kind == kind && n.data[field].as_str() == Some(value)) {
            n.done = Some(status.to_string());
            n.read = true;
        }
        write_json(&self.notes_path(uid), list);
    }

    /* ----- shared playlists ----- */

    fn shared_path(&self, id: &str) -> PathBuf {
        self.dir.join("shared").join(format!("{id}.json"))
    }

    /// (playlists you own or collaborate on, playlists you're invited to)
    pub fn list_for(&self, uid: &str) -> (Vec<Shared>, Vec<Shared>) {
        let map = self.shared.lock().unwrap();
        let mut mine: Vec<Shared> = map.values().filter(|s| s.can_edit(uid)).cloned().collect();
        let mut invites: Vec<Shared> = map.values().filter(|s| s.invited.iter().any(|i| i == uid)).cloned().collect();
        mine.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        invites.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        (mine, invites)
    }

    pub fn create(&self, uid: &str, name: &str, description: &str, tracks: Vec<String>, public: bool) -> Shared {
        let s = Shared {
            id: format!("pl_{}", random_hex(6)),
            name: clean(name, 60, "New playlist"),
            description: clean(description, 200, ""),
            owner: uid.to_string(),
            collaborators: vec![],
            invited: vec![],
            public,
            tracks: tracks.into_iter().map(|id| Entry { id, added_by: uid.to_string(), at: crate::auth::now() }).collect(),
            rev: 1,
        };
        write_json(&self.shared_path(&s.id), &s);
        self.shared.lock().unwrap().insert(s.id.clone(), s.clone());
        s
    }

    /// Apply one change to a shared playlist. `names` maps user id -> display name (for notifications and
    /// to ignore invites for people who don't exist).
    pub fn op(&self, uid: &str, id: &str, body: &Value, names: &HashMap<String, String>) -> Result<OpResult, Fail> {
        let kind = body["op"].as_str().unwrap_or("");
        let mut map = self.shared.lock().unwrap();
        let pl = map.get_mut(id).ok_or((404u16, "That playlist doesn't exist any more.".to_string()))?;
        let me = names.get(uid).cloned().unwrap_or_else(|| "Someone".into());
        let forbidden = || (403u16, "You can't do that on this playlist.".to_string());
        let is_owner = pl.owner == uid;

        match kind {
            "accept" => {
                if !pl.invited.iter().any(|i| i == uid) { return Err(forbidden()); }
                pl.invited.retain(|i| i != uid);
                pl.collaborators.push(uid.to_string());
                self.set_done(uid, "playlist_invite", "playlistId", id, "accepted");
                self.add_note(&pl.owner, "playlist_joined", format!("{me} joined \"{}\"", pl.name), json!({ "playlistId": id }));
            }
            "decline" => {
                if !pl.invited.iter().any(|i| i == uid) { return Err(forbidden()); }
                pl.invited.retain(|i| i != uid);
                self.set_done(uid, "playlist_invite", "playlistId", id, "declined");
            }
            "leave" => {
                if !pl.collaborators.iter().any(|c| c == uid) { return Err(forbidden()); }
                pl.collaborators.retain(|c| c != uid);
            }
            "delete" | "unshare" => {
                if !is_owner { return Err(forbidden()); }
                let last = pl.clone();
                map.remove(id);
                let _ = std::fs::remove_file(self.shared_path(id));
                return Ok(OpResult { playlist: last, gone: true });
            }
            "update" => {
                if !is_owner { return Err(forbidden()); }
                if let Some(n) = body["name"].as_str() { pl.name = clean(n, 60, &pl.name); }
                if let Some(d) = body["description"].as_str() { pl.description = clean(d, 200, ""); }
                if let Some(p) = body["public"].as_bool() { pl.public = p; }
            }
            "invite" => {
                if !is_owner { return Err(forbidden()); }
                for v in body["userIds"].as_array().cloned().unwrap_or_default() {
                    let Some(u) = v.as_str() else { continue };
                    if u == uid || !names.contains_key(u) || pl.collaborators.iter().any(|c| c == u) || pl.invited.iter().any(|i| i == u) { continue; }
                    pl.invited.push(u.to_string());
                    self.add_note(u, "playlist_invite", format!("{me} invited you to edit \"{}\"", pl.name),
                        json!({ "playlistId": id, "playlistName": pl.name, "from": me }));
                }
            }
            "uninvite" => {
                if !is_owner { return Err(forbidden()); }
                let u = body["userId"].as_str().unwrap_or("");
                pl.collaborators.retain(|c| c != u);
                pl.invited.retain(|i| i != u);
            }
            "add" => {
                if !pl.can_edit(uid) { return Err(forbidden()); }
                for v in body["ids"].as_array().cloned().unwrap_or_default() {
                    if let Some(t) = v.as_str() {
                        if !pl.tracks.iter().any(|e| e.id == t) { pl.tracks.push(Entry { id: t.to_string(), added_by: uid.to_string(), at: crate::auth::now() }); }
                    }
                }
            }
            "remove" => {
                if !pl.can_edit(uid) { return Err(forbidden()); }
                let i = body["index"].as_u64().unwrap_or(u64::MAX) as usize;
                // the track id guards against removing the wrong row if the list changed underneath
                if i >= pl.tracks.len() || body["trackId"].as_str().map(|t| t != pl.tracks[i].id).unwrap_or(false) {
                    return Err((409, "That playlist just changed. Try again.".into()));
                }
                pl.tracks.remove(i);
            }
            "insert" => {
                if !pl.can_edit(uid) { return Err(forbidden()); }
                let i = (body["index"].as_u64().unwrap_or(0) as usize).min(pl.tracks.len());
                let t = body["entry"]["id"].as_str().ok_or((400u16, "Missing song.".to_string()))?;
                let by = body["entry"]["addedBy"].as_str().unwrap_or(uid).to_string();
                let at = body["entry"]["at"].as_u64().unwrap_or_else(crate::auth::now);
                pl.tracks.insert(i, Entry { id: t.to_string(), added_by: by, at });
            }
            "move" => {
                if !pl.can_edit(uid) { return Err(forbidden()); }
                let from = body["from"].as_u64().unwrap_or(u64::MAX) as usize;
                let to = body["to"].as_u64().unwrap_or(u64::MAX) as usize;
                if from >= pl.tracks.len() || to >= pl.tracks.len() { return Err((400, "Bad position.".into())); }
                let e = pl.tracks.remove(from);
                pl.tracks.insert(to, e);
            }
            _ => return Err((400, "Unknown change.".into())),
        }
        pl.rev += 1;
        write_json(&self.shared_path(id), pl);
        Ok(OpResult { playlist: pl.clone(), gone: false })
    }

    /* ----- household ----- */

    /// A person's playlists the household can see: public shared ones they own, and public local ones.
    pub fn public_playlists(&self, uid: &str) -> Vec<Value> {
        let mut out: Vec<Value> = self.shared.lock().unwrap().values().filter(|s| s.owner == uid && s.public).map(|s| json!({
            "id": s.id, "name": s.name, "description": s.description, "owner": uid, "shared": true,
            "collaborators": s.collaborators, "count": s.tracks.len(),
            "tracks": s.tracks.iter().map(|e| json!({ "id": e.id, "addedBy": e.added_by, "at": e.at })).collect::<Vec<_>>()
        })).collect();
        let blob = self.dir.join("userdata").join(format!("{uid}.json"));
        if let Some(v) = crate::vault::read(&blob).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            for p in v["playlists"].as_array().cloned().unwrap_or_default() {
                if p["public"].as_bool() != Some(true) || p["liked"].as_bool() == Some(true) { continue; }
                let tracks: Vec<Value> = p["tracks"].as_array().cloned().unwrap_or_default().iter()
                    .filter_map(|t| t["id"].as_str().map(|id| json!({ "id": id, "addedBy": uid, "at": t["at"].as_u64().map(|a| if a > 100_000_000_000 { a / 1000 } else { a }).unwrap_or(0) }))).collect();
                out.push(json!({
                    "id": p["id"], "name": p["name"], "description": p["description"], "owner": uid, "shared": false,
                    "collaborators": [], "count": tracks.len(), "tracks": tracks
                }));
            }
        }
        out
    }
}

/// Trim, cap the length, and fall back when empty.
fn clean(s: &str, max: usize, fallback: &str) -> String {
    let t: String = s.trim().chars().take(max).collect();
    if t.is_empty() { fallback.to_string() } else { t }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("vh-social-{}", random_hex(4)));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn names() -> HashMap<String, String> {
        [("a", "Alex"), ("b", "Sam"), ("c", "Nia")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    fn op(s: &Social, uid: &str, id: &str, body: Value) -> Result<OpResult, Fail> {
        s.op(uid, id, &body, &names())
    }

    #[test]
    fn invite_accept_edit_flow() {
        let dir = temp();
        let s = Social::load(&dir);
        let pl = s.create("a", "Road Trip", "", vec!["t1".into()], false);
        // only the owner invites
        assert_eq!(op(&s, "b", &pl.id, json!({ "op": "invite", "userIds": ["c"] })).err().unwrap().0, 403);
        let r = op(&s, "a", &pl.id, json!({ "op": "invite", "userIds": ["b", "c", "zzz", "a"] })).unwrap();
        assert_eq!(r.playlist.invited, vec!["b", "c"], "unknown users and the owner are skipped");
        // the invitee got a notification
        let n = s.notes_for("b");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].kind, "playlist_invite");
        // an invited (not yet accepted) person can't edit
        assert_eq!(op(&s, "b", &pl.id, json!({ "op": "add", "ids": ["t2"] })).err().unwrap().0, 403);
        // accept, then edit
        let r = op(&s, "b", &pl.id, json!({ "op": "accept" })).unwrap();
        assert_eq!(r.playlist.collaborators, vec!["b"]);
        assert_eq!(s.notes_for("b")[0].done.as_deref(), Some("accepted"));
        assert_eq!(s.notes_for("a")[0].kind, "playlist_joined");
        let r = op(&s, "b", &pl.id, json!({ "op": "add", "ids": ["t2", "t1", "t3"] })).unwrap();
        let ids: Vec<&str> = r.playlist.tracks.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["t1", "t2", "t3"], "duplicates are skipped");
        assert_eq!(r.playlist.tracks[1].added_by, "b");
        // move and remove
        op(&s, "b", &pl.id, json!({ "op": "move", "from": 2, "to": 0 })).unwrap();
        let r = op(&s, "a", &pl.id, json!({ "op": "remove", "index": 0, "trackId": "t3" })).unwrap();
        assert_eq!(r.playlist.tracks.len(), 2);
        assert_eq!(op(&s, "a", &pl.id, json!({ "op": "remove", "index": 0, "trackId": "wrong" })).err().unwrap().0, 409);
        // a collaborator can't rename or invite; the owner can
        assert_eq!(op(&s, "b", &pl.id, json!({ "op": "update", "name": "Mine now" })).err().unwrap().0, 403);
        assert_eq!(op(&s, "a", &pl.id, json!({ "op": "update", "name": "  Summer  ", "public": true })).unwrap().playlist.name, "Summer");
        // decline removes the invite
        let r = op(&s, "c", &pl.id, json!({ "op": "decline" })).unwrap();
        assert!(r.playlist.invited.is_empty());
        // leaving
        let r = op(&s, "b", &pl.id, json!({ "op": "leave" })).unwrap();
        assert!(r.playlist.collaborators.is_empty());
        // survives a restart
        let again = Social::load(&dir);
        let (mine, _) = again.list_for("a");
        assert_eq!(mine.len(), 1);
        assert!(mine[0].rev > 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_and_public_playlists() {
        let dir = temp();
        let s = Social::load(&dir);
        let pl = s.create("a", "Shared", "x", vec!["t1".into()], true);
        assert_eq!(s.public_playlists("a").len(), 1);
        assert!(s.public_playlists("b").is_empty());
        // local public playlists come from the person's own app data
        std::fs::create_dir_all(dir.join("userdata")).unwrap();
        std::fs::write(dir.join("userdata").join("b.json"), json!({ "playlists": [
            { "id": "pl_1", "name": "Mine", "public": true, "tracks": [{ "id": "t9" }] },
            { "id": "pl_2", "name": "Private", "tracks": [] },
            { "id": "pl_liked", "name": "Liked Songs", "public": true, "liked": true, "tracks": [] }
        ] }).to_string()).unwrap();
        let b = s.public_playlists("b");
        assert_eq!(b.len(), 1, "private and Liked Songs stay private");
        assert_eq!(b[0]["name"], "Mine");
        assert_eq!(op(&s, "b", &pl.id, json!({ "op": "delete" })).err().unwrap().0, 403);
        assert!(op(&s, "a", &pl.id, json!({ "op": "delete" })).unwrap().gone);
        assert!(s.list_for("a").0.is_empty());
        assert_eq!(op(&s, "a", &pl.id, json!({ "op": "add", "ids": ["t1"] })).err().unwrap().0, 404);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn notifications_read_state_and_presence() {
        let dir = temp();
        let s = Social::load(&dir);
        let a = s.add_note("a", "party_invite", "Sam invited you".into(), json!({ "partyId": "p1" }));
        s.add_note("a", "request_ready", "Ready".into(), json!({}));
        assert_eq!(s.notes_for("a").iter().filter(|n| !n.read).count(), 2);
        s.mark_read("a", Some(&[a.clone()]));
        assert_eq!(s.notes_for("a").iter().filter(|n| !n.read).count(), 1);
        s.set_done("a", "party_invite", "partyId", "p1", "joined");
        assert_eq!(s.notes_for("a").iter().find(|n| n.id == a).unwrap().done.as_deref(), Some("joined"));
        s.mark_read("a", None);
        assert!(s.notes_for("a").iter().all(|n| n.read));
        // presence: live only when playing and recent
        s.set_presence("a", Some(Presence { track_id: "t1".into(), playing: true, at: now() }));
        assert_eq!(s.presence_of("a").unwrap().1, true);
        s.set_presence("a", Some(Presence { track_id: "t1".into(), playing: true, at: now() - 120 }));
        assert_eq!(s.presence_of("a").unwrap().1, false, "stale report is not live");
        s.set_presence("a", None);
        assert!(s.presence_of("a").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
