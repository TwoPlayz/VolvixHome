//! Streaming at a lower quality (to save data on a phone). It uses ffmpeg when it's installed (the Docker image has it).
//! The first time a song is wanted at a lower quality it's converted once and kept in <data>/cache/transcode, then
//! served like any file (so seeking works). Songs that are already small enough are sent as they are. Without ffmpeg
//! everything simply streams in original quality.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The qualities a person can ask for: (name, AAC kbps).
pub const PROFILES: [(&str, u32); 3] = [("high", 256), ("normal", 160), ("low", 96)];

pub struct Transcoder {
    ffmpeg: Option<PathBuf>,
    dir: PathBuf,
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

pub fn kbps_of(q: &str) -> Option<u32> {
    PROFILES.iter().find(|(n, _)| *n == q).map(|(_, k)| *k)
}

/// Is a song that's already this size or smaller (lossy) worth converting? No: converting again only loses quality.
pub fn already_small(lossless: bool, source_kbps: Option<u32>, target: u32) -> bool {
    !lossless && source_kbps.map(|k| k <= target + 16).unwrap_or(false)
}

impl Transcoder {
    pub fn new(data_dir: &Path) -> Transcoder {
        let wanted = std::env::var("VH_FFMPEG").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "ffmpeg".into());
        let works = std::process::Command::new(&wanted).arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
        Transcoder {
            ffmpeg: if works { Some(PathBuf::from(wanted)) } else { None },
            dir: data_dir.join("cache").join("transcode"),
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn available(&self) -> bool {
        self.ffmpeg.is_some()
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The converted file for a song (made now if it isn't there yet).
    pub async fn ensure(&self, src: &Path, id: &str, quality: &str) -> Result<PathBuf, String> {
        let ffmpeg = self.ffmpeg.clone().ok_or("No ffmpeg.")?;
        let kbps = kbps_of(quality).ok_or("Unknown quality.")?;
        let out = self.dir.join(format!("{id}-{quality}.m4a"));
        if std::fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false) {
            let _ = filetime_touch(&out);
            return Ok(out);
        }
        // one conversion per song and quality at a time, whoever asks
        let lock = self.locks.lock().unwrap().entry(format!("{id}-{quality}")).or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))).clone();
        let _guard = lock.lock().await;
        if std::fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false) {
            return Ok(out);
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let part = self.dir.join(format!("{id}-{quality}.part.m4a"));
        let run = tokio::process::Command::new(ffmpeg)
            .args(["-nostdin", "-v", "error", "-y", "-i"]).arg(src)
            .args(["-vn", "-map_metadata", "-1", "-c:a", "aac", "-b:a"]).arg(format!("{kbps}k"))
            .args(["-movflags", "+faststart"]).arg(&part)
            .kill_on_drop(true)
            .status();
        let status = tokio::time::timeout(Duration::from_secs(180), run).await.map_err(|_| "Converting took too long.".to_string())?.map_err(|e| e.to_string())?;
        if !status.success() || std::fs::metadata(&part).map(|m| m.len() == 0).unwrap_or(true) {
            let _ = std::fs::remove_file(&part);
            return Err("Couldn't convert that song.".into());
        }
        std::fs::rename(&part, &out).map_err(|e| e.to_string())?;
        Ok(out)
    }
}

fn filetime_touch(p: &Path) -> std::io::Result<()> {
    // reading it counts as use: rewrite the modified time so the clean-up keeps what's played
    let f = std::fs::OpenOptions::new().write(true).open(p)?;
    f.set_modified(std::time::SystemTime::now())
}

/// Delete converted files until the folder is under `max_bytes`, oldest first, and anything not used for `max_days`.
pub fn clean(dir: &Path, max_bytes: u64, max_days: u64) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd.flatten().filter_map(|e| {
        let m = e.metadata().ok()?;
        Some((m.modified().ok()?, m.len(), e.path()))
    }).collect();
    files.sort_by_key(|f| f.0);
    let limit = Duration::from_secs(max_days * 86400);
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    let mut removed = 0;
    for (at, len, path) in files {
        let old = at.elapsed().map(|a| a > limit).unwrap_or(false);
        if (old || total > max_bytes) && std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_and_skipping() {
        assert_eq!(kbps_of("low"), Some(96));
        assert_eq!(kbps_of("original"), None);
        assert!(already_small(false, Some(128), 160), "a 128 kbps mp3 is already small enough");
        assert!(!already_small(false, Some(320), 160));
        assert!(!already_small(true, None, 160), "lossless always converts");
        assert!(!already_small(false, None, 160), "unknown bitrate converts");
    }

    #[test]
    fn clean_removes_oldest_until_small_enough() {
        let dir = std::env::temp_dir().join(format!("vh-tc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, n) in ["a", "b", "c"].iter().enumerate() {
            let p = dir.join(n);
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
            f.set_modified(std::time::SystemTime::now() - Duration::from_secs((3 - i as u64) * 10)).unwrap();
        }
        assert_eq!(clean(&dir, 150, 30), 2); // 300 bytes -> down to 100: the two oldest go
        assert!(dir.join("c").exists() && !dir.join("a").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
