//! Wolves, crows, the moose and the raiders' voices.

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::rng::Rng;

pub(super) struct Howl {
    secs: f32,
    start_hz: f32,
    peak_hz: f32,
    end_hz: f32,
    rise: f32,
    fall: f32,
    vibrato_hz: f32,
    cutoff: f32,
    amp: f32,
    echo: bool,
}

impl Howl {
    pub(super) fn near(v: usize) -> Howl {
        let t = [
            (3.0, 380.0, 640.0, 410.0, 0.7, 0.8, 5.5),
            (2.4, 450.0, 780.0, 450.0, 0.5, 0.7, 6.5),
            (3.6, 330.0, 560.0, 300.0, 1.0, 1.0, 4.8),
            (2.0, 520.0, 700.0, 380.0, 0.35, 0.6, 7.2),
        ][v % 4];
        Howl {
            secs: t.0,
            start_hz: t.1,
            peak_hz: t.2,
            end_hz: t.3,
            rise: t.4,
            fall: t.5,
            vibrato_hz: t.6,
            cutoff: 3000.0,
            amp: 0.42,
            echo: false,
        }
    }
    pub(super) fn far(v: usize) -> Howl {
        let mut h = Howl::near(v + 1);
        h.secs += 1.0;
        h.cutoff = 900.0;
        h.amp = 0.3;
        h.echo = true;
        h
    }
}

pub(super) fn howl(mut rng: Rng, p: &Howl) -> Vec<f32> {
    let total = len(p.secs);
    let mut phase = 0.0;
    let mut lp = LowPass::new(p.cutoff);
    let mut out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let hold_end = p.secs - p.fall;
            let base = if t < p.rise {
                p.start_hz + (p.peak_hz - p.start_hz) * (t / p.rise)
            } else if t < hold_end {
                p.peak_hz - 25.0 * (t - p.rise) / (hold_end - p.rise).max(0.1)
            } else {
                p.peak_hz - 25.0 - (p.peak_hz - 25.0 - p.end_hz) * ((t - hold_end) / p.fall)
            };
            // Vibrato grows as the howl settles.
            let depth = 4.0 + 8.0 * (t / p.secs);
            let f = base + depth * (TAU * p.vibrato_hz * t).sin();
            phase += TAU * f / SR;
            let tone = phase.sin() + 0.25 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin();
            let breath = noise(&mut rng) * 0.05;
            let envelope = (t / 0.35).min(1.0) * ((p.secs - t) / 0.7).clamp(0.0, 1.0);
            lp.run(tone + breath) * envelope
        })
        .collect();
    if p.echo {
        echo(&mut out, 0.38, 0.5, 0.45, 1.6);
    }
    fade_edges(&mut out, 0.05);
    normalize(out, p.amp)
}

pub(super) fn snarl(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 0.38 + 0.3 * v;
    let base = 110.0 + 50.0 * ((v * 3.0).sin().abs());
    let buzz_hz = 26.0 + 12.0 * v;
    let mut lp = LowPass::new(900.0 + 500.0 * v);
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let buzz = 0.5 + 0.5 * (TAU * buzz_hz * t).sin();
            let growl = (TAU * base * t).sin().signum() * 0.4;
            (lp.run(noise(&mut rng)) * 2.0 * buzz + growl * buzz) * env(t, 0.02, 5.0 + 3.0 * v)
        })
        .collect();
    normalize(out, 0.6)
}

pub(super) fn yelp(rng: Rng, v: f32) -> Vec<f32> {
    let _ = rng;
    let secs = 0.22 + 0.14 * v;
    let start = 1050.0 + 220.0 * v;
    let mut phase = 0.0;
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let f = start - (1100.0 + 400.0 * v) * t + 40.0 * (TAU * 28.0 * t).sin();
            phase += TAU * f / SR;
            (phase.sin() + 0.3 * (2.0 * phase).sin()) * env(t, 0.01, 8.0 + 3.0 * v)
        })
        .collect();
    normalize(out, 0.45)
}

/// A crow's caw: two or three harsh, falling, nasal barks.
pub(super) fn caw(mut rng: Rng, v: f32) -> Vec<f32> {
    let barks = 2 + (v * 1.99) as usize;
    let mut out: Vec<f32> = Vec::new();
    for k in 0..barks {
        let secs = 0.2 + 0.07 * v + 0.03 * k as f32;
        let f0 = 520.0 + 100.0 * v - 35.0 * k as f32;
        let mut phase = 0.0;
        let mut lp = LowPass::new(2600.0);
        for i in 0..len(secs) {
            let t = i as f32 / SR;
            // Pitch drops through the bark; a fast flutter roughens it.
            let f = f0 * (1.0 - 0.3 * t / secs) * (1.0 + 0.04 * (TAU * 85.0 * t).sin());
            phase += TAU * f / SR;
            let buzz = phase.sin() + 0.6 * (2.0 * phase).sin() + 0.45 * (3.0 * phase).sin() + 0.3 * (4.0 * phase).sin();
            let rasp = 0.75 + 0.25 * (TAU * 70.0 * t).sin().signum();
            out.push(lp.run(buzz * rasp * 0.5 + noise(&mut rng) * 0.12) * env(t, 0.012, 7.0));
        }
        out.extend(std::iter::repeat_n(0.0, len(0.08)));
    }
    normalize(out, 0.45)
}

