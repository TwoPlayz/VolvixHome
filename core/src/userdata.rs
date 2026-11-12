//! Changes to your app data (`/api/me/data`), the same data the website keeps: playlists, Liked
//! Songs, recently played and play stats.
//!
//! The website saves the whole document, newest copy wins. So the apps never send their own old copy:
//! each change is an [`Edit`], and saving means "fetch the server's copy, apply the edits not saved
//! yet, send it back" ([`save_edits`]). A playlist made on the website a minute ago survives.
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::error::Result;
use crate::model::{Playlist, PlaylistEntry, UserData};

pub const LIKED_ID: &str = "pl_liked";
/// You, inside your own app data.
pub const ME: &str = "u_me";

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// an album was played (Home's "Recently played")
    Recent(String),
    /// a song counted as played (after 30 s, or half of a short song)
    Play { track: String, at: u64 },
    /// seconds listened, for the yearly Rewind
    Listened { at: u64, secs: u64 },
    CreatePlaylist(Playlist),
    UpdatePlaylist { id: String, name: Option<String>, description: Option<String>, public: Option<bool> },
    DeletePlaylist(String),
    AddTracks { id: String, tracks: Vec<String>, at: u64 },
    /// remove the song at `index`, checked against its id (if the list changed, the first match goes)
    RemoveAt { id: String, index: usize, track: String },
    Move { id: String, from: usize, to: usize },
    Like { track: String, on: bool, at: u64 },
    /// one of the website's settings (`settings.leveling`, `settings.shareActivity`, …)
    Setting(String, Value),
    /// the website's theme: "system", "dark" or "light"
    Theme(String),
    /// follow or stop following an artist (Library → Artists → Following)
    FollowArtist { name: String, on: bool },
    /// the picture chosen for one of your playlists (0 removes it)
    PlaylistCover { id: String, version: u64 },
    /// the drawn look of a playlist's cover (None = the automatic one)
    PlaylistLook { id: String, look: Option<serde_json::Value> },
}

/// One of the website's settings, or `default` when unset.
pub fn setting(d: &UserData, key: &str) -> Option<Value> {
    d.rest.get("settings").and_then(|s| s.get(key)).cloned()
}

/// Now, in seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The calendar year (UTC) of a unix time.
pub fn year_of(secs: u64) -> i64 {
    // days since 1970-01-01 -> civil year (Howard Hinnant's algorithm)
    let z = (secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + if m <= 2 { 1 } else { 0 }
}

/// New playlist id, in the website's style ("pl_…").
pub fn new_playlist_id() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) as u64;
    format!("pl_{}{}", radix36(t), radix36(fastrand::u64(36 * 36..36 * 36 * 36)))
}

