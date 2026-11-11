//! A song on the server as a file you can read and seek: reads stream it, a seek far away opens a
//! new range request at that spot (so jumping to 3:00 doesn't download the first three minutes).
use std::io::{self, BufReader, Read, Seek, SeekFrom};

use crate::client::Client;

/// Forward seeks shorter than this just read ahead instead of opening a new request.
const SKIP_AHEAD: u64 = 256 * 1024;

pub struct HttpFile {
    client: Client,
    track_id: String,
    len: Option<u64>,
    /// where the next read comes from
    pos: u64,
    /// the open response, and the byte it's at
    body: Option<(BufReader<Box<dyn Read + Send + Sync>>, u64)>,
}

impl HttpFile {
    /// Opens the song (one request, which also tells us its size).
    pub fn open(client: &Client, track_id: &str) -> io::Result<HttpFile> {
        let mut f = HttpFile { client: client.clone(), track_id: track_id.to_string(), len: None, pos: 0, body: None };
        f.reopen(0)?;
        Ok(f)
    }

    pub fn len(&self) -> Option<u64> {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == Some(0)
    }

    fn reopen(&mut self, at: u64) -> io::Result<()> {
        let (reader, total, partial) = self.client.open_stream(&self.track_id, at).map_err(|e| io::Error::other(e.to_string()))?;
        if total.is_some() && (at == 0 || partial) {
            self.len = total;
        }
        let mut r = BufReader::with_capacity(64 * 1024, reader);
        let mut here = if partial { at } else { 0 };
        // a server that ignores ranges sends from the start: skip to where we want
        if here < at {
            io::copy(&mut (&mut r).take(at - here), &mut io::sink())?;
            here = at;
        }
        self.body = Some((r, here));
        Ok(())
    }
}

impl Read for HttpFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(len) = self.len {
            if self.pos >= len {
                return Ok(0);
            }
        }
        let need_open = match &self.body {
            Some((_, at)) => *at != self.pos,
            None => true,
        };
        if need_open {
            // a short hop forward: read through it on the open connection
            let skipped = match &mut self.body {
                Some((r, at)) if self.pos > *at && self.pos - *at <= SKIP_AHEAD => {
                    let gap = self.pos - *at;
                    let n = io::copy(&mut r.take(gap), &mut io::sink())?;
                    *at += n;
                    n == gap
                }
                _ => false,
            };
            if !skipped {
                self.reopen(self.pos)?;
            }
        }
        let (r, at) = self.body.as_mut().expect("opened above");
        let n = r.read(buf)?;
        *at += n as u64;
        self.pos += n as u64;
        Ok(n)
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
