//! What the world sounds like, as rules (no Bevy, so they're unit-tested):
//! how muffled a sound is by what stands between you and it, which echo a
//! gunshot leaves in the open or in a room, what a boot sounds like in snow
//! at a walk, a run or deep cold, how loud each layer of wind is out of doors
//! and through a wall, and what each room murmurs to itself.

use super::combat::WeaponKind;
use super::interiors::Interior;
use super::sfx::step_sound;
use super::synth::Sound;
use super::terrain::Surface;

// ---------------------------------------------------------------------------
// Occlusion
// ---------------------------------------------------------------------------

/// How much of a sound survives each thing in its way.
const TREE_PASS: f32 = 0.78;
const WALL_PASS: f32 = 0.35;
/// Passing between a room and the outdoors (or two rooms).
const ROOM_PASS: f32 = 0.14;

/// How blocked a sound is, 0 (clear) to 1 (all but gone), given the trees
/// (`circles`) and walls (`rects`) its path crosses and whether it's in
/// another room from the listener.
pub fn occlusion(source_room: Option<Interior>, listener_room: Option<Interior>, circles: usize, rects: usize) -> f32 {
    let mut pass = TREE_PASS.powi(circles.min(8) as i32) * WALL_PASS.powi(rects.min(4) as i32);
    if source_room != listener_room {
        pass *= ROOM_PASS;
    }
    (1.0 - pass).clamp(0.0, 0.95)
}

/// What a listener hears of a sound that's `occlusion` blocked.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Heard {
    /// Multiply the volume by this.
    pub gain: f32,
    /// Use the dull, filtered version of the clip.
    pub muffled: bool,
}

pub fn heard(occlusion: f32) -> Heard {
    Heard { gain: 1.0 - 0.55 * occlusion, muffled: occlusion > 0.3 }
}

// ---------------------------------------------------------------------------
// Gunfire
// ---------------------------------------------------------------------------

/// The echo that follows a shot: long and rolling over open snow, short and
/// bright in a room.
pub fn shot_tail(room: Option<Interior>) -> Sound {
    if room.is_some() {
        Sound::ShotTailIndoor
    } else {
        Sound::ShotTailOutdoor
    }
}

/// How loud the echo is next to the shot, by gun (the shotgun booms).
pub fn tail_gain(shot: Sound) -> f32 {
    match shot {
        Sound::ShotgunShot => 0.85,
        Sound::RifleShot => 0.7,
        _ => 0.55,
    }
}

/// Is this sound a gunshot?
pub fn is_gunshot(sound: Sound) -> bool {
    matches!(sound, Sound::RifleShot | Sound::ShotgunShot | Sound::RevolverShot)
}

/// The mechanical sound that follows a shot, and when (seconds later): the
/// pipe rifle's bolt, the revolver's hammer thumbed back. The shotgun breaks
/// open to reload, so it has none.
pub fn follow_up(kind: WeaponKind) -> Option<(Sound, f32)> {
    match kind {
        WeaponKind::PipeRifle => Some((Sound::BoltClack, 0.34)),
        WeaponKind::Revolver => Some((Sound::HammerCock, 0.3)),
        WeaponKind::ScrapShotgun | WeaponKind::IceAxe => None,
    }
}

/// The same for a raider's gun (rifle: bolt, revolver: hammer; shotgun none),
/// as (sound, seconds later).
pub fn follow_up_for_gun(gun: super::raider::Gun) -> Option<(Sound, f32)> {
    use super::raider::Gun;
    match gun {
        Gun::Rifle => Some((Sound::BoltClack, 0.0)),
        Gun::Revolver => Some((Sound::HammerCock, 0.0)),
        Gun::Shotgun => None,
    }
}

// ---------------------------------------------------------------------------
// Footsteps
// ---------------------------------------------------------------------------

