// VOID — ambient bed.
//
// The old build shipped a busy WebAudio loop (arpeggios, clicks, noise bursts).
// It was rejected as "messy". This is the replacement: a slow, warm drone that
// never repeats a bar, never spikes, and sits far behind the UI.
//
// Structure of one voice (all sine-based, no samples, no files):
//   * fundamental 55 Hz  — the floor of the room
//   * octave + twelfth   — body, gently detuned for a slow beat
//   * filtered noise     — "air", moving at 17 mHz
//   * sparse bell        — one soft strike every ~11 s, pentatonic, 6 s tail
// Fade-in 6 s, stereo width from a 31 mHz LFO, peak gain 0.20.

use std::f64::consts::TAU;
use std::time::Duration;

use rodio::{OutputStream, Sink, Source};

pub const PRESETS: &[(&str, &str)] = &[
    ("Nocturne", "55 Hz drone · warm, slow, no pulse"),
    ("Aurora", "110 Hz air · brighter, wide stereo"),
    ("Cosmos", "detuned fifths · deep, meditative"),
    ("Trauma", "41 Hz sub · pulsing, cinematic, heavy"),
];

pub struct Ambient {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
    pub playing: bool,
    pub volume: f32,
    pub preset: usize,
    /// 0..1 — how much pulse and movement the bed carries.
    pub intensity: f32,
    pub error: Option<String>,
    started: Option<std::time::Instant>,
    /// when the current bed started: drives the 1.6 s gain ramp
    fade_at: Option<std::time::Instant>,
}

impl Ambient {
    pub fn new(preset: usize, volume: f32) -> Self {
        Self {
            stream: None,
            sink: None,
            playing: false,
            volume: volume.clamp(0.0, 1.0),
            preset: preset.min(PRESETS.len() - 1),
            intensity: 0.5,
            error: None,
            started: None,
            fade_at: None,
        }
    }

    fn ensure_device(&mut self) -> bool {
        if self.sink.is_some() {
            return true;
        }
        match OutputStream::try_default() {
            Ok((stream, handle)) => {
                self.stream = Some(stream);
                match Sink::try_new(&handle) {
                    Ok(sink) => {
                        sink.set_volume(0.0);
                        self.sink = Some(sink);
                        self.error = None;
                        true
                    }
                    Err(e) => {
                        self.error = Some(format!("output device: {e}"));
                        let _ = &handle;
                        false
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("no audio device ({e})"));
                false
            }
        }
    }

    pub fn play(&mut self) {
        if !self.ensure_device() {
            self.playing = false;
            return;
        }
        if let Some(sink) = &self.sink {
            if sink.empty() {
                sink.append(Bed::new(44_100, self.preset, self.intensity));
            }
            sink.set_volume(0.0);
            sink.play();
        }
        self.playing = true;
        self.started = Some(std::time::Instant::now());
        self.fade_at = Some(std::time::Instant::now());
    }

    /// 0..1 gain ramp, so a preset change or a fresh start never clicks.
    fn fade(&self) -> f32 {
        match self.fade_at {
            Some(t) => {
                let e = (t.elapsed().as_secs_f32() / 1.6).clamp(0.0, 1.0);
                e * e * (3.0 - 2.0 * e)
            }
            None => 1.0,
        }
    }

    pub fn set_intensity(&mut self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        if (v - self.intensity).abs() < 0.001 {
            return;
        }
        self.intensity = v;
        // the bed bakes intensity into its synthesis, so restart it
        let was = self.playing;
        if let Some(sink) = &self.sink {
            sink.stop();
            sink.clear();
        }
        self.sink = None;
        if was {
            self.play();
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
        self.playing = false;
    }

    pub fn toggle(&mut self) {
        if self.playing {
            self.stop();
        } else {
            self.play();
        }
    }

    pub fn gain(&self) -> f32 {
        // perceptual curve — quiet at the bottom of the slider, calm at the top
        let v = self.volume.clamp(0.0, 1.0);
        v * v * 1.15
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.gain() * self.fade());
        }
    }

    pub fn set_preset(&mut self, p: usize) {
        let p = p.min(PRESETS.len() - 1);
        if p == self.preset {
            return;
        }
        self.preset = p;
        let was = self.playing;
        if let Some(sink) = &self.sink {
            sink.stop();
            sink.clear();
        }
        self.sink = None;
        if was {
            self.play();
        }
    }

