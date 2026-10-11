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
