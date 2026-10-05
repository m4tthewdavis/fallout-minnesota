//! The first quest line, "Why Did the Overseer Open the Door?", on screen:
//! the conversation panel (Pip-Boy ice blue, replies you pick with the keyboard),
//! the places the quest sends you (the wrecked convoy, the Mills' breaker
//! panel), the Glowmoose that has to die, and the level-up perk screen.
//! What gets said, and the rules for stages, XP and perks, are in
//! `sim::dialogue` and `sim::quest`.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use crate::theme::{ACCENT, ACCENT_DIM, MENU_PANEL, SELECTED_FILL};

use crate::assets::GameAssets;
use crate::enemy::{Body, Dying, Species};
use crate::moose::{spawn_moose, MooseAssets};
use crate::pipboy::{total_upgrades, PipState};
use crate::player::Player;
use crate::saves::{AutosaveRequest, Loaded, StoryFlags};
use crate::sim::collision::{self, Shape};
use crate::sim::dialogue::{Gift, Npc, Session};
use crate::sim::loot;
use crate::sim::progress;
use crate::sim::quest::{self as rules, Effects, Objective, Perk, Spot, Stage};
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, RAD_SOURCES, SHELTERS};
use crate::state::{alive, outdoors, Colliders, Game, Hostile, Messages, RngRes, SfxQueue, Talking};
use crate::world::{glow, ground, mat, prop, PulseLight};


/// Where the wrecked convoy lies, on the highway west of the lake road.
pub const CONVOY_AT: (f32, f32) = (-110.0, 97.0);

/// What your perks do right now (recomputed when your flags change).
#[derive(Resource, Default)]
pub struct Perks(pub Effects);

/// Start a conversation.
#[derive(Event)]
pub struct StartTalk(pub Npc);

/// Use a place the quest asks you to use.
#[derive(Event)]
pub struct UseSpot(pub Spot);

/// What the conversation panel is showing.
pub(crate) enum Conversation {
    Talk(Session),
    Perks { offer: Vec<Perk>, selected: usize },
}

#[derive(Resource, Default)]
pub(crate) struct Talk(Option<Conversation>);

/// Seconds before the level-up screen may open again after you put it off.
#[derive(Resource, Default)]
struct PerkDelay(f32);

/// The territorial Glowmoose that has to be killed for Sven.
#[derive(Component)]
pub(crate) struct QuestMoose;

/// Shown only while the Mills have power.
#[derive(Component)]
struct PowerOn;

/// Shown only while they don't.
#[derive(Component)]
struct PowerOff;

#[derive(Component)]
struct TalkRoot;
#[derive(Component)]
struct TalkSpeaker;
#[derive(Component)]
struct TalkBody;
#[derive(Component)]
struct TalkRow(usize);
#[derive(Component)]
struct TalkHint;

const ROWS: usize = 4;

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Perks>()
            .init_resource::<Talk>()
            .init_resource::<PerkDelay>()
            .add_event::<StartTalk>()
            .add_event::<UseSpot>()
            .add_systems(Startup, (build_talk_ui, spawn_quest_world.after(crate::state::WorldGen)))
            .add_systems(
                Update,
                (
                    drive_talk.run_if(|t: Res<Talk>| t.0.is_some()),
                    begin_talk,
                    use_spots,
                    moose_deeds,
                    ensure_quest_moose.run_if(alive.and(outdoors)),
                    (level_watch, offer_perks.run_if(alive)).chain(),
                    sync_perks,
                    apply_power,
                    render_talk,
                )
                    .chain(),
            );
    }
}

// ---------------------------------------------------------------------------
// The world the quest uses
// ---------------------------------------------------------------------------

