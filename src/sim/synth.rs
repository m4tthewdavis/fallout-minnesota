//! Procedural sound effects. Every sound in the prototype is synthesised at
//! start-up and handed to Bevy as an in-memory WAV, so the project needs no
//! audio files.

use super::rng::Rng;

pub const SAMPLE_RATE: u32 = 22_050;
const SR: f32 = SAMPLE_RATE as f32;
const TAU: f32 = std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    WindCalm,
    WindStorm,
    Siren,
    Gunshot,
    DryClick,
    Jam,
    Reload,
    Step1,
    Step2,
    Step3,
    HowlNear,
    HowlFar,
    Snarl,
    Yelp,
    IceCrack,
    Pickup,
    Craft,
}

impl Sound {
    pub const ALL: [Sound; 17] = [
        Sound::WindCalm,
        Sound::WindStorm,
        Sound::Siren,
        Sound::Gunshot,
        Sound::DryClick,
        Sound::Jam,
        Sound::Reload,
        Sound::Step1,
        Sound::Step2,
        Sound::Step3,
        Sound::HowlNear,
        Sound::HowlFar,
        Sound::Snarl,
        Sound::Yelp,
        Sound::IceCrack,
        Sound::Pickup,
        Sound::Craft,
    ];

    /// Mono samples in -1..=1.
    pub fn samples(self) -> Vec<f32> {
        match self {
            Sound::WindCalm => wind(8.0, 0.18, false, 11),
            Sound::WindStorm => wind(8.0, 0.42, true, 12),
            Sound::Siren => siren(),
            Sound::Gunshot => gunshot(),
            Sound::DryClick => click(0.5),
            Sound::Jam => jam(),
            Sound::Reload => reload(),
            Sound::Step1 => step(21),
            Sound::Step2 => step(22),
            Sound::Step3 => step(23),
            Sound::HowlNear => howl(0.55, false),
            Sound::HowlFar => howl(0.22, true),
            Sound::Snarl => snarl(),
            Sound::Yelp => yelp(),
            Sound::IceCrack => ice_crack(),
            Sound::Pickup => blips(&[880.0, 1320.0]),
            Sound::Craft => blips(&[660.0, 880.0, 1320.0]),
        }
    }

    pub fn wav(self) -> Vec<u8> {
        encode_wav(&self.samples())
    }
}

fn len(secs: f32) -> usize {
    (secs * SR) as usize
}

/// One-pole low-pass filter.
struct LowPass {
    y: f32,
    a: f32,
}

impl LowPass {
    fn new(cutoff_hz: f32) -> Self {
        let mut lp = LowPass { y: 0.0, a: 0.0 };
        lp.set(cutoff_hz);
        lp
    }
    fn set(&mut self, cutoff_hz: f32) {
        self.a = 1.0 - (-TAU * cutoff_hz / SR).exp();
    }
    fn run(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

fn noise(rng: &mut Rng) -> f32 {
    rng.f32() * 2.0 - 1.0
}

fn env(t: f32, attack: f32, decay_rate: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) * decay_rate).exp()
    }
}

/// Fade the last `secs` of a loop into its start so it repeats seamlessly.
fn make_loopable(mut s: Vec<f32>, secs: f32) -> Vec<f32> {
    let n = len(secs).min(s.len() / 2);
    let body = s.len() - n;
    for i in 0..n {
        let t = i as f32 / n as f32;
        s[i] = s[i] * t + s[body + i] * (1.0 - t);
    }
    s.truncate(body);
    s
}

fn wind(secs: f32, amp: f32, storm: bool, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let total = len(secs + 1.0);
    let mut lp = LowPass::new(400.0);
    let mut lp2 = LowPass::new(300.0);
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let t = i as f32 / SR;
        let gust = 0.6 + 0.4 * (TAU * t / secs * 2.0).sin() * (TAU * t / secs * 3.0 + 1.0).sin();
        lp.set(if storm {
            500.0 + 700.0 * gust
        } else {
            250.0 + 300.0 * gust
        });
        let n = noise(&mut rng);
        let mut v = lp2.run(lp.run(n)) * 3.0 * gust;
        if storm {
            // A thin whistle through the pines.
            let f = 700.0 + 180.0 * (TAU * t / secs * 2.0).sin();
            v += 0.08 * (TAU * f * t).sin() * gust * gust;
        }
        out.push(v * amp);
    }
    make_loopable(out, 1.0)
}

fn siren() -> Vec<f32> {
    // Classic civil-defence wail: one long rise and fall every 4 seconds.
    let total = len(8.0);
    let mut phase = 0.0;
    let mut lp = LowPass::new(2500.0);
    (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let f = 420.0 + 380.0 * (0.5 - 0.5 * (TAU * t / 4.0).cos());
            phase += TAU * f / SR;
            let tone = (3.0 * phase.sin()).tanh() * 0.8 + 0.2 * (2.0 * phase).sin();
            lp.run(tone) * 0.32
        })
        .collect()
}

fn gunshot() -> Vec<f32> {
    let mut rng = Rng::new(31);
    let mut lp = LowPass::new(3000.0);
    (0..len(0.6))
        .map(|i| {
            let t = i as f32 / SR;
            let crack = noise(&mut rng) * env(t, 0.001, 22.0);
            let thump = (TAU * (90.0 - 40.0 * t) * t).sin() * env(t, 0.002, 9.0);
            (lp.run(crack) * 1.4 + thump * 0.8).clamp(-1.0, 1.0) * 0.85
        })
        .collect()
}

