//! "Why Did the Overseer Open the Door?": the first quest line, and the
//! perks you pick as you level. Everything is kept as named flags in one
//! set (saved with the game), so nothing here needs its own save format.
//!
//! The three deeds (the convoy, the Mills' power, the Glowmoose) are flagged
//! when you do them, whether or not you've been given the job yet: a gun-toting
//! wanderer who has already cleared the moose isn't made to do it twice.

use std::collections::BTreeSet;

pub type Flags = BTreeSet<String>;

pub const TITLE: &str = "Why Did the Overseer Open the Door?";

pub const STARTED: &str = "quest.started";
pub const REPORTED: &str = "quest.reported";
/// How you told the vault what you found.
pub const ENDING_TRUTH: &str = "ending.truth";
pub const ENDING_COVER: &str = "ending.cover";

pub fn has(flags: &Flags, flag: &str) -> bool {
    flags.contains(flag)
}

pub fn set(flags: &mut Flags, flag: &str) -> bool {
    flags.insert(flag.to_string())
}

/// The three things the Overseer wants done, in any order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Objective {
    Convoy,
    Power,
    Moose,
}

impl Objective {
    pub const ALL: [Objective; 3] = [Objective::Convoy, Objective::Power, Objective::Moose];

    pub fn flag(self) -> &'static str {
        match self {
            Objective::Convoy => "found.convoy",
            Objective::Power => "fixed.power",
            Objective::Moose => "killed.glowmoose",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Objective::Convoy => "Investigate the missing supply convoy on the highway",
            Objective::Power => "Restore power at Golden Atomic Mills",
            Objective::Moose => "Defeat the Glowmoose threatening Sven's Shanty",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Objective::Convoy => "The old US-169 runs east-west across the south of the map. Lundgren saw the trucks go by.",
            Objective::Power => "The Mills are in the north-east. The breaker panel is by the silos; Olson says three scrap of wire will do.",
            Objective::Moose => "Sven's Shanty is the fish house in the west. The moose grazes to the east of it. Mind the charge.",
        }
    }

    /// XP for doing it.
    pub fn xp(self) -> u32 {
        match self {
            Objective::Convoy => 80,
            Objective::Power => 100,
            Objective::Moose => 150,
        }
    }
}

pub fn done(flags: &Flags, o: Objective) -> bool {
    has(flags, o.flag())
}

pub fn objectives_done(flags: &Flags) -> usize {
    Objective::ALL.iter().filter(|o| done(flags, **o)).count()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// You haven't heard the Overseer's recording yet.
    Unstarted,
    /// Out doing the three deeds.
    Active,
    /// All three done: go back to the vault.
    ReadyToReport,
    Complete,
}

pub fn stage(flags: &Flags) -> Stage {
    if has(flags, REPORTED) {
        Stage::Complete
    } else if !has(flags, STARTED) {
        Stage::Unstarted
    } else if objectives_done(flags) == Objective::ALL.len() {
        Stage::ReadyToReport
    } else {
        Stage::Active
    }
}

/// XP from the quest so far: each deed as you do it, and a bonus for reporting.
pub const REPORT_XP: u32 = 250;

pub fn quest_xp(flags: &Flags) -> u32 {
    let deeds: u32 = Objective::ALL.iter().filter(|o| done(flags, **o)).map(|o| o.xp()).sum();
    deeds + if has(flags, REPORTED) { REPORT_XP } else { 0 }
}

/// A line of the quest log: the text and whether it's ticked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LogLine {
    pub text: String,
    pub done: bool,
}

/// The NOTES page entry for the quest, top to bottom.
pub fn log(flags: &Flags) -> Vec<LogLine> {
    let line = |text: &str, done: bool| LogLine { text: text.to_string(), done };
    let mut v = Vec::new();
    match stage(flags) {
        Stage::Unstarted => v.push(line("Find out why the Overseer opened the door: use the terminal in the Vault 143 lobby", false)),
        stage => {
            v.push(line("Hear the Overseer's recording at the terminal in Vault 143", true));
            for o in Objective::ALL {
                v.push(line(o.title(), done(flags, o)));
            }
            v.push(line("Report back at the Vault 143 lobby", stage == Stage::Complete));
        }
    }
    v
}

/// The hint for what to do next, if there's something to do.
pub fn next_hint(flags: &Flags) -> Option<String> {
    match stage(flags) {
        Stage::Unstarted => Some("The Overseer left a recording on the terminal in the Vault 143 lobby.".to_string()),
        Stage::Active => Objective::ALL.iter().find(|o| !done(flags, **o)).map(|o| o.hint().to_string()),
        Stage::ReadyToReport => Some("Return to the Overseer's terminal in the Vault 143 lobby.".to_string()),
        Stage::Complete => None,
    }
}

// ---------------------------------------------------------------------------
// Places you use things
// ---------------------------------------------------------------------------

/// Out-of-the-way things in the world that the quest asks you to use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spot {
    /// The wrecked Vault-Tec convoy on the highway.
    Convoy,
    /// The breaker panel at Golden Atomic Mills.
    Breaker,
}

/// Scrap it costs to rewire the breaker panel.
pub const BREAKER_SCRAP: u32 = 3;

impl Spot {
    pub fn objective(self) -> Objective {
        match self {
            Spot::Convoy => Objective::Convoy,
            Spot::Breaker => Objective::Power,
        }
    }

