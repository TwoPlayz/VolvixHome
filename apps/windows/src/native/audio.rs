//! Sound. One audio thread owns the output device (rodio on cpal, decoding with symphonia); the UI
//! sends it commands and reads a shared status. Songs stream from the server through
//! `volvix_core::stream::HttpFile` (playback starts right away, seeking opens a range request), or
//! play from disk when they've been downloaded.
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, Sink, Source};
use volvix_core::stream::HttpFile;
use volvix_core::Client;

/// What the UI reads to draw the player.
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// the song loaded (playing or paused)
    pub track: Option<String>,
    pub playing: bool,
    /// still opening the song
    pub loading: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    /// bumped each time a song plays to its end (the UI moves the queue on)
    pub ended: u64,
    /// the loaded song has played to its end (Play starts it again)
    pub finished: bool,
    /// bumped each time the next song took over by itself while this one faded out (crossfade)
    pub advanced: u64,
    pub error: Option<String>,
}

/// Where a song's bytes come from.
pub enum Media {
    Http(HttpFile),
    File(std::fs::File),
}
impl Read for Media {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Media::Http(h) => h.read(buf),
            Media::File(f) => f.read(buf),
        }
    }
}
impl Seek for Media {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        match self {
            Media::Http(h) => h.seek(to),
            Media::File(f) => f.seek(to),
        }
    }
}

type Opened = Result<(Decoder<Media>, Option<Duration>), String>;

enum Cmd {
    Load { client: Client, track: String, local: Option<PathBuf>, play: bool, gen: u64 },
    Loaded { gen: u64, track: String, result: Opened, play: bool },
    /// open the next song ahead of time, so it can fade in while this one fades out
    Preload { client: Client, track: String, local: Option<PathBuf>, gain: f32, gen: u64 },
    Preloaded { gen: u64, track: String, result: Opened, gain: f32 },
    Play,
    Pause,
    Seek(Duration),
    Volume(f32),
    /// volume leveling for the loaded song (ReplayGain, as a multiplier)
    Gain(f32),
    Stop,
}

#[derive(Clone)]
pub struct Player {
    tx: Sender<Cmd>,
    status: Arc<Mutex<Status>>,
    gen: Arc<Mutex<u64>>,
    eq: crate::eq::Shared,
    /// crossfade length in tenths of a second (0 = off)
    fade: Arc<AtomicU32>,
}

impl Player {
    pub fn start() -> Player {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let eq = crate::eq::shared();
        let s = status.clone();
        let back = tx.clone();
        let e = eq.clone();
        let fade = Arc::new(AtomicU32::new(0));
        let f = fade.clone();
        std::thread::Builder::new().name("audio".into()).spawn(move || audio_thread(rx, back, s, e, f)).expect("audio thread");
        Player { tx, status, gen: Arc::new(Mutex::new(0)), eq, fade }
    }

    /// Seconds of crossfade between songs (0 = none).
    pub fn set_crossfade(&self, secs: f32) {
        self.fade.store((secs.clamp(0.0, 12.0) * 10.0) as u32, Ordering::Relaxed);
    }

    /// Open the next song now (it starts fading in when the current one is `crossfade` seconds from its end).
    pub fn preload(&self, client: &Client, track: &str, local: Option<PathBuf>, gain: f32) {
        let gen = *self.gen.lock().unwrap();
        let _ = self.tx.send(Cmd::Preload { client: client.clone(), track: track.to_string(), local, gain, gen });
    }

