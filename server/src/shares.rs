//! Share links: a link to one album or playlist that works for anyone who has it, until it expires or you cancel it.
//! The page behind it is a tiny stand-alone player (web/share.html). The link only opens the songs that were in the
//! album or playlist when you made it: nothing else on the server, and no sign-in is created. Links are random 128-bit
//! values, last 1, 7 or 30 days, and anyone who made one can cancel it (people who manage members can cancel any).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Share {
    pub token: String,
    pub kind: String, // "album" | "playlist"
    pub name: String,
    pub by: String,   // user id
    #[serde(rename = "byName")]
    pub by_name: String,
    pub tracks: Vec<String>,
    pub created: u64,
    pub expires: u64,
}

pub struct Shares {
    file: PathBuf,
    items: Mutex<Vec<Share>>,
}

pub const DAYS: [u64; 3] = [1, 7, 30];
const MAX_TRACKS: usize = 500;
const MAX_PER_PERSON: usize = 50;

impl Shares {
    pub fn load(dir: &Path) -> Shares {
        let file = dir.join("shares.json");
        let items = crate::vault::read(&file).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Shares { file, items: Mutex::new(items) }
    }

    fn save(&self, items: &[Share]) {
        crate::vault::write(&self.file, &serde_json::to_string_pretty(items).unwrap_or_default());
    }

    pub fn create(&self, kind: &str, name: &str, by: &str, by_name: &str, tracks: Vec<String>, days: u64, now: u64) -> Result<Share, String> {
        if kind != "album" && kind != "playlist" { return Err("Share an album or a playlist.".into()); }
        if !DAYS.contains(&days) { return Err("Pick 1, 7 or 30 days.".into()); }
        if tracks.is_empty() { return Err("There's nothing in it to share.".into()); }
        let mut items = self.items.lock().unwrap();
        items.retain(|s| s.expires > now);
        if items.iter().filter(|s| s.by == by).count() >= MAX_PER_PERSON { return Err("You have a lot of links open. Cancel some first.".into()); }
        let s = Share {
            token: crate::auth::random_hex(16), kind: kind.into(), name: name.chars().take(80).collect(), by: by.into(), by_name: by_name.into(),
            tracks: tracks.into_iter().take(MAX_TRACKS).collect(), created: now, expires: now + days * 86400,
        };
        items.push(s.clone());
        self.save(&items);
        Ok(s)
    }

    /// A link that exists and hasn't expired.
    pub fn get(&self, token: &str, now: u64) -> Option<Share> {
        self.items.lock().unwrap().iter().find(|s| s.token == token && s.expires > now).cloned()
    }

    pub fn mine(&self, by: &str, all: bool, now: u64) -> Vec<Share> {
        let mut v: Vec<Share> = self.items.lock().unwrap().iter().filter(|s| s.expires > now && (all || s.by == by)).cloned().collect();
        v.sort_by(|a, b| b.created.cmp(&a.created));
        v
    }

    /// Cancel a link: its maker, or anyone allowed to (`any`).
    pub fn revoke(&self, token: &str, by: &str, any: bool) -> bool {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|s| !(s.token == token && (any || s.by == by)));
        let gone = items.len() != before;
        if gone { self.save(&items); }
        gone
    }

    pub fn purge(&self, now: u64) {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|s| s.expires > now);
        if items.len() != before { self.save(&items); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_expire_and_can_be_cancelled() {
        let dir = std::env::temp_dir().join(format!("vh-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = Shares::load(&dir);
        assert!(s.create("song", "x", "u1", "A", vec!["t".into()], 7, 100).is_err());
        assert!(s.create("album", "x", "u1", "A", vec![], 7, 100).is_err());
        assert!(s.create("album", "x", "u1", "A", vec!["t".into()], 3, 100).is_err());
        let a = s.create("album", "Tidewater", "u1", "Alex", vec!["t1".into(), "t2".into()], 7, 100).unwrap();
        assert!(s.get(&a.token, 200).is_some());
        assert!(s.get(&a.token, 100 + 7 * 86400 + 1).is_none(), "expired");
        assert!(s.get("nope", 200).is_none());
        // someone else can't cancel it, its maker can
        assert!(!s.revoke(&a.token, "u2", false));
        assert!(s.revoke(&a.token, "u1", false));
        assert!(s.get(&a.token, 200).is_none());
        // survives a restart
        let b = s.create("playlist", "Mix", "u1", "Alex", vec!["t1".into()], 1, 100).unwrap();
        assert!(Shares::load(&dir).get(&b.token, 150).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
