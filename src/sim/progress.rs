//! Experience and level, shown on the Pip-Boy. You earn XP for kills,
//! places discovered, weapon upgrades and crafting your coat.

/// Total XP for what you've done so far.
pub fn experience(kills: u32, places: usize, upgrades: usize, coat: bool) -> u32 {
    kills * 25 + places as u32 * 20 + upgrades as u32 * 15 + if coat { 40 } else { 0 }
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

    #[test]
    fn levels_climb_with_experience() {
        assert_eq!(level(0), (1, 0, 100));
        assert_eq!(level(99).0, 1);
        assert_eq!(level(100), (2, 0, 200));
        assert_eq!(level(650), (4, 50, 400));
        assert!(experience(4, 3, 1, true) > experience(4, 3, 1, false));
        assert_eq!(experience(0, 0, 0, false), 0);
    }
}
