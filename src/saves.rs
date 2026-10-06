//! Saving and loading: five slots (autosave, quicksave and three manual
//! ones) kept as JSON files in the user's data folder. What a save contains,
//! how it is validated and how old formats are handled is in `sim::save`;
//! this file gathers the game's state into one, writes it, and applies a
//! loaded save back onto the running world.
//!
//! The world is built from a fixed seed, so a save names crates and pickups by
//! where they are. Loading puts you, your things and the world's opened
//! crates and taken items back as they were; wolves and moose are not saved,
//! so they are cleared and repopulate.

use std::collections::{BTreeSet, HashSet};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::enemy::Body;
use crate::interact::Container;
use crate::menu::{close_menu, SaveRequest, SlotSummaries};
use crate::pipboy::PipState;
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::menu::{slot_name, AUTOSAVE, QUICKSAVE, SLOTS};
use crate::sim::save::{world_key, LootedSave, PlayerSave, SaveGame, Snapshot, WorldKey};
use crate::sim::terrain;
use crate::sim::interiors::Interior;
use crate::state::{alive, ClockRes, CurrentInterior, Game, Messages, Paused, WeatherRes};
use crate::storage;
use crate::world::{Collected, LootRing, Pickup};

/// Seconds played (counted only while the game is running, not paused).
#[derive(Resource, Default)]
pub struct PlayTime(pub f32);

/// Story and quest flags, saved with the game.
#[derive(Resource, Default)]
pub struct StoryFlags(pub BTreeSet<String>);

/// Ask for the autosave slot to be written (sleeping, taking shelter).
#[derive(Event)]
pub struct AutosaveRequest;

/// A save has just been loaded (other systems drop what they remembered).
#[derive(Event)]
pub struct Loaded;

/// A save that has been read and checked and is about to be applied.
#[derive(Resource)]
struct PendingLoad {
    save: Box<SaveGame>,
    slot: usize,
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayTime>()
            .init_resource::<StoryFlags>()
            .add_event::<AutosaveRequest>()
            .add_event::<Loaded>()
            .add_systems(Startup, refresh_slots)
            .add_systems(
                Update,
                (
                    play_time.run_if(alive),
                    quicksave_key.run_if(alive),
                    auto_load,
                    handle_requests,
                    (apply_game_state, apply_world_state).chain().run_if(resource_exists::<PendingLoad>),
                )
                    .chain(),
            );
    }
}

pub fn slot_file(slot: usize) -> String {
    format!("save_{slot}.json")
}

/// One line per slot for the menus: what's in it, or `None` if it's empty.
/// A slot's one-line summary and when it was written (0 if unknown).
fn read_slot(slot: usize) -> (Option<String>, u64) {
    let Some(text) = storage::read(&storage::data_dir(), &slot_file(slot)) else { return (None, 0) };
    match SaveGame::parse(&text) {
        Ok(save) => (Some(save.describe()), save.saved_at),
        Err(_) => (Some("damaged save".to_string()), 0),
    }
}

fn refresh_slots(mut slots: ResMut<SlotSummaries>) {
    for slot in 0..SLOTS {
        (slots.0[slot], slots.1[slot]) = read_slot(slot);
    }
}

fn play_time(time: Res<Time>, mut play: ResMut<PlayTime>) {
    play.0 += time.delta_secs();
}

/// F4 writes the quicksave slot.
fn quicksave_key(keys: Res<ButtonInput<KeyCode>>, mut requests: EventWriter<SaveRequest>) {
    if keys.just_pressed(KeyCode::F4) {
        requests.write(SaveRequest::Save(QUICKSAVE));
    }
}

/// Screenshot and test mode: FMN_AUTOLOAD=<slot> loads that slot a moment after start.
fn auto_load(real: Res<Time<Real>>, mut done: Local<bool>, mut requests: EventWriter<SaveRequest>) {
    if *done || real.elapsed_secs() < 1.5 {
        return;
    }
    *done = true;
    if let Some(slot) = std::env::var("FMN_AUTOLOAD").ok().and_then(|s| s.parse::<usize>().ok()) {
        requests.write(SaveRequest::Load(slot));
    }
}

