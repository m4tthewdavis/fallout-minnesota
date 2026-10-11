//! Guns, their mechanisms and the echoes they leave behind.

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::rng::Rng;

pub(super) struct Shot {
    crack_lp: f32,
    crack_decay: f32,
    thump_hz: f32,
    thump_decay: f32,
    ring_hz: f32,
    ring_amp: f32,
    tail_secs: f32,
    tail_lp: f32,
    tail_decay: f32,
    tail_amp: f32,
    /// A felt-more-than-heard sub-bass boom under the crack.
    sub_hz: f32,
    sub_amp: f32,
    amp: f32,
}

impl Shot {
    pub(super) fn rifle(v: f32) -> Shot {
        Shot {
            crack_lp: 2700.0 + 900.0 * v,
            crack_decay: 22.0 + 6.0 * v,
            thump_hz: 82.0 + 28.0 * v,
            thump_decay: 9.0,
            ring_hz: 0.0,
            ring_amp: 0.0,
            tail_secs: 1.0,
            tail_lp: 750.0,
            tail_decay: 5.0 + 1.5 * v,
            tail_amp: 0.14,
            sub_hz: 64.0,
            sub_amp: 0.7,
            amp: 0.95,
        }
    }
    pub(super) fn shotgun(v: f32) -> Shot {
        Shot {
            crack_lp: 2300.0 + 500.0 * v,
            crack_decay: 13.0,
            thump_hz: 55.0 + 12.0 * v,
            thump_decay: 6.0,
            ring_hz: 0.0,
            ring_amp: 0.0,
            tail_secs: 1.5,
            tail_lp: 600.0,
            tail_decay: 3.6,
            tail_amp: 0.2,
            sub_hz: 46.0,
            sub_amp: 1.1,
            amp: 1.0,
        }
    }
    pub(super) fn revolver(v: f32) -> Shot {
        Shot {
            crack_lp: 4200.0 + 700.0 * v,
            crack_decay: 32.0,
            thump_hz: 125.0 + 25.0 * v,
            thump_decay: 14.0,
            ring_hz: 1800.0 + 500.0 * v,
            ring_amp: 0.1,
            tail_secs: 0.7,
            tail_lp: 1100.0,
            tail_decay: 8.0,
            tail_amp: 0.1,
            sub_hz: 72.0,
            sub_amp: 0.5,
            amp: 0.9,
        }
    }
}

pub(super) fn shot(mut rng: Rng, p: &Shot) -> Vec<f32> {
    let total = len(p.tail_secs + 0.2);
    let mut crack_lp = LowPass::new(p.crack_lp);
    let mut tail_lp = LowPass::new(p.tail_lp);
    let mut sub_phase = 0.0f32;
    let mut out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            // The boom drops in pitch as the pressure wave rolls away.
            sub_phase += TAU * (p.sub_hz * (1.0 - 1.4 * t).max(0.45)) / SR;
            let sub = sub_phase.sin() * env(t, 0.004, 6.0) * p.sub_amp;
            let crack = crack_lp.run(noise(&mut rng)) * env(t, 0.0008, p.crack_decay);
            let thump = (TAU * (p.thump_hz - 40.0 * t) * t).sin() * env(t, 0.002, p.thump_decay);
            let ring = (TAU * p.ring_hz * t).sin() * env(t, 0.001, 16.0) * p.ring_amp;
            let tail = tail_lp.run(noise(&mut rng)) * env(t, 0.02, p.tail_decay) * p.tail_amp * 3.0;
            crack * 1.5 + thump * 0.8 + sub + ring + tail
        })
        .collect();
    echo(&mut out, 0.085, 0.4, 0.22, 0.25);
    for v in &mut out {
        *v = v.tanh();
    }
    fade_edges(&mut out, 0.01);
    normalize(out, p.amp)
}

pub(super) fn click(mut rng: Rng, amp: f32, tone: f32) -> Vec<f32> {
    (0..len(0.07))
        .map(|i| {
            let t = i as f32 / SR;
            (noise(&mut rng) * 0.5 + (TAU * tone * t).sin()) * env(t, 0.0005, 90.0) * amp
        })
        .collect()
}

pub(super) fn jam(mut rng: Rng, v: f32) -> Vec<f32> {
    let f = 800.0 + 300.0 * v;
    (0..len(0.35))
        .map(|i| {
            let t = i as f32 / SR;
            let clank = (TAU * f * t).sin() + 0.7 * (TAU * f * 1.51 * t).sin() + 0.4 * (TAU * f * 2.38 * t).sin();
            (clank * 0.35 + noise(&mut rng) * 0.3) * env(t, 0.001, 14.0) * 0.7
        })
        .collect()
}