pub(super) fn growl(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 1.1 + 0.5 * v;
    let mut lp = LowPass::new(450.0 + 80.0 * v);
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let rattle = 0.55 + 0.45 * (TAU * (22.0 + 6.0 * (TAU * 0.8 * t).sin() + 4.0 * v) * t).sin();
            let body = (TAU * (80.0 + 10.0 * v) * t).sin() * 0.5 + (TAU * 123.0 * t).sin().signum() * 0.2;
            let envelope = (t / 0.15).min(1.0) * ((secs - t) / 0.3).clamp(0.0, 1.0);
            (lp.run(noise(&mut rng)) * 2.2 + body) * rattle * envelope
        })
        .collect();
    normalize(out, 0.5)
}

pub(super) fn moose_bellow(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 1.5 + 0.6 * v;
    let f0 = 72.0 + 14.0 * v;
    let mut phase = 0.0;
    let mut lp = LowPass::new(1100.0);
    let total = len(secs);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let x = t / secs;
            let f = f0 * (1.0 + 0.5 * (x * std::f32::consts::PI).sin()) + 3.0 * (TAU * 7.0 * t).sin();
            phase += TAU * f / SR;
            let mut tone = 0.0;
            for h in 1..=6 {
                tone += (phase * h as f32).sin() / h as f32;
            }
            let rasp = noise(&mut rng) * 0.1;
            let envelope = (t / 0.12).min(1.0) * ((secs - t) / 0.5).clamp(0.0, 1.0);
            lp.run(tone + rasp) * envelope
        })
        .collect();
    normalize(out, 0.8)
}

pub(super) fn moose_grunt(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 0.3 + 0.1 * v;
    let mut lp = LowPass::new(700.0);
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let tone = (TAU * (95.0 - 25.0 * t / secs + 8.0 * v) * t).sin() + 0.4 * (TAU * 190.0 * t).sin();
            lp.run(tone + noise(&mut rng) * 0.2) * env(t, 0.02, 9.0)
        })
        .collect();
    normalize(out, 0.7)
}

pub(super) fn hoof(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut lp = LowPass::new(1200.0);
    let out: Vec<f32> = (0..len(0.22))
        .map(|i| {
            let t = i as f32 / SR;
            let thump = (TAU * (52.0 + 8.0 * v) * t).sin() * env(t, 0.003, 18.0);
            lp.run(noise(&mut rng)) * env(t, 0.001, 55.0) * 0.8 + thump
        })
        .collect();
    normalize(out, 0.8)
}

/// A man's voice, roughly: a buzzing glottal source shaped by two formants.
/// `shout` is a drawn-out "hey!"; otherwise a short grunt of pain.
pub(super) fn raider_voice(mut rng: Rng, v: f32, shout: bool) -> Vec<f32> {
    let secs = if shout { 0.55 + 0.15 * v } else { 0.2 + 0.08 * v };
    let f0 = 105.0 + 28.0 * v + rng.range(-5.0, 5.0);
    let mut phase = 0.0f32;
    let mut f1 = BandPass::new(450.0, 820.0);
    let mut f2 = BandPass::new(900.0, 1500.0);
    let mut breath = LowPass::new(3000.0);
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let u = t / secs;
            // Pitch: a shout rises then falls; a grunt drops.
            let pitch = if shout { f0 * (1.0 + 0.5 * (std::f32::consts::PI * u).sin() + 0.1 * (TAU * 5.0 * t).sin()) } else { f0 * (1.2 - 0.4 * u) };
            phase += TAU * pitch / SR;
            // A buzzy pulse train (rich in harmonics).
            let pulse = (phase.sin() + 0.7 * (2.0 * phase).sin() + 0.5 * (3.0 * phase).sin() + 0.35 * (4.0 * phase).sin() + 0.25 * (5.0 * phase).sin()) * 0.35;
            // Vowel: "eh-ay" for a shout, "uh" for a grunt: the formants slide.
            let (c1, c2) = if shout { (550.0 + 250.0 * u, 1300.0 + 900.0 * u) } else { (520.0, 1050.0) };
            f1 = BandPass::new(c1 * 0.7, c1 * 1.3);
            f2 = BandPass::new(c2 * 0.75, c2 * 1.25);
            let voiced = f1.run(pulse) * 2.2 + f2.run(pulse) * 1.4;
            let air = breath.run(noise(&mut rng)) * 0.08;
            let amp = if shout { (u * 14.0).min(1.0) * (1.0 - u).powf(0.6) } else { (u * 30.0).min(1.0) * (1.0 - u) };
            (voiced + air) * amp
        })
        .collect();
    let mut out = out;
    fade_edges(&mut out, 0.01);
    normalize(out, 0.5)
}
