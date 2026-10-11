//! The words on the Pip-Boy: list pages, the STATUS readout and the header.
//! Pure text built from the rules' own types, so what the screen says is
//! unit-tested without a window; `pipboy.rs` only puts it in text nodes.

use super::combat::{Arsenal, Upgrade, WeaponKind};
use super::daynight::Clock;
use super::menu::{slot_name, AUTOSAVE, QUICKSAVE, SLOTS};
use super::pipnav::Page;
use super::survival::{Inventory, Survival};
use super::weather::Phase;

/// A text bar: `[####------]`.
pub fn bar(v: f32, width: usize) -> String {
    let n = (v.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("[{}{}]", "#".repeat(n), "-".repeat(width - n))
}

/// The header line: level and XP, vitals and the date. `level` is
/// `progress::level(xp)`: (level, XP into it, XP it spans).
pub fn header(s: &Survival, level: (u32, u32, u32), clock: &Clock) -> String {
    let (lvl, into, span) = level;
    format!(
        "LVL {}   HP {:.0}/{:.0}   HEAT {:.0}%   RAD {:.0}   XP {}/{}   DAY {} {}",
        lvl,
        s.health,
        s.max_health(),
        s.body_heat,
        s.rads,
        into,
        span,
        clock.day,
        clock.label()
    )
}

/// How the STATUS page reads your state, which also picks the mascot's pose.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Condition {
    /// Health as a share of the current maximum.
    pub hp: f32,
    pub cold: bool,
    pub hurt: bool,
    pub irradiated: bool,
}

impl Condition {
    pub fn of(s: &Survival) -> Condition {
        let hp = s.health / s.max_health().max(1.0);
        Condition { hp, cold: s.body_heat < 40.0, hurt: hp < 0.35, irradiated: s.rads > 100.0 }
    }
}

/// The list of things wrong with (or right about) you, one line each.
pub fn status_effects(s: &Survival, c: &Condition, has_coat: bool, sheltered: bool) -> Vec<&'static str> {
    let mut effects = Vec::new();
    if c.cold {
        effects.push("COLD  heat draining fast");
    }
    if s.frostbite {
        effects.push("FROSTBITE  -HP over time");
    }
    if c.irradiated {
        effects.push("IRRADIATED  max HP reduced");
    }
    if c.hurt {
        effects.push("BADLY HURT  use a Stimpak (H)");
    }
    if has_coat {
        effects.push("FROSTFANG COAT  +insulation");
    }
    if sheltered {
        effects.push("SHELTERED  warming up");
    }
    if effects.is_empty() {
        effects.push("None. Feeling Minnesota fine.");
    }
    effects
}

/// The STATUS page's main text: conditions, weather, radiation. `air` and
/// `feels` are in Fahrenheit; `timer` is the weather phase's seconds left.
pub fn status_text(s: &Survival, effects: &[&str], phase: Phase, timer: f32, air: f32, feels: f32) -> String {
    let weather_line = match phase {
        Phase::Calm => "Calm".to_string(),
        Phase::Warning => format!("SIREN: blizzard in {:.0}s", timer.max(0.0)),
        Phase::Blizzard => format!("RAD-BLIZZARD {:.0}s", timer.max(0.0)),
    };
    format!(
        "CONDITIONS\n{}\n\nWEATHER\n  {}\n  Air {:.0}F, feels {:.0}F\n\nRADIATION\n  {:.0} rads\n  {}",
        effects.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n"),
        weather_line,
        air,
        feels,
        s.rads,
        match Survival::BASE_MAX_HEALTH - s.max_health() {
            lost if lost >= 0.5 => format!("Max HP -{lost:.0}"),
            _ => "Max HP unaffected".to_string(),
        },
    )
}

/// The column beside the mascot.
pub fn status_side(s: &Survival, kills: u32, pelts: u32, scrap: u32) -> String {
    format!(
        "THAWBORN\nVault 143\n\nHP    {:.0}/{:.0}\nHEAT  {:.0}%\nRADS  {:.0}\n\nKILLS {}\nPELTS {}\nSCRAP {}",
        s.health,
        s.max_health(),
        s.body_heat,
        s.rads,
        kills,
        pelts,
        scrap
    )
}

