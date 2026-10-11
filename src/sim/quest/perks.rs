//! Perks you pick as you level.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Perk {
    FrostHardy,
    RadResistant,
    QuickHands,
    Scrounger,
    FieldMedic,
}

impl Perk {
    pub const ALL: [Perk; 5] = [Perk::FrostHardy, Perk::RadResistant, Perk::QuickHands, Perk::Scrounger, Perk::FieldMedic];

    pub fn id(self) -> &'static str {
        match self {
            Perk::FrostHardy => "frost_hardy",
            Perk::RadResistant => "rad_resistant",
            Perk::QuickHands => "quick_hands",
            Perk::Scrounger => "scrounger",
            Perk::FieldMedic => "field_medic",
        }
    }

    pub fn flag(self) -> String {
        format!("perk.{}", self.id())
    }

    pub fn name(self) -> &'static str {
        match self {
            Perk::FrostHardy => "Frost Hardy",
            Perk::RadResistant => "Rad Resistant",
            Perk::QuickHands => "Quick Hands",
            Perk::Scrounger => "Scrounger",
            Perk::FieldMedic => "Field Medic",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Perk::FrostHardy => "Your body loses heat 20% slower.",
            Perk::RadResistant => "You take 30% less radiation.",
            Perk::QuickHands => "Reloading takes 25% less time.",
            Perk::Scrounger => "Every scrap you find, you find one more.",
            Perk::FieldMedic => "Stimpaks, hotdish and the stove heal 50% more.",
        }
    }
}

pub fn has_perk(flags: &Flags, p: Perk) -> bool {
    has(flags, &p.flag())
}

pub fn perks_taken(flags: &Flags) -> Vec<Perk> {
    Perk::ALL.into_iter().filter(|p| has_perk(flags, *p)).collect()
}

/// Perk picks you've earned but not used: one per level after the first, and
/// never more than there are perks left.
pub fn picks_available(level: u32, flags: &Flags) -> usize {
    let earned = level.saturating_sub(1) as usize;
    let taken = perks_taken(flags).len();
    let left = Perk::ALL.len() - taken;
    earned.saturating_sub(taken).min(left)
}

/// Up to three perks to choose from. Which three depends on how many you've
/// taken, so each pick shows something different.
pub fn offer(flags: &Flags) -> Vec<Perk> {
    let left: Vec<Perk> = Perk::ALL.into_iter().filter(|p| !has_perk(flags, *p)).collect();
    let start = perks_taken(flags).len() % left.len().max(1);
    (0..left.len().min(3)).map(|i| left[(start + i) % left.len()]).collect()
}

/// What your perks change, as multipliers and bonuses.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Effects {
    /// Added to your coat's insulation (0..1 scale).
    pub insulation: f32,
    /// Fraction of radiation resisted.
    pub rad_resist: f32,
    pub reload_time: f32,
    pub extra_scrap: u32,
    /// Healing from aid items and the stove is multiplied by this.
    pub aid_heal: f32,
}

impl Default for Effects {
    fn default() -> Self {
        Effects { insulation: 0.0, rad_resist: 0.0, reload_time: 1.0, extra_scrap: 0, aid_heal: 1.0 }
    }
}

pub fn effects(flags: &Flags) -> Effects {
    let mut e = Effects::default();
    if has_perk(flags, Perk::FrostHardy) {
        e.insulation = 0.2;
    }
    if has_perk(flags, Perk::RadResistant) {
        e.rad_resist = 0.3;
    }
    if has_perk(flags, Perk::QuickHands) {
        e.reload_time = 0.75;
    }
    if has_perk(flags, Perk::Scrounger) {
        e.extra_scrap = 1;
    }
    if has_perk(flags, Perk::FieldMedic) {
        e.aid_heal = 1.5;
    }
    e
}