    /// The equalizer for what is playing and what plays next (changes are heard within a moment).
    pub fn set_eq(&self, e: crate::eq::EqSettings) {
        crate::eq::set(&self.eq, e);
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Open a song and (if `play`) start it. `local` = a downloaded copy to play instead of streaming.
    /// A newer load cancels an older one still opening.
    pub fn load(&self, client: &Client, track: &str, local: Option<PathBuf>, play: bool) {
        let gen = {
            let mut g = self.gen.lock().unwrap();
            *g += 1;
            *g
        };
        let _ = self.tx.send(Cmd::Load { client: client.clone(), track: track.to_string(), local, play, gen });
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
    pub fn set_gain(&self, g: f32) {
        let _ = self.tx.send(Cmd::Gain(g));
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

fn open(client: &Client, track: &str, local: Option<PathBuf>) -> Opened {
    let media = match local.and_then(|p| std::fs::File::open(p).ok()) {
        Some(f) => Media::File(f),
        None => Media::Http(HttpFile::open(client, track).map_err(|e| e.to_string())?),
    };
    let dec = Decoder::new(media).map_err(|e| format!("Can't play this song ({e})."))?;
    let total = dec.total_duration();
    Ok((dec, total))
}

/// The song that is fading out while the next one fades in.
struct Fade {
    old: Sink,
    old_gain: f32,
    started: Instant,
    secs: f32,
}

fn audio_thread(rx: Receiver<Cmd>, back: Sender<Cmd>, status: Arc<Mutex<Status>>, eq: crate::eq::Shared, fade: Arc<AtomicU32>) {
    // the output device; without one (no speakers), every command just reports the error
    let (mut _stream, mut handle) = match OutputStream::try_default() {
        Ok(x) => x,
        Err(e) => {
            let msg = format!("No sound output ({e}). Plug in speakers or headphones, then restart the app.");
            status.lock().unwrap().error = Some(msg.clone());
            // still show what was picked, so the app makes sense without speakers
            for cmd in rx {
                if let Cmd::Load { track, .. } = cmd {
                    let mut st = status.lock().unwrap();
                    st.track = Some(track);
                    st.playing = false;
                    st.loading = false;
                    st.error = Some(msg.clone());
                }
            }
            return;
        }
    };
    let mut sink: Option<Sink> = None;
    let mut volume = 1.0f32;
    let mut gain = 1.0f32;
    let mut latest_gen = 0u64;
    let mut was_busy = false;
    // the next song, opened ahead of time (track, sound, length, its volume leveling)
    let mut next: Option<(String, Decoder<Media>, Option<Duration>, f32)> = None;
    let mut fading: Option<Fade> = None;
    // what is loaded (to open it again on a new device) and where to carry on once it has been
    let mut current: Option<(Client, String, Option<PathBuf>)> = None;
    let mut resume_at: Option<Duration> = None;
    // the Windows default output device: when it changes (headphones, a speaker, another DAC) the sound follows it
    let mut last_device = default_device_name();
    let mut device_check = Instant::now();

    loop {
        let wait = if fading.is_some() { 30 } else { 100 };
        match rx.recv_timeout(Duration::from_millis(wait)) {
            Ok(Cmd::Preload { client, track, local, gain, gen }) => {
                let back = back.clone();
                std::thread::spawn(move || {
                    let result = open(&client, &track, local);
                    let _ = back.send(Cmd::Preloaded { gen, track, result, gain });
                });
            }
            Ok(Cmd::Preloaded { gen, track, result, gain }) => {
                if gen == latest_gen {
                    if let Ok((dec, total)) = result {
                        next = Some((track, dec, total, gain));
                    }
                }
            }
            Ok(Cmd::Load { client, track, local, play, gen }) => {
                latest_gen = gen;
                current = Some((client.clone(), track.clone(), local.clone()));
                next = None;
                if let Some(f) = fading.take() {
                    f.old.stop();
                }
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
                // opening may talk to the server: do it off this thread so pause/volume stay instant
                let back = back.clone();
                std::thread::spawn(move || {
                    let result = open(&client, &track, local);
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
                        s.set_volume(volume * gain);
                        s.append(crate::eq::EqSource::new(dec.convert_samples::<f32>(), eq.clone()));
                        if play {
                            s.play();
                        } else {
                            s.pause();
                        }
                        st.track = Some(track);
                        st.duration = total;
                        st.playing = play;
                        if let Some(at) = resume_at.take() {
                            let _ = s.try_seek(at);
                            st.position = at;
                        }
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
                if let Some(f) = &fading {
                    f.old.play();
                }
            }
            Ok(Cmd::Pause) => {
                if let Some(s) = &sink {
                    s.pause();
                }
                if let Some(f) = &fading {
                    f.old.pause();
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
                    s.set_volume(volume * gain);
                }
            }
            Ok(Cmd::Gain(g)) => {
                gain = g.clamp(0.05, 4.0);
                if let Some(s) = &sink {
                    s.set_volume(volume * gain);
                }
            }
            Ok(Cmd::Stop) => {
                next = None;
                if let Some(f) = fading.take() {
                    f.old.stop();
                }
                if let Some(s) = sink.take() {
                    s.stop();
                }
                was_busy = false;
                let mut st = status.lock().unwrap();
                st.track = None; // nothing is loaded any more
                st.playing = false;
                st.loading = false;
                st.finished = false;
                st.duration = None;
                st.position = Duration::ZERO;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // the output device changed: open the new one and carry on where the song was
        if device_check.elapsed() >= Duration::from_secs(1) {
            device_check = Instant::now();
            let now = default_device_name();
            if now.is_some() && now != last_device {
                last_device = now;
                let (was_playing, pos, finished) = {
                    let st = status.lock().unwrap();
                    (st.playing, st.position, st.finished)
                };
                if let Some(f) = fading.take() {
                    f.old.stop();
                }
                let had_sink = sink.take().map(|s| s.stop()).is_some();
                match OutputStream::try_default() {
                    Ok((st2, h2)) => {
                        _stream = st2;
                        handle = h2;
                        next = None;
                        was_busy = false;
                        if had_sink && !finished {
                            if let Some((client, track, local)) = current.clone() {
                                resume_at = Some(pos);
                                let _ = back.send(Cmd::Load { client, track, local, play: was_playing, gen: latest_gen });
                            }
                        }
                    }
                    Err(e) => {
                        status.lock().unwrap().error = Some(format!("Can't use the new sound output ({e})."));
                    }
                }
            }
        }
        // crossfade: when the song is `fade` seconds from its end and the next one is ready, start it quietly
        let fade_secs = fade.load(Ordering::Relaxed) as f32 / 10.0;
        if fade_secs > 0.0 && fading.is_none() && next.is_some() && sink.is_some() {
            let (playing, pos, total) = {
                let st = status.lock().unwrap();
                (st.playing, st.position, st.duration)
            };
            if let (true, Some(total)) = (playing, total) {
                let secs = fade_secs.min(total.as_secs_f32() / 2.0);
                if secs > 0.2 && total.as_secs_f32() - pos.as_secs_f32() <= secs {
                    if let (Some((track, dec, length, new_gain)), Ok(s2)) = (next.take(), Sink::try_new(&handle)) {
                        s2.set_volume(0.0);
                        s2.append(crate::eq::EqSource::new(dec.convert_samples::<f32>(), eq.clone()));
                        s2.play();
                        if let Some(old) = sink.replace(s2) {
                            fading = Some(Fade { old, old_gain: gain, started: Instant::now(), secs });
                        }
                        gain = new_gain;
                        was_busy = true;
                        let mut st = status.lock().unwrap();
                        st.track = Some(track);
                        st.duration = length;
                        st.position = Duration::ZERO;
                        st.finished = false;
                        st.playing = true;
                        st.advanced += 1;
                    }
                }
            }
        }
        // the two songs' volumes follow a quarter circle while they swap (the sound stays even)
        if let Some(f) = &fading {
            let t = (f.started.elapsed().as_secs_f32() / f.secs).clamp(0.0, 1.0);
            f.old.set_volume(volume * f.old_gain * (t * std::f32::consts::FRAC_PI_2).cos());
            if let Some(s) = &sink {
                s.set_volume(volume * gain * (t * std::f32::consts::FRAC_PI_2).sin());
            }
            if t >= 1.0 {
                if let Some(f) = fading.take() {
                    f.old.stop();
                }
                if let Some(s) = &sink {
                    s.set_volume(volume * gain);
                }
            }
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

/// The name of the system's default sound output (None when there is none).
fn default_device_name() -> Option<String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    rodio::cpal::default_host().default_output_device().and_then(|d| d.name().ok())
}

/// ReplayGain as a volume multiplier: `mode` is "track" or "album" ("off" = 1.0). Never boosts past
/// the song's loudest sample (no clipping).
pub fn replay_gain(rg: Option<&volvix_core::ReplayGain>, mode: &str) -> f32 {
    let Some(rg) = rg else { return 1.0 };
    let db = match mode {
        "track" => rg.track.or(rg.album),
        "album" => rg.album.or(rg.track),
        _ => None,
    };
    let Some(db) = db else { return 1.0 };
    let mut g = 10f32.powf(db / 20.0);
    if let Some(peak) = rg.peak.filter(|p| *p > 0.0) {
        g = g.min(1.0 / peak);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains() {
        let rg = volvix_core::ReplayGain { track: Some(-6.0), album: Some(-3.0), peak: Some(0.9) };
        assert!((replay_gain(Some(&rg), "track") - 0.501).abs() < 0.01);
        assert!((replay_gain(Some(&rg), "album") - 0.708).abs() < 0.01);
        assert_eq!(replay_gain(Some(&rg), "off"), 1.0);
        let loud = volvix_core::ReplayGain { track: Some(6.0), album: None, peak: Some(0.8) };
        assert!((replay_gain(Some(&loud), "track") - 1.25).abs() < 0.01, "capped by the peak");
    }
}
