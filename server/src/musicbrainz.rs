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
const BURST: f64 = 3.0; // calls allowed at once
const PACE: f64 = 1.1;  // seconds per call after that

pub struct Client {
    http: reqwest::Client,
    base: String, // VH_MUSICBRAINZ_URL for a mirror (or tests); default musicbrainz.org
    gate: AsyncMutex<(f64, Instant)>, // a small token bucket: a short burst is fine, then about one call a second
    cache: Mutex<HashMap<String, (Instant, Value)>>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                // MusicBrainz asks for an app name, version and a contact URL
                .user_agent(concat!("VolvixHome/", env!("CARGO_PKG_VERSION"), " ( https://github.com/TwoPlayz/VolvixHome )"))
                .build()
                .expect("http client"),
            base: std::env::var("VH_MUSICBRAINZ_URL").ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty()).unwrap_or_else(|| DEFAULT_BASE.to_string()),
            gate: AsyncMutex::new((BURST, Instant::now())),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn get(&self, path_and_query: &str) -> Result<Value, String> {
        if let Some(v) = self.cached(path_and_query) {
            return Ok(v);
        }
        // MusicBrainz allows about one call a second on average. A search needs three calls (songs, albums, artists), so
        // a small burst is allowed and then the pace is held: the first results appear at once instead of after 3 seconds.
        loop {
            let wait = {
                let mut g = self.gate.lock().await;
                let now = Instant::now();
                g.0 = (g.0 + now.duration_since(g.1).as_secs_f64() / PACE).min(BURST);
                g.1 = now;
                if g.0 >= 1.0 { g.0 -= 1.0; break; }
                Duration::from_secs_f64((1.0 - g.0) * PACE)
            };
            tokio::time::sleep(wait).await;
        }
        let url = format!("{}/{path_and_query}", self.base);
        let mut res = self.http.get(&url).header("Accept", "application/json").send().await
            .map_err(|_| "Couldn't reach MusicBrainz.".to_string())?;
        // busy or asked too fast (typing quickly sends several searches): wait a moment and ask once more
        if matches!(res.status().as_u16(), 503 | 429) {
            tokio::time::sleep(Duration::from_millis(1600)).await;
            res = self.http.get(&url).header("Accept", "application/json").send().await
                .map_err(|_| "Couldn't reach MusicBrainz.".to_string())?;
        }
        if matches!(res.status().as_u16(), 503 | 429) {
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
        cache.get(key).filter(|(at, _)| at.elapsed() < Duration::from_secs(1800)).map(|(_, v)| v.clone())
    }
    fn store(&self, key: &str, v: Value) {
        let mut cache = self.cache.lock().unwrap();
        if cache.len() > 1000 {
            cache.clear();
        }
        cache.insert(key.to_string(), (Instant::now(), v));
    }

    /// Just one kind of result ("songs", "albums" or "artists"), so the page can show each as soon as it arrives.
    pub async fn search_part(&self, term: &str, kind: &str) -> Result<Vec<Value>, String> {
        let q = encode(term);
        match kind {
            "songs" => self.get(&format!("recording?query={q}&fmt=json&limit=25")).await.map(|v| parse_songs(&v)),
            "albums" => self.get(&format!("release-group?query={q}%20AND%20(primarytype:album%20OR%20primarytype:ep)&fmt=json&limit=20")).await.map(|v| parse_albums(&v)),
            "artists" => self.get(&format!("artist?query={q}&fmt=json&limit=10")).await.map(|v| parse_artists(&v)),
            _ => Err("Unknown kind.".into()),
        }
    }

    /// Songs, albums and artists for a search term.
    pub async fn search(&self, term: &str) -> Result<SearchResults, String> {
        let q = encode(term);
        let (rec_path, rg_path, ar_path) = (
            format!("recording?query={q}&fmt=json&limit=30"),
            format!("release-group?query={q}%20AND%20(primarytype:album%20OR%20primarytype:ep)&fmt=json&limit=25"),
            format!("artist?query={q}&fmt=json&limit=12"),
        );
        let (recordings, groups, artists) = tokio::join!(self.get(&rec_path), self.get(&rg_path), self.get(&ar_path));
        Ok(SearchResults {
            songs: recordings.map(|v| parse_songs(&v)).unwrap_or_default(),
            albums: groups.map(|v| parse_albums(&v)).unwrap_or_default(),
            artists: artists.map(|v| parse_artists(&v)).unwrap_or_default(),
        })
    }
}

