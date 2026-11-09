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

/// What a person can be allowed to do. An admin can do all of it.
pub const PERMS: [&str; 6] = ["request", "rescan", "parties", "members", "settings", "share"];
/// What a new member gets until an admin changes it.
pub const DEFAULT_PERMS: [&str; 2] = ["request", "parties"];

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub name: String,
    pub hash: String,
    pub admin: bool,
    pub created: u64,
    /// an admin turned explicit songs off for this person
    #[serde(default)]
    pub hide_explicit: bool,
    /// profile picture version (seconds); 0 = none
    #[serde(default)]
    pub avatar: u64,
    /// what they may do (None = the member defaults). Admins can do everything whatever this says.
    #[serde(default)]
    pub perms: Option<Vec<String>>,
}

impl User {
    /// Everything this person is allowed to do.
    pub fn effective(&self) -> Vec<String> {
        if self.admin { return PERMS.iter().map(|p| p.to_string()).collect(); }
        match &self.perms {
            Some(l) => PERMS.iter().filter(|p| l.iter().any(|x| x == *p)).map(|p| p.to_string()).collect(),
            None => DEFAULT_PERMS.iter().map(|p| p.to_string()).collect(),
        }
    }
    pub fn has(&self, perm: &str) -> bool {
        self.admin || self.effective().iter().any(|p| p == perm)
    }
}

/// What the app gets to see about a user (never the hash).
#[derive(Serialize)]
pub struct PublicUser {
    pub id: String,
    pub username: String,
    pub name: String,
    pub admin: bool,
    #[serde(rename = "hideExplicit")]
    pub hide_explicit: bool,
    pub avatar: u64,
    pub perms: Vec<String>,
}

impl From<&User> for PublicUser {
    fn from(u: &User) -> Self {
        PublicUser { id: u.id.clone(), username: u.username.clone(), name: u.name.clone(), admin: u.admin, hide_explicit: u.hide_explicit, avatar: u.avatar, perms: u.effective() }
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
        let read = |name: &str| crate::vault::read(&dir.join(name));
        let users = read("users.json").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut sessions: HashMap<String, Session> = read("sessions.json").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let cutoff = now().saturating_sub(SESSION_DAYS * 86400);
        sessions.retain(|_, s| s.created > cutoff);
        Accounts { dir: dir.to_path_buf(), users, sessions }
    }

    fn save(&self) {
        // write to a temp file first so a crash never leaves a half-written file
        let write = |name: &str, body: String| {
            crate::vault::write(&self.dir.join(name), &body);
        };
        write("users.json", serde_json::to_string_pretty(&self.users).unwrap_or_default());
        write("sessions.json", serde_json::to_string_pretty(&self.sessions).unwrap_or_default());
    }

    /// A slug of the name that no one else is using.
    fn free_username(&self, name: &str) -> String {
        let mut slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        slug = slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
        if slug.len() < 2 {
            slug = "user".into();
        }
        let mut candidate = slug.clone();
        let mut n = 2;
        while self.users.iter().any(|u| u.username == candidate) {
            candidate = format!("{slug}{n}");
            n += 1;
        }
        candidate
    }

    pub fn needs_setup(&self) -> bool {
        self.users.is_empty()
    }

    /// Sign-in is by name (the household's first names); the short internal username also works.
    pub fn find(&self, who: &str) -> Option<&User> {
        let u = who.trim().to_lowercase();
        if u.is_empty() {
            return None;
        }
        self.users.iter().find(|x| x.username == u || x.name.trim().to_lowercase() == u)
    }