fn spawn_quest_world(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;
    // ---- The convoy: three Vault-Tec trucks' worth of wreckage on the road ----
    let (cx, cz) = CONVOY_AT;
    // (dx, dz, yaw, flipped)
    for (n, (dx, dz, yaw, flipped)) in [(-5.0f32, -1.5f32, 0.2f32, false), (4.5, 1.8, 2.9, true), (0.5, -2.2, -0.5, false)].into_iter().enumerate() {
        let (x, z) = (cx + dx, cz + dz);
        let gy = ground(x, z);
        let (y, roll) = if flipped { (gy + 1.45, std::f32::consts::PI) } else { (gy, 0.0) };
        commands.spawn((
            SceneRoot(assets.covered_car.clone()),
            Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(roll)).with_scale(Vec3::splat(1.0 + 0.15 * n as f32)),
        ));
        let (ax, az) = (yaw.sin() * 1.2, yaw.cos() * 1.2);
        solid.push(Shape::Circle { x: x + ax, z: z + az, r: 1.0 });
        solid.push(Shape::Circle { x: x - ax, z: z - az, r: 1.0 });
    }
    // Spilled cargo: crates, drums, tyres and fuel.
    for (n, (dx, dz)) in [(-2.0f32, 1.8f32), (1.8, -0.4), (-0.4, 3.0), (2.8, -2.6), (-3.3, -3.0), (0.0, 0.8)].into_iter().enumerate() {
        let (x, z) = (cx + dx, cz + dz);
        let scene = [&assets.crate_military, &assets.barrel, &assets.tyre, &assets.crate_wood, &assets.jerrycan, &assets.crate_military][n];
        prop(&mut commands, scene, Vec3::new(x, ground(x, z), z), n as f32 * 1.3, if n == 0 || n == 5 { 1.2 } else { 1.0 });
    }
    // Still smouldering, after all this time.
    crate::particles::spawn_fire(&mut commands, &mut meshes, &mut materials, &assets.soft, Vec3::new(cx + 1.0, ground(cx + 1.0, cz - 3.4), cz - 3.4), 0.45, 91.0, false);
    crate::interiors::spawn_spot(&mut commands, Vec3::new(cx, ground(cx, cz) + 1.0, cz + 0.4), Spot::Convoy);

    // ---- Golden Atomic Mills: the breaker panel and the beacon ----
    let (gx, gz, _, _) = RAD_SOURCES[1];
    let gzz = gz + 9.0;
    let (px, pz) = (gx + 1.8, gzz + 5.2);
    let py = ground(px, pz);
    let box_mat = mat(&mut materials, Color::srgb(0.32, 0.33, 0.33));
    let dark = mat(&mut materials, Color::srgb(0.05, 0.05, 0.05));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.14, 1.5, 0.14))), MeshMaterial3d(assets.pole_wood.clone()), Transform::from_xyz(px - 0.4, py + 0.75, pz)));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.14, 1.5, 0.14))), MeshMaterial3d(assets.pole_wood.clone()), Transform::from_xyz(px + 0.4, py + 0.75, pz)));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.9, 1.0, 0.3))), MeshMaterial3d(box_mat), Transform::from_xyz(px, py + 1.3, pz)));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(0.74, 0.04, 0.02))), MeshMaterial3d(dark.clone()), Transform::from_xyz(px, py + 1.3, pz + 0.16)));
    let red = glow(&mut materials, Color::srgb(1.0, 0.1, 0.05), LinearRgba::rgb(12.0, 0.6, 0.2));
    let green = glow(&mut materials, Color::srgb(0.2, 1.0, 0.3), LinearRgba::rgb(0.6, 8.0, 1.2));
    commands.spawn((Mesh3d(meshes.add(Sphere::new(0.06))), MeshMaterial3d(red), Transform::from_xyz(px, py + 1.65, pz + 0.17), PowerOff, NotShadowCaster));
    commands.spawn((Mesh3d(meshes.add(Sphere::new(0.06))), MeshMaterial3d(green), Transform::from_xyz(px, py + 1.65, pz + 0.17), PowerOn, Visibility::Hidden, NotShadowCaster));
    solid.push(Shape::rect_centered(px, pz, 1.1, 0.6));
    crate::interiors::spawn_spot(&mut commands, Vec3::new(px, py + 1.0, pz), Spot::Breaker);
    // The beacon on the tallest silo: dark until the power is back.
    let (bx, bz) = (gx + 5.0, gzz);
    let top = ground(bx, bz) - 0.3 + 22.0 + 2.9;
    let beacon = glow(&mut materials, Color::srgb(1.0, 0.9, 0.7), LinearRgba::rgb(14.0, 10.0, 5.0));
    commands.spawn((Mesh3d(meshes.add(Sphere::new(0.45))), MeshMaterial3d(beacon), Transform::from_xyz(bx, top, bz), PowerOn, Visibility::Hidden, NotShadowCaster));
    commands.spawn((
        PointLight { color: Color::srgb(1.0, 0.88, 0.65), intensity: 4_000_000.0, range: 70.0, ..default() },
        Transform::from_xyz(bx, top + 0.6, bz),
        PulseLight { base: 4_000_000.0, speed: 1.4 },
        PowerOn,
        Visibility::Hidden,
    ));
}

