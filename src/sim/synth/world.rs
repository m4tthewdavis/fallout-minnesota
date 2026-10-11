//! Ice, radiation, pickups, crafting, wind, fire, the siren and the rooms' own sounds.

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::mathx;
use crate::sim::rng::Rng;

pub(super) fn ice_crack(mut rng: Rng) -> Vec<f32> {
    let total = len(1.2);
    let mut out = vec![0.0; total];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        *o += (TAU * 55.0 * t).sin() * env(t, 0.005, 4.0) * 0.6;
    }
    for _ in 0..14 {
        let start = (rng.f32() * total as f32 * 0.6) as usize;
        let f = rng.range(1500.0, 4200.0);
        let a = rng.range(0.2, 0.5);
        for j in 0..len(0.08) {
            if let Some(o) = out.get_mut(start + j) {
                let t = j as f32 / SR;
                *o += ((TAU * f * t).sin() + noise(&mut rng) * 0.6) * env(t, 0.0005, 60.0) * a;
            }
        }
    }
    out.iter().map(|v| v.clamp(-1.0, 1.0)).collect()
}

pub(super) fn ice_creak(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut lp = LowPass::new(1400.0);
    let mut phase = 0.0;
    let secs = 0.7 + 0.5 * v;
    let base = 170.0 + 60.0 * v;
    (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            let f = base + 120.0 * (t / secs) + 30.0 * (TAU * 7.0 * t).sin();
            phase += TAU * f / SR;
            let stick = if rng.chance(0.15) { 1.0 } else { 0.4 };
            let tone = (phase.sin() * 3.0).tanh() * stick;
            let envelope = (t / 0.08).min(1.0) * ((secs - t) / 0.3).clamp(0.0, 1.0);
            lp.run(tone + noise(&mut rng) * 0.2) * envelope * 0.35
        })
        .collect()
}

/// One Geiger click: a soft tick with a little ring.
pub(super) fn geiger(mut rng: Rng, v: f32) -> Vec<f32> {
    let f = 2400.0 + 1100.0 * v;
    let out: Vec<f32> = (0..len(0.03))
        .map(|i| {
            let t = i as f32 / SR;
            (noise(&mut rng) * 0.7 + (TAU * f * t).sin() * 0.5) * env(t, 0.0003, 260.0)
        })
        .collect();
    normalize(out, 0.5)
}

pub(super) fn pickup_ammo(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.35)];
    for (at, f, a) in [(0.0, 2300.0 + 300.0 * v, 0.5), (0.07, 3000.0 + 300.0 * v, 0.35)] {
        let ping: Vec<f32> = (0..len(0.12))
            .map(|i| {
                let t = i as f32 / SR;
                ((TAU * f * t).sin() + 0.4 * (TAU * f * 2.7 * t).sin() + noise(&mut rng) * 0.2) * env(t, 0.0005, 38.0)
            })
            .collect();
        mix_at(&mut out, at, &ping, a);
    }
    normalize(out, 0.5)
}

/// A soft sine note with a hint of overtone and a gentle fall-off.
pub(super) fn note(f: f32, secs: f32, decay: f32) -> Vec<f32> {
    (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            ((TAU * f * t).sin() + 0.2 * (TAU * f * 2.0 * t).sin()) * env(t, 0.004, decay)
        })
        .collect()
}

pub(super) fn pickup_med(v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.5)];
    let base = 740.0 * (1.0 + 0.08 * v);
    mix_at(&mut out, 0.0, &note(base, 0.3, 11.0), 0.5);
    mix_at(&mut out, 0.1, &note(base * 1.5, 0.35, 9.0), 0.45);
    normalize(out, 0.4)
}

pub(super) fn pickup_food(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut lp = LowPass::new(900.0);
    let out: Vec<f32> = (0..len(0.2))
        .map(|i| {
            let t = i as f32 / SR;
            let f = 340.0 + 40.0 * v - 600.0 * t;
            (TAU * f * t).sin() * env(t, 0.003, 17.0) + lp.run(noise(&mut rng)) * env(t, 0.001, 50.0) * 0.4
        })
        .collect();
    normalize(out, 0.45)
}

