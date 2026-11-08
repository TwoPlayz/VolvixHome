//! MusicBrainz: search for songs, albums and artists.
//!
//! MusicBrainz is the open music database Lidarr itself uses, so the ids we get here (release-group and
//! artist MBIDs) are exactly what a request hands to Lidarr. We only read from it, and follow its rules:
//! a descriptive User-Agent, and no more than one request a second (serialized + spaced here, plus a short
//! cache so repeat searches don't call out again). Cover art comes from the Cover Art Archive.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;

const DEFAULT_BASE: &str = "https://musicbrainz.org/ws/2";
pub const COVER_ART: &str = "https://coverartarchive.org";

pub struct Client {
    http: reqwest::Client,
    base: String, // VH_MUSICBRAINZ_URL for a mirror (or tests); default musicbrainz.org
    gate: AsyncMutex<Instant>, // enforces >= 1s between calls
    cache: Mutex<HashMap<String, (Instant, Value)>>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                // MusicBrainz asks for an app name, version and a contact URL
                .user_agent(concat!("VolvixHome/", env!("CARGO_PKG_VERSION"), " ( https://github.com/twoplayz/volvix-home )"))
                .build()
                .expect("http client"),
            base: std::env::var("VH_MUSICBRAINZ_URL").ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty()).unwrap_or_else(|| DEFAULT_BASE.to_string()),
            gate: AsyncMutex::new(Instant::now() - Duration::from_secs(2)),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn get(&self, path_and_query: &str) -> Result<Value, String> {
        if let Some(v) = self.cached(path_and_query) {
            return Ok(v);
        }
        // one call at a time, at least 1.1s apart (MusicBrainz rate limit)
        {
            let mut last = self.gate.lock().await;
            let wait = Duration::from_millis(1100).checked_sub(last.elapsed());
            if let Some(w) = wait {
                tokio::time::sleep(w).await;
            }
            *last = Instant::now();
        }
        let url = format!("{}/{path_and_query}", self.base);
        let res = self.http.get(&url).header("Accept", "application/json").send().await
            .map_err(|_| "Couldn't reach MusicBrainz.".to_string())?;
        if res.status().as_u16() == 503 {
            return Err("MusicBrainz is busy. Try again in a moment.".into());
        }
        if !res.status().is_success() {
            return Err(format!("MusicBrainz said {}.", res.status()));
        }
        let v: Value = res.json().await.map_err(|_| "MusicBrainz sent something unexpected.".to_string())?;
        self.store(path_and_query, v.clone());
        Ok(v)
    }

    fn cached(&self, key: &str) -> Option<Value> {
        let cache = self.cache.lock().unwrap();
        cache.get(key).filter(|(at, _)| at.elapsed() < Duration::from_secs(600)).map(|(_, v)| v.clone())
    }
    fn store(&self, key: &str, v: Value) {
        let mut cache = self.cache.lock().unwrap();
        if cache.len() > 200 {
            cache.clear();
        }
        cache.insert(key.to_string(), (Instant::now(), v));
    }

    /// Songs, albums and artists for a search term.
    pub async fn search(&self, term: &str) -> Result<SearchResults, String> {
        let q = encode(term);
        let (rec_path, rg_path, ar_path) = (
            format!("recording?query={q}&fmt=json&limit=12"),
            format!("release-group?query={q}%20AND%20(primarytype:album%20OR%20primarytype:ep)&fmt=json&limit=14"),
            format!("artist?query={q}&fmt=json&limit=6"),
        );
        let (recordings, groups, artists) = tokio::join!(self.get(&rec_path), self.get(&rg_path), self.get(&ar_path));
        Ok(SearchResults {
            songs: recordings.map(|v| parse_songs(&v)).unwrap_or_default(),
            albums: groups.map(|v| parse_albums(&v)).unwrap_or_default(),
            artists: artists.map(|v| parse_artists(&v)).unwrap_or_default(),
        })
    }
}

pub struct SearchResults {
    pub songs: Vec<Value>,
    pub albums: Vec<Value>,
    pub artists: Vec<Value>,
}

fn encode(term: &str) -> String {
    // keep letters/digits/spaces; spaces -> %20; drop characters special to the query parser
    term.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().collect::<Vec<_>>()
            } else if c == ' ' {
                vec![' ']
            } else {
                vec![]
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("%20")
}

