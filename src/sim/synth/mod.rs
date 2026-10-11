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

mod creatures;
mod dsp;
mod music;
mod steps;
mod ui;
mod weapons;
mod world;

use self::creatures::*;
use self::dsp::*;
use self::music::*;
use self::steps::*;
use self::ui::*;
use self::weapons::*;
use self::world::*;

#[cfg(test)]
mod tests;

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
    // Rad-crows.
    Caw,
    // Gunfire's echo: the open snowfield's long, rolling answer, or a room's short slap.
    ShotTailOutdoor,
    ShotTailIndoor,
    // Mechanical clicks after a shot.
    BoltClack,
    HammerCock,
    // A calm breeze and a howling blizzard, to layer over the wind loops.
    WindBreeze,
    WindHowl,
    // Inside: machinery hum, drips, creaking timber.
    RoomHum,
    Drip,
    Creak,
    // The raiders' voices.
    RaiderShout,
    RaiderGrunt,
    // Boots: a running crunch and the squeak of dry snow in deep cold.
    StepSnowRun,
    StepSnowSqueak,
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
        Sound::Caw,
        Sound::ShotTailOutdoor,
        Sound::ShotTailIndoor,
        Sound::BoltClack,
        Sound::HammerCock,
        Sound::WindBreeze,
        Sound::WindHowl,
        Sound::RoomHum,
        Sound::Drip,
        Sound::Creak,
        Sound::RaiderShout,
        Sound::RaiderGrunt,
        Sound::StepSnowRun,
        Sound::StepSnowSqueak,
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
            Sound::Caw => 4,
            Sound::ShotTailOutdoor | Sound::ShotTailIndoor | Sound::BoltClack | Sound::HammerCock => 2,
            Sound::Drip | Sound::Creak | Sound::RaiderGrunt => 3,
            Sound::RaiderShout | Sound::StepSnowRun | Sound::StepSnowSqueak => 4,
            Sound::WindBreeze | Sound::WindHowl | Sound::RoomHum => 1,
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

    #[cfg(test)]
    /// True for sounds that are meant to loop.
    pub fn is_loop(self) -> bool {
        matches!(
            self,
            Sound::WindLow
                | Sound::WindMid
                | Sound::WindHigh
                | Sound::WindBreeze
                | Sound::WindHowl
                | Sound::RoomHum
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
            Sound::Caw => p(Sfx, 0.55, 0.1, 0.2, 0.3, self, true, Some(28.0)),
            Sound::ShotTailOutdoor | Sound::ShotTailIndoor => p(Sfx, 0.5, 0.03, 0.1, 0.0, self, false, None),
            Sound::BoltClack | Sound::HammerCock => p(Sfx, 0.5, 0.05, 0.15, 0.1, self, false, None),
            Sound::WindBreeze | Sound::WindHowl | Sound::RoomHum => p(Ambience, 1.0, 0.0, 0.0, 0.0, self, false, None),
            Sound::Drip => p(Sfx, 0.45, 0.1, 0.3, 1.0, self, false, None),
            Sound::Creak => p(Sfx, 0.5, 0.08, 0.25, 2.0, self, false, None),
            Sound::RaiderShout => p(Sfx, 0.75, 0.07, 0.15, 1.5, self, true, Some(30.0)),
            Sound::RaiderGrunt => p(Sfx, 0.7, 0.08, 0.15, 0.2, self, true, Some(18.0)),
            Sound::StepSnowRun => p(Sfx, 0.85, 0.08, 0.2, 0.1, self, false, None),
            Sound::StepSnowSqueak => p(Sfx, 0.5, 0.1, 0.2, 0.1, self, false, None),
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
            Sound::Caw => caw(rng, vf),
            Sound::ShotTailOutdoor => shot_tail(rng, false, vf),
            Sound::ShotTailIndoor => shot_tail(rng, true, vf),
            Sound::BoltClack => bolt_clack(rng, vf),
            Sound::HammerCock => hammer_cock(rng, vf),
            Sound::WindBreeze => wind_breeze(rng),
            Sound::WindHowl => wind_howl(rng),
            Sound::RoomHum => room_hum(rng),
            Sound::Drip => drip(rng, vf),
            Sound::Creak => creak(rng, vf),
            Sound::RaiderShout => raider_voice(rng, vf, true),
            Sound::RaiderGrunt => raider_voice(rng, vf, false),
            Sound::StepSnowRun => step_snow_run(rng),
            Sound::StepSnowSqueak => step_snow_squeak(rng, vf),
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

/// The sound as heard through a wall, a door or a thicket: the highs gone,
/// the body dulled. (Two cascaded one-pole low-passes.)
pub fn muffled(samples: &[f32]) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let mut a = LowPass::new(900.0);
    let mut b = LowPass::new(900.0);
    let mut out: Vec<f32> = samples.iter().map(|&x| b.run(a.run(x))).collect();
    let after = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if after > 1e-6 {
        // Keep roughly the original loudness (a muffled shot is still a loud shot,
        // just a dull one), but never louder than before.
        let k = (peak / after).min(2.5);
        for v in &mut out {
            *v *= k;
        }
    }
    out
}

impl Sound {
    /// Does it matter what's between you and it? Gunfire, voices and boots
    /// get a muffled version for when you hear them through a wall or trees.
    pub fn muffleable(self) -> bool {
        matches!(
            self,
            Sound::RifleShot
                | Sound::ShotgunShot
                | Sound::RevolverShot
                | Sound::ShotTailOutdoor
                | Sound::ShotTailIndoor
                | Sound::BoltClack
                | Sound::HammerCock
                | Sound::Snarl
                | Sound::Yelp
                | Sound::Growl
                | Sound::HowlNear
                | Sound::HowlFar
                | Sound::MooseBellow
                | Sound::MooseGrunt
                | Sound::Caw
                | Sound::RaiderShout
                | Sound::RaiderGrunt
                | Sound::StepSnow
                | Sound::StepSnowRun
                | Sound::StepSnowSqueak
                | Sound::StepIce
                | Sound::StepRoad
                | Sound::StepConcrete
                | Sound::StepWood
                | Sound::Hoof
        )
    }

    /// The same clip as heard through something in the way.
    pub fn samples_muffled(self, variant: usize) -> Vec<f32> {
        muffled(&self.samples(variant))
    }

    pub fn wav_muffled(self, variant: usize) -> Vec<u8> {
        encode_wav(&self.samples_muffled(variant))
    }

    /// A clip and, for sounds that [`muffleable`](Self::muffleable), its
    /// muffled twin, from a single run of the synthesiser: the muffled clip is
    /// the plain one put through a filter, so there is no need to build it
    /// twice. Byte for byte what [`wav`](Self::wav) and
    /// [`wav_muffled`](Self::wav_muffled) return, in about half the time for
    /// the 95 clips that have both.
    pub fn wav_pair(self, variant: usize) -> (Vec<u8>, Option<Vec<u8>>) {
        let samples = self.samples(variant);
        let muffled = self.muffleable().then(|| encode_wav(&muffled(&samples)));
        (encode_wav(&samples), muffled)
    }
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
