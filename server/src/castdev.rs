//! Casting from the server to Chromecasts and other Google Cast TVs / speakers.
//!
//! Browsers only cast from secure (https) pages in Chrome, and the Volvix apps' built-in browsers can't cast at all.
//! So the server does it: it finds Cast devices on the network (mDNS, `_googlecast._tcp`), and talks to them directly
//! over the Cast protocol (an encrypted link on port 8009 carrying small JSON messages). It starts Google's standard
//! media player on the device and hands it a signed link to the song (see cast.rs), which the device fetches itself.
//! The app keeps the queue and the clock, as before: it tells the server what to play, and follows the device's
//! play / pause and position.
//!
//! In Docker, finding devices automatically needs `network_mode: host` (multicast doesn't cross Docker's own network).
//! Devices can also be added by their address.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

/// Google's standard media player app on every Cast device.
const MEDIA_APP: &str = "CC1AD845";
const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const SENDER: &str = "sender-volvix";
const RECEIVER: &str = "receiver-0";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub model: String,
    pub host: String,
    pub port: u16,
    /// added by its address (kept in cast_devices.json), not found automatically
    #[serde(default)]
    pub manual: bool,
}

/* ---------------- the Cast message format ----------------
 * Each message is a small protobuf ("CastMessage") behind a 4-byte big-endian length:
 *   1 protocol_version (varint, 0)   2 source_id (string)   3 destination_id (string)
 *   4 namespace (string)             5 payload_type (varint, 0 = text)   6 payload_utf8 (string)
 * Only text payloads are used, so this is written out by hand instead of pulling in a protobuf library. */

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 { out.push(b); break; }
        out.push(b | 0x80);
    }
}
fn put_str(out: &mut Vec<u8>, field: u8, s: &str) {
    out.push((field << 3) | 2);
    put_varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

pub fn encode(source: &str, dest: &str, namespace: &str, payload: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(payload.len() + 96);
    m.extend_from_slice(&[0x08, 0x00]); // protocol_version = CASTV2_1_0
    put_str(&mut m, 2, source);
    put_str(&mut m, 3, dest);
    put_str(&mut m, 4, namespace);
    m.extend_from_slice(&[0x28, 0x00]); // payload_type = STRING
    put_str(&mut m, 6, payload);
    let mut framed = (m.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(&m);
    framed
}

#[derive(Debug, PartialEq)]
pub struct Message {
    pub source: String,
    pub dest: String,
    pub namespace: String,
    pub payload: String,
}

/// One CastMessage (without its length prefix).
pub fn decode(b: &[u8]) -> Option<Message> {
    let (mut i, mut m) = (0usize, Message { source: String::new(), dest: String::new(), namespace: String::new(), payload: String::new() });
    let varint = |b: &[u8], i: &mut usize| -> Option<u64> {
        let (mut v, mut shift) = (0u64, 0);
        loop {
            let byte = *b.get(*i)?; *i += 1;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 { return Some(v); }
            shift += 7; if shift > 63 { return None; }
        }
    };
    while i < b.len() {
        let key = varint(b, &mut i)?;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => { varint(b, &mut i)?; }
            2 => {
                let len = varint(b, &mut i)? as usize;
                let bytes = b.get(i..i + len)?; i += len;
                let s = String::from_utf8_lossy(bytes).into_owned();
                match field { 2 => m.source = s, 3 => m.dest = s, 4 => m.namespace = s, 6 => m.payload = s, _ => {} }
            }
            _ => return None,
        }
    }
    Some(m)
}

/* ---------------- encrypted link (Cast devices use their own self-made certificates) ---------------- */

#[derive(Debug)]
struct AnyCert(Arc<rustls::crypto::CryptoProvider>);
impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>,
        _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &rustls::pki_types::CertificateDer<'_>, dss: &rustls::DigitallySignedStruct)
        -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &rustls::pki_types::CertificateDer<'_>, dss: &rustls::DigitallySignedStruct)
        -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions().expect("tls versions")
        .dangerous().with_custom_certificate_verifier(Arc::new(AnyCert(provider)))
        .with_no_client_auth();
    Arc::new(cfg)
}

/* ---------------- one open link to a device ---------------- */

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    /// IDLE, BUFFERING, PLAYING, PAUSED
    pub player_state: String,
    pub current_time: f64,
    /// why it went idle: FINISHED, CANCELLED, INTERRUPTED, ERROR
    pub idle_reason: Option<String>,
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    /// which song the device has (our track id)
    pub track: Option<String>,
    #[serde(skip)]
    pub at: Option<Instant>,
}