/// Beacon and panel lamp follow the Mills' power.
fn apply_power(flags: Res<StoryFlags>, mut on: Query<&mut Visibility, (With<PowerOn>, Without<PowerOff>)>, mut off: Query<&mut Visibility, (With<PowerOff>, Without<PowerOn>)>) {
    let powered = rules::done(&flags.0, Objective::Power);
    let (a, b) = if powered { (Visibility::Inherited, Visibility::Hidden) } else { (Visibility::Hidden, Visibility::Inherited) };
    for mut v in &mut on {
        v.set_if_neq(a);
    }
    for mut v in &mut off {
        v.set_if_neq(b);
    }
}

/// Keep Sven's Glowmoose in the world until it's been killed: a big bull,
/// grazing east of the shanty.
fn ensure_quest_moose(
    mut commands: Commands,
    flags: Res<StoryFlags>,
    assets: Option<Res<MooseAssets>>,
    mut rng: ResMut<RngRes>,
    colliders: Res<Colliders>,
    existing: Query<(), With<QuestMoose>>,
) {
    if rules::done(&flags.0, Objective::Moose) || !existing.is_empty() {
        return;
    }
    let Some(assets) = assets else { return };
    let (sx, sz) = SHELTERS[1];
    let Some((x, z)) = [(15.0f32, -10.0f32), (18.0, 6.0), (12.0, 14.0), (-14.0, 12.0), (20.0, -18.0)]
        .into_iter()
        .map(|(dx, dz)| (sx + dx, sz + dz))
        .find(|&(x, z)| terrain::is_open_ground(x, z) && !collision::blocked(x, z, 2.0, &colliders.0))
    else {
        return;
    };
    let id = spawn_moose(&mut commands, &assets, Vec2::new(x, z), &mut rng);
    commands.entity(id).insert((QuestMoose, Body::new(Species::Moose, 320.0, 1.6, 1.25)));
}

/// The deed is done when Sven's moose starts to fall.
fn moose_deeds(
    mut flags: ResMut<StoryFlags>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut autosave: EventWriter<AutosaveRequest>,
    fallen: Query<(), (With<QuestMoose>, Added<Dying>)>,
) {
    if fallen.is_empty() || rules::done(&flags.0, Objective::Moose) {
        return;
    }
    rules::set(&mut flags.0, Objective::Moose.flag());
    announce(Objective::Moose, &flags.0, &mut msgs, &mut sfx, &mut autosave);
}

/// Tell the player what a deed was worth and what's next, and save.
fn announce(o: Objective, flags: &rules::Flags, msgs: &mut Messages, sfx: &mut SfxQueue, autosave: &mut EventWriter<AutosaveRequest>) {
    let started = rules::has(flags, rules::STARTED);
    let next = match rules::stage(flags) {
        Stage::ReadyToReport => "  All done: report to the Overseer.".to_string(),
        _ if !started => "  (The Overseer's terminal in Vault 143 would like to hear about it.)".to_string(),
        _ => String::new(),
    };
    msgs.show(format!("Objective complete: {}  +{} XP{}", o.title(), o.xp(), next), 6.0);
    sfx.play(Sound::PickupMed);
    autosave.write(AutosaveRequest);
}

