//! Listening parties: one shared queue and playback position that everyone in the party follows.
//!
//! The server owns the truth. Every change (play, pause, skip, add a song…) is an action sent here;
//! the server applies it and tells everyone the new state, so people can never drift into different
//! queues. Position is stored as "at time T the song was at P and it's playing" and each device works out
//! where it is now, so nothing needs to be sent every second.
//!
//! Who plays the sound is each person's choice (`output`): "own" plays on their device, "host" keeps their
//! device silent because the music is coming out of the host's speaker (same room, same car).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::auth::random_hex;

pub type Fail = (u16, String);

const MAX_QUEUE: usize = 500;

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct Member {
    pub id: String,
    pub output: String,
}

pub struct Party {
    pub id: String,
    pub host: String,
    pub members: Vec<Member>,
    pub invited: Vec<String>,
    pub queue: Vec<String>,
    pub index: i64,
    pub playing: bool,
    /// where the current song was at `updated` (ms)
    pub pos_ms: i64,
    pub updated: i64,
    pub version: u64,
    tx: broadcast::Sender<String>,
}

fn e(code: u16, msg: &str) -> Fail {
    (code, msg.to_string())
}

impl Party {
    pub fn state(&self) -> Value {
        json!({
            "id": self.id, "host": self.host,
            "members": self.members.iter().map(|m| json!({ "id": m.id, "output": m.output })).collect::<Vec<_>>(),
            "invited": self.invited, "queue": self.queue, "index": self.index, "playing": self.playing,
            "posMs": self.pos_ms, "updatedAt": self.updated, "serverNow": now_ms(), "version": self.version
        })
    }

    fn is_member(&self, uid: &str) -> bool {
        self.members.iter().any(|m| m.id == uid)
    }

    pub fn current_pos(&self, now: i64) -> i64 {
        if self.playing { self.pos_ms + (now - self.updated) } else { self.pos_ms }
    }

    fn push(&self) {
        let _ = self.tx.send(json!({ "type": "state", "state": self.state() }).to_string());
    }

    /// Roll on to the next song(s) when the current one has finished. Returns true if anything changed.
    /// A song of unknown length (0) waits for a device to report that it ended.
    fn settle(&mut self, now: i64, dur_ms: &dyn Fn(&str) -> i64) -> bool {
        if !self.playing || self.index < 0 {
            return false;
        }
        let mut pos = self.pos_ms + (now - self.updated);
        let mut changed = false;
        loop {
            let Some(id) = self.queue.get(self.index as usize).cloned() else { self.playing = false; changed = true; break };
            let d = dur_ms(&id);
            if d <= 0 || pos < d + 800 {
                break;
            }
            if (self.index as usize) + 1 < self.queue.len() {
                pos -= d;
                self.index += 1;
            } else {
                self.playing = false;
                pos = d;
            }
            changed = true;
            if !self.playing { break; }
        }
        if changed {
            self.pos_ms = pos;
            self.updated = now;
            self.version += 1;
        }
        changed
    }

    fn start_next(&mut self, now: i64) {
        if (self.index + 1) < self.queue.len() as i64 {
            self.index += 1;
            self.pos_ms = 0;
            self.updated = now;
        } else {
            self.playing = false;
            self.pos_ms = 0;
            self.updated = now;
        }
    }
}

pub enum Left {
    /// the host left, so the party is over (these invites are now moot)
    Ended { id: String, invited: Vec<String> },
    Stayed,
}

pub struct Parties {
    inner: Mutex<HashMap<String, Party>>,
}

impl Parties {
    pub fn new() -> Parties {
        Parties { inner: Mutex::new(HashMap::new()) }
    }

    /// The party someone is in, if any.
    pub fn of(&self, uid: &str) -> Option<Value> {
        self.inner.lock().unwrap().values().find(|p| p.is_member(uid)).map(|p| p.state())
    }