struct Link {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    status: Mutex<Status>,
    /// the media player app's transport id and session id once it's running
    app: Mutex<Option<(String, String)>>,
    media_session: Mutex<Option<i64>>,
}

impl Link {
    fn send(&self, dest: &str, ns: &str, payload: Value) {
        let _ = self.tx.send(encode(SENDER, dest, ns, &payload.to_string()));
    }
    /// Send a message with a requestId and wait for the answer that carries the same id.
    async fn ask(&self, dest: &str, ns: &str, mut payload: Value, wait: Duration) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        payload["requestId"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().unwrap().insert(id, tx);
        self.send(dest, ns, payload);
        match tokio::time::timeout(wait, rx).await {
            Ok(Ok(v)) => Ok(v),
            _ => { self.waiting.lock().unwrap().remove(&id); Err("The TV didn't answer.".into()) }
        }
    }
    fn alive(&self) -> bool { !self.tx.is_closed() }
}

fn apply_status(link: &Link, msg: &Value) {
    match msg["type"].as_str() {
        Some("RECEIVER_STATUS") => {
            let st = &msg["status"];
            let mut s = link.status.lock().unwrap();
            if let Some(v) = st["volume"]["level"].as_f64() { s.volume = Some(v); }
            if let Some(m) = st["volume"]["muted"].as_bool() { s.muted = Some(m); }
            let app = st["applications"].as_array().and_then(|a| a.iter().find(|x| x["appId"] == MEDIA_APP)).cloned();
            match app {
                Some(a) => {
                    if let (Some(t), Some(sid)) = (a["transportId"].as_str(), a["sessionId"].as_str()) {
                        *link.app.lock().unwrap() = Some((t.to_string(), sid.to_string()));
                    }
                }
                None => {
                    // someone else took over the TV (or it was stopped): our player isn't running any more
                    if link.app.lock().unwrap().is_some() && st["applications"].is_array() {
                        *link.app.lock().unwrap() = None;
                        s.player_state = "IDLE".into();
                        s.idle_reason = Some("CANCELLED".into());
                    }
                }
            }
        }
        Some("MEDIA_STATUS") => {
            if let Some(m) = msg["status"].as_array().and_then(|a| a.first()) {
                let mut s = link.status.lock().unwrap();
                if let Some(id) = m["mediaSessionId"].as_i64() { *link.media_session.lock().unwrap() = Some(id); }
                if let Some(p) = m["playerState"].as_str() { s.player_state = p.to_string(); }
                if let Some(t) = m["currentTime"].as_f64() { s.current_time = t; s.at = Some(Instant::now()); }
                s.idle_reason = m["idleReason"].as_str().map(String::from);
                if let Some(v) = m["volume"]["level"].as_f64() { s.volume = Some(v); }
            }
        }
        _ => {}
    }
}

async fn open(dev: &Device) -> Result<Arc<Link>, String> {
    let addr: SocketAddr = format!("{}:{}", dev.host, dev.port).parse().map_err(|_| "That TV's address isn't valid.".to_string())?;
    let tcp = tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(addr)).await
        .map_err(|_| format!("Couldn't reach {} (it may be off).", dev.name))?
        .map_err(|_| format!("Couldn't reach {} (it may be off).", dev.name))?;
    let name = rustls::pki_types::ServerName::IpAddress(addr.ip().into());
    let tls = tokio_rustls::TlsConnector::from(tls_config()).connect(name, tcp).await.map_err(|e| format!("Couldn't connect to {}: {e}", dev.name))?;
    let (mut rd, mut wr) = tokio::io::split(tls);
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let link = Arc::new(Link { tx, next: AtomicU64::new(1), waiting: Mutex::new(HashMap::new()), status: Mutex::new(Status { connected: true, ..Default::default() }),
        app: Mutex::new(None), media_session: Mutex::new(None) });

    // writer (an empty frame means: hang up)
    tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if frame.is_empty() { let _ = wr.shutdown().await; break; }
            if wr.write_all(&frame).await.is_err() { break; }
        }
    });
    // reader: answers pings, keeps the status, hands answers to whoever asked
    let l = link.clone();
    tokio::spawn(async move {
        let mut len = [0u8; 4];
        loop {
            if rd.read_exact(&mut len).await.is_err() { break; }
            let n = u32::from_be_bytes(len) as usize;
            if n > 1 << 20 { break; }
            let mut buf = vec![0u8; n];
            if rd.read_exact(&mut buf).await.is_err() { break; }
            let Some(m) = decode(&buf) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&m.payload) else { continue };
            if m.namespace == NS_HEARTBEAT && v["type"] == "PING" { l.send(&m.source, NS_HEARTBEAT, json!({ "type": "PONG" })); continue; }
            if m.namespace == NS_CONNECTION && v["type"] == "CLOSE" {
                if m.source == RECEIVER { break; }
                *l.app.lock().unwrap() = None; // the player app closed our connection to it
                continue;
            }
            apply_status(&l, &v);
            if let Some(id) = v["requestId"].as_u64().filter(|i| *i > 0) {
                if let Some(w) = l.waiting.lock().unwrap().remove(&id) { let _ = w.send(v.clone()); }
            }
        }
        l.status.lock().unwrap().connected = false;
        l.waiting.lock().unwrap().clear();
        let _ = l.tx.send(Vec::new()); // the device hung up: stop the writer too
    });
    // heartbeat: the device drops quiet connections
    let l = link.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if !l.status.lock().unwrap().connected { break; }
            l.send(RECEIVER, NS_HEARTBEAT, json!({ "type": "PING" }));
        }
    });

    link.send(RECEIVER, NS_CONNECTION, json!({ "type": "CONNECT" }));
    Ok(link)
}

