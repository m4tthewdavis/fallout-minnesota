//! What lootable containers hold, and how it is handed over: random supply
//! caches (scrap, ammo, medicine, food) and the three weapon finds.

use super::combat::{Arsenal, WeaponKind};
use super::rng::Rng;
use super::survival::{Inventory, Item};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loot {
    /// An item picked up `n` times over (piles of scrap, boxes of ammo).
    Item(Item, u32),
    Weapon(WeaponKind),
}

impl Loot {
    pub fn describe(&self) -> String {
        match self {
            Loot::Item(item, n) => {
                let total = item.amount() * n;
                if total > 1 {
                    format!("{} x{}", item.name(), total)
                } else {
                    item.name().to_string()
                }
            }
            Loot::Weapon(kind) => kind.name().to_string(),
        }
    }
}

/// A random supply cache: two to four entries, always something useful for
/// survival, never a weapon.
pub fn roll_cache(rng: &mut Rng) -> Vec<Loot> {
    // (item, weight, most piles)
    const TABLE: [(Item, f32, u32); 8] = [
        (Item::Scrap, 4.0, 3),
        (Item::Ammo, 2.5, 1),
        (Item::Shells, 1.5, 1),
        (Item::RevolverRounds, 1.5, 1),
        (Item::Stimpak, 1.5, 1),
        (Item::RadAway, 1.0, 1),
        (Item::Hotdish, 1.5, 1),
        (Item::Scrap, 2.0, 2),
    ];
    let total: f32 = TABLE.iter().map(|t| t.1).sum();
    let count = 2 + (rng.f32() * 2.99) as usize;
    let mut out: Vec<Loot> = Vec::new();
    for _ in 0..count {
        let mut pick = rng.f32() * total;
        for (item, weight, most) in TABLE {
            pick -= weight;
            if pick <= 0.0 {
                let piles = 1 + (rng.f32() * most as f32) as u32 % most;
                match out.iter_mut().find(|l| matches!(l, Loot::Item(i, _) if *i == item)) {
                    Some(Loot::Item(_, n)) => *n += piles,
                    _ => out.push(Loot::Item(item, piles)),
                }
                break;
            }
        }
    }
    // Every cache is worth opening: make sure there is scrap or ammo in it.
    if !out.iter().any(|l| matches!(l, Loot::Item(Item::Scrap | Item::Ammo | Item::Shells | Item::RevolverRounds, _))) {
        out.push(Loot::Item(Item::Scrap, 1));
    }
    out
}

/// The container a weapon is found in: the weapon, a first load of its
/// ammunition and a little scrap to start tinkering.
pub fn weapon_cache(kind: WeaponKind) -> Vec<Loot> {
    let mut out = vec![Loot::Weapon(kind)];
    match kind {
        WeaponKind::PipeRifle => out.push(Loot::Item(Item::Ammo, 1)),
        WeaponKind::ScrapShotgun => out.push(Loot::Item(Item::Shells, 2)),
        WeaponKind::Revolver => out.push(Loot::Item(Item::RevolverRounds, 2)),
        WeaponKind::IceAxe => {}
    }
    out.push(Loot::Item(Item::Scrap, 2));
    out
}

/// Hand the loot over. Returns what was gained, ready to show the player.
pub fn grant(loot: &[Loot], inv: &mut Inventory, arsenal: &mut Arsenal) -> Vec<String> {
    let mut gained = Vec::new();
    for l in loot {
        match *l {
            Loot::Item(item, n) => {
                for _ in 0..n {
                    inv.add(item);
                }
                gained.push(l.describe());
            }
            Loot::Weapon(kind) => {
                if arsenal.unlock(kind) {
                    gained.push(format!("{} (press {} to equip)", kind.name(), kind.slot() + 1));
                } else {
                    // Already have one: take it apart for scrap.
                    inv.add(Item::Scrap);
                    gained.push(format!("spare {} (scrapped)", kind.name()));
                }
            }
        }
    }
    gained
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_are_never_empty_or_armed() {
        let mut rng = Rng::new(12);
        for _ in 0..2000 {
            let cache = roll_cache(&mut rng);
            assert!(cache.len() >= 2 || cache.iter().any(|l| matches!(l, Loot::Item(_, n) if *n > 1)));
            assert!(cache.iter().all(|l| matches!(l, Loot::Item(_, n) if *n >= 1)), "caches hold items only");
            assert!(cache.iter().any(|l| matches!(l, Loot::Item(Item::Scrap | Item::Ammo | Item::Shells | Item::RevolverRounds, _))));
        }
    }

    #[test]
    fn caches_vary_and_favour_scrap() {
        let mut rng = Rng::new(3);
        let mut scrap = 0;
        let mut kinds = std::collections::HashSet::new();
        for _ in 0..500 {
            for l in roll_cache(&mut rng) {
                if let Loot::Item(i, _) = l {
                    kinds.insert(i.name());
                    if i == Item::Scrap {
                        scrap += 1;
                    }
                }
            }
        }
        assert!(kinds.len() >= 6, "variety: {kinds:?}");
        assert!(scrap > 300, "scrap is the commonest find ({scrap})");
    }

    #[test]
    fn weapon_caches_unlock_the_weapon_and_start_its_ammo() {
        let mut inv = Inventory::starting_kit();
        let mut arsenal = Arsenal::starting();
        let msgs = grant(&weapon_cache(WeaponKind::ScrapShotgun), &mut inv, &mut arsenal);
        assert!(arsenal.owned[WeaponKind::ScrapShotgun.slot()]);
        assert_eq!(inv.shells, 12);
        assert_eq!(inv.scrap, 6);
        assert!(msgs.iter().any(|m| m.contains("Scrap Shotgun") && m.contains("press 2")));
        let msgs = grant(&weapon_cache(WeaponKind::Revolver), &mut inv, &mut arsenal);
        assert!(arsenal.owned[WeaponKind::Revolver.slot()] && inv.revolver_rounds == 24);
        assert!(msgs[0].contains("press 3"));
        grant(&weapon_cache(WeaponKind::IceAxe), &mut inv, &mut arsenal);
        assert!(arsenal.owned[WeaponKind::IceAxe.slot()]);
    }

    #[test]
    fn a_second_copy_of_a_weapon_is_scrapped() {
        let mut inv = Inventory::starting_kit();
        let mut arsenal = Arsenal::starting();
        grant(&[Loot::Weapon(WeaponKind::Revolver)], &mut inv, &mut arsenal);
        let before = inv.scrap;
        let msgs = grant(&[Loot::Weapon(WeaponKind::Revolver)], &mut inv, &mut arsenal);
        assert_eq!(inv.scrap, before + Item::Scrap.amount());
        assert!(msgs[0].contains("scrapped"));
    }

    #[test]
    fn loot_descriptions_show_totals() {
        assert_eq!(Loot::Item(Item::Scrap, 2).describe(), "Scrap x6");
        assert_eq!(Loot::Item(Item::Stimpak, 1).describe(), "Stimpak");
        assert_eq!(Loot::Item(Item::Shells, 2).describe(), "Shotgun shells x12");
        assert_eq!(Loot::Weapon(WeaponKind::IceAxe).describe(), "Ice Axe");
    }
}
