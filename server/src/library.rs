//! Music library: scans a folder of audio files (the folder Lidarr imports into) and builds the catalog
//! the app shows: artists, albums and tracks, with cover art and audio quality.
//!
//! Tags come first; for untagged files the Lidarr-style layout `Artist/Album/NN - Title.ext` fills the gaps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use serde::Serialize;
use walkdir::WalkDir;

pub const AUDIO: &[&str] = &["mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "opus", "wav", "aif", "aiff", "wv", "ape"];
const LOSSLESS: &[&str] = &["flac", "wav", "aif", "aiff", "wv", "ape"];
const COVER_FILES: &[&str] = &["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "front.png", "album.jpg"];

/// Stable short id from text (FNV-1a 64), so ids survive rescans and restarts.
pub fn id(prefix: &str, text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}{h:012x}").chars().take(prefix.len() + 12).collect()
}

#[derive(Serialize, Clone)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct Quality {
    pub codec: String,
    pub lossless: bool,
    pub bits: Option<u8>,
    pub khz: Option<f64>,
    pub kbps: Option<u32>,
}

#[derive(Serialize, Clone)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub genre: Option<String>,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<String>,
    pub cover: bool,
    pub quality: Quality,
    /// newest file time, for "Recently added"
    pub added: u64,
}

#[derive(Serialize, Clone)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(rename = "albumId")]
    pub album_id: String,
    pub no: u32,
    pub disc: u32,
    pub duration: u32,
}

/// What /api/library returns.
#[derive(Serialize, Default)]
pub struct Catalog {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

/// Where the art for an album comes from.
#[derive(Clone)]
enum CoverSource {
    File(PathBuf),
    Embedded(PathBuf),
}

#[derive(Default)]
pub struct Library {
    pub catalog: Catalog,
    files: HashMap<String, PathBuf>,
    covers: HashMap<String, CoverSource>,
    pub scanning: bool,
    pub scanned_at: Option<SystemTime>,
}

impl Library {
    pub fn track_path(&self, id: &str) -> Option<&PathBuf> {
        self.files.get(id)
    }

    /// Cover bytes for an album (embedded picture or an image file in the album folder).
    pub fn cover(&self, album_id: &str) -> Option<Vec<u8>> {
        match self.covers.get(album_id)? {
            CoverSource::File(p) => std::fs::read(p).ok(),
            CoverSource::Embedded(p) => {
                let tagged = lofty::read_from_path(p).ok()?;
                let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
                tag.pictures().first().map(|pic| pic.data().to_vec())
            }
        }
    }
}

struct Scanned {
    path: PathBuf,
    title: String,
    artist: String,
    album_artist: String,
    album: String,
    no: u32,
    disc: u32,
    year: Option<u32>,
    genre: Option<String>,
    duration: u32,
    quality: Quality,
    embedded_art: bool,
    modified: u64,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// "03 - Song Name" / "03. Song Name" / "1-03 Song Name" -> (3, "Song Name")
fn from_file_name(stem: &str) -> (u32, String) {
    let s = stem.trim();
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    let no = digits.rsplit('-').next().and_then(|d| d.parse::<u32>().ok()).unwrap_or(0);
    let rest = s[digits.len()..].trim_start_matches(|c: char| c == ' ' || c == '-' || c == '.' || c == '_').trim();
    (no, if rest.is_empty() { s.to_string() } else { rest.to_string() })
}

fn read_file(path: &Path, root: &Path) -> Option<Scanned> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel.iter().map(|p| p.to_string_lossy().to_string()).collect();
    // Lidarr layout: Artist/Album/Track (deeper folders like "CD 1" are fine)
    let folder_artist = if parts.len() >= 3 { Some(parts[0].clone()) } else { None };
    let folder_album = if parts.len() >= 2 { Some(parts[parts.len() - 2].clone()) } else { None };
    let (file_no, file_title) = from_file_name(&path.file_stem()?.to_string_lossy());
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let tagged = lofty::read_from_path(path).ok();
    let props = tagged.as_ref().map(|t| t.properties().clone());
    let tag = tagged.as_ref().and_then(|t| t.primary_tag().or_else(|| t.first_tag()));

    let title = clean(tag.and_then(|t| t.title().map(|v| v.to_string()))).unwrap_or(file_title);
    let artist = clean(tag.and_then(|t| t.artist().map(|v| v.to_string())))
        .or_else(|| folder_artist.clone())
        .unwrap_or_else(|| "Unknown artist".into());
    let album_artist = clean(tag.and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(String::from)))
        .or_else(|| folder_artist.clone())
        .unwrap_or_else(|| artist.clone());
    let album = clean(tag.and_then(|t| t.album().map(|v| v.to_string())))
        .or(folder_album)
        .unwrap_or_else(|| "Unknown album".into());
    let year = tag
        .and_then(|t| t.get_string(&ItemKey::Year).or_else(|| t.get_string(&ItemKey::RecordingDate)).map(String::from))
        .and_then(|d| d.get(0..4).and_then(|y| y.parse::<u32>().ok()));
    let genre = clean(tag.and_then(|t| t.genre().map(|v| v.to_string())));
    let no = tag.and_then(|t| t.track()).unwrap_or(file_no);
    let disc = tag.and_then(|t| t.disk()).unwrap_or(1);
    let embedded_art = tag.map(|t| !t.pictures().is_empty()).unwrap_or(false);

