//! The equalizer: a preamp and ten peaking filters (32 Hz … 16 kHz, Q 1.1), the same as the website's
//! (web/js/fx.js), run on the sound before it reaches the speakers. The settings are shared with the website
//! (`settings.eq` in your app data: on, preset, pre, gains).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::Source;
use serde_json::{json, Value};

pub const BANDS: [f32; 10] = [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
const Q: f32 = 1.1;

/// Name shown, key stored, preamp dB, gains dB.
pub const PRESETS: [(&str, &str, f32, [f32; 10]); 11] = [
    ("Flat", "flat", 0.0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Bass boost", "bass", -3.0, [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Treble boost", "treble", -3.0, [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0]),
    ("Vocal", "vocal", -2.0, [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0]),
    ("Rock", "rock", -3.0, [5.0, 4.0, 2.0, -1.0, -2.0, -1.0, 2.0, 4.0, 5.0, 5.0]),
    ("Pop", "pop", -2.0, [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0]),
    ("Jazz", "jazz", -2.0, [3.0, 2.0, 1.0, 2.0, -2.0, -2.0, 0.0, 1.0, 2.0, 3.0]),
    ("Classical", "classical", -2.0, [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0]),
    ("Electronic", "electronic", -3.0, [5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 1.0, 4.0, 5.0]),
    ("Loudness", "loudness", -4.0, [6.0, 4.0, 0.0, 0.0, -1.0, 0.0, -1.0, 0.0, 3.0, 5.0]),
    ("Late night (quieter highs and lows)", "late", 0.0, [-3.0, -2.0, 0.0, 1.0, 1.0, 1.0, 0.0, -1.0, -3.0, -4.0]),
];

#[derive(Debug, Clone, PartialEq)]
pub struct EqSettings {
    pub on: bool,
    /// "flat", a preset's key, or "custom"
    pub preset: String,
    pub pre_db: f32,
    pub gains: [f32; 10],
}

impl Default for EqSettings {
    fn default() -> Self {
        EqSettings { on: false, preset: "flat".into(), pre_db: 0.0, gains: [0.0; 10] }
    }
}

impl EqSettings {
    /// Read `settings.eq` as the website saves it.
    pub fn from_json(v: Option<&Value>) -> EqSettings {
        let Some(v) = v else { return EqSettings::default() };
        let mut e = EqSettings { on: v["on"].as_bool().unwrap_or(false), preset: v["preset"].as_str().unwrap_or("flat").to_string(), pre_db: v["pre"].as_f64().unwrap_or(0.0) as f32, gains: [0.0; 10] };
        if let Some(list) = v["gains"].as_array() {
            for (i, g) in list.iter().take(10).enumerate() {
                e.gains[i] = g.as_f64().unwrap_or(0.0) as f32;
            }
        }
        e
    }

    pub fn to_json(&self) -> Value {
        json!({ "on": self.on, "preset": self.preset, "pre": self.pre_db, "gains": self.gains })
    }

    pub fn preset_name(&self) -> Option<&'static str> {
        PRESETS.iter().find(|p| p.1 == self.preset).map(|p| p.0)
    }

    pub fn with_preset(&self, name: &str) -> EqSettings {
        match PRESETS.iter().find(|p| p.0 == name) {
            Some(p) => EqSettings { on: self.on, preset: p.1.into(), pre_db: p.2, gains: p.3 },
            None => self.clone(),
        }
    }
}

/// The settings the sound is using, and a number that changes whenever they do.
pub type Shared = Arc<Mutex<(u64, EqSettings)>>;

pub fn shared() -> Shared {
    Arc::new(Mutex::new((0, EqSettings::default())))
}

pub fn set(shared: &Shared, e: EqSettings) {
    if let Ok(mut s) = shared.lock() {
        if s.1 != e {
            s.0 += 1;
            s.1 = e;
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    /// A peaking filter (RBJ cookbook): `gain_db` at `freq`.
    fn peaking(freq: f32, gain_db: f32, rate: f32) -> Option<Biquad> {
        if gain_db.abs() < 0.05 || freq >= rate / 2.0 {
            return None;
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * freq / rate;
        let alpha = w0.sin() / (2.0 * Q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Some(Biquad { b0: (1.0 + alpha * a) / a0, b1: -2.0 * cos / a0, b2: (1.0 - alpha * a) / a0, a1: -2.0 * cos / a0, a2: (1.0 - alpha / a) / a0 })
    }
}

/// A source with the equalizer applied.
pub struct EqSource<S: Source<Item = f32>> {
    inner: S,
    shared: Shared,
    version: u64,
    filters: Vec<Biquad>,
    /// per channel, per filter: the two delay values
    state: Vec<Vec<[f32; 2]>>,
    pre: f32,
    on: bool,
    channels: u16,
    rate: u32,
    index: usize,
}

impl<S: Source<Item = f32>> EqSource<S> {
    pub fn new(inner: S, shared: Shared) -> Self {
        let (channels, rate) = (inner.channels().max(1), inner.sample_rate().max(1));
        let mut s = EqSource { inner, shared, version: u64::MAX, filters: vec![], state: vec![], pre: 1.0, on: false, channels, rate, index: 0 };
        s.refresh();
        s
    }

    fn refresh(&mut self) {
        let (v, e) = match self.shared.lock() {
            Ok(g) => (g.0, g.1.clone()),
            Err(_) => return,
        };
        if v == self.version {
            return;
        }
        self.version = v;
        self.on = e.on;
        self.pre = if e.on { 10f32.powf(e.pre_db / 20.0) } else { 1.0 };
        self.filters = if e.on { BANDS.iter().zip(e.gains.iter()).filter_map(|(f, g)| Biquad::peaking(*f, *g, self.rate as f32)).collect() } else { vec![] };
        // keep the delay values when only the numbers changed (no click); start fresh if the count changed
        if self.state.len() != self.channels as usize || self.state.iter().any(|s| s.len() != self.filters.len()) {
            self.state = vec![vec![[0.0; 2]; self.filters.len()]; self.channels as usize];
        }
    }
}

impl<S: Source<Item = f32>> Iterator for EqSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let x = self.inner.next()?;
        if self.index % 2048 == 0 {
            self.refresh();
        }
        let ch = self.index % self.channels as usize;
        self.index = self.index.wrapping_add(1);
        if !self.on {
            return Some(x);
        }
        let mut y = x * self.pre;
        for (f, z) in self.filters.iter().zip(self.state[ch].iter_mut()) {
            let out = f.b0 * y + z[0];
            z[0] = f.b1 * y - f.a1 * out + z[1];
            z[1] = f.b2 * y - f.a2 * out;
            y = out;
        }
        Some(y.clamp(-1.0, 1.0))
    }
}

impl<S: Source<Item = f32>> Source for EqSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        for s in self.state.iter_mut() {
            for z in s.iter_mut() {
                *z = [0.0; 2];
            }
        }
        self.index = 0;
        Ok(())
    }
}