/// Make sure Google's media player is running on the device and connected to us; returns its transport id + session id.
async fn ensure_player(link: &Link) -> Result<(String, String), String> {
    if let Some(a) = link.app.lock().unwrap().clone() { return Ok(a); }
    let st = link.ask(RECEIVER, NS_RECEIVER, json!({ "type": "GET_STATUS" }), Duration::from_secs(5)).await?;
    apply_status(link, &st);
    if link.app.lock().unwrap().is_none() {
        let r = link.ask(RECEIVER, NS_RECEIVER, json!({ "type": "LAUNCH", "appId": MEDIA_APP }), Duration::from_secs(20)).await?;
        if r["type"] == "LAUNCH_ERROR" { return Err(format!("The TV wouldn't start its player ({}).", r["reason"].as_str().unwrap_or("no reason given"))); }
        apply_status(link, &r);
        // some devices answer before the app is fully up: ask again for a moment
        for _ in 0..20 {
            if link.app.lock().unwrap().is_some() { break; }
            tokio::time::sleep(Duration::from_millis(300)).await;
            if let Ok(s) = link.ask(RECEIVER, NS_RECEIVER, json!({ "type": "GET_STATUS" }), Duration::from_secs(3)).await { apply_status(link, &s); }
        }
    }
    let (transport, session) = link.app.lock().unwrap().clone().ok_or("The TV's player didn't start.")?;
    link.send(&transport, NS_CONNECTION, json!({ "type": "CONNECT" }));
    Ok((transport, session))
}

/* ---------------- what the server offers ---------------- */

pub struct Media {
    pub url: String,
    pub content_type: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub image: Option<String>,
    pub duration: Option<f64>,
    pub track: String,
}

pub struct Caster {
    found: Arc<Mutex<HashMap<String, Device>>>,
    manual: Mutex<Vec<Device>>,
    file: PathBuf,
    links: tokio::sync::Mutex<HashMap<String, Arc<Link>>>,
}

impl Caster {
    pub fn new(data_dir: &Path) -> Caster {
        let file = data_dir.join("cast_devices.json");
        let manual: Vec<Device> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let c = Caster { found: Arc::new(Mutex::new(HashMap::new())), manual: Mutex::new(manual), file, links: tokio::sync::Mutex::new(HashMap::new()) };
        c.discover();
        c
    }

