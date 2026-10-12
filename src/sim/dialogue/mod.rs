//! Conversations: the Overseer's recording and the four fish-house survivors.
//! A conversation is a small graph of [`Node`]s (what's said, and the replies
//! on offer). Picking a reply can set story flags, hand you something once,
//! and lead to another node or end the talk. The words are all here, the
//! screen that shows them is in `quest.rs`.
//!
//! After the first quest the Overseer offers a second ("The Last Pump"), and
//! the survivors each have a word about it.

use super::quest::{self, Flags, Objective, PumpStage, ENDING_COVER, ENDING_TRUTH, PUMP_REPORTED, PUMP_STARTED, REPORTED, STARTED};

mod script;

use self::script::NODES;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Npc {
    Overseer,
    Lundgren,
    Sven,
    Olson,
    Ole,
}

impl Npc {
    pub const ALL: [Npc; 5] = [Npc::Overseer, Npc::Lundgren, Npc::Sven, Npc::Olson, Npc::Ole];

    /// The survivor who lives in fish house `i`.
    pub fn for_house(i: u8) -> Npc {
        [Npc::Lundgren, Npc::Sven, Npc::Olson, Npc::Ole][i as usize % 4]
    }

    pub fn name(self) -> &'static str {
        match self {
            Npc::Overseer => "the Overseer's terminal",
            Npc::Lundgren => "Lundgren",
            Npc::Sven => "Sven",
            Npc::Olson => "Olson",
            Npc::Ole => "Ole",
        }
    }
}

/// Something a survivor hands over, once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gift {
    Hotdish(u32),
    Scrap(u32),
    Stimpak(u32),
}

impl Gift {
    pub fn describe(self) -> String {
        match self {
            Gift::Hotdish(n) => format!("+{n} Hotdish"),
            Gift::Scrap(n) => format!("+{n} Scrap"),
            Gift::Stimpak(n) => format!("+{n} Stimpak"),
        }
    }
}

/// When a reply is on offer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needs {
    Always,
    /// Only if this flag is set.
    Flag(&'static str),
    /// Only if it isn't (so a gift can be taken once).
    NoFlag(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub text: &'static str,
    /// The node it leads to; `None` ends the conversation.
    pub next: Option<&'static str>,
    /// Flags set when picked.
    pub set: &'static [&'static str],
    pub needs: Needs,
    pub gift: Option<Gift>,
}

#[derive(Debug)]
pub struct Node {
    pub id: &'static str,
    pub speaker: &'static str,
    pub text: &'static str,
    pub choices: &'static [Choice],
    /// Show the quest log under the text (the Overseer's progress check).
    pub with_log: bool,
}

const fn say(text: &'static str, next: Option<&'static str>) -> Choice {
    Choice { text, next, set: &[], needs: Needs::Always, gift: None }
}

const BYE: Choice = say("Goodbye.", None);

pub fn node(id: &str) -> Option<&'static Node> {
    NODES.iter().find(|n| n.id == id)
}

/// Where a conversation with `npc` starts, given what has happened.
pub fn entry(npc: Npc, flags: &Flags) -> &'static str {
    use quest::{done, has, Stage};
    match npc {
        Npc::Overseer => match quest::stage(flags) {
            Stage::Unstarted => "ov.intro",
            Stage::Active => "ov.wait",
            Stage::ReadyToReport => "ov.report",
            Stage::Complete => match quest::pump_stage(flags) {
                PumpStage::Active => "ov.pump.wait",
                PumpStage::ReadyToInstall => "ov.pump.install",
                PumpStage::ReadyToReport => "ov.pump.report",
                PumpStage::Complete if has(flags, ENDING_TRUTH) => "ov.pump.after.truth",
                PumpStage::Complete => "ov.pump.after.cover",
                _ if has(flags, ENDING_TRUTH) => "ov.after.truth",
                _ => "ov.after.cover",
            },
        },
        Npc::Lundgren if done(flags, Objective::Convoy) => "lundgren.after",
        Npc::Lundgren => "lundgren.hello",
        Npc::Sven if done(flags, Objective::Moose) => "sven.after",
        Npc::Sven => "sven.hello",
        Npc::Olson if done(flags, Objective::Power) => "olson.after",
        Npc::Olson => "olson.hello",
        Npc::Ole if quest::pump_stage(flags) == PumpStage::Complete && has(flags, ENDING_TRUTH) => "ole.pump.truth",
        Npc::Ole if quest::pump_stage(flags) == PumpStage::Complete => "ole.pump.cover",
        Npc::Ole if has(flags, REPORTED) && has(flags, ENDING_TRUTH) => "ole.truth",
        Npc::Ole if has(flags, REPORTED) => "ole.cover",
        Npc::Ole => "ole.hello",
    }
}

/// The replies on offer in a node right now; a node with none still lets you leave.
pub fn choices(node: &'static Node, flags: &Flags) -> Vec<&'static Choice> {
    let v: Vec<&Choice> = node
        .choices
        .iter()
        .filter(|c| match c.needs {
            Needs::Always => true,
            Needs::Flag(f) => flags.contains(f),
            Needs::NoFlag(f) => !flags.contains(f),
        })
        .collect();
    if v.is_empty() {
        vec![&BYE]
    } else {
        v
    }
}

/// What picking a reply did.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Step {
    pub gift: Option<Gift>,
    /// The talk is over.
    pub ended: bool,
}

/// A conversation in progress.
#[derive(Clone, Debug)]
pub struct Session {
    node: &'static Node,
    pub selected: usize,
}

impl Session {
    pub fn new(npc: Npc, flags: &Flags) -> Session {
        let node = node(entry(npc, flags)).expect("every entry point is a real node");
        Session { node, selected: 0 }
    }

    pub fn node(&self) -> &'static Node {
        self.node
    }

    pub fn choices(&self, flags: &Flags) -> Vec<&'static Choice> {
        choices(self.node, flags)
    }

    /// Move the highlight, wrapping round.
    pub fn move_by(&mut self, delta: i32, flags: &Flags) {
        let n = self.choices(flags).len() as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(n) as usize;
    }

    /// Take reply `index` (None if there is no such reply).
    pub fn pick(&mut self, index: usize, flags: &mut Flags) -> Option<Step> {
        let choice = *self.choices(flags).get(index)?;
        for f in choice.set {
            quest::set(flags, f);
        }
        let step = Step { gift: choice.gift, ended: choice.next.is_none() };
        if let Some(next) = choice.next {
            self.node = node(next).expect("choices lead to real nodes");
            self.selected = 0;
        }
        Some(step)
    }
}
