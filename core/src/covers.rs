//! Covers on disk. A cover with a version (`?v=`) never changes, so once it's here it's never
//! fetched again; a new picture gets a new version and so a new file.
use std::path::{Path, PathBuf};

use crate::client::Client;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct CoverCache {
    dir: PathBuf,
}

impl CoverCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        CoverCache { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where this exact picture lives on disk. Unversioned covers aren't kept (they could change).
    pub fn path_for(&self, album_id: &str, size: u32, version: u64) -> Option<PathBuf> {
        if version == 0 || !album_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return None;
        }
        Some(self.dir.join(format!("{album_id}-{size}-{version}.img")))
    }

    /// Already on disk? (no network)
    pub fn cached(&self, album_id: &str, size: u32, version: u64) -> Option<Vec<u8>> {
        std::fs::read(self.path_for(album_id, size, version)?).ok()
    }

    /// The cover's bytes: from disk if we have them, otherwise from the server (then kept).
    pub fn get(&self, client: &Client, album_id: &str, size: u32, version: u64) -> Result<Vec<u8>> {
        if let Some(b) = self.cached(album_id, size, version) {
            return Ok(b);
        }
        let bytes = client.cover_bytes(album_id, size, version)?;
        if let Some(path) = self.path_for(album_id, size, version) {
            let _ = std::fs::create_dir_all(&self.dir);
            // write then rename, so a half-written file never looks like a cover
            let tmp = path.with_extension("part");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
            self.forget_older(album_id, size, version);
        }
        Ok(bytes)
    }

    /// Older versions of this cover at this size are no use any more.
    fn forget_older(&self, album_id: &str, size: u32, keep: u64) {
        let prefix = format!("{album_id}-{size}-");
        let keep_name = format!("{album_id}-{size}-{keep}.img");
        if let Ok(list) = std::fs::read_dir(&self.dir) {
            for e in list.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name != keep_name {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let c = CoverCache::new("x");
        assert!(c.path_for("a1", 640, 0).is_none(), "unversioned covers aren't kept");
        assert!(c.path_for("../etc", 640, 3).is_none());
        assert_eq!(c.path_for("a1", 640, 3).unwrap(), PathBuf::from("x").join("a1-640-3.img"));
    }

    #[test]
    fn older_versions_are_removed() {
        let dir = std::env::temp_dir().join(format!("vh-covers-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a1-640-1.img"), b"old").unwrap();
        std::fs::write(dir.join("a1-1200-1.img"), b"other size").unwrap();
        std::fs::write(dir.join("a1-640-2.img"), b"new").unwrap();
        let c = CoverCache::new(&dir);
        c.forget_older("a1", 640, 2);
        assert!(!dir.join("a1-640-1.img").exists());
        assert!(dir.join("a1-1200-1.img").exists());
        assert_eq!(c.cached("a1", 640, 2).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
