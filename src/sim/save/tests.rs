use super::*;

/// A game with something unusual about every part of it.
struct Rig {
    survival: Survival,
    inv: Inventory,
    arsenal: Arsenal,
    clock: Clock,
    weather: Weather,
    fog: Fog,
}

fn rig() -> Rig {
    let mut survival = Survival::new();
    survival.health = 63.0;
    survival.body_heat = 41.5;
    survival.rads = 230.0;
    survival.frostbite = true;
    let mut inv = Inventory::starting_kit();
    inv.stimpaks = 5;
    inv.scrap = 21;
    inv.pelts = 2;
    inv.shells = 8;
    inv.has_frostfang_coat = true;
    let mut arsenal = Arsenal::starting();
    arsenal.unlock(WeaponKind::ScrapShotgun);
    arsenal.unlock(WeaponKind::IceAxe);
    arsenal.weapons[0].apply_upgrade(Upgrade::ExtendedMag).unwrap();
    arsenal.weapons[0].apply_upgrade(Upgrade::HeavyLoads).unwrap();
    arsenal.weapons[0].mag = 7;
    arsenal.weapons[1].apply_upgrade(Upgrade::Choke).unwrap();
    arsenal.weapons[1].mag = 1;
    arsenal.weapons[1].jammed = true;
    arsenal.current = 1;
    let clock = Clock { hours: 21.25, day: 4 };
    let weather = Weather { phase: Phase::Blizzard, timer: 33.5, blizzards: 3 };
    let mut fog = Fog::new();
    fog.reveal(0.0, 150.0, 40.0);
    fog.reveal(-90.0, -30.0, 30.0);
    fog.landmarks_found[0] = true;
    fog.landmarks_found[3] = true;
    Rig { survival, inv, arsenal, clock, weather, fog }
}

fn snapshot(r: &Rig) -> Snapshot<'_> {
    Snapshot {
        survival: &r.survival,
        inv: &r.inv,
        arsenal: &r.arsenal,
        kills: 17,
        clock: &r.clock,
        weather: &r.weather,
        fog: &r.fog,
        player: PlayerSave { x: -78.0, y: 1.25, z: -41.0, yaw: 2.0, pitch: -0.3 },
        play_secs: 4_000.0,
        saved_at: 1_700_000_000,
        opened: vec![[10, 20], [-4, 7]],
        looted: vec![LootedSave { key: [3, 4], items: vec![1, 0, 1] }],
        collected: vec![[100, -100]],
        interior: None,
        flags: vec!["met_olson".into(), "convoy_found".into()],
    }
}

fn restore(save: &SaveGame) -> (Survival, Inventory, Arsenal, u32, Clock, Weather, Fog) {
    let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 0, Clock::new(), Weather::new(), Fog::new());
    save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
    (s, i, a, k, c, w, f)
}

#[test]
fn a_save_round_trips_through_text_and_restores_everything() {
    let r = rig();
    let save = SaveGame::capture(&snapshot(&r));
    let again = SaveGame::parse(&save.to_json()).expect("our own file reads back");
    assert_eq!(again, save);

    let (s, i, a, kills, c, w, f) = restore(&again);
    assert_eq!((s.health, s.body_heat, s.rads, s.frostbite), (63.0, 41.5, 230.0, true));
    assert_eq!((i.stimpaks, i.scrap, i.pelts, i.shells, i.has_frostfang_coat), (5, 21, 2, 8, true));
    assert_eq!(kills, 17);
    assert_eq!((c.hours, c.day), (21.25, 4));
    assert_eq!((w.phase, w.timer, w.blizzards), (Phase::Blizzard, 33.5, 3));
    assert_eq!(f.seen_count(), r.fog.seen_count());
    assert_eq!(f.landmarks_found, r.fog.landmarks_found);
    assert_eq!(a.owned, [true, true, false, true]);
    assert_eq!(a.current, 1);
    // Upgrades are re-applied, so their effects come back with them.
    let rifle = a.get(WeaponKind::PipeRifle);
    assert!(rifle.has_upgrade(Upgrade::ExtendedMag) && rifle.has_upgrade(Upgrade::HeavyLoads));
    assert_eq!(rifle.mag_size, r.arsenal.weapons[0].mag_size);
    assert_eq!(rifle.damage, r.arsenal.weapons[0].damage);
    assert_eq!(rifle.mag, 7);
    let shotgun = a.get(WeaponKind::ScrapShotgun);
    assert!(shotgun.has_upgrade(Upgrade::Choke) && shotgun.jammed);
    assert_eq!(shotgun.spread, r.arsenal.weapons[1].spread);
    assert_eq!(shotgun.mag, 1);
    assert!(!a.get(WeaponKind::Revolver).has_upgrade(Upgrade::Choke));
    assert_eq!(again.opened, vec![[-4, 7], [10, 20]], "kept (sorted)");
    assert_eq!(again.looted, vec![LootedSave { key: [3, 4], items: vec![0, 1] }], "partial loot kept, tidied");
    assert_eq!(again.collected, vec![[100, -100]]);
    assert_eq!(again.flags, vec!["convoy_found".to_string(), "met_olson".to_string()]);
}