impl Client {
    /// An artist and everything they've released (albums, EPs and singles), newest first.
    pub async fn artist_page(&self, mbid: &str) -> Result<Value, String> {
        let a = self.get(&format!("artist/{mbid}?fmt=json")).await?;
        let g = self.get(&format!("release-group?artist={mbid}&type=album%7Cep%7Csingle&inc=artist-credits&fmt=json&limit=100")).await?;
        let mut albums = parse_albums(&g);
        albums.sort_by(|x, y| y["year"].as_str().unwrap_or("0").cmp(x["year"].as_str().unwrap_or("0")));
        Ok(json!({ "foreignId": mbid, "name": a["name"], "type": a["type"], "disambiguation": a["disambiguation"], "country": a["country"], "image": artist_image(mbid), "albums": albums }))
    }

    /// One release-group: its details and the songs on it (from its earliest official release).
    pub async fn album_page(&self, rgid: &str) -> Result<Value, String> {
        let rg = self.get(&format!("release-group/{rgid}?inc=artist-credits&fmt=json")).await?;
        let rels = self.get(&format!("release?release-group={rgid}&status=official&inc=recordings&fmt=json&limit=25")).await?;
        let mut best: Option<&Value> = None;
        let list = rels["releases"].as_array().cloned().unwrap_or_default();
        for r in &list {
            let has = r["media"].as_array().map(|m| m.iter().any(|d| d["tracks"].as_array().map(|t| !t.is_empty()).unwrap_or(false))).unwrap_or(false);
            if !has { continue; }
            let earlier = best.map(|b| r["date"].as_str().filter(|d| !d.is_empty()).unwrap_or("9999") < b["date"].as_str().filter(|d| !d.is_empty()).unwrap_or("9999")).unwrap_or(true);
            if earlier { best = Some(r); }
        }
        let mut tracks = Vec::new();
        if let Some(r) = best {
            for (di, d) in r["media"].as_array().cloned().unwrap_or_default().iter().enumerate() {
                for t in d["tracks"].as_array().cloned().unwrap_or_default().iter() {
                    tracks.push(json!({ "disc": d["position"].as_u64().unwrap_or(di as u64 + 1), "no": t["position"], "title": t["title"], "lengthMs": t["length"] }));
                }
            }
        }
        Ok(json!({
            "foreignId": rgid, "title": rg["title"], "artist": artist_credit(&rg), "artistId": artist_id(&rg),
            "year": year(rg["first-release-date"].as_str()), "type": rg["primary-type"], "secondary": rg["secondary-types"],
            "image": cover(rgid), "tracks": tracks
        }))
    }

    /// Who made a song, from MusicBrainz: find the recording, then read its relationships
    /// (producer, engineer, mixer, performers, and via its work: composer, lyricist, writer).
    /// Returns (role, names) pairs, or an empty list when MusicBrainz doesn't know the song.
    pub async fn credits(&self, title: &str, artist: &str) -> Result<Vec<(String, String)>, String> {
        let q = format!("recording:%22{}%22%20AND%20artist:%22{}%22", encode(title), encode(artist));
        let found = self.get(&format!("recording?query={q}&fmt=json&limit=5")).await?;
        let want = norm(title);
        let best = found["recordings"].as_array().cloned().unwrap_or_default().into_iter()
            .filter(|r| r["score"].as_u64().unwrap_or(0) >= 85 && norm(r["title"].as_str().unwrap_or("")) == want)
            .next();
        let Some(id) = best.as_ref().and_then(|r| r["id"].as_str()) else { return Ok(vec![]) };
        let rec = self.get(&format!("recording/{id}?inc=artist-rels+work-rels+work-level-rels&fmt=json")).await?;
        Ok(parse_credits(&rec))
    }
}

/// Where MusicBrainz says an artist has a Wikipedia page / Wikidata item.
#[derive(Debug, Default, PartialEq)]
pub struct ArtistLinks {
    pub wikipedia: Option<String>, // English article title, e.g. "Harbor_Lights_(band)"
    pub wikidata: Option<String>,  // e.g. "Q123"
}

