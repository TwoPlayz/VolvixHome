//! Lyrics for a song. Looked for in this order:
//!   1. a `.lrc` file next to the song (same name), then a `.txt`
//!   2. lyrics embedded in the file's tags (plain, or timestamped LRC text)
//!   3. LRCLIB (lrclib.net), a free community lyrics database. Set VH_LYRICS_ONLINE=0 to never call out.
//! Synced lyrics are a list of lines with the millisecond each one starts at; plain lyrics are just text.
//! Enhanced LRC (`[00:01.00]<00:01.00>Hel<00:01.40>lo`) also gives each line its words with the millisecond each starts at.

use std::path::Path;
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use serde_json::{json, Value};

const DEFAULT_LRCLIB: &str = "https://lrclib.net";

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub ms: i64,
    pub text: String,
    /// Word-by-word timing (enhanced LRC). Empty when the line is only synced as a whole.
    /// Each word's text keeps its spacing, so joining them gives the line back.
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub ms: i64,
    pub text: String,
}

fn line_json(l: &Line) -> Value {
    let mut v = json!({ "ms": l.ms, "text": l.text });
    if !l.words.is_empty() {
        v["words"] = json!(l.words.iter().map(|w| json!({ "ms": w.ms, "text": w.text })).collect::<Vec<_>>());
    }
    v
}

fn line_from_json(l: &Value) -> Option<Line> {
    let words = l["words"].as_array().map(|ws| ws.iter()
        .filter_map(|w| Some(Word { ms: w["ms"].as_i64()?, text: w["text"].as_str()?.to_string() }))
        .collect()).unwrap_or_default();
    Some(Line { ms: l["ms"].as_i64()?, text: l["text"].as_str()?.to_string(), words })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lyrics {
    pub synced: Vec<Line>,
    pub plain: String,
    pub source: String, // "file", "tag" or "lrclib"
    pub instrumental: bool,
}

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.synced.is_empty() && self.plain.trim().is_empty() && !self.instrumental
    }

    pub fn to_json(&self) -> Value {
        json!({
            "synced": self.synced.iter().map(line_json).collect::<Vec<_>>(),
            "plain": self.plain,
            "source": self.source,
            "instrumental": self.instrumental,
        })
    }

    pub fn from_json(v: &Value) -> Option<Lyrics> {
        let synced = v["synced"].as_array()?.iter().filter_map(line_from_json).collect();
        Some(Lyrics {
            synced,
            plain: v["plain"].as_str().unwrap_or("").to_string(),
            source: v["source"].as_str().unwrap_or("").to_string(),
            instrumental: v["instrumental"].as_bool().unwrap_or(false),
        })
    }
}

/// Parse LRC text: `[01:23.45] words`, several stamps on one line, `[offset:+250]`, and enhanced LRC's
/// per-word `<01:23.45>` stamps. Returns no lines when the text has no timestamps (it's plain lyrics).
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut offset = 0i64;
    let mut out: Vec<Line> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim_start_matches('\u{feff}').trim();
        let mut stamps: Vec<i64> = Vec::new();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            let tag = &rest[1..end];
            if let Some(ms) = parse_stamp(tag) {
                stamps.push(ms);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            } // other tags (ti:, ar:, al:, by:) are metadata: skipped
            rest = rest[end + 1..].trim_start();
        }
        let text = strip_word_stamps(rest);
        let first = stamps.first().copied().unwrap_or(0);
        let words = parse_words(rest, first);
        for ms in stamps {
            // a line repeated under several stamps (a chorus): its word stamps belong to the first one, so shift them
            let shift = ms - first;
            let words = words.iter().map(|w| Word { ms: w.ms + shift, text: w.text.clone() }).collect();
            out.push(Line { ms, text: text.clone(), words });
        }
    }
    // a positive offset makes lyrics appear earlier, as in the LRC spec
    for l in out.iter_mut() {
        l.ms = (l.ms - offset).max(0);
        for w in l.words.iter_mut() {
            w.ms = (w.ms - offset).max(0);
        }
    }
    out.sort_by_key(|l| l.ms);
    out
}

fn parse_stamp(tag: &str) -> Option<i64> {
    let (m, s) = tag.split_once(':')?;
    let m: i64 = m.trim().parse().ok()?;
    let s = s.trim().replace(',', ".");
    let secs: f64 = s.parse().ok()?;
    if !(0.0..60.0).contains(&secs) || m < 0 {
        return None;
    }
    Some(m * 60_000 + (secs * 1000.0).round() as i64)
}