/// Everything a save reads from the running game.
#[derive(SystemParam)]
struct WorldView<'w, 's> {
    game: Res<'w, Game>,
    weather: Res<'w, WeatherRes>,
    clock: Res<'w, ClockRes>,
    pip: Res<'w, PipState>,
    play: Res<'w, PlayTime>,
    flags: Res<'w, StoryFlags>,
    interior: Res<'w, CurrentInterior>,
    player: Query<'w, 's, (&'static Transform, &'static Player)>,
    containers: Query<'w, 's, (&'static Transform, &'static Container)>,
    taken: Query<'w, 's, &'static Transform, (With<Pickup>, With<Collected>)>,
}

impl WorldView<'_, '_> {
    fn capture(&self) -> SaveGame {
        let (feet, yaw, pitch) = self
            .player
            .single()
            .map(|(tf, p)| (tf.translation - Vec3::Y * EYE_HEIGHT, p.yaw, p.pitch))
            .unwrap_or((Vec3::new(terrain::PLAYER_SPAWN.0, 0.0, terrain::PLAYER_SPAWN.1), 0.0, 0.0));
        let key = |tf: &Transform| world_key(tf.translation.x, tf.translation.z);
        let opened: Vec<WorldKey> = self.containers.iter().filter(|(_, c)| c.opened).map(|(tf, _)| key(tf)).collect();
        let looted: Vec<LootedSave> = self
            .containers
            .iter()
            .filter(|(_, c)| !c.opened && c.taken.iter().any(|t| *t))
            .map(|(tf, c)| LootedSave { key: key(tf), items: c.taken_indices() })
            .collect();
        let collected: Vec<WorldKey> = self.taken.iter().map(key).collect();
        let saved_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        SaveGame::capture(&Snapshot {
            survival: &self.game.survival,
            inv: &self.game.inv,
            arsenal: &self.game.arsenal,
            kills: self.game.kills,
            clock: &self.clock.0,
            weather: &self.weather.weather,
            fog: &self.pip.fog,
            player: PlayerSave { x: feet.x, y: feet.y, z: feet.z, yaw, pitch },
            play_secs: self.play.0,
            saved_at,
            opened,
            looted,
            collected,
            interior: self.interior.0.map(|i| i.id()),
            flags: self.flags.0.iter().cloned().collect(),
        })
    }
}

/// Carry out save and load requests from the menus, the Pip-Boy and the
/// quicksave key.
#[allow(clippy::too_many_arguments)]
fn handle_requests(
    mut commands: Commands,
    mut requests: EventReader<SaveRequest>,
    mut autosaves: EventReader<AutosaveRequest>,
    view: WorldView,
    mut msgs: ResMut<Messages>,
    mut slots: ResMut<SlotSummaries>,
    mut paused: ResMut<Paused>,
    mut vtime: ResMut<Time<Virtual>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let mut jobs: Vec<(SaveRequest, bool)> = requests.read().map(|r| (*r, false)).collect();
    jobs.extend(autosaves.read().map(|_| (SaveRequest::Save(AUTOSAVE), true)));
    let dir = storage::data_dir();
    for (job, quiet) in jobs {
        match job {
            SaveRequest::Save(slot) => {
                if view.game.death.is_some() {
                    msgs.show("You can't save while you're dead.", 2.5);
                    continue;
                }
                let save = view.capture();
                match storage::write(&dir, &slot_file(slot), &save.to_json()) {
                    Ok(()) => {
                        info!("saved {} at ({:.1}, {:.1}): {}", slot_name(slot), save.player.x, save.player.z, save.describe());
                        slots.0[slot] = Some(save.describe());
                        slots.1[slot] = save.saved_at;
                        msgs.show(if quiet { "Autosaved.".to_string() } else { format!("Saved: {}", slot_name(slot)) }, 2.5);
                        if paused.0 {
                            close_menu(&mut paused, &mut vtime, &mut windows);
                        }
                    }
                    Err(e) => {
                        warn!("could not save: {e}");
                        msgs.show(format!("Could not save: {e}"), 4.0);
                    }
                }
            }
            SaveRequest::Load(slot) => {
                let Some(text) = storage::read(&dir, &slot_file(slot)) else {
                    msgs.show(format!("{} is empty.", slot_name(slot)), 2.5);
                    continue;
                };
                match SaveGame::parse(&text) {
                    Ok(save) => {
                        commands.insert_resource(PendingLoad { save: Box::new(save), slot });
                    }
                    Err(e) => {
                        warn!("could not load {}: {e:?}", slot_name(slot));
                        msgs.show(e.to_string(), 4.0);
                    }
                }
            }
        }
    }
}

