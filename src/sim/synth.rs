//! Procedural sound. Every sound in the game is synthesised at start-up and
//! handed to Bevy as an in-memory WAV, so the project needs no audio files.
//!
//! A [`Sound`] is a logical event ("a footstep on ice", "a wolf snarls").
//! Frequent sounds have several *variants* that differ in pitch, timing and
//! texture, so the game can avoid playing the same clip twice in a row. Each
//! sound also has a [`Profile`]: its mixer bus, base loudness, how much its
//! pitch and volume wobble on each play, how often it may play, and how far
//! away it can be heard.

use super::rng::Rng;

pub const SAMPLE_RATE: u32 = 22_050;
const SR: f32 = SAMPLE_RATE as f32;
const TAU: f32 = std::f32::consts::TAU;

/// Which volume slider a sound belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bus {
    /// Gunshots, footsteps, wolves, pickups.
    Sfx,
    /// Wind, fire, siren: the background bed.
    Ambience,
    Music,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    // Footsteps by surface.
    StepSnow,
    StepIce,
    StepRoad,
    StepConcrete,
    StepWood,
    // Guns.
    RifleShot,
    ShotgunShot,
    RevolverShot,
    DryClick,
    Jam,
    ShellTink,
    // Reload and handling cues.
    ClunkOut,
    ClunkIn,
    BoltRack,
    BreakOpen,
    BreakClose,
    CylinderSpin,
    Swing,
    MeleeHit,
    // Wolves.
    HowlNear,
    HowlFar,
    Snarl,
    Yelp,
    Growl,
    // The mutated moose.
    MooseBellow,
    MooseGrunt,
    Hoof,
    // World.
    IceCrack,
    IceCreak,
    Geiger,
    // Items and crafting.
    PickupAmmo,
    PickupMed,
    PickupFood,
    PickupScrap,
    Craft,
    ContainerOpen,
    // Ambience loops and one-shots.
    WindLow,
    WindMid,
    WindHigh,
    Gust,
    Fire,
    Siren,
    // Music, one loop per mood.
    MusicCalm,
    MusicTense,
    MusicDanger,
    // Pip-Boy interface.
    PipOn,
    PipOff,
    /// Burst of CRT static when the Pip-Boy changes page.
    PipStatic,
    /// Soft tick moving through a list.
    PipScroll,
    /// Transformer hum and CRT whine while the Pip-Boy is up (loops).
    PipHum,
    /// Keep last: the tests rely on it.
    UiTab,
}

/// How a sound sits in the mix and how it may be played.
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub bus: Bus,
    /// Base loudness 0..1 before the player's volume sliders.
    pub volume: f32,
    /// Random pitch change on each play, as a fraction (0.06 = +-6%).
    pub pitch: f32,
    /// Random volume change on each play, as a fraction.
    pub vary: f32,
    /// Seconds before this sound (or its group) may play again.
    pub cooldown: f32,
    /// Sounds in one group share a cooldown (near and far howls).
    pub group: Sound,
    /// Rapid repeats get quieter, then recover.
    pub fatigue: bool,
    /// Distance in metres within which a positioned sound is at full
    /// volume. It then falls off with the square of the distance.
    pub spatial_ref: Option<f32>,
}

