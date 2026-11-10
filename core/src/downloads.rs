//! Offline: songs saved on this device, which albums/playlists you chose to keep, and a copy of the
//! library so the app still opens when the server can't be reached.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::model::Catalog;

#[derive(Debug, Clone)]
pub struct Downloads {
    dir: PathBuf,
}

/// Something you chose to keep offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// "album" | "playlist"
    pub kind: String,
    pub id: String,
}

fn safe(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Downloads {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Downloads { dir: dir.into() }
    }

    fn tracks_dir(&self) -> PathBuf {
        self.dir.join("tracks")
    }

    /// Where a downloaded song lives (whether or not it's there yet).
    pub fn path(&self, track_id: &str) -> Option<PathBuf> {
        safe(track_id).then(|| self.tracks_dir().join(format!("{track_id}.audio")))
    }

    pub fn has(&self, track_id: &str) -> bool {
        self.path(track_id).map(|p| p.exists()).unwrap_or(false)
    }

    /// Download a song (no-op if it's already here). `progress(bytes so far, total)`.
    pub fn download(&self, client: &Client, track_id: &str, mut progress: impl FnMut(u64, Option<u64>)) -> Result<PathBuf> {
        let path = self.path(track_id).ok_or_else(|| Error::Disk("bad song id".into()))?;
        if path.exists() {
            return Ok(path);
        }
        std::fs::create_dir_all(self.tracks_dir())?;
        let (mut reader, total, _) = client.open_stream(track_id, 0)?;
        let tmp = path.with_extension("part");
        let mut out = std::fs::File::create(&tmp)?;
        let mut buf = vec![0u8; 128 * 1024];
        let mut got = 0u64;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            got += n as u64;
            progress(got, total);
        }
        out.flush()?;
        drop(out);
        if total.map(|t| t != got).unwrap_or(false) {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Unreachable("the download stopped part way".into()));
        }
        std::fs::rename(&tmp, &path)?;
        Ok(path)
    }

    pub fn remove(&self, track_id: &str) {
        if let Some(p) = self.path(track_id) {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Ids of every song saved here.
    pub fn list(&self) -> Vec<String> {
        std::fs::read_dir(self.tracks_dir())
            .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".audio")).map(String::from)).collect())
            .unwrap_or_default()
    }

    /// Bytes used by downloaded songs.
    pub fn size(&self) -> u64 {
        std::fs::read_dir(self.tracks_dir()).map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0)
    }

    /* ---------- what you chose to keep ---------- */

    pub fn saved(&self) -> Vec<Saved> {
        std::fs::read_to_string(self.dir.join("saved.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn set_saved(&self, list: &[Saved]) -> Result<()> {
        write_atomic(&self.dir, "saved.json", &serde_json::to_vec_pretty(list).unwrap_or_default())
    }

    /// Delete songs no saved album/playlist needs any more. `needed` = every song id still wanted.
    pub fn prune(&self, needed: &std::collections::HashSet<String>) -> usize {
        let mut n = 0;
        for id in self.list() {
            if !needed.contains(&id) {
                self.remove(&id);
                n += 1;
            }
        }
        n
    }

    /* ---------- the library, for opening offline ---------- */

    pub fn save_catalog(&self, c: &Catalog) -> Result<()> {
        write_atomic(&self.dir, "library.json", &serde_json::to_vec(c).unwrap_or_default())
    }

    pub fn load_catalog(&self) -> Option<Catalog> {
        std::fs::read(self.dir.join("library.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())
    }
}

fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.part"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dir.join(name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_and_files() {
        let dir = std::env::temp_dir().join(format!("vh-dl-{}", fastrand::u64(..)));
        let d = Downloads::new(&dir);
        assert!(d.path("../x").is_none());
        assert!(d.saved().is_empty());
        d.set_saved(&[Saved { kind: "album".into(), id: "a1".into() }]).unwrap();
        assert_eq!(d.saved()[0].id, "a1");
        std::fs::create_dir_all(dir.join("tracks")).unwrap();
        std::fs::write(d.path("t1").unwrap(), b"abc").unwrap();
        std::fs::write(d.path("t2").unwrap(), b"de").unwrap();
        assert!(d.has("t1"));
        assert_eq!(d.size(), 5);
        let keep: std::collections::HashSet<String> = ["t1".to_string()].into();
        assert_eq!(d.prune(&keep), 1);
        assert_eq!(d.list(), vec!["t1"]);
        d.save_catalog(&Catalog { scanning: true, ..Default::default() }).unwrap();
        assert!(d.load_catalog().unwrap().scanning);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