fn artist_credit(v: &Value) -> String {
    v["artist-credit"].as_array()
        .map(|cs| cs.iter().map(|c| {
            let name = c["name"].as_str().or_else(|| c["artist"]["name"].as_str()).unwrap_or("");
            let join = c["joinphrase"].as_str().unwrap_or("");
            format!("{name}{join}")
        }).collect::<String>())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn year(date: Option<&str>) -> Option<String> {
    date.and_then(|d| d.get(0..4)).filter(|y| y.len() == 4).map(String::from)
}

fn cover(rg_id: &str) -> String {
    format!("{COVER_ART}/release-group/{rg_id}/front-250")
}

/// Albums (release-groups of type album/ep).
fn parse_albums(v: &Value) -> Vec<Value> {
    v["release-groups"].as_array().cloned().unwrap_or_default().iter()
        .filter(|g| g["id"].as_str().is_some())
        .map(|g| {
            let id = g["id"].as_str().unwrap_or("");
            json!({
                "foreignId": id, "title": g["title"], "artist": artist_credit(g),
                "year": year(g["first-release-date"].as_str()),
                "type": g["primary-type"], "image": cover(id)
            })
        })
        .collect()
}

/// Artists.
fn parse_artists(v: &Value) -> Vec<Value> {
    v["artists"].as_array().cloned().unwrap_or_default().iter()
        .filter(|a| a["id"].as_str().is_some())
        .map(|a| json!({
            "foreignId": a["id"], "name": a["name"],
            "type": a["type"], "disambiguation": a["disambiguation"],
            "country": a["country"]
        }))
        .collect()
}

/// Songs (recordings). For each, find a release-group to get it from: a single if one exists, else an album.
fn parse_songs(v: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in v["recordings"].as_array().cloned().unwrap_or_default().iter() {
        let title = r["title"].as_str().unwrap_or("");
        let artist = artist_credit(r);
        // de-dupe the many releases of the same song
        let key = format!("{}|{}", title.to_lowercase(), artist.to_lowercase());
        if title.is_empty() || !seen.insert(key) {
            continue;
        }
        // scan the releases this recording appears on for album- and single-type release-groups
        let mut album: Option<(String, Option<String>)> = None; // (rgId, date)
        let mut single: Option<(String, Option<String>)> = None;
        for rel in r["releases"].as_array().cloned().unwrap_or_default().iter() {
            let rg = &rel["release-group"];
            let (Some(rg_id), Some(ptype)) = (rg["id"].as_str(), rg["primary-type"].as_str()) else { continue };
            let date = rg["first-release-date"].as_str().or_else(|| rel["date"].as_str()).map(String::from);
            let slot = match ptype {
                "Single" => &mut single,
                "Album" | "EP" => &mut album,
                _ => continue,
            };
            // keep the earliest-dated release-group in each slot
            let better = match slot {
                None => true,
                Some((_, existing)) => date.as_deref().unwrap_or("9999") < existing.as_deref().unwrap_or("9999"),
            };
            if better {
                *slot = Some((rg_id.to_string(), date));
            }
        }
        if album.is_none() && single.is_none() {
            continue; // nothing Lidarr could fetch
        }
        let pick = album.clone().or_else(|| single.clone());
        let year = pick.as_ref().and_then(|(_, d)| year(d.as_deref()));
        let cover_rg = pick.as_ref().map(|(id, _)| id.clone()).unwrap_or_default();
        out.push(json!({
            "title": title, "artist": artist, "year": year,
            "length": r["length"].as_u64().map(|ms| ms / 1000),
            "albumId": album.map(|(id, _)| id),
            "singleId": single.map(|(id, _)| id),
            "image": cover(&cover_rg)
        }));
        if out.len() >= 6 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_queries() {
        assert_eq!(encode("Low Tide"), "low%20tide");
        assert_eq!(encode("AC/DC: Back!"), "acdc%20back");
        assert_eq!(encode("  spaced  out "), "spaced%20out");
    }

    #[test]
    fn builds_artist_credit() {
        let v = json!({ "artist-credit": [
            { "name": "Odessa Rowe", "joinphrase": " feat. " },
            { "name": "Nia", "joinphrase": "" }
        ] });
        assert_eq!(artist_credit(&v), "Odessa Rowe feat. Nia");
    }

    #[test]
    fn song_prefers_single_and_album() {
        let v = json!({ "recordings": [{
            "title": "North Star", "length": 211000,
            "artist-credit": [{ "name": "Odessa Rowe" }],
            "releases": [
                { "release-group": { "id": "rg-album", "primary-type": "Album", "first-release-date": "2026-02-06" } },
                { "release-group": { "id": "rg-single", "primary-type": "Single", "first-release-date": "2025-11-01" } }
            ]
        }]});
        let songs = parse_songs(&v);
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0]["albumId"], "rg-album");
        assert_eq!(songs[0]["singleId"], "rg-single");
        assert_eq!(songs[0]["length"], 211);
        assert_eq!(songs[0]["year"], "2026"); // album is the default cover/year
    }

    #[test]
    fn song_with_no_release_group_is_skipped() {
        let v = json!({ "recordings": [{ "title": "Demo", "artist-credit": [{ "name": "X" }], "releases": [] }]});
        assert!(parse_songs(&v).is_empty());
    }
}
