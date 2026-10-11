//! Places you use things, like the pump parts and the breaker.

use super::*;

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
