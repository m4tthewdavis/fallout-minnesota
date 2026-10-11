//! "Why Did the Overseer Open the Door?": the first quest line, and the
//! perks you pick as you level. Everything is kept as named flags in one
//! set (saved with the game), so nothing here needs its own save format.
//!
//! The three deeds (the convoy, the Mills' power, the Glowmoose) are flagged
//! when you do them, whether or not you've been given the job yet: a gun-toting
//! wanderer who has already cleared the moose isn't made to do it twice.

use std::collections::BTreeSet;

mod pump;
mod spots;
mod perks;

pub use self::pump::*;
pub use self::spots::*;
pub use self::perks::*;

#[cfg(test)]
mod tests;

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