/// E at the convoy or the breaker panel.
#[allow(clippy::too_many_arguments)]
fn use_spots(
    mut events: EventReader<UseSpot>,
    mut flags: ResMut<StoryFlags>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut rng: ResMut<RngRes>,
    perks: Res<Perks>,
    mut autosave: EventWriter<AutosaveRequest>,
) {
    for UseSpot(spot) in events.read() {
        let objective = spot.objective();
        if rules::done(&flags.0, objective) {
            continue;
        }
        match spot {
            Spot::Convoy => {
                let cache = loot::roll_cache(&mut rng.0);
                let Game { inv, arsenal, .. } = &mut *game;
                let scrap_before = inv.scrap;
                let gained = loot::grant(&cache, inv, arsenal);
                if inv.scrap > scrap_before {
                    inv.scrap += perks.0.extra_scrap;
                }
                msgs.show(
                    format!("The convoy's manifest: one replacement coolant pump, 'VAULT 143, OVERSEER ONLY'. The cab doors are torn open from the outside. You find: {}.", gained.join(", ")),
                    7.0,
                );
            }
            Spot::Breaker => {
                if game.inv.scrap < rules::BREAKER_SCRAP {
                    msgs.show(format!("The fuses are burnt out. Rewiring takes {} scrap, and you have {}.", rules::BREAKER_SCRAP, game.inv.scrap), 3.5);
                    sfx.play(Sound::DryClick);
                    continue;
                }
                game.inv.scrap -= rules::BREAKER_SCRAP;
                sfx.play(Sound::Craft);
            }
        }
        rules::set(&mut flags.0, objective.flag());
        announce(objective, &flags.0, &mut msgs, &mut sfx, &mut autosave);
    }
}

// ---------------------------------------------------------------------------
// Levels and perks
// ---------------------------------------------------------------------------

fn current_level(game: &Game, pip: &PipState, flags: &rules::Flags) -> u32 {
    let places = pip.fog.landmarks_found.iter().filter(|f| **f).count();
    progress::level(progress::experience(game.kills, places, total_upgrades(game), game.inv.has_frostfang_coat, rules::quest_xp(flags))).0
}

/// Announce each new level as it's reached (not when a save loads).
fn level_watch(
    game: Res<Game>,
    pip: Res<PipState>,
    flags: Res<StoryFlags>,
    mut loaded: EventReader<Loaded>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut last: Local<u32>,
) {
    let level = current_level(&game, &pip, &flags.0);
    if loaded.read().count() > 0 || *last == 0 {
        *last = level;
        return;
    }
    if level > *last {
        msgs.show(format!("LEVEL UP! You are now level {level}."), 4.0);
        sfx.play(Sound::PickupMed);
    }
    *last = level;
}

/// When a perk pick is waiting, and nothing dangerous is near, show the choice.
#[allow(clippy::too_many_arguments)]
fn offer_perks(
    time: Res<Time>,
    game: Res<Game>,
    pip: Res<PipState>,
    flags: Res<StoryFlags>,
    mut delay: ResMut<PerkDelay>,
    mut talk: ResMut<Talk>,
    mut talking: ResMut<Talking>,
    player: Query<&Transform, With<Player>>,
    hostile: Query<&Transform, (With<Hostile>, Without<Dying>, Without<Player>)>,
) {
    delay.0 = (delay.0 - time.delta_secs()).max(0.0);
    if talk.0.is_some() || delay.0 > 0.0 {
        return;
    }
    if rules::picks_available(current_level(&game, &pip, &flags.0), &flags.0) == 0 {
        return;
    }
    let Ok(p) = player.single() else { return };
    if hostile.iter().any(|h| h.translation.distance(p.translation) < 35.0) {
        return;
    }
    talk.0 = Some(Conversation::Perks { offer: rules::offer(&flags.0), selected: 0 });
    talking.0 = true;
}

fn sync_perks(flags: Res<StoryFlags>, mut perks: ResMut<Perks>) {
    if flags.is_changed() {
        perks.0 = rules::effects(&flags.0);
    }
}

// ---------------------------------------------------------------------------
// Conversations
// ---------------------------------------------------------------------------

fn begin_talk(mut events: EventReader<StartTalk>, flags: Res<StoryFlags>, mut talk: ResMut<Talk>, mut talking: ResMut<Talking>, mut sfx: ResMut<SfxQueue>) {
    let Some(StartTalk(npc)) = events.read().last() else { return };
    if talk.0.is_some() {
        return;
    }
    talk.0 = Some(Conversation::Talk(Session::new(*npc, &flags.0)));
    talking.0 = true;
    sfx.play(Sound::PipOn);
}