/// Chance a walking step in snow squeaks: none until it's cold, then more
/// the colder it gets (dry snow below about -10F squeals underfoot).
pub fn squeak_chance(temp_f: f32) -> f32 {
    ((-5.0 - temp_f) / 45.0).clamp(0.0, 0.7)
}

/// Which sound a boot makes and how loud. Running crunches harder and louder
/// than walking; walking in deep cold squeaks. `roll` is a random 0..1.
pub fn footstep(surface: Surface, running: bool, temp_f: f32, roll: f32) -> (Sound, f32) {
    match surface {
        Surface::Snow if running => (Sound::StepSnowRun, 1.0),
        Surface::Snow if roll < squeak_chance(temp_f) => (Sound::StepSnowSqueak, 0.85),
        Surface::Snow => (Sound::StepSnow, 0.7),
        other => (step_sound(other), if running { 1.0 } else { 0.7 }),
    }
}

/// How loud somebody else's footsteps are, by how fast they're moving
/// (metres per second): a creeping step is faint, a run is plain.
pub fn step_gain_for_speed(speed: f32) -> f32 {
    (0.25 + 0.14 * speed).clamp(0.25, 0.85)
}

// ---------------------------------------------------------------------------
// Wind
// ---------------------------------------------------------------------------

/// Where you are for the purposes of wind: out in it, or behind walls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelter {
    Outdoors,
    Inside(Interior),
}

impl Shelter {
    pub fn of(room: Option<Interior>) -> Shelter {
        room.map_or(Shelter::Outdoors, Shelter::Inside)
    }
}

/// The volume of each layer of wind, 0..1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WindMix {
    pub breeze: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    pub howl: f32,
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The wind layers for a wind strength `w` (0 calm, 1 full blizzard): a soft
/// breeze when it's quiet, the rumble, rush and hiss growing with the wind,
/// and a howl only once it's really blowing. Behind walls the highs vanish
/// and what's left is a dull rumble; the stockroom, with its broken skylight,
/// lets more through than the vault.
pub fn wind_mix(w: f32, shelter: Shelter) -> WindMix {
    let w = w.clamp(0.0, 1.0);
    let open = WindMix {
        breeze: 0.12 + 0.55 * (1.0 - smoothstep(0.1, 0.6, w)),
        low: (0.45 + 0.5 * w) * 0.6,
        mid: (0.08 + 0.9 * w) * 0.55,
        high: (0.03 + 0.95 * w * w) * 0.5,
        howl: smoothstep(0.5, 1.0, w) * 0.9,
    };
    // (breeze, low, mid, high, howl) scale factors.
    let k = match shelter {
        Shelter::Outdoors => [1.0, 1.0, 1.0, 1.0, 1.0],
        Shelter::Inside(Interior::FishHouse(_)) => [0.25, 0.55, 0.18, 0.05, 0.22],
        Shelter::Inside(Interior::Mart) => [0.2, 0.5, 0.3, 0.12, 0.45],
        Shelter::Inside(Interior::VaultLobby) => [0.0, 0.2, 0.05, 0.0, 0.05],
        // Deep underground: only the reactor's own rumble.
        Shelter::Inside(Interior::Reactor) => [0.0, 0.08, 0.0, 0.0, 0.0],
    };
    WindMix { breeze: open.breeze * k[0], low: open.low * k[1], mid: open.mid * k[2], high: open.high * k[3], howl: open.howl * k[4] }
}

// ---------------------------------------------------------------------------
// Rooms
// ---------------------------------------------------------------------------

/// The level of the machine-hum loop in a room.
pub fn room_hum_level(room: Option<Interior>) -> f32 {
    match room {
        Some(Interior::VaultLobby) => 0.55,
        Some(Interior::Reactor) => 0.9,
        Some(Interior::Mart) => 0.12,
        Some(Interior::FishHouse(_)) => 0.0,
        None => 0.0,
    }
}

