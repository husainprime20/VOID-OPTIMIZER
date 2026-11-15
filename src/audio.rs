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
];

pub struct Ambient {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
    pub playing: bool,
    pub volume: f32,
    pub preset: usize,
    pub error: Option<String>,
    started: Option<std::time::Instant>,
}

impl Ambient {
    pub fn new(preset: usize, volume: f32) -> Self {
        Self {
            stream: None,
            sink: None,
            playing: false,
            volume: volume.clamp(0.0, 1.0),
            preset: preset.min(PRESETS.len() - 1),
            error: None,
            started: None,
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
                sink.append(Bed::new(44_100, self.preset));
            }
            sink.set_volume(self.gain());
            sink.play();
        }
        self.playing = true;
        self.started = Some(std::time::Instant::now());
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
            sink.set_volume(self.gain());
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
    pub fn level(&self) -> f32 {
        if !self.playing {
            return 0.0;
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
    fn new(sr: u32, preset: usize) -> Self {
        Self {
            sr,
            t: 0.0,
            preset,
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
            _ => (55.0, 0.22, 0.007),
        };

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

        let fade = (t / 6.0).min(1.0) as f32; // 6 s fade-in
        let breathe = 0.72 + 0.28 * (lfo_a * 0.5 + 0.5);

        (body + fifth + air) * fade * breathe + bell * fade
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