pub(super) fn shell_tink(mut rng: Rng, v: f32) -> Vec<f32> {
    let f = 2900.0 + 700.0 * v + rng.range(-100.0, 100.0);
    let mut out = vec![0.0; len(0.4)];
    for (offset, a) in [(0.0, 0.25), (0.11 + 0.02 * v, 0.14), (0.2 + 0.03 * v, 0.07)] {
        let clip: Vec<f32> = (0..len(0.12))
            .map(|j| {
                let t = j as f32 / SR;
                ((TAU * f * t).sin() + 0.6 * (TAU * f * 1.56 * t).sin()) * env(t, 0.0005, 45.0)
            })
            .collect();
        mix_at(&mut out, offset, &clip, a);
    }
    out
}

/// Magazine or shell leaving (`out`) or entering a gun: a metal clunk.
pub(super) fn clunk(mut rng: Rng, out_: bool, v: f32) -> Vec<f32> {
    let f = if out_ { 210.0 } else { 260.0 } * (1.0 + 0.12 * v);
    let mut out = vec![0.0; len(0.3)];
    let body: Vec<f32> = (0..len(0.18))
        .map(|i| {
            let t = i as f32 / SR;
            ((TAU * f * t).sin() + 0.6 * (TAU * f * 2.3 * t).sin() + 0.3 * (TAU * f * 4.1 * t).sin()) * env(t, 0.001, 30.0)
        })
        .collect();
    mix_at(&mut out, 0.0, &body, 0.5);
    let mut lp = LowPass::new(3000.0);
    let tick: Vec<f32> = (0..len(0.03)).map(|i| lp.run(noise(&mut rng)) * env(i as f32 / SR, 0.0005, 120.0)).collect();
    mix_at(&mut out, if out_ { 0.0 } else { 0.06 }, &tick, 0.7);
    if !out_ {
        // Seating click.
        let seat: Vec<f32> = (0..len(0.05)).map(|i| (TAU * 1500.0 * i as f32 / SR).sin() * env(i as f32 / SR, 0.0005, 80.0)).collect();
        mix_at(&mut out, 0.09, &seat, 0.35);
    }
    normalize(out, 0.6)
}

pub(super) fn bolt_rack(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.4)];
    // Slide hiss.
    let mut bp = BandPass::new(500.0, 2400.0);
    let slide: Vec<f32> = (0..len(0.12))
        .map(|i| {
            let t = i as f32 / SR;
            bp.run(noise(&mut rng)) * (t / 0.12).min(1.0) * 1.3
        })
        .collect();
    mix_at(&mut out, 0.0, &slide, 0.4);
    for (at, f, a) in [(0.12, 900.0 + 200.0 * v, 0.7), (0.27, 1250.0 + 150.0 * v, 0.55)] {
        let clack: Vec<f32> = (0..len(0.09))
            .map(|i| {
                let t = i as f32 / SR;
                ((TAU * f * t).sin() + 0.5 * (TAU * f * 1.7 * t).sin() + noise(&mut rng) * 0.3) * env(t, 0.0005, 55.0)
            })
            .collect();
        mix_at(&mut out, at, &clack, a);
    }
    normalize(out, 0.6)
}

/// A shotgun breaking open or snapping shut.
pub(super) fn hinge(mut rng: Rng, open: bool, v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.4)];
    if open {
        let creak: Vec<f32> = (0..len(0.22))
            .map(|i| {
                let t = i as f32 / SR;
                let f = 280.0 + 260.0 * t / 0.22 + 25.0 * v;
                ((TAU * f * t).sin().signum() * 0.18 + (TAU * f * 1.5 * t).sin() * 0.1) * env(t, 0.02, 7.0)
            })
            .collect();
        mix_at(&mut out, 0.0, &creak, 1.0);
    }
    let at = if open { 0.2 } else { 0.0 };
    let snap: Vec<f32> = (0..len(0.1))
        .map(|i| {
            let t = i as f32 / SR;
            ((TAU * (1000.0 + 300.0 * v) * t).sin() + (TAU * 340.0 * t).sin() * 0.8 + noise(&mut rng) * 0.4) * env(t, 0.0006, 38.0)
        })
        .collect();
    mix_at(&mut out, at, &snap, 0.6);
    normalize(out, 0.55)
}

pub(super) fn cylinder_spin(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.5)];
    let mut t0 = 0.0;
    for i in 0..9 {
        let gap = 0.045 + 0.006 * i as f32;
        let tick: Vec<f32> = (0..len(0.02))
            .map(|j| ((TAU * (2600.0 + 200.0 * v) * j as f32 / SR).sin() + noise(&mut rng) * 0.5) * env(j as f32 / SR, 0.0003, 160.0))
            .collect();
        mix_at(&mut out, t0, &tick, 0.4);
        t0 += gap;
    }
    out
}

pub(super) fn swing(mut rng: Rng, v: f32) -> Vec<f32> {
    let total = len(0.32);
    let mut bp = BandPass::new(300.0 + 100.0 * v, 1500.0 + 400.0 * v);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / total as f32;
            bp.run(noise(&mut rng)) * (t * std::f32::consts::PI).sin().powf(1.5) * 1.5
        })
        .collect();
    normalize(out, 0.55)
}

