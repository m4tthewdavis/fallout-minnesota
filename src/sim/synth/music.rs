//! The three background music beds (calm, tense, danger).

use super::dsp::*;
use super::{SR, TAU};
use crate::sim::mathx;
use crate::sim::rng::Rng;

pub(super) fn semis(semitones_from_a4: f32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 / 12.0)
}

/// Sparse glassy bell notes dropped over a bed.
pub(super) fn bells(out: &mut [f32], rng: &mut Rng, scale: &[f32], secs: f32, gap: (f32, f32), amp: f32) {
    let mut t = rng.range(1.0, 4.0);
    while t < secs - 4.0 {
        let f = semis(scale[(rng.f32() * scale.len() as f32) as usize % scale.len()] + 12.0);
        let ring = rng.range(1.8, 3.2);
        let clip: Vec<f32> = (0..len(ring))
            .map(|j| {
                let tt = j as f32 / SR;
                ((TAU * f * tt).sin() + 0.3 * (TAU * f * 2.01 * tt).sin() + 0.1 * (TAU * f * 3.97 * tt).sin()) * env(tt, 0.004, 2.0)
            })
            .collect();
        mix_at(out, t, &clip, amp * rng.range(0.6, 1.0));
        t += rng.range(gap.0, gap.1);
    }
}

/// Slow chord pad: each note is a pair of detuned soft saws.
pub(super) fn pad(out: &mut [f32], chords: &[[f32; 4]], chord_secs: f32, gain: f32, cutoff: f32) {
    let mut lp = LowPass::new(cutoff);
    let mut phases = [[0.0f32; 2]; 4];
    // The last note value seen per voice (as bits) and its frequency.
    let mut pitch = [(u32::MAX, 0.0f32); 4];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let pos = (t / chord_secs) % chords.len() as f32;
        let idx = pos as usize % chords.len();
        let next = (idx + 1) % chords.len();
        let frac = pos - mathx::floor(pos);
        // Glide between chords over the last 2 seconds.
        let blend = ((frac * chord_secs - (chord_secs - 2.0)) / 2.0).clamp(0.0, 1.0);
        let mut v = 0.0;
        for k in 0..4 {
            // The pitch only moves during the glide between chords (a sixth
            // of the time); the rest of the time it is the last sample's, and
            // the `powf` in `semis` is the most expensive thing in the loop.
            let note = chords[idx][k] + (chords[next][k] - chords[idx][k]) * blend;
            if note.to_bits() != pitch[k].0 {
                pitch[k] = (note.to_bits(), semis(note));
            }
            let f = pitch[k].1;
            for (d, detune) in [0.997f32, 1.003].iter().enumerate() {
                phases[k][d] = mathx::fract(phases[k][d] + f * detune / SR);
                v += (2.0 * phases[k][d] - 1.0) * if k == 0 { 0.7 } else { 0.45 };
            }
        }
        let breathe = 0.7 + 0.3 * (TAU * t / (chord_secs * 2.0)).sin();
        lp.set(cutoff * (0.6 + 0.4 * breathe));
        *o += lp.run(v) * gain * breathe;
    }
}

/// "The Long Winter": a slow, cold pad in D minor with sparse glass notes.
pub(super) fn music_calm(mut rng: Rng) -> Vec<f32> {
    let secs = 72.0;
    let chords: [[f32; 4]; 6] = [
        [-31.0, -19.0, -16.0, -12.0], // Dm
        [-35.0, -23.0, -19.0, -16.0], // Bb
        [-36.0, -24.0, -16.0, -12.0], // F/A
        [-33.0, -21.0, -17.0, -14.0], // C
        [-31.0, -19.0, -15.0, -12.0], // Dm (maj third colour)
        [-38.0, -26.0, -19.0, -14.0], // Gm/G
    ];
    let mut out = vec![0.0f32; len(secs + 3.0)];
    pad(&mut out, &chords, 12.0, 0.075, 900.0);
    bells(&mut out, &mut rng, &[-7.0, -4.0, -2.0, 0.0, 3.0, 5.0, 8.0], secs, (4.0, 10.0), 0.06);
    normalize(make_loopable(out, 3.0), 0.4)
}

/// Dread: a low drone, a slow heartbeat, and uneasy high ticks.
pub(super) fn music_tense(mut rng: Rng) -> Vec<f32> {
    let secs = 60.0;
    let total = len(secs + 3.0);
    let mut out = vec![0.0f32; total];
    let mut lp = LowPass::new(220.0);
    let mut p = [0.0f32; 3];
    // The three drone pitches never change: work them out once, not per sample.
    let drone = [semis(-43.0), semis(-43.0) * 1.0595, semis(-31.0) * 1.414];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let mut v = 0.0;
        for (k, f) in drone.into_iter().enumerate() {
            p[k] = mathx::fract(p[k] + f / SR);
            v += (2.0 * p[k] - 1.0) * [0.7, 0.4, 0.15][k];
        }
        let swell = 0.6 + 0.4 * (TAU * t / 15.0).sin();
        *o += lp.run(v) * 0.16 * swell;
    }
    // Heartbeat: two soft thumps every 1.1 s.
    let mut t = 0.0;
    while t < secs {
        for (at, a) in [(0.0, 0.5), (0.22, 0.35)] {
            let thump: Vec<f32> = (0..len(0.25))
                .map(|j| {
                    let tt = j as f32 / SR;
                    (TAU * (58.0 - 20.0 * tt) * tt).sin() * env(tt, 0.004, 14.0)
                })
                .collect();
            mix_at(&mut out, t + at, &thump, a * 0.4);
        }
        t += 1.1;
    }
    // Uneasy high pings in a minor-second cluster.
    bells(&mut out, &mut rng, &[-2.0, -1.0, 4.0, 5.0, 10.0], secs, (3.0, 8.0), 0.035);
    normalize(make_loopable(out, 3.0), 0.4)
}

/// Danger: a driving low pulse, wind swells and a rising alarm tone.
pub(super) fn music_danger(mut rng: Rng) -> Vec<f32> {
    let secs = 48.0;
    let total = len(secs + 2.0);
    let mut out = vec![0.0f32; total];
    let step = 0.375; // eighth notes at 80 bpm
    let notes = [-43.0, -43.0, -43.0, -40.0, -43.0, -43.0, -41.0, -43.0];
    let note_hz = notes.map(semis); // one `powf` per note, not per sample
    let mut phase = 0.0;
    let mut lp = LowPass::new(500.0);
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let n = (t / step) as usize;
        let local = t - n as f32 * step;
        let f = note_hz[n % notes.len()];
        phase = mathx::fract(phase + f / SR);
        let saw = 2.0 * phase - 1.0;
        let pump = env(local, 0.005, 6.0);
        lp.set(300.0 + 700.0 * (0.5 + 0.5 * (TAU * t / 16.0).sin()));
        *o += lp.run(saw) * pump * 0.22;
    }
    // Wind swell every 16 s.
    let mut wl = LowPass::new(600.0);
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let swell = (0.5 - 0.5 * (TAU * t / 16.0).cos()).powi(2);
        *o += wl.run(noise(&mut rng)) * swell * 0.25;
    }
    // A rising alarm-ish tone at the end of each 16 s phrase.
    let mut ph = 0.0;
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let x = (t % 16.0) / 16.0;
        if x > 0.75 {
            let k = (x - 0.75) / 0.25;
            ph += TAU * (300.0 + 500.0 * k) / SR;
            *o += (3.0 * ph.sin()).tanh() * 0.05 * k;
        }
    }
    normalize(make_loopable(out, 2.0), 0.45)
}
