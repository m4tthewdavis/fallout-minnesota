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
/// The follow-up quest, offered once the first is complete.
pub const TITLE2: &str = "The Last Pump";

pub const STARTED: &str = "quest.started";
pub const REPORTED: &str = "quest.reported";
/// How you told the vault what you found.
pub const ENDING_TRUTH: &str = "ending.truth";
pub const ENDING_COVER: &str = "ending.cover";

/// Quest 2: agreed to build the reactor's replacement pump.
pub const PUMP_STARTED: &str = "pump.started";
/// Quest 2: the pump is installed at the reactor.
pub const PUMP_FIXED: &str = "fixed.pump";
/// Quest 2: told the Overseer it runs.
pub const PUMP_REPORTED: &str = "pump.reported";

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

/// XP for fitting the new pump, and for telling the Overseer it runs.
pub const INSTALL_XP: u32 = 200;
pub const PUMP_REPORT_XP: u32 = 300;

/// XP from both quests so far.
pub fn quest_xp(flags: &Flags) -> u32 {
    let deeds: u32 = Objective::ALL.iter().filter(|o| done(flags, **o)).map(|o| o.xp()).sum();
    let parts: u32 = Part::ALL.iter().filter(|p| pump_done(flags, **p)).map(|p| p.xp()).sum();
    deeds
        + if has(flags, REPORTED) { REPORT_XP } else { 0 }
        + parts
        + if has(flags, PUMP_FIXED) { INSTALL_XP } else { 0 }
        + if has(flags, PUMP_REPORTED) { PUMP_REPORT_XP } else { 0 }
}

// ---------------------------------------------------------------------------
// Quest 2: The Last Pump
// ---------------------------------------------------------------------------

/// The three parts of the replacement coolant pump, gathered in any order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Impeller,
    Coupling,
    Seals,
}

/// Pelts it takes to cut the pump's seals.
pub const SEAL_PELTS: u32 = 3;

impl Part {
    pub const ALL: [Part; 3] = [Part::Impeller, Part::Coupling, Part::Seals];

    pub fn flag(self) -> &'static str {
        match self {
            Part::Impeller => "found.impeller",
            Part::Coupling => "found.coupling",
            Part::Seals => "made.seals",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Part::Impeller => "Find a pump impeller in the Bullseye-Mart stockroom",
            Part::Coupling => "Take a power coupling from the transformer at Golden Atomic Mills",
            Part::Seals => "Cut pump seals from Frostfang hide at the reactor room workbench",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Part::Impeller => "Lundgren says there is a crate in the back stockroom of the Bullseye-Mart.",
            Part::Coupling => "The transformer at Golden Atomic Mills only gives up its coupling once the power is on. Olson knows.",
            Part::Seals => "Bring three Frostfang pelts to the workbench in the Vault 143 reactor room.",
        }
    }

    /// XP for getting it.
    pub fn xp(self) -> u32 {
        match self {
            Part::Impeller => 60,
            Part::Coupling => 80,
            Part::Seals => 60,
        }
    }
}

pub fn pump_done(flags: &Flags, p: Part) -> bool {
    has(flags, p.flag())
}

pub fn parts_done(flags: &Flags) -> usize {
    Part::ALL.iter().filter(|p| pump_done(flags, **p)).count()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PumpStage {
    /// The first quest isn't finished.
    Locked,
    /// The Overseer will give you the job if you ask.
    Offered,
    /// Out finding the three parts.
    Active,
    /// Parts in hand: fit the pump at the reactor.
    ReadyToInstall,
    /// It runs: tell the Overseer.
    ReadyToReport,
    Complete,
}

pub fn pump_stage(flags: &Flags) -> PumpStage {
    if stage(flags) != Stage::Complete {
        PumpStage::Locked
    } else if has(flags, PUMP_REPORTED) {
        PumpStage::Complete
    } else if !has(flags, PUMP_STARTED) {
        PumpStage::Offered
    } else if parts_done(flags) < Part::ALL.len() {
        PumpStage::Active
    } else if !has(flags, PUMP_FIXED) {
        PumpStage::ReadyToInstall
    } else {
        PumpStage::ReadyToReport
    }
}

/// A line of the quest log: the text and whether it's ticked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LogLine {
    pub text: String,
    pub done: bool,
}

