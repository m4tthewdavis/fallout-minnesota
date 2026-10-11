//! Footsteps on each surface.

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::rng::Rng;

/// Two-part footfall: heel then toe, with random timing and weight.
pub(super) fn footfall(
    secs: f32,
    rng: &mut Rng,
    mut hit: impl FnMut(&mut Rng, usize) -> Vec<f32>,
) -> Vec<f32> {
    let mut out = vec![0.0; len(secs)];
    let heel = hit(rng, 0);
    mix_at(&mut out, 0.0, &heel, 1.0);
    let toe = hit(rng, 1);
    mix_at(&mut out, rng.range(0.06, 0.11), &toe, rng.range(0.4, 0.7));
    out
}

pub(super) fn step_snow(mut rng: Rng) -> Vec<f32> {
    let hi = rng.range(2200.0, 3400.0);
    let lo = rng.range(280.0, 600.0);
    let crackle = rng.range(0.02, 0.05);
    let decay = rng.range(16.0, 28.0);
    let out = footfall(0.26, &mut rng, |rng, _| {
        let mut bp = BandPass::new(lo, hi);
        (0..len(0.16))
            .map(|i| {
                let t = i as f32 / SR;
                let n = noise(rng);
                let spike = if rng.chance(crackle) { noise(rng) * 1.6 } else { 0.0 };
                (bp.run(n) * 1.8 + spike * 0.4) * env(t, 0.008, decay)
            })
            .collect()
    });
    normalize(out, 0.4)
}

pub(super) fn step_ice(mut rng: Rng) -> Vec<f32> {
    let tick = rng.range(1700.0, 2900.0);
    let thud = rng.range(95.0, 140.0);
    let creak = rng.chance(0.35);
    let mut out = footfall(0.3, &mut rng, |rng, part| {
        let mut lp = LowPass::new(6000.0);
        (0..len(0.14))
            .map(|i| {
                let t = i as f32 / SR;
                let ring = (TAU * tick * (1.0 + 0.1 * part as f32) * t).sin() * env(t, 0.0008, 70.0);
                let body = (TAU * thud * t).sin() * env(t, 0.003, 26.0) * 0.8;
                lp.run(noise(rng) * env(t, 0.0005, 90.0) * 0.5) + ring * 0.6 + body
            })
            .collect()
    });
    if creak {
        let c: Vec<f32> = (0..len(0.14))
            .map(|i| {
                let t = i as f32 / SR;
                (TAU * (620.0 - 800.0 * t) * t).sin() * env(t, 0.01, 18.0) * 0.25
            })
            .collect();
        mix_at(&mut out, 0.08, &c, 1.0);
    }
    normalize(out, 0.38)
}

pub(super) fn step_road(mut rng: Rng) -> Vec<f32> {
    let hi = rng.range(3500.0, 5200.0);
    let out = footfall(0.22, &mut rng, |rng, _| {
        let mut bp = BandPass::new(1300.0, hi);
        (0..len(0.1))
            .map(|i| {
                let t = i as f32 / SR;
                let grit = if rng.chance(0.06) { noise(rng) } else { 0.0 };
                (bp.run(noise(rng)) * 1.4 + grit * 0.5) * env(t, 0.002, 48.0)
            })
            .collect()
    });
    normalize(out, 0.36)
}

pub(super) fn step_concrete(mut rng: Rng) -> Vec<f32> {
    let ring = rng.range(1100.0, 1700.0);
    let out = footfall(0.24, &mut rng, |rng, part| {
        let mut lp = LowPass::new(4800.0);
        (0..len(0.12))
            .map(|i| {
                let t = i as f32 / SR;
                let click = lp.run(noise(rng)) * env(t, 0.0008, 70.0);
                let thump = (TAU * 85.0 * t).sin() * env(t, 0.002, 38.0) * 0.7;
                let ping = (TAU * ring * t).sin() * env(t, 0.001, 90.0) * 0.15 * (1.0 - 0.4 * part as f32);
                click + thump + ping
            })
            .collect()
    });
    normalize(out, 0.4)
}

pub(super) fn step_wood(mut rng: Rng) -> Vec<f32> {
    let f = rng.range(130.0, 210.0);
    let creak = rng.chance(0.3);
    let mut out = footfall(0.28, &mut rng, |rng, part| {
        let mut lp = LowPass::new(1100.0);
        (0..len(0.14))
            .map(|i| {
                let t = i as f32 / SR;
                let thud = (TAU * f * (1.0 - 0.15 * t * 6.0) * t).sin() * env(t, 0.002, 26.0);
                let knock = lp.run(noise(rng)) * env(t, 0.001, 40.0) * 1.2;
                (thud + knock * 0.6) * (1.0 - 0.3 * part as f32)
            })
            .collect()
    });
    if creak {
        let c: Vec<f32> = (0..len(0.2))
            .map(|i| {
                let t = i as f32 / SR;
                let s = ((TAU * (260.0 + 380.0 * t) * t).sin()).signum() * 0.12 + (TAU * 410.0 * t).sin() * 0.1;
                s * env(t, 0.03, 9.0)
            })
            .collect();
        mix_at(&mut out, 0.05, &c, 1.0);
    }
    normalize(out, 0.42)
}

/// A boot crunching hard through snow when running: heavier, with a second
/// crunch under it.
pub(super) fn step_snow_run(mut rng: Rng) -> Vec<f32> {
    let base = step_snow(Rng::new(rng.next_u64()));
    let mut out = base.clone();
    out.resize(len(0.34), 0.0);
    let mut bp = BandPass::new(180.0, 1800.0);
    let grit: Vec<f32> = (0..len(0.2))
        .map(|i| {
            let t = i as f32 / SR;
            let spike = if rng.chance(0.04) { noise(&mut rng) * 1.4 } else { 0.0 };
            (bp.run(noise(&mut rng)) * 1.6 + spike * 0.5) * env(t, 0.01, 14.0)
        })
        .collect();
    mix_at(&mut out, 0.045, &grit, 0.9);
    normalize(out, 0.55)
}

/// Dry snow below about -10F squeaks and creaks underfoot: a high squeal.
pub(super) fn step_snow_squeak(mut rng: Rng, v: f32) -> Vec<f32> {
    let f0 = 2100.0 + 500.0 * v + rng.range(-120.0, 120.0);
    let mut phase = 0.0f32;
    let mut lp = LowPass::new(4200.0);
    let out: Vec<f32> = (0..len(0.22))
        .map(|i| {
            let t = i as f32 / SR;
            let u = t / 0.22;
            phase += TAU * (f0 * (1.0 + 0.25 * u + 0.03 * (TAU * 31.0 * t).sin())) / SR;
            let squeal = phase.sin() * 0.4 + (phase * 2.01).sin() * 0.12;
            let crunch = lp.run(noise(&mut rng)) * 0.5;
            (squeal + crunch) * env(t, 0.01, 11.0)
        })
        .collect();
    normalize(out, 0.3)
}
