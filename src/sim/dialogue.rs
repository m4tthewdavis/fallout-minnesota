//! Conversations: the Overseer's recording and the four fish-house survivors.
//! A conversation is a small graph of [`Node`]s (what's said, and the replies
//! on offer). Picking a reply can set story flags, hand you something once,
//! and lead to another node or end the talk. The words are all here, the
//! screen that shows them is in `quest.rs`.

use super::quest::{self, Flags, Objective, ENDING_COVER, ENDING_TRUTH, REPORTED, STARTED};

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

const OVERSEER: &str = "OVERSEER (recording)";

static NODES: &[Node] = &[
    // ---- The Overseer's terminal ----
    Node {
        id: "ov.intro",
        speaker: OVERSEER,
        text: "Overseer's log. If you're hearing this, you walked out of Vault 143, and the door is open because I opened it. I won't say why until you've done three things for me.\n\nA supply convoy was sent up US-169 for us and never arrived. The beacon at Golden Atomic Mills went dark in the first winter, and I want it lit. And a Glowmoose is tearing through the survivors at Sven's Shanty.\n\nDo these, then come back and I'll tell you everything.",
        choices: &[
            Choice { set: &[STARTED], ..say("I'll do it.", None) },
            say("Why should I trust you?", Some("ov.trust")),
            say("Not today.", None),
        ],
        with_log: false,
    },
    Node {
        id: "ov.trust",
        speaker: OVERSEER,
        text: "You shouldn't. Trust the cold; it has never once lied to anyone. But I know what is on the other side of that door, and you are the only one who can reach it.",
        choices: &[Choice { set: &[STARTED], ..say("All right. I'll do it.", None) }, say("Go back.", Some("ov.intro"))],
        with_log: false,
    },
    Node {
        id: "ov.wait",
        speaker: OVERSEER,
        text: "Come back when it's finished. This is what's left:",
        choices: &[say("I'm on it.", None)],
        with_log: true,
    },
    Node {
        id: "ov.report",
        speaker: OVERSEER,
        text: "All three, then. The convoy, the Mills and the moose. Sit down.\n\nThe door didn't open by itself. The reactor's coolant lines froze in the Long Winter and Vault 143 has about two winters left. The convoy carried the replacement pump, and now it's wreckage. I opened the door because someone had to go and look.\n\nWhat do I tell the others?",
        choices: &[
            Choice { set: &[REPORTED, ENDING_TRUTH], ..say("Tell them the truth.", Some("ov.end.truth")) },
            Choice { set: &[REPORTED, ENDING_COVER], ..say("Tell them the door opened on its own. Keep them calm.", Some("ov.end.cover")) },
            say("I need to think about it.", None),
        ],
        with_log: false,
    },
    Node {
        id: "ov.end.truth",
        speaker: OVERSEER,
        text: "Then I will. There will be fear, and there will be work, and frightened people work hard. Thank you, wanderer. Vault 143's door stays open to you.",
        choices: &[say("Stay warm, Overseer.", None)],
        with_log: false,
    },
    Node {
        id: "ov.end.cover",
        speaker: OVERSEER,
        text: "A comfortable lie. I'll give it to them, and I'll keep looking for another pump. Thank you, wanderer. The door stays open to you, and the lie stays between us.",
        choices: &[say("Stay warm, Overseer.", None)],
        with_log: false,
    },
    Node {
        id: "ov.after.truth",
        speaker: OVERSEER,
        text: "The vault is awake and arguing, which is more than it has done in two hundred years. We are building a new pump out of what you brought. Go on, there is more winter out there than there is in here.",
        choices: &[say("Goodbye.", None)],
        with_log: false,
    },
    Node {
        id: "ov.after.cover",
        speaker: OVERSEER,
        text: "The vault is calm. I hear every one of its footsteps and wonder how long calm lasts. Go on, wanderer. There is more winter out there than there is in here.",
        choices: &[say("Goodbye.", None)],
        with_log: false,
    },
    // ---- Lundgren: the convoy ----
    Node {
        id: "lundgren.hello",
        speaker: "LUNDGREN",
        text: "Uff da, a Vault-dweller! Sit, sit. I never thought I'd live to see that door open.\n\nI'll tell you what I saw. A week before it opened, three Vault-Tec trucks went by on the old highway, heading east, lights on. None came back. There was smoke on the road after.",
        choices: &[
            say("Where did they go down?", Some("lundgren.where")),
            Choice { set: &["gift.lundgren"], needs: Needs::NoFlag("gift.lundgren"), gift: Some(Gift::Hotdish(1)), ..say("Got anything to eat?", Some("lundgren.food")) },
            say("Take care.", None),
        ],
        with_log: false,
    },
    Node {
        id: "lundgren.where",
        speaker: "LUNDGREN",
        text: "West of here, out on US-169, a little past the lake road. You'll see the tyres before you see the trucks. Take a rifle. Whatever stopped them might still be around.",
        choices: &[
            Choice { set: &["gift.lundgren"], needs: Needs::NoFlag("gift.lundgren"), gift: Some(Gift::Hotdish(1)), ..say("Got anything to eat?", Some("lundgren.food")) },
            say("Thanks.", None),
        ],
        with_log: false,
    },
    Node {
        id: "lundgren.food",
        speaker: "LUNDGREN",
        text: "Hotdish, straight from the pot. Eat it warm, and don't ask what's in it.",
        choices: &[say("Thanks.", None)],
        with_log: false,
    },
    Node {
        id: "lundgren.after",
        speaker: "LUNDGREN",
        text: "You found the trucks? Then it's true. The vault sent help and something took it. Whatever the Overseer says, folks out here aren't forgotten. Stay warm.",
        choices: &[say("I will.", None)],
        with_log: false,
    },
    // ---- Sven: the Glowmoose ----
    Node {
        id: "sven.hello",
        speaker: "SVEN",
        text: "Stay low and keep your voice down. A Glowmoose has been grazing east of the shanty for a week, antlers lit up like a Christmas tree. It tore my nets and put Gunnar through the ice. I can't leave.\n\nIf you've got a gun and the nerve, I'd pay what I've got.",
        choices: &[
            Choice { set: &["talked.sven"], ..say("I'll take care of it.", Some("sven.accept")) },
            say("How do I beat it?", Some("sven.advice")),
            say("Not my problem.", Some("sven.refuse")),
        ],
        with_log: false,
    },
    Node {
        id: "sven.accept",
        speaker: "SVEN",
        text: "Bless you. It charges in a straight line, so step aside at the last second and let it hit a tree. Then give it everything you've got.",
        choices: &[say("Back in a bit.", None)],
        with_log: false,
    },
    Node {
        id: "sven.advice",
        speaker: "SVEN",
        text: "Never stand still. When it paws the snow, it's about to charge. Step sideways with a pine behind you and it'll stun itself. Then shoot, and don't stop.",
        choices: &[Choice { set: &["talked.sven"], ..say("I'll take care of it.", Some("sven.accept")) }, say("Go back.", Some("sven.hello"))],
        with_log: false,
    },
    Node {
        id: "sven.refuse",
        speaker: "SVEN",
        text: "Well. Minnesota Nice says I shouldn't say what I think.",
        choices: &[Choice { set: &["talked.sven"], ..say("Wait. I'll help.", Some("sven.accept")) }, say("Goodbye.", None)],
        with_log: false,
    },
    Node {
        id: "sven.after",
        speaker: "SVEN",
        text: "It's gone? You... well. Here. It's all the scrap I've pulled out of the lake this winter. Take it, I won't need it now.",
        choices: &[
            Choice { set: &["gift.sven"], needs: Needs::NoFlag("gift.sven"), gift: Some(Gift::Scrap(10)), ..say("Take the scrap.", None) },
            say("Glad to help.", None),
        ],
        with_log: false,
    },
    // ---- Olson: the Mills ----
    Node {
        id: "olson.hello",
        speaker: "OLSON",
        text: "Bait, tackle and bad news. The beacon on the Mills' silo is still up there, and if somebody put power back to it, the whole county would see a light.\n\nThe breaker panel's fused. A few strands of copper stripped from scrap and a steady hand would fix it: three scrap. The radiation up there cooks you slowly, so don't dawdle.",
        choices: &[
            say("Where's the panel?", Some("olson.where")),
            say("Why light a beacon?", Some("olson.why")),
            say("Goodbye.", None),
        ],
        with_log: false,
    },
    Node {
        id: "olson.where",
        speaker: "OLSON",
        text: "North-east, at Golden Atomic Mills. It's a grey box on a post, beside the silos. If your Geiger counter's screaming, you're in the right place.",
        choices: &[say("Go back.", Some("olson.hello")), say("Thanks.", None)],
        with_log: false,
    },
    Node {
        id: "olson.why",
        speaker: "OLSON",
        text: "The Overseer asked, I hear. And a light means someone's home. After two hundred winters of dark, I'd take a light.",
        choices: &[say("Go back.", Some("olson.hello"))],
        with_log: false,
    },
    Node {
        id: "olson.after",
        speaker: "OLSON",
        text: "I saw it from the ice last night: the Mills lit up like the Fourth of July. You've done something good for everyone out here. Take this, it's the only Stimpak I've got.",
        choices: &[
            Choice { set: &["gift.olson"], needs: Needs::NoFlag("gift.olson"), gift: Some(Gift::Stimpak(1)), ..say("Take the Stimpak.", None) },
            say("No trouble.", None),
        ],
        with_log: false,
    },
    // ---- Ole: rumours ----
    Node {
        id: "ole.hello",
        speaker: "OLE",
        text: "Ya, you look like somebody who wants to know why the Overseer opened the door. Everybody does. I'll tell you what I've heard.",
        choices: &[
            say("What have you heard?", Some("ole.rumors")),
            say("Do you trust the Overseer?", Some("ole.trust")),
            Choice { needs: Needs::Flag("found.convoy"), ..say("I found the wrecked convoy.", Some("ole.convoy")) },
            say("Just passing through.", None),
        ],
        with_log: false,
    },
    Node {
        id: "ole.rumors",
        speaker: "OLE",
        text: "Some say a signal on the old radio. Some say the vault's running short of something. Me, I say you don't open a door shut for two hundred years unless the house is on fire.",
        choices: &[say("Go back.", Some("ole.hello"))],
        with_log: false,
    },
    Node {
        id: "ole.convoy",
        speaker: "OLE",
        text: "Vault-Tec trucks, with the cab doors torn off from the outside? Then the Overseer's been asking the wrong questions. Nothing human peels a door like that. Something big, and hungry for more than cargo.",
        choices: &[say("Go back.", Some("ole.hello"))],
        with_log: false,
    },
    Node {
        id: "ole.trust",
        speaker: "OLE",
        text: "I trust the ice, the stove and my dog, in that order. The Overseer's somewhere after the dog.",
        choices: &[say("Go back.", Some("ole.hello"))],
        with_log: false,
    },
    Node {
        id: "ole.truth",
        speaker: "OLE",
        text: "Heard the vault told everyone the real story. Frightening. But a frightened man lights his stove in October, a calm one finds out in February. Good on you.",
        choices: &[say("Goodbye.", None)],
        with_log: false,
    },
    Node {
        id: "ole.cover",
        speaker: "OLE",
        text: "Heard the vault says the door opened on its own. Hm. In my experience doors don't. But folks are calm, and maybe that's worth something. I won't ask.",
        choices: &[say("Goodbye.", None)],
        with_log: false,
    },
];

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
            Stage::Complete if has(flags, ENDING_TRUTH) => "ov.after.truth",
            Stage::Complete => "ov.after.cover",
        },
        Npc::Lundgren if done(flags, Objective::Convoy) => "lundgren.after",
        Npc::Lundgren => "lundgren.hello",
        Npc::Sven if done(flags, Objective::Moose) => "sven.after",
        Npc::Sven => "sven.hello",
        Npc::Olson if done(flags, Objective::Power) => "olson.after",
        Npc::Olson => "olson.hello",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn flags(names: &[&str]) -> Flags {
        names.iter().map(|s| s.to_string()).collect()
    }

    /// Every flag combination worth entering conversations with.
    fn states() -> Vec<Flags> {
        let mut v = vec![Flags::new(), flags(&[STARTED])];
        for o in Objective::ALL {
            v.push(flags(&[o.flag()]));
        }
        v.push(flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose"]));
        v.push(flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose", REPORTED, ENDING_TRUTH]));
        v.push(flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose", REPORTED, ENDING_COVER]));
        v
    }

    #[test]
    fn node_ids_are_unique_and_every_node_has_words() {
        let mut seen = BTreeSet::new();
        for n in NODES {
            assert!(seen.insert(n.id), "duplicate {}", n.id);
            assert!(!n.text.is_empty() && !n.speaker.is_empty(), "{}", n.id);
            assert!(!n.choices.is_empty(), "{} has no replies", n.id);
            assert!(n.choices.len() <= 4, "{}: the screen shows four replies", n.id);
            for c in n.choices {
                assert!(c.text.len() <= 70, "{}: reply too long: {}", n.id, c.text);
            }
        }
    }

    #[test]
    fn every_reply_leads_somewhere_real() {
        for n in NODES {
            for c in n.choices {
                if let Some(next) = c.next {
                    assert!(node(next).is_some(), "{} -> {next} doesn't exist", n.id);
                }
            }
        }
    }

    #[test]
    fn every_node_can_be_reached_and_every_conversation_can_end() {
        let mut reached = BTreeSet::new();
        for npc in Npc::ALL {
            for f in states() {
                let mut stack = vec![entry(npc, &f)];
                while let Some(id) = stack.pop() {
                    if !reached.insert(id) {
                        continue;
                    }
                    for c in node(id).unwrap().choices {
                        stack.extend(c.next);
                    }
                }
            }
        }
        for n in NODES {
            assert!(reached.contains(n.id), "{} is never reached", n.id);
        }
        // From every node there's a way out: keep ending nodes, grow to ones that reach them.
        let mut ends: BTreeSet<&str> = NODES.iter().filter(|n| n.choices.iter().any(|c| c.next.is_none())).map(|n| n.id).collect();
        loop {
            let before = ends.len();
            for n in NODES {
                if n.choices.iter().any(|c| c.next.is_some_and(|x| ends.contains(x))) {
                    ends.insert(n.id);
                }
            }
            if ends.len() == before {
                break;
            }
        }
        for n in NODES {
            assert!(ends.contains(n.id), "{} is a trap", n.id);
        }
    }

    #[test]
    fn gifts_can_only_be_taken_once() {
        for n in NODES {
            for c in n.choices.iter().filter(|c| c.gift.is_some()) {
                let Needs::NoFlag(f) = c.needs else { panic!("{}: a gift with no limit", n.id) };
                assert!(c.set.contains(&f), "{}: taking it must set {f}", n.id);
            }
        }
        let mut f = Flags::new();
        let mut s = Session::new(Npc::Lundgren, &f);
        let step = s.pick(1, &mut f).unwrap();
        assert_eq!(step.gift, Some(Gift::Hotdish(1)));
        let again = Session::new(Npc::Lundgren, &f);
        assert!(again.choices(&f).iter().all(|c| c.gift.is_none()), "it's gone from the menu");
    }

    #[test]
    fn the_overseer_follows_the_quest() {
        let mut f = Flags::new();
        assert_eq!(entry(Npc::Overseer, &f), "ov.intro");
        let mut s = Session::new(Npc::Overseer, &f);
        let step = s.pick(0, &mut f).unwrap();
        assert!(step.ended && quest::has(&f, STARTED), "agreeing starts the quest");
        assert_eq!(entry(Npc::Overseer, &f), "ov.wait");
        for o in Objective::ALL {
            quest::set(&mut f, o.flag());
        }
        assert_eq!(entry(Npc::Overseer, &f), "ov.report");
        let mut s = Session::new(Npc::Overseer, &f);
        let step = s.pick(0, &mut f).unwrap();
        assert!(!step.ended, "the ending has a closing word");
        assert_eq!(s.node().id, "ov.end.truth");
        assert!(quest::has(&f, ENDING_TRUTH) && quest::stage(&f) == quest::Stage::Complete);
        assert_eq!(entry(Npc::Overseer, &f), "ov.after.truth");
        assert_eq!(entry(Npc::Ole, &f), "ole.truth", "the news travels");
    }

    #[test]
    fn the_cover_story_is_a_different_ending() {
        let mut f = flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose"]);
        let mut s = Session::new(Npc::Overseer, &f);
        s.pick(1, &mut f).unwrap();
        assert!(quest::has(&f, ENDING_COVER) && !quest::has(&f, ENDING_TRUTH));
        assert_eq!(entry(Npc::Overseer, &f), "ov.after.cover");
        assert_eq!(entry(Npc::Ole, &f), "ole.cover");
    }

    #[test]
    fn thinking_it_over_does_not_end_the_quest() {
        let mut f = flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose"]);
        let mut s = Session::new(Npc::Overseer, &f);
        let step = s.pick(2, &mut f).unwrap();
        assert!(step.ended);
        assert_eq!(quest::stage(&f), quest::Stage::ReadyToReport);
    }

    #[test]
    fn survivors_notice_what_you_have_done() {
        let f = flags(&["killed.glowmoose", "found.convoy", "fixed.power"]);
        assert_eq!(entry(Npc::Sven, &f), "sven.after");
        assert_eq!(entry(Npc::Lundgren, &f), "lundgren.after");
        assert_eq!(entry(Npc::Olson, &f), "olson.after");
        let none = Flags::new();
        let ole = node("ole.hello").unwrap();
        assert!(!choices(ole, &none).iter().any(|c| c.next == Some("ole.convoy")), "he can't know yet");
        assert!(choices(ole, &f).iter().any(|c| c.next == Some("ole.convoy")), "but he'll hear about the convoy");
        assert_eq!(entry(Npc::Sven, &none), "sven.hello");
        assert_eq!(entry(Npc::Ole, &none), "ole.hello");
    }

    #[test]
    fn each_fish_house_has_its_own_survivor() {
        let names: BTreeSet<&str> = (0..4).map(|i| Npc::for_house(i).name()).collect();
        assert_eq!(names.len(), 4);
        assert!(!names.contains(Npc::Overseer.name()));
    }

    #[test]
    fn the_highlight_wraps_and_a_bad_pick_does_nothing() {
        let mut f = Flags::new();
        let mut s = Session::new(Npc::Sven, &f);
        let n = s.choices(&f).len();
        s.move_by(-1, &f);
        assert_eq!(s.selected, n - 1);
        s.move_by(1, &f);
        assert_eq!(s.selected, 0);
        assert_eq!(s.pick(9, &mut f), None);
        assert_eq!(s.node().id, "sven.hello");
        s.pick(1, &mut f).unwrap();
        assert_eq!(s.node().id, "sven.advice");
        assert_eq!(s.selected, 0, "a new node starts at the top");
    }
}
