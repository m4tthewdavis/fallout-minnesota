//! Experience and level, shown on the Pip-Boy. You earn XP for kills,
//! places discovered, weapon upgrades and crafting your coat.

/// Total XP for what you've done so far; `quest` is what the quest line has paid out.
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