const NUMBER_KEYS: [KeyCode; ROWS] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];

/// Up/down and Enter drive the panel; Esc leaves.
#[allow(clippy::too_many_arguments)]
fn drive_talk(
    keys: Res<ButtonInput<KeyCode>>,
    mut talk: ResMut<Talk>,
    mut talking: ResMut<Talking>,
    mut flags: ResMut<StoryFlags>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut delay: ResMut<PerkDelay>,
    mut autosave: EventWriter<AutosaveRequest>,
) {
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    let confirm = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyE);
    let quick = NUMBER_KEYS.iter().position(|k| keys.just_pressed(*k));
    let leave = keys.just_pressed(KeyCode::Escape);
    let Some(conv) = talk.0.as_mut() else { return };
    let mut close = false;
    match conv {
        Conversation::Talk(session) => {
            if up || down {
                session.move_by(if up { -1 } else { 1 }, &flags.0);
                sfx.play(Sound::PipScroll);
            }
            let pick = quick.or(confirm.then_some(session.selected));
            if let Some(i) = pick {
                let before = rules::stage(&flags.0);
                if let Some(step) = session.pick(i, &mut flags.0) {
                    sfx.play(Sound::UiTab);
                    if let Some(gift) = step.gift {
                        match gift {
                            Gift::Hotdish(n) => game.inv.hotdish += n,
                            Gift::Scrap(n) => game.inv.scrap += n,
                            Gift::Stimpak(n) => game.inv.stimpaks += n,
                        }
                        msgs.show(format!("Received: {}", gift.describe()), 3.0);
                        sfx.play(Sound::PickupFood);
                    }
                    let after = rules::stage(&flags.0);
                    if after != before {
                        match after {
                            Stage::Active => msgs.show(format!("New quest: {}", rules::TITLE), 5.0),
                            Stage::Complete => msgs.show(format!("Quest complete: {}  +{} XP", rules::TITLE, rules::REPORT_XP), 6.0),
                            _ => {}
                        }
                        autosave.write(AutosaveRequest);
                    }
                    close = step.ended;
                }
            }
            close |= leave;
        }
        Conversation::Perks { offer, selected } => {
            if (up || down) && !offer.is_empty() {
                *selected = (*selected as i32 + if up { -1 } else { 1 }).rem_euclid(offer.len() as i32) as usize;
                sfx.play(Sound::PipScroll);
            }
            let pick = quick.or(confirm.then_some(*selected)).and_then(|i| offer.get(i).copied());
            if let Some(perk) = pick {
                rules::set(&mut flags.0, &perk.flag());
                msgs.show(format!("Perk gained: {}. {}", perk.name(), perk.describe()), 5.0);
                sfx.play(Sound::PickupMed);
                autosave.write(AutosaveRequest);
                close = true;
            } else if leave {
                // Put it off for a bit; the pick isn't lost.
                delay.0 = 60.0;
                close = true;
            }
        }
    }
    if close {
        talk.0 = None;
        talking.0 = false;
        sfx.play(Sound::PipOff);
    }
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

fn build_talk_ui(mut commands: Commands, assets: Res<GameAssets>) {
    let font = |size: f32| TextFont { font: assets.font.clone(), font_size: size, ..default() };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(36.0),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(130),
            TalkRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(780.0),
                    max_width: Val::Percent(94.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(MENU_PANEL),
                BorderColor(ACCENT.with_alpha(0.7)),
            ))
            .with_children(|p| {
                p.spawn((Text::new(""), font(19.0), TextColor(ACCENT), TalkSpeaker));
                p.spawn((Text::new(""), font(17.0), TextColor(ACCENT.with_alpha(0.9)), TalkBody));
                p.spawn(Node { height: Val::Px(2.0), width: Val::Percent(100.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() }).insert(BackgroundColor(ACCENT_DIM));
                for i in 0..ROWS {
                    p.spawn((
                        Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), display: Display::None, ..default() },
                        Text::new(""),
                        font(17.0),
                        TextColor(ACCENT),
                        BackgroundColor(Color::NONE),
                        TalkRow(i),
                    ));
                }
                p.spawn((Text::new(""), font(12.0), TextColor(ACCENT_DIM), TalkHint));
            });
        });
}

