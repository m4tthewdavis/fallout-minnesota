//! The save-game format: what is remembered, how it is written, and how an
//! old, damaged or hand-edited file is read back without ever crashing or
//! loading nonsense.
//!
//! A save holds the player's state (position, vitals, inventory, weapons and
//! upgrades), the clock and weather, the fog of war, which containers have
//! been opened and which pickups taken (by position, since the world is the
//! same every game), the quest flags and the play time. Enemies are not
//! saved: they repopulate when you load.
//!
//! Files carry a `version`. Newer files are refused with a clear message;
//! older ones are upgraded step by step in [`migrate`]. New optional fields
//! can be added without a version bump because every field has a default.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::combat::{Arsenal, Upgrade, Weapon, WeaponKind};
use super::daynight::Clock;
use super::interiors::{self, Interior};
use super::mapdata::{self, Fog, FOG_CELLS};
use super::progress;
use super::survival::{Inventory, Survival};
use super::terrain::{HALF_SIZE, PLAYER_SPAWN};
use super::weather::{Phase, Weather};

/// The format this build writes.
pub const SAVE_VERSION: u32 = 1;
/// The oldest format this build can still read.
pub const OLDEST_SUPPORTED: u32 = 1;

/// Most entries any list in a save may hold (a guard against huge files).
const MAX_LIST: usize = 20_000;
/// Largest count of any one item.
const MAX_COUNT: u32 = 9_999;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSave {
    /// Feet position.
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for PlayerSave {
    fn default() -> Self {
        PlayerSave { x: PLAYER_SPAWN.0, y: 0.0, z: PLAYER_SPAWN.1, yaw: 0.0, pitch: 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SurvivalSave {
    pub health: f32,
    pub body_heat: f32,
    pub rads: f32,
    pub frostbite: bool,
}

impl Default for SurvivalSave {
    fn default() -> Self {
        let s = Survival::new();
        SurvivalSave { health: s.health, body_heat: s.body_heat, rads: s.rads, frostbite: s.frostbite }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InventorySave {
    pub stimpaks: u32,
    pub radaway: u32,
    pub hotdish: u32,
    pub ammo_reserve: u32,
    pub shells: u32,
    pub revolver_rounds: u32,
    pub scrap: u32,
    pub pelts: u32,
    pub has_frostfang_coat: bool,
}

impl Default for InventorySave {
    fn default() -> Self {
        let i = Inventory::starting_kit();
        InventorySave {
            stimpaks: i.stimpaks,
            radaway: i.radaway,
            hotdish: i.hotdish,
            ammo_reserve: i.ammo_reserve,
            shells: i.shells,
            revolver_rounds: i.revolver_rounds,
            scrap: i.scrap,
            pelts: i.pelts,
            has_frostfang_coat: i.has_frostfang_coat,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WeaponSave {
    /// Rounds in the magazine (cut down to the magazine size on load).
    pub mag: u32,
    pub jammed: bool,
    /// Indexed like [`Upgrade::ALL`].
    pub upgrades: [bool; 4],
}

impl Default for WeaponSave {
    fn default() -> Self {
        WeaponSave { mag: u32::MAX, jammed: false, upgrades: [false; 4] }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ArsenalSave {
    pub owned: [bool; 4],
    pub current: usize,
    /// One per weapon slot, in [`WeaponKind::ALL`] order.
    pub weapons: Vec<WeaponSave>,
}

impl Default for ArsenalSave {
    fn default() -> Self {
        ArsenalSave { owned: [true, false, false, false], current: 0, weapons: vec![WeaponSave::default(); 4] }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockSave {
    pub hours: f32,
    pub day: u32,
}

impl Default for ClockSave {
    fn default() -> Self {
        let c = Clock::new();
        ClockSave { hours: c.hours, day: c.day }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PhaseSave {
    Calm,
    Warning,
    Blizzard,
}

impl From<Phase> for PhaseSave {
    fn from(p: Phase) -> Self {
        match p {
            Phase::Calm => PhaseSave::Calm,
            Phase::Warning => PhaseSave::Warning,
            Phase::Blizzard => PhaseSave::Blizzard,
        }
    }
}

impl From<PhaseSave> for Phase {
    fn from(p: PhaseSave) -> Self {
        match p {
            PhaseSave::Calm => Phase::Calm,
            PhaseSave::Warning => Phase::Warning,
            PhaseSave::Blizzard => Phase::Blizzard,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherSave {
    pub phase: PhaseSave,
    pub timer: f32,
    pub blizzards: u32,
}

impl Default for WeatherSave {
    fn default() -> Self {
        let w = Weather::new();
        WeatherSave { phase: w.phase.into(), timer: w.timer, blizzards: w.blizzards }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FogSave {
    /// Seen cells as alternating run lengths (see `Fog::seen_runs`). Empty means nothing seen.
    pub runs: Vec<u32>,
    pub found: Vec<bool>,
}

/// A position in the world to the nearest quarter metre, used to name a
/// container or pickup in a save (the world is the same every game).
pub type WorldKey = [i32; 2];

pub fn world_key(x: f32, z: f32) -> WorldKey {
    [(x * 4.0).round() as i32, (z * 4.0).round() as i32]
}

/// A container you've taken some (not all) of: which entries are gone.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LootedSave {
    pub key: WorldKey,
    pub items: Vec<u8>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveGame {
    pub version: u32,
    /// Seconds since 1970 when it was written.
    pub saved_at: u64,
    /// Where you were, for the slot label ("near Lundgren's Fish House").
    pub place: String,
    pub play_secs: f32,
    pub kills: u32,
    pub player: PlayerSave,
    pub survival: SurvivalSave,
    pub inventory: InventorySave,
    pub arsenal: ArsenalSave,
    pub clock: ClockSave,
    pub weather: WeatherSave,
    pub fog: FogSave,
    /// Containers you've opened.
    pub opened: Vec<WorldKey>,
    /// Containers you've taken some things from.
    pub looted: Vec<LootedSave>,
    /// Pickups you've taken.
    pub collected: Vec<WorldKey>,
    /// The interior you're in, if any (an id from the interiors list).
    pub interior: Option<String>,
    /// Quest and story flags.
    pub flags: Vec<String>,
}

impl Default for SaveGame {
    fn default() -> Self {
        SaveGame {
            version: SAVE_VERSION,
            saved_at: 0,
            place: String::new(),
            play_secs: 0.0,
            kills: 0,
            player: PlayerSave::default(),
            survival: SurvivalSave::default(),
            inventory: InventorySave::default(),
            arsenal: ArsenalSave::default(),
            clock: ClockSave::default(),
            weather: WeatherSave::default(),
            fog: FogSave::default(),
            opened: Vec::new(),
            looted: Vec::new(),
            collected: Vec::new(),
            interior: None,
            flags: Vec::new(),
        }
    }
}

/// Everything about the running game that a save records.
pub struct Snapshot<'a> {
    pub survival: &'a Survival,
    pub inv: &'a Inventory,
    pub arsenal: &'a Arsenal,
    pub kills: u32,
    pub clock: &'a Clock,
    pub weather: &'a Weather,
    pub fog: &'a Fog,
    pub player: PlayerSave,
    pub play_secs: f32,
    pub saved_at: u64,
    pub opened: Vec<WorldKey>,
    pub looted: Vec<LootedSave>,
    pub collected: Vec<WorldKey>,
    pub interior: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoadError {
    /// Not valid JSON, or cut short.
    Corrupt,
    /// Valid JSON, but not one of our saves.
    NotASave,
    /// Written by a newer version of the game.
    TooNew(u32),
    /// Older than the oldest format we can read.
    TooOld(u32),
    /// Right shape of file, wrong contents.
    Invalid(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Corrupt => write!(f, "That save file is damaged."),
            LoadError::NotASave => write!(f, "That isn't a Fallout: Minnesota save."),
            LoadError::TooNew(v) => write!(f, "That save was made by a newer version of the game (format {v})."),
            LoadError::TooOld(v) => write!(f, "That save is too old to load (format {v})."),
            LoadError::Invalid(_) => write!(f, "That save has unexpected contents."),
        }
    }
}

/// Bring a save written in format `from` up to [`SAVE_VERSION`]. There is
/// nothing to upgrade yet; when the format changes, add a step here that
/// rewrites the value for the next version (and bump `OLDEST_SUPPORTED` when
/// a very old format is finally dropped).
fn migrate(value: Value, from: u32) -> Result<Value, LoadError> {
    match from {
        SAVE_VERSION => Ok(value),
        other => Err(LoadError::TooOld(other)),
    }
}

fn finite_or(v: f32, def: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        def
    }
}

fn cap<T>(list: &mut Vec<T>) {
    list.truncate(MAX_LIST);
}

impl SaveGame {
    pub fn capture(s: &Snapshot) -> SaveGame {
        let mut save = SaveGame {
            version: SAVE_VERSION,
            saved_at: s.saved_at,
            place: place_name(s.player.x, s.player.z),
            play_secs: s.play_secs,
            kills: s.kills,
            player: s.player,
            survival: SurvivalSave { health: s.survival.health, body_heat: s.survival.body_heat, rads: s.survival.rads, frostbite: s.survival.frostbite },
            inventory: InventorySave {
                stimpaks: s.inv.stimpaks,
                radaway: s.inv.radaway,
                hotdish: s.inv.hotdish,
                ammo_reserve: s.inv.ammo_reserve,
                shells: s.inv.shells,
                revolver_rounds: s.inv.revolver_rounds,
                scrap: s.inv.scrap,
                pelts: s.inv.pelts,
                has_frostfang_coat: s.inv.has_frostfang_coat,
            },
            arsenal: ArsenalSave {
                owned: s.arsenal.owned,
                current: s.arsenal.current,
                weapons: s.arsenal.weapons.iter().map(|w| WeaponSave { mag: w.mag, jammed: w.jammed, upgrades: w.upgrades }).collect(),
            },
            clock: ClockSave { hours: s.clock.hours, day: s.clock.day },
            weather: WeatherSave { phase: s.weather.phase.into(), timer: s.weather.timer, blizzards: s.weather.blizzards },
            fog: FogSave { runs: s.fog.seen_runs(), found: s.fog.landmarks_found.clone() },
            opened: s.opened.clone(),
            looted: s.looted.clone(),
            collected: s.collected.clone(),
            interior: s.interior.clone(),
            flags: s.flags.clone(),
        };
        // NaN would be written as `null` and never read back.
        save.sanitize();
        save
    }

    /// Pull every value into a legal range and fix up anything inconsistent.
    pub fn sanitize(&mut self) {
        self.version = SAVE_VERSION;
        self.play_secs = finite_or(self.play_secs, 0.0).clamp(0.0, 1.0e8);
        // An interior we don't know (from a newer game, or edited) means outdoors.
        if self.interior.as_deref().is_some_and(|id| Interior::parse(id).is_none()) {
            self.interior = None;
        }
        let d = PlayerSave::default();
        let p = &mut self.player;
        let on_map = |v: f32| v.is_finite() && v.abs() <= HALF_SIZE - 2.0;
        // An interior sits off the map, so only clamp outdoors.
        if self.interior.is_none() {
            if !(on_map(p.x) && on_map(p.z)) {
                (p.x, p.z) = (d.x, d.z);
            }
        } else {
            p.x = finite_or(p.x, d.x).clamp(-5000.0, 5000.0);
            p.z = finite_or(p.z, d.z).clamp(-5000.0, 5000.0);
            // Standing outside the room you're said to be in: put you at its door.
            let room = self.interior.as_deref().and_then(Interior::parse);
            if room.is_some_and(|r| interiors::zone_at(p.x, p.z) != Some(r)) {
                let (ex, ez, _) = room.map(Interior::entry).unwrap_or((d.x, d.z, 0.0));
                (p.x, p.z, p.y) = (ex, ez, interiors::FLOOR_Y);
            }
        }
        p.y = finite_or(p.y, 0.0).clamp(-500.0, 500.0);
        p.yaw = finite_or(p.yaw, 0.0);
        p.pitch = finite_or(p.pitch, 0.0).clamp(-1.5, 1.5);

        let s = &mut self.survival;
        let def = SurvivalSave::default();
        s.body_heat = finite_or(s.body_heat, def.body_heat).clamp(0.0, 100.0);
        s.rads = finite_or(s.rads, def.rads).clamp(0.0, Survival::MAX_RADS);
        // A living player has at least a sliver of health, and no more than radiation allows.
        let max_health = (Survival::BASE_MAX_HEALTH - s.rads / 10.0).max(1.0);
        s.health = finite_or(s.health, def.health).clamp(1.0, max_health);

        let i = &mut self.inventory;
        for count in [&mut i.stimpaks, &mut i.radaway, &mut i.hotdish, &mut i.ammo_reserve, &mut i.shells, &mut i.revolver_rounds, &mut i.scrap, &mut i.pelts] {
            *count = (*count).min(MAX_COUNT);
        }

        let a = &mut self.arsenal;
        a.weapons.truncate(4);
        while a.weapons.len() < 4 {
            a.weapons.push(WeaponSave::default());
        }
        if !a.owned.iter().any(|o| *o) {
            a.owned[0] = true;
        }
        if a.current >= 4 || !a.owned[a.current] {
            a.current = a.owned.iter().position(|o| *o).unwrap_or(0);
        }

        let c = &mut self.clock;
        c.hours = finite_or(c.hours, 7.5).rem_euclid(24.0);
        c.day = c.day.clamp(1, 100_000);

        let w = &mut self.weather;
        w.timer = finite_or(w.timer, 60.0).clamp(0.0, 600.0);
        w.blizzards = w.blizzards.min(100_000);

        // Fog runs must cover the map exactly; otherwise forget them.
        let total: u64 = self.fog.runs.iter().map(|r| *r as u64).sum();
        if total != (FOG_CELLS * FOG_CELLS) as u64 {
            self.fog.runs.clear();
        }
        cap(&mut self.fog.runs);
        cap(&mut self.fog.found);
        cap(&mut self.opened);
        cap(&mut self.collected);
        for list in [&mut self.opened, &mut self.collected] {
            list.sort_unstable();
            list.dedup();
        }
        // Partly looted: sorted, no repeats, no impossible entries, and nothing
        // that's also marked fully opened.
        cap(&mut self.looted);
        for l in &mut self.looted {
            l.items.retain(|i| (*i as usize) < 16);
            l.items.sort_unstable();
            l.items.dedup();
        }
        let opened: std::collections::HashSet<WorldKey> = self.opened.iter().copied().collect();
        self.looted.retain(|l| !l.items.is_empty() && !opened.contains(&l.key));
        self.looted.sort_by_key(|l| l.key);
        self.looted.dedup_by_key(|l| l.key);
        self.flags.retain(|f| !f.is_empty() && f.len() <= 80);
        self.flags.sort();
        self.flags.dedup();
        self.flags.truncate(500);
        self.place.truncate(80);
    }

    /// Put this save's state into the running game's pieces. The save should
    /// already be sanitised (parse and capture both do that).
    #[allow(clippy::too_many_arguments)]
    pub fn apply(&self, survival: &mut Survival, inv: &mut Inventory, arsenal: &mut Arsenal, kills: &mut u32, clock: &mut Clock, weather: &mut Weather, fog: &mut Fog) {
        let s = &self.survival;
        *survival = Survival { health: s.health, body_heat: s.body_heat, rads: s.rads, frostbite: s.frostbite, god: survival.god };
        let i = &self.inventory;
        *inv = Inventory {
            stimpaks: i.stimpaks,
            radaway: i.radaway,
            hotdish: i.hotdish,
            ammo_reserve: i.ammo_reserve,
            shells: i.shells,
            revolver_rounds: i.revolver_rounds,
            scrap: i.scrap,
            pelts: i.pelts,
            has_frostfang_coat: i.has_frostfang_coat,
        };
        *arsenal = Arsenal::starting();
        arsenal.owned = self.arsenal.owned;
        arsenal.current = self.arsenal.current.min(3);
        for (slot, kind) in WeaponKind::ALL.iter().enumerate() {
            let saved = self.arsenal.weapons.get(slot).copied().unwrap_or_default();
            let mut w = Weapon::new(*kind);
            for up in Upgrade::ALL {
                if saved.upgrades[up as usize] {
                    // An upgrade that no longer suits the weapon is simply skipped.
                    let _ = w.apply_upgrade(up);
                }
            }
            w.mag = saved.mag.min(w.mag_size);
            w.jammed = saved.jammed;
            arsenal.weapons[slot] = w;
        }
        *kills = self.kills;
        *clock = Clock { hours: self.clock.hours, day: self.clock.day };
        *weather = Weather { phase: self.weather.phase.into(), timer: self.weather.timer, blizzards: self.weather.blizzards };
        if self.fog.runs.is_empty() {
            *fog = Fog::new();
            fog.restore(&[(FOG_CELLS * FOG_CELLS) as u32], &self.fog.found);
        } else {
            fog.restore(&self.fog.runs, &self.fog.found);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Read a save from text, upgrading old formats and refusing unusable ones.
    pub fn parse(text: &str) -> Result<SaveGame, LoadError> {
        let value: Value = serde_json::from_str(text).map_err(|_| LoadError::Corrupt)?;
        let version = value.get("version").and_then(Value::as_u64).ok_or(LoadError::NotASave)?;
        if version > SAVE_VERSION as u64 {
            return Err(LoadError::TooNew(version.min(u32::MAX as u64) as u32));
        }
        if version < OLDEST_SUPPORTED as u64 {
            return Err(LoadError::TooOld(version as u32));
        }
        let value = migrate(value, version as u32)?;
        let mut save: SaveGame = serde_json::from_value(value).map_err(|e| LoadError::Invalid(e.to_string()))?;
        save.sanitize();
        Ok(save)
    }

    pub fn level(&self) -> u32 {
        let places = self.fog.found.iter().filter(|f| **f).count();
        let upgrades = self.arsenal.weapons.iter().map(|w| w.upgrades.iter().filter(|u| **u).count()).sum();
        let flags: super::quest::Flags = self.flags.iter().cloned().collect();
        progress::level(progress::experience(self.kills, places, upgrades, self.inventory.has_frostfang_coat, super::quest::quest_xp(&flags))).0
    }

    /// One line for a slot in the menu: day and time, level, place, play time.
    pub fn describe(&self) -> String {
        let h = self.clock.hours.floor() as u32 % 24;
        let m = ((self.clock.hours.fract() * 60.0) as u32).min(59);
        let secs = self.play_secs as u64;
        let played = if secs >= 3600 { format!("{}h {:02}m", secs / 3600, secs % 3600 / 60) } else { format!("{}m", secs / 60) };
        let place = if self.interior.is_some() { "indoors".to_string() } else { self.place.clone() };
        format!("Day {} {:02}:{:02}  Lvl {}  {}  {}", self.clock.day, h, m, self.level(), place, played)
    }
}

/// "inside Lundgren's Fish House" in a room, "near Lundgren's Fish House" for
/// the closest named place within 90 m, otherwise "Mille Lacs".
pub fn place_name(x: f32, z: f32) -> String {
    if let Some(room) = interiors::zone_at(x, z) {
        return format!("inside {}", room.name());
    }
    mapdata::landmarks()
        .iter()
        .map(|l| (l, (l.x - x).hypot(l.z - z)))
        .filter(|(_, d)| *d < 90.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(l, _)| format!("near {}", l.name))
        .unwrap_or_else(|| "Mille Lacs".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game with something unusual about every part of it.
    struct Rig {
        survival: Survival,
        inv: Inventory,
        arsenal: Arsenal,
        clock: Clock,
        weather: Weather,
        fog: Fog,
    }

    fn rig() -> Rig {
        let mut survival = Survival::new();
        survival.health = 63.0;
        survival.body_heat = 41.5;
        survival.rads = 230.0;
        survival.frostbite = true;
        let mut inv = Inventory::starting_kit();
        inv.stimpaks = 5;
        inv.scrap = 21;
        inv.pelts = 2;
        inv.shells = 8;
        inv.has_frostfang_coat = true;
        let mut arsenal = Arsenal::starting();
        arsenal.unlock(WeaponKind::ScrapShotgun);
        arsenal.unlock(WeaponKind::IceAxe);
        arsenal.weapons[0].apply_upgrade(Upgrade::ExtendedMag).unwrap();
        arsenal.weapons[0].apply_upgrade(Upgrade::HeavyLoads).unwrap();
        arsenal.weapons[0].mag = 7;
        arsenal.weapons[1].apply_upgrade(Upgrade::Choke).unwrap();
        arsenal.weapons[1].mag = 1;
        arsenal.weapons[1].jammed = true;
        arsenal.current = 1;
        let clock = Clock { hours: 21.25, day: 4 };
        let weather = Weather { phase: Phase::Blizzard, timer: 33.5, blizzards: 3 };
        let mut fog = Fog::new();
        fog.reveal(0.0, 150.0, 40.0);
        fog.reveal(-90.0, -30.0, 30.0);
        fog.landmarks_found[0] = true;
        fog.landmarks_found[3] = true;
        Rig { survival, inv, arsenal, clock, weather, fog }
    }

    fn snapshot(r: &Rig) -> Snapshot<'_> {
        Snapshot {
            survival: &r.survival,
            inv: &r.inv,
            arsenal: &r.arsenal,
            kills: 17,
            clock: &r.clock,
            weather: &r.weather,
            fog: &r.fog,
            player: PlayerSave { x: -78.0, y: 1.25, z: -41.0, yaw: 2.0, pitch: -0.3 },
            play_secs: 4_000.0,
            saved_at: 1_700_000_000,
            opened: vec![[10, 20], [-4, 7]],
            looted: vec![LootedSave { key: [3, 4], items: vec![1, 0, 1] }],
            collected: vec![[100, -100]],
            interior: None,
            flags: vec!["met_olson".into(), "convoy_found".into()],
        }
    }

    fn restore(save: &SaveGame) -> (Survival, Inventory, Arsenal, u32, Clock, Weather, Fog) {
        let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 0, Clock::new(), Weather::new(), Fog::new());
        save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
        (s, i, a, k, c, w, f)
    }

    #[test]
    fn a_save_round_trips_through_text_and_restores_everything() {
        let r = rig();
        let save = SaveGame::capture(&snapshot(&r));
        let again = SaveGame::parse(&save.to_json()).expect("our own file reads back");
        assert_eq!(again, save);

        let (s, i, a, kills, c, w, f) = restore(&again);
        assert_eq!((s.health, s.body_heat, s.rads, s.frostbite), (63.0, 41.5, 230.0, true));
        assert_eq!((i.stimpaks, i.scrap, i.pelts, i.shells, i.has_frostfang_coat), (5, 21, 2, 8, true));
        assert_eq!(kills, 17);
        assert_eq!((c.hours, c.day), (21.25, 4));
        assert_eq!((w.phase, w.timer, w.blizzards), (Phase::Blizzard, 33.5, 3));
        assert_eq!(f.seen_count(), r.fog.seen_count());
        assert_eq!(f.landmarks_found, r.fog.landmarks_found);
        assert_eq!(a.owned, [true, true, false, true]);
        assert_eq!(a.current, 1);
        // Upgrades are re-applied, so their effects come back with them.
        let rifle = a.get(WeaponKind::PipeRifle);
        assert!(rifle.has_upgrade(Upgrade::ExtendedMag) && rifle.has_upgrade(Upgrade::HeavyLoads));
        assert_eq!(rifle.mag_size, r.arsenal.weapons[0].mag_size);
        assert_eq!(rifle.damage, r.arsenal.weapons[0].damage);
        assert_eq!(rifle.mag, 7);
        let shotgun = a.get(WeaponKind::ScrapShotgun);
        assert!(shotgun.has_upgrade(Upgrade::Choke) && shotgun.jammed);
        assert_eq!(shotgun.spread, r.arsenal.weapons[1].spread);
        assert_eq!(shotgun.mag, 1);
        assert!(!a.get(WeaponKind::Revolver).has_upgrade(Upgrade::Choke));
        assert_eq!(again.opened, vec![[-4, 7], [10, 20]], "kept (sorted)");
        assert_eq!(again.looted, vec![LootedSave { key: [3, 4], items: vec![0, 1] }], "partial loot kept, tidied");
        assert_eq!(again.collected, vec![[100, -100]]);
        assert_eq!(again.flags, vec!["convoy_found".to_string(), "met_olson".to_string()]);
    }

    #[test]
    fn what_a_save_does_not_cover_is_reset_cleanly() {
        let r = rig();
        let save = SaveGame::capture(&snapshot(&r));
        let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 0, Clock::new(), Weather::new(), Fog::new());
        a.weapons[0].busy = 1.0;
        a.draw = 0.3;
        save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
        assert_eq!(a.weapons[0].busy, 0.0, "no half-finished reload");
        assert_eq!(a.draw, 0.0);
        assert_eq!(a.weapons[0].cooldown, 0.0);
    }

    #[test]
    fn loading_overwrites_a_game_already_in_progress() {
        let r = rig();
        let save = SaveGame::capture(&snapshot(&r));
        let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 99, Clock::new(), Weather::new(), Fog::new());
        f.reveal(100.0, 100.0, 60.0);
        i.hotdish = 40;
        a.weapons[2].apply_upgrade(Upgrade::HeavyLoads).unwrap();
        save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
        assert_eq!(k, 17);
        assert_eq!(i.hotdish, r.inv.hotdish, "not the 40 held before");
        assert!(!a.weapons[2].has_upgrade(Upgrade::HeavyLoads), "upgrades fitted after the save are gone");
        assert_eq!(f.seen_count(), r.fog.seen_count(), "map seen after the save is forgotten");
    }

    #[test]
    fn a_new_games_save_is_valid_and_describes_the_start() {
        let save = SaveGame::default();
        let again = SaveGame::parse(&save.to_json()).unwrap();
        assert_eq!(again, save);
        assert!(again.describe().starts_with("Day 1 07:30  Lvl 1"));
        assert_eq!(again.level(), 1);
    }

    #[test]
    fn describing_a_save_shows_day_time_level_place_and_play_time() {
        let r = rig();
        let save = SaveGame::capture(&snapshot(&r));
        let text = save.describe();
        assert!(text.starts_with("Day 4 21:15"), "{text}");
        assert!(text.contains("1h 06m"), "{text}");
        assert!(text.contains(&save.place), "{text}");
        assert!(save.level() > 1, "17 kills and a few finds make a higher level");
        let short = SaveGame { play_secs: 600.0, interior: Some("fish_house_1".into()), ..save };
        let t = short.describe();
        assert!(t.contains("10m") && t.contains("indoors"), "{t}");
    }

    #[test]
    fn places_are_named_when_close_and_generic_when_far() {
        let first = &mapdata::landmarks()[0];
        assert_eq!(place_name(first.x, first.z + 5.0), format!("near {}", first.name));
        assert_eq!(place_name(-9000.0, 9000.0), "Mille Lacs");
        let (ox, oz) = Interior::VaultLobby.origin();
        assert_eq!(place_name(ox, oz), "inside Vault 143");
    }

    #[test]
    fn a_save_that_isnt_json_or_isnt_ours_is_refused_politely() {
        assert_eq!(SaveGame::parse(""), Err(LoadError::Corrupt));
        assert_eq!(SaveGame::parse("not json"), Err(LoadError::Corrupt));
        assert_eq!(SaveGame::parse("{\"version\": 1, \"player\": "), Err(LoadError::Corrupt), "cut off mid-write");
        assert_eq!(SaveGame::parse("{}"), Err(LoadError::NotASave), "an empty object must not load as a fresh game");
        assert_eq!(SaveGame::parse("[1, 2, 3]"), Err(LoadError::NotASave));
        assert_eq!(SaveGame::parse("{\"version\": \"one\"}"), Err(LoadError::NotASave));
        for e in [LoadError::Corrupt, LoadError::NotASave, LoadError::TooNew(9), LoadError::TooOld(0), LoadError::Invalid("x".into())] {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn newer_and_older_formats_are_rejected_rather_than_misread() {
        let newer = format!("{{\"version\": {}, \"future_field\": true}}", SAVE_VERSION + 1);
        assert_eq!(SaveGame::parse(&newer), Err(LoadError::TooNew(SAVE_VERSION + 1)));
        assert_eq!(SaveGame::parse("{\"version\": 99999999999}"), Err(LoadError::TooNew(u32::MAX)));
        assert_eq!(SaveGame::parse("{\"version\": 0}"), Err(LoadError::TooOld(0)));
        assert!(LoadError::TooNew(2).to_string().contains("newer"));
    }

    #[test]
    fn missing_fields_take_defaults_and_unknown_ones_are_ignored() {
        let save = SaveGame::parse(r#"{ "version": 1, "kills": 3, "mystery": {"a": 1}, "player": { "x": 12.0 } }"#).unwrap();
        assert_eq!(save.kills, 3);
        assert_eq!(save.player.x, 12.0);
        assert_eq!(save.player.z, PlayerSave::default().z, "missing z keeps its default");
        assert_eq!(save.inventory, InventorySave::default());
        assert_eq!(save.arsenal.owned, [true, false, false, false]);
    }

    #[test]
    fn wrong_types_are_an_error_not_a_crash() {
        for bad in [
            r#"{ "version": 1, "kills": "many" }"#,
            r#"{ "version": 1, "player": 5 }"#,
            r#"{ "version": 1, "weather": { "phase": "Hail" } }"#,
            r#"{ "version": 1, "survival": { "health": null } }"#,
            r#"{ "version": 1, "inventory": { "stimpaks": -3 } }"#,
        ] {
            assert!(matches!(SaveGame::parse(bad), Err(LoadError::Invalid(_))), "{bad}");
        }
    }

    #[test]
    fn out_of_range_values_are_pulled_into_range() {
        let save = SaveGame::parse(
            r#"{ "version": 1,
                 "player": { "x": 99999.0, "z": -99999.0, "pitch": 9.0 },
                 "survival": { "health": 5000.0, "body_heat": -40.0, "rads": 900.0 },
                 "inventory": { "stimpaks": 4000000000, "scrap": 50000 },
                 "arsenal": { "owned": [false, false, false, false], "current": 17, "weapons": [] },
                 "clock": { "hours": 61.0, "day": 0 },
                 "weather": { "phase": "Warning", "timer": 99999.0 } }"#,
        )
        .unwrap();
        assert_eq!((save.player.x, save.player.z), (PlayerSave::default().x, PlayerSave::default().z), "off the map: back to the start");
        assert_eq!(save.player.pitch, 1.5);
        assert_eq!(save.survival.body_heat, 0.0);
        assert_eq!(save.survival.rads, 900.0);
        assert_eq!(save.survival.health, 10.0, "900 rads leave room for only 10 HP");
        assert_eq!((save.inventory.stimpaks, save.inventory.scrap), (MAX_COUNT, MAX_COUNT));
        assert_eq!(save.arsenal.owned, [true, false, false, false], "you always own at least one weapon");
        assert_eq!(save.arsenal.current, 0);
        assert_eq!(save.arsenal.weapons.len(), 4, "missing weapon entries are filled in");
        assert!((0.0..24.0).contains(&save.clock.hours));
        assert_eq!(save.clock.day, 1);
        assert_eq!(save.weather.timer, 600.0);
    }

    #[test]
    fn a_dead_or_dying_save_is_given_a_sliver_of_health() {
        let save = SaveGame::parse(r#"{ "version": 1, "survival": { "health": 0.0 } }"#).unwrap();
        assert_eq!(save.survival.health, 1.0, "you can't load into the death screen");
    }

    #[test]
    fn the_current_weapon_must_be_one_you_own() {
        let save = SaveGame::parse(r#"{ "version": 1, "arsenal": { "owned": [true, false, true, false], "current": 1 } }"#).unwrap();
        assert_eq!(save.arsenal.current, 0, "slot 1 isn't owned, so back to the first weapon you have");
        let save = SaveGame::parse(r#"{ "version": 1, "arsenal": { "owned": [false, false, true, false], "current": 3 } }"#).unwrap();
        assert_eq!(save.arsenal.current, 2);
    }

    #[test]
    fn interiors_keep_their_off_map_positions_but_outdoor_positions_are_checked() {
        let (ox, oz) = Interior::FishHouse(1).origin();
        let inside = SaveGame::parse(&format!(r#"{{ "version": 1, "interior": "fish_house_1", "player": {{ "x": {ox}, "y": 0.0, "z": {oz} }} }}"#)).unwrap();
        assert_eq!((inside.player.x, inside.player.z), (ox, oz));
        let outside = SaveGame::parse(r#"{ "version": 1, "player": { "x": 1500.0, "z": 1500.0 } }"#).unwrap();
        assert_ne!(outside.player.x, 1500.0);
    }

    #[test]
    fn an_unknown_interior_means_outdoors_and_a_stray_position_goes_to_the_door() {
        let lost = SaveGame::parse(r#"{ "version": 1, "interior": "castle", "player": { "x": 1000.0, "z": 1000.0 } }"#).unwrap();
        assert_eq!(lost.interior, None, "a room we don't know isn't a room");
        assert_eq!((lost.player.x, lost.player.z), (PlayerSave::default().x, PlayerSave::default().z), "and the off-map position is replaced");
        let wandered = SaveGame::parse(r#"{ "version": 1, "interior": "vault_lobby", "player": { "x": 5.0, "z": 5.0, "y": 9.0 } }"#).unwrap();
        let (ex, ez, _) = Interior::VaultLobby.entry();
        assert_eq!((wandered.player.x, wandered.player.z, wandered.player.y), (ex, ez, interiors::FLOOR_Y), "you're put at the lobby's door");
        let fine = SaveGame::parse(&format!(r#"{{ "version": 1, "interior": "mart", "player": {{ "x": {}, "z": {} }} }}"#, Interior::Mart.origin().0, Interior::Mart.origin().1)).unwrap();
        assert_eq!(fine.interior.as_deref(), Some("mart"));
        assert_eq!(fine.player.x, Interior::Mart.origin().0, "a position inside its room is kept");
    }

    #[test]
    fn partly_looted_containers_are_tidied_and_never_double_counted() {
        let mut save = SaveGame {
            opened: vec![[1, 1]],
            looted: vec![
                LootedSave { key: [1, 1], items: vec![0] },
                LootedSave { key: [2, 2], items: vec![] },
                LootedSave { key: [3, 3], items: vec![2, 99, 2] },
            ],
            ..SaveGame::default()
        };
        save.sanitize();
        assert_eq!(save.looted, vec![LootedSave { key: [3, 3], items: vec![2] }]);
        // An old save with no such field still loads.
        let old = SaveGame::parse(r#"{ "version": 1, "opened": [[5, 5]] }"#).unwrap();
        assert!(old.looted.is_empty());
    }

    #[test]
    fn fog_that_does_not_cover_the_map_is_dropped_but_the_rest_loads() {
        let save = SaveGame::parse(r#"{ "version": 1, "kills": 4, "fog": { "runs": [3, 4, 5], "found": [true] } }"#).unwrap();
        assert!(save.fog.runs.is_empty(), "wrong total: forget the fog");
        assert_eq!(save.kills, 4, "everything else is kept");
        let (_, _, _, _, _, _, fog) = restore(&save);
        assert_eq!(fog.seen_count(), 0);
        assert!(fog.landmarks_found[0], "found places still apply");
    }

    #[test]
    fn huge_lists_and_odd_flags_are_trimmed() {
        let mut save = SaveGame { opened: (0..MAX_LIST as i32 + 500).map(|i| [i, 0]).collect(), ..SaveGame::default() };
        save.flags = vec!["".into(), "a".into(), "a".into(), "x".repeat(200)];
        save.sanitize();
        assert_eq!(save.opened.len(), MAX_LIST);
        assert_eq!(save.flags, vec!["a".to_string()], "empty, duplicate and absurdly long flags removed");
    }

    #[test]
    fn nan_never_reaches_the_file() {
        let mut r = rig();
        r.survival.rads = f32::NAN;
        r.clock.hours = f32::INFINITY;
        let mut snap = snapshot(&r);
        snap.player.x = f32::NAN;
        let save = SaveGame::capture(&snap);
        let text = save.to_json();
        // (A null for "no interior" is fine; a null where a number belongs is not.)
        let again = SaveGame::parse(&text).expect("a save made from broken numbers still loads");
        assert!(again.survival.rads.is_finite() && again.clock.hours.is_finite() && again.player.x.is_finite());
        assert_eq!(again.player.x, PlayerSave::default().x, "an impossible position becomes the start");
    }

    #[test]
    fn world_keys_are_stable_to_a_quarter_metre() {
        assert_eq!(world_key(10.0, 20.0), world_key(10.05, 19.95));
        assert_ne!(world_key(10.0, 20.0), world_key(10.5, 20.0));
        assert_eq!(world_key(-3.0, 0.0), [-12, 0]);
    }
}