pub(super) fn pickup_scrap(mut rng: Rng) -> Vec<f32> {
    let mut out = vec![0.0; len(0.4)];
    let mut at = 0.0;
    for _ in 0..5 {
        let f = rng.range(1300.0, 3200.0);
        let tick: Vec<f32> = (0..len(0.05))
            .map(|i| {
                let t = i as f32 / SR;
                ((TAU * f * t).sin() + noise(&mut rng) * 0.6) * env(t, 0.0004, 70.0)
            })
            .collect();
        mix_at(&mut out, at, &tick, rng.range(0.25, 0.5));
        at += rng.range(0.02, 0.07);
    }
    normalize(out, 0.5)
}

pub(super) fn craft(v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.9)];
    let base = 520.0 * (1.0 + 0.1 * v);
    for (k, ratio) in [1.0, 1.25, 1.5].into_iter().enumerate() {
        mix_at(&mut out, k as f32 * 0.14, &note(base * ratio, 0.4, 7.0), 0.45);
    }
    normalize(out, 0.4)
}

pub(super) fn container_open(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.6)];
    let creak: Vec<f32> = (0..len(0.3))
        .map(|i| {
            let t = i as f32 / SR;
            let f = 240.0 + 200.0 * t / 0.3 + 30.0 * v;
            ((TAU * f * t).sin().signum() * 0.15 + (TAU * f * 1.4 * t).sin() * 0.1) * env(t, 0.03, 6.0)
        })
        .collect();
    mix_at(&mut out, 0.0, &creak, 1.0);
    let mut lp = LowPass::new(1200.0);
    let thunk: Vec<f32> = (0..len(0.15))
        .map(|i| {
            let t = i as f32 / SR;
            (TAU * 110.0 * t).sin() * env(t, 0.002, 24.0) + lp.run(noise(&mut rng)) * env(t, 0.001, 50.0)
        })
        .collect();
    mix_at(&mut out, 0.28, &thunk, 0.6);
    normalize(out, 0.55)
}

#[derive(Clone, Copy)]
pub(super) enum WindLayer {
    Low,
    Mid,
    High,
}

/// One layer of the wind. The three layers have different lengths (23, 29
/// and 37 seconds), so played together their combined repeat is about
/// 4 hours long: the loop can't be heard.
pub(super) fn wind_layer(mut rng: Rng, layer: WindLayer) -> Vec<f32> {
    let secs = match layer {
        WindLayer::Low => 23.0f32,
        WindLayer::Mid => 29.0,
        WindLayer::High => 37.0,
    };
    let total = len(secs + 2.0);
    let mut lp1 = LowPass::new(300.0);
    let mut lp2 = LowPass::new(300.0);
    let mut hp = LowPass::new(1500.0);
    let mut out = Vec::with_capacity(total);
    // A few incommensurate sines make a gust envelope that never feels periodic.
    let (p1, p2, p3) = (rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU));
    for i in 0..total {
        let t = i as f32 / SR;
        let w = t / secs;
        // Each sine completes a whole number of cycles over the loop.
        let gust = 0.55
            + 0.25 * (TAU * 2.0 * w + p1).sin()
            + 0.15 * (TAU * 5.0 * w + p2).sin()
            + 0.08 * (TAU * 11.0 * w + p3).sin();
        let n = noise(&mut rng);
        let v = match layer {
            WindLayer::Low => {
                lp1.set(140.0 + 120.0 * gust);
                lp2.run(lp1.run(n)) * 5.0 * gust
            }
            WindLayer::Mid => {
                lp1.set(500.0 + 700.0 * gust);
                lp2.set(260.0);
                (lp1.run(n) - lp2.run(n)) * 3.2 * gust
            }
            WindLayer::High => {
                let hiss = n - hp.run(n);
                let f = 760.0 + 150.0 * (TAU * 3.0 * w + p1).sin() + 60.0 * (TAU * 9.0 * w).sin();
                let whistle = (TAU * f * t).sin() * 0.05 * gust * gust;
                hiss * 0.18 * gust * gust + whistle
            }
        };
        out.push(v);
    }
    let looped = make_loopable(out, 2.0);
    normalize(looped, 0.5)
}