fn radix36(mut n: u64) -> String {
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![];
    loop {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn entry(id: &str, at: u64) -> PlaylistEntry {
    PlaylistEntry { id: id.to_string(), added_by: ME.into(), at }
}

fn stats_root(d: &mut UserData) -> &mut Map<String, Value> {
    let s = d.rest.entry("stats").or_insert_with(|| json!({ "tracks": {}, "years": {} }));
    if !s.is_object() {
        *s = json!({ "tracks": {}, "years": {} });
    }
    let o = s.as_object_mut().expect("object");
    for k in ["tracks", "years"] {
        if !o.get(k).map(Value::is_object).unwrap_or(false) {
            o.insert(k.into(), json!({}));
        }
    }
    o
}

fn year_entry(root: &mut Map<String, Value>, year: i64) -> &mut Map<String, Value> {
    let years = root.get_mut("years").and_then(Value::as_object_mut).expect("years");
    let y = years.entry(year.to_string()).or_insert_with(|| json!({ "plays": {}, "sec": 0 }));
    if !y.is_object() {
        *y = json!({ "plays": {}, "sec": 0 });
    }
    let y = y.as_object_mut().expect("object");
    if !y.get("plays").map(Value::is_object).unwrap_or(false) {
        y.insert("plays".into(), json!({}));
    }
    y
}

/// Apply one change to the data.
pub fn apply(d: &mut UserData, e: &Edit) {
    match e {
        Edit::Recent(album) => {
            d.recent.retain(|a| a != album);
            d.recent.insert(0, album.clone());
            d.recent.truncate(30);
        }
        Edit::Play { track, at } => {
            let root = stats_root(d);
            let tracks = root.get_mut("tracks").and_then(Value::as_object_mut).expect("tracks");
            let t = tracks.entry(track.clone()).or_insert_with(|| json!({ "first": at, "n": 0 }));
            let n = t["n"].as_u64().unwrap_or(0) + 1;
            t["n"] = json!(n);
            t["last"] = json!(at);
            // a short play history, so recent listening counts more in mixes
            let log = root.entry("log").or_insert_with(|| json!([]));
            if !log.is_array() {
                *log = json!([]);
            }
            let list = log.as_array_mut().expect("array");
            list.push(json!([track, at]));
            if list.len() > 3000 {
                let cut = list.len() - 3000;
                list.drain(0..cut);
            }
            let y = year_entry(root, year_of(*at));
            let plays = y.get_mut("plays").and_then(Value::as_object_mut).expect("plays");
            let c = plays.get(track).and_then(Value::as_u64).unwrap_or(0) + 1;
            plays.insert(track.clone(), json!(c));
        }
        Edit::Listened { at, secs } => {
            let root = stats_root(d);
            let y = year_entry(root, year_of(*at));
            let s = y.get("sec").and_then(Value::as_f64).unwrap_or(0.0) + *secs as f64;
            y.insert("sec".into(), json!(s));
        }
        Edit::CreatePlaylist(pl) => {
            if d.playlists.iter().any(|p| p.id == pl.id) {
                return;
            }
            // new playlists go under Liked Songs, which stays first
            let at = if d.playlists.first().map(|p| p.liked || p.id == LIKED_ID).unwrap_or(false) { 1 } else { 0 };
            d.playlists.insert(at, pl.clone());
        }
        Edit::UpdatePlaylist { id, name, description, public } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                if let Some(n) = name.as_ref().map(|n| n.trim()).filter(|n| !n.is_empty()) {
                    p.name = n.to_string();
                }
                if let Some(x) = description {
                    p.description = x.trim().to_string();
                }
                if let Some(x) = public {
                    p.public = *x;
                }
            }
        }
        Edit::DeletePlaylist(id) => {
            if id != LIKED_ID {
                d.playlists.retain(|p| p.id != *id);
            }
        }
        Edit::AddTracks { id, tracks, at } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                for t in tracks {
                    if !p.tracks.iter().any(|x| x.id == *t) {
                        p.tracks.push(entry(t, *at));
                    }
                }
            }
        }
        Edit::RemoveAt { id, index, track } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                if p.tracks.get(*index).map(|t| t.id == *track).unwrap_or(false) {
                    p.tracks.remove(*index);
                } else if let Some(i) = p.tracks.iter().position(|t| t.id == *track) {
                    p.tracks.remove(i);
                }
            }
        }
        Edit::Move { id, from, to } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                if *from < p.tracks.len() && from != to {
                    let item = p.tracks.remove(*from);
                    let to = (*to).min(p.tracks.len());
                    p.tracks.insert(to, item);
                }
            }
        }
        Edit::Like { track, on, at } => {
            let idx = match d.playlists.iter().position(|p| p.id == LIKED_ID) {
                Some(i) => i,
                None => {
                    if !on {
                        return;
                    }
                    d.playlists.insert(
                        0,
                        Playlist {
                            id: LIKED_ID.into(),
                            name: "Liked Songs".into(),
                            description: "Songs you've liked.".into(),
                            owner: ME.into(),
                            liked: true,
                            ..Default::default()
                        },
                    );
                    0
                }
            };
            let p = &mut d.playlists[idx];
            p.tracks.retain(|t| t.id != *track);
            if *on {
                // the newest like goes on top
                p.tracks.insert(0, entry(track, *at));
            }
        }
        Edit::Setting(key, value) => {
            let s = d.rest.entry("settings").or_insert_with(|| json!({}));
            if !s.is_object() {
                *s = json!({});
            }
            s.as_object_mut().expect("object").insert(key.clone(), value.clone());
        }
        Edit::Theme(t) => {
            d.rest.insert("theme".into(), json!(t));
        }
        Edit::FollowArtist { name, on } => {
            let list = d.rest.entry("followedArtists").or_insert_with(|| json!([]));
            if !list.is_array() {
                *list = json!([]);
            }
            let list = list.as_array_mut().expect("array");
            list.retain(|n| n.as_str().map(|n| n.to_lowercase() != name.to_lowercase()).unwrap_or(false));
            if *on {
                list.push(json!(name));
            }
        }
        Edit::PlaylistLook { id, look } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                match look {
                    Some(l) => {
                        p.rest.insert("look".into(), l.clone());
                    }
                    None => {
                        p.rest.remove("look");
                    }
                }
            }
        }
        Edit::PlaylistCover { id, version } => {
            if let Some(p) = d.playlists.iter_mut().find(|p| p.id == *id) {
                if *version == 0 {
                    p.rest.remove("cover");
                } else {
                    p.rest.insert("cover".into(), json!(version));
                }
            }
        }
    }
}

