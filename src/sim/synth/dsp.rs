//! Small building blocks shared by every synthesised sound: filters, envelopes, noise, echo and loop-fading.

use super::{SR, TAU};
use crate::sim::rng::Rng;

pub(super) fn len(secs: f32) -> usize {
    (secs * SR) as usize
}

/// One-pole low-pass filter.
pub(super) struct LowPass {
    y: f32,
    a: f32,
}

impl LowPass {
    pub(super) fn new(cutoff_hz: f32) -> Self {
        let mut lp = LowPass { y: 0.0, a: 0.0 };
        lp.set(cutoff_hz);
        lp
    }
    pub(super) fn set(&mut self, cutoff_hz: f32) {
        self.a = 1.0 - (-TAU * cutoff_hz / SR).exp();
    }
    pub(super) fn run(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

/// Band-pass made from two low-passes.
pub(super) struct BandPass {
    hi: LowPass,
    lo: LowPass,
}

impl BandPass {
    pub(super) fn new(low_hz: f32, high_hz: f32) -> Self {
        BandPass {
            hi: LowPass::new(high_hz),
            lo: LowPass::new(low_hz),
        }
    }
    pub(super) fn run(&mut self, x: f32) -> f32 {
        self.hi.run(x) - self.lo.run(x)
    }
}

pub(super) fn noise(rng: &mut Rng) -> f32 {
    rng.f32() * 2.0 - 1.0
}

pub(super) fn env(t: f32, attack: f32, decay_rate: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) * decay_rate).exp()
    }
}

/// Add `clip` into `out` starting at `secs`, scaled by `gain`.
pub(super) fn mix_at(out: &mut [f32], secs: f32, clip: &[f32], gain: f32) {
    let start = len(secs);
    for (j, v) in clip.iter().enumerate() {
        if let Some(o) = out.get_mut(start + j) {
            *o += v * gain;
        }
    }
}

/// Scale so the loudest sample is `peak`.
pub(super) fn normalize(mut s: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if m > 1e-6 {
        for v in &mut s {
            *v *= peak / m;
        }
    }
    s
}

/// A simple echo / room tail: `delay` seconds, `feedback` per repeat.
pub(super) fn echo(s: &mut Vec<f32>, delay: f32, feedback: f32, mix: f32, tail: f32) {
    let d = len(delay).max(1);
    s.extend(std::iter::repeat_n(0.0, len(tail)));
    let dry = s.clone();
    let mut fb = vec![0.0f32; s.len()];
    for i in 0..s.len() {
        let wet = if i >= d { dry[i - d] + fb[i - d] * feedback } else { 0.0 };
        fb[i] = wet;
        s[i] += wet * mix;
    }
}

/// Fade the last `secs` of a loop into its start so it repeats seamlessly.
pub(super) fn make_loopable(mut s: Vec<f32>, secs: f32) -> Vec<f32> {
    let n = len(secs).min(s.len() / 2);
    let body = s.len() - n;
    for i in 0..n {
        let t = i as f32 / n as f32;
        s[i] = s[i] * t + s[body + i] * (1.0 - t);
    }
    s.truncate(body);
    s
}

pub(super) fn fade_edges(s: &mut [f32], secs: f32) {
    let n = len(secs).min(s.len() / 2).max(1);
    let total = s.len();
    for i in 0..n {
        let k = i as f32 / n as f32;
        s[i] *= k;
        s[total - 1 - i] *= k;
    }
}
