//! The rules for *how* sounds are played, kept free of Bevy so they can be
//! unit-tested: picking a variant that isn't the one just played, random
//! pitch and volume, per-sound cooldowns, repeat fatigue, the player's
//! volume sliders, the music director that crossfades moods and leaves long
//! silences, the Geiger-counter click rate, and the reload sound cues.

use std::collections::HashMap;

use super::rng::Rng;
use super::synth::{Bus, Sound};
use super::combat::WeaponKind;
use super::survival::Item;
use super::terrain::Surface;

/// The sound of picking an item up.
pub fn pickup_sound(item: Item) -> Sound {
    match item {
        Item::Ammo | Item::Shells | Item::RevolverRounds => Sound::PickupAmmo,
        Item::Scrap => Sound::PickupScrap,
        Item::Stimpak | Item::RadAway => Sound::PickupMed,
        Item::Hotdish => Sound::PickupFood,
    }
}

/// The footstep sound for a surface.
pub fn step_sound(surface: Surface) -> Sound {
    match surface {
        Surface::Snow => Sound::StepSnow,
        Surface::Ice => Sound::StepIce,
        Surface::Road => Sound::StepRoad,
        Surface::Concrete => Sound::StepConcrete,
        Surface::Wood => Sound::StepWood,
    }
}

/// Move `current` towards `target`, at most `rate` per second.
pub fn approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let step = rate * dt;
    if (target - current).abs() <= step {
        target
    } else {
        current + step * (target - current).signum()
    }
}

// ---------------------------------------------------------------------------
// Variants, jitter, cooldowns
// ---------------------------------------------------------------------------

/// How one particular play of a sound should be rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub variant: usize,
    /// Playback speed (1.0 = unchanged); changes pitch.
    pub speed: f32,
    /// Final volume 0..1 before the player's sliders.
    pub volume: f32,
}

/// Chooses variants, wobbles pitch and volume, and enforces cooldowns.
#[derive(Default)]
pub struct Player {
    last_variant: HashMap<Sound, usize>,
    last_played: HashMap<Sound, f32>,
    fatigue: HashMap<Sound, f32>,
    fatigue_at: HashMap<Sound, f32>,
}

/// How much quieter each immediate repeat gets, and how fast that recovers.
const FATIGUE_PER_PLAY: f32 = 0.16;
const FATIGUE_RECOVERY: f32 = 0.25;
const FATIGUE_MAX: f32 = 0.5;

impl Player {
    pub fn new() -> Self {
        Player::default()
    }

    /// A random variant that is never the same as the previous one.
    pub fn pick(&mut self, sound: Sound, rng: &mut Rng) -> usize {
        let n = sound.variants();
        if n <= 1 {
            return 0;
        }
        // Choose among the other n-1 variants.
        let mut v = (rng.f32() * (n - 1) as f32) as usize % (n - 1);
        if let Some(&last) = self.last_variant.get(&sound) {
            if v >= last {
                v += 1;
            }
        }
        self.last_variant.insert(sound, v);
        v
    }

    /// Current fatigue (0 = fresh) for a sound group at time `now`.
    fn fatigue_now(&self, group: Sound, now: f32) -> f32 {
        let f = self.fatigue.get(&group).copied().unwrap_or(0.0);
        let at = self.fatigue_at.get(&group).copied().unwrap_or(now);
        (f - (now - at) * FATIGUE_RECOVERY).max(0.0)
    }

    /// Decide whether `sound` may play at `now` (seconds), and how.
    /// `gain` scales the volume (a quiet footstep, a muffled shot).
    pub fn request(&mut self, sound: Sound, now: f32, gain: f32, rng: &mut Rng) -> Option<Play> {
        let p = sound.profile();
        if let Some(&t) = self.last_played.get(&p.group) {
            if now - t < p.cooldown {
                return None;
            }
        }
        let fatigue = if p.fatigue { self.fatigue_now(p.group, now).min(FATIGUE_MAX) } else { 0.0 };
        self.last_played.insert(p.group, now);
        if p.fatigue {
            let f = (self.fatigue_now(p.group, now) + FATIGUE_PER_PLAY).min(FATIGUE_MAX);
            self.fatigue.insert(p.group, f);
            self.fatigue_at.insert(p.group, now);
        }
        let variant = self.pick(sound, rng);
        let speed = 1.0 + p.pitch * (rng.f32() * 2.0 - 1.0);
        let wobble = 1.0 + p.vary * (rng.f32() * 2.0 - 1.0);
        Some(Play {
            variant,
            speed,
            volume: (p.volume * wobble * (1.0 - fatigue) * gain).clamp(0.0, 1.0),
        })
    }
}