/// A single gust of wind: a swell that rises and falls, 4-6 seconds.
pub(super) fn gust(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 4.0 + 2.0 * v;
    let total = len(secs);
    let mut lp = LowPass::new(400.0);
    let mut lp2 = LowPass::new(250.0);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let x = i as f32 / total as f32;
            let swell = (x * std::f32::consts::PI).sin().powf(1.6);
            lp.set(250.0 + 900.0 * swell);
            let n = noise(&mut rng);
            let band = lp.run(n) - lp2.run(n) * 0.6;
            let whistle = (TAU * (620.0 + 180.0 * swell + 40.0 * v) * i as f32 / SR).sin() * 0.04 * swell * swell;
            (band * 2.5 + whistle) * swell
        })
        .collect();
    normalize(out, 0.5)
}

/// A looping fire-barrel crackle: soft roar plus random pops. Each of the
/// three loops has a different length and pop pattern.
pub(super) fn fire_loop(mut rng: Rng, secs: f32) -> Vec<f32> {
    let total = len(secs + 0.5);
    let mut lp = LowPass::new(350.0);
    let mut out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let flutter = 0.7 + 0.3 * (TAU * 0.9 * t).sin() * (TAU * 1.7 * t).cos();
            lp.run(noise(&mut rng)) * 0.9 * flutter
        })
        .collect();
    for _ in 0..(secs * 14.0) as usize {
        let start = (rng.f32() * total as f32) as usize;
        let a = rng.range(0.06, 0.45);
        let decay = rng.range(150.0, 520.0);
        let ring = rng.chance(0.15);
        let f = rng.range(900.0, 2500.0);
        for j in 0..len(0.04) {
            if let Some(o) = out.get_mut(start + j) {
                let t = j as f32 / SR;
                let tone = if ring { (TAU * f * t).sin() * 0.5 } else { 0.0 };
                *o += (noise(&mut rng) + tone) * env(t, 0.0003, decay) * a;
            }
        }
    }
    let out: Vec<f32> = out.into_iter().map(|v| v.clamp(-1.0, 1.0)).collect();
    normalize(make_loopable(out, 0.5), 0.45)
}

pub(super) fn siren() -> Vec<f32> {
    // Classic civil-defence wail: one long rise and fall every 4 seconds,
    // softened and rounded so it warns without grating.
    let total = len(8.0);
    let mut phase = 0.0;
    let mut lp = LowPass::new(1400.0);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let f = 400.0 + 340.0 * (0.5 - 0.5 * (TAU * t / 4.0).cos());
            phase += TAU * f / SR;
            let tone = (2.0 * phase.sin()).tanh() * 0.7 + 0.25 * (2.0 * phase).sin();
            lp.run(tone)
        })
        .collect();
    normalize(out, 0.28)
}

/// A calm breeze: soft, low, slowly breathing. 41 s, so it never lines up
/// with the other wind layers.
pub(super) fn wind_breeze(mut rng: Rng) -> Vec<f32> {
    let secs = 41.0f32;
    let total = len(secs + 2.0);
    let mut lp1 = LowPass::new(260.0);
    let mut lp2 = LowPass::new(260.0);
    let (p1, p2) = (rng.range(0.0, TAU), rng.range(0.0, TAU));
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let w = t / secs;
            // Whole numbers of cycles over the loop: it joins up seamlessly.
            let breath = 0.6 + 0.25 * (TAU * 7.0 * w + p1).sin() + 0.15 * (TAU * 17.0 * w + p2).sin();
            lp1.set(180.0 + 260.0 * breath);
            lp2.run(lp1.run(noise(&mut rng))) * breath * 4.0
        })
        .collect();
    normalize(make_loopable(out, 2.0), 0.4)
}

