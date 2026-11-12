//! Your devices: the places you're signed in right now, so music can move between them.
//! Each open copy of Volvix Home keeps a live connection and shows up here by name. Only one device plays at a time:
//! when one starts, the others pause, and "play on another device" hands over the song, the queue and the spot.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Value};
use tokio::sync::broadcast;

struct Dev {
    name: String,
    conns: u32, // open connections (a page reload briefly has two)
}

struct Mine {
    devices: HashMap<String, Dev>,
    active: Option<String>,
    /// what the playing device last said it is playing (queue, song, spot), so the others can show it and control it
    now: Option<Value>,
    tx: broadcast::Sender<String>,
}

#[derive(Default)]
pub struct Devices {
    inner: Mutex<HashMap<String, Mine>>,
}

fn snapshot(m: &Mine) -> String {
    let mut list: Vec<Value> = m.devices.iter().map(|(id, d)| json!({ "id": id, "name": d.name, "active": m.active.as_deref() == Some(id.as_str()) })).collect();
    list.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()).then(a["id"].as_str().cmp(&b["id"].as_str())));
    let now = m.now.as_ref().map(|n| { let mut n = n.clone(); n["serverNow"] = json!(crate::party::now_ms()); n });
    json!({ "type": "devices", "devices": list, "active": m.active, "now": now }).to_string()
}

fn clean(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(40).collect();
    if n.trim().is_empty() { "Unknown device".into() } else { n.trim().to_string() }
}

impl Devices {
    pub fn new() -> Devices {
        Devices::default()
    }

    /// A device opened its connection. Returns the stream of events and the current list.
    pub fn connect(&self, uid: &str, dev: &str, name: &str) -> (broadcast::Receiver<String>, String) {
        let mut all = self.inner.lock().unwrap();
        let m = all.entry(uid.to_string()).or_insert_with(|| Mine { devices: HashMap::new(), active: None, now: None, tx: broadcast::channel(32).0 });
        let d = m.devices.entry(dev.to_string()).or_insert(Dev { name: clean(name), conns: 0 });
        d.name = clean(name);
        d.conns += 1;
        let rx = m.tx.subscribe();
        let snap = snapshot(m);
        let _ = m.tx.send(snap.clone());
        (rx, snap)
    }

    pub fn disconnect(&self, uid: &str, dev: &str) {
        let mut all = self.inner.lock().unwrap();
        let Some(m) = all.get_mut(uid) else { return };
        let gone = match m.devices.get_mut(dev) { Some(d) => { d.conns = d.conns.saturating_sub(1); d.conns == 0 } None => false };
        if gone {
            m.devices.remove(dev);
            if m.active.as_deref() == Some(dev) { m.active = None; m.now = None; }
            let _ = m.tx.send(snapshot(m));
        }
        if m.devices.is_empty() { all.remove(uid); }
    }

    #[cfg(test)]
    fn snapshot_has_now(&self, uid: &str) -> bool {
        self.inner.lock().unwrap().get(uid).map(|m| m.now.is_some()).unwrap_or(false)
    }

    /// This device started playing: it's the one that plays now (the others hear about it and pause).
    pub fn set_active(&self, uid: &str, dev: &str) -> bool {
        let mut all = self.inner.lock().unwrap();
        let Some(m) = all.get_mut(uid) else { return false };
        if !m.devices.contains_key(dev) { return false; }
        if m.active.as_deref() != Some(dev) {
            m.active = Some(dev.to_string());
            m.now = None;
            let _ = m.tx.send(snapshot(m));
        }
        true
    }