    /// What E offers here right now.
    pub fn prompt(self, flags: &Flags, scrap: u32) -> String {
        let done = done(flags, self.objective());
        match (self, done) {
            (Spot::Convoy, false) => "[E] Search the wrecked convoy".to_string(),
            (Spot::Convoy, true) => "The convoy has been searched".to_string(),
            (Spot::Breaker, false) => format!("[E] Rewire the breaker panel ({BREAKER_SCRAP} scrap, you have {scrap})"),
            (Spot::Breaker, true) => "The breaker panel hums. The power is on".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Perks
// ---------------------------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(names: &[&str]) -> Flags {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_quest_moves_through_its_stages() {
        let mut f = Flags::new();
        assert_eq!(stage(&f), Stage::Unstarted);
        set(&mut f, STARTED);
        assert_eq!(stage(&f), Stage::Active);
        for o in Objective::ALL {
            assert_eq!(stage(&f), Stage::Active, "until the last is done");
            set(&mut f, o.flag());
        }
        assert_eq!(stage(&f), Stage::ReadyToReport);
        set(&mut f, REPORTED);
        assert_eq!(stage(&f), Stage::Complete);
    }

    #[test]
    fn deeds_done_early_still_count() {
        let mut f = flags(&[Objective::Moose.flag(), Objective::Convoy.flag()]);
        assert_eq!(stage(&f), Stage::Unstarted, "but the quest isn't started by them");
        assert_eq!(objectives_done(&f), 2);
        assert_eq!(quest_xp(&f), Objective::Moose.xp() + Objective::Convoy.xp());
        set(&mut f, STARTED);
        set(&mut f, Objective::Power.flag());
        assert_eq!(stage(&f), Stage::ReadyToReport, "starting late loses nothing");
    }

    #[test]
    fn reporting_is_the_biggest_reward() {
        let all = flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose"]);
        let mut finished = all.clone();
        set(&mut finished, REPORTED);
        assert_eq!(quest_xp(&finished), quest_xp(&all) + REPORT_XP);
        assert!(REPORT_XP > Objective::ALL.iter().map(|o| o.xp()).max().unwrap());
    }

    #[test]
    fn the_log_ticks_things_off_and_points_to_what_is_next() {
        let f = flags(&[STARTED, "fixed.power"]);
        let log = log(&f);
        assert_eq!(log.len(), 5, "heard it, three deeds, report back");
        assert!(log[0].done && log[2].done);
        assert!(!log[1].done && !log[3].done && !log[4].done);
        assert!(next_hint(&f).unwrap().contains("US-169"), "first one left is the convoy");
        assert_eq!(next_hint(&flags(&[STARTED, REPORTED])), None);
        assert_eq!(super::log(&Flags::new()).len(), 1);
        assert!(next_hint(&Flags::new()).unwrap().contains("terminal"));
    }

    #[test]
    fn spots_say_what_they_do_and_when_they_are_spent() {
        let none = Flags::new();
        assert!(Spot::Breaker.prompt(&none, 1).contains("3 scrap, you have 1"));
        assert!(Spot::Convoy.prompt(&none, 0).starts_with("[E]"));
        let after = flags(&[Objective::Convoy.flag(), Objective::Power.flag()]);
        assert!(!Spot::Convoy.prompt(&after, 0).starts_with("[E]"));
        assert!(!Spot::Breaker.prompt(&after, 9).starts_with("[E]"));
    }

    #[test]
    fn perk_picks_are_earned_one_per_level_and_used_up() {
        let mut f = Flags::new();
        assert_eq!(picks_available(1, &f), 0);
        assert_eq!(picks_available(2, &f), 1);
        assert_eq!(picks_available(4, &f), 3);
        set(&mut f, &Perk::QuickHands.flag());
        assert_eq!(picks_available(4, &f), 2);
        assert_eq!(picks_available(1, &f), 0, "a level 1 save with a perk doesn't underflow");
        for p in Perk::ALL {
            set(&mut f, &p.flag());
        }
        assert_eq!(picks_available(99, &f), 0, "nothing left to pick");
    }

    #[test]
    fn an_offer_is_up_to_three_untaken_perks_that_change_as_you_pick() {
        let mut f = Flags::new();
        let first = offer(&f);
        assert_eq!(first.len(), 3);
        let mut seen = first.clone();
        seen.dedup();
        assert_eq!(seen.len(), 3, "no repeats");
        set(&mut f, &first[0].flag());
        let second = offer(&f);
        assert_eq!(second.len(), 3);
        assert!(!second.contains(&first[0]), "you can't be offered what you have");
        assert_ne!(first, second);
        for p in Perk::ALL {
            set(&mut f, &p.flag());
        }
        assert!(offer(&f).is_empty());
    }

    #[test]
    fn perks_change_the_numbers_they_say_they_do() {
        assert_eq!(effects(&Flags::new()), Effects::default());
        let e = effects(&flags(&[&Perk::FrostHardy.flag(), &Perk::QuickHands.flag(), &Perk::Scrounger.flag()]));
        assert!(e.insulation > 0.0 && e.reload_time < 1.0 && e.extra_scrap == 1);
        assert_eq!(e.rad_resist, 0.0);
        assert!(effects(&flags(&[&Perk::FieldMedic.flag()])).aid_heal > 1.0);
        assert!(effects(&flags(&[&Perk::RadResistant.flag()])).rad_resist > 0.0);
    }

    #[test]
    fn every_perk_has_a_unique_id_and_words() {
        let mut ids: Vec<&str> = Perk::ALL.iter().map(|p| p.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Perk::ALL.len());
        for p in Perk::ALL {
            assert!(!p.name().is_empty() && !p.describe().is_empty());
        }
    }
}