    /// A slow, fake VU for the settings page — tracks the drone's own LFO.
    /// Also advances the fade ramp (called once per frame).
    pub fn level(&mut self) -> f32 {
        if !self.playing {
            return 0.0;
        }
        if let Some(sink) = &self.sink {
            sink.set_volume(self.gain() * self.fade());
        }
        let t = self
            .started
            .map(|s| s.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let lfo = (t * 0.031 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        (0.12 + 0.55 * lfo * self.volume).clamp(0.0, 1.0)
    }
}

impl Drop for Ambient {
    fn drop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
    }
}

/// The generator. Infinite, stereo, sample-accurate.
struct Bed {
    sr: u32,
    t: f64,
    preset: usize,
    intensity: f32,
    pulse_t: f64,
    lp: f32,
    rng: u32,
    bell_t: f64,
    bell_f: f64,
    bell_index: usize,
    ch: u32,
    last: f32,
}

const BELL_NOTES: [f64; 5] = [220.0, 261.63, 329.63, 392.0, 440.0];

impl Bed {
    fn new(sr: u32, preset: usize, intensity: f32) -> Self {
        Self {
            sr,
            t: 0.0,
            preset,
            intensity: intensity.clamp(0.0, 1.0),
            pulse_t: 0.0,
            lp: 0.0,
            rng: 0x9E37_79B9,
            bell_t: 30.0,
            bell_f: 261.63,
            bell_index: 0,
            ch: 0,
            last: 0.0,
        }
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn mono(&mut self, dt: f64) -> f32 {
        self.t += dt;
        let t = self.t;

        let (root, air_amt, bell_amt): (f64, f32, f32) = match self.preset {
            1 => (110.0, 0.55, 0.004),
            2 => (73.42, 0.30, 0.010),
            3 => (41.20, 0.46, 0.018),
            _ => (55.0, 0.22, 0.007),
        };
        let bell_amt = bell_amt * (0.45 + 1.15 * self.intensity);

        // slow modulators (milli-Hz: these never line up, so it never loops)
        let lfo_a = ((t * 0.031) * TAU).sin() as f32;
        let lfo_b = ((t * 0.017) * TAU).sin() as f32;

        let p0 = t * root * TAU;
        let body = p0.sin() as f32 * 0.55
            + (p0 * 2.0).sin() as f32 * 0.16
            + (p0 * 3.0 + 0.4).sin() as f32 * 0.05;

        // two detuned fifths -> a 2 mHz beat, the "warmth"
        let det = 1.0 + 0.0009 * lfo_b as f64;
        let fifth = ((t * root * 1.5 * det) * TAU).sin() as f32
            + ((t * root * 1.5 / det) * TAU).sin() as f32;
        let fifth = fifth * 0.10;

        // air: one-pole low-passed noise, opening and closing with lfo_b
        let n = self.noise();
        let k = 0.010 + 0.006 * (lfo_b * 0.5 + 0.5);
        self.lp += (n - self.lp) * k;
        let air = self.lp * air_amt * (0.35 + 0.65 * (lfo_a * 0.5 + 0.5));

        // bells: sparse, soft, long
        self.bell_t += dt;
        let strike_at = 11.0 + (self.bell_index % 3) as f64 * 2.5;
        if self.bell_t > strike_at {
            self.bell_t = 0.0;
            self.bell_index = (self.bell_index + 3) % BELL_NOTES.len();
            self.bell_f = BELL_NOTES[self.bell_index];
        }
        let bell = if self.bell_t < 6.0 {
            let env = (-self.bell_t * 0.85).exp() as f32;
            let swell = (self.bell_t * 3.5).min(1.0) as f32;
            ((t * self.bell_f * TAU).sin() as f32
                + (t * self.bell_f * 2.0 * TAU).sin() as f32 * 0.25)
                * env
                * swell
                * bell_amt
        } else {
            0.0
        };

        // TRAUMA: a sub-bass pulse clock plus a slow minor-second cluster.
        // Nothing here is sampled — it is all synthesised from sines and noise.
        let mut trauma = 0.0f32;
        if self.preset == 3 {
            self.pulse_t += dt;
            let period = 8.5 - 3.5 * self.intensity as f64;
            if self.pulse_t > period {
                self.pulse_t = 0.0;
            }
            let env = (-self.pulse_t * 0.85).exp() as f32;
            let thump = (t * 41.2 * TAU).sin() as f32 * env * (0.26 + 0.34 * self.intensity);
            let glide = ((t * 43.65 * TAU).sin() as f32) * 0.07 * (0.4 + 0.6 * (lfo_b * 0.5 + 0.5));
            trauma = thump + glide;
        }

        let fade = (t / 6.0).min(1.0) as f32; // 6 s fade-in
        let breathe = 0.72 + 0.28 * (lfo_a * 0.5 + 0.5);

        (body + fifth + air) * fade * breathe + bell * fade + trauma * fade
    }
}

impl Iterator for Bed {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let dt = 1.0 / self.sr as f64;
        if self.ch == 0 {
            self.last = self.mono(dt);
            let lfo = ((self.t * 0.031) * TAU).sin() as f32;
            self.ch = 1;
            // left channel: width from the slow LFO, gentle
            Some((self.last * (1.0 + 0.05 * lfo) * 0.20).clamp(-0.9, 0.9))
        } else {
            let lfo = ((self.t * 0.031) * TAU).sin() as f32;
            self.ch = 0;
            Some((self.last * (1.0 - 0.05 * lfo) * 0.20).clamp(-0.9, 0.9))
        }
    }
}

impl Source for Bed {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.sr
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Kept public so the settings page can show a device probe result.
pub fn probe() -> Result<String, String> {
    match OutputStream::try_default() {
        Ok((_s, _h)) => Ok("output device ready".to_string()),
        Err(e) => Err(format!("no audio device ({e})")),
    }
}