pub fn parse_artist_links(v: &Value) -> ArtistLinks {
    let mut out = ArtistLinks::default();
    for r in v["relations"].as_array().cloned().unwrap_or_default() {
        let url = r["url"]["resource"].as_str().unwrap_or("");
        match r["type"].as_str() {
            Some("wikipedia") => {
                let (host, rest) = url.split_once("://").and_then(|(_, r)| r.split_once('/')).unwrap_or(("", ""));
                if host.starts_with("en.") { if let Some(t) = rest.strip_prefix("wiki/") { out.wikipedia = Some(t.split('#').next().unwrap_or(t).to_string()); } }
            }
            Some("wikidata") => { out.wikidata = url.rsplit('/').next().filter(|q| q.starts_with('Q')).map(String::from); }
            _ => {}
        }
    }
    out
}

impl Client {
    /// The Wikipedia / Wikidata pages MusicBrainz lists for an artist (matched by exact name).
    pub async fn artist_links(&self, name: &str) -> Result<Option<ArtistLinks>, String> {
        let found = self.get(&format!("artist?query=artist:%22{}%22&fmt=json&limit=5", encode(name))).await?;
        let want = norm(name);
        let id = found["artists"].as_array().cloned().unwrap_or_default().into_iter()
            .find(|a| norm(a["name"].as_str().unwrap_or("")) == want)
            .and_then(|a| a["id"].as_str().map(String::from));
        let Some(id) = id else { return Ok(None) };
        let rec = self.get(&format!("artist/{id}?inc=url-rels&fmt=json")).await?;
        Ok(Some(parse_artist_links(&rec)))
    }

    /// The Wikipedia/Wikidata links of the artist with this MusicBrainz id (no name guessing).
    pub async fn artist_links_by_id(&self, id: &str) -> Result<ArtistLinks, String> {
        let rec = self.get(&format!("artist/{id}?inc=url-rels&fmt=json")).await?;
        Ok(parse_artist_links(&rec))
    }
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}