    let (duration, bits, khz, kbps) = match &props {
        Some(p) => (
            p.duration().as_secs() as u32,
            p.bit_depth(),
            p.sample_rate().map(|r| (r as f64 / 100.0).round() / 10.0),
            p.audio_bitrate(),
        ),
        None => (0, None, None, None),
    };
    // m4a can be ALAC (lossless) or AAC: ALAC reports a bit depth, AAC doesn't
    let lossless = LOSSLESS.contains(&ext.as_str()) || ((ext == "m4a" || ext == "mp4") && bits.is_some());
    let codec = match ext.as_str() {
        "m4a" | "mp4" => if lossless { "ALAC" } else { "AAC" }.to_string(),
        "oga" => "OGG".into(),
        e => e.to_uppercase(),
    };

    Some(Scanned {
        path: path.to_path_buf(),
        title,
        artist,
        album_artist,
        album,
        no,
        disc,
        year,
        genre,
        duration,
        quality: Quality { codec, lossless, bits: if lossless { bits } else { None }, khz, kbps },
        embedded_art,
        modified,
    })
}

/// Walk the music folder and build a fresh library.
pub fn scan(root: &Path) -> Library {
    let mut found: Vec<Scanned> = WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| AUDIO.contains(&x.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .filter_map(|e| read_file(e.path(), root))
        .collect();
    found.sort_by(|a, b| {
        (a.album_artist.to_lowercase(), a.album.to_lowercase(), a.disc, a.no, a.title.clone())
            .cmp(&(b.album_artist.to_lowercase(), b.album.to_lowercase(), b.disc, b.no, b.title.clone()))
    });

    let mut lib = Library::default();
    let mut albums: Vec<Album> = Vec::new();
    let mut album_index: HashMap<String, usize> = HashMap::new();
    let mut artists: HashMap<String, Artist> = HashMap::new();

    for s in found {
        let album_id = id("a", &format!("{}\u{1f}{}", s.album_artist, s.album));
        let track_id = id("t", &s.path.to_string_lossy());
        for name in [&s.album_artist, &s.artist] {
            artists.entry(name.to_lowercase()).or_insert_with(|| Artist { id: id("r", name), name: name.clone() });
        }
        let idx = *album_index.entry(album_id.clone()).or_insert_with(|| {
            albums.push(Album {
                id: album_id.clone(),
                title: s.album.clone(),
                artist: s.album_artist.clone(),
                year: s.year,
                genre: s.genre.clone(),
                track_ids: Vec::new(),
                cover: false,
                quality: s.quality.clone(),
                added: 0,
            });
            albums.len() - 1
        });
        let album = &mut albums[idx];
        album.track_ids.push(track_id.clone());
        album.added = album.added.max(s.modified);
        if album.year.is_none() {
            album.year = s.year;
        }
        // album quality = the best file on it
        let better = |q: &Quality, than: &Quality| {
            (q.lossless, q.bits.unwrap_or(0), q.khz.unwrap_or(0.0) as u32, q.kbps.unwrap_or(0))
                > (than.lossless, than.bits.unwrap_or(0), than.khz.unwrap_or(0.0) as u32, than.kbps.unwrap_or(0))
        };
        if better(&s.quality, &album.quality) {
            album.quality = s.quality.clone();
        }
        // cover: an image in the folder beats embedded art
        if !lib.covers.contains_key(&album_id) {
            if let Some(dir) = s.path.parent() {
                if let Some(f) = COVER_FILES.iter().map(|n| dir.join(n)).find(|p| p.exists()) {
                    lib.covers.insert(album_id.clone(), CoverSource::File(f));
                }
            }
        }
        if !lib.covers.contains_key(&album_id) && s.embedded_art {
            lib.covers.insert(album_id.clone(), CoverSource::Embedded(s.path.clone()));
        }
        album.cover = lib.covers.contains_key(&album_id);

        lib.files.insert(track_id.clone(), s.path.clone());
        lib.catalog.tracks.push(Track {
            id: track_id,
            title: s.title,
            artist: s.artist,
            album_id,
            no: s.no,
            disc: s.disc,
            duration: s.duration,
        });
    }

    let mut artists: Vec<Artist> = artists.into_values().collect();
    artists.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    lib.catalog.artists = artists;
    lib.catalog.albums = albums;
    lib.scanned_at = Some(SystemTime::now());
    lib
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        assert_eq!(from_file_name("03 - Low Tide"), (3, "Low Tide".into()));
        assert_eq!(from_file_name("1-07 Fog Signal"), (7, "Fog Signal".into()));
        assert_eq!(from_file_name("12. Tidewater"), (12, "Tidewater".into()));
        assert_eq!(from_file_name("Untitled"), (0, "Untitled".into()));
    }

    #[test]
    fn ids_are_stable_and_case_insensitive() {
        assert_eq!(id("a", "Harbor Lights\u{1f}Tidewater"), id("a", "harbor lights\u{1f}tidewater"));
        assert_ne!(id("a", "x"), id("a", "y"));
        assert_eq!(id("t", "abc").len(), 13);
    }
}
