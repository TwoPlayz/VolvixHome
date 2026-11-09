//! Music library: scans a folder of audio files (the folder Lidarr imports into) and builds the catalog
//! the app shows: artists, albums and tracks, with cover art and audio quality.
//!
//! Tags come first; for untagged files the Lidarr-style layout `Artist/Album/NN - Title.ext` fills the gaps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey, Tag};
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

/// One line of credits: a role and who did it ("Producer" -> "Jane Doe, Sam Lee").
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Credit {
    pub role: String,
    pub names: String,
}

/// Tag fields that name people, in the order they're shown.
const CREDIT_ROLES: &[(ItemKey, &str)] = &[
    (ItemKey::Performer, "Performed by"),
    (ItemKey::Writer, "Written by"),
    (ItemKey::Composer, "Composed by"),
    (ItemKey::Lyricist, "Lyrics by"),
    (ItemKey::Arranger, "Arranged by"),
    (ItemKey::Producer, "Produced by"),
    (ItemKey::Engineer, "Engineered by"),
    (ItemKey::MixEngineer, "Mixed by"),
    (ItemKey::Remixer, "Remixed by"),
    (ItemKey::Conductor, "Conducted by"),
    (ItemKey::Publisher, "Publisher"),
];

/// All values of a tag field, trimmed and de-duplicated, as "A, B" (None when empty).
fn joined(tag: &Tag, key: &ItemKey) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for v in tag.get_strings(key) {
        let v = v.trim();
        if !v.is_empty() && !seen.iter().any(|s| s.eq_ignore_ascii_case(v)) {
            seen.push(v.to_string());
        }
    }
    if seen.is_empty() { None } else { Some(seen.join(", ")) }
}

/// Credits written in the file's tags (who wrote, produced, mixed…), plus the ISRC.
pub fn credits_from_tag(tag: &Tag) -> Vec<Credit> {
    let mut out: Vec<Credit> = CREDIT_ROLES.iter()
        .filter_map(|(key, role)| joined(tag, key).map(|names| Credit { role: role.to_string(), names }))
        .collect();
    if let Some(isrc) = joined(tag, &ItemKey::Isrc) {
        out.push(Credit { role: "ISRC".into(), names: isrc });
    }
    out
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
    pub label: Option<String>,
    pub copyright: Option<String>,
    #[serde(rename = "catalogNumber")]
    pub catalog_number: Option<String>,
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
    pub credits: Vec<Credit>,
    pub explicit: bool,
    /// ReplayGain from the file's tags, in dB: how much to turn it up or down so songs sound equally loud
    #[serde(rename = "replayGain", skip_serializing_if = "Option::is_none")]
    pub replay_gain: Option<ReplayGain>,
}

#[derive(Serialize, Clone, Default, PartialEq, Debug)]
pub struct ReplayGain {
    pub track: Option<f32>,
    pub album: Option<f32>,
    /// loudest sample (1.0 = full scale): used to stop the boost from clipping
    pub peak: Option<f32>,
}

/// -6.50 dB / +3.2 / -6.5dB -> -6.5
fn parse_gain(s: &str) -> Option<f32> {
    s.trim().trim_end_matches(|c: char| c.is_alphabetic() || c.is_whitespace()).replace(',', ".").parse::<f32>().ok().filter(|v| v.is_finite() && v.abs() < 60.0)
}

pub fn replay_gain_from_tag(tag: &Tag) -> Option<ReplayGain> {
    let g = |k: &ItemKey| tag.get_string(k).and_then(parse_gain);
    let rg = ReplayGain {
        track: g(&ItemKey::ReplayGainTrackGain),
        album: g(&ItemKey::ReplayGainAlbumGain),
        peak: tag.get_string(&ItemKey::ReplayGainTrackPeak).and_then(|p| p.trim().parse::<f32>().ok()).filter(|p| *p > 0.0),
    };
    if rg.track.is_none() && rg.album.is_none() { None } else { Some(rg) }
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
    credits: Vec<Credit>,
    label: Option<String>,
    copyright: Option<String>,
    catalog: Option<String>,
    explicit: bool,
    replay_gain: Option<ReplayGain>,
}

