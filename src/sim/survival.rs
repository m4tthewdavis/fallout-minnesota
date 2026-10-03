//! Body Heat, radiation, health and inventory rules.
//!
//! Design doc: "Body Heat meter - a fourth survival need. It drains with wind
//! chill and night. At 0 the player gains Frostbite."

/// Everything about the player's surroundings that affects survival this tick.
#[derive(Clone, Copy, Debug)]
pub struct Exposure {
    /// Air temperature in degrees Fahrenheit.
    pub air_temp_f: f32,
    /// Extra degrees of cold from wind (ignored while sheltered).
    pub wind_chill_f: f32,
    /// Inside a fish house / warming shelter.
    pub sheltered: bool,
    /// Next to a fire barrel or other heat source.
    pub near_heat: bool,
    /// Incoming radiation, rads per second, before resistance.
    pub rads_per_sec: f32,
    pub sprinting: bool,
    /// 0.0 = naked, 0.9 = best possible insulation.
    pub insulation: f32,
    /// 0.0 = no protection, 1.0 = immune.
    pub rad_resist: f32,
}

impl Default for Exposure {
    fn default() -> Self {
        Exposure {
            air_temp_f: 0.0,
            wind_chill_f: 0.0,
            sheltered: false,
            near_heat: false,
            rads_per_sec: 0.0,
            sprinting: false,
            insulation: 0.0,
            rad_resist: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    Cold,
    Radiation,
    Wounds,
}

impl DeathCause {
    pub fn describe(self) -> &'static str {
        match self {
            DeathCause::Cold => "You froze solid. Another statue for the Long Winter.",
            DeathCause::Radiation => "The glowing snow finally got you.",
            DeathCause::Wounds => "Mauled in the snow. The Frostfangs eat well tonight.",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Survival {
    pub health: f32,
    /// 0..=100. At 0 the player has frostbite.
    pub body_heat: f32,
    /// 0..=MAX_RADS. Every 10 rads removes 1 max health.
    pub rads: f32,
    pub frostbite: bool,
}

impl Default for Survival {
    fn default() -> Self {
        Self::new()
    }
}

impl Survival {
    pub const BASE_MAX_HEALTH: f32 = 100.0;
    pub const MAX_RADS: f32 = 1000.0;
    /// Heat points per second lost per degree F below freezing.
    pub const COLD_LOSS_PER_DEGREE: f32 = 0.012;
    pub const FIRE_WARMTH_PER_SEC: f32 = 12.0;
    pub const FROSTBITE_DAMAGE_PER_SEC: f32 = 4.0;

    pub fn new() -> Self {
        Survival {
            health: Self::BASE_MAX_HEALTH,
            body_heat: 100.0,
            rads: 0.0,
            frostbite: false,
        }
    }

    /// Radiation eats into the top of the health bar, as in mainline Fallout.
    pub fn max_health(&self) -> f32 {
        (Self::BASE_MAX_HEALTH - self.rads / 10.0).max(0.0)
    }

    /// The temperature the player's body actually feels.
    pub fn effective_temp(exp: &Exposure) -> f32 {
        if exp.sheltered {
            exp.air_temp_f + 25.0
        } else {
            exp.air_temp_f - exp.wind_chill_f
        }
    }

    /// Body Heat change per second (positive = warming).
    pub fn heat_rate(exp: &Exposure) -> f32 {
        if exp.near_heat {
            return Self::FIRE_WARMTH_PER_SEC;
        }
        let eff = Self::effective_temp(exp);
        let insulation = exp.insulation.clamp(0.0, 0.9);
        let loss = (32.0 - eff).max(0.0) * Self::COLD_LOSS_PER_DEGREE * (1.0 - insulation);
        let exertion = if exp.sprinting { 0.35 } else { 0.0 };
        exertion - loss
    }

    /// Advance survival by `dt` seconds. Returns a cause if the player died.
    pub fn tick(&mut self, exp: &Exposure, dt: f32) -> Option<DeathCause> {
        self.body_heat = (self.body_heat + Self::heat_rate(exp) * dt).clamp(0.0, 100.0);

        let resist = exp.rad_resist.clamp(0.0, 1.0);
        self.rads = (self.rads + exp.rads_per_sec.max(0.0) * (1.0 - resist) * dt).min(Self::MAX_RADS);

        self.frostbite = self.body_heat <= 0.0;
        if self.frostbite {
            self.health -= Self::FROSTBITE_DAMAGE_PER_SEC * dt;
        } else if self.body_heat > 60.0 {
            // A warm body slowly heals.
            self.health += 0.5 * dt;
        }
        self.health = self.health.min(self.max_health());

        if self.rads >= Self::MAX_RADS {
            self.health = 0.0;
            return Some(DeathCause::Radiation);
        }
        if self.health <= 0.0 {
            self.health = 0.0;
            return Some(if self.frostbite {
                DeathCause::Cold
            } else {
                DeathCause::Wounds
            });
        }
        None
    }

    pub fn damage(&mut self, amount: f32) -> Option<DeathCause> {
        self.health -= amount;
        if self.health <= 0.0 {
            self.health = 0.0;
            Some(if self.frostbite {
                DeathCause::Cold
            } else {
                DeathCause::Wounds
            })
        } else {
            None
        }
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health());
    }

    pub fn warm(&mut self, amount: f32) {
        self.body_heat = (self.body_heat + amount).clamp(0.0, 100.0);
    }

    pub fn purge_rads(&mut self, amount: f32) {
        self.rads = (self.rads - amount).max(0.0);
    }
}

use super::combat::{Ammo, Upgrade, Weapon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Stimpak,
    RadAway,
    Hotdish,
    /// Pipe-rifle rounds.
    Ammo,
    Shells,
    RevolverRounds,
    Scrap,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Stimpak => "Stimpak",
            Item::RadAway => "RadAway",
            Item::Hotdish => "Vault 143 Hotdish",
            Item::Ammo => "Pipe rounds",
            Item::Shells => "Shotgun shells",
            Item::RevolverRounds => "Revolver rounds",
            Item::Scrap => "Scrap",
        }
    }

    /// How many you get from one pickup.
    pub fn amount(self) -> u32 {
        match self {
            Item::Ammo => Inventory::AMMO_PER_BOX,
            Item::Shells => Inventory::SHELLS_PER_BOX,
            Item::RevolverRounds => Inventory::REVOLVER_PER_BOX,
            Item::Scrap => Inventory::SCRAP_PER_PILE,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Inventory {
    pub stimpaks: u32,
    pub radaway: u32,
    pub hotdish: u32,
    /// Pipe-rifle rounds in reserve.
    pub ammo_reserve: u32,
    pub shells: u32,
    pub revolver_rounds: u32,
    pub scrap: u32,
    pub pelts: u32,
    pub has_frostfang_coat: bool,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::starting_kit()
    }
}

impl Inventory {
    pub const PELTS_FOR_COAT: u32 = 3;
    pub const AMMO_PER_BOX: u32 = 12;
    pub const SHELLS_PER_BOX: u32 = 6;
    pub const REVOLVER_PER_BOX: u32 = 12;
    pub const SCRAP_PER_PILE: u32 = 3;

    /// What the Overseer hands you at the Vault 143 door.
    pub fn starting_kit() -> Self {
        Inventory {
            stimpaks: 2,
            radaway: 1,
            hotdish: 2,
            ammo_reserve: 24,
            shells: 0,
            revolver_rounds: 0,
            scrap: 0,
            pelts: 0,
            has_frostfang_coat: false,
        }
    }

    /// Vault 143 jumpsuit + parka = 0.25; the Frostfang coat adds 0.35.
    pub fn insulation(&self) -> f32 {
        0.25 + if self.has_frostfang_coat { 0.35 } else { 0.0 }
    }

    pub fn add(&mut self, item: Item) {
        match item {
            Item::Stimpak => self.stimpaks += 1,
            Item::RadAway => self.radaway += 1,
            Item::Hotdish => self.hotdish += 1,
            Item::Ammo => self.ammo_reserve += Self::AMMO_PER_BOX,
            Item::Shells => self.shells += Self::SHELLS_PER_BOX,
            Item::RevolverRounds => self.revolver_rounds += Self::REVOLVER_PER_BOX,
            Item::Scrap => self.scrap += Self::SCRAP_PER_PILE,
        }
    }

    /// Reserve ammunition of a kind.
    pub fn reserve(&self, ammo: Ammo) -> u32 {
        match ammo {
            Ammo::PipeRounds => self.ammo_reserve,
            Ammo::Shells => self.shells,
            Ammo::RevolverRounds => self.revolver_rounds,
        }
    }

    pub fn reserve_mut(&mut self, ammo: Ammo) -> &mut u32 {
        match ammo {
            Ammo::PipeRounds => &mut self.ammo_reserve,
            Ammo::Shells => &mut self.shells,
            Ammo::RevolverRounds => &mut self.revolver_rounds,
        }
    }

    /// Spend scrap to fit an upgrade to a weapon at a workbench.
    pub fn craft_upgrade(&mut self, weapon: &mut Weapon, up: Upgrade) -> Result<(), &'static str> {
        if !up.applies_to(weapon.kind) {
            return Err("That upgrade doesn't fit this weapon.");
        }
        if weapon.has_upgrade(up) {
            return Err("Already fitted.");
        }
        if self.scrap < up.scrap_cost() {
            return Err("Not enough scrap.");
        }
        weapon.apply_upgrade(up)?;
        self.scrap -= up.scrap_cost();
        Ok(())
    }

    pub fn craft_coat(&mut self) -> Result<(), &'static str> {
        if self.has_frostfang_coat {
            return Err("You already wear a Frostfang coat.");
        }
        if self.pelts < Self::PELTS_FOR_COAT {
            return Err("You need 3 Frostfang pelts to craft a coat.");
        }
        self.pelts -= Self::PELTS_FOR_COAT;
        self.has_frostfang_coat = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outside(temp: f32, wind: f32) -> Exposure {
        Exposure {
            air_temp_f: temp,
            wind_chill_f: wind,
            insulation: 0.25,
            ..Default::default()
        }
    }

    #[test]
    fn cold_drains_heat_and_fire_restores_it() {
        let mut s = Survival::new();
        let exp = outside(-35.0, 40.0);
        for _ in 0..100 {
            s.tick(&exp, 0.1);
        }
        assert!(s.body_heat < 95.0, "heat should drop in a blizzard: {}", s.body_heat);
        let before = s.body_heat;
        let fire = Exposure { near_heat: true, ..exp };
        s.tick(&fire, 1.0);
        assert!(s.body_heat > before);
    }

    #[test]
    fn blizzard_is_much_colder_than_calm() {
        let calm = Survival::heat_rate(&outside(-8.0, 10.0));
        let storm = Survival::heat_rate(&outside(-35.0, 40.0));
        assert!(storm < calm * 1.8, "calm {calm}, storm {storm}");
    }

    #[test]
    fn coat_slows_heat_loss() {
        let mut inv = Inventory::starting_kit();
        let base = outside(-35.0, 40.0);
        let without = Survival::heat_rate(&Exposure {
            insulation: inv.insulation(),
            ..base
        });
        inv.pelts = 3;
        inv.craft_coat().unwrap();
        let with = Survival::heat_rate(&Exposure {
            insulation: inv.insulation(),
            ..base
        });
        assert!(with > without);
        assert_eq!(inv.pelts, 0);
        assert!(inv.craft_coat().is_err());
    }

    #[test]
    fn frostbite_kills_by_cold() {
        let mut s = Survival::new();
        s.body_heat = 0.0;
        let exp = outside(-60.0, 40.0);
        let mut cause = None;
        for _ in 0..1000 {
            if let Some(c) = s.tick(&exp, 0.1) {
                cause = Some(c);
                break;
            }
        }
        assert_eq!(cause, Some(DeathCause::Cold));
    }

    #[test]
    fn radiation_lowers_max_health_and_can_kill() {
        let mut s = Survival::new();
        let exp = Exposure {
            air_temp_f: 40.0,
            rads_per_sec: 100.0,
            ..Default::default()
        };
        s.tick(&exp, 3.0);
        assert!((s.max_health() - 70.0).abs() < 0.01);
        assert!(s.health <= 70.0);
        let cause = s.tick(&exp, 10.0);
        assert_eq!(cause, Some(DeathCause::Radiation));
    }

    #[test]
    fn shelter_blocks_wind() {
        let open = outside(-20.0, 30.0);
        let inside = Exposure {
            sheltered: true,
            ..open
        };
        assert!(Survival::effective_temp(&inside) > Survival::effective_temp(&open));
    }

    #[test]
    fn items_stack() {
        let mut inv = Inventory::starting_kit();
        inv.add(Item::Ammo);
        assert_eq!(inv.ammo_reserve, 36);
        inv.add(Item::Stimpak);
        assert_eq!(inv.stimpaks, 3);
    }

    #[test]
    fn every_item_adds_to_the_right_pile() {
        let mut inv = Inventory::starting_kit();
        inv.add(Item::Shells);
        inv.add(Item::RevolverRounds);
        inv.add(Item::Scrap);
        assert_eq!((inv.shells, inv.revolver_rounds, inv.scrap), (6, 12, 3));
        assert_eq!(inv.reserve(Ammo::Shells), 6);
        *inv.reserve_mut(Ammo::RevolverRounds) -= 2;
        assert_eq!(inv.revolver_rounds, 10);
        assert_eq!(Item::Scrap.amount(), 3);
        assert_eq!(Item::Stimpak.amount(), 1);
        for item in [Item::Ammo, Item::Shells, Item::RevolverRounds, Item::Scrap] {
            assert!(item.amount() > 1, "{item:?}");
        }
    }

    #[test]
    fn crafting_an_upgrade_spends_scrap() {
        use crate::sim::combat::WeaponKind;
        let mut inv = Inventory::starting_kit();
        let mut rifle = Weapon::new(WeaponKind::PipeRifle);
        assert_eq!(inv.craft_upgrade(&mut rifle, Upgrade::InsulatedAction), Err("Not enough scrap."));
        inv.scrap = 20;
        assert_eq!(inv.craft_upgrade(&mut rifle, Upgrade::InsulatedAction), Ok(()));
        assert_eq!(inv.scrap, 14);
        assert!(rifle.has_upgrade(Upgrade::InsulatedAction));
        assert_eq!(inv.craft_upgrade(&mut rifle, Upgrade::InsulatedAction), Err("Already fitted."));
        assert_eq!(inv.craft_upgrade(&mut rifle, Upgrade::Choke), Err("That upgrade doesn't fit this weapon."));
        assert_eq!(inv.scrap, 14, "failed crafts cost nothing");
    }
}