/// Turn a recording's relationships into credits, grouped by role in the order they first appear.
fn parse_credits(rec: &Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |role: String, name: &str| {
        if name.is_empty() { return; }
        match out.iter_mut().find(|(r, _)| *r == role) {
            Some((_, names)) => { if !names.iter().any(|n| n.eq_ignore_ascii_case(name)) { names.push(name.to_string()); } }
            None => out.push((role, vec![name.to_string()])),
        }
    };
    let attrs = |rel: &Value| -> Vec<String> {
        rel["attributes"].as_array().cloned().unwrap_or_default().iter().filter_map(|a| a.as_str().map(String::from)).collect()
    };
    for rel in rec["relations"].as_array().cloned().unwrap_or_default() {
        let ty = rel["type"].as_str().unwrap_or("");
        if let Some(name) = rel["artist"]["name"].as_str() {
            let a = attrs(&rel);
            let role = match ty {
                "producer" => "Produced by".to_string(),
                "engineer" => if a.iter().any(|x| x == "mix") { "Mixed by".into() } else if a.iter().any(|x| x == "mastering") { "Mastered by".into() }
                    else if a.iter().any(|x| x == "recording") { "Recorded by".into() } else { "Engineered by".into() },
                "mix" => "Mixed by".into(),
                "recording" => "Recorded by".into(),
                "mastering" => "Mastered by".into(),
                "arranger" => "Arranged by".into(),
                "conductor" => "Conducted by".into(),
                "remixer" => "Remixed by".into(),
                "performer" => "Performed by".into(),
                "vocal" => a.first().map(|x| capitalize(x)).unwrap_or_else(|| "Vocals".into()),
                "instrument" => a.first().map(|x| capitalize(x)).unwrap_or_else(|| "Instruments".into()),
                _ => continue,
            };
            add(role, name);
        }
        // the song's written side lives on its work
        if ty == "performance" {
            for wr in rel["work"]["relations"].as_array().cloned().unwrap_or_default() {
                let (Some(name), Some(wty)) = (wr["artist"]["name"].as_str(), wr["type"].as_str()) else { continue };
                let role = match wty {
                    "composer" => "Composed by", "lyricist" => "Lyrics by", "writer" => "Written by", "arranger" => "Arranged by", _ => continue,
                };
                add(role.to_string(), name);
            }
        }
    }
    out.into_iter().map(|(role, names)| (role, names.join(", "))).collect()
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

fn artist_id(v: &Value) -> Option<String> {
    v["artist-credit"].as_array().and_then(|cs| cs.first()).and_then(|c| c["artist"]["id"].as_str()).map(String::from)
}

fn year(date: Option<&str>) -> Option<String> {
    date.and_then(|d| d.get(0..4)).filter(|y| y.len() == 4).map(String::from)
}

/// Where the app asks for an album's picture: our own server, which fetches it once from the Cover Art Archive and keeps it.
fn cover(rg_id: &str) -> String {
    format!("/api/discover/image/{rg_id}")
}

/// Where the app asks for an artist's photo (Wikipedia, found through MusicBrainz), fetched once and kept by our server.
fn artist_image(mbid: &str) -> String {
    format!("/api/discover/artist-image/{mbid}")
}

/// Albums (release-groups of type album/ep).
fn parse_albums(v: &Value) -> Vec<Value> {
    v["release-groups"].as_array().cloned().unwrap_or_default().iter()
        .filter(|g| g["id"].as_str().is_some())
        .map(|g| {
            let id = g["id"].as_str().unwrap_or("");
            json!({
                "foreignId": id, "title": g["title"], "artist": artist_credit(g), "artistId": artist_id(g),
                "year": year(g["first-release-date"].as_str()),
                "type": g["primary-type"], "secondary": g["secondary-types"], "image": cover(id)
            })
        })
        .collect()
}

/// Artists.
fn parse_artists(v: &Value) -> Vec<Value> {
    v["artists"].as_array().cloned().unwrap_or_default().iter()
        .filter(|a| a["id"].as_str().is_some())
        .map(|a| json!({
            "foreignId": a["id"], "name": a["name"], "image": artist_image(a["id"].as_str().unwrap_or("")),
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
    fn credits_from_relationships() {
        let rec = json!({ "relations": [
            { "type": "producer", "artist": { "name": "Sam Lee" } },
            { "type": "engineer", "attributes": ["mix"], "artist": { "name": "Ana Ruiz" } },
            { "type": "engineer", "attributes": ["recording"], "artist": { "name": "Bo Chen" } },
            { "type": "vocal", "attributes": ["lead vocals"], "artist": { "name": "Odessa Rowe" } },
            { "type": "instrument", "attributes": ["guitar"], "artist": { "name": "Kit Moss" } },
            { "type": "instrument", "attributes": ["guitar"], "artist": { "name": "kit moss" } },
            { "type": "performance", "work": { "relations": [
                { "type": "composer", "artist": { "name": "Odessa Rowe" } },
                { "type": "lyricist", "artist": { "name": "Jane Doe" } },
                { "type": "lyricist", "artist": { "name": "Odessa Rowe" } },
                { "type": "publisher", "artist": { "name": "Ignored" } }
            ] } },
            { "type": "video", "artist": { "name": "Not a credit" } }
        ] });
        let c = parse_credits(&rec);
        let roles: Vec<&str> = c.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(roles, vec!["Produced by", "Mixed by", "Recorded by", "Lead vocals", "Guitar", "Composed by", "Lyrics by"]);
        assert_eq!(c[4].1, "Kit Moss", "same person in another case isn't repeated");
        assert_eq!(c[6].1, "Jane Doe, Odessa Rowe");
        assert!(parse_credits(&json!({})).is_empty());
    }

    #[test]
    fn artist_links_are_read_from_relations() {
        let v = json!({ "relations": [
            { "type": "official homepage", "url": { "resource": "https://example.com" } },
            { "type": "wikipedia", "url": { "resource": "https://en.wikipedia.org/wiki/Harbor_Lights_(band)#History" } },
            { "type": "wikidata", "url": { "resource": "https://www.wikidata.org/wiki/Q4242" } }
        ] });
        let l = parse_artist_links(&v);
        assert_eq!(l.wikipedia.as_deref(), Some("Harbor_Lights_(band)"));
        assert_eq!(l.wikidata.as_deref(), Some("Q4242"));
        assert_eq!(parse_artist_links(&json!({ "relations": [{ "type": "wikipedia", "url": { "resource": "https://de.wikipedia.org/wiki/X" } }] })), ArtistLinks::default());
    }

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