/// The words of an enhanced-LRC line: each `<mm:ss.xx>` starts a word that runs until the next stamp. Text before the
/// first stamp starts with the line. A stamp with nothing after it just marks where the last word ends, so it's dropped.
/// Empty when the line has no word stamps.
fn parse_words(rest: &str, line_ms: i64) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut seen_stamp = false;
    let mut cur = Word { ms: line_ms, text: String::new() };
    let mut s = rest;
    while !s.is_empty() {
        if s.starts_with('<') {
            if let Some(end) = s.find('>') {
                if let Some(ms) = parse_stamp(&s[1..end]) {
                    seen_stamp = true;
                    if !cur.text.trim().is_empty() {
                        words.push(cur);
                    } else if let Some(last) = words.last_mut() {
                        last.text.push_str(&cur.text); // keep the spacing with the word before
                    }
                    cur = Word { ms, text: String::new() };
                    s = &s[end + 1..];
                    continue;
                }
            }
        }
        let c = s.chars().next().unwrap();
        cur.text.push(c);
        s = &s[c.len_utf8()..];
    }
    if !seen_stamp {
        return Vec::new();
    }
    if !cur.text.trim().is_empty() {
        words.push(cur);
    }
    // the line's text is trimmed, so trim the ends of the words to match
    if let Some(w) = words.first_mut() { w.text = w.text.trim_start().to_string(); }
    if let Some(w) = words.last_mut() { w.text = w.text.trim_end().to_string(); }
    words
}

fn strip_word_stamps(s: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for c in s.chars() {
        match c {
            '<' => skipping = true,
            '>' if skipping => skipping = false,
            _ if !skipping => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn from_text(text: &str, source: &str) -> Option<Lyrics> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let synced = parse_lrc(text);
    let l = if synced.is_empty() {
        Lyrics { plain: text.replace("\r\n", "\n"), source: source.into(), ..Default::default() }
    } else {
        let plain = synced.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        Lyrics { synced, plain, source: source.into(), instrumental: false }
    };
    Some(l)
}

/// Lyrics that sit next to the song or inside it.
pub fn from_files(song: &Path) -> Option<Lyrics> {
    for ext in ["lrc", "txt"] {
        if let Ok(text) = std::fs::read_to_string(song.with_extension(ext)) {
            if let Some(l) = from_text(&text, "file") {
                return Some(l);
            }
        }
    }
    let tagged = lofty::read_from_path(song).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let text = tag.get_string(&ItemKey::Lyrics)?;
    from_text(text, "tag")
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    online: bool,
}

impl Client {
    pub fn new() -> Client {
        let base = std::env::var("VH_LRCLIB_URL").ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty())
            .unwrap_or_else(|| DEFAULT_LRCLIB.to_string());
        let online = !matches!(std::env::var("VH_LYRICS_ONLINE").ok().as_deref().map(str::trim), Some("0") | Some("false") | Some("off"));
        Client {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("VolvixHome/", env!("CARGO_PKG_VERSION"), " (https://github.com/TwoPlayz/VolvixHome)"))
                .build()
                .expect("http client"),
            base,
            online,
        }
    }

    pub fn online(&self) -> bool {
        self.online
    }

    /// Ok(None) = LRCLIB doesn't have it. Err = couldn't ask (don't remember that as "none").
    pub async fn find(&self, title: &str, artist: &str, album: &str, duration: u32) -> Result<Option<Lyrics>, String> {
        if !self.online {
            return Ok(None);
        }
        let url = format!("{}/api/get", self.base);
        let mut q: Vec<(&str, String)> = vec![("track_name", title.to_string()), ("artist_name", artist.to_string())];
        if !album.is_empty() {
            q.push(("album_name", album.to_string()));
        }
        if duration > 0 {
            q.push(("duration", duration.to_string()));
        }
        let res = self.http.get(&url).query(&q).send().await.map_err(|_| "Couldn't reach the lyrics service.".to_string())?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(format!("The lyrics service said {}.", res.status()));
        }
        let v: Value = res.json().await.map_err(|_| "The lyrics service sent something unexpected.".to_string())?;
        Ok(from_lrclib(&v))
    }
}

