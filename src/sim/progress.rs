//! Experience and level, shown on the Pip-Boy. You earn XP for kills,
//! places discovered, weapon upgrades and crafting your coat.

use super::quest::{self, Flags};

/// Total XP for what you've done so far; `quest` is what the quest line has
/// paid out. The game uses [`breakdown`]; the tests check it against this.
#[cfg(test)]
pub fn experience(kills: u32, places: usize, upgrades: usize, coat: bool, quest: u32) -> u32 {
    kills * 25 + places as u32 * 20 + upgrades as u32 * 15 + if coat { 40 } else { 0 } + quest
}

/// Where XP came from, so the game can say what a gain was for.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Breakdown {
    pub kills: u32,
    pub places: u32,
    pub upgrades: u32,
    pub coat: u32,
    pub quest: u32,
}

impl Breakdown {
    pub fn new(kills: u32, places: usize, upgrades: usize, coat: bool, quest: u32) -> Breakdown {
        Breakdown { kills: kills * 25, places: places as u32 * 20, upgrades: upgrades as u32 * 15, coat: if coat { 40 } else { 0 }, quest }
    }

    pub fn total(&self) -> u32 {
        self.kills + self.places + self.upgrades + self.coat + self.quest
    }

    /// (level, XP into this level, XP this level spans) for this much XP.
    pub fn level(&self) -> (u32, u32, u32) {
        level(self.total())
    }
}

/// How many places the player has discovered (landmark flags that are set).
pub fn places_found(landmarks_found: &[bool]) -> usize {
    landmarks_found.iter().filter(|f| **f).count()
}

/// Where the player's XP comes from right now, from the raw game state. The
/// Pip-Boy header, the level-up banner, the perk check and the save-slot label
/// all call this, so they can never disagree about your level.
///
/// `upgrades` is [`Arsenal::upgrades_fitted`](super::combat::Arsenal::upgrades_fitted)
/// (or [`ArsenalSave::upgrades_fitted`](super::save::ArsenalSave::upgrades_fitted)
/// for a file on disk).
pub fn breakdown(kills: u32, landmarks_found: &[bool], upgrades: usize, coat: bool, flags: &Flags) -> Breakdown {
    Breakdown::new(kills, places_found(landmarks_found), upgrades, coat, quest::quest_xp(flags))
}

/// What the last change in XP was: (amount, why). If several things moved
/// at once, the biggest is named.
pub fn gain(before: &Breakdown, now: &Breakdown) -> Option<(u32, &'static str)> {
    let gains = [
        (now.kills.saturating_sub(before.kills), "Kill"),
        (now.places.saturating_sub(before.places), "Discovery"),
        (now.upgrades.saturating_sub(before.upgrades), "Weapon mod"),
        (now.coat.saturating_sub(before.coat), "Crafting"),
        (now.quest.saturating_sub(before.quest), "Objective"),
    ];
    if now.total() <= before.total() {
        return None;
    }
    let total: u32 = gains.iter().map(|g| g.0).sum();
    let best = gains.iter().max_by_key(|g| g.0)?;
    (total > 0).then_some((total, best.1))
}

/// XP needed to reach `level` (level 1 needs none, then 100, 300, 600...).
pub fn xp_for_level(level: u32) -> u32 {
    50 * level * level.saturating_sub(1)
}