/// The random noise a room makes now and then: (sound, shortest, longest
/// gap in seconds).
pub fn room_noise(room: Option<Interior>) -> Option<(Sound, f32, f32)> {
    match room {
        Some(Interior::FishHouse(_)) => Some((Sound::Creak, 12.0, 34.0)),
        Some(Interior::Mart) => Some((Sound::Drip, 3.5, 10.0)),
        Some(Interior::VaultLobby) | Some(Interior::Reactor) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUSE: Option<Interior> = Some(Interior::FishHouse(0));

    #[test]
    fn nothing_in_the_way_means_nothing_lost() {
        assert_eq!(occlusion(None, None, 0, 0), 0.0);
        let h = heard(0.0);
        assert_eq!((h.gain, h.muffled), (1.0, false));
    }

    #[test]
    fn trees_dull_a_sound_and_walls_dull_it_more() {
        let one_tree = occlusion(None, None, 1, 0);
        let five_trees = occlusion(None, None, 5, 0);
        let wall = occlusion(None, None, 0, 1);
        assert!(one_tree > 0.0 && one_tree < five_trees && five_trees < 1.0);
        assert!(wall > one_tree * 2.0, "a wall: {wall}, a tree: {one_tree}");
        assert!(!heard(one_tree).muffled, "one tree doesn't muffle");
        assert!(heard(wall).muffled && heard(five_trees).muffled, "a wall or a thicket does");
        assert!(heard(wall).gain < heard(one_tree).gain);
    }

    #[test]
    fn a_wall_between_rooms_nearly_silences_it() {
        let across = occlusion(HOUSE, None, 0, 0);
        assert!(across > 0.8, "{across}");
        assert!(occlusion(HOUSE, HOUSE, 0, 0) == 0.0, "same room, no walls: clear");
        assert!(occlusion(Some(Interior::Mart), HOUSE, 0, 0) > 0.8, "different rooms");
        assert!(heard(across).gain < 0.6);
        // Never completely silent: you still hear a gunshot through the wall.
        assert!(occlusion(HOUSE, None, 8, 4) <= 0.95 && heard(0.95).gain > 0.4);
    }

    #[test]
    fn gunshots_get_the_right_echo() {
        assert_eq!(shot_tail(None), Sound::ShotTailOutdoor);
        assert_eq!(shot_tail(HOUSE), Sound::ShotTailIndoor);
        assert_eq!(shot_tail(Some(Interior::VaultLobby)), Sound::ShotTailIndoor);
        assert!(tail_gain(Sound::ShotgunShot) > tail_gain(Sound::RevolverShot));
        assert!(is_gunshot(Sound::RifleShot) && !is_gunshot(Sound::Snarl));
    }

    #[test]
    fn the_rifle_and_revolver_click_after_a_shot_but_the_shotgun_does_not() {
        let (rifle, rd) = follow_up(WeaponKind::PipeRifle).unwrap();
        let (revolver, vd) = follow_up(WeaponKind::Revolver).unwrap();
        assert_eq!((rifle, revolver), (Sound::BoltClack, Sound::HammerCock));
        assert!(rd > 0.15 && vd > 0.15, "after the shot's own noise has passed");
        assert!(follow_up(WeaponKind::ScrapShotgun).is_none() && follow_up(WeaponKind::IceAxe).is_none());
    }

    #[test]
    fn snow_crunches_harder_when_you_run_and_squeaks_when_it_is_very_cold() {
        assert_eq!(footstep(Surface::Snow, true, 20.0, 0.0), (Sound::StepSnowRun, 1.0));
        assert_eq!(footstep(Surface::Snow, false, 20.0, 0.0).0, Sound::StepSnow, "no squeak when it's mild");
        assert_eq!(footstep(Surface::Snow, false, -35.0, 0.1).0, Sound::StepSnowSqueak, "squeaks in deep cold");
        assert_eq!(footstep(Surface::Snow, false, -35.0, 0.95).0, Sound::StepSnow, "but not every step");
        assert!(footstep(Surface::Snow, true, 0.0, 0.5).1 > footstep(Surface::Snow, false, 0.0, 0.5).1, "louder running");
        // Other surfaces keep their own sound, louder when running.
        assert_eq!(footstep(Surface::Road, false, 0.0, 0.0), (Sound::StepRoad, 0.7));
        assert_eq!(footstep(Surface::Wood, true, 0.0, 0.0), (Sound::StepWood, 1.0));
    }

    #[test]
    fn the_squeak_chance_grows_with_the_cold() {
        assert_eq!(squeak_chance(10.0), 0.0);
        assert!(squeak_chance(-20.0) > 0.2 && squeak_chance(-40.0) > squeak_chance(-20.0));
        assert!(squeak_chance(-90.0) <= 0.7);
    }

    #[test]
    fn footsteps_are_louder_the_faster_you_go() {
        assert!(step_gain_for_speed(1.0) < step_gain_for_speed(4.0));
        assert!((0.25..=0.85).contains(&step_gain_for_speed(0.0)) && step_gain_for_speed(50.0) <= 0.85);
    }

    #[test]
    fn a_calm_day_is_breeze_a_blizzard_is_howl() {
        let calm = wind_mix(0.1, Shelter::Outdoors);
        let storm = wind_mix(1.0, Shelter::Outdoors);
        assert!(calm.breeze > storm.breeze && calm.howl == 0.0, "{calm:?}");
        assert!(storm.howl > 0.5 && storm.breeze < calm.breeze * 0.4, "{storm:?}");
        assert!(storm.mid > calm.mid && storm.high > calm.high && storm.low > calm.low);
        // Climbing wind never lowers the howl.
        let mut last = 0.0;
        for i in 0..=20 {
            let h = wind_mix(i as f32 / 20.0, Shelter::Outdoors).howl;
            assert!(h >= last - 1e-6);
            last = h;
        }
    }

    #[test]
    fn behind_walls_the_wind_is_a_dull_rumble() {
        let out = wind_mix(0.9, Shelter::Outdoors);
        let house = wind_mix(0.9, Shelter::Inside(Interior::FishHouse(2)));
        let vault = wind_mix(0.9, Shelter::Inside(Interior::VaultLobby));
        assert!(house.high < out.high * 0.1 && house.mid < out.mid * 0.3, "the highs are gone");
        assert!(house.low > out.low * 0.4, "the rumble gets through");
        assert!(house.howl < out.howl * 0.3);
        assert!(vault.low < house.low && vault.mid < house.mid && vault.howl < house.howl, "deeper in the earth");
        let mart = wind_mix(0.9, Shelter::Inside(Interior::Mart));
        assert!(mart.howl > house.howl, "the broken skylight lets the howl in");
    }

    #[test]
    fn each_room_has_its_own_murmur() {
        assert!(room_hum_level(Some(Interior::VaultLobby)) > room_hum_level(Some(Interior::Mart)));
        assert!(room_hum_level(Some(Interior::Reactor)) > room_hum_level(Some(Interior::VaultLobby)), "the reactor is the loudest room");
        let deep = wind_mix(0.9, Shelter::Inside(Interior::Reactor));
        assert!(deep.mid == 0.0 && deep.howl == 0.0 && deep.low < wind_mix(0.9, Shelter::Inside(Interior::VaultLobby)).low);
        assert_eq!(room_hum_level(HOUSE), 0.0);
        assert_eq!(room_hum_level(None), 0.0);
        let (s, lo, hi) = room_noise(HOUSE).unwrap();
        assert_eq!(s, Sound::Creak);
        assert!(lo > 5.0 && hi > lo);
        assert_eq!(room_noise(Some(Interior::Mart)).unwrap().0, Sound::Drip);
        assert!(room_noise(None).is_none() && room_noise(Some(Interior::VaultLobby)).is_none());
    }
}