pub fn from_lrclib(v: &Value) -> Option<Lyrics> {
    if v["instrumental"].as_bool() == Some(true) {
        return Some(Lyrics { instrumental: true, source: "lrclib".into(), ..Default::default() });
    }
    let synced = v["syncedLyrics"].as_str().map(parse_lrc).unwrap_or_default();
    let plain = v["plainLyrics"].as_str().map(|s| s.trim().to_string()).unwrap_or_default();
    let plain = if plain.is_empty() { synced.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n") } else { plain };
    let l = Lyrics { synced, plain, source: "lrclib".into(), instrumental: false };
    if l.is_empty() { None } else { Some(l) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stamps_and_sorts() {
        let l = parse_lrc("[ti:Song]\n[00:12.50] second\n[00:01.00]first\n[01:02,5] third");
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], Line { ms: 1000, text: "first".into(), words: vec![] });
        assert_eq!(l[1].ms, 12_500);
        assert_eq!(l[2].ms, 62_500);
    }

    #[test]
    fn repeated_stamps_and_blank_lines() {
        let l = parse_lrc("[00:10.00][00:30.00]chorus\n[00:20.00]");
        assert_eq!(l.iter().map(|x| x.ms).collect::<Vec<_>>(), vec![10_000, 20_000, 30_000]);
        assert_eq!(l[1].text, "");
        assert_eq!(l[2].text, "chorus");
    }

    #[test]
    fn offset_shifts_lines() {
        let l = parse_lrc("[offset:500]\n[00:05.00]hi");
        assert_eq!(l[0].ms, 4500);
    }

    #[test]
    fn word_stamps_give_words() {
        let l = parse_lrc("[00:01.00]<00:01.00>hel<00:01.40>lo");
        assert_eq!(l[0].text, "hello");
        assert_eq!(l[0].words, vec![Word { ms: 1000, text: "hel".into() }, Word { ms: 1400, text: "lo".into() }]);
    }

    #[test]
    fn enhanced_lrc_spacing_lead_in_and_end_stamp() {
        let l = parse_lrc("[00:05.00] I <00:05.50>saw <00:06.00>you<00:07.00>");
        assert_eq!(l[0].text, "I saw you");
        let w: Vec<(i64, &str)> = l[0].words.iter().map(|w| (w.ms, w.text.as_str())).collect();
        assert_eq!(w, vec![(5000, "I "), (5500, "saw "), (6000, "you")]);
        assert_eq!(l[0].words.iter().map(|w| w.text.as_str()).collect::<String>(), l[0].text);
    }

    #[test]
    fn repeated_lines_shift_word_stamps_and_offset_applies() {
        let l = parse_lrc("[offset:100]\n[00:10.00][00:30.00]<00:10.00>la <00:10.50>la");
        assert_eq!(l[0].words[1].ms, 10_400);
        assert_eq!(l[1].ms, 29_900);
        assert_eq!(l[1].words[0].ms, 29_900);
        assert_eq!(l[1].words[1].ms, 30_400);
    }

    #[test]
    fn plain_lrc_lines_have_no_words() {
        let l = parse_lrc("[00:01.00]just a line");
        assert!(l[0].words.is_empty());
        assert!(line_json(&l[0]).get("words").is_none());
    }

    #[test]
    fn plain_text_is_not_synced() {
        let l = from_text("just words\nmore words", "tag").unwrap();
        assert!(l.synced.is_empty());
        assert_eq!(l.plain, "just words\nmore words");
        assert!(from_text("   ", "tag").is_none());
    }

    #[test]
    fn lrclib_json() {
        let v = json!({ "plainLyrics": "a\nb", "syncedLyrics": "[00:01.00]a\n[00:02.00]b" });
        let l = from_lrclib(&v).unwrap();
        assert_eq!(l.synced.len(), 2);
        assert_eq!(from_lrclib(&json!({ "instrumental": true })).unwrap().instrumental, true);
        assert!(from_lrclib(&json!({ "plainLyrics": null, "syncedLyrics": null })).is_none());
    }

    #[test]
    fn json_roundtrip() {
        let l = from_text("[00:01.00]hi", "file").unwrap();
        assert_eq!(Lyrics::from_json(&l.to_json()).unwrap(), l);
        let l = from_text("[00:01.00]<00:01.00>hi <00:01.50>there", "file").unwrap();
        assert_eq!(Lyrics::from_json(&l.to_json()).unwrap(), l);
    }

    #[test]
    fn sidecar_file_wins() {
        let dir = std::env::temp_dir().join(format!("vh-lyr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let song = dir.join("a.wav");
        std::fs::write(&song, b"not audio").unwrap();
        std::fs::write(dir.join("a.lrc"), "[00:02.00]hello").unwrap();
        let l = from_files(&song).unwrap();
        assert_eq!(l.source, "file");
        assert_eq!(l.synced[0].ms, 2000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
