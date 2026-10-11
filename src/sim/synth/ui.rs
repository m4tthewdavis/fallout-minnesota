//! The Pip-Boy's blips, static, hum and the menu tick.

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::rng::Rng;

pub(super) fn pip_blip(on: bool) -> Vec<f32> {
    let mut phase = 0.0;
    let out: Vec<f32> = (0..len(0.2))
        .map(|i| {
            let t = i as f32 / SR;
            let x = t / 0.2;
            let f = if on { 380.0 + 700.0 * x } else { 1080.0 - 700.0 * x };
            phase += TAU * f / SR;
            (phase.sin() * 0.6 + (2.0 * phase).sin() * 0.15) * env(t, 0.005, 12.0)
        })
        .collect();
    normalize(out, 0.4)
}

/// A crackling burst of white noise, like a CRT changing channel.
pub(super) fn pip_static(mut rng: Rng) -> Vec<f32> {
    let mut crackle = 0.0;
    let out: Vec<f32> = (0..len(0.22))
        .map(|i| {
            let t = i as f32 / SR;
            if rng.f32() < 0.02 {
                crackle = noise(&mut rng) * 1.5;
            }
            crackle *= 0.97;
            (noise(&mut rng) * 0.6 + crackle) * env(t, 0.003, 14.0)
        })
        .collect();
    normalize(out, 0.4)
}

/// A dry, low tick for moving through a list.
pub(super) fn pip_scroll() -> Vec<f32> {
    let out: Vec<f32> = (0..len(0.035))
        .map(|i| {
            let t = i as f32 / SR;
            ((TAU * 820.0 * t).sin() + 0.4 * (TAU * 2460.0 * t).sin()) * env(t, 0.0005, 110.0)
        })
        .collect();
    normalize(out, 0.3)
}

/// Mains hum (60 Hz and harmonics), a faint CRT whine and the odd crackle.
/// Whole cycles over the 3-second loop so it repeats without a click.
pub(super) fn pip_hum(mut rng: Rng) -> Vec<f32> {
    let n = len(3.0);
    let out: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let hum = (TAU * 60.0 * t).sin() * 0.5 + (TAU * 120.0 * t).sin() * 0.35 + (TAU * 180.0 * t).sin() * 0.15;
            let whine = (TAU * 7800.0 * t).sin() * 0.03;
            let crackle = if rng.f32() < 0.0008 { noise(&mut rng) * 0.6 } else { 0.0 };
            hum * (0.85 + 0.15 * (TAU * t / 1.5).sin()) + whine + crackle
        })
        .collect();
    normalize(out, 0.35)
}

pub(super) fn ui_tick() -> Vec<f32> {
    let out: Vec<f32> = (0..len(0.05))
        .map(|i| {
            let t = i as f32 / SR;
            (TAU * 1400.0 * t).sin() * env(t, 0.001, 70.0)
        })
        .collect();
    normalize(out, 0.35)
}