pub(super) fn melee_hit(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut lp = LowPass::new(2400.0);
    let total = len(0.35);
    let out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let thud = (TAU * (95.0 + 20.0 * v - 30.0 * t) * t).sin() * env(t, 0.002, 14.0);
            let crack = lp.run(noise(&mut rng)) * env(t, 0.0008, 40.0);
            thud * 0.9 + crack * 0.9
        })
        .collect();
    normalize(out, 0.85)
}

/// What a gunshot leaves behind. Outdoors: a dull, low rumble and a few soft
/// answers off distant treelines. Indoors: a dense, bright slap that dies in
/// well under a second. Played just after the shot itself.
pub(super) fn shot_tail(mut rng: Rng, indoor: bool, v: f32) -> Vec<f32> {
    let secs = if indoor { 0.9 } else { 2.4 };
    let mut out = vec![0.0f32; len(secs)];
    if indoor {
        // Early reflections off the walls, then a dense decaying wash.
        let mut lp = LowPass::new(3800.0 - 600.0 * v);
        for k in 0..9 {
            let at = 0.004 + 0.008 * k as f32 + rng.range(0.0, 0.006);
            let amp = 0.55 * (-(k as f32) * 0.3).exp();
            let burst: Vec<f32> = (0..len(0.02)).map(|i| noise(&mut rng) * env(i as f32 / SR, 0.0005, 160.0)).collect();
            mix_at(&mut out, at, &burst, amp);
        }
        for (i, o) in out.iter_mut().enumerate() {
            let t = i as f32 / SR;
            let wash = lp.run(noise(&mut rng)) * env(t, 0.03, 6.5 - 1.0 * v) * 0.35;
            // A room's boom: one low mode.
            let boom = (TAU * (95.0 + 20.0 * v) * t).sin() * env(t, 0.01, 8.0) * 0.28;
            *o += wash + boom;
        }
    } else {
        // Treeline answers: soft, dark, spaced out and thinning.
        for (delay, amp) in [(0.2 + 0.05 * v, 0.5f32), (0.47 + 0.08 * v, 0.3), (0.9 + 0.1 * v, 0.17)] {
            let mut lp = LowPass::new(900.0);
            let echo_len = len(0.55);
            let burst: Vec<f32> = (0..echo_len)
                .map(|i| {
                    let t = i as f32 / SR;
                    lp.set(1000.0 - 600.0 * (t / 0.55));
                    lp.run(noise(&mut rng)) * env(t, 0.03, 6.0) * 2.4
                })
                .collect();
            mix_at(&mut out, delay, &burst, amp);
        }
        // The rolling rumble underneath.
        let mut lp = LowPass::new(170.0);
        for (i, o) in out.iter_mut().enumerate() {
            let t = i as f32 / SR;
            *o += lp.run(noise(&mut rng)) * env(t, 0.08, 2.0) * 1.6;
        }
    }
    fade_edges(&mut out, 0.02);
    normalize(out, 0.7)
}

/// Working a bolt: a heavy clack, a pause, and the lighter clack of it going home.
pub(super) fn bolt_clack(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; len(0.4)];
    for (at, amp, f) in [(0.0, 1.0f32, 1700.0 + 300.0 * v), (0.13 + 0.03 * v, 0.7, 2300.0 + 300.0 * v)] {
        let clip: Vec<f32> = (0..len(0.16))
            .map(|i| {
                let t = i as f32 / SR;
                let ring = (TAU * f * t).sin() + 0.6 * (TAU * f * 1.62 * t).sin() + 0.4 * (TAU * f * 2.7 * t).sin();
                (ring * 0.3 + noise(&mut rng) * 0.5) * env(t, 0.0004, 60.0) + (TAU * 190.0 * t).sin() * env(t, 0.001, 38.0) * 0.5
            })
            .collect();
        mix_at(&mut out, at, &clip, amp);
    }
    normalize(out, 0.55)
}

/// Thumbing back a revolver's hammer: ratchet ticks rising to a final click.
pub(super) fn hammer_cock(mut rng: Rng, v: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; len(0.35)];
    for k in 0..4 {
        let at = 0.03 * k as f32 * (1.0 + 0.1 * v) + if k == 3 { 0.03 } else { 0.0 };
        let f = 1500.0 + 350.0 * k as f32;
        let amp = if k == 3 { 1.0 } else { 0.5 };
        let clip: Vec<f32> = (0..len(0.05))
            .map(|i| {
                let t = i as f32 / SR;
                ((TAU * f * t).sin() * 0.5 + noise(&mut rng) * 0.4) * env(t, 0.0003, 120.0)
            })
            .collect();
        mix_at(&mut out, at, &clip, amp);
    }
    normalize(out, 0.5)
}
