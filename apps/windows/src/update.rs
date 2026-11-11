//! Updates from GitHub releases (TwoPlayz/VolvixHome). The release has `volvix-home.exe` and, next to it,
//! `volvix-home.exe.sha256`. The new exe is downloaded to the app's own folder, checked against that hash, and
//! swapped in (Windows lets a running exe be renamed, not overwritten): the old one becomes `volvix-home.old`
//! until the next start. The app then restarts itself into the new copy (the caller does that on `Ready`).

use std::io::Read;
use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};

const REPO: &str = "TwoPlayz/VolvixHome";

#[derive(Clone, Debug)]
pub enum State {
    Checking,
    UpToDate,
    Downloading(String),
    /// installed on disk; the new version starts after a restart
    Ready(String),
    Failed(String),
}

impl State {
    pub fn to_json(&self) -> Value {
        use serde_json::json;
        match self {
            State::Checking => json!({ "state": "checking" }),
            State::UpToDate => json!({ "state": "current" }),
            State::Downloading(v) => json!({ "state": "downloading", "version": v }),
            State::Ready(v) => json!({ "state": "ready", "version": v }),
            State::Failed(m) => json!({ "state": "failed", "message": m }),
        }
    }
}

fn nums(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches('v').split('.').map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0)).collect()
}

/// Is `a` a newer version than `b`? ("0.10.0" > "0.9.3")
pub fn newer(a: &str, b: &str) -> bool {
    let (x, y) = (nums(a), nums(b));
    for i in 0..x.len().max(y.len()) {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q { return p > q; }
    }
    false
}

fn dir() -> PathBuf {
    let d = crate::app_dir().join("update");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn old_exe() -> Option<PathBuf> {
    std::env::current_exe().ok().map(|e| e.with_extension("old"))
}

/// After an update the previous exe is still on disk: remove it.
pub fn clean_old() {
    if let Some(p) = old_exe() { let _ = std::fs::remove_file(p); }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).user_agent(concat!("VolvixHome-app/", env!("CARGO_PKG_VERSION"))).build()
}

/// Check, download, verify and install. `manual` = the person asked (so say "up to date" too).
pub fn run(on: &dyn Fn(State)) -> State {
    let a = agent();
    let release: Value = match a.get(&format!("https://api.github.com/repos/{REPO}/releases/latest")).set("Accept", "application/vnd.github+json").call() {
        Ok(r) => match r.into_json() { Ok(v) => v, Err(_) => return State::Failed("Couldn't read the update information.".into()) },
        Err(ureq::Error::Status(404, _)) => return State::Failed("GitHub has no published release to update from. Publish one by pushing a v* tag, and the repository must be public, since the app can't sign in to GitHub.".into()),
        Err(ureq::Error::Status(403, _)) | Err(ureq::Error::Status(429, _)) => return State::Failed("GitHub is limiting update checks right now. Try again in a little while.".into()),
        Err(_) => return State::Failed("Couldn't reach GitHub.".into()),
    };
    let tag = release["tag_name"].as_str().unwrap_or("");
    if !newer(tag, env!("CARGO_PKG_VERSION")) { return State::UpToDate; }
    let version = tag.trim_start_matches('v').to_string();
    let find = |name: &str| release["assets"].as_array().and_then(|l| l.iter().find(|x| x["name"].as_str() == Some(name))).and_then(|x| x["browser_download_url"].as_str()).map(String::from);
    let Some(exe_url) = find(crate::RELEASE_EXE) else { return State::Failed("That release has no Windows app.".into()) };
    let sha_url = find(&format!("{}.sha256", crate::RELEASE_EXE));

    on(State::Downloading(version.clone()));
    let mut bytes = Vec::new();
    match a.get(&exe_url).call() {
        Ok(r) => { if r.into_reader().take(200 * 1024 * 1024).read_to_end(&mut bytes).is_err() { return State::Failed("The download was interrupted.".into()); } }
        Err(_) => return State::Failed("Couldn't download the update.".into()),
    }
    // refuse anything that isn't the file the release published
    match sha_url {
        Some(u) => {
            let want = a.get(&u).call().ok().and_then(|r| r.into_string().ok()).and_then(|t| t.split_whitespace().next().map(|s| s.to_lowercase()));
            let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
            if want.as_deref() != Some(got.as_str()) { return State::Failed("The download didn't match its checksum, so it wasn't installed.".into()); }
        }
        None => return State::Failed("That release has no checksum, so it wasn't installed.".into()),
    }
    if bytes.len() < 1_000_000 || &bytes[..2] != b"MZ" { return State::Failed("The download isn't a Windows program.".into()); }

    let new = dir().join(format!("{}.new", crate::RELEASE_EXE));
    if std::fs::write(&new, &bytes).is_err() { return State::Failed("Couldn't save the update.".into()); }
    match install(&new) {
        Ok(()) => State::Ready(version),
        Err(e) => State::Failed(e),
    }
}

fn install(new: &PathBuf) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = old_exe().ok_or("No program path.")?;
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|_| "Couldn't replace the app (is it in a protected folder?).".to_string())?;
    if let Err(e) = std::fs::copy(new, &exe) {
        let _ = std::fs::rename(&old, &exe); // put the working one back
        return Err(format!("Couldn't install the update: {e}"));
    }
    let _ = std::fs::remove_file(new);
    Ok(())
}

/// Start the new copy (it waits for this one to exit). Returns true when it started.
pub fn restart() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    std::process::Command::new(exe).arg("--after-update").spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("v0.10.0", "0.9.3"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(newer("0.1.1", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("v0.1.0", "0.1.1"));
        assert!(newer("0.2", "0.1.9"));
    }
}
