//! Playing on TVs and speakers (Google Cast, found and driven by the server) and listening parties.
//! The same server calls the website makes (web/js/cast.js, web/js/party.js).
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::{enc, Client};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CastDevice {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model: String,
}

/// What a TV says it is doing.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CastStatus {
    #[serde(default)]
    pub connected: bool,
    #[serde(default, rename = "playerState")]
    pub player_state: String,
    #[serde(default, rename = "idleReason")]
    pub idle_reason: String,
    #[serde(default, rename = "currentTime")]
    pub current_time: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PartyMember {
    pub id: String,
    #[serde(default)]
    pub output: String,
}

/// A listening party as the server describes it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Party {
    pub id: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub members: Vec<PartyMember>,
    #[serde(default)]
    pub invited: Vec<String>,
    #[serde(default)]
    pub queue: Vec<String>,
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub playing: bool,
    #[serde(default, rename = "posMs")]
    pub pos_ms: i64,
    #[serde(default)]
    pub version: u64,
}

fn read<T: for<'de> Deserialize<'de>>(res: ureq::Response) -> Result<T> {
    res.into_json::<T>().map_err(|e| Error::BadAnswer(e.to_string()))
}

impl Client {
    /* ---------- TVs and speakers ---------- */

    pub fn cast_devices(&self) -> Result<Vec<CastDevice>> {
        let v: Value = self.get_json("/api/cast/devices")?;
        Ok(serde_json::from_value(v["devices"].clone()).unwrap_or_default())
    }

    /// Add a TV by its address, when the server doesn't find it by itself.
    pub fn cast_add(&self, host: &str) -> Result<CastDevice> {
        let v: Value = read(self.request("POST", "/api/cast/devices").send_json(json!({ "host": host }))?)?;
        serde_json::from_value(v["device"].clone()).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Play a song on a TV. The TV fetches it from the server itself (`origin` = the server's address).
    pub fn cast_play(&self, device: &str, track: &str, at: f64, autoplay: bool) -> Result<()> {
        let body = json!({ "device": device, "track": track, "at": at.max(0.0), "autoplay": autoplay, "origin": self.base() });
        self.request("POST", "/api/cast/play").send_json(body)?;
        Ok(())
    }

    /// `action`: play, pause, stop, seek (value = seconds), volume (value = 0..1).
    pub fn cast_control(&self, device: &str, action: &str, value: Option<f64>) -> Result<()> {
        let mut body = json!({ "device": device, "action": action });
        if let Some(v) = value {
            body["value"] = json!(v);
        }
        self.request("POST", "/api/cast/control").send_json(body)?;
        Ok(())
    }

    pub fn cast_status(&self, device: &str) -> Result<CastStatus> {
        let v: Value = self.get_json(&format!("/api/cast/status?device={}", enc(device)))?;
        Ok(serde_json::from_value(v["status"].clone()).unwrap_or_default())
    }

    /* ---------- listening parties ---------- */

    /// The party you're in, if any.
    pub fn party_current(&self) -> Result<Option<Party>> {
        let v: Value = self.get_json("/api/party")?;
        Ok(serde_json::from_value(v["party"].clone()).ok())
    }

    fn party_answer(res: ureq::Response) -> Result<Party> {
        let v: Value = read(res)?;
        serde_json::from_value(v["party"].clone()).map_err(|e| Error::BadAnswer(e.to_string()))
    }

    /// Start one: what's playing becomes its queue, and `invite` (server user ids) are asked to join.
    pub fn party_start(&self, invite: &[String], queue: &[String], index: usize, pos_ms: i64, playing: bool) -> Result<Party> {
        let queue: Vec<&String> = queue.iter().take(500).collect();
        let body = json!({ "invite": invite, "queue": queue, "index": index, "posMs": pos_ms, "playing": playing });
        Self::party_answer(self.request("POST", "/api/party").send_json(body)?)
    }

    /// Join after being invited. `output`: "own" (sound here) or "host" (the host's speaker).
    pub fn party_join(&self, id: &str, output: &str) -> Result<Party> {
        Self::party_answer(self.request("POST", &format!("/api/party/{}/join", enc(id))).send_json(json!({ "output": output }))?)
    }

    /// Leave; the host leaving ends it for everyone.
    pub fn party_leave(&self, id: &str) -> Result<()> {
        self.request("POST", &format!("/api/party/{}/leave", enc(id))).send_json(json!({}))?;
        Ok(())
    }

    pub fn party_invite(&self, id: &str, users: &[String]) -> Result<Party> {
        Self::party_answer(self.request("POST", &format!("/api/party/{}/invite", enc(id))).send_json(json!({ "userIds": users }))?)
    }
}