    pub fn create(&self, host: &str, invite: &[String], queue: Vec<String>, index: i64, pos_ms: i64, playing: bool) -> Result<Value, Fail> {
        let mut map = self.inner.lock().unwrap();
        if map.values().any(|p| p.is_member(host)) {
            return Err(e(409, "You're already in a party. Leave it first."));
        }
        let queue: Vec<String> = queue.into_iter().take(MAX_QUEUE).collect();
        let index = if queue.is_empty() { -1 } else { index.clamp(0, queue.len() as i64 - 1) };
        let (tx, _) = broadcast::channel(32);
        let p = Party {
            id: format!("party{}", random_hex(4)), host: host.to_string(),
            members: vec![Member { id: host.to_string(), output: "own".into() }],
            invited: invite.iter().filter(|i| *i != host).cloned().collect(),
            playing: playing && index >= 0, pos_ms: if index >= 0 { pos_ms.max(0) } else { 0 }, updated: now_ms(),
            queue, index, version: 1, tx,
        };
        let s = p.state();
        map.insert(p.id.clone(), p);
        Ok(s)
    }

    /// Join (you must be invited). `output` is "own" or "host".
    pub fn join(&self, uid: &str, id: &str, output: &str) -> Result<Value, Fail> {
        let mut map = self.inner.lock().unwrap();
        if !map.contains_key(id) { return Err(e(404, "That party has ended.")); }
        if map.values().any(|p| p.id != id && p.is_member(uid)) {
            return Err(e(409, "You're in another party. Leave it first."));
        }
        let p = map.get_mut(id).unwrap();
        if !p.is_member(uid) && !p.invited.iter().any(|i| i == uid) {
            return Err(e(403, "You need an invite to join this party."));
        }
        let output = if uid == p.host || output != "host" { "own" } else { "host" };
        p.invited.retain(|i| i != uid);
        match p.members.iter_mut().find(|m| m.id == uid) {
            Some(m) => m.output = output.to_string(),
            None => p.members.push(Member { id: uid.to_string(), output: output.to_string() }),
        }
        p.version += 1;
        p.push();
        Ok(p.state())
    }

    /// Leave your party. The host leaving ends it for everyone.
    pub fn leave(&self, uid: &str) -> Option<Left> {
        let mut map = self.inner.lock().unwrap();
        let id = map.values().find(|p| p.is_member(uid))?.id.clone();
        if map[&id].host == uid {
            let p = map.remove(&id).unwrap();
            let _ = p.tx.send(json!({ "type": "ended" }).to_string());
            return Some(Left::Ended { id, invited: p.invited });
        }
        let p = map.get_mut(&id).unwrap();
        p.members.retain(|m| m.id != uid);
        p.version += 1;
        p.push();
        Some(Left::Stayed)
    }

    /// Anyone in the party can invite more people. Returns the new state and who was newly invited.
    pub fn invite(&self, uid: &str, id: &str, ids: &[String], exists: &dyn Fn(&str) -> bool) -> Result<(Value, Vec<String>), Fail> {
        let mut map = self.inner.lock().unwrap();
        let p = map.get_mut(id).ok_or_else(|| e(404, "That party has ended."))?;
        if !p.is_member(uid) { return Err(e(403, "You're not in this party.")); }
        let mut fresh = Vec::new();
        for u in ids {
            if exists(u) && !p.is_member(u) && !p.invited.iter().any(|i| i == u) {
                p.invited.push(u.clone());
                fresh.push(u.clone());
            }
        }
        if !fresh.is_empty() { p.version += 1; p.push(); }
        Ok((p.state(), fresh))
    }

    pub fn set_output(&self, uid: &str, id: &str, output: &str) -> Result<Value, Fail> {
        let mut map = self.inner.lock().unwrap();
        let p = map.get_mut(id).ok_or_else(|| e(404, "That party has ended."))?;
        let host = p.host.clone();
        let m = p.members.iter_mut().find(|m| m.id == uid).ok_or_else(|| e(403, "You're not in this party."))?;
        m.output = if uid == host || output != "host" { "own".into() } else { "host".into() };
        p.version += 1;
        p.push();
        Ok(p.state())
    }