/// The NOTES page entry for the quests, top to bottom: the first quest's
/// lines, then (once it is complete) the second's.
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
    match pump_stage(flags) {
        PumpStage::Locked => {}
        PumpStage::Offered => v.push(line(&format!("{TITLE2}: ask the Overseer if there is anything else he needs"), false)),
        ps => {
            v.push(line(&format!("{TITLE2}: agree to build the reactor's replacement pump"), true));
            for p in Part::ALL {
                v.push(line(p.title(), pump_done(flags, p)));
            }
            v.push(line("Install the pump at the reactor in Vault 143", has(flags, PUMP_FIXED)));
            v.push(line("Tell the Overseer the pump runs", ps == PumpStage::Complete));
        }
    }
    v
}

/// The hint for what to do next, if there's something to do. Quest 1 comes
/// first; quest 2's hints start once it is complete.
pub fn next_hint(flags: &Flags) -> Option<String> {
    match stage(flags) {
        Stage::Unstarted => return Some("The Overseer left a recording on the terminal in the Vault 143 lobby.".to_string()),
        Stage::Active => return Objective::ALL.iter().find(|o| !done(flags, **o)).map(|o| o.hint().to_string()),
        Stage::ReadyToReport => return Some("Return to the Overseer's terminal in the Vault 143 lobby.".to_string()),
        Stage::Complete => {}
    }
    match pump_stage(flags) {
        PumpStage::Locked | PumpStage::Complete => None,
        PumpStage::Offered => Some("The Overseer may have more work. Ask him at the terminal if there is anything else.".to_string()),
        PumpStage::Active => Part::ALL.iter().find(|p| !pump_done(flags, **p)).map(|p| p.hint().to_string()),
        PumpStage::ReadyToInstall => Some("You have every part. Fit the pump at the reactor in Vault 143.".to_string()),
        PumpStage::ReadyToReport => Some("The pump runs. Tell the Overseer at his terminal in the Vault 143 lobby.".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Places you use things
// ---------------------------------------------------------------------------

/// Out-of-the-way things in the world that the quests ask you to use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spot {
    /// The wrecked Vault-Tec convoy on the highway.
    Convoy,
    /// The breaker panel at Golden Atomic Mills.
    Breaker,
    /// The crate in the Bullseye-Mart stockroom.
    Impeller,
    /// The transformer at Golden Atomic Mills.
    Coupling,
    /// The workbench in the Vault 143 reactor room.
    Seals,
    /// The pump housing in the Vault 143 reactor room.
    Pump,
}

/// Scrap it costs to rewire the breaker panel.
pub const BREAKER_SCRAP: u32 = 3;

const NOT_YET: &str = "You have no use for that yet.";

impl Spot {
    /// The first quest's objective this spot completes, if it is one of those.
    pub fn objective(self) -> Option<Objective> {
        match self {
            Spot::Convoy => Some(Objective::Convoy),
            Spot::Breaker => Some(Objective::Power),
            _ => None,
        }
    }

    /// The second quest's part this spot yields, if it is one of those.
    pub fn part(self) -> Option<Part> {
        match self {
            Spot::Impeller => Some(Part::Impeller),
            Spot::Coupling => Some(Part::Coupling),
            Spot::Seals => Some(Part::Seals),
            _ => None,
        }
    }

    /// The flag using this spot sets.
    pub fn flag(self) -> &'static str {
        match (self.objective(), self.part()) {
            (Some(o), _) => o.flag(),
            (_, Some(p)) => p.flag(),
            _ => PUMP_FIXED,
        }
    }

    /// XP for using it (for the first time).
    pub fn xp(self) -> u32 {
        match (self.objective(), self.part()) {
            (Some(o), _) => o.xp(),
            (_, Some(p)) => p.xp(),
            _ => INSTALL_XP,
        }
    }

    pub fn done(self, flags: &Flags) -> bool {
        has(flags, self.flag())
    }

    fn done_message(self) -> &'static str {
        match self {
            Spot::Convoy => "The convoy has been searched",
            Spot::Breaker => "The breaker panel hums. The power is on",
            Spot::Impeller => "You already have the pump impeller",
            Spot::Coupling => "You already pulled the power coupling",
            Spot::Seals => "The pump seals are cut and ready",
            Spot::Pump => "The new coolant pump is running",
        }
    }

    /// Whether using it does anything right now; if not, why not. Scrap for
    /// the breaker panel is the caller's check, as it always was.
    pub fn usable(self, flags: &Flags, pelts: u32) -> Result<(), &'static str> {
        if self.objective().is_some() {
            return if self.done(flags) { Err(self.done_message()) } else { Ok(()) };
        }
        if matches!(pump_stage(flags), PumpStage::Locked | PumpStage::Offered) {
            return Err(NOT_YET);
        }
        if self.done(flags) {
            return Err(self.done_message());
        }
        match self {
            Spot::Coupling if !has(flags, Objective::Power.flag()) => Err("The transformer is dead. Get the Mills' power back on first."),
            Spot::Seals if pelts < SEAL_PELTS => Err("You need 3 Frostfang pelts to cut the seals."),
            Spot::Pump => match (pump_done(flags, Part::Impeller), pump_done(flags, Part::Coupling), pump_done(flags, Part::Seals)) {
                (true, true, true) => Ok(()),
                (false, false, false) => Err("The pump needs an impeller, a power coupling and seals. You have none of them."),
                (false, false, true) => Err("The pump still needs an impeller and a power coupling."),
                (false, true, false) => Err("The pump still needs an impeller and seals."),
                (true, false, false) => Err("The pump still needs a power coupling and seals."),
                (false, true, true) => Err("The pump still needs an impeller."),
                (true, false, true) => Err("The pump still needs a power coupling."),
                (true, true, false) => Err("The pump still needs seals."),
            },
            _ => Ok(()),
        }
    }

    /// What E offers here right now.
    pub fn prompt(self, flags: &Flags, scrap: u32, pelts: u32) -> String {
        // The pump's parts say nothing until you know what they're for.
        if (self.part().is_some() || self == Spot::Pump) && matches!(pump_stage(flags), PumpStage::Locked | PumpStage::Offered) {
            return String::new();
        }
        let seals_short = self == Spot::Seals && !self.done(flags) && !matches!(pump_stage(flags), PumpStage::Locked | PumpStage::Offered);
        match self.usable(flags, pelts) {
            Err(why) if !seals_short => why.to_string(),
            _ => match self {
                Spot::Convoy => "[E] Search the wrecked convoy".to_string(),
                Spot::Breaker => format!("[E] Rewire the breaker panel ({BREAKER_SCRAP} scrap, you have {scrap})"),
                Spot::Impeller => "[E] Open the crate".to_string(),
                Spot::Coupling => "[E] Pull the power coupling from the transformer".to_string(),
                Spot::Seals => format!("[E] Cut seals from Frostfang hide ({SEAL_PELTS} pelts, you have {pelts})"),
                Spot::Pump => "[E] Install the new coolant pump".to_string(),
            },
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
        // Quest 1 complete now leads on to quest 2's offer instead of ending the hints.
        assert!(next_hint(&flags(&[STARTED, REPORTED])).unwrap().contains("anything else"));
        assert_eq!(next_hint(&flags(&[STARTED, REPORTED, PUMP_STARTED, PUMP_REPORTED, "found.impeller", "found.coupling", "made.seals", PUMP_FIXED])), None);
        assert_eq!(super::log(&Flags::new()).len(), 1);
        assert!(next_hint(&Flags::new()).unwrap().contains("terminal"));
    }

    #[test]
    fn spots_say_what_they_do_and_when_they_are_spent() {
        let none = Flags::new();
        assert!(Spot::Breaker.prompt(&none, 1, 0).contains("3 scrap, you have 1"));
        assert!(Spot::Convoy.prompt(&none, 0, 0).starts_with("[E]"));
        let after = flags(&[Objective::Convoy.flag(), Objective::Power.flag()]);
        assert!(!Spot::Convoy.prompt(&after, 0, 0).starts_with("[E]"));
        assert!(!Spot::Breaker.prompt(&after, 9, 0).starts_with("[E]"));
    }

    // ---- Quest 2: The Last Pump ----

    const Q1: [&str; 6] = [STARTED, REPORTED, "found.convoy", "fixed.power", "killed.glowmoose", ENDING_TRUTH];

    fn q2(extra: &[&str]) -> Flags {
        let mut f = flags(&Q1);
        for e in extra {
            set(&mut f, e);
        }
        f
    }

    #[test]
    fn the_pump_job_only_opens_when_the_first_quest_is_done() {
        assert_eq!(pump_stage(&Flags::new()), PumpStage::Locked);
        assert_eq!(pump_stage(&flags(&[STARTED, "found.convoy"])), PumpStage::Locked);
        assert_eq!(pump_stage(&flags(&[PUMP_STARTED, "found.impeller"])), PumpStage::Locked, "no skipping ahead");
        assert_eq!(pump_stage(&q2(&[])), PumpStage::Offered);
    }

    #[test]
    fn the_pump_moves_through_its_stages() {
        let mut f = q2(&[]);
        set(&mut f, PUMP_STARTED);
        for p in Part::ALL {
            assert_eq!(pump_stage(&f), PumpStage::Active, "until the last part");
            set(&mut f, p.flag());
        }
        assert_eq!(parts_done(&f), 3);
        assert_eq!(pump_stage(&f), PumpStage::ReadyToInstall);
        set(&mut f, PUMP_FIXED);
        assert_eq!(pump_stage(&f), PumpStage::ReadyToReport);
        set(&mut f, PUMP_REPORTED);
        assert_eq!(pump_stage(&f), PumpStage::Complete);
    }

    #[test]
    fn parts_found_before_agreeing_still_count() {
        let mut f = q2(&["found.impeller", "found.coupling", "made.seals"]);
        assert_eq!(pump_stage(&f), PumpStage::Offered);
        set(&mut f, PUMP_STARTED);
        assert_eq!(pump_stage(&f), PumpStage::ReadyToInstall, "agreeing late loses nothing");
    }

    #[test]
    fn quest_two_pays_for_parts_install_and_report() {
        let base = quest_xp(&q2(&[]));
        assert_eq!(base, quest_xp(&flags(&Q1)));
        assert_eq!(quest_xp(&q2(&["found.impeller"])), base + 60);
        assert_eq!(quest_xp(&q2(&["found.coupling"])), base + 80);
        assert_eq!(quest_xp(&q2(&["made.seals"])), base + 60);
        let parts = Part::ALL.iter().map(|p| p.xp()).sum::<u32>();
        assert_eq!(parts, 200);
        let all = q2(&["found.impeller", "found.coupling", "made.seals", PUMP_FIXED]);
        assert_eq!(quest_xp(&all), base + parts + 200);
        let mut done = all.clone();
        set(&mut done, PUMP_REPORTED);
        assert_eq!(quest_xp(&done), base + parts + 200 + 300);
        assert_eq!(quest_xp(&done) - quest_xp(&flags(&Q1)), 700);
    }

    #[test]
    fn the_log_and_hints_carry_on_into_quest_two() {
        assert_eq!(log(&flags(&[STARTED])).len(), 5, "quest 2 hidden until quest 1 is done");
        let offered = log(&q2(&[]));
        assert_eq!(offered.len(), 6);
        assert!(offered[..5].iter().all(|l| l.done), "quest 1 lines come first");
        assert!(offered[5].text.contains(TITLE2) && !offered[5].done);
        let mut f = q2(&[PUMP_STARTED, "found.coupling"]);
        let l = log(&f);
        assert_eq!(l.len(), 5 + 1 + 3 + 2);
        assert!(l[5].done && l[7].done && !l[6].done && !l[8].done && !l[9].done && !l[10].done);
        assert!(next_hint(&f).unwrap().contains("Bullseye-Mart"), "first part left is the impeller");
        set(&mut f, "found.impeller");
        assert!(next_hint(&f).unwrap().contains("Frostfang"));
        set(&mut f, "made.seals");
        assert!(next_hint(&f).unwrap().contains("reactor"));
        set(&mut f, PUMP_FIXED);
        assert!(next_hint(&f).unwrap().contains("Tell the Overseer"));
        set(&mut f, PUMP_REPORTED);
        assert!(log(&f).iter().all(|l| l.done));
        assert_eq!(next_hint(&f), None);
    }

    #[test]
    fn the_new_spots_refuse_until_you_can_use_them() {
        let early = q2(&[]);
        for s in [Spot::Impeller, Spot::Coupling, Spot::Seals, Spot::Pump] {
            assert_eq!(s.usable(&early, 99), Err("You have no use for that yet."), "{s:?} before agreeing");
            assert_eq!(s.prompt(&early, 9, 9), "", "{s:?} shows nothing until you know what it is for");
            assert!(!s.prompt(&early, 9, 9).starts_with("[E]"));
        }
        let locked = flags(&[PUMP_STARTED]);
        assert_eq!(Spot::Impeller.usable(&locked, 9), Err("You have no use for that yet."));
        let go = q2(&[PUMP_STARTED]);
        assert_eq!(Spot::Impeller.usable(&go, 0), Ok(()));
        assert!(Spot::Impeller.prompt(&go, 0, 0).starts_with("[E]"));
    }

    #[test]
    fn the_coupling_needs_the_mills_power() {
        let mut f = q2(&[PUMP_STARTED]);
        f.remove("fixed.power");
        assert!(Spot::Coupling.usable(&f, 0).unwrap_err().contains("Mills' power"));
        assert!(!Spot::Coupling.prompt(&f, 0, 0).starts_with("[E]"));
        set(&mut f, "fixed.power");
        assert_eq!(Spot::Coupling.usable(&f, 0), Ok(()));
        assert!(Spot::Coupling.prompt(&f, 0, 0).starts_with("[E]"));
    }

    #[test]
    fn seals_cost_pelts() {
        let f = q2(&[PUMP_STARTED]);
        assert_eq!(SEAL_PELTS, 3);
        assert!(Spot::Seals.usable(&f, SEAL_PELTS - 1).unwrap_err().contains(&SEAL_PELTS.to_string()));
        assert_eq!(Spot::Seals.usable(&f, SEAL_PELTS), Ok(()));
        assert_eq!(Spot::Seals.usable(&f, 50), Ok(()));
        assert!(Spot::Seals.prompt(&f, 0, 1).contains("3 pelts, you have 1"));
        assert!(Spot::Seals.usable(&f, 0).is_err());
    }

    #[test]
    fn the_pump_needs_all_three_parts_and_says_which_are_missing() {
        let mut f = q2(&[PUMP_STARTED]);
        assert!(Spot::Pump.usable(&f, 0).unwrap_err().contains("none"));
        set(&mut f, "found.impeller");
        let e = Spot::Pump.usable(&f, 0).unwrap_err();
        assert!(e.contains("power coupling") && e.contains("seals") && !e.contains("impeller"));
        set(&mut f, "made.seals");
        let e = Spot::Pump.usable(&f, 0).unwrap_err();
        assert!(e.contains("power coupling") && !e.contains("seals") && !e.contains("impeller"));
        assert!(!Spot::Pump.prompt(&f, 0, 0).starts_with("[E]"));
        set(&mut f, "found.coupling");
        assert_eq!(Spot::Pump.usable(&f, 0), Ok(()));
        assert!(Spot::Pump.prompt(&f, 0, 0).starts_with("[E]"));
        // every combination of two parts gives a message naming exactly the missing one
        for missing in Part::ALL {
            let mut g = q2(&[PUMP_STARTED]);
            for p in Part::ALL.into_iter().filter(|p| *p != missing) {
                set(&mut g, p.flag());
            }
            assert!(Spot::Pump.usable(&g, 0).is_err());
        }
    }

    #[test]
    fn spots_that_are_done_stay_done() {
        let f = q2(&[PUMP_STARTED, "found.impeller", "found.coupling", "made.seals", PUMP_FIXED]);
        for s in [Spot::Impeller, Spot::Coupling, Spot::Seals, Spot::Pump] {
            assert!(s.done(&f));
            assert!(s.usable(&f, 99).is_err());
            assert!(!s.prompt(&f, 99, 99).starts_with("[E]"), "{s:?}");
        }
        assert!(Spot::Convoy.usable(&f, 0).is_err() && Spot::Breaker.usable(&f, 0).is_err());
        assert_eq!(Spot::Convoy.usable(&Flags::new(), 0), Ok(()));
    }

    #[test]
    fn every_spot_sets_the_flag_and_pays_the_xp_it_says() {
        assert_eq!(Spot::Convoy.flag(), Objective::Convoy.flag());
        assert_eq!(Spot::Breaker.xp(), Objective::Power.xp());
        assert_eq!(Spot::Seals.flag(), Part::Seals.flag());
        assert_eq!(Spot::Pump.flag(), PUMP_FIXED);
        assert_eq!(Spot::Pump.xp(), INSTALL_XP);
        assert_eq!(Spot::Convoy.objective(), Some(Objective::Convoy));
        assert_eq!(Spot::Pump.objective(), None);
        assert_eq!(Spot::Coupling.part(), Some(Part::Coupling));
        let mut flags: Vec<&str> = Part::ALL.iter().map(|p| p.flag()).collect();
        flags.extend(Objective::ALL.iter().map(|o| o.flag()));
        flags.extend([PUMP_STARTED, PUMP_FIXED, PUMP_REPORTED, STARTED, REPORTED]);
        let n = flags.len();
        flags.sort_unstable();
        flags.dedup();
        assert_eq!(flags.len(), n, "no flag is reused");
        for p in Part::ALL {
            assert!(!p.title().is_empty() && !p.hint().is_empty());
        }
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
