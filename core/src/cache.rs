//! Songs kept on this device as they are queued, so they play from disk (steady, and without the server) and
//! replaying them costs no data. Not the same as downloads: nobody chose these, and the oldest ones go when the
//! cache is over its limit or you clear it.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::client::Client;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct SongCache {
    dir: PathBuf,
}

fn safe(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl SongCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        SongCache { dir: dir.into().join("songs") }
    }

    /// Where a song is (or would be) kept.
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        safe(id).then(|| self.dir.join(format!("{id}.audio")))
    }

    pub fn has(&self, id: &str) -> bool {
        self.path(id).map(|p| p.exists()).unwrap_or(false)
    }

    /// Note that a song was just used (the oldest unused ones are removed first).
    pub fn touch(&self, id: &str) {
        if let Some(p) = self.path(id) {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(p) {
                let _ = f.set_modified(SystemTime::now());
            }
        }
    }

    /// Download a whole song into the cache (nothing to do when it is there already). A song that stops part way
    /// is thrown away rather than kept broken.
    pub fn fetch(&self, client: &Client, id: &str) -> Result<()> {
        let path = self.path(id).ok_or_else(|| Error::Disk("bad song id".into()))?;
        if path.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        let (mut reader, total, _) = client.open_stream(id, 0)?;
        let tmp = path.with_extension("part");
        let mut out = std::fs::File::create(&tmp)?;
        let mut buf = vec![0u8; 128 * 1024];
        let mut got = 0u64;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    drop(out);
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
            };
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            got += n as u64;
        }
        out.flush()?;
        drop(out);
        if total.map(|t| t != got).unwrap_or(false) {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Unreachable("the song stopped part way".into()));
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// (songs kept, bytes used).
    pub fn stats(&self) -> (usize, u64) {
        let mut n = 0;
        let mut bytes = 0;
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if let Ok(m) = e.metadata() {
                    bytes += m.len();
                    if e.file_name().to_string_lossy().ends_with(".audio") {
                        n += 1;
                    }
                }
            }
        }
        (n, bytes)
    }

    /// Remove everything. Returns the bytes freed.
    pub fn clear(&self) -> u64 {
        let (_, bytes) = self.stats();
        let _ = std::fs::remove_dir_all(&self.dir);
        bytes
    }

    /// Keep the cache under `max` bytes by removing the songs used longest ago (never one of `keep`).
    pub fn trim(&self, max: u64, keep: &[String]) {
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return };
        let mut files: Vec<(SystemTime, u64, PathBuf, String)> = rd
            .flatten()
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                let name = e.file_name().to_string_lossy().to_string();
                let id = name.strip_suffix(".audio")?.to_string();
                Some((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path(), id))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        for (_, len, path, id) in files {
            if total <= max {
                break;
            }
            if keep.contains(&id) {
                continue;
            }
            if std::fs::remove_file(path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
    }
}
