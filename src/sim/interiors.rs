//! Interior spaces: the four fish houses, the Vault 143 lobby, the
//! Bullseye-Mart stockroom and the reactor level below the vault. Each is a
//! small room built far off the map (so it can't be seen or walked into from
//! outside) and reached through a door (from the world, or, for the reactor,
//! from the lobby above it). This
//! module is the pure part: where each room is, how big it is, where its doors
//! open to, what the air feels like inside, plus sleeping, cooking and the
//! timing of the fade that hides the move.

use super::daynight::Clock;
use super::survival::{AidResult, Inventory, Survival};
use super::terrain::{Surface, SHELTERS, VAULT_POS};
use super::weather::{Phase, Weather};

/// The Bullseye-Mart ruin's centre (the sign, walls and loading dock).
pub const MART_POS: (f32, f32) = (-40.0, -110.0);

/// Height of every interior floor.
pub const FLOOR_Y: f32 = 0.0;

/// Interiors sit on a row this far off the map, this far apart.
const ROW_X: f32 = 1000.0;
const ROW_Z: f32 = 1000.0;
const SPACING: f32 = 60.0;

pub const FISH_HOUSE_NAMES: [&str; 4] = ["Lundgren's Fish House", "Sven's Shanty", "Olson's Bait & Tackle", "Ole's Ice Shack"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Interior {
    /// One of the four fish houses, in the order of [`SHELTERS`].
    FishHouse(u8),
    VaultLobby,
    Mart,
    /// The vault's reactor level, down a stair from the lobby.
    Reactor,
}

pub const ALL: [Interior; 7] = [
    Interior::FishHouse(0),
    Interior::FishHouse(1),
    Interior::FishHouse(2),
    Interior::FishHouse(3),
    Interior::VaultLobby,
    Interior::Mart,
    Interior::Reactor,
];

impl Interior {
    /// The name used in saves ("fish_house_2", "vault_lobby", "mart").
    pub fn id(self) -> String {
        match self {
            Interior::FishHouse(i) => format!("fish_house_{i}"),
            Interior::VaultLobby => "vault_lobby".to_string(),
            Interior::Mart => "mart".to_string(),
            Interior::Reactor => "reactor".to_string(),
        }
    }

    pub fn parse(id: &str) -> Option<Interior> {
        match id {
            "vault_lobby" => Some(Interior::VaultLobby),
            "mart" => Some(Interior::Mart),
            "reactor" => Some(Interior::Reactor),
            _ => {
                let n: u8 = id.strip_prefix("fish_house_")?.parse().ok()?;
                (n < 4).then_some(Interior::FishHouse(n))
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Interior::FishHouse(i) => FISH_HOUSE_NAMES[i as usize % 4],
            Interior::VaultLobby => "Vault 143",
            Interior::Mart => "the Bullseye-Mart stockroom",
            Interior::Reactor => "the Vault 143 reactor level",
        }
    }

    fn slot(self) -> usize {
        ALL.iter().position(|i| *i == self).unwrap_or(0)
    }

    /// Centre of the floor, in world coordinates (far off the map).
    pub fn origin(self) -> (f32, f32) {
        (ROW_X + SPACING * self.slot() as f32, ROW_Z)
    }

    /// Half the room's width (x) and depth (z).
    pub fn half(self) -> (f32, f32) {
        match self {
            Interior::FishHouse(_) => (2.6, 2.1),
            Interior::VaultLobby => (7.5, 5.8),
            Interior::Mart => (9.5, 7.5),
            Interior::Reactor => (6.5, 5.5),
        }
    }

    pub fn height(self) -> f32 {
        match self {
            Interior::FishHouse(_) => 2.5,
            Interior::VaultLobby => 4.2,
            Interior::Mart => 4.4,
            Interior::Reactor => 5.2,
        }
    }

    /// What you hear underfoot.
    pub fn surface(self) -> Surface {
        match self {
            Interior::FishHouse(_) => Surface::Wood,
            _ => Surface::Concrete,
        }
    }

    /// Heated rooms warm you up; the stockroom is out of the wind but no warmer.
    pub fn warm(self) -> bool {
        !matches!(self, Interior::Mart)
    }

    /// Room light: colour and brightness.
    pub fn ambient(self) -> ([f32; 3], f32) {
        match self {
            Interior::FishHouse(_) => ([1.0, 0.8, 0.55], 190.0),
            Interior::VaultLobby => ([0.8, 0.9, 1.0], 260.0),
            Interior::Mart => ([0.6, 0.66, 0.72], 120.0),
            Interior::Reactor => ([0.62, 0.74, 0.84], 190.0),
        }
    }

    /// A point `local` metres from the room's centre, in the world.
    pub fn at(self, lx: f32, lz: f32) -> (f32, f32) {
        let (ox, oz) = self.origin();
        (ox + lx, oz + lz)
    }

    /// Where you stand when you come in, and which way you face (yaw; 0 is
    /// north, -z). The exit door is behind you, in the south wall.
    pub fn entry(self) -> (f32, f32, f32) {
        let (_, hd) = self.half();
        let (x, z) = self.at(0.0, hd - 1.1);
        (x, z, 0.0)
    }

    /// The centre of the door you leave by.
    pub fn exit_door(self) -> (f32, f32) {
        let (_, hd) = self.half();
        self.at(0.0, hd - 0.1)
    }

    /// The room this one is reached from, if it's down a stair rather than
    /// out in the world.
    pub fn parent(self) -> Option<Interior> {
        match self {
            Interior::Reactor => Some(Interior::VaultLobby),
            _ => None,
        }
    }

    /// Where the door in is (the point you stand near to use it): in the
    /// world, or inside the parent room. The reactor's stair door is in the
    /// lobby's north wall, beyond the Overseer's desk.
    pub fn outdoor_door(self) -> (f32, f32) {
        match self {
            Interior::Reactor => {
                let (_, hd) = Interior::VaultLobby.half();
                Interior::VaultLobby.at(-5.2, -hd + 0.3)
            }
            Interior::FishHouse(i) => {
                let (sx, sz) = SHELTERS[i as usize % 4];
                (sx + 0.3, sz)
            }
            Interior::VaultLobby => (VAULT_POS.0, VAULT_POS.1 - 1.6),
            Interior::Mart => (MART_POS.0 + 7.0, MART_POS.1 - 7.0),
        }
    }

    /// Where you come out (in the world, or in the parent room) and which way you face.
    pub fn outside(self) -> (f32, f32, f32) {
        match self {
            Interior::Reactor => {
                let (_, hd) = Interior::VaultLobby.half();
                let (x, z) = Interior::VaultLobby.at(-5.2, -hd + 1.5);
                (x, z, std::f32::consts::PI)
            }
            // East of the door, past the stove barrel, looking out over the lake.
            Interior::FishHouse(i) => {
                let (sx, sz) = SHELTERS[i as usize % 4];
                (sx + 0.7, sz + 1.35, -std::f32::consts::FRAC_PI_2)
            }
            Interior::VaultLobby => (VAULT_POS.0, VAULT_POS.1 - 5.0, 0.0),
            // Back in the ruin's yard, facing south away from the north wall.
            Interior::Mart => (MART_POS.0 + 7.0, MART_POS.1 - 5.3, std::f32::consts::PI),
        }
    }

    /// The prompt shown at the door in.
    pub fn enter_prompt(self) -> String {
        match self {
            Interior::Reactor => "[E] Go down the stair to the reactor level".to_string(),
            _ => format!("[E] Enter {}", self.name()),
        }
    }

    /// The prompt shown at the door out.
    pub fn leave_prompt(self) -> String {
        match self.parent() {
            Some(up) => format!("[E] Go back up to {}", up.name()),
            None => format!("[E] Leave {}", self.name()),
        }
    }
}

/// The interior whose floor the point is over, if any.
pub fn zone_at(x: f32, z: f32) -> Option<Interior> {
    ALL.iter().copied().find(|i| {
        let (ox, oz) = i.origin();
        let (hw, hd) = i.half();
        (x - ox).abs() <= hw + 0.5 && (z - oz).abs() <= hd + 0.5
    })
}

// ---------------------------------------------------------------------------
// Sleeping and cooking
// ---------------------------------------------------------------------------

/// How long a night in a bunk is.
pub const SLEEP_HOURS: f32 = 8.0;

/// What a night's sleep did.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SleepReport {
    pub hours: f32,
    pub healed: f32,
}

impl Clock {
    /// Move the clock on by `hours`, rolling over to new days.
    pub fn advance_hours(&mut self, hours: f32) {
        self.hours += hours;
        while self.hours >= 24.0 {
            self.hours -= 24.0;
            self.day += 1;
        }
    }
}

/// Sleep in a bunk: eight hours pass, you heal fully (as far as radiation
/// allows) and warm through, the storm blows over, and the frostbite eases.
/// Radiation doesn't go away.
pub fn sleep(clock: &mut Clock, survival: &mut Survival, weather: &mut Weather) -> SleepReport {
    clock.advance_hours(SLEEP_HOURS);
    let before = survival.health;
    survival.heal(Survival::BASE_MAX_HEALTH);
    if survival.body_heat < 85.0 {
        survival.warm(85.0 - survival.body_heat);
    }
    survival.frostbite = false;
    // Whatever was blowing outside has passed; a quiet spell follows.
    if weather.phase != Phase::Calm {
        weather.phase = Phase::Calm;
    }
    weather.timer = weather.timer.max(80.0);
    SleepReport { hours: SLEEP_HOURS, healed: survival.health - before }
}

/// Heat a hotdish on the stove: better than eating it cold (+60 Heat, +10 HP
/// instead of +35 and +5).
pub fn cook_hotdish(inv: &mut Inventory, s: &mut Survival) -> AidResult {
    if inv.hotdish == 0 {
        return AidResult::Refused("You have no hotdish to heat.");
    }
    inv.hotdish -= 1;
    s.warm(60.0);
    s.heal(10.0);
    AidResult::Used("You heat a hotdish on the stove. Steaming, bubbling and only a little radioactive. +60 Heat")
}

// ---------------------------------------------------------------------------
// The fade that hides a door
// ---------------------------------------------------------------------------

pub const FADE_OUT: f32 = 0.35;
pub const FADE_IN: f32 = 0.45;

/// A fade to black and back. The move happens at the moment the screen is
/// fully black.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    t: f32,
    /// Seconds to stay black (sleeping holds longer so the text can be read).
    hold: f32,
    switched: bool,
}