/// Draw whatever the panel should show; hidden when there's no conversation.
#[allow(clippy::too_many_arguments)]
fn render_talk(
    talk: Res<Talk>,
    flags: Res<StoryFlags>,
    game: Res<Game>,
    pip: Res<PipState>,
    mut root: Query<&mut Node, (With<TalkRoot>, Without<TalkRow>)>,
    mut speaker: Query<&mut Text, (With<TalkSpeaker>, Without<TalkBody>, Without<TalkRow>, Without<TalkHint>)>,
    mut body: Query<&mut Text, (With<TalkBody>, Without<TalkSpeaker>, Without<TalkRow>, Without<TalkHint>)>,
    mut hint: Query<&mut Text, (With<TalkHint>, Without<TalkSpeaker>, Without<TalkBody>, Without<TalkRow>)>,
    mut rows: Query<(&TalkRow, &mut Node, &mut Text, &mut BackgroundColor), (Without<TalkRoot>, Without<TalkSpeaker>, Without<TalkBody>, Without<TalkHint>)>,
) {
    let (Ok(mut root), Ok(mut speaker), Ok(mut body), Ok(mut hint)) = (root.single_mut(), speaker.single_mut(), body.single_mut(), hint.single_mut()) else { return };
    let Some(conv) = &talk.0 else {
        root.display = Display::None;
        return;
    };
    root.display = Display::Flex;
    let (name, text, items, selected, footer): (String, String, Vec<String>, usize, &str) = match conv {
        Conversation::Talk(s) => {
            let node = s.node();
            let mut text = node.text.to_string();
            if node.with_log {
                text.push('\n');
                for l in rules::log(&flags.0) {
                    text.push_str(&format!("\n  [{}] {}", if l.done { "X" } else { " " }, l.text));
                }
                if let Some(h) = rules::next_hint(&flags.0) {
                    text.push_str(&format!("\n\n{h}"));
                }
            }
            let items = s.choices(&flags.0).iter().map(|c| c.text.to_string()).collect();
            (node.speaker.to_string(), text, items, s.selected, "UP/DOWN select    ENTER or 1-4 choose    ESC leave")
        }
        Conversation::Perks { offer, selected } => {
            let level = current_level(&game, &pip, &flags.0);
            let items = offer.iter().map(|p| format!("{}: {}", p.name(), p.describe())).collect();
            (format!("LEVEL {level}"), "You've earned a perk. Choose one.".to_string(), items, *selected, "UP/DOWN select    ENTER or 1-3 choose    ESC decide later")
        }
    };
    speaker.set_if_neq(Text::new(name));
    body.set_if_neq(Text::new(text));
    hint.set_if_neq(Text::new(footer));
    for (row, mut node, mut t, mut bg) in &mut rows {
        match items.get(row.0) {
            Some(item) => {
                node.display = Display::Flex;
                t.set_if_neq(Text::new(format!("{} {}. {}", if row.0 == selected { ">" } else { " " }, row.0 + 1, item)));
                bg.set_if_neq(BackgroundColor(if row.0 == selected { SELECTED_FILL } else { Color::NONE }));
            }
            None => node.display = Display::None,
        }
    }
}

/// Screenshot helper (`FMN_SLAY=1`): fell Sven's moose, to see the quest react.
pub(crate) fn dev_slay(mut commands: Commands, moose: Query<(Entity, &Transform, &Body), (With<QuestMoose>, Without<Dying>)>) {
    for (e, tf, body) in &moose {
        commands.entity(e).insert(Dying::new(tf, body, 1.0));
    }
}

/// Screenshot helper: open `FMN_TALK=<npc|perks>` straight away.
pub(crate) fn open_dev_talk(name: &str, flags: &rules::Flags, talk: &mut Talk, talking: &mut Talking) {
    talk.0 = if name == "perks" {
        Some(Conversation::Perks { offer: rules::offer(flags), selected: 0 })
    } else {
        Npc::ALL.into_iter().find(|n| format!("{n:?}").eq_ignore_ascii_case(name)).map(|npc| Conversation::Talk(Session::new(npc, flags)))
    };
    talking.0 = talk.0.is_some();
}
