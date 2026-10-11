//! Quest 2, The Last Pump.

use super::*;

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