/// The howl of a blizzard in the eaves: a resonant, wandering, rising and
/// falling moan with a thin hiss over it. 47 s.
pub(super) fn wind_howl(mut rng: Rng) -> Vec<f32> {
    let secs = 47.0f32;
    let total = len(secs + 2.0);
    let (p1, p2, p3) = (rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU));
    let mut phase = 0.0f32;
    let mut phase2 = 0.0f32;
    let mut hp = LowPass::new(2500.0);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let w = t / secs;
            let swell = (0.5 + 0.5 * (TAU * 3.0 * w + p1).sin()).powf(1.6);
            let drift = 0.5 + 0.5 * (TAU * 5.0 * w + p2).sin();
            let f = 330.0 + 330.0 * drift + 40.0 * (TAU * 13.0 * w + p3).sin();
            phase += TAU * f / SR;
            phase2 += TAU * f * 1.51 / SR;
            let moan = phase.sin() * 0.5 + phase2.sin() * 0.22 + (phase * 2.0).sin() * 0.1;
            let n = noise(&mut rng);
            let hiss = (n - hp.run(n)) * 0.25;
            (moan * 0.7 + hiss) * (0.15 + 0.85 * swell)
        })
        .collect();
    normalize(make_loopable(out, 2.0), 0.45)
}

/// A vault's machine hum: mains hum and its harmonics, a slow beat, and the
/// breath of a vent. 8 s, with a whole number of cycles of every tone.
pub(super) fn room_hum(mut rng: Rng) -> Vec<f32> {
    let secs = 8.0f32;
    let total = len(secs + 1.0);
    let mut lp = LowPass::new(520.0);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let hum = (TAU * 50.0 * t).sin() * 0.5 + (TAU * 100.0 * t).sin() * 0.25 + (TAU * 150.0 * t).sin() * 0.12 + (TAU * 250.0 * t).sin() * 0.04;
            let beat = 0.85 + 0.15 * (TAU * 0.25 * t).sin();
            let vent = lp.run(noise(&mut rng)) * 0.5;
            hum * beat + vent * (0.6 + 0.4 * (TAU * 0.125 * t).sin())
        })
        .collect();
    normalize(make_loopable(out, 1.0), 0.4)
}

/// A drip falling into standing water: a plink dropping in pitch, and its echo.
pub(super) fn drip(mut rng: Rng, v: f32) -> Vec<f32> {
    let f0 = 1500.0 + 500.0 * v + rng.range(-80.0, 80.0);
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..len(0.55))
        .map(|i| {
            let t = i as f32 / SR;
            phase += TAU * (f0 * (1.0 - 0.55 * (t / 0.05).min(1.0))) / SR;
            phase.sin() * env(t, 0.0006, 38.0)
        })
        .collect();
    echo(&mut out, 0.17, 0.3, 0.35, 0.25);
    normalize(out, 0.35)
}

/// Timber creaking as it contracts in the cold: a slow, wavering, rasping groan.
pub(super) fn creak(mut rng: Rng, v: f32) -> Vec<f32> {
    let secs = 0.9 + 0.5 * v;
    let (a, b, c) = (rng.range(0.0, TAU), rng.range(0.0, TAU), 70.0 + 30.0 * v);
    let mut phase = 0.0f32;
    let mut lp = LowPass::new(900.0);
    let out: Vec<f32> = (0..len(secs))
        .map(|i| {
            let t = i as f32 / secs / SR;
            let f = c + 55.0 * (TAU * 1.5 * t + a).sin() + 25.0 * (TAU * 4.0 * t + b).sin();
            phase += TAU * f.max(30.0) / SR;
            // A sawtooth, scraped: lots of harmonics, jittered in amplitude.
            let saw = mathx::fract(phase / TAU) * 2.0 - 1.0;
            let scrape = 0.6 + 0.4 * noise(&mut rng);
            lp.run(saw * scrape) * (TAU * 0.5 * t).sin().abs().powf(0.7)
        })
        .collect();
    let mut out = out;
    fade_edges(&mut out, 0.05);
    normalize(out, 0.4)
}