impl Sound {
    pub const ALL: &'static [Sound] = &[
        Sound::StepSnow,
        Sound::StepIce,
        Sound::StepRoad,
        Sound::StepConcrete,
        Sound::StepWood,
        Sound::RifleShot,
        Sound::ShotgunShot,
        Sound::RevolverShot,
        Sound::DryClick,
        Sound::Jam,
        Sound::ShellTink,
        Sound::ClunkOut,
        Sound::ClunkIn,
        Sound::BoltRack,
        Sound::BreakOpen,
        Sound::BreakClose,
        Sound::CylinderSpin,
        Sound::Swing,
        Sound::MeleeHit,
        Sound::HowlNear,
        Sound::HowlFar,
        Sound::Snarl,
        Sound::Yelp,
        Sound::Growl,
        Sound::MooseBellow,
        Sound::MooseGrunt,
        Sound::Hoof,
        Sound::IceCrack,
        Sound::IceCreak,
        Sound::Geiger,
        Sound::PickupAmmo,
        Sound::PickupMed,
        Sound::PickupFood,
        Sound::PickupScrap,
        Sound::Craft,
        Sound::ContainerOpen,
        Sound::WindLow,
        Sound::WindMid,
        Sound::WindHigh,
        Sound::Gust,
        Sound::Fire,
        Sound::Siren,
        Sound::MusicCalm,
        Sound::MusicTense,
        Sound::MusicDanger,
        Sound::PipOn,
        Sound::PipOff,
        Sound::PipStatic,
        Sound::PipScroll,
        Sound::PipHum,
        Sound::UiTab,
    ];

    /// How many variants of this sound exist.
    pub fn variants(self) -> usize {
        match self {
            Sound::StepSnow | Sound::StepIce | Sound::StepRoad | Sound::StepConcrete | Sound::StepWood => 6,
            Sound::RifleShot => 4,
            Sound::ShotgunShot | Sound::RevolverShot | Sound::DryClick => 3,
            Sound::Jam | Sound::ClunkOut | Sound::ClunkIn | Sound::CylinderSpin => 2,
            Sound::BoltRack | Sound::Swing | Sound::MeleeHit => 3,
            Sound::ShellTink => 3,
            Sound::BreakOpen | Sound::BreakClose => 2,
            Sound::HowlNear | Sound::HowlFar | Sound::Snarl | Sound::Yelp => 4,
            Sound::Growl => 3,
            Sound::MooseBellow | Sound::MooseGrunt | Sound::Hoof => 3,
            Sound::IceCrack => 2,
            Sound::IceCreak => 3,
            Sound::Geiger => 5,
            Sound::PickupAmmo | Sound::PickupMed | Sound::PickupFood | Sound::PickupScrap => 3,
            Sound::Craft | Sound::ContainerOpen => 2,
            Sound::Gust => 4,
            Sound::Fire => 3,
            Sound::WindLow
            | Sound::WindMid
            | Sound::WindHigh
            | Sound::Siren
            | Sound::MusicCalm
            | Sound::MusicTense
            | Sound::MusicDanger
            | Sound::PipOn
            | Sound::PipOff
            | Sound::PipStatic
            | Sound::PipScroll
            | Sound::PipHum
            | Sound::UiTab => 1,
        }
    }

    /// True for sounds that are meant to loop.
    pub fn is_loop(self) -> bool {
        matches!(
            self,
            Sound::WindLow
                | Sound::WindMid
                | Sound::WindHigh
                | Sound::Fire
                | Sound::Siren
                | Sound::MusicCalm
                | Sound::MusicTense
                | Sound::MusicDanger
                | Sound::PipHum
        )
    }

    pub fn profile(self) -> Profile {
        use Bus::*;
        let p = |bus, volume, pitch, vary, cooldown, group, fatigue, spatial_ref| Profile {
            bus,
            volume,
            pitch,
            vary,
            cooldown,
            group,
            fatigue,
            spatial_ref,
        };
        match self {
            Sound::StepSnow => p(Sfx, 0.7, 0.08, 0.2, 0.12, self, false, None),
            Sound::StepIce => p(Sfx, 0.65, 0.08, 0.2, 0.12, self, false, None),
            Sound::StepRoad => p(Sfx, 0.65, 0.07, 0.2, 0.12, self, false, None),
            Sound::StepConcrete => p(Sfx, 0.65, 0.07, 0.2, 0.12, self, false, None),
            Sound::StepWood => p(Sfx, 0.7, 0.07, 0.2, 0.12, self, false, None),
            Sound::RifleShot => p(Sfx, 0.85, 0.04, 0.08, 0.0, self, false, None),
            Sound::ShotgunShot => p(Sfx, 0.95, 0.04, 0.08, 0.0, self, false, None),
            Sound::RevolverShot => p(Sfx, 0.85, 0.04, 0.08, 0.0, self, false, None),
            Sound::DryClick => p(Sfx, 0.6, 0.05, 0.15, 0.25, self, true, None),
            Sound::Jam => p(Sfx, 0.7, 0.04, 0.1, 0.3, self, false, None),
            Sound::ShellTink => p(Sfx, 0.5, 0.1, 0.3, 0.08, self, false, Some(8.0)),
            Sound::ClunkOut | Sound::ClunkIn => p(Sfx, 0.7, 0.05, 0.12, 0.1, self, false, None),
            Sound::BoltRack => p(Sfx, 0.75, 0.04, 0.12, 0.1, self, false, None),
            Sound::BreakOpen | Sound::BreakClose | Sound::CylinderSpin => p(Sfx, 0.7, 0.04, 0.12, 0.1, self, false, None),
            Sound::Swing => p(Sfx, 0.6, 0.08, 0.15, 0.1, self, false, None),
            Sound::MeleeHit => p(Sfx, 0.85, 0.06, 0.1, 0.1, self, false, None),
            Sound::HowlNear => p(Sfx, 0.6, 0.05, 0.12, 18.0, Sound::HowlNear, true, Some(45.0)),
            Sound::HowlFar => p(Sfx, 0.55, 0.05, 0.15, 18.0, Sound::HowlNear, true, Some(70.0)),
            Sound::Snarl => p(Sfx, 0.6, 0.08, 0.15, 0.6, self, true, Some(10.0)),
            Sound::Yelp => p(Sfx, 0.65, 0.1, 0.15, 0.15, self, true, Some(14.0)),
            Sound::Growl => p(Sfx, 0.6, 0.06, 0.15, 7.0, self, true, Some(12.0)),
            Sound::MooseBellow => p(Sfx, 0.65, 0.05, 0.1, 6.0, self, true, Some(40.0)),
            Sound::MooseGrunt => p(Sfx, 0.7, 0.06, 0.15, 4.0, self, true, Some(18.0)),
            Sound::Hoof => p(Sfx, 0.8, 0.08, 0.2, 0.2, self, false, Some(20.0)),
            Sound::IceCrack => p(Sfx, 0.9, 0.04, 0.05, 0.5, self, false, None),
            Sound::IceCreak => p(Sfx, 0.7, 0.08, 0.15, 1.5, self, true, None),
            Sound::Geiger => p(Sfx, 0.3, 0.12, 0.35, 0.06, self, false, None),
            Sound::PickupAmmo | Sound::PickupMed | Sound::PickupFood | Sound::PickupScrap => {
                p(Sfx, 0.55, 0.05, 0.1, 0.12, self, false, None)
            }
            Sound::Craft => p(Sfx, 0.55, 0.03, 0.1, 0.3, self, false, None),
            Sound::ContainerOpen => p(Sfx, 0.7, 0.05, 0.1, 0.3, self, false, Some(12.0)),
            Sound::WindLow => p(Ambience, 1.0, 0.0, 0.0, 0.0, self, false, None),
            Sound::WindMid => p(Ambience, 1.0, 0.0, 0.0, 0.0, self, false, None),
            Sound::WindHigh => p(Ambience, 1.0, 0.0, 0.0, 0.0, self, false, None),
            Sound::Gust => p(Ambience, 0.7, 0.1, 0.25, 5.0, self, false, None),
            Sound::Fire => p(Ambience, 0.9, 0.0, 0.0, 0.0, self, false, Some(5.0)),
            Sound::Siren => p(Ambience, 0.5, 0.0, 0.0, 0.0, self, false, None),
            Sound::MusicCalm | Sound::MusicTense | Sound::MusicDanger => p(Music, 1.0, 0.0, 0.0, 0.0, self, false, None),
            Sound::PipOn | Sound::PipOff => p(Sfx, 0.45, 0.0, 0.0, 0.1, self, false, None),
            Sound::UiTab => p(Sfx, 0.4, 0.04, 0.05, 0.04, self, false, None),
            Sound::PipStatic => p(Sfx, 0.35, 0.05, 0.1, 0.05, self, false, None),
            Sound::PipScroll => p(Sfx, 0.3, 0.06, 0.1, 0.03, self, false, None),
            Sound::PipHum => p(Sfx, 0.25, 0.0, 0.0, 0.0, self, false, None),
        }
    }

    /// Mono samples in -1..=1 for one variant (taken modulo the variant count).
    pub fn samples(self, variant: usize) -> Vec<f32> {
        let v = variant % self.variants();
        let seed = (self as u64 + 1) * 1_000 + v as u64 * 7_919 + 13;
        let rng = Rng::new(seed);
        let vf = v as f32 / self.variants().max(2).saturating_sub(1) as f32; // 0..=1 across the variants
        match self {
            Sound::StepSnow => step_snow(rng),
            Sound::StepIce => step_ice(rng),
            Sound::StepRoad => step_road(rng),
            Sound::StepConcrete => step_concrete(rng),
            Sound::StepWood => step_wood(rng),
            Sound::RifleShot => shot(rng, &Shot::rifle(vf)),
            Sound::ShotgunShot => shot(rng, &Shot::shotgun(vf)),
            Sound::RevolverShot => shot(rng, &Shot::revolver(vf)),
            Sound::DryClick => click(rng, 0.45 + 0.15 * vf, 2000.0 + 700.0 * vf),
            Sound::Jam => jam(rng, vf),
            Sound::ShellTink => shell_tink(rng, vf),
            Sound::ClunkOut => clunk(rng, true, vf),
            Sound::ClunkIn => clunk(rng, false, vf),
            Sound::BoltRack => bolt_rack(rng, vf),
            Sound::BreakOpen => hinge(rng, true, vf),
            Sound::BreakClose => hinge(rng, false, vf),
            Sound::CylinderSpin => cylinder_spin(rng, vf),
            Sound::Swing => swing(rng, vf),
            Sound::MeleeHit => melee_hit(rng, vf),
            Sound::HowlNear => howl(rng, &Howl::near(v)),
            Sound::HowlFar => howl(rng, &Howl::far(v)),
            Sound::Snarl => snarl(rng, vf),
            Sound::Yelp => yelp(rng, vf),
            Sound::Growl => growl(rng, vf),
            Sound::MooseBellow => moose_bellow(rng, vf),
            Sound::MooseGrunt => moose_grunt(rng, vf),
            Sound::Hoof => hoof(rng, vf),
            Sound::IceCrack => ice_crack(rng),
            Sound::IceCreak => ice_creak(rng, vf),
            Sound::Geiger => geiger(rng, vf),
            Sound::PickupAmmo => pickup_ammo(rng, vf),
            Sound::PickupMed => pickup_med(vf),
            Sound::PickupFood => pickup_food(rng, vf),
            Sound::PickupScrap => pickup_scrap(rng),
            Sound::Craft => craft(vf),
            Sound::ContainerOpen => container_open(rng, vf),
            Sound::WindLow => wind_layer(rng, WindLayer::Low),
            Sound::WindMid => wind_layer(rng, WindLayer::Mid),
            Sound::WindHigh => wind_layer(rng, WindLayer::High),
            Sound::Gust => gust(rng, vf),
            Sound::Fire => fire_loop(rng, [6.1, 8.3, 11.0][v]),
            Sound::Siren => siren(),
            Sound::MusicCalm => music_calm(rng),
            Sound::MusicTense => music_tense(rng),
            Sound::MusicDanger => music_danger(rng),
            Sound::PipOn => pip_blip(true),
            Sound::PipOff => pip_blip(false),
            Sound::UiTab => ui_tick(),
            Sound::PipStatic => pip_static(rng),
            Sound::PipScroll => pip_scroll(),
            Sound::PipHum => pip_hum(rng),
        }
    }

    pub fn wav(self, variant: usize) -> Vec<u8> {
        encode_wav(&self.samples(variant))
    }
}