#[test]
fn what_a_save_does_not_cover_is_reset_cleanly() {
    let r = rig();
    let save = SaveGame::capture(&snapshot(&r));
    let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 0, Clock::new(), Weather::new(), Fog::new());
    a.weapons[0].busy = 1.0;
    a.draw = 0.3;
    save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
    assert_eq!(a.weapons[0].busy, 0.0, "no half-finished reload");
    assert_eq!(a.draw, 0.0);
    assert_eq!(a.weapons[0].cooldown, 0.0);
}

#[test]
fn loading_overwrites_a_game_already_in_progress() {
    let r = rig();
    let save = SaveGame::capture(&snapshot(&r));
    let (mut s, mut i, mut a, mut k, mut c, mut w, mut f) = (Survival::new(), Inventory::starting_kit(), Arsenal::starting(), 99, Clock::new(), Weather::new(), Fog::new());
    f.reveal(100.0, 100.0, 60.0);
    i.hotdish = 40;
    a.weapons[2].apply_upgrade(Upgrade::HeavyLoads).unwrap();
    save.apply(&mut s, &mut i, &mut a, &mut k, &mut c, &mut w, &mut f);
    assert_eq!(k, 17);
    assert_eq!(i.hotdish, r.inv.hotdish, "not the 40 held before");
    assert!(!a.weapons[2].has_upgrade(Upgrade::HeavyLoads), "upgrades fitted after the save are gone");
    assert_eq!(f.seen_count(), r.fog.seen_count(), "map seen after the save is forgotten");
}

#[test]
fn a_new_games_save_is_valid_and_describes_the_start() {
    let save = SaveGame::default();
    let again = SaveGame::parse(&save.to_json()).unwrap();
    assert_eq!(again, save);
    assert!(again.describe().starts_with("Day 1 07:30  Lvl 1"));
    assert_eq!(again.level(), 1);
}

#[test]
fn describing_a_save_shows_day_time_level_place_and_play_time() {
    let r = rig();
    let save = SaveGame::capture(&snapshot(&r));
    let text = save.describe();
    assert!(text.starts_with("Day 4 21:15"), "{text}");
    assert!(text.contains("1h 06m"), "{text}");
    assert!(text.contains(&save.place), "{text}");
    assert!(save.level() > 1, "17 kills and a few finds make a higher level");
    let short = SaveGame { play_secs: 600.0, interior: Some("fish_house_1".into()), ..save };
    let t = short.describe();
    assert!(t.contains("10m") && t.contains("indoors"), "{t}");
}

#[test]
fn places_are_named_when_close_and_generic_when_far() {
    let first = &mapdata::landmarks()[0];
    assert_eq!(place_name(first.x, first.z + 5.0), format!("near {}", first.name));
    assert_eq!(place_name(-9000.0, 9000.0), "Mille Lacs");
    let (ox, oz) = Interior::VaultLobby.origin();
    assert_eq!(place_name(ox, oz), "inside Vault 143");
}