/// The rows and details for a list page.
pub fn list_items(page: Page, inv: &Inventory, arsenal: &Arsenal, slots: &[Option<String>]) -> Vec<(String, String)> {
    match page {
        Page::Special => [
            ("STRENGTH", 5, "Hauling hotdish and ice augers. Affects how hard you swing the ice axe."),
            ("PERCEPTION", 6, "Spotting a Frostfang against fresh snow, or a cache under a drift."),
            ("ENDURANCE", 6, "Mille Lacs winters are long. So is the Long Winter."),
            ("CHARISMA", 4, "Minnesota Nice. Mostly wasted on wolves."),
            ("INTELLIGENCE", 6, "Enough to fit a choke to a scrap shotgun at a bait shop bench."),
            ("AGILITY", 5, "How fast you get that magazine in with numb fingers."),
            ("LUCK", 5, "The vault opened. The bomb in the crater didn't. You're doing okay."),
        ]
        .into_iter()
        .map(|(n, v, d)| (format!("{n:<14}{v:>2}"), format!("{n}  {v}\n\n{d}")))
        .collect(),
        Page::Weapons => WeaponKind::ALL
            .iter()
            .filter(|k| arsenal.owned[k.slot()])
            .map(|&k| {
                let w = arsenal.get(k);
                let held = if arsenal.current == k.slot() { "*" } else { " " };
                let label = format!("{held}{}", k.name());
                let ammo = match k.ammo() {
                    Some(a) => format!("{}/{}  +{} {}", w.mag, w.mag_size, inv.reserve(a), a.name()),
                    None => "melee".to_string(),
                };
                let fitted: Vec<&str> = Upgrade::ALL.iter().filter(|u| w.has_upgrade(**u)).map(|u| u.name()).collect();
                let next = match w.next_upgrade() {
                    Some(up) if k.ammo().is_some() => format!("NEXT MOD  {} ({} scrap)\n  {}", up.name(), up.scrap_cost(), up.describe()),
                    _ => "Fully modded.".to_string(),
                };
                let detail = format!(
                    "{}\n\nDAM   {:.0}{}\nAMMO  {}\nRATE  {:.1}/s\nRANGE {:.0} m\nCND   {}\n\nMODS  {}\n\n{}",
                    k.name().to_uppercase(),
                    w.damage,
                    if w.pellets > 1 { format!(" x{}", w.pellets) } else { String::new() },
                    ammo,
                    1.0 / w.fire_interval.max(0.01),
                    w.range,
                    bar(if w.jammed { 0.4 } else { 0.9 }, 10),
                    if fitted.is_empty() { "none".to_string() } else { fitted.join(", ") },
                    next
                );
                (label, detail)
            })
            .collect(),
        Page::Apparel => {
            let mut v = vec![
                ("*Vault 143 Jumpsuit".to_string(), "VAULT 143 JUMPSUIT\n\nDT 1   WARMTH +5\n\nBlue and gold, freshly pressed in 2077.\nNot rated for minus forty.".to_string()),
                ("*Toque and Scarf".to_string(), "TOQUE AND SCARF\n\nDT 0   WARMTH +5\n\nKnitted by the Overseer's aunt.\nThe pom-pom is regulation.".to_string()),
                ("*Pip-Boy 3000".to_string(), "PIP-BOY 3000\n\nYour wrist computer: map, stats,\nGeiger counter. Battery: 197 years.".to_string()),
            ];
            if inv.has_frostfang_coat {
                v.insert(0, ("*Frostfang Coat".to_string(), "FROSTFANG COAT\n\nDT 3   WARMTH +40\n\nStitched from Frostfang pelts at a fish\nhouse workbench. Smells like wet wolf.".to_string()));
            } else {
                v.push((
                    " Frostfang Coat (not made)".to_string(),
                    format!("FROSTFANG COAT\n\nNot crafted yet: {} of {} pelts.\nPress C at a fish house.", inv.pelts, Inventory::PELTS_FOR_COAT),
                ));
            }
            v
        }
        Page::Aid => vec![
            (format!("Stimpak ({})", inv.stimpaks), "STIMPAK\n\n+40 HP\n\nENTER to use (or H any time).".to_string()),
            (format!("RadAway ({})", inv.radaway), "RADAWAY\n\n-150 rads\n\nENTER to use (or X any time).".to_string()),
            (format!("Vault 143 Hotdish ({})", inv.hotdish), "VAULT 143 HOTDISH\n\n+35 HEAT  +5 HP\n\nTater tots, cream of mushroom and\nsomething that glows. ENTER or F.".to_string()),
        ],
        Page::Saves => (0..SLOTS)
            .map(|slot| {
                let name = slot_name(slot);
                let summary = slots.get(slot).cloned().flatten().unwrap_or_else(|| "- empty -".to_string());
                let how = match slot {
                    AUTOSAVE => "Written for you when you sleep or take\nshelter. L loads it.".to_string(),
                    QUICKSAVE => "F4 saves here from anywhere in the game.\nENTER saves now, L loads it.".to_string(),
                    _ => "ENTER saves here, L loads it.".to_string(),
                };
                (format!("{name:<10}"), format!("{}\n\n{}\n\n{}", name.to_uppercase(), summary, how))
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Pip-Boy's old code, copied as it was (only `game.inv` and
    // `game.arsenal` became arguments), to prove the move changed no word on
    // the screen.
    mod old {
        use super::*;

        pub fn bar(v: f32, width: usize) -> String {
            let n = (v.clamp(0.0, 1.0) * width as f32).round() as usize;
            format!("[{}{}]", "#".repeat(n), "-".repeat(width - n))
        }

        /// The rows and details for a list page.
        pub fn list_items(page: Page, inv: &Inventory, arsenal: &Arsenal, slots: &[Option<String>]) -> Vec<(String, String)> {
            match page {
                Page::Special => [
                    ("STRENGTH", 5, "Hauling hotdish and ice augers. Affects how hard you swing the ice axe."),
                    ("PERCEPTION", 6, "Spotting a Frostfang against fresh snow, or a cache under a drift."),
                    ("ENDURANCE", 6, "Mille Lacs winters are long. So is the Long Winter."),
                    ("CHARISMA", 4, "Minnesota Nice. Mostly wasted on wolves."),
                    ("INTELLIGENCE", 6, "Enough to fit a choke to a scrap shotgun at a bait shop bench."),
                    ("AGILITY", 5, "How fast you get that magazine in with numb fingers."),
                    ("LUCK", 5, "The vault opened. The bomb in the crater didn't. You're doing okay."),
                ]
                .into_iter()
                .map(|(n, v, d)| (format!("{n:<14}{v:>2}"), format!("{n}  {v}\n\n{d}")))
                .collect(),
                Page::Weapons => WeaponKind::ALL
                    .iter()
                    .filter(|k| arsenal.owned[k.slot()])
                    .map(|&k| {
                        let w = arsenal.get(k);
                        let held = if arsenal.current == k.slot() { "*" } else { " " };
                        let label = format!("{held}{}", k.name());
                        let ammo = match k.ammo() {
                            Some(a) => format!("{}/{}  +{} {}", w.mag, w.mag_size, inv.reserve(a), a.name()),
                            None => "melee".to_string(),
                        };
                        let fitted: Vec<&str> = Upgrade::ALL.iter().filter(|u| w.has_upgrade(**u)).map(|u| u.name()).collect();
                        let next = match w.next_upgrade() {
                            Some(up) if k.ammo().is_some() => format!("NEXT MOD  {} ({} scrap)\n  {}", up.name(), up.scrap_cost(), up.describe()),
                            _ => "Fully modded.".to_string(),
                        };
                        let detail = format!(
                            "{}\n\nDAM   {:.0}{}\nAMMO  {}\nRATE  {:.1}/s\nRANGE {:.0} m\nCND   {}\n\nMODS  {}\n\n{}",
                            k.name().to_uppercase(),
                            w.damage,
                            if w.pellets > 1 { format!(" x{}", w.pellets) } else { String::new() },
                            ammo,
                            1.0 / w.fire_interval.max(0.01),
                            w.range,
                            bar(if w.jammed { 0.4 } else { 0.9 }, 10),
                            if fitted.is_empty() { "none".to_string() } else { fitted.join(", ") },
                            next
                        );
                        (label, detail)
                    })
                    .collect(),
                Page::Apparel => {
                    let mut v = vec![
                        ("*Vault 143 Jumpsuit".to_string(), "VAULT 143 JUMPSUIT\n\nDT 1   WARMTH +5\n\nBlue and gold, freshly pressed in 2077.\nNot rated for minus forty.".to_string()),
                        ("*Toque and Scarf".to_string(), "TOQUE AND SCARF\n\nDT 0   WARMTH +5\n\nKnitted by the Overseer's aunt.\nThe pom-pom is regulation.".to_string()),
                        ("*Pip-Boy 3000".to_string(), "PIP-BOY 3000\n\nYour wrist computer: map, stats,\nGeiger counter. Battery: 197 years.".to_string()),
                    ];
                    if inv.has_frostfang_coat {
                        v.insert(0, ("*Frostfang Coat".to_string(), "FROSTFANG COAT\n\nDT 3   WARMTH +40\n\nStitched from Frostfang pelts at a fish\nhouse workbench. Smells like wet wolf.".to_string()));
                    } else {
                        v.push((
                            " Frostfang Coat (not made)".to_string(),
                            format!("FROSTFANG COAT\n\nNot crafted yet: {} of {} pelts.\nPress C at a fish house.", inv.pelts, Inventory::PELTS_FOR_COAT),
                        ));
                    }
                    v
                }
                Page::Aid => vec![
                    (format!("Stimpak ({})", inv.stimpaks), "STIMPAK\n\n+40 HP\n\nENTER to use (or H any time).".to_string()),
                    (format!("RadAway ({})", inv.radaway), "RADAWAY\n\n-150 rads\n\nENTER to use (or X any time).".to_string()),
                    (format!("Vault 143 Hotdish ({})", inv.hotdish), "VAULT 143 HOTDISH\n\n+35 HEAT  +5 HP\n\nTater tots, cream of mushroom and\nsomething that glows. ENTER or F.".to_string()),
                ],
                Page::Saves => (0..SLOTS)
                    .map(|slot| {
                        let name = slot_name(slot);
                        let summary = slots.get(slot).cloned().flatten().unwrap_or_else(|| "- empty -".to_string());
                        let how = match slot {
                            AUTOSAVE => "Written for you when you sleep or take\nshelter. L loads it.".to_string(),
                            QUICKSAVE => "F4 saves here from anywhere in the game.\nENTER saves now, L loads it.".to_string(),
                            _ => "ENTER saves here, L loads it.".to_string(),
                        };
                        (format!("{name:<10}"), format!("{}\n\n{}\n\n{}", name.to_uppercase(), summary, how))
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
    }

    const PAGES: [Page; 8] = [Page::Status, Page::Special, Page::Weapons, Page::Apparel, Page::Aid, Page::Map, Page::Notes, Page::Saves];

    fn states() -> Vec<(Inventory, Arsenal)> {
        let mut out = Vec::new();
        let mut rng = crate::sim::rng::Rng::new(21);
        for _ in 0..60 {
            let mut inv = Inventory::starting_kit();
            inv.has_frostfang_coat = rng.chance(0.5);
            inv.pelts = (rng.f32() * 5.0) as u32;
            inv.stimpaks = (rng.f32() * 9.0) as u32;
            inv.shells = (rng.f32() * 40.0) as u32;
            let mut a = Arsenal::starting();
            for k in WeaponKind::ALL {
                if rng.chance(0.6) {
                    a.unlock(k);
                }
            }
            for w in a.weapons.iter_mut() {
                for up in Upgrade::ALL {
                    if rng.chance(0.4) {
                        let _ = w.apply_upgrade(up);
                    }
                }
                w.jammed = rng.chance(0.3);
            }
            a.current = a.owned.iter().position(|o| *o).unwrap_or(0);
            out.push((inv, a));
        }
        out
    }

    #[test]
    fn every_list_page_reads_exactly_as_it_did() {
        let slot_sets: [[Option<String>; SLOTS]; 2] = [
            std::array::from_fn(|_| None),
            std::array::from_fn(|i| Some(format!("Day {} 07:30  Lvl {}  near Sven's Shanty  1m", i + 1, i + 2))),
        ];
        for (inv, arsenal) in states() {
            for slots in &slot_sets {
                for page in PAGES {
                    assert_eq!(list_items(page, &inv, &arsenal, slots), old::list_items(page, &inv, &arsenal, slots), "{page:?}");
                }
            }
        }
    }

    #[test]
    fn the_pages_say_what_a_player_expects() {
        let inv = Inventory::starting_kit();
        let arsenal = Arsenal::starting();
        let none: [Option<String>; SLOTS] = std::array::from_fn(|_| None);

        let weapons = list_items(Page::Weapons, &inv, &arsenal, &none);
        assert_eq!(weapons.len(), 1, "only the pipe rifle at the start");
        assert_eq!(weapons[0].0, "*Pipe Rifle", "the one in hand is starred");
        assert!(weapons[0].1.contains("AMMO  ") && weapons[0].1.contains("+24 pipe rounds"), "{}", weapons[0].1);
        assert!(weapons[0].1.contains("NEXT MOD"));

        let apparel = list_items(Page::Apparel, &inv, &arsenal, &none);
        assert_eq!(apparel.len(), 4);
        assert!(apparel[3].0.contains("not made") && apparel[3].1.contains("0 of 3 pelts"));
        let mut coated = inv.clone();
        coated.has_frostfang_coat = true;
        let apparel = list_items(Page::Apparel, &coated, &arsenal, &none);
        assert_eq!(apparel[0].0, "*Frostfang Coat", "the coat goes first once made");
        assert_eq!(apparel.len(), 4);

        let aid = list_items(Page::Aid, &inv, &arsenal, &none);
        assert_eq!(aid[0].0, format!("Stimpak ({})", inv.stimpaks));

        let saves = list_items(Page::Saves, &inv, &arsenal, &none);
        assert_eq!(saves.len(), SLOTS);
        assert!(saves[AUTOSAVE].1.contains("- empty -") && saves[AUTOSAVE].1.contains("Written for you"));
        assert!(saves[QUICKSAVE].1.contains("F4 saves here"));
        // A short list of summaries (or none at all) never panics.
        assert_eq!(list_items(Page::Saves, &inv, &arsenal, &[]).len(), SLOTS);

        assert_eq!(list_items(Page::Special, &inv, &arsenal, &none).len(), 7);
        assert!(list_items(Page::Status, &inv, &arsenal, &none).is_empty() && list_items(Page::Map, &inv, &arsenal, &none).is_empty());
    }

    #[test]
    fn bars_fill_in_proportion_and_never_overflow() {
        assert_eq!(bar(0.0, 10), "[----------]");
        assert_eq!(bar(1.0, 10), "[##########]");
        assert_eq!(bar(0.4, 10), "[####------]");
        assert_eq!(bar(7.0, 10), "[##########]", "over-full is clamped");
        assert_eq!(bar(-3.0, 10), "[----------]");
        assert_eq!(bar(0.5, 0), "[]");
        for v in [0.0, 0.05, 0.5, 0.95, 1.0] {
            assert_eq!(bar(v, 10), old::bar(v, 10));
        }
    }

    #[test]
    fn the_status_page_names_your_troubles() {
        let mut s = Survival::new();
        let c = Condition::of(&s);
        assert_eq!(status_effects(&s, &c, false, false), vec!["None. Feeling Minnesota fine."]);
        s.body_heat = 30.0;
        s.frostbite = true;
        s.rads = 150.0;
        s.health = 10.0;
        let c = Condition::of(&s);
        assert!(c.cold && c.hurt && c.irradiated);
        let list = status_effects(&s, &c, true, true);
        assert_eq!(list.len(), 6);
        assert!(list[0].starts_with("COLD") && list[5].starts_with("SHELTERED"));

        let text = status_text(&s, &list, Phase::Warning, 41.6, 12.0, -8.4);
        assert!(text.contains("SIREN: blizzard in 42s") && text.contains("Air 12F, feels -8F"), "{text}");
        assert!(text.contains("Max HP -15"), "150 rads cost 15 max HP: {text}");
        assert!(status_text(&Survival::new(), &["x"], Phase::Calm, 5.0, 0.0, 0.0).contains("Max HP unaffected"));
        assert!(status_text(&s, &list, Phase::Blizzard, -3.0, 0.0, 0.0).contains("RAD-BLIZZARD 0s"), "never a negative countdown");
        let side = status_side(&s, 4, 2, 9);
        assert!(side.contains("KILLS 4") && side.contains("PELTS 2") && side.contains("SCRAP 9"));
    }

    #[test]
    fn the_header_shows_level_vitals_and_the_date() {
        let s = Survival::new();
        let clock = Clock { hours: 7.5, day: 3 };
        // The Pip-Boy's old inline format string.
        let (lvl, into, span) = (2, 40, 200);
        let old = format!(
            "LVL {}   HP {:.0}/{:.0}   HEAT {:.0}%   RAD {:.0}   XP {}/{}   DAY {} {}",
            lvl,
            s.health,
            s.max_health(),
            s.body_heat,
            s.rads,
            into,
            span,
            clock.day,
            clock.label()
        );
        assert_eq!(header(&s, (lvl, into, span), &clock), old);
        assert!(old.contains("DAY 3 07:30"));
    }
}