// ---------------------------------------------------------------------------
// DSP helpers
// ---------------------------------------------------------------------------

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

/// Band-pass made from two low-passes.
struct BandPass {
    hi: LowPass,
    lo: LowPass,
}

impl BandPass {
    fn new(low_hz: f32, high_hz: f32) -> Self {
        BandPass {
            hi: LowPass::new(high_hz),
            lo: LowPass::new(low_hz),
        }
    }
    fn run(&mut self, x: f32) -> f32 {
        self.hi.run(x) - self.lo.run(x)
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

/// Add `clip` into `out` starting at `secs`, scaled by `gain`.
fn mix_at(out: &mut [f32], secs: f32, clip: &[f32], gain: f32) {
    let start = len(secs);
    for (j, v) in clip.iter().enumerate() {
        if let Some(o) = out.get_mut(start + j) {
            *o += v * gain;
        }
    }
}

/// Scale so the loudest sample is `peak`.
fn normalize(mut s: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if m > 1e-6 {
        for v in &mut s {
            *v *= peak / m;
        }
    }
    s
}

/// A simple echo / room tail: `delay` seconds, `feedback` per repeat.
fn echo(s: &mut Vec<f32>, delay: f32, feedback: f32, mix: f32, tail: f32) {
    let d = len(delay).max(1);
    s.extend(std::iter::repeat(0.0).take(len(tail)));
    let dry = s.clone();
    let mut fb = vec![0.0f32; s.len()];
    for i in 0..s.len() {
        let wet = if i >= d { dry[i - d] + fb[i - d] * feedback } else { 0.0 };
        fb[i] = wet;
        s[i] += wet * mix;
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

fn fade_edges(s: &mut [f32], secs: f32) {
    let n = len(secs).min(s.len() / 2).max(1);
    let total = s.len();
    for i in 0..n {
        let k = i as f32 / n as f32;
        s[i] *= k;
        s[total - 1 - i] *= k;
    }
}

// ---------------------------------------------------------------------------
// Footsteps
// ---------------------------------------------------------------------------

/// Two-part footfall: heel then toe, with random timing and weight.
fn footfall(
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

fn step_snow(mut rng: Rng) -> Vec<f32> {
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

fn step_ice(mut rng: Rng) -> Vec<f32> {
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

fn step_road(mut rng: Rng) -> Vec<f32> {
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

fn step_concrete(mut rng: Rng) -> Vec<f32> {
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

fn step_wood(mut rng: Rng) -> Vec<f32> {
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

// ---------------------------------------------------------------------------
// Guns
// ---------------------------------------------------------------------------

struct Shot {
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
    amp: f32,
}

impl Shot {
    fn rifle(v: f32) -> Shot {
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
            amp: 0.95,
        }
    }
    fn shotgun(v: f32) -> Shot {
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
            amp: 1.0,
        }
    }
    fn revolver(v: f32) -> Shot {
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
            amp: 0.9,
        }
    }
}

fn shot(mut rng: Rng, p: &Shot) -> Vec<f32> {
    let total = len(p.tail_secs + 0.2);
    let mut crack_lp = LowPass::new(p.crack_lp);
    let mut tail_lp = LowPass::new(p.tail_lp);
    let mut out: Vec<f32> = (0..total)
        .map(|i| {
            let t = i as f32 / SR;
            let crack = crack_lp.run(noise(&mut rng)) * env(t, 0.0008, p.crack_decay);
            let thump = (TAU * (p.thump_hz - 40.0 * t) * t).sin() * env(t, 0.002, p.thump_decay);
            let ring = (TAU * p.ring_hz * t).sin() * env(t, 0.001, 16.0) * p.ring_amp;
            let tail = tail_lp.run(noise(&mut rng)) * env(t, 0.02, p.tail_decay) * p.tail_amp * 3.0;
            crack * 1.5 + thump * 0.8 + ring + tail
        })
        .collect();
    echo(&mut out, 0.085, 0.4, 0.22, 0.25);
    for v in &mut out {
        *v = v.tanh();
    }
    fade_edges(&mut out, 0.01);
    normalize(out, p.amp)
}

fn click(mut rng: Rng, amp: f32, tone: f32) -> Vec<f32> {
    (0..len(0.07))
        .map(|i| {
            let t = i as f32 / SR;
            (noise(&mut rng) * 0.5 + (TAU * tone * t).sin()) * env(t, 0.0005, 90.0) * amp
        })
        .collect()
}

fn jam(mut rng: Rng, v: f32) -> Vec<f32> {
    let f = 800.0 + 300.0 * v;
    (0..len(0.35))
        .map(|i| {
            let t = i as f32 / SR;
            let clank = (TAU * f * t).sin() + 0.7 * (TAU * f * 1.51 * t).sin() + 0.4 * (TAU * f * 2.38 * t).sin();
            (clank * 0.35 + noise(&mut rng) * 0.3) * env(t, 0.001, 14.0) * 0.7
        })
        .collect()
}

fn shell_tink(mut rng: Rng, v: f32) -> Vec<f32> {
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
fn clunk(mut rng: Rng, out_: bool, v: f32) -> Vec<f32> {
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

fn bolt_rack(mut rng: Rng, v: f32) -> Vec<f32> {
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
fn hinge(mut rng: Rng, open: bool, v: f32) -> Vec<f32> {
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

fn cylinder_spin(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn swing(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn melee_hit(mut rng: Rng, v: f32) -> Vec<f32> {
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

// ---------------------------------------------------------------------------
// Wolves and the moose
// ---------------------------------------------------------------------------

struct Howl {
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
    fn near(v: usize) -> Howl {
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
    fn far(v: usize) -> Howl {
        let mut h = Howl::near(v + 1);
        h.secs += 1.0;
        h.cutoff = 900.0;
        h.amp = 0.3;
        h.echo = true;
        h
    }
}

fn howl(mut rng: Rng, p: &Howl) -> Vec<f32> {
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

fn snarl(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn yelp(rng: Rng, v: f32) -> Vec<f32> {
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

fn growl(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn moose_bellow(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn moose_grunt(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn hoof(mut rng: Rng, v: f32) -> Vec<f32> {
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

// ---------------------------------------------------------------------------
// Ice, rads, items
// ---------------------------------------------------------------------------

fn ice_crack(mut rng: Rng) -> Vec<f32> {
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

fn ice_creak(mut rng: Rng, v: f32) -> Vec<f32> {
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
fn geiger(mut rng: Rng, v: f32) -> Vec<f32> {
    let f = 2400.0 + 1100.0 * v;
    let out: Vec<f32> = (0..len(0.03))
        .map(|i| {
            let t = i as f32 / SR;
            (noise(&mut rng) * 0.7 + (TAU * f * t).sin() * 0.5) * env(t, 0.0003, 260.0)
        })
        .collect();
    normalize(out, 0.5)
}

fn pickup_ammo(mut rng: Rng, v: f32) -> Vec<f32> {
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
fn note(f: f32, secs: f32, decay: f32) -> Vec<f32> {
    (0..len(secs))
        .map(|i| {
            let t = i as f32 / SR;
            ((TAU * f * t).sin() + 0.2 * (TAU * f * 2.0 * t).sin()) * env(t, 0.004, decay)
        })
        .collect()
}

fn pickup_med(v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.5)];
    let base = 740.0 * (1.0 + 0.08 * v);
    mix_at(&mut out, 0.0, &note(base, 0.3, 11.0), 0.5);
    mix_at(&mut out, 0.1, &note(base * 1.5, 0.35, 9.0), 0.45);
    normalize(out, 0.4)
}

fn pickup_food(mut rng: Rng, v: f32) -> Vec<f32> {
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

fn pickup_scrap(mut rng: Rng) -> Vec<f32> {
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

fn craft(v: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(0.9)];
    let base = 520.0 * (1.0 + 0.1 * v);
    for (k, ratio) in [1.0, 1.25, 1.5].into_iter().enumerate() {
        mix_at(&mut out, k as f32 * 0.14, &note(base * ratio, 0.4, 7.0), 0.45);
    }
    normalize(out, 0.4)
}

fn container_open(mut rng: Rng, v: f32) -> Vec<f32> {
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

// ---------------------------------------------------------------------------
// Ambience
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum WindLayer {
    Low,
    Mid,
    High,
}

/// One layer of the wind. The three layers have different lengths (23, 29
/// and 37 seconds), so played together their combined repeat is about
/// 4 hours long: the loop can't be heard.
fn wind_layer(mut rng: Rng, layer: WindLayer) -> Vec<f32> {
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
fn gust(mut rng: Rng, v: f32) -> Vec<f32> {
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
fn fire_loop(mut rng: Rng, secs: f32) -> Vec<f32> {
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

fn siren() -> Vec<f32> {
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

// ---------------------------------------------------------------------------
// Music
// ---------------------------------------------------------------------------

fn semis(semitones_from_a4: f32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 / 12.0)
}

/// Sparse glassy bell notes dropped over a bed.
fn bells(out: &mut [f32], rng: &mut Rng, scale: &[f32], secs: f32, gap: (f32, f32), amp: f32) {
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
fn pad(out: &mut [f32], chords: &[[f32; 4]], chord_secs: f32, gain: f32, cutoff: f32) {
    let mut lp = LowPass::new(cutoff);
    let mut phases = [[0.0f32; 2]; 4];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let pos = (t / chord_secs) % chords.len() as f32;
        let idx = pos as usize % chords.len();
        let next = (idx + 1) % chords.len();
        let frac = pos - pos.floor();
        // Glide between chords over the last 2 seconds.
        let blend = ((frac * chord_secs - (chord_secs - 2.0)) / 2.0).clamp(0.0, 1.0);
        let mut v = 0.0;
        for k in 0..4 {
            let f = semis(chords[idx][k] + (chords[next][k] - chords[idx][k]) * blend);
            for (d, detune) in [0.997f32, 1.003].iter().enumerate() {
                phases[k][d] = (phases[k][d] + f * detune / SR).fract();
                v += (2.0 * phases[k][d] - 1.0) * if k == 0 { 0.7 } else { 0.45 };
            }
        }
        let breathe = 0.7 + 0.3 * (TAU * t / (chord_secs * 2.0)).sin();
        lp.set(cutoff * (0.6 + 0.4 * breathe));
        *o += lp.run(v) * gain * breathe;
    }
}

/// "The Long Winter": a slow, cold pad in D minor with sparse glass notes.
fn music_calm(mut rng: Rng) -> Vec<f32> {
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
fn music_tense(mut rng: Rng) -> Vec<f32> {
    let secs = 60.0;
    let total = len(secs + 3.0);
    let mut out = vec![0.0f32; total];
    let mut lp = LowPass::new(220.0);
    let mut p = [0.0f32; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let mut v = 0.0;
        for (k, f) in [semis(-43.0), semis(-43.0) * 1.0595, semis(-31.0) * 1.414].into_iter().enumerate() {
            p[k] = (p[k] + f / SR).fract();
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
fn music_danger(mut rng: Rng) -> Vec<f32> {
    let secs = 48.0;
    let total = len(secs + 2.0);
    let mut out = vec![0.0f32; total];
    let step = 0.375; // eighth notes at 80 bpm
    let notes = [-43.0, -43.0, -43.0, -40.0, -43.0, -43.0, -41.0, -43.0];
    let mut phase = 0.0;
    let mut lp = LowPass::new(500.0);
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let n = (t / step) as usize;
        let local = t - n as f32 * step;
        let f = semis(notes[n % notes.len()]);
        phase = (phase + f / SR).fract();
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

// ---------------------------------------------------------------------------
// Pip-Boy interface
// ---------------------------------------------------------------------------

fn pip_blip(on: bool) -> Vec<f32> {
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
fn pip_static(mut rng: Rng) -> Vec<f32> {
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
fn pip_scroll() -> Vec<f32> {
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
fn pip_hum(mut rng: Rng) -> Vec<f32> {
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

fn ui_tick() -> Vec<f32> {
    let out: Vec<f32> = (0..len(0.05))
        .map(|i| {
            let t = i as f32 / SR;
            (TAU * 1400.0 * t).sin() * env(t, 0.001, 70.0)
        })
        .collect();
    normalize(out, 0.35)
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

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
    }

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
    fn the_list_covers_every_sound_once() {
        for (i, s) in Sound::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i, "{s:?} is out of order in Sound::ALL");
        }
        assert_eq!(Sound::UiTab as usize + 1, Sound::ALL.len());
    }

    #[test]
    fn every_variant_is_audible_finite_and_never_clips() {
        for &sound in Sound::ALL {
            for v in 0..sound.variants() {
                let s = sound.samples(v);
                assert!(s.len() > 500, "{sound:?}/{v} too short");
                assert!(s.iter().all(|x| x.is_finite()), "{sound:?}/{v} has NaN");
                let p = peak(&s);
                assert!(p > 0.02, "{sound:?}/{v} is silent (peak {p})");
                assert!(p <= 1.0 + 1e-3, "{sound:?}/{v} clips (peak {p})");
            }
        }
    }

    #[test]
    fn variants_really_differ() {
        for &sound in Sound::ALL {
            if sound.variants() < 2 {
                continue;
            }
            let a = sound.samples(0);
            let b = sound.samples(1);
            let same = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6);
            assert!(!same, "{sound:?} variants 0 and 1 are identical");
        }
    }

    #[test]
    fn frequent_sounds_have_enough_variants() {
        for s in [Sound::StepSnow, Sound::StepIce, Sound::StepRoad, Sound::StepConcrete, Sound::StepWood] {
            assert!(s.variants() >= 6, "{s:?}");
        }
        for s in [Sound::RifleShot, Sound::Snarl, Sound::Yelp, Sound::HowlNear, Sound::HowlFar, Sound::Geiger] {
            assert!(s.variants() >= 3, "{s:?}");
        }
    }

    #[test]
    fn loops_are_long_and_wind_layers_do_not_line_up() {
        let secs = |s: Sound| s.samples(0).len() as f32 / SAMPLE_RATE as f32;
        assert!(secs(Sound::WindLow) > 20.0 && secs(Sound::WindMid) > 26.0 && secs(Sound::WindHigh) > 34.0);
        // Co-prime lengths (23, 29, 37 s) mean the combined wind repeats only
        // after 23 * 29 * 37 seconds (about 6.9 hours).
        let l: Vec<u32> = [Sound::WindLow, Sound::WindMid, Sound::WindHigh].iter().map(|s| secs(*s).round() as u32).collect();
        assert_eq!(l, vec![23, 29, 37]);
        for fire in 0..3 {
            assert!(Sound::Fire.samples(fire).len() as f32 / SAMPLE_RATE as f32 > 5.5);
        }
        for m in [Sound::MusicCalm, Sound::MusicTense, Sound::MusicDanger] {
            assert!(secs(m) >= 45.0, "{m:?}");
        }
    }

    #[test]
    fn loops_are_seamless() {
        for s in [Sound::WindLow, Sound::WindMid, Sound::WindHigh, Sound::MusicCalm, Sound::MusicTense, Sound::MusicDanger, Sound::Fire] {
            for v in 0..s.variants() {
                let w = s.samples(v);
                // The seam must look like any other step between samples.
                let jump = (w[0] - w[w.len() - 1]).abs();
                let biggest_step = w.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0f32, f32::max);
                assert!(jump <= biggest_step + 1e-6, "{s:?}/{v} jumps by {jump} at the seam (normal steps reach {biggest_step})");
            }
        }
    }

    #[test]
    fn mix_levels_are_sane() {
        // Music and ambience sit well under the effects.
        let bed = rms(&Sound::MusicCalm.samples(0)) * Sound::MusicCalm.profile().volume;
        let shot = rms(&Sound::RifleShot.samples(0)[..4000]) * Sound::RifleShot.profile().volume;
        assert!(bed < shot * 0.6, "music {bed} vs shot {shot}");
        // The far howl is quieter than the near one.
        assert!(peak(&Sound::HowlFar.samples(0)) * Sound::HowlFar.profile().volume < peak(&Sound::HowlNear.samples(0)) * Sound::HowlNear.profile().volume);
        // The Geiger tick is quiet and very short.
        let g = Sound::Geiger.samples(0);
        assert!(g.len() < len(0.05) && Sound::Geiger.profile().volume <= 0.35);
    }

    #[test]
    fn profiles_make_sense() {
        for &s in Sound::ALL {
            let p = s.profile();
            assert!((0.0..=1.0).contains(&p.volume), "{s:?}");
            assert!(p.pitch < 0.2 && p.vary < 0.5, "{s:?}");
            assert_eq!(p.group.profile().group, p.group, "{s:?}'s group must be its own group");
            if s.is_loop() {
                assert_eq!(p.pitch, 0.0, "{s:?} loops must not wobble in pitch");
            }
        }
        // Howls share one cooldown so a pack can't stack them.
        assert_eq!(Sound::HowlFar.profile().group, Sound::HowlNear);
    }
}
