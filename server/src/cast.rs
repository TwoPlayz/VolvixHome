//! Cast links. A Chromecast (or any TV / speaker) fetches the song itself, and it can't hold your sign-in cookie.
//! So the app asks for a short-lived signed token and puts it in the song's address (`/api/stream/<id>?cast=<token>`).
//! The token names one person, expires after 12 hours, and only unlocks streaming songs and covers: nothing else.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha = Hmac<Sha256>;

pub const LIFETIME: u64 = 12 * 3600;

fn mac(key: &str, payload: &str) -> HmacSha {
    let mut m = <HmacSha as Mac>::new_from_slice(key.as_bytes()).expect("any key length");
    m.update(payload.as_bytes());
    m
}

pub fn sign(key: &str, user_id: &str, expires: u64) -> String {
    let payload = format!("{user_id}.{expires}");
    let sig: String = mac(key, &payload).finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!("{payload}.{sig}")
}

/// The person a valid, unexpired token belongs to.
pub fn verify(key: &str, token: &str, now: u64) -> Option<String> {
    let mut parts = token.rsplitn(2, '.');
    let sig = parts.next()?;
    let payload = parts.next()?;
    let (uid, exp) = payload.split_once('.')?;
    if exp.parse::<u64>().ok()? < now { return None; }
    let bytes: Vec<u8> = (0..sig.len() / 2).filter_map(|i| u8::from_str_radix(sig.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
    if bytes.len() * 2 != sig.len() { return None; }
    mac(key, payload).verify_slice(&bytes).ok()?;
    Some(uid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_verify_expire_and_resist_tampering() {
        let t = sign("k", "u123", 2000);
        assert_eq!(verify("k", &t, 1000).as_deref(), Some("u123"));
        assert!(verify("k", &t, 2001).is_none(), "expired");
        assert!(verify("other", &t, 1000).is_none(), "wrong key");
        assert!(verify("k", &t.replace("u123", "u999"), 1000).is_none(), "another person");
        assert!(verify("k", &t.replace(".2000.", ".9999."), 1000).is_none(), "longer life");
        assert!(verify("k", "garbage", 1000).is_none());
    }
}
