//! The loot list you see when you look at a container: what each entry says,
//! how the highlight moves and scrolls, and which entries are still there.
//! Pure rules; `fo4ui.rs` draws it.

use super::loot::Loot;
use super::survival::Item;

/// Rows shown at once.
pub const ROWS: usize = 6;

/// The tag in front of an entry: "[Ammo] Pipe rounds (12)".
pub fn category(l: &Loot) -> &'static str {
    match l {
        Loot::Item(Item::Ammo | Item::Shells | Item::RevolverRounds, _) => "Ammo",
        Loot::Item(Item::Scrap, _) => "Junk",
        Loot::Item(Item::Stimpak | Item::RadAway, _) => "Aid",
        Loot::Item(Item::Hotdish, _) => "Food",
        Loot::Weapon(_) => "Weapon",
    }
}

/// What an entry is called, with its count.
pub fn label(l: &Loot) -> String {
    match l {
        Loot::Item(item, n) => {
            let total = item.amount() * n;
            if total > 1 {
                format!("{} ({total})", item.name())
            } else {
                item.name().to_string()
            }
        }
        Loot::Weapon(kind) => kind.name().to_string(),
    }
}

/// Indices of the entries still in the container.
pub fn remaining(taken: &[bool]) -> Vec<usize> {
    taken.iter().enumerate().filter(|(_, t)| !**t).map(|(i, _)| i).collect()
}

/// Move the highlight by `delta` among `len` entries, wrapping round.
pub fn step(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (selected as i32 + delta).rem_euclid(len as i32) as usize
    }
}

/// The slice of `len` entries to show so `selected` is on screen: (start, end).
pub fn window(selected: usize, len: usize) -> (usize, usize) {
    let shown = ROWS.min(len);
    if shown == 0 {
        return (0, 0);
    }
    // Keep the highlight a couple of rows from the top while there's more below.
    let start = selected.saturating_sub(2).min(len - shown);
    (start, start + shown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::combat::WeaponKind;

    #[test]
    fn entries_are_tagged_and_counted() {
        let ammo = Loot::Item(Item::Ammo, 2);
        assert_eq!(category(&ammo), "Ammo");
        assert_eq!(label(&ammo), format!("Pipe rounds ({})", Item::Ammo.amount() * 2));
        assert_eq!(category(&Loot::Item(Item::Scrap, 1)), "Junk");
        assert_eq!(category(&Loot::Item(Item::Stimpak, 1)), "Aid");
        assert_eq!(label(&Loot::Item(Item::Stimpak, 1)), "Stimpak", "a single item has no count");
        assert_eq!(category(&Loot::Item(Item::Hotdish, 1)), "Food");
        assert_eq!((category(&Loot::Weapon(WeaponKind::ScrapShotgun)), label(&Loot::Weapon(WeaponKind::ScrapShotgun))), ("Weapon", "Scrap Shotgun".to_string()));
    }

    #[test]
    fn what_is_left_is_what_has_not_been_taken() {
        assert_eq!(remaining(&[false, true, false]), vec![0, 2]);
        assert!(remaining(&[true, true]).is_empty());
        assert!(remaining(&[]).is_empty());
    }

    #[test]
    fn the_highlight_wraps() {
        assert_eq!(step(0, -1, 3), 2);
        assert_eq!(step(2, 1, 3), 0);
        assert_eq!(step(1, 1, 3), 2);
        assert_eq!(step(0, 1, 0), 0);
    }

    #[test]
    fn the_window_always_shows_the_highlight() {
        for len in 0..=12usize {
            for sel in 0..len.max(1) {
                let (a, b) = window(sel, len);
                assert!(b <= len && b - a <= ROWS && a <= b, "{sel}/{len}: {a}..{b}");
                if len > 0 {
                    assert!((a..b).contains(&sel), "{sel}/{len} not in {a}..{b}");
                }
            }
        }
        assert_eq!(window(0, 3), (0, 3), "a short list shows whole");
        assert_eq!(window(0, 10), (0, ROWS));
        assert_eq!(window(9, 10), (10 - ROWS, 10));
    }
}