#[test]
fn a_save_that_isnt_json_or_isnt_ours_is_refused_politely() {
    assert_eq!(SaveGame::parse(""), Err(LoadError::Corrupt));
    assert_eq!(SaveGame::parse("not json"), Err(LoadError::Corrupt));
    assert_eq!(SaveGame::parse("{\"version\": 1, \"player\": "), Err(LoadError::Corrupt), "cut off mid-write");
    assert_eq!(SaveGame::parse("{}"), Err(LoadError::NotASave), "an empty object must not load as a fresh game");
    assert_eq!(SaveGame::parse("[1, 2, 3]"), Err(LoadError::NotASave));
    assert_eq!(SaveGame::parse("{\"version\": \"one\"}"), Err(LoadError::NotASave));
    for e in [LoadError::Corrupt, LoadError::NotASave, LoadError::TooNew(9), LoadError::TooOld(0), LoadError::Invalid("x".into())] {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn newer_and_older_formats_are_rejected_rather_than_misread() {
    let newer = format!("{{\"version\": {}, \"future_field\": true}}", SAVE_VERSION + 1);
    assert_eq!(SaveGame::parse(&newer), Err(LoadError::TooNew(SAVE_VERSION + 1)));
    assert_eq!(SaveGame::parse("{\"version\": 99999999999}"), Err(LoadError::TooNew(u32::MAX)));
    assert_eq!(SaveGame::parse("{\"version\": 0}"), Err(LoadError::TooOld(0)));
    assert!(LoadError::TooNew(2).to_string().contains("newer"));
}

#[test]
fn missing_fields_take_defaults_and_unknown_ones_are_ignored() {
    let save = SaveGame::parse(r#"{ "version": 1, "kills": 3, "mystery": {"a": 1}, "player": { "x": 12.0 } }"#).unwrap();
    assert_eq!(save.kills, 3);
    assert_eq!(save.player.x, 12.0);
    assert_eq!(save.player.z, PlayerSave::default().z, "missing z keeps its default");
    assert_eq!(save.inventory, InventorySave::default());
    assert_eq!(save.arsenal.owned, [true, false, false, false]);
}

#[test]
fn wrong_types_are_an_error_not_a_crash() {
    for bad in [
        r#"{ "version": 1, "kills": "many" }"#,
        r#"{ "version": 1, "player": 5 }"#,
        r#"{ "version": 1, "weather": { "phase": "Hail" } }"#,
        r#"{ "version": 1, "survival": { "health": null } }"#,
        r#"{ "version": 1, "inventory": { "stimpaks": -3 } }"#,
    ] {
        assert!(matches!(SaveGame::parse(bad), Err(LoadError::Invalid(_))), "{bad}");
    }
}

#[test]
fn out_of_range_values_are_pulled_into_range() {
    let save = SaveGame::parse(
        r#"{ "version": 1,
             "player": { "x": 99999.0, "z": -99999.0, "pitch": 9.0 },
             "survival": { "health": 5000.0, "body_heat": -40.0, "rads": 900.0 },
             "inventory": { "stimpaks": 4000000000, "scrap": 50000 },
             "arsenal": { "owned": [false, false, false, false], "current": 17, "weapons": [] },
             "clock": { "hours": 61.0, "day": 0 },
             "weather": { "phase": "Warning", "timer": 99999.0 } }"#,
    )
    .unwrap();
    assert_eq!((save.player.x, save.player.z), (PlayerSave::default().x, PlayerSave::default().z), "off the map: back to the start");
    assert_eq!(save.player.pitch, 1.5);
    assert_eq!(save.survival.body_heat, 0.0);
    assert_eq!(save.survival.rads, 900.0);
    assert_eq!(save.survival.health, 10.0, "900 rads leave room for only 10 HP");
    assert_eq!((save.inventory.stimpaks, save.inventory.scrap), (MAX_COUNT, MAX_COUNT));
    assert_eq!(save.arsenal.owned, [true, false, false, false], "you always own at least one weapon");
    assert_eq!(save.arsenal.current, 0);
    assert_eq!(save.arsenal.weapons.len(), 4, "missing weapon entries are filled in");
    assert!((0.0..24.0).contains(&save.clock.hours));
    assert_eq!(save.clock.day, 1);
    assert_eq!(save.weather.timer, 600.0);
}

#[test]
fn a_dead_or_dying_save_is_given_a_sliver_of_health() {
    let save = SaveGame::parse(r#"{ "version": 1, "survival": { "health": 0.0 } }"#).unwrap();
    assert_eq!(save.survival.health, 1.0, "you can't load into the death screen");
}

#[test]
fn the_current_weapon_must_be_one_you_own() {
    let save = SaveGame::parse(r#"{ "version": 1, "arsenal": { "owned": [true, false, true, false], "current": 1 } }"#).unwrap();
    assert_eq!(save.arsenal.current, 0, "slot 1 isn't owned, so back to the first weapon you have");
    let save = SaveGame::parse(r#"{ "version": 1, "arsenal": { "owned": [false, false, true, false], "current": 3 } }"#).unwrap();
    assert_eq!(save.arsenal.current, 2);
}

#[test]
fn interiors_keep_their_off_map_positions_but_outdoor_positions_are_checked() {
    let (ox, oz) = Interior::FishHouse(1).origin();
    let inside = SaveGame::parse(&format!(r#"{{ "version": 1, "interior": "fish_house_1", "player": {{ "x": {ox}, "y": 0.0, "z": {oz} }} }}"#)).unwrap();
    assert_eq!((inside.player.x, inside.player.z), (ox, oz));
    let outside = SaveGame::parse(r#"{ "version": 1, "player": { "x": 1500.0, "z": 1500.0 } }"#).unwrap();
    assert_ne!(outside.player.x, 1500.0);
}

#[test]
fn an_unknown_interior_means_outdoors_and_a_stray_position_goes_to_the_door() {
    let lost = SaveGame::parse(r#"{ "version": 1, "interior": "castle", "player": { "x": 1000.0, "z": 1000.0 } }"#).unwrap();
    assert_eq!(lost.interior, None, "a room we don't know isn't a room");
    assert_eq!((lost.player.x, lost.player.z), (PlayerSave::default().x, PlayerSave::default().z), "and the off-map position is replaced");
    let wandered = SaveGame::parse(r#"{ "version": 1, "interior": "vault_lobby", "player": { "x": 5.0, "z": 5.0, "y": 9.0 } }"#).unwrap();
    let (ex, ez, _) = Interior::VaultLobby.entry();
    assert_eq!((wandered.player.x, wandered.player.z, wandered.player.y), (ex, ez, interiors::FLOOR_Y), "you're put at the lobby's door");
    let fine = SaveGame::parse(&format!(r#"{{ "version": 1, "interior": "mart", "player": {{ "x": {}, "z": {} }} }}"#, Interior::Mart.origin().0, Interior::Mart.origin().1)).unwrap();
    assert_eq!(fine.interior.as_deref(), Some("mart"));
    assert_eq!(fine.player.x, Interior::Mart.origin().0, "a position inside its room is kept");
}

#[test]
fn partly_looted_containers_are_tidied_and_never_double_counted() {
    let mut save = SaveGame {
        opened: vec![[1, 1]],
        looted: vec![
            LootedSave { key: [1, 1], items: vec![0] },
            LootedSave { key: [2, 2], items: vec![] },
            LootedSave { key: [3, 3], items: vec![2, 99, 2] },
        ],
        ..SaveGame::default()
    };
    save.sanitize();
    assert_eq!(save.looted, vec![LootedSave { key: [3, 3], items: vec![2] }]);
    // An old save with no such field still loads.
    let old = SaveGame::parse(r#"{ "version": 1, "opened": [[5, 5]] }"#).unwrap();
    assert!(old.looted.is_empty());
}

#[test]
fn fog_that_does_not_cover_the_map_is_dropped_but_the_rest_loads() {
    let save = SaveGame::parse(r#"{ "version": 1, "kills": 4, "fog": { "runs": [3, 4, 5], "found": [true] } }"#).unwrap();
    assert!(save.fog.runs.is_empty(), "wrong total: forget the fog");
    assert_eq!(save.kills, 4, "everything else is kept");
    let (_, _, _, _, _, _, fog) = restore(&save);
    assert_eq!(fog.seen_count(), 0);
    assert!(fog.landmarks_found[0], "found places still apply");
}

#[test]
fn huge_lists_and_odd_flags_are_trimmed() {
    let mut save = SaveGame { opened: (0..MAX_LIST as i32 + 500).map(|i| [i, 0]).collect(), ..SaveGame::default() };
    save.flags = vec!["".into(), "a".into(), "a".into(), "x".repeat(200)];
    save.sanitize();
    assert_eq!(save.opened.len(), MAX_LIST);
    assert_eq!(save.flags, vec!["a".to_string()], "empty, duplicate and absurdly long flags removed");
}

#[test]
fn nan_never_reaches_the_file() {
    let mut r = rig();
    r.survival.rads = f32::NAN;
    r.clock.hours = f32::INFINITY;
    let mut snap = snapshot(&r);
    snap.player.x = f32::NAN;
    let save = SaveGame::capture(&snap);
    let text = save.to_json();
    // (A null for "no interior" is fine; a null where a number belongs is not.)
    let again = SaveGame::parse(&text).expect("a save made from broken numbers still loads");
    assert!(again.survival.rads.is_finite() && again.clock.hours.is_finite() && again.player.x.is_finite());
    assert_eq!(again.player.x, PlayerSave::default().x, "an impossible position becomes the start");
}

#[test]
fn world_keys_are_stable_to_a_quarter_metre() {
    assert_eq!(world_key(10.0, 20.0), world_key(10.05, 19.95));
    assert_ne!(world_key(10.0, 20.0), world_key(10.5, 20.0));
    assert_eq!(world_key(-3.0, 0.0), [-12, 0]);
}