pub fn is_liked(d: &UserData, track: &str) -> bool {
    d.playlists.iter().find(|p| p.id == LIKED_ID).map(|p| p.tracks.iter().any(|t| t.id == track)).unwrap_or(false)
}

/// Save edits safely: fetch the server's copy, apply them on top, send it back. Returns the new copy.
pub fn save_edits(client: &Client, edits: &[Edit]) -> Result<UserData> {
    let mut d = client.user_data()?.unwrap_or_default();
    for e in edits {
        apply(&mut d, e);
    }
    client.save_user_data(&d)?;
    Ok(d)
}

/// Your listening, read from the data: plays per song (count, last time) and the play log.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// track -> (times played, last played)
    pub plays: HashMap<String, (u64, u64)>,
    /// (track, when), oldest first
    pub log: Vec<(String, u64)>,
}

pub fn stats(d: &UserData) -> Stats {
    let s = d.rest.get("stats");
    let plays = s
        .and_then(|s| s.get("tracks"))
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), (v["n"].as_u64().unwrap_or(0), v["last"].as_u64().unwrap_or(0)))).collect())
        .unwrap_or_default();
    let log = s
        .and_then(|s| s.get("log"))
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|e| Some((e.get(0)?.as_str()?.to_string(), e.get(1)?.as_u64()?))).collect())
        .unwrap_or_default();
    Stats { plays, log }
}

/// Counts a song as played after 30 seconds (or half of a shorter song), like the website.
#[derive(Debug, Default)]
pub struct PlayCounter {
    track: Option<String>,
    heard: f64,
    counted: bool,
    pending_secs: f64,
}

impl PlayCounter {
    /// Call while a song plays; `dt` = seconds since the last call. Returns edits to save.
    pub fn tick(&mut self, track: &str, duration: u32, dt: f64) -> Vec<Edit> {
        let mut out = vec![];
        if self.track.as_deref() != Some(track) {
            self.track = Some(track.to_string());
            self.heard = 0.0;
            self.counted = false;
        }
        self.heard += dt;
        self.pending_secs += dt;
        let need = 30f64.min(if duration > 0 { duration as f64 } else { 60.0 } / 2.0);
        if !self.counted && self.heard >= need {
            self.counted = true;
            out.push(Edit::Play { track: track.to_string(), at: now() });
        } else if self.counted && duration > 0 && self.heard >= duration as f64 {
            // looped (repeat one): counts again
            self.heard = 0.0;
            self.counted = false;
        }
        if self.pending_secs >= 60.0 {
            out.push(Edit::Listened { at: now(), secs: self.pending_secs as u64 });
            self.pending_secs -= self.pending_secs.floor();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(1_704_067_199), 2023); // 2023-12-31 23:59:59
        assert_eq!(year_of(1_704_067_200), 2024);
        assert_eq!(year_of(1_798_675_200), 2026); // 2026-12-31
    }