    pub fn by_id(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|x| x.id == id)
    }

    /// username is optional: when empty it's made from the name (\Sam Patel\ -> \sam-patel\, then \sam-patel2\…).
    pub fn create(&mut self, username: &str, name: &str, password: &str, admin: bool) -> Result<User, String> {
        let name = name.trim();
        if name.chars().count() < 2 {
            return Err("Please enter a name (2 or more letters).".into());
        }
        if self.find(name).is_some() {
            return Err("Someone in your household already has that name.".into());
        }
        let username = if username.trim().is_empty() { self.free_username(name) } else { username.trim().to_lowercase() };
        if username.len() < 2 || !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return Err("Usernames use letters, numbers, dots, dashes or underscores (2 or more).".into());
        }
        if password.chars().count() < 8 {
            return Err("Passwords need at least 8 characters.".into());
        }
        if self.find(&username).is_some() {
            return Err("Someone in your household already has that name.".into());
        }
        let user = User {
            id: format!("u{}", random_hex(6)),
            username,
            name: if name.trim().is_empty() { "Listener".into() } else { name.trim().chars().take(60).collect() },
            hash: hash_password(password)?,
            admin,
            created: now(),
            hide_explicit: false,
            avatar: 0,
            perms: None,
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

    /// Forget sign-ins past their time.
    pub fn purge_expired(&mut self) {
        let cutoff = now().saturating_sub(SESSION_DAYS * 86400);
        let before = self.sessions.len();
        self.sessions.retain(|_, s| s.created > cutoff);
        if self.sessions.len() != before { self.save(); }
    }

    pub fn set_hide_explicit(&mut self, user_id: &str, on: bool) -> Result<(), String> {
        self.users.iter_mut().find(|u| u.id == user_id).ok_or("No such user")?.hide_explicit = on;
        self.save();
        Ok(())
    }

    /// Replace what someone may do. Having every permission makes them an admin. At least one admin must remain.
    pub fn set_perms(&mut self, user_id: &str, perms: &[String]) -> Result<(), String> {
        let admins = self.users.iter().filter(|u| u.admin).count();
        let all = PERMS.iter().all(|p| perms.iter().any(|x| x == p));
        let user = self.users.iter_mut().find(|u| u.id == user_id).ok_or("No such user")?;
        if user.admin && !all && admins <= 1 {
            return Err("Keep at least one admin.".into());
        }
        user.admin = all;
        user.perms = Some(PERMS.iter().filter(|p| perms.iter().any(|x| x == *p)).map(|p| p.to_string()).collect());
        self.save();
        Ok(())
    }
    pub fn set_avatar(&mut self, user_id: &str, version: u64) {
        if let Some(u) = self.users.iter_mut().find(|u| u.id == user_id) {
            u.avatar = version;
            self.save();
        }
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
        self.sessions.insert(crate::vault::sha256_hex(&token), Session { user_id: user_id.to_string(), created: now() });
        self.save();
        token
    }

    pub fn end_session(&mut self, token: &str) {
        if self.sessions.remove(&crate::vault::sha256_hex(token)).is_some() {
            self.save();
        }
    }

    pub fn user_for_token(&self, token: &str) -> Option<&User> {
        let s = self.sessions.get(&crate::vault::sha256_hex(token))?;
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
    fn permissions_default_grow_and_keep_an_admin() {
        let dir = std::env::temp_dir().join(format!("vh-perm-{}", random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Accounts::load(&dir);
        let boss = a.create("", "Boss", "password1", true).unwrap();
        let kid = a.create("", "Kid", "password1", false).unwrap();
        // a new member has the basics only
        assert!(kid.has("request") && kid.has("parties") && !kid.has("rescan") && !kid.has("members"));
        assert!(boss.has("settings") && boss.effective().len() == PERMS.len());
        // give one extra permission: still not an admin
        a.set_perms(&kid.id, &["request".into(), "rescan".into()]).unwrap();
        let kid = a.by_id(&kid.id).unwrap().clone();
        assert!(kid.has("rescan") && !kid.has("parties") && !kid.admin);
        // every permission makes an admin; unknown names are dropped
        a.set_perms(&kid.id, &PERMS.iter().map(|p| p.to_string()).chain(std::iter::once("fly".to_string())).collect::<Vec<_>>()).unwrap();
        assert!(a.by_id(&kid.id).unwrap().admin);
        assert!(!a.by_id(&kid.id).unwrap().perms.as_ref().unwrap().contains(&"fly".to_string()));
        // the last admin can't be demoted
        a.set_perms(&kid.id, &["request".into()]).unwrap(); // boss is still admin
        assert!(a.set_perms(&boss.id, &["request".into()]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn passwords_round_trip() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn sign_in_by_name_without_a_username() {
        let dir = std::env::temp_dir().join(format!("vh-test-{}", random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Accounts::load(&dir);
        let sam = a.create("", "Sam Patel", "password1", true).unwrap();
        assert_eq!(sam.username, "sam-patel");
        // found by name in any case, or by the internal username
        assert_eq!(a.find("sam patel").unwrap().id, sam.id);
        assert_eq!(a.find("  SAM PATEL ").unwrap().id, sam.id);
        assert_eq!(a.find("sam-patel").unwrap().id, sam.id);
        // names are unique, ignoring case
        assert!(a.create("", "sam PATEL", "password1", false).is_err());
        // first names that slug the same still get their own account
        let a2 = a.create("", "Sam-Patel!", "password1", false);
        assert!(a2.is_ok() && a2.unwrap().username == "sam-patel2");
        assert!(a.create("", "X", "password1", false).is_err(), "name too short");
        assert!(a.find("").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accounts_rules() {
        let dir = std::env::temp_dir().join(format!("vh-test-{}", random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Accounts::load(&dir);
        assert!(a.needs_setup());
        let admin = a.create("Justin", "Justin", "password1", true).unwrap();
        assert_eq!(admin.username, "justin");
        assert!(a.create("justin", "x", "password1", false).is_err(), "duplicate / too short");
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
