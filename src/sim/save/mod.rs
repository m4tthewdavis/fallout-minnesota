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

mod parts;

pub use self::parts::*;

#[cfg(test)]
mod tests;

/// The format this build writes.
pub const SAVE_VERSION: u32 = 1;
/// The oldest format this build can still read.
pub const OLDEST_SUPPORTED: u32 = 1;

/// Most entries any list in a save may hold (a guard against huge files).
const MAX_LIST: usize = 20_000;
/// Largest count of any one item.
const MAX_COUNT: u32 = 9_999;

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
        let flags: super::quest::Flags = self.flags.iter().cloned().collect();
        progress::breakdown(self.kills, &self.fog.found, self.arsenal.upgrades_fitted(), self.inventory.has_frostfang_coat, &flags).level().0
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