fn click(amp: f32) -> Vec<f32> {
    let mut rng = Rng::new(41);
    (0..len(0.06))
        .map(|i| {
            let t = i as f32 / SR;
            (noise(&mut rng) * 0.5 + (TAU * 2400.0 * t).sin()) * env(t, 0.0005, 90.0) * amp
        })
        .collect()
}

fn jam() -> Vec<f32> {
    let mut rng = Rng::new(51);
    (0..len(0.35))
        .map(|i| {
            let t = i as f32 / SR;
            let clank = (TAU * 930.0 * t).sin() + 0.7 * (TAU * 1410.0 * t).sin() + 0.4 * (TAU * 2210.0 * t).sin();
            (clank * 0.35 + noise(&mut rng) * 0.3) * env(t, 0.001, 14.0) * 0.7
        })
        .collect()
}

fn reload() -> Vec<f32> {
    let mut out = vec![0.0; len(0.9)];
    let c = click(0.6);
    let mut rng = Rng::new(61);
    let mut lp = LowPass::new(1800.0);
    // Mag out, a metallic slide, mag in.
    for (offset, gain) in [(0.05, 1.0), (0.75, 1.2)] {
        let start = len(offset);
        for (j, v) in c.iter().enumerate() {
            if let Some(o) = out.get_mut(start + j) {
                *o += v * gain;
            }
        }
    }
    for i in len(0.3)..len(0.55) {
        let t = (i - len(0.3)) as f32 / SR;
        out[i] += lp.run(noise(&mut rng)) * 0.35 * (TAU * t / 0.5).sin().abs();
    }
    out
}

fn step(seed: u64) -> Vec<f32> {
    // Snow crunch: band-passed noise with random crackles.
    let mut rng = Rng::new(seed);
    let mut lo = LowPass::new(2500.0);
    let mut lo2 = LowPass::new(400.0);
    (0..len(0.2))
        .map(|i| {
            let t = i as f32 / SR;
            let n = noise(&mut rng);
            let band = lo.run(n) - lo2.run(n);
            let crackle = if rng.chance(0.02) { noise(&mut rng) * 1.5 } else { 0.0 };
            (band * 1.6 + crackle * 0.3) * env(t, 0.01, 20.0) * 0.35
        })
        .collect()
}

fn howl(amp: f32, far: bool) -> Vec<f32> {
    let mut rng = Rng::new(71);
    let total = len(3.0);
    let mut phase = 0.0;
    let mut lp = LowPass::new(if far { 900.0 } else { 3000.0 });
    (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let base = if t < 0.7 {
                380.0 + 260.0 * (t / 0.7)
            } else if t < 2.2 {
                640.0 - 20.0 * (t - 0.7)
            } else {
                610.0 - 200.0 * (t - 2.2) / 0.8
            };
            let f = base + 9.0 * (TAU * 5.5 * t).sin();
            phase += TAU * f / SR;
            let tone = phase.sin() + 0.25 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin();
            let breath = noise(&mut rng) * 0.05;
            let envelope = (t / 0.35).min(1.0) * ((3.0 - t) / 0.7).clamp(0.0, 1.0);
            lp.run(tone + breath) * envelope * amp
        })
        .collect()
}

fn snarl() -> Vec<f32> {
    let mut rng = Rng::new(81);
    let mut lp = LowPass::new(1200.0);
    (0..len(0.45))
        .map(|i| {
            let t = i as f32 / SR;
            let buzz = 0.5 + 0.5 * (TAU * 32.0 * t).sin();
            let growl = (TAU * 140.0 * t).sin().signum() * 0.4;
            (lp.run(noise(&mut rng)) * 2.0 * buzz + growl * buzz) * env(t, 0.02, 6.0) * 0.6
        })
        .collect()
}

fn yelp() -> Vec<f32> {
    let mut phase = 0.0;
    (0..len(0.28))
        .map(|i| {
            let t = i as f32 / SR;
            let f = 1150.0 - 1400.0 * t;
            phase += TAU * f / SR;
            (phase.sin() + 0.3 * (2.0 * phase).sin()) * env(t, 0.01, 9.0) * 0.4
        })
        .collect()
}

fn ice_crack() -> Vec<f32> {
    let mut rng = Rng::new(91);
    let total = len(1.2);
    let mut out = vec![0.0; total];
    // Deep boom.
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        *o += (TAU * 55.0 * t).sin() * env(t, 0.005, 4.0) * 0.6;
    }
    // Sharp splintering cracks, each a short ringing ping.
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

fn blips(freqs: &[f32]) -> Vec<f32> {
    let each = len(0.09);
    let mut out = Vec::with_capacity(each * freqs.len());
    for &f in freqs {
        for i in 0..each {
            let t = i as f32 / SR;
            out.push((TAU * f * t).sin().signum() * 0.12 * env(t, 0.002, 25.0));
        }
    }
    out
}

/// 16-bit mono PCM WAV file.
pub fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let wav = encode_wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]), 36 + 6);
    }

    #[test]
    fn every_sound_is_audible_and_sane() {
        for sound in Sound::ALL {
            let s = sound.samples();
            assert!(s.len() > 500, "{sound:?} too short");
            assert!(s.iter().all(|v| v.is_finite()), "{sound:?} has NaN");
            let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.02, "{sound:?} is silent (peak {peak})");
            assert!(peak <= 1.0 + 1e-3, "{sound:?} clips (peak {peak})");
        }
    }

    #[test]
    fn far_howl_is_quieter() {
        let peak = |s: Sound| s.samples().iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak(Sound::HowlFar) < peak(Sound::HowlNear));
        assert!(peak(Sound::WindCalm) < peak(Sound::WindStorm));
    }
}
