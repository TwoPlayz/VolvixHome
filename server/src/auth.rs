//! Household accounts: users with Argon2 password hashes, and sign-in sessions.
//! Stored as small JSON files in the data folder (users.json, sessions.json). The first person to
//! open a fresh server creates the admin account; the admin adds everyone else.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub name: String,
    pub hash: String,
    pub admin: bool,
    pub created: u64,
}

/// What the app gets to see about a user (never the hash).
#[derive(Serialize)]
pub struct PublicUser {
    pub id: String,
    pub username: String,
    pub name: String,
    pub admin: bool,
}

impl From<&User> for PublicUser {
    fn from(u: &User) -> Self {
        PublicUser { id: u.id.clone(), username: u.username.clone(), name: u.name.clone(), admin: u.admin }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub user_id: String,
    pub created: u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok()).unwrap_or(false)
}

/// Sessions last 180 days; signing out removes them.
const SESSION_DAYS: u64 = 180;

pub struct Accounts {
    dir: PathBuf,
    pub users: Vec<User>,
    pub sessions: HashMap<String, Session>,
}

impl Accounts {
    pub fn load(dir: &Path) -> Accounts {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let users = read("users.json").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut sessions: HashMap<String, Session> = read("sessions.json").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let cutoff = now().saturating_sub(SESSION_DAYS * 86400);
        sessions.retain(|_, s| s.created > cutoff);
        Accounts { dir: dir.to_path_buf(), users, sessions }
    }

    fn save(&self) {
        // write to a temp file first so a crash never leaves a half-written file
        let write = |name: &str, body: String| {
            let tmp = self.dir.join(format!("{name}.tmp"));
            if std::fs::write(&tmp, body).is_ok() {
                let _ = std::fs::rename(&tmp, self.dir.join(name));
            }
        };
        write("users.json", serde_json::to_string_pretty(&self.users).unwrap_or_default());
        write("sessions.json", serde_json::to_string_pretty(&self.sessions).unwrap_or_default());
    }

    pub fn needs_setup(&self) -> bool {
        self.users.is_empty()
    }

    pub fn find(&self, username: &str) -> Option<&User> {
        let u = username.trim().to_lowercase();
        self.users.iter().find(|x| x.username == u)
    }

    pub fn by_id(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|x| x.id == id)
    }

    pub fn create(&mut self, username: &str, name: &str, password: &str, admin: bool) -> Result<User, String> {
        let username = username.trim().to_lowercase();
        if username.len() < 2 || !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return Err("Usernames use letters, numbers, dots, dashes or underscores (2 or more).".into());
        }
        if password.chars().count() < 8 {
            return Err("Passwords need at least 8 characters.".into());
        }
        if self.find(&username).is_some() {
            return Err("That username is taken.".into());
        }
        let user = User {
            id: format!("u{}", random_hex(6)),
            username,
            name: if name.trim().is_empty() { "Listener".into() } else { name.trim().chars().take(60).collect() },
            hash: hash_password(password)?,
            admin,
            created: now(),
        };
        self.users.push(user.clone());
        self.save();
        Ok(user)
    }

    pub fn set_password(&mut self, user_id: &str, password: &str) -> Result<(), String> {
        if password.chars().count() < 8 {
            return Err("Passwords need at least 8 characters.".into());
        }
        let hash = hash_password(password)?;
        let user = self.users.iter_mut().find(|u| u.id == user_id).ok_or("No such user")?;
        user.hash = hash;
        // other devices have to sign in again with the new password
        self.sessions.retain(|_, s| s.user_id != user_id);
        self.save();
        Ok(())
    }

    pub fn remove(&mut self, user_id: &str) -> Result<(), String> {
        let admins = self.users.iter().filter(|u| u.admin).count();
        let user = self.by_id(user_id).ok_or("No such user")?;
        if user.admin && admins <= 1 {
            return Err("Keep at least one admin.".into());
        }
        self.users.retain(|u| u.id != user_id);
        self.sessions.retain(|_, s| s.user_id != user_id);
        self.save();
        Ok(())
    }

    pub fn start_session(&mut self, user_id: &str) -> String {
        let token = random_hex(32);
        self.sessions.insert(token.clone(), Session { user_id: user_id.to_string(), created: now() });
        self.save();
        token
    }

    pub fn end_session(&mut self, token: &str) {
        if self.sessions.remove(token).is_some() {
            self.save();
        }
    }

    pub fn user_for_token(&self, token: &str) -> Option<&User> {
        let s = self.sessions.get(token)?;
        if s.created + SESSION_DAYS * 86400 < now() {
            return None;
        }
        self.by_id(&s.user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_round_trip() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn accounts_rules() {
        let dir = std::env::temp_dir().join(format!("vh-test-{}", random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Accounts::load(&dir);
        assert!(a.needs_setup());
        let admin = a.create("Justin", "Justin", "password1", true).unwrap();
        assert_eq!(admin.username, "justin");
        assert!(a.create("justin", "x", "password1", false).is_err(), "duplicate");
        assert!(a.create("kid", "Kid", "short", false).is_err(), "short password");
        let kid = a.create("kid", "Kid", "password2", false).unwrap();
        let t = a.start_session(&kid.id);
        assert_eq!(a.user_for_token(&t).unwrap().id, kid.id);
        assert!(a.remove(&admin.id).is_err(), "last admin stays");
        a.set_password(&kid.id, "newpassword").unwrap();
        assert!(a.user_for_token(&t).is_none(), "password change signs out other devices");
        let again = Accounts::load(&dir);
        assert_eq!(again.users.len(), 2, "saved to disk");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