    /// The playing device tells the others what it is playing. Only the active device may.
    pub fn report(&self, uid: &str, dev: &str, mut state: Value) -> bool {
        let mut all = self.inner.lock().unwrap();
        let Some(m) = all.get_mut(uid) else { return false };
        if m.active.as_deref() != Some(dev) {
            // a device that is playing right now is the one playing, even if its "I'm playing" message got lost
            let playing = state["playing"].as_bool().unwrap_or(false);
            if !(playing && m.devices.contains_key(dev)) { return false; }
            m.active = Some(dev.to_string());
            m.now = None;
            let _ = m.tx.send(snapshot(m));
        }
        let t = crate::party::now_ms();
        state["updatedAt"] = json!(t);
        state["serverNow"] = json!(t);
        m.now = Some(state.clone());
        let _ = m.tx.send(json!({ "type": "now", "device": dev, "state": state }).to_string());
        true
    }

    /// Ask a device to do something (play, pause, next, previous, seek, jump to a song in the queue).
    pub fn command(&self, uid: &str, to: &str, cmd: Value) -> Result<(), String> {
        let all = self.inner.lock().unwrap();
        let m = all.get(uid).ok_or("No devices are connected.")?;
        if !m.devices.contains_key(to) { return Err("That device isn't online any more.".into()); }
        let _ = m.tx.send(json!({ "type": "command", "to": to, "cmd": cmd }).to_string());
        Ok(())
    }

    /// Hand playback to another of your devices, with the song, queue and position.
    pub fn transfer(&self, uid: &str, from: &str, to: &str, state: Value) -> Result<(), String> {
        let mut all = self.inner.lock().unwrap();
        let m = all.get_mut(uid).ok_or("No devices are connected.")?;
        if !m.devices.contains_key(to) { return Err("That device isn't online any more.".into()); }
        m.active = Some(to.to_string());
        m.now = None;
        let _ = m.tx.send(json!({ "type": "transfer", "from": from, "to": to, "state": state }).to_string());
        let _ = m.tx.send(snapshot(m));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_come_and_go_and_hand_over() {
        let d = Devices::new();
        let (mut rx_a, _) = d.connect("u1", "pc", "Windows PC");
        let (_rx_b, snap) = d.connect("u1", "phone", "iPhone");
        assert!(snap.contains("iPhone") && snap.contains("Windows PC"));
        // someone else's devices are separate
        let (_rx_c, other) = d.connect("u2", "x", "Tablet");
        assert!(!other.contains("iPhone"));
        assert!(d.set_active("u1", "pc"));
        assert!(d.transfer("u1", "pc", "phone", json!({ "index": 2 })).is_ok());
        let mut saw_transfer = false;
        while let Ok(m) = rx_a.try_recv() { if m.contains("\"transfer\"") && m.contains("\"index\":2") { saw_transfer = true; } }
        assert!(saw_transfer);
        assert!(d.transfer("u1", "pc", "gone", json!({})).is_err());
        d.disconnect("u1", "phone");
        assert!(d.transfer("u1", "pc", "phone", json!({})).is_err());
        assert!(!d.set_active("u1", "phone"));
    }

    #[test]
    fn the_playing_device_reports_and_others_command_it() {
        let d = Devices::new();
        let (mut rx_a, _) = d.connect("u1", "pc", "PC");
        let (_rx_b, _) = d.connect("u1", "phone", "Phone");
        assert!(!d.report("u1", "pc", json!({ "index": 1 })), "only the active device reports");
        assert!(d.report("u1", "phone", json!({ "index": 0, "playing": true })), "a device that is playing takes over");
        assert!(d.snapshot_has_now("u1"));
        assert!(d.set_active("u1", "pc"));
        assert!(d.report("u1", "pc", json!({ "index": 1, "playing": true })));
        let (_rx_c, snap) = d.connect("u1", "tablet", "Tablet");
        assert!(snap.contains("\"now\"") && snap.contains("\"index\":1"));
        assert!(d.command("u1", "pc", json!({ "op": "pause" })).is_ok());
        let mut saw = false;
        while let Ok(m) = rx_a.try_recv() { if m.contains("\"command\"") && m.contains("pause") { saw = true; } }
        assert!(saw);
        assert!(d.command("u1", "gone", json!({})).is_err());
        assert!(d.set_active("u1", "phone"));
        assert!(!d.snapshot_has_now("u1"), "a new active device clears the old report");
    }
}
