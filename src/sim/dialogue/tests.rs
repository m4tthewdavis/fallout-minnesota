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
    let q1 = [STARTED, "found.convoy", "fixed.power", "killed.glowmoose", REPORTED];
    for ending in [ENDING_TRUTH, ENDING_COVER] {
        let mut base = q1.to_vec();
        base.push(ending);
        v.push(flags(&base));
        base.push(PUMP_STARTED);
        v.push(flags(&base));
        base.extend(["found.impeller", "found.coupling", "made.seals"]);
        v.push(flags(&base));
        base.push("fixed.pump");
        v.push(flags(&base));
        base.push(PUMP_REPORTED);
        v.push(flags(&base));
    }
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

/// Quest 1 done, with the given ending.
fn after_quest_one(ending: &'static str) -> Flags {
    flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose", REPORTED, ending])
}

#[test]
fn the_whole_pump_quest_plays_through_in_dialogue_and_flags() {
    for ending in [ENDING_TRUTH, ENDING_COVER] {
        let truth = ending == ENDING_TRUTH;
        let mut f = after_quest_one(ending);
        // offered: the after-talk has the extra reply
        assert_eq!(entry(Npc::Overseer, &f), if truth { "ov.after.truth" } else { "ov.after.cover" });
        let mut s = Session::new(Npc::Overseer, &f);
        let step = s.pick(0, &mut f).unwrap();
        assert!(!step.ended);
        assert_eq!(s.node().id, "ov.pump.offer");
        s.pick(1, &mut f).unwrap();
        assert_eq!(s.node().id, "ov.pump.parts");
        s.pick(1, &mut f).unwrap();
        assert_eq!(s.node().id, "ov.pump.offer");
        assert!(!quest::has(&f, PUMP_STARTED), "asking questions is not agreeing");
        let step = s.pick(0, &mut f).unwrap();
        assert!(step.ended && quest::has(&f, PUMP_STARTED));
        assert_eq!(quest::pump_stage(&f), PumpStage::Active);
        // the reply is gone from the after nodes, and the progress check shows the log
        assert_eq!(entry(Npc::Overseer, &f), "ov.pump.wait");
        assert!(node("ov.pump.wait").unwrap().with_log);
        // the parts, in a scrambled order
        for part in [quest::Part::Seals, quest::Part::Impeller, quest::Part::Coupling] {
            assert_eq!(entry(Npc::Overseer, &f), "ov.pump.wait");
            quest::set(&mut f, part.flag());
        }
        assert_eq!(entry(Npc::Overseer, &f), "ov.pump.install");
        quest::set(&mut f, quest::PUMP_FIXED);
        assert_eq!(entry(Npc::Overseer, &f), "ov.pump.report");
        let mut s = Session::new(Npc::Overseer, &f);
        let replies = s.choices(&f);
        assert_eq!(replies.len(), 2, "one ending offered, plus 'a moment'");
        assert_eq!(s.pick(1, &mut f).map(|st| st.ended), Some(true), "needing a moment ends the talk");
        assert!(!quest::has(&f, PUMP_REPORTED));
        let mut s = Session::new(Npc::Overseer, &f);
        let step = s.pick(0, &mut f).unwrap();
        assert!(!step.ended);
        assert_eq!(s.node().id, if truth { "ov.pump.end.truth" } else { "ov.pump.end.cover" });
        assert_eq!(quest::pump_stage(&f), PumpStage::Complete);
        assert_eq!(entry(Npc::Overseer, &f), if truth { "ov.pump.after.truth" } else { "ov.pump.after.cover" });
        assert_eq!(entry(Npc::Ole, &f), if truth { "ole.pump.truth" } else { "ole.pump.cover" });
        assert!(s.pick(0, &mut f).unwrap().ended);
        assert_eq!(quest::quest_xp(&f) - quest::quest_xp(&after_quest_one(ending)), 700);
    }
}

#[test]
fn the_pump_endings_read_differently() {
    let t = node("ov.pump.end.truth").unwrap();
    let c = node("ov.pump.end.cover").unwrap();
    assert_ne!(t.text, c.text);
    assert_ne!(node("ole.pump.truth").unwrap().text, node("ole.pump.cover").unwrap().text);
    assert_ne!(node("ov.pump.after.truth").unwrap().text, node("ov.pump.after.cover").unwrap().text);
}

#[test]
fn nobody_is_offered_the_pump_before_the_first_quest_ends() {
    for f in [Flags::new(), flags(&[STARTED]), flags(&[STARTED, "found.convoy", "fixed.power", "killed.glowmoose"])] {
        let id = entry(Npc::Overseer, &f);
        assert!(!id.starts_with("ov.pump") && !id.starts_with("ov.after"), "{id}");
    }
    // even with the flag set by accident, the Overseer keeps to quest 1
    assert_eq!(entry(Npc::Overseer, &flags(&[STARTED, PUMP_STARTED])), "ov.wait");
}

#[test]
fn the_survivors_know_about_the_pump_once_you_have_agreed() {
    let before = after_quest_one(ENDING_TRUTH);
    let mut started = before.clone();
    quest::set(&mut started, PUMP_STARTED);
    for (npc, after_node, pump_node) in [(Npc::Lundgren, "lundgren.after", "lundgren.pump"), (Npc::Sven, "sven.after", "sven.pump"), (Npc::Olson, "olson.after", "olson.pump")] {
        assert_eq!(entry(npc, &started), after_node);
        let n = node(after_node).unwrap();
        assert!(!choices(n, &before).iter().any(|c| c.next == Some(pump_node)), "{pump_node} before agreeing");
        let mut f = started.clone();
        let mut s = Session::new(npc, &f);
        let i = s.choices(&f).iter().position(|c| c.next == Some(pump_node)).unwrap_or_else(|| panic!("{pump_node} not offered"));
        s.pick(i, &mut f).unwrap();
        assert_eq!(s.node().id, pump_node);
    }
    assert!(node("olson.pump").unwrap().text.contains("power"));
    assert!(node("lundgren.pump").unwrap().text.contains("stockroom"));
    // Ole says nothing new until the pump is reported
    assert_eq!(entry(Npc::Ole, &started), "ole.truth");
}

#[test]
fn the_olson_stimpak_is_still_there_with_the_pump_reply_added() {
    let mut f = after_quest_one(ENDING_TRUTH);
    quest::set(&mut f, PUMP_STARTED);
    let s = Session::new(Npc::Olson, &f);
    assert_eq!(s.choices(&f).len(), 3);
    assert!(s.choices(&f)[0].gift.is_some());
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