/// Step one of a load: the numbers (vitals, inventory, weapons, clock, weather,
/// fog, story flags).
#[allow(clippy::too_many_arguments)]
fn apply_game_state(
    pending: Res<PendingLoad>,
    mut game: ResMut<Game>,
    mut weather: ResMut<WeatherRes>,
    mut clock: ResMut<ClockRes>,
    mut pip: ResMut<PipState>,
    mut flags: ResMut<StoryFlags>,
    mut interior: ResMut<CurrentInterior>,
    mut play: ResMut<PlayTime>,
) {
    let s = &pending.save;
    let game = &mut *game;
    s.apply(&mut game.survival, &mut game.inv, &mut game.arsenal, &mut game.kills, &mut clock.0, &mut weather.weather, &mut pip.fog);
    game.death = None;
    game.hurt_flash = 0.0;
    game.recoil = 0.0;
    weather.just_changed = None;
    // The map picture is redrawn from the restored fog next time it's needed.
    pip.dirty = true;
    flags.0 = s.flags.iter().cloned().collect();
    interior.0 = s.interior.as_deref().and_then(Interior::parse);
    play.0 = s.play_secs;
}

/// Step two: the world itself (where you stand, which crates are open, which
/// items are gone, and the enemies), then back into the game.
#[allow(clippy::too_many_arguments)]
fn apply_world_state(
    mut commands: Commands,
    pending: Res<PendingLoad>,
    mut msgs: ResMut<Messages>,
    mut paused: ResMut<Paused>,
    mut vtime: ResMut<Time<Virtual>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut player: Query<(&mut Transform, &mut Player), (Without<Container>, Without<Pickup>)>,
    mut containers: Query<(&Transform, &mut Container), Without<Player>>,
    pickups: Query<(Entity, &Transform), (With<Pickup>, Without<Player>, Without<Container>)>,
    rings: Query<(Entity, &LootRing)>,
    bodies: Query<Entity, With<Body>>,
    mut loaded: EventWriter<Loaded>,
) {
    let s = &pending.save;
    if let Ok((mut tf, mut p)) = player.single_mut() {
        *p = Player::new();
        p.yaw = s.player.yaw;
        p.pitch = s.player.pitch;
        let floor = if s.interior.is_some() { s.player.y } else { terrain::walk_height(s.player.x, s.player.z) };
        tf.translation = Vec3::new(s.player.x, floor + EYE_HEIGHT, s.player.z);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
    }

    // Crates: open exactly the ones that were open.
    let opened: HashSet<WorldKey> = s.opened.iter().copied().collect();
    let looted: std::collections::HashMap<WorldKey, &Vec<u8>> = s.looted.iter().map(|l| (l.key, &l.items)).collect();
    for (tf, mut c) in &mut containers {
        let key = world_key(tf.translation.x, tf.translation.z);
        c.restore(opened.contains(&key), looted.get(&key).map(|v| v.as_slice()).unwrap_or(&[]));
        let ring = if c.opened { Visibility::Hidden } else { Visibility::Inherited };
        commands.entity(c.ring()).insert(ring);
    }

    // Pickups: bring back the ones not yet taken, hide the ones that were.
    let taken: HashSet<WorldKey> = s.collected.iter().copied().collect();
    let mut gone: HashSet<Entity> = HashSet::new();
    for (e, tf) in &pickups {
        if taken.contains(&world_key(tf.translation.x, tf.translation.z)) {
            gone.insert(e);
            commands.entity(e).insert((Collected, Visibility::Hidden));
        } else {
            commands.entity(e).remove::<Collected>().insert(Visibility::Inherited);
        }
    }
    for (ring, owner) in &rings {
        commands.entity(ring).insert(if gone.contains(&owner.0) { Visibility::Hidden } else { Visibility::Inherited });
    }

    // Enemies aren't saved: clear them (and any corpses) and let the packs return.
    for e in &bodies {
        commands.entity(e).despawn();
    }
    commands.run_system_cached(crate::wolves::spawn_initial_packs);
    commands.run_system_cached(crate::moose::spawn_initial);
    commands.run_system_cached(crate::crows::spawn_initial);
    commands.run_system_cached(crate::raiders::spawn_initial);

    info!("loaded {} at ({:.1}, {:.1}): {}", slot_name(pending.slot), s.player.x, s.player.z, s.describe());
    msgs.show(format!("Loaded: {}", slot_name(pending.slot)), 3.0);
    if paused.0 {
        close_menu(&mut paused, &mut vtime, &mut windows);
    }
    loaded.write(Loaded);
    commands.remove_resource::<PendingLoad>();
}
