//! Sound. One audio thread owns the output device (rodio on cpal, decoding with symphonia); the UI
//! sends it commands and reads a shared status. Songs stream from the server through
//! `volvix_core::stream::HttpFile`, so playback starts right away and seeking opens a range request.
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink, Source};
use volvix_core::stream::HttpFile;
use volvix_core::Client;

/// What the UI reads to draw the player.
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// the song loaded (playing or paused)
    pub track: Option<String>,
    pub playing: bool,
    /// still opening the song on the server
    pub loading: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    /// bumped each time a song plays to its end (the UI moves the queue on)
    pub ended: u64,
    /// the loaded song has played to its end (Play starts it again)
    pub finished: bool,
    pub error: Option<String>,
}

enum Cmd {
    Load { client: Client, track: String, play: bool, gen: u64 },
    Loaded { gen: u64, track: String, result: Result<(Decoder<HttpFile>, Option<Duration>), String>, play: bool },
    Play,
    Pause,
    Seek(Duration),
    Volume(f32),
    Stop,
}

#[derive(Clone)]
pub struct Player {
    tx: Sender<Cmd>,
    status: Arc<Mutex<Status>>,
    gen: Arc<Mutex<u64>>,
}

impl Player {
    pub fn start() -> Player {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let s = status.clone();
        let back = tx.clone();
        std::thread::Builder::new().name("audio".into()).spawn(move || audio_thread(rx, back, s)).expect("audio thread");
        Player { tx, status, gen: Arc::new(Mutex::new(0)) }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Open a song and (if `play`) start it. A newer load cancels an older one still opening.
    pub fn load(&self, client: &Client, track: &str, play: bool) {
        let gen = {
            let mut g = self.gen.lock().unwrap();
            *g += 1;
            *g
        };
        let _ = self.tx.send(Cmd::Load { client: client.clone(), track: track.to_string(), play, gen });
    }
    pub fn play(&self) {
        let _ = self.tx.send(Cmd::Play);
    }
    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }
    pub fn seek(&self, to: Duration) {
        let _ = self.tx.send(Cmd::Seek(to));
    }
    pub fn set_volume(&self, v: f32) {
        let _ = self.tx.send(Cmd::Volume(v));
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

fn open(client: &Client, track: &str) -> Result<(Decoder<HttpFile>, Option<Duration>), String> {
    let file = HttpFile::open(client, track).map_err(|e| e.to_string())?;
    let dec = Decoder::new(file).map_err(|e| format!("Can't play this song ({e})."))?;
    let total = dec.total_duration();
    Ok((dec, total))
}

fn audio_thread(rx: Receiver<Cmd>, back: Sender<Cmd>, status: Arc<Mutex<Status>>) {
    // the output device; without one (no speakers), every command just reports the error
    let out = OutputStream::try_default();
    let (_stream, handle) = match out {
        Ok(x) => x,
        Err(e) => {
            status.lock().unwrap().error = Some(format!("No sound output ({e})."));
            for _ in rx {}
            return;
        }
    };
    let mut sink: Option<Sink> = None;
    let mut volume = 1.0f32;
    let mut latest_gen = 0u64;
    let mut was_busy = false;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Cmd::Load { client, track, play, gen }) => {
                latest_gen = gen;
                if let Some(s) = sink.take() {
                    s.stop();
                }
                was_busy = false;
                {
                    let mut st = status.lock().unwrap();
                    st.track = Some(track.clone());
                    st.loading = true;
                    st.playing = play;
                    st.position = Duration::ZERO;
                    st.duration = None;
                    st.error = None;
                    st.finished = false;
                }
                // opening talks to the server: do it off this thread so pause/volume stay instant
                let back = back.clone();
                std::thread::spawn(move || {
                    let result = open(&client, &track);
                    let _ = back.send(Cmd::Loaded { gen, track, result, play });
                });
            }
            Ok(Cmd::Loaded { gen, track, result, play }) => {
                if gen != latest_gen {
                    continue; // another song was chosen meanwhile
                }
                let mut st = status.lock().unwrap();
                st.loading = false;
                match result.and_then(|(dec, total)| Sink::try_new(&handle).map(|s| (s, dec, total)).map_err(|e| e.to_string())) {
                    Ok((s, dec, total)) => {
                        s.set_volume(volume);
                        s.append(dec);
                        if play {
                            s.play();
                        } else {
                            s.pause();
                        }
                        st.track = Some(track);
                        st.duration = total;
                        st.playing = play;
                        sink = Some(s);
                        was_busy = true;
                    }
                    Err(e) => {
                        st.error = Some(e);
                        st.playing = false;
                    }
                }
            }
            Ok(Cmd::Play) => {
                if let Some(s) = &sink {
                    s.play();
                    status.lock().unwrap().playing = true;
                }
            }
            Ok(Cmd::Pause) => {
                if let Some(s) = &sink {
                    s.pause();
                }
                status.lock().unwrap().playing = false;
            }
            Ok(Cmd::Seek(to)) => {
                if let Some(s) = &sink {
                    let _ = s.try_seek(to);
                    status.lock().unwrap().position = to;
                }
            }
            Ok(Cmd::Volume(v)) => {
                volume = v.clamp(0.0, 1.0);
                if let Some(s) = &sink {
                    s.set_volume(volume);
                }
            }
            Ok(Cmd::Stop) => {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                was_busy = false;
                let mut st = status.lock().unwrap();
                st.playing = false;
                st.position = Duration::ZERO;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // keep the status fresh, and notice when a song has played to the end
        if let Some(s) = &sink {
            let mut st = status.lock().unwrap();
            st.position = s.get_pos();
            if was_busy && s.empty() {
                was_busy = false;
                st.playing = false;
                st.finished = true;
                st.ended += 1;
            }
        }
    }
}