// ---------------------------------------------------------------------------
// Mixer
// ---------------------------------------------------------------------------

/// The player's volume sliders and mute switch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix {
    pub master: f32,
    pub sfx: f32,
    pub ambience: f32,
    pub music: f32,
    pub muted: bool,
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            master: 0.8,
            sfx: 1.0,
            ambience: 0.8,
            music: 0.6,
            muted: false,
        }
    }
}

impl Mix {
    /// Linear gain for a bus.
    pub fn gain(&self, bus: Bus) -> f32 {
        if self.muted {
            return 0.0;
        }
        self.master
            * match bus {
                Bus::Sfx => self.sfx,
                Bus::Ambience => self.ambience,
                Bus::Music => self.music,
            }
    }

    /// Nudge a slider by `delta`, staying in 0..=1.
    pub fn nudge(slider: &mut f32, delta: f32) {
        *slider = ((*slider + delta) * 10.0).round() / 10.0;
        *slider = slider.clamp(0.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// Music
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Calm,
    Tense,
    Danger,
}

impl Mood {
    pub const ALL: [Mood; 3] = [Mood::Calm, Mood::Tense, Mood::Danger];

    pub fn sound(self) -> Sound {
        match self {
            Mood::Calm => Sound::MusicCalm,
            Mood::Tense => Sound::MusicTense,
            Mood::Danger => Sound::MusicDanger,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What is going on that should colour the music.
#[derive(Clone, Copy, Debug, Default)]
pub struct MusicContext {
    pub blizzard: bool,
    pub siren: bool,
    /// Distance to the nearest hunting enemy, if any.
    pub enemy_dist: Option<f32>,
}

/// Decides which music is audible: calm music comes and goes in windows with
/// long silences between them; tension and danger take over when something
/// threatens, hold for a while so they don't flicker, and fade out again.
#[derive(Debug)]
pub struct MusicDirector {
    /// Calm music is in a "play" window (otherwise it is a silent gap).
    window_open: bool,
    window_timer: f32,
    /// Seconds a threat mood must still be held.
    hold: f32,
    mood: Mood,
    pub levels: [f32; 3],
}

/// Levels each mood plays at (the music slider is applied later).
const LEVEL: [f32; 3] = [0.8, 0.9, 1.0];

impl MusicDirector {
    pub fn new(rng: &mut Rng) -> Self {
        MusicDirector {
            window_open: false,
            // The game opens quietly: wind only, then music drifts in.
            window_timer: rng.range(20.0, 40.0),
            hold: 0.0,
            mood: Mood::Calm,
            levels: [0.0; 3],
        }
    }

    pub fn mood(&self) -> Mood {
        self.mood
    }

    pub fn update(&mut self, dt: f32, ctx: &MusicContext, rng: &mut Rng) -> [f32; 3] {
        let threat = if ctx.blizzard {
            Some(Mood::Danger)
        } else if ctx.siren || ctx.enemy_dist.is_some_and(|d| d < 45.0) {
            Some(Mood::Tense)
        } else {
            None
        };
        if let Some(m) = threat {
            // A stronger mood always wins; hold it for a while.
            if self.hold <= 0.0 || m as usize >= self.mood as usize {
                self.mood = m;
            }
            self.hold = if m == Mood::Danger { 12.0 } else { 20.0 };
        } else if self.hold > 0.0 {
            self.hold -= dt;
        }

        let threatened = self.hold > 0.0;
        if !threatened {
            self.mood = Mood::Calm;
            self.window_timer -= dt;
            if self.window_timer <= 0.0 {
                self.window_open = !self.window_open;
                self.window_timer = if self.window_open {
                    rng.range(80.0, 150.0)
                } else {
                    rng.range(90.0, 220.0)
                };
            }
        }

        for m in Mood::ALL {
            let want = if threatened {
                if m == self.mood { LEVEL[m.index()] } else { 0.0 }
            } else if m == Mood::Calm && self.window_open {
                LEVEL[0]
            } else {
                0.0
            };
            let rate = if want > self.levels[m.index()] { 0.25 } else { 0.16 };
            self.levels[m.index()] = approach(self.levels[m.index()], want, rate, dt);
        }
        self.levels
    }
}

// ---------------------------------------------------------------------------
// Geiger counter
// ---------------------------------------------------------------------------

/// Clicks per second for a radiation rate in rads/second: silent in clean
/// air, a lazy tick on the ice, a busy but never frantic patter in a
/// blizzard or by a crater.
pub fn geiger_clicks_per_sec(rads_per_sec: f32) -> f32 {
    if rads_per_sec < 0.2 {
        return 0.0;
    }
    (0.55 * rads_per_sec.powf(0.8)).min(7.0)
}

// ---------------------------------------------------------------------------
// Reload cues
// ---------------------------------------------------------------------------

/// A sound to play at a moment during a reload animation: (progress 0..1, sound).
pub type Cue = (f32, Sound);

/// Sounds for reloading (or clearing a frozen action), timed to the
/// animation in `sim::viewmodel`.
pub fn cues(kind: WeaponKind, unjamming: bool) -> Vec<Cue> {
    if unjamming {
        return match kind {
            WeaponKind::PipeRifle => vec![(0.04, Sound::Jam), (0.3, Sound::BoltRack), (0.7, Sound::BoltRack)],
            WeaponKind::ScrapShotgun => vec![(0.04, Sound::Jam), (0.22, Sound::BreakOpen), (0.7, Sound::BreakClose)],
            WeaponKind::Revolver => vec![(0.04, Sound::Jam), (0.3, Sound::CylinderSpin), (0.7, Sound::ClunkIn)],
            WeaponKind::IceAxe => vec![],
        };
    }
    match kind {
        // In step with the hands in sim::viewmodel: magazine out, seated, bolt.
        WeaponKind::PipeRifle => vec![(0.12, Sound::ClunkOut), (0.7, Sound::ClunkIn), (0.79, Sound::BoltRack)],
        WeaponKind::ScrapShotgun => vec![
            (0.17, Sound::BreakOpen),
            (0.3, Sound::ClunkOut),
            (0.56, Sound::ClunkIn),
            (0.64, Sound::ClunkIn),
            (0.83, Sound::BreakClose),
        ],
        WeaponKind::Revolver => vec![
            (0.18, Sound::ClunkOut),
            (0.38, Sound::ClunkOut),
            (0.64, Sound::ClunkIn),
            (0.8, Sound::CylinderSpin),
        ],
        WeaponKind::IceAxe => vec![],
    }
}

/// The sound of a weapon going off (or being swung).
pub fn fire_sound(kind: WeaponKind) -> Sound {
    match kind {
        WeaponKind::PipeRifle => Sound::RifleShot,
        WeaponKind::ScrapShotgun => Sound::ShotgunShot,
        WeaponKind::Revolver => Sound::RevolverShot,
        WeaponKind::IceAxe => Sound::Swing,
    }
}

/// The cues that fall in the progress interval `(from, to]`.
pub fn cues_between(cues: &[Cue], from: f32, to: f32) -> Vec<Sound> {
    cues.iter().filter(|(at, _)| *at > from && *at <= to).map(|(_, s)| *s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Rng {
        Rng::new(77)
    }

    #[test]
    fn never_the_same_variant_twice_in_a_row() {
        let mut p = Player::new();
        let mut r = rng();
        for sound in [Sound::StepSnow, Sound::RifleShot, Sound::Snarl, Sound::Geiger, Sound::Yelp, Sound::HowlNear] {
            let mut last = None;
            let mut seen = std::collections::HashSet::new();
            for _ in 0..500 {
                let v = p.pick(sound, &mut r);
                assert!(v < sound.variants());
                assert_ne!(Some(v), last, "{sound:?} repeated variant {v}");
                last = Some(v);
                seen.insert(v);
            }
            assert_eq!(seen.len(), sound.variants(), "{sound:?} should use every variant");
        }
    }

    #[test]
    fn single_variant_sounds_always_pick_zero() {
        let mut p = Player::new();
        assert_eq!(p.pick(Sound::Siren, &mut rng()), 0);
        assert_eq!(p.pick(Sound::Siren, &mut rng()), 0);
    }

    #[test]
    fn pitch_and_volume_wobble_stay_in_range() {
        let mut p = Player::new();
        let mut r = rng();
        let prof = Sound::StepSnow.profile();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..400 {
            let play = p.request(Sound::StepSnow, i as f32, 1.0, &mut r).unwrap();
            assert!(play.speed >= 1.0 - prof.pitch - 1e-4 && play.speed <= 1.0 + prof.pitch + 1e-4);
            assert!(play.volume <= 1.0);
            lo = lo.min(play.speed);
            hi = hi.max(play.speed);
        }
        assert!(hi - lo > prof.pitch, "pitch really varies ({lo}..{hi})");
    }

    #[test]
    fn consecutive_plays_are_never_identical() {
        let mut p = Player::new();
        let mut r = rng();
        let mut prev: Option<Play> = None;
        for i in 0..300 {
            let play = p.request(Sound::RifleShot, i as f32 * 0.4, 1.0, &mut r).unwrap();
            if let Some(q) = prev {
                assert_ne!(play, q);
                assert_ne!(play.variant, q.variant);
            }
            prev = Some(play);
        }
    }

    #[test]
    fn cooldowns_block_early_repeats_and_howls_share_one() {
        let mut p = Player::new();
        let mut r = rng();
        assert!(p.request(Sound::Geiger, 10.0, 1.0, &mut r).is_some());
        assert!(p.request(Sound::Geiger, 10.02, 1.0, &mut r).is_none(), "too soon");
        assert!(p.request(Sound::Geiger, 10.07, 1.0, &mut r).is_some());
        // A near howl blocks a far howl (same group) for the cooldown.
        assert!(p.request(Sound::HowlNear, 100.0, 1.0, &mut r).is_some());
        assert!(p.request(Sound::HowlFar, 105.0, 1.0, &mut r).is_none());
        assert!(p.request(Sound::HowlNear, 110.0, 1.0, &mut r).is_none());
        assert!(p.request(Sound::HowlFar, 119.0, 1.0, &mut r).is_some());
        // Gunshots have no cooldown: every trigger pull is heard.
        for k in 0..5 {
            assert!(p.request(Sound::RifleShot, 200.0 + k as f32 * 0.01, 1.0, &mut r).is_some());
        }
    }

    #[test]
    fn rapid_repeats_get_quieter_and_then_recover() {
        let mut p = Player::new();
        let mut r = rng();
        // Fixed-volume sound: zero wobble would be ideal, so compare averages.
        let vol = |p: &mut Player, r: &mut Rng, t: f32| p.request(Sound::Yelp, t, 1.0, r).map(|x| x.volume).unwrap();
        let first = (0..30).map(|i| {
            let mut q = Player::new();
            vol(&mut q, &mut Rng::new(i), 0.0)
        }).sum::<f32>() / 30.0;
        let mut t = 0.0;
        let mut last = 0.0;
        for _ in 0..8 {
            t += 0.2;
            last = vol(&mut p, &mut r, t);
        }
        assert!(last < first * 0.8, "eighth rapid yelp ({last}) is quieter than a fresh one ({first})");
        // After a long rest it recovers fully.
        let rested = (0..30).map(|i| {
            let mut q = Player::new();
            q.request(Sound::Yelp, 0.0, 1.0, &mut Rng::new(i));
            vol(&mut q, &mut Rng::new(i + 100), 60.0)
        }).sum::<f32>() / 30.0;
        assert!((rested - first).abs() < first * 0.2, "rested {rested} vs fresh {first}");
        // Non-fatiguing sounds don't get quieter.
        let a = p.request(Sound::RifleShot, 500.0, 1.0, &mut r).unwrap().volume;
        for k in 1..6 {
            p.request(Sound::RifleShot, 500.0 + k as f32 * 0.3, 1.0, &mut r);
        }
        let b = p.request(Sound::RifleShot, 502.0, 1.0, &mut r).unwrap().volume;
        let prof = Sound::RifleShot.profile();
        assert!(a > prof.volume * (1.0 - prof.vary - 0.01) && b > prof.volume * (1.0 - prof.vary - 0.01));
    }

    #[test]
    fn sounds_never_exceed_full_scale() {
        let mut p = Player::new();
        let mut r = rng();
        for (i, &s) in Sound::ALL.iter().enumerate() {
            if s.is_loop() {
                continue;
            }
            let play = p.request(s, i as f32 * 100.0, 2.0, &mut r).unwrap();
            assert!(play.volume <= 1.0 && play.volume >= 0.0, "{s:?}");
        }
    }

    #[test]
    fn mixer_applies_sliders_and_mute() {
        let m = Mix::default();
        assert!(m.gain(Bus::Music) < m.gain(Bus::Sfx), "music defaults below effects");
        assert!(m.gain(Bus::Ambience) < m.gain(Bus::Sfx));
        let muted = Mix { muted: true, ..m };
        for bus in [Bus::Sfx, Bus::Ambience, Bus::Music] {
            assert_eq!(muted.gain(bus), 0.0);
        }
        let half = Mix { master: 0.5, ..m };
        assert!((half.gain(Bus::Sfx) - m.gain(Bus::Sfx) * 0.5 / 0.8).abs() < 1e-6);
        let mut s = 0.95;
        Mix::nudge(&mut s, 0.1);
        assert_eq!(s, 1.0);
        Mix::nudge(&mut s, -2.0);
        assert_eq!(s, 0.0);
    }

    fn run(director: &mut MusicDirector, ctx: &MusicContext, secs: f32, rng: &mut Rng) {
        let mut t = 0.0;
        while t < secs {
            director.update(0.1, ctx, rng);
            t += 0.1;
        }
    }

    #[test]
    fn music_opens_quiet_and_leaves_long_silences() {
        let mut r = rng();
        let mut d = MusicDirector::new(&mut r);
        let calm = MusicContext::default();
        let mut silent = 0;
        let mut total = 0;
        let mut longest_gap = 0.0f32;
        let mut gap = 0.0f32;
        for i in 0..(30 * 60 * 10) {
            let l = d.update(0.1, &calm, &mut r);
            if i < 100 {
                assert!(l.iter().all(|v| *v < 0.05), "no music in the first 10 seconds");
            }
            total += 1;
            if l.iter().sum::<f32>() < 0.02 {
                silent += 1;
                gap += 0.1;
                longest_gap = longest_gap.max(gap);
            } else {
                gap = 0.0;
            }
        }
        let frac = silent as f32 / total as f32;
        assert!(frac > 0.35 && frac < 0.8, "silent {:.0}% of the time", frac * 100.0);
        assert!(longest_gap > 70.0, "longest silence {longest_gap}s");
    }

    #[test]
    fn threats_bring_the_matching_mood() {
        let mut r = rng();
        let mut d = MusicDirector::new(&mut r);
        run(&mut d, &MusicContext::default(), 5.0, &mut r);
        // Wolves close in: tension fades in even during a silent gap.
        let wolves = MusicContext {
            enemy_dist: Some(30.0),
            ..Default::default()
        };
        run(&mut d, &wolves, 12.0, &mut r);
        assert_eq!(d.mood(), Mood::Tense);
        assert!(d.levels[Mood::Tense as usize] > 0.5 && d.levels[0] < 0.1);
        // A blizzard escalates to danger.
        let storm = MusicContext {
            blizzard: true,
            enemy_dist: Some(30.0),
            ..Default::default()
        };
        run(&mut d, &storm, 15.0, &mut r);
        assert_eq!(d.mood(), Mood::Danger);
        assert!(d.levels[Mood::Danger as usize] > 0.7 && d.levels[Mood::Tense as usize] < 0.2);
        // After it passes, the threat mood is held a while, then fades out.
        run(&mut d, &MusicContext::default(), 5.0, &mut r);
        assert_eq!(d.mood(), Mood::Danger, "held so it doesn't flicker");
        run(&mut d, &MusicContext::default(), 60.0, &mut r);
        assert!(d.levels[Mood::Danger as usize] < 0.01);
        for l in d.levels {
            assert!((0.0..=1.0).contains(&l));
        }
    }

    #[test]
    fn a_brief_wolf_sighting_does_not_flicker_the_music() {
        let mut r = rng();
        let mut d = MusicDirector::new(&mut r);
        let near = MusicContext {
            enemy_dist: Some(20.0),
            ..Default::default()
        };
        run(&mut d, &near, 1.0, &mut r);
        let mut changes = 0;
        let mut last = d.mood();
        for _ in 0..150 {
            d.update(0.1, &MusicContext::default(), &mut r);
            if d.mood() != last {
                changes += 1;
                last = d.mood();
            }
        }
        assert!(changes <= 1, "mood changed {changes} times");
    }

    #[test]
    fn geiger_is_gentle() {
        assert_eq!(geiger_clicks_per_sec(0.0), 0.0);
        assert_eq!(geiger_clicks_per_sec(0.1), 0.0);
        let ice = geiger_clicks_per_sec(2.5);
        let blizzard = geiger_clicks_per_sec(6.0);
        let crater = geiger_clicks_per_sec(15.0);
        assert!(ice > 0.5 && ice < 1.5, "ice {ice}");
        assert!(blizzard < 3.0, "blizzard {blizzard}");
        assert!(crater <= 7.0 && crater > blizzard, "crater {crater}");
        // The old rule clicked 15 times a second in a blizzard.
        assert!(blizzard < 15.0 / 4.0);
        let mut prev = 0.0;
        for i in 1..100 {
            let c = geiger_clicks_per_sec(i as f32 * 0.5);
            assert!(c >= prev && c <= 7.0);
            prev = c;
        }
    }

    #[test]
    fn every_surface_has_its_own_footstep() {
        let all = [Surface::Snow, Surface::Ice, Surface::Road, Surface::Concrete, Surface::Wood];
        let sounds: std::collections::HashSet<Sound> = all.iter().map(|s| step_sound(*s)).collect();
        assert_eq!(sounds.len(), all.len());
    }

    #[test]
    fn rifle_cues_follow_the_animation() {
        let reload = cues(WeaponKind::PipeRifle, false);
        assert_eq!(reload.iter().map(|c| c.1).collect::<Vec<_>>(), vec![Sound::ClunkOut, Sound::ClunkIn, Sound::BoltRack]);
        // The hand pulls the magazine at 0.12 and seats a fresh one by 0.72
        // (see viewmodel.rs, which also checks each cue against the motion).
        assert!(reload[0].0 >= 0.1 && reload[0].0 < 0.35);
        assert!(reload[1].0 > 0.55 && reload[1].0 <= 0.75);
        assert!(reload[2].0 > reload[1].0 && reload[2].0 < 0.9);
        assert_eq!(cues(WeaponKind::PipeRifle, true).iter().filter(|c| c.1 == Sound::BoltRack).count(), 2);
    }

    #[test]
    fn every_weapon_has_sorted_in_range_cues() {
        for kind in WeaponKind::ALL {
            for unjam in [false, true] {
                let c = cues(kind, unjam);
                if kind == WeaponKind::IceAxe {
                    assert!(c.is_empty(), "the axe has no reload");
                } else {
                    assert!(!c.is_empty(), "{kind:?} unjam={unjam}");
                }
                assert!(c.windows(2).all(|w| w[0].0 < w[1].0), "{kind:?} cues out of order");
                assert!(c.iter().all(|(t, _)| (0.0..=1.0).contains(t)));
            }
        }
        // Every weapon sounds different when fired.
        let fires: std::collections::HashSet<Sound> = WeaponKind::ALL.iter().map(|k| fire_sound(*k)).collect();
        assert_eq!(fires.len(), 4);
    }

    #[test]
    fn approach_stops_on_target() {
        assert_eq!(approach(0.0, 1.0, 0.5, 1.0), 0.5);
        assert_eq!(approach(0.9, 1.0, 0.5, 1.0), 1.0);
        assert_eq!(approach(1.0, 0.0, 0.25, 2.0), 0.5);
    }

    #[test]
    fn reload_cues_fire_once_each() {
        let cues = [(0.2, Sound::ClunkOut), (0.6, Sound::ClunkIn), (0.8, Sound::BoltRack)];
        let mut heard = Vec::new();
        let mut prev = -0.001;
        for k in 0..=100 {
            let p = k as f32 / 100.0;
            heard.extend(cues_between(&cues, prev, p));
            prev = p;
        }
        assert_eq!(heard, vec![Sound::ClunkOut, Sound::ClunkIn, Sound::BoltRack]);
    }
}
