//! Private data at rest.
//!  - Passwords are never stored: only Argon2 hashes (see auth.rs).
//!  - Sign-in tokens are stored as SHA-256 hashes, so a copy of sessions.json can't be used to sign in.
//!  - Everything else the server keeps about people (accounts, playlists, history, Lidarr key, requests) can be
//!    encrypted with AES-256-GCM: set VH_DATA_KEY to a long secret and files are written encrypted
//!    (`VHENC1:` + base64). Existing plain files are still read, and get encrypted the next time they change.
//!    Lose the key and the data is unreadable, so keep a copy somewhere safe.

use std::path::Path;
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

const PREFIX: &str = "VHENC1:";
static KEY: OnceLock<Option<[u8; 32]>> = OnceLock::new();

fn key() -> Option<&'static [u8; 32]> {
    KEY.get_or_init(|| {
        let secret = std::env::var("VH_DATA_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let mut out = [0u8; 32];
        Argon2::default().hash_password_into(secret.trim().as_bytes(), b"volvix-home-data-key", &mut out).ok()?;
        Some(out)
    })
    .as_ref()
}

pub fn enabled() -> bool {
    key().is_some()
}

pub fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn encrypt(text: &str) -> String {
    let Some(k) = key() else { return text.to_string() };
    let cipher = Aes256Gcm::new_from_slice(k).expect("32-byte key");
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    match cipher.encrypt(Nonce::from_slice(&nonce), text.as_bytes()) {
        Ok(ct) => {
            let mut blob = nonce.to_vec();
            blob.extend(ct);
            format!("{PREFIX}{}", B64.encode(blob))
        }
        Err(_) => text.to_string(),
    }
}

pub fn decrypt(text: &str) -> Option<String> {
    let Some(rest) = text.strip_prefix(PREFIX) else { return Some(text.to_string()) };
    let k = key()?; // encrypted file but no key: unreadable
    let blob = B64.decode(rest.trim()).ok()?;
    if blob.len() < 13 {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(k).ok()?;
    let pt = cipher.decrypt(Nonce::from_slice(&blob[..12]), &blob[12..]).ok()?;
    String::from_utf8(pt).ok()
}

/// Read a data file (decrypting it when needed).
pub fn read(path: &Path) -> Option<String> {
    decrypt(&std::fs::read_to_string(path).ok()?)
}

/// Write a data file (encrypted when VH_DATA_KEY is set), through a temp file so a crash never leaves half a file.
pub fn write(path: &Path, text: &str) -> bool {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, encrypt(text)).is_ok() && std::fs::rename(&tmp, path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through_and_hash_is_stable() {
        assert_eq!(decrypt("hello").unwrap(), "hello");
        assert_eq!(sha256_hex("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn encrypted_round_trip() {
        // exercise the cipher directly with a fixed key (the env key is process-wide)
        let cipher = Aes256Gcm::new_from_slice(&[7u8; 32]).unwrap();
        let ct = cipher.encrypt(Nonce::from_slice(&[1u8; 12]), b"secret".as_ref()).unwrap();
        assert_ne!(ct, b"secret");
        assert_eq!(cipher.decrypt(Nonce::from_slice(&[1u8; 12]), ct.as_ref()).unwrap(), b"secret");
        assert!(cipher.decrypt(Nonce::from_slice(&[2u8; 12]), ct.as_ref()).is_err());
    }
}