    /// Listen to a party's updates (members only). Gives the receiver and the current state.
    pub fn subscribe(&self, uid: &str, id: &str) -> Result<(broadcast::Receiver<String>, Value), Fail> {
        let map = self.inner.lock().unwrap();
        let p = map.get(id).ok_or_else(|| e(404, "That party has ended."))?;
        if !p.is_member(uid) { return Err(e(403, "You're not in this party.")); }
        Ok((p.tx.subscribe(), p.state()))
    }

    /// Apply one action from a member and tell everyone.
    pub fn action(&self, uid: &str, id: &str, body: &Value, known_track: &dyn Fn(&str) -> bool, dur_ms: &dyn Fn(&str) -> i64) -> Result<Value, Fail> {
        let now = now_ms();
        let mut map = self.inner.lock().unwrap();
        let p = map.get_mut(id).ok_or_else(|| e(404, "That party has ended."))?;
        if !p.is_member(uid) { return Err(e(403, "You're not in this party.")); }
        p.settle(now, dur_ms);
        let ids = |key: &str| -> Vec<String> {
            body[key].as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_str()).filter(|t| known_track(t)).map(String::from).collect()
        };
        let len = p.queue.len() as i64;
        match body["op"].as_str().unwrap_or("") {
            "play" => {
                if p.index < 0 && len > 0 { p.index = 0; p.pos_ms = 0; }
                if !p.playing && p.index >= 0 { p.playing = true; p.updated = now; }
            }
            "pause" => {
                if p.playing { p.pos_ms += now - p.updated; p.updated = now; p.playing = false; }
            }
            "seek" => {
                let ms = body["posMs"].as_i64().ok_or_else(|| e(400, "Missing position."))?;
                p.pos_ms = ms.max(0); p.updated = now;
            }
            "next" => p.start_next(now),
            "prev" => {
                if p.current_pos(now) > 3000 || p.index <= 0 { p.pos_ms = 0; p.updated = now; }
                else { p.index -= 1; p.pos_ms = 0; p.updated = now; }
            }
            "playAt" => {
                let i = body["index"].as_i64().ok_or_else(|| e(400, "Missing position."))?;
                if i < 0 || i >= len { return Err(e(400, "No such song in the queue.")); }
                p.index = i; p.pos_ms = 0; p.updated = now; p.playing = true;
            }
            // a device reports that its song ended (for songs the server doesn't know the length of)
            "ended" => {
                let cur = p.queue.get(p.index.max(0) as usize).cloned().unwrap_or_default();
                let d = dur_ms(&cur);
                if body["index"].as_i64() == Some(p.index) && p.playing && (d <= 0 || p.current_pos(now) >= d - 5000) {
                    p.start_next(now);
                }
            }
            "add" => {
                let add = ids("ids");
                if add.is_empty() { return Err(e(400, "Nothing to add.")); }
                let room = MAX_QUEUE.saturating_sub(p.queue.len());
                p.queue.extend(add.into_iter().take(room));
                if p.index < 0 && !p.queue.is_empty() { p.index = 0; p.pos_ms = 0; p.updated = now; }
            }
            "addNext" => {
                let add = ids("ids");
                if add.is_empty() { return Err(e(400, "Nothing to add.")); }
                let at = if p.index < 0 { 0 } else { (p.index + 1) as usize };
                for (k, t) in add.into_iter().enumerate() {
                    if p.queue.len() >= MAX_QUEUE { break; }
                    p.queue.insert(at + k, t);
                }
                if p.index < 0 && !p.queue.is_empty() { p.index = 0; p.pos_ms = 0; p.updated = now; }
            }
            "remove" => {
                let i = body["index"].as_i64().ok_or_else(|| e(400, "Missing position."))?;
                if i < 0 || i >= len { return Err(e(400, "No such song in the queue.")); }
                p.queue.remove(i as usize);
                if i < p.index { p.index -= 1; }
                else if i == p.index {
                    // the playing song was removed: the next one takes its place, from the start
                    p.pos_ms = 0; p.updated = now;
                    // it was the last song: nothing follows it, so stop on the one before
                    if p.index >= p.queue.len() as i64 { p.index = p.queue.len() as i64 - 1; p.playing = false; }
                }
                if p.queue.is_empty() { p.index = -1; p.playing = false; p.pos_ms = 0; }
            }
            "move" => {
                let (from, to) = (body["from"].as_i64().unwrap_or(-1), body["to"].as_i64().unwrap_or(-1));
                if from < 0 || to < 0 || from >= len || to >= len { return Err(e(400, "Bad position.")); }
                let item = p.queue.remove(from as usize);
                p.queue.insert(to as usize, item);
                // keep following the song that's playing
                if from == p.index { p.index = to; }
                else if from < p.index && to >= p.index { p.index -= 1; }
                else if from > p.index && to <= p.index { p.index += 1; }
            }
            // replace the whole queue (the host playing an album or playlist)
            "setQueue" => {
                if p.host != uid { return Err(e(403, "Only the host can replace the queue. Add songs instead.")); }
                let q: Vec<String> = ids("ids").into_iter().take(MAX_QUEUE).collect();
                if q.is_empty() { return Err(e(400, "Nothing to play.")); }
                p.index = body["start"].as_i64().unwrap_or(0).clamp(0, q.len() as i64 - 1);
                p.queue = q; p.pos_ms = 0; p.updated = now;
                p.playing = body["playing"].as_bool().unwrap_or(true);
            }
            _ => return Err(e(400, "Unknown action.")),
        }
        p.version += 1;
        p.push();
        Ok(p.state())
    }

    /// Once a second: move parties on to the next song when one has finished. Every few seconds
    /// (`beat`) also resend the state, so a device that drifted or missed a message catches up.
    pub fn tick(&self, beat: bool, dur_ms: &dyn Fn(&str) -> i64) {
        let now = now_ms();
        let mut map = self.inner.lock().unwrap();
        for p in map.values_mut() {
            let changed = p.settle(now, dur_ms);
            if changed || beat { p.push(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dur(id: &str) -> i64 { if id.starts_with('x') { 0 } else { 10_000 } } // "x…" songs have unknown length
    fn known(id: &str) -> bool { id.starts_with('t') || id.starts_with('x') }
    fn all(_: &str) -> bool { true }
    fn act(p: &Parties, uid: &str, id: &str, body: Value) -> Result<Value, Fail> { p.action(uid, id, &body, &known, &dur) }
    fn queue(s: &Value) -> Vec<String> { s["queue"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect() }
    fn state_of(p: &Parties, id: &str) -> Value { p.inner.lock().unwrap().get(id).map(|x| x.state()).unwrap() }
    fn start(p: &Parties) -> String {
        let s = p.create("a", &["b".into(), "c".into()], vec!["t1".into(), "t2".into(), "t3".into()], 0, 0, true).unwrap();
        s["id"].as_str().unwrap().to_string()
    }

    #[test]
    fn invites_joining_and_leaving() {
        let p = Parties::new();
        let id = start(&p);
        assert_eq!(p.of("a").unwrap()["host"], "a");
        assert!(p.of("b").is_none(), "invited isn't joined");
        assert_eq!(p.join("z", &id, "own").err().unwrap().0, 403, "no invite, no entry");
        let s = p.join("b", &id, "host").unwrap();
        assert_eq!(s["members"].as_array().unwrap().len(), 2);
        assert_eq!(s["members"][1]["output"], "host");
        assert_eq!(s["invited"].as_array().unwrap().len(), 1);
        // the host's own speaker is always their device
        p.set_output("a", &id, "host").unwrap();
        assert_eq!(p.of("a").unwrap()["members"][0]["output"], "own");
        // switch output
        assert_eq!(p.set_output("b", &id, "own").unwrap()["members"][1]["output"], "own");
        // one party at a time
        assert_eq!(p.create("b", &[], vec![], 0, 0, false).err().unwrap().0, 409);
        // anyone can invite, nobody outside can
        let (_, fresh) = p.invite("b", &id, &["d".into(), "b".into(), "c".into()], &all).unwrap();
        assert_eq!(fresh, vec!["d"], "already-invited and already-joined people are skipped");
        assert_eq!(p.invite("z", &id, &["q".into()], &all).err().unwrap().0, 403);
        // a guest leaving keeps the party; the host leaving ends it
        assert!(matches!(p.leave("b"), Some(Left::Stayed)));
        assert!(p.of("b").is_none());
        match p.leave("a") { Some(Left::Ended { invited, .. }) => assert_eq!(invited.len(), 2), _ => panic!("host leaving should end it") }
        assert_eq!(p.join("c", &id, "own").err().unwrap().0, 404, "nothing left to join");
    }

    #[test]
    fn playback_controls_follow_the_clock() {
        let p = Parties::new();
        let id = start(&p);
        p.join("b", &id, "own").unwrap();
        let s = act(&p, "b", &id, json!({ "op": "pause" })).unwrap();
        assert_eq!(s["playing"], false);
        assert!(s["posMs"].as_i64().unwrap() < 500);
        assert_eq!(act(&p, "a", &id, json!({ "op": "seek", "posMs": 4000 })).unwrap()["posMs"], 4000);
        let s = act(&p, "b", &id, json!({ "op": "play" })).unwrap();
        assert_eq!(s["playing"], true);
        // next / prev: prev restarts a song that has been playing a while, else goes back
        assert_eq!(act(&p, "a", &id, json!({ "op": "next" })).unwrap()["index"], 1);
        act(&p, "a", &id, json!({ "op": "seek", "posMs": 5000 })).unwrap();
        assert_eq!(act(&p, "a", &id, json!({ "op": "prev" })).unwrap()["index"], 1, "past 3 s: restart");
        assert_eq!(act(&p, "a", &id, json!({ "op": "prev" })).unwrap()["index"], 0, "near the start: previous song");
        assert_eq!(act(&p, "a", &id, json!({ "op": "playAt", "index": 2 })).unwrap()["index"], 2);
        assert_eq!(act(&p, "a", &id, json!({ "op": "playAt", "index": 9 })).err().unwrap().0, 400);
        // outsiders can't touch it
        assert_eq!(act(&p, "z", &id, json!({ "op": "pause" })).err().unwrap().0, 403);
        // the end of the queue stops playback instead of looping
        let s = act(&p, "a", &id, json!({ "op": "next" })).unwrap();
        assert_eq!((s["playing"].clone(), s["index"].clone()), (json!(false), json!(2)));
    }

    #[test]
    fn queue_edits_keep_the_playing_song() {
        let p = Parties::new();
        let id = start(&p);
        act(&p, "a", &id, json!({ "op": "playAt", "index": 1 })).unwrap(); // t2 playing
        let s = act(&p, "a", &id, json!({ "op": "add", "ids": ["t4", "nope", "t5"] })).unwrap();
        assert_eq!(queue(&s), vec!["t1", "t2", "t3", "t4", "t5"], "unknown songs are ignored");
        let s = act(&p, "a", &id, json!({ "op": "addNext", "ids": ["t9"] })).unwrap();
        assert_eq!(queue(&s)[2], "t9");
        // remove one before the playing song: the index shifts, the song keeps playing
        let s = act(&p, "a", &id, json!({ "op": "remove", "index": 0 })).unwrap();
        assert_eq!(s["index"], 0);
        assert_eq!(queue(&s)[0], "t2");
        // move the playing song, and move others around it
        let s = act(&p, "a", &id, json!({ "op": "move", "from": 0, "to": 3 })).unwrap();
        assert_eq!(s["index"], 3);
        assert_eq!(queue(&s)[3], "t2");
        let s = act(&p, "a", &id, json!({ "op": "move", "from": 4, "to": 0 })).unwrap();
        assert_eq!(s["index"], 4, "a song moved from after to before pushes the playing one down");
        assert_eq!(queue(&s)[4], "t2");
        assert_eq!(act(&p, "a", &id, json!({ "op": "move", "from": 0, "to": 99 })).err().unwrap().0, 400);
        // removing the playing song starts the next one from the top
        act(&p, "a", &id, json!({ "op": "seek", "posMs": 6000 })).unwrap();
        let s = act(&p, "a", &id, json!({ "op": "remove", "index": 4 })).unwrap();
        assert_eq!(s["posMs"], 0);
        // only the host replaces the queue; guests add
        p.join("b", &id, "own").unwrap();
        assert_eq!(act(&p, "b", &id, json!({ "op": "setQueue", "ids": ["t1"] })).err().unwrap().0, 403);
        let s = act(&p, "a", &id, json!({ "op": "setQueue", "ids": ["t7", "t8"], "start": 1, "playing": true })).unwrap();
        assert_eq!((queue(&s), s["index"].clone(), s["playing"].clone()), (vec!["t7".to_string(), "t8".to_string()], json!(1), json!(true)));
        // emptying the queue
        act(&p, "a", &id, json!({ "op": "remove", "index": 0 })).unwrap();
        let s = act(&p, "a", &id, json!({ "op": "remove", "index": 0 })).unwrap();
        assert_eq!((s["index"].clone(), s["playing"].clone()), (json!(-1), json!(false)));
    }

    #[test]
    fn songs_advance_by_themselves_and_end_reports_dont_double_skip() {
        let p = Parties::new();
        let id = start(&p);
        // pretend 25 s have passed: 10 s songs, so the party should be 5 s into the third
        {
            let mut map = p.inner.lock().unwrap();
            let party = map.get_mut(&id).unwrap();
            party.updated = now_ms() - 25_000;
        }
        p.tick(false, &dur);
        let s = state_of(&p, &id);
        assert_eq!(s["index"], 2);
        assert!(s["posMs"].as_i64().unwrap() >= 4_900 && s["posMs"].as_i64().unwrap() < 7_000, "{}", s["posMs"]);
        // far past the end of the last song: stops
        {
            let mut map = p.inner.lock().unwrap();
            map.get_mut(&id).unwrap().updated = now_ms() - 60_000;
        }
        p.tick(false, &dur);
        let s = state_of(&p, &id);
        assert_eq!((s["playing"].clone(), s["index"].clone()), (json!(false), json!(2)));
        // unknown-length songs: the first device to say it ended moves on; a repeat of the report doesn't skip again
        let q = p.create("h", &[], vec!["x1".into(), "x2".into(), "x3".into()], 0, 0, true).unwrap();
        let qid = q["id"].as_str().unwrap().to_string();
        assert_eq!(act(&p, "h", &qid, json!({ "op": "ended", "index": 0 })).unwrap()["index"], 1);
        assert_eq!(act(&p, "h", &qid, json!({ "op": "ended", "index": 0 })).unwrap()["index"], 1, "stale report ignored");
        // a song with a known length can't be skipped by an early (wrong) report
        let r = p.create("k", &[], vec!["t1".into(), "t2".into()], 0, 0, true).unwrap();
        let rid = r["id"].as_str().unwrap().to_string();
        assert_eq!(act(&p, "k", &rid, json!({ "op": "ended", "index": 0 })).unwrap()["index"], 0);
    }
}