/// Marked explicit: the iTunes advisory tag (1 = explicit, 2 = clean), or "(Explicit)" in the title.
fn is_explicit(tag: Option<&Tag>, title: &str) -> bool {
    let advisory = tag.and_then(|t| t.get_string(&ItemKey::ParentalAdvisory)).map(|v| v.trim().to_lowercase());
    match advisory.as_deref() {
        Some("1") | Some("explicit") | Some("yes") | Some("true") => return true,
        Some(_) => return false, // clean / none: the tag wins over the title
        None => {}
    }
    let t = title.to_lowercase();
    t.contains("(explicit)") || t.contains("[explicit]")
}

#[cfg(test)]
mod explicit_tests {
    use super::*;

    #[test]
    fn replay_gain_is_read_from_tags() {
        use lofty::tag::TagType;
        let mut tag = Tag::new(TagType::VorbisComments);
        assert!(replay_gain_from_tag(&tag).is_none());
        tag.insert_text(ItemKey::ReplayGainTrackGain, "-6.50 dB".into());
        tag.insert_text(ItemKey::ReplayGainAlbumGain, "+2.1dB".into());
        tag.insert_text(ItemKey::ReplayGainTrackPeak, "0.987654".into());
        let rg = replay_gain_from_tag(&tag).unwrap();
        assert_eq!(rg.track, Some(-6.5));
        assert_eq!(rg.album, Some(2.1));
        assert_eq!(rg.peak, Some(0.987654));
        assert_eq!(parse_gain("garbage"), None);
    }

    #[test]
    fn title_marker_means_explicit() {
        assert!(is_explicit(None, "Song (Explicit)"));
        assert!(is_explicit(None, "Song [explicit]"));
        assert!(!is_explicit(None, "Plain song"));
    }
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
    let credits = tag.map(credits_from_tag).unwrap_or_default();
    let label = tag.and_then(|t| joined(t, &ItemKey::Label));
    let copyright = tag.and_then(|t| joined(t, &ItemKey::CopyrightMessage));
    let catalog = tag.and_then(|t| joined(t, &ItemKey::CatalogNumber));
    let explicit = is_explicit(tag, &title);
    let replay_gain = tag.and_then(replay_gain_from_tag);

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
        credits,
        label,
        copyright,
        catalog,
        explicit,
        replay_gain,
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
                label: None,
                copyright: None,
                catalog_number: None,
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
        // label / copyright / catalog number: the first track that has them
        if album.label.is_none() { album.label = s.label.clone(); }
        if album.copyright.is_none() { album.copyright = s.copyright.clone(); }
        if album.catalog_number.is_none() { album.catalog_number = s.catalog.clone(); }
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
            credits: s.credits,
            explicit: s.explicit,
            replay_gain: s.replay_gain,
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
    fn credits_come_from_tags() {
        use lofty::tag::TagType;
        // Vorbis comments are what FLAC / Ogg files use (plain fields for every role)
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::Composer, "Jane Doe".into());
        tag.push(lofty::tag::TagItem::new(ItemKey::Composer, lofty::tag::ItemValue::Text("jane doe".into()))); // same person, other case
        tag.push(lofty::tag::TagItem::new(ItemKey::Producer, lofty::tag::ItemValue::Text("Sam Lee".into())));
        tag.push(lofty::tag::TagItem::new(ItemKey::Producer, lofty::tag::ItemValue::Text("Ana Ruiz".into())));
        tag.insert_text(ItemKey::Isrc, "USX9P2600001".into());
        tag.insert_text(ItemKey::Label, "Northbound Records".into());
        let c = credits_from_tag(&tag);
        assert_eq!(c, vec![
            Credit { role: "Composed by".into(), names: "Jane Doe".into() },
            Credit { role: "Produced by".into(), names: "Sam Lee, Ana Ruiz".into() },
            Credit { role: "ISRC".into(), names: "USX9P2600001".into() },
        ]);
        assert_eq!(joined(&tag, &ItemKey::Label).as_deref(), Some("Northbound Records"));
        assert_eq!(joined(&tag, &ItemKey::Lyricist), None);
        assert!(credits_from_tag(&Tag::new(TagType::VorbisComments)).is_empty());
    }

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