/// (level, XP into this level, XP this level spans).
pub fn level(xp: u32) -> (u32, u32, u32) {
    let mut lvl = 1;
    while xp >= xp_for_level(lvl + 1) {
        lvl += 1;
    }
    let base = xp_for_level(lvl);
    (lvl, xp - base, xp_for_level(lvl + 1) - base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::combat::{Arsenal, Upgrade, WeaponKind};
    use crate::sim::mapdata::Fog;
    use crate::sim::rng::Rng;
    use crate::sim::save::{SaveGame, WeaponSave};
    use crate::sim::survival::{Inventory, Survival};
    use crate::sim::daynight::Clock;
    use crate::sim::weather::Weather;

    #[test]
    fn xp_gains_say_what_they_were_for() {
        let a = Breakdown::new(3, 2, 1, false, 0);
        assert_eq!(a.total(), experience(3, 2, 1, false, 0));
        assert_eq!(gain(&a, &Breakdown::new(4, 2, 1, false, 0)), Some((25, "Kill")));
        assert_eq!(gain(&a, &Breakdown::new(3, 3, 1, false, 0)), Some((20, "Discovery")));
        assert_eq!(gain(&a, &Breakdown::new(3, 2, 2, false, 0)), Some((15, "Weapon mod")));
        assert_eq!(gain(&a, &Breakdown::new(3, 2, 1, true, 0)), Some((40, "Crafting")));
        assert_eq!(gain(&a, &Breakdown::new(3, 2, 1, false, 100)), Some((100, "Objective")));
        assert_eq!(gain(&a, &a), None);
        // Several at once: the total, named for the biggest.
        assert_eq!(gain(&a, &Breakdown::new(4, 3, 1, false, 150)), Some((195, "Objective")));
        // XP going down (loading an earlier save) is not a gain.
        assert_eq!(gain(&Breakdown::new(9, 0, 0, false, 0), &a), None);
    }

    /// The Pip-Boy's old `total_upgrades`, kept here as the yardstick.
    fn old_total_upgrades(arsenal: &Arsenal) -> usize {
        WeaponKind::ALL.iter().filter(|k| arsenal.owned[k.slot()]).map(|k| Upgrade::ALL.iter().filter(|u| arsenal.get(*k).has_upgrade(**u)).count()).sum()
    }

    /// Every flag that pays quest XP.
    fn quest_flags() -> Vec<&'static str> {
        let mut v = vec![quest::REPORTED, quest::PUMP_FIXED, quest::PUMP_REPORTED];
        v.extend(quest::Objective::ALL.iter().map(|o| o.flag()));
        v.extend(quest::Part::ALL.iter().map(|p| p.flag()));
        v
    }

    /// A random but legal arsenal: weapons found in any order, upgrades fitted
    /// only where they suit (the game refuses the rest).
    fn random_arsenal(rng: &mut Rng) -> Arsenal {
        let mut a = Arsenal::starting();
        for k in WeaponKind::ALL {
            if rng.chance(0.5) {
                a.unlock(k);
            }
        }
        for slot in 0..4 {
            if a.owned[slot] {
                for up in Upgrade::ALL {
                    if rng.chance(0.5) {
                        let _ = a.weapons[slot].apply_upgrade(up);
                    }
                }
            }
        }
        a
    }

    #[test]
    fn one_breakdown_gives_the_same_level_as_the_four_old_copies() {
        let mut rng = Rng::new(11);
        let mut levels_seen = std::collections::BTreeSet::new();
        for _ in 0..2_000 {
            let arsenal = random_arsenal(&mut rng);
            let kills = (rng.f32() * 40.0) as u32;
            let found: Vec<bool> = (0..crate::sim::mapdata::landmarks().len()).map(|_| rng.chance(0.5)).collect();
            let coat = rng.chance(0.5);
            let mut flags = Flags::new();
            for flag in quest_flags() {
                if rng.chance(0.5) {
                    flags.insert(flag.to_string());
                }
            }

            assert_eq!(arsenal.upgrades_fitted(), old_total_upgrades(&arsenal));
            // quest.rs / pipboy.rs / fo4ui.rs, before:
            let places = found.iter().filter(|f| **f).count();
            let old_xp = experience(kills, places, old_total_upgrades(&arsenal), coat, quest::quest_xp(&flags));
            let old_breakdown = Breakdown::new(kills, places, old_total_upgrades(&arsenal), coat, quest::quest_xp(&flags));
            // after:
            let now = breakdown(kills, &found, arsenal.upgrades_fitted(), coat, &flags);
            assert_eq!(now, old_breakdown);
            assert_eq!(now.total(), old_xp);
            assert_eq!(now.level(), level(old_xp));
            levels_seen.insert(now.level().0);
        }
        assert!(levels_seen.len() > 3, "the sample covers a spread of levels: {levels_seen:?}");
    }

    #[test]
    fn a_save_slot_shows_the_level_the_game_will_show_after_loading_it() {
        let mut rng = Rng::new(5);
        for _ in 0..300 {
            let arsenal = random_arsenal(&mut rng);
            let found: Vec<bool> = (0..crate::sim::mapdata::landmarks().len()).map(|_| rng.chance(0.5)).collect();
            let mut save = SaveGame::default();
            save.kills = (rng.f32() * 30.0) as u32;
            save.fog.found = found.clone();
            save.inventory.has_frostfang_coat = rng.chance(0.5);
            save.arsenal.owned = arsenal.owned;
            save.arsenal.weapons = arsenal.weapons.iter().map(|w| WeaponSave { mag: w.mag, jammed: w.jammed, upgrades: w.upgrades }).collect();
            let flags: Flags = save.flags.iter().cloned().collect();
            let live = breakdown(save.kills, &found, arsenal.upgrades_fitted(), save.inventory.has_frostfang_coat, &flags).level().0;
            assert_eq!(save.level(), live);
        }
        // Hand-edited nonsense (upgrades on a weapon you do not own, or that
        // do not suit it) earns nothing, just as it would after loading.
        let mut save = SaveGame::default();
        save.arsenal.weapons[1].upgrades = [true; 4];
        save.arsenal.weapons[0].upgrades = [false, false, false, true];
        let mut a = Arsenal::starting();
        let mut rest = (Survival::new(), Inventory::starting_kit(), 0, Clock::new(), Weather::new(), Fog::new());
        save.apply(&mut rest.0, &mut rest.1, &mut a, &mut rest.2, &mut rest.3, &mut rest.4, &mut rest.5);
        assert_eq!(save.arsenal.upgrades_fitted(), a.upgrades_fitted());
        assert_eq!(a.upgrades_fitted(), 1);
    }

    #[test]
    fn levels_climb_with_experience() {
        assert_eq!(level(0), (1, 0, 100));
        assert_eq!(level(99).0, 1);
        assert_eq!(level(100), (2, 0, 200));
        assert_eq!(level(650), (4, 50, 400));
        assert!(experience(4, 3, 1, true, 0) > experience(4, 3, 1, false, 0));
        assert_eq!(experience(0, 0, 0, false, 0), 0);
        assert_eq!(experience(0, 0, 0, false, 80), 80, "quest XP counts");
    }
}
