//! A song on the server as a file you can read and seek. A background thread downloads ahead of what is being
//! played (so a slow moment on the network, or a busy server, never starves the sound) and carries on after a
//! dropped connection; a seek far away starts a new range request at that spot, so jumping to 3:00 doesn't
//! download the first three minutes.
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::client::Client;

/// How far ahead of the playing position the download runs.
const AHEAD: u64 = 16 * 1024 * 1024;
/// Played bytes are let go of in chunks, keeping a little behind for small seeks back.
const TRIM_AT: usize = 6 * 1024 * 1024;
const TRIM_BY: usize = 4 * 1024 * 1024;
/// A seek forward by less than this just waits for the download to get there.
const NEAR: u64 = 1024 * 1024;
/// A read waits this long for data before giving up.
const STALL: Duration = Duration::from_secs(30);
/// Reconnect attempts after a dropped connection (each a little later than the last).
const RETRIES: u32 = 6;

struct State {
    /// downloaded bytes, from file offset `start`
    buf: Vec<u8>,
    start: u64,
    done: bool,
    error: Option<String>,
    /// each seek to somewhere not downloaded starts a new generation; older downloads stop
    gen: u64,
    /// where the player is reading (the download keeps `AHEAD` bytes in front of it)
    pos: u64,
}

struct Shared {
    st: Mutex<State>,
    cv: Condvar,
}

pub struct HttpFile {
    client: Client,
    track_id: String,
    len: Option<u64>,
    pos: u64,
    shared: Arc<Shared>,
}

type Body = Box<dyn Read + Send + Sync>;

impl HttpFile {
    /// Opens the song (one request, which also tells us its size) and starts downloading it.
    pub fn open(client: &Client, track_id: &str) -> io::Result<HttpFile> {
        let (reader, total, _) = client.open_stream(track_id, 0).map_err(|e| io::Error::other(e.to_string()))?;
        let shared = Arc::new(Shared { st: Mutex::new(State { buf: Vec::new(), start: 0, done: false, error: None, gen: 0, pos: 0 }), cv: Condvar::new() });
        spawn(shared.clone(), client.clone(), track_id.to_string(), 0, 0, Some(reader));
        Ok(HttpFile { client: client.clone(), track_id: track_id.to_string(), len: total, pos: 0, shared })
    }

    pub fn len(&self) -> Option<u64> {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == Some(0)
    }
}

fn spawn(shared: Arc<Shared>, client: Client, track: String, from: u64, gen: u64, first: Option<Body>) {
    std::thread::Builder::new()
        .name("song-download".into())
        .spawn(move || download(shared, client, track, from, gen, first))
        .ok();
}

fn download(shared: Arc<Shared>, client: Client, track: String, from: u64, gen: u64, first: Option<Body>) {
    let mut at = from;
    let mut reader = first;
    let mut tries = 0u32;
    let fail = |msg: String| {
        let mut st = shared.st.lock().unwrap();
        if st.gen == gen {
            st.error = Some(msg);
            shared.cv.notify_all();
        }
    };
    loop {
        let mut r = match reader.take() {
            Some(r) => r,
            None => match client.open_stream(&track, at) {
                Ok((mut r, _, partial)) => {
                    // a server that ignores ranges sends from the start: skip to where we want
                    if !partial && at > 0 && io::copy(&mut (&mut r).take(at), &mut io::sink()).is_err() {
                        tries += 1;
                        if tries > RETRIES {
                            return fail("The connection to the server dropped.".into());
                        }
                        std::thread::sleep(Duration::from_millis(300 * tries as u64));
                        continue;
                    }
                    r
                }
                Err(e) => {
                    tries += 1;
                    if tries > RETRIES {
                        return fail(e.to_string());
                    }
                    std::thread::sleep(Duration::from_millis(300 * tries as u64));
                    continue;
                }
            },
        };
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            // don't run further ahead than needed
            {
                let mut st = shared.st.lock().unwrap();
                loop {
                    if st.gen != gen {
                        return;
                    }
                    let end = st.start + st.buf.len() as u64;
                    if end.saturating_sub(st.pos) < AHEAD {
                        break;
                    }
                    st = shared.cv.wait_timeout(st, Duration::from_millis(250)).unwrap().0;
                }
            }
            match r.read(&mut chunk) {
                Ok(0) => {
                    let mut st = shared.st.lock().unwrap();
                    if st.gen == gen {
                        st.done = true;
                        shared.cv.notify_all();
                    }
                    return;
                }
                Ok(n) => {
                    let mut st = shared.st.lock().unwrap();
                    if st.gen != gen {
                        return;
                    }
                    st.buf.extend_from_slice(&chunk[..n]);
                    at += n as u64;
                    tries = 0;
                    shared.cv.notify_all();
                }
                Err(_) => {
                    // the connection dropped: carry on from here on a new one
                    tries += 1;
                    if tries > RETRIES {
                        return fail("The connection to the server dropped.".into());
                    }
                    std::thread::sleep(Duration::from_millis(300 * tries as u64));
                    break;
                }
            }
        }
    }
}

impl Read for HttpFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        if let Some(len) = self.len {
            if self.pos >= len {
                return Ok(0);
            }
        }
        let waited = Instant::now();
        let mut st = self.shared.st.lock().unwrap();
        st.pos = self.pos;
        loop {
            let end = st.start + st.buf.len() as u64;
            if self.pos >= st.start && self.pos < end {
                let from = (self.pos - st.start) as usize;
                let n = out.len().min(st.buf.len() - from);
                out[..n].copy_from_slice(&st.buf[from..from + n]);
                self.pos += n as u64;
                st.pos = self.pos;
                if (self.pos - st.start) as usize > TRIM_AT {
                    st.buf.drain(..TRIM_BY);
                    st.start += TRIM_BY as u64;
                }
                self.shared.cv.notify_all();
                return Ok(n);
            }
            if self.pos >= end && st.done {
                return Ok(0);
            }
            if self.pos >= end && self.pos - end <= NEAR && st.error.is_none() {
                if waited.elapsed() > STALL {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "the server is not sending"));
                }
                st = self.shared.cv.wait_timeout(st, Duration::from_millis(100)).unwrap().0;
                continue;
            }
            if let Some(e) = st.error.clone() {
                if self.pos >= st.start && self.pos >= end {
                    return Err(io::Error::other(e));
                }
            }
            // somewhere that isn't downloaded (a seek back past what was kept, or far ahead): start there
            st.gen += 1;
            let gen = st.gen;
            st.buf.clear();
            st.start = self.pos;
            st.pos = self.pos;
            st.done = false;
            st.error = None;
            spawn(self.shared.clone(), self.client.clone(), self.track_id.clone(), self.pos, gen, None);
            self.shared.cv.notify_all();
        }
    }
}

impl Seek for HttpFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let new = match to {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(d) => match self.len {
                Some(len) => len as i128 + d as i128,
                None => return Err(io::Error::new(io::ErrorKind::Unsupported, "size unknown")),
            },
        };
        if new < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = new as u64;
        Ok(self.pos)
    }
}

impl Drop for HttpFile {
    fn drop(&mut self) {
        // stop the download
        if let Ok(mut st) = self.shared.st.lock() {
            st.gen += 1;
        }
        self.shared.cv.notify_all();
    }
}