    /// Keep a list of the Cast devices that announce themselves on the network.
    fn discover(&self) {
        let found = self.found.clone();
        std::thread::spawn(move || {
            let Ok(daemon) = mdns_sd::ServiceDaemon::new() else { eprintln!("cast: can't look for TVs on this network"); return };
            let Ok(rx) = daemon.browse("_googlecast._tcp.local.") else { return };
            while let Ok(ev) = rx.recv() {
                match ev {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        let Some(ip) = info.get_addresses_v4().into_iter().next() else { continue };
                        let id = info.get_property_val_str("id").unwrap_or(info.get_fullname()).to_string();
                        let name = info.get_property_val_str("fn").unwrap_or("Cast device").to_string();
                        let model = info.get_property_val_str("md").unwrap_or("").to_string();
                        found.lock().unwrap().insert(id.clone(), Device { id, name, model, host: ip.to_string(), port: info.get_port(), manual: false });
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                        found.lock().unwrap().retain(|_, d| !fullname.contains(&d.id));
                    }
                    _ => {}
                }
            }
        });
    }

    pub fn devices(&self) -> Vec<Device> {
        let mut list: Vec<Device> = self.found.lock().unwrap().values().cloned().collect();
        for m in self.manual.lock().unwrap().iter() {
            if !list.iter().any(|d| d.host == m.host) { list.push(m.clone()); }
        }
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        list
    }

    pub fn device(&self, id: &str) -> Option<Device> {
        self.devices().into_iter().find(|d| d.id == id)
    }

    /// Add a device by its address: checks it really is a Cast device, and asks it for its name.
    pub async fn add(&self, host: &str) -> Result<Device, String> {
        let ip: IpAddr = host.trim().parse().map_err(|_| "Type the TV's address, like 192.168.1.40.".to_string())?;
        let mut dev = Device { id: format!("ip-{ip}"), name: format!("Cast device ({ip})"), model: String::new(), host: ip.to_string(), port: 8009, manual: true };
        let link = open(&dev).await?;
        let ok = link.ask(RECEIVER, NS_RECEIVER, json!({ "type": "GET_STATUS" }), Duration::from_secs(5)).await;
        let _ = link.tx.send(Vec::new());
        ok.map_err(|_| "That address didn't answer like a Cast device.".to_string())?;
        // its name, from the little info page Cast devices serve
        if let Ok(r) = reqwest::Client::new().get(format!("http://{ip}:8008/setup/eureka_info?params=name,device_info")).timeout(Duration::from_secs(3)).send().await {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(n) = v["name"].as_str().filter(|n| !n.is_empty()) { dev.name = n.to_string(); }
                if let Some(m) = v["device_info"]["model_name"].as_str() { dev.model = m.to_string(); }
            }
        }
        let mut list = self.manual.lock().unwrap();
        list.retain(|d| d.host != dev.host);
        list.push(dev.clone());
        let _ = std::fs::write(&self.file, serde_json::to_string_pretty(&*list).unwrap_or_default());
        Ok(dev)
    }

    pub fn remove(&self, id: &str) {
        let mut list = self.manual.lock().unwrap();
        list.retain(|d| d.id != id);
        let _ = std::fs::write(&self.file, serde_json::to_string_pretty(&*list).unwrap_or_default());
    }

    async fn link(&self, dev: &Device) -> Result<Arc<Link>, String> {
        let mut links = self.links.lock().await;
        if let Some(l) = links.get(&dev.id) { if l.alive() && l.status.lock().unwrap().connected { return Ok(l.clone()); } }
        let l = open(dev).await?;
        links.insert(dev.id.clone(), l.clone());
        Ok(l)
    }

    /// Play a song on the device, starting at `at` seconds.
    pub async fn play(&self, dev: &Device, m: Media, at: f64, autoplay: bool) -> Result<Status, String> {
        let link = self.link(dev).await?;
        let (transport, session) = ensure_player(&link).await?;
        let mut meta = json!({ "metadataType": 3, "title": m.title, "artist": m.artist, "albumName": m.album });
        if let Some(img) = &m.image { meta["images"] = json!([{ "url": img }]); }
        let mut media = json!({ "contentId": m.url, "contentUrl": m.url, "streamType": "BUFFERED", "contentType": m.content_type, "metadata": meta });
        if let Some(d) = m.duration { media["duration"] = json!(d); }
        let r = link.ask(&transport, NS_MEDIA, json!({ "type": "LOAD", "sessionId": session, "media": media, "autoplay": autoplay, "currentTime": at.max(0.0) }), Duration::from_secs(20)).await?;
        match r["type"].as_str() {
            Some("LOAD_FAILED") | Some("LOAD_CANCELLED") | Some("INVALID_REQUEST") => {
                return Err(format!("The TV couldn't play that song ({}).", r["reason"].as_str().or(r["detailedErrorCode"].as_str()).unwrap_or(r["type"].as_str().unwrap_or("?"))));
            }
            _ => {}
        }
        link.status.lock().unwrap().track = Some(m.track);
        Ok(self.status_of(&link))
    }

    /// play, pause, seek (value = seconds), volume (value = 0..1), stop
    pub async fn control(&self, dev: &Device, action: &str, value: Option<f64>) -> Result<Status, String> {
        let link = self.link(dev).await?;
        if action == "volume" {
            link.send(RECEIVER, NS_RECEIVER, json!({ "type": "SET_VOLUME", "requestId": link.next.fetch_add(1, Ordering::Relaxed), "volume": { "level": value.unwrap_or(0.5).clamp(0.0, 1.0) } }));
            return Ok(self.status_of(&link));
        }
        if action == "stop" {
            if let Some((_, session)) = link.app.lock().unwrap().clone() {
                link.send(RECEIVER, NS_RECEIVER, json!({ "type": "STOP", "sessionId": session, "requestId": link.next.fetch_add(1, Ordering::Relaxed) }));
            }
            *link.app.lock().unwrap() = None;
            let _ = link.tx.send(Vec::new());
            self.links.lock().await.remove(&dev.id);
            return Ok(Status::default());
        }
        let (transport, _) = link.app.lock().unwrap().clone().ok_or("Nothing is playing on that TV.")?;
        let msid = (*link.media_session.lock().unwrap()).ok_or("Nothing is playing on that TV.")?;
        let mut msg = match action {
            "play" => json!({ "type": "PLAY" }),
            "pause" => json!({ "type": "PAUSE" }),
            "seek" => json!({ "type": "SEEK", "currentTime": value.unwrap_or(0.0).max(0.0) }),
            _ => return Err("Unknown action.".into()),
        };
        msg["mediaSessionId"] = json!(msid);
        let r = link.ask(&transport, NS_MEDIA, msg, Duration::from_secs(8)).await?;
        apply_status(&link, &r);
        Ok(self.status_of(&link))
    }

    pub async fn status(&self, dev: &Device) -> Status {
        let links = self.links.lock().await;
        match links.get(&dev.id) { Some(l) => self.status_of(l), None => Status::default() }
    }

    fn status_of(&self, l: &Link) -> Status {
        let mut s = l.status.lock().unwrap().clone();
        s.connected = s.connected && l.alive();
        // the device reports its position now and then: add the time since, while it plays
        if s.player_state == "PLAYING" { if let Some(at) = s.at { s.current_time += at.elapsed().as_secs_f64(); } }
        s
    }
}

