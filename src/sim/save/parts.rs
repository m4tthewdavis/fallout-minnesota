//! The saved form of each part of the game (player, vitals, pack, weapons, clock, weather, fog, looted containers).

use super::*;

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

impl ArsenalSave {
    /// How many upgrades the game would show as fitted once this is loaded:
    /// only on weapons you own, and only ones that suit the weapon (loading
    /// skips the rest). Matches `Arsenal::upgrades_fitted` after `apply`.
    pub fn upgrades_fitted(&self) -> usize {
        WeaponKind::ALL
            .iter()
            .filter(|k| self.owned[k.slot()])
            .map(|k| {
                let saved = self.weapons.get(k.slot()).copied().unwrap_or_default();
                Upgrade::ALL.iter().filter(|u| saved.upgrades[**u as usize] && u.applies_to(*k)).count()
            })
            .sum()
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
