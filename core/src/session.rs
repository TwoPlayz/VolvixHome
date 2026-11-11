//! The server address and sign-in, kept on disk so the app opens straight to your music.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;
use crate::model::User;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SavedSession {
    /// "https://music.example.com"
    pub server: String,
    #[serde(default)]
    pub token: Option<String>,
    /// who signed in last (shown before the server answers)
    #[serde(default)]
    pub user: Option<User>,
}

impl SavedSession {
    pub fn file(dir: &Path) -> PathBuf {
        dir.join("session.json")
    }

    pub fn load(dir: &Path) -> Option<SavedSession> {
        let text = std::fs::read_to_string(Self::file(dir)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("session.json.part");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(&tmp, Self::file(dir))?;
        Ok(())
    }

    /// Sign out on this device: keep the address (to sign in again), forget the token.
    pub fn signed_out(&self) -> SavedSession {
        SavedSession { server: self.server.clone(), token: None, user: self.user.clone() }
    }

    pub fn client(&self) -> Client {
        Client::new(&self.server, self.token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("vh-session-{}", fastrand::u64(..)));
        assert!(SavedSession::load(&dir).is_none());
        let s = SavedSession { server: "http://localhost:4747".into(), token: Some("abc".into()), user: None };
        s.save(&dir).unwrap();
        assert_eq!(SavedSession::load(&dir).unwrap(), s);
        assert_eq!(s.signed_out().token, None);
        assert_eq!(s.client().token(), Some("abc"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