/// The address of this server as seen from a device on the network (the interface that routes to it).
pub fn local_ip_towards(host: &str) -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect(format!("{host}:8009")).ok()?;
    Some(s.local_addr().ok()?.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let framed = encode("sender-0", "receiver-0", NS_RECEIVER, r#"{"type":"GET_STATUS","requestId":1}"#);
        let len = u32::from_be_bytes(framed[0..4].try_into().unwrap()) as usize;
        assert_eq!(len, framed.len() - 4);
        let m = decode(&framed[4..]).unwrap();
        assert_eq!(m.source, "sender-0");
        assert_eq!(m.dest, "receiver-0");
        assert_eq!(m.namespace, NS_RECEIVER);
        assert!(m.payload.contains("GET_STATUS"));
    }

    #[test]
    fn long_payloads_use_multi_byte_lengths() {
        let big = "x".repeat(70_000);
        let framed = encode("a", "b", "c", &big);
        assert_eq!(decode(&framed[4..]).unwrap().payload.len(), 70_000);
    }

    #[test]
    fn status_messages_update_the_link_state() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let l = Link { tx, next: AtomicU64::new(1), waiting: Mutex::new(HashMap::new()), status: Mutex::new(Status::default()), app: Mutex::new(None), media_session: Mutex::new(None) };
        apply_status(&l, &json!({ "type": "RECEIVER_STATUS", "status": { "volume": { "level": 0.4 }, "applications": [{ "appId": MEDIA_APP, "transportId": "t-1", "sessionId": "s-1" }] } }));
        assert_eq!(l.app.lock().unwrap().clone(), Some(("t-1".into(), "s-1".into())));
        apply_status(&l, &json!({ "type": "MEDIA_STATUS", "status": [{ "mediaSessionId": 7, "playerState": "PLAYING", "currentTime": 12.5 }] }));
        assert_eq!(*l.media_session.lock().unwrap(), Some(7));
        assert_eq!(l.status.lock().unwrap().player_state, "PLAYING");
        apply_status(&l, &json!({ "type": "RECEIVER_STATUS", "status": { "applications": [] } }));
        assert!(l.app.lock().unwrap().is_none(), "another app took over");
    }
}