/// What one step of a [`Fade`] did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FadeStep {
    /// 0 = clear, 1 = black.
    pub alpha: f32,
    /// True on exactly one step: the screen is black, make the move now.
    pub switch_now: bool,
    pub done: bool,
}

impl Fade {
    pub fn new(hold: f32) -> Fade {
        // Always at least a moment fully black, so the switch lands in it.
        Fade { t: 0.0, hold: hold.max(0.05), switched: false }
    }

    pub fn step(&mut self, dt: f32) -> FadeStep {
        self.t += dt.max(0.0);
        let black_from = FADE_OUT;
        let black_until = FADE_OUT + self.hold;
        let end = black_until + FADE_IN;
        let alpha = if self.t < black_from {
            self.t / FADE_OUT
        } else if self.t < black_until {
            1.0
        } else {
            1.0 - ((self.t - black_until) / FADE_IN)
        };
        let switch_now = !self.switched && self.t >= black_from;
        if switch_now {
            self.switched = true;
        }
        FadeStep { alpha: alpha.clamp(0.0, 1.0), switch_now, done: self.t >= end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_interior_has_a_name_and_a_round_trip_id() {
        for i in ALL {
            assert_eq!(Interior::parse(&i.id()), Some(i), "{i:?}");
            assert!(!i.name().is_empty());
            assert!(i.enter_prompt().starts_with("[E] "));
            assert!(i.leave_prompt().starts_with("[E] "));
        }
        assert_eq!(Interior::parse("fish_house_4"), None, "only four fish houses");
        assert_eq!(Interior::parse("fish_house_x"), None);
        assert_eq!(Interior::parse(""), None);
        assert_eq!(Interior::parse("castle"), None);
        assert_eq!(Interior::FishHouse(2).name(), "Olson's Bait & Tackle");
    }

    #[test]
    fn interiors_are_far_off_the_map_and_never_overlap() {
        for a in ALL {
            let (ox, oz) = a.origin();
            let (hw, hd) = a.half();
            assert!(ox - hw > 400.0 && oz - hd > 400.0, "{a:?} is well beyond the 200 m map");
            for b in ALL {
                if a != b {
                    let (bx, bz) = b.origin();
                    let (bw, bd) = b.half();
                    let gap = (ox - bx).abs() - hw - bw;
                    assert!(gap > 5.0 || (oz - bz).abs() - hd - bd > 5.0, "{a:?} and {b:?} are separated");
                }
            }
        }
    }

    #[test]
    fn a_point_belongs_to_its_own_interior_only() {
        for i in ALL {
            let (ox, oz) = i.origin();
            assert_eq!(zone_at(ox, oz), Some(i));
            let (ex, ez, _) = i.entry();
            assert_eq!(zone_at(ex, ez), Some(i), "you arrive inside {i:?}");
            let (dx, dz) = i.exit_door();
            assert_eq!(zone_at(dx, dz), Some(i), "the exit door is on the room's edge, inside its zone");
        }
        for (x, z) in [(0.0, 0.0), (0.0, 152.0), (-199.0, 199.0), (300.0, 300.0), (999.0, 2000.0)] {
            assert_eq!(zone_at(x, z), None, "({x}, {z}) is outdoors");
        }
    }

    #[test]
    fn you_arrive_a_few_steps_from_the_door_you_leave_by_and_face_into_the_room() {
        for i in ALL {
            let (ex, ez, yaw) = i.entry();
            let (dx, dz) = i.exit_door();
            let d = (ex - dx).hypot(ez - dz);
            assert!((0.8..2.5).contains(&d), "{i:?}: {d} m from the exit door (close enough to use it, not on top of it)");
            assert_eq!(yaw, 0.0, "facing north, into the room, with the door behind you");
            assert!(dz > ez, "the door is behind you (south)");
        }
    }

    #[test]
    fn doors_from_the_world_are_outdoors_and_you_step_out_clear_of_them() {
        for i in ALL.into_iter().filter(|i| i.parent().is_none()) {
            let (dx, dz) = i.outdoor_door();
            let (ox, oz, _) = i.outside();
            assert_eq!(zone_at(dx, dz), None, "{i:?}'s door is in the world");
            assert_eq!(zone_at(ox, oz), None);
            let gap = (dx - ox).hypot(dz - oz);
            assert!((0.8..6.0).contains(&gap), "{i:?}: you come out {gap} m from the door, near enough to go back in");
            assert!(ox.abs() < 200.0 && oz.abs() < 200.0, "and on the map");
        }
    }

    #[test]
    fn the_reactor_is_down_a_stair_from_the_lobby() {
        let r = Interior::Reactor;
        assert_eq!(r.parent(), Some(Interior::VaultLobby));
        assert_eq!(ALL.iter().filter(|i| i.parent().is_some()).count(), 1);
        let (dx, dz) = r.outdoor_door();
        assert_eq!(zone_at(dx, dz), Some(Interior::VaultLobby), "the stair door is in the lobby");
        let (ox, oz, yaw) = r.outside();
        assert_eq!(zone_at(ox, oz), Some(Interior::VaultLobby), "you come back up into the lobby");
        let gap = (dx - ox).hypot(dz - oz);
        assert!((0.8..3.0).contains(&gap), "{gap} m from the door: close enough to go back down");
        assert!((yaw - std::f32::consts::PI).abs() < 1e-5, "facing into the room, away from the north wall");
        assert!(oz > dz, "south of the door");
        // Clear of the desk, the pillars and the locker.
        let (lx, lz) = (ox - Interior::VaultLobby.origin().0, oz - Interior::VaultLobby.origin().1);
        assert!(lx < -4.0 && lz < 0.0, "in the north-west corner: ({lx}, {lz})");
        assert_eq!(Interior::parse("reactor"), Some(r));
        assert_eq!(r.name(), "the Vault 143 reactor level");
        assert!(r.warm(), "the reactor level is heated by the reactor");
    }

    #[test]
    fn fish_house_doors_follow_their_shelters() {
        for n in 0..4u8 {
            let (sx, sz) = SHELTERS[n as usize];
            let (dx, dz) = Interior::FishHouse(n).outdoor_door();
            assert!((dx - sx).abs() < 1.0 && (dz - sz).abs() < 0.5);
        }
        let (vx, vz) = Interior::VaultLobby.outdoor_door();
        assert_eq!((vx, vz), (VAULT_POS.0, VAULT_POS.1 - 1.6));
    }

    #[test]
    fn heated_rooms_warm_you_and_the_stockroom_does_not() {
        assert!(Interior::FishHouse(1).warm() && Interior::VaultLobby.warm());
        assert!(!Interior::Mart.warm());
        assert_eq!(Interior::FishHouse(0).surface(), Surface::Wood);
        assert_eq!(Interior::Mart.surface(), Surface::Concrete);
        for i in ALL {
            let (c, b) = i.ambient();
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)) && b > 50.0 && b < 400.0, "{i:?}");
        }
        assert!(Interior::Mart.ambient().1 < Interior::FishHouse(0).ambient().1, "the stockroom is the darkest");
    }

    fn hurt_and_cold() -> (Clock, Survival, Weather) {
        let mut s = Survival::new();
        s.health = 20.0;
        s.body_heat = 12.0;
        s.frostbite = true;
        s.rads = 300.0;
        (Clock { hours: 22.0, day: 3 }, s, Weather { phase: Phase::Blizzard, timer: 5.0, blizzards: 2 })
    }

    #[test]
    fn a_night_in_a_bunk_heals_warms_and_passes_eight_hours() {
        let (mut c, mut s, mut w) = hurt_and_cold();
        let r = sleep(&mut c, &mut s, &mut w);
        assert_eq!(r.hours, SLEEP_HOURS);
        assert_eq!((c.hours, c.day), (6.0, 4), "22:00 + 8 h = 06:00 next day");
        assert_eq!(s.health, s.max_health(), "healed fully");
        assert_eq!(r.healed, s.max_health() - 20.0);
        assert!(s.max_health() < 100.0, "radiation still lowers the maximum");
        assert_eq!(s.rads, 300.0, "sleep doesn't cure radiation");
        assert_eq!(s.body_heat, 85.0);
        assert!(!s.frostbite);
        assert_eq!(w.phase, Phase::Calm, "the storm blew over");
        assert!(w.timer >= 80.0);
        assert_eq!(w.blizzards, 2, "the count of storms past isn't rewritten");
    }

    #[test]
    fn sleeping_never_takes_warmth_away_or_shortens_a_calm_spell() {
        let mut s = Survival::new();
        s.body_heat = 97.0;
        let mut c = Clock { hours: 7.0, day: 1 };
        let mut w = Weather { phase: Phase::Calm, timer: 300.0, blizzards: 0 };
        let r = sleep(&mut c, &mut s, &mut w);
        assert_eq!(s.body_heat, 97.0);
        assert_eq!(w.timer, 300.0);
        assert_eq!(r.healed, 0.0);
        assert_eq!((c.hours, c.day), (15.0, 1));
    }

    #[test]
    fn the_clock_rolls_over_midnight_and_long_jumps() {
        let mut c = Clock { hours: 23.0, day: 1 };
        c.advance_hours(2.0);
        assert_eq!((c.hours, c.day), (1.0, 2));
        c.advance_hours(50.0);
        assert_eq!((c.hours, c.day), (3.0, 4));
        c.advance_hours(0.0);
        assert_eq!((c.hours, c.day), (3.0, 4));
    }

    #[test]
    fn cooking_a_hotdish_beats_eating_it_cold_and_needs_one() {
        let mut inv = Inventory::starting_kit();
        let mut s = Survival::new();
        s.body_heat = 20.0;
        s.health = 80.0;
        let hot = cook_hotdish(&mut inv, &mut s);
        assert!(hot.used());
        assert_eq!((s.body_heat, s.health, inv.hotdish), (80.0, 90.0, 1));
        let mut cold_inv = Inventory::starting_kit();
        let mut cold = Survival::new();
        cold.body_heat = 20.0;
        cold.health = 80.0;
        crate::sim::survival::use_aid(crate::sim::survival::Aid::Hotdish, &mut cold_inv, &mut cold);
        assert!(s.body_heat > cold.body_heat && s.health > cold.health, "cooked is better");
        inv.hotdish = 0;
        assert!(!cook_hotdish(&mut inv, &mut s).used());
        assert_eq!(s.body_heat, 80.0, "nothing happens without a hotdish");
    }

    #[test]
    fn a_fade_goes_dark_switches_once_and_clears() {
        let mut f = Fade::new(0.0);
        let mut switches = 0;
        let last;
        let mut peak = 0.0f32;
        let mut steps = 0;
        loop {
            let s = f.step(0.05);
            steps += 1;
            assert!((0.0..=1.0).contains(&s.alpha));
            if s.switch_now {
                switches += 1;
                assert!(s.alpha > 0.95, "the move happens when it's black: {}", s.alpha);
            }
            peak = peak.max(s.alpha);
            if s.done {
                last = s.alpha;
                break;
            }
            assert!(steps < 100, "it ends");
        }
        assert_eq!(switches, 1);
        assert_eq!(peak, 1.0);
        assert!(last < 0.05, "ends clear");
        assert!(steps as f32 * 0.05 < FADE_OUT + FADE_IN + 0.2, "and quickly");
    }

    #[test]
    fn a_long_hold_stays_black_for_reading_and_one_big_step_still_switches_once() {
        let mut f = Fade::new(1.5);
        f.step(FADE_OUT);
        for _ in 0..10 {
            assert_eq!(f.step(0.1).alpha, 1.0, "held black");
        }
        let mut big = Fade::new(0.0);
        let s = big.step(10.0);
        assert!(s.switch_now && s.done, "a huge frame still switches and finishes");
        assert!(!big.step(0.1).switch_now, "but never switches twice");
        let mut neg = Fade::new(-5.0);
        assert!(!neg.step(-1.0).switch_now, "negative time and hold are ignored");
    }
}