    #[test]
    fn playlists_edit() {
        let mut d = UserData::default();
        apply(&mut d, &Edit::Like { track: "t1".into(), on: true, at: 1 });
        assert!(is_liked(&d, "t1"));
        let pl = Playlist { id: "p1".into(), name: "Road".into(), owner: ME.into(), ..Default::default() };
        apply(&mut d, &Edit::CreatePlaylist(pl));
        assert_eq!(d.playlists[0].id, LIKED_ID, "Liked Songs stays first");
        assert_eq!(d.playlists[1].id, "p1");
        apply(&mut d, &Edit::AddTracks { id: "p1".into(), tracks: vec!["a".into(), "b".into(), "a".into(), "c".into()], at: 5 });
        assert_eq!(d.playlists[1].track_ids(), vec!["a", "b", "c"], "no doubles");
        apply(&mut d, &Edit::Move { id: "p1".into(), from: 2, to: 0 });
        assert_eq!(d.playlists[1].track_ids(), vec!["c", "a", "b"]);
        // the list changed meanwhile: index 0 isn't "b" any more, so the "b" goes wherever it is
        apply(&mut d, &Edit::RemoveAt { id: "p1".into(), index: 0, track: "b".into() });
        assert_eq!(d.playlists[1].track_ids(), vec!["c", "a"]);
        apply(&mut d, &Edit::UpdatePlaylist { id: "p1".into(), name: Some("  ".into()), description: Some(" hi ".into()), public: Some(true) });
        assert_eq!(d.playlists[1].name, "Road", "an empty name keeps the old one");
        assert_eq!(d.playlists[1].description, "hi");
        apply(&mut d, &Edit::DeletePlaylist(LIKED_ID.into()));
        apply(&mut d, &Edit::DeletePlaylist("p1".into()));
        assert_eq!(d.playlists.len(), 1, "Liked Songs can't be deleted");
        apply(&mut d, &Edit::Like { track: "t1".into(), on: false, at: 2 });
        assert!(!is_liked(&d, "t1"));
        d.rest.insert("settings".into(), json!({ "gapless": true }));
        apply(&mut d, &Edit::Setting("leveling".into(), json!("album")));
        assert_eq!(setting(&d, "leveling"), Some(json!("album")));
        assert_eq!(setting(&d, "gapless"), Some(json!(true)), "other settings kept");
        assert!(new_playlist_id().starts_with("pl_"));
    }

    #[test]
    fn stats_record() {
        let mut d = UserData::default();
        apply(&mut d, &Edit::Play { track: "t1".into(), at: 1_780_000_000 });
        apply(&mut d, &Edit::Play { track: "t1".into(), at: 1_780_000_100 });
        apply(&mut d, &Edit::Listened { at: 1_780_000_100, secs: 90 });
        let s = stats(&d);
        assert_eq!(s.plays["t1"], (2, 1_780_000_100));
        assert_eq!(s.log.len(), 2);
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["stats"]["years"]["2026"]["plays"]["t1"], 2);
        assert_eq!(v["stats"]["years"]["2026"]["sec"], 90.0);
    }

    #[test]
    fn play_counter() {
        let mut c = PlayCounter::default();
        assert!(c.tick("t1", 200, 29.0).is_empty());
        assert!(matches!(c.tick("t1", 200, 1.5)[0], Edit::Play { .. }));
        assert!(c.tick("t1", 200, 10.0).iter().all(|e| !matches!(e, Edit::Play { .. })), "counted once");
        // a short song counts at half its length
        let mut c = PlayCounter::default();
        assert!(matches!(c.tick("t2", 20, 10.0)[0], Edit::Play { .. }));
    }
}
