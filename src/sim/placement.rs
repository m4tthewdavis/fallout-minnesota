//! Where scatter and loot may go: a random spot on open ground that keeps
//! clear of the vault, the ruins, the highway wrecks and anything solid.
//!
//! The world is built from one seeded `Rng`, so the number of draws each call
//! makes is part of the map: two per try, in x then z order, and nothing else.
//! Change that and every tree, rock and pickup moves.

use super::collision::SpatialGrid;
use super::rng::Rng;
use super::terrain::{self, CARS, HALF_SIZE, RUINS};

/// Where [`open_spot`] may put things: how far in from the map edge, how many
/// tries, and how far to stay from the vault, ruins and cars.
pub struct SpotRules {
    pub edge: f32,
    pub tries: usize,
    pub vault_clear: f32,
    pub ruin_pad: f32,
    pub car_clear: f32,
}

/// Trees, rocks and drifts: anywhere on open ground not in a ruin or on a car.
pub const SCATTER: SpotRules = SpotRules { edge: 0.0, tries: 40, vault_clear: 0.0, ruin_pad: 0.0, car_clear: 5.0 };
/// Loot and camps: well inside the map and clear of the vault, with margins round landmarks.
pub const PLACED: SpotRules = SpotRules { edge: 12.0, tries: 60, vault_clear: 30.0, ruin_pad: 4.0, car_clear: 6.0 };

/// A random point on open ground that obeys `rules` and is clear of `solid`.
pub fn open_spot(rng: &mut Rng, solid: &SpatialGrid, clearance: f32, rules: &SpotRules) -> Option<(f32, f32)> {
    for _ in 0..rules.tries {
        let x = rng.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
        let z = rng.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
        if !terrain::is_open_ground(x, z) || terrain::dist_to_vault(x, z) < rules.vault_clear {
            continue;
        }
        if RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r + rules.ruin_pad)
            || CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < rules.car_clear)
            || solid.blocked(x, z, clearance)
        {
            continue;
        }
        return Some((x, z));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::collision::Shape;

    /// The Bevy layer's version before it moved here (data from the same lists).
    fn old_open_spot(rng: &mut Rng, solid: &SpatialGrid, clearance: f32, rules: &SpotRules) -> Option<(f32, f32)> {
        const OLD_RUINS: [(f32, f32, f32); 2] = [(-40.0, -110.0, 20.0), (140.0, -140.0, 18.0)];
        const OLD_CARS: [(f32, f32, f32); 6] = [(-150.0, 95.0, 1.62), (-128.0, 92.0, 1.4), (-60.0, 98.0, 4.6), (60.0, 140.0, 2.6), (150.0, 100.0, 1.75), (165.0, 20.0, 1.7)];
        for _ in 0..rules.tries {
            let x = rng.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
            let z = rng.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
            if !terrain::is_open_ground(x, z) || terrain::dist_to_vault(x, z) < rules.vault_clear {
                continue;
            }
            if OLD_RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r + rules.ruin_pad)
                || OLD_CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < rules.car_clear)
                || solid.blocked(x, z, clearance)
            {
                continue;
            }
            return Some((x, z));
        }
        None
    }

    fn crowded_grid(seed: u64) -> SpatialGrid {
        let mut rng = Rng::new(seed);
        let mut grid = SpatialGrid::default();
        for _ in 0..300 {
            grid.push(Shape::Circle { x: rng.range(-200.0, 200.0), z: rng.range(-200.0, 200.0), r: rng.range(0.3, 6.0) });
        }
        grid
    }

    #[test]
    fn the_same_seed_still_puts_everything_in_the_same_place() {
        for seed in 0..40u64 {
            let grid = crowded_grid(seed);
            for (rules, clear) in [(&SCATTER, 1.0), (&SCATTER, 3.0), (&PLACED, 1.5)] {
                let (mut a, mut b) = (Rng::new(seed * 7 + 1), Rng::new(seed * 7 + 1));
                for _ in 0..20 {
                    assert_eq!(open_spot(&mut a, &grid, clear, rules), old_open_spot(&mut b, &grid, clear, rules));
                    // Same number of draws too, or the next call would drift.
                    assert_eq!(a.next_u64(), b.next_u64());
                }
            }
        }
    }

    #[test]
    fn a_spot_obeys_every_rule() {
        let grid = crowded_grid(3);
        let mut rng = Rng::new(9);
        let mut found = 0;
        for _ in 0..300 {
            let Some((x, z)) = open_spot(&mut rng, &grid, 1.5, &PLACED) else { continue };
            found += 1;
            assert!(x.abs() <= HALF_SIZE - PLACED.edge && z.abs() <= HALF_SIZE - PLACED.edge, "inside the margin");
            assert!(terrain::is_open_ground(x, z));
            assert!(terrain::dist_to_vault(x, z) >= PLACED.vault_clear, "clear of the vault");
            assert!(RUINS.iter().all(|&(rx, rz, r)| (x - rx).hypot(z - rz) >= r + PLACED.ruin_pad));
            assert!(CARS.iter().all(|&(cx, cz, _)| (x - cx).hypot(z - cz) >= PLACED.car_clear));
            assert!(!grid.blocked(x, z, 1.5));
        }
        assert!(found > 30, "most calls find room: {found}");
    }

    #[test]
    fn a_full_map_gives_up_after_its_tries_and_draws_exactly_that_many_numbers() {
        // One wall of solid ground over the whole map: nothing can be placed.
        let mut grid = SpatialGrid::default();
        grid.push(Shape::Rect { x0: -300.0, z0: -300.0, x1: 300.0, z1: 300.0 });
        let (mut a, mut b) = (Rng::new(1), Rng::new(1));
        assert_eq!(open_spot(&mut a, &grid, 1.0, &SCATTER), None);
        for _ in 0..SCATTER.tries * 2 {
            b.f32();
        }
        assert_eq!(a.next_u64(), b.next_u64(), "two draws per try, none after the last");
        // No tries means no draws and no spot.
        let none = SpotRules { tries: 0, ..SCATTER };
        let mut c = Rng::new(1);
        assert_eq!(open_spot(&mut c, &SpatialGrid::default(), 1.0, &none), None);
        assert_eq!(c.next_u64(), Rng::new(1).next_u64());
    }
}
