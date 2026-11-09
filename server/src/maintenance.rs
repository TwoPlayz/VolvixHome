//! The nightly housekeeping: the scheduled security and safety jobs. Runs once shortly after start and then every 24 hours.
//!  - sign-ins that have expired are removed
//!  - a dated backup of everything the server knows about people (accounts, playlists, history, settings) goes to
//!    <data>/backups/YYYYMMDD, keeping the newest VH_BACKUP_KEEP days (default 7; 0 turns backups off).
//!    If VH_DATA_KEY is set the backup is encrypted too, because the files are copied as they are.
//!  - old lyrics / credits look-ups are cleaned out of the cache
//! (The weekly dependency audit and Docker refresh run on GitHub: .github/workflows/security.yml.)

use std::path::{Path, PathBuf};

/// Days since 1970 -> (year, month, day). Civil-from-days (Howard Hinnant).
pub fn ymd(secs: u64) -> (i64, u32, u32) {
    let z = (secs / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Copy the people-data to backups/<date>, then delete all but the newest `keep` backups.
pub fn backup(data: &Path, now: u64, keep: usize) -> Result<PathBuf, String> {
    let (y, m, d) = ymd(now);
    let dest = data.join("backups").join(format!("{y:04}{m:02}{d:02}"));
    let _ = std::fs::remove_dir_all(&dest); // today's backup is replaced, not stacked
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    // sessions.json is left out on purpose: a backup shouldn't hold sign-ins
    for f in ["users.json", "lidarr.json", "requests.json"] {
        let p = data.join(f);
        if p.exists() {
            std::fs::copy(&p, dest.join(f)).map_err(|e| e.to_string())?;
        }
    }
    for dir in ["userdata", "shared", "notifications", "avatars"] {
        let p = data.join(dir);
        if p.is_dir() {
            copy_dir(&p, &dest.join(dir)).map_err(|e| e.to_string())?;
        }
    }
    let mut all: Vec<PathBuf> = std::fs::read_dir(data.join("backups")).map_err(|e| e.to_string())?
        .filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    all.sort();
    while all.len() > keep.max(1) {
        let _ = std::fs::remove_dir_all(all.remove(0));
    }
    Ok(dest)
}

/// Remove cached look-ups that haven't been touched for `days`.
pub fn clean_cache(data: &Path, days: u64) -> usize {
    let mut removed = 0;
    let limit = std::time::Duration::from_secs(days * 86400);
    for sub in ["credits", "lyrics"] {
        let Ok(rd) = std::fs::read_dir(data.join("cache").join(sub)) else { continue };
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|a| a > limit).unwrap_or(false);
            if old && std::fs::remove_file(e.path()).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(ymd(0), (1970, 1, 1));
        assert_eq!(ymd(1_709_164_800), (2024, 2, 29)); // a leap day
        assert_eq!(ymd(1_798_761_600), (2027, 1, 1));
    }

    #[test]
    fn backups_are_dated_pruned_and_skip_sessions() {
        let dir = std::env::temp_dir().join(format!("vh-bk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("userdata")).unwrap();
        std::fs::write(dir.join("users.json"), "[]").unwrap();
        std::fs::write(dir.join("sessions.json"), "{}").unwrap();
        std::fs::write(dir.join("userdata").join("a.json"), "{}").unwrap();
        for day in 0..4u64 {
            backup(&dir, 1_700_000_000 + day * 86400, 3).unwrap();
        }
        let kept: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().flatten().collect();
        assert_eq!(kept.len(), 3);
        let newest = backup(&dir, 1_700_000_000 + 3 * 86400, 3).unwrap();
        assert!(newest.join("users.json").exists() && newest.join("userdata").join("a.json").exists());
        assert!(!newest.join("sessions.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
