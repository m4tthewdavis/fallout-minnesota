//! Enterable interiors: the four fish houses (a bunk to sleep in, a stove to
//! heat a meal on, a stash), the Vault 143 lobby (blast door, the Overseer's
//! terminal) and the Bullseye-Mart stockroom (shelving, caches, the shotgun
//! footlocker).
//!
//! Each room is built once at start-up, far off the map, and shown only while
//! you're in it. A door fades the screen to black, moves you across, swaps the
//! weather and light for the room's own, and fades back. Where things are,
//! how sleeping and cooking work and the fade's timing are in
//! `sim::interiors`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::sim::keys::Bind;

use crate::assets::GameAssets;
use crate::characters::PersonKit;
use crate::interact::{spawn_container, ContainerAssets, FixtureClaim};
use crate::meshes::to_mesh_tangents;
use crate::player::{Player, EYE_HEIGHT};
use crate::quest::{Perks, StartTalk, UseSpot};
use crate::saves::{AutosaveRequest, StoryFlags};
use crate::sim::dialogue::Npc;
use crate::sim::quest::{self, Spot, Stage};
use crate::sim::collision::Shape;
use crate::sim::combat::WeaponKind;
use crate::sim::interiors::{self, Fade, Interior};
use crate::sim::loot;
use crate::sim::meshgen;
use crate::sim::terrain;
use crate::state::{alive, ClockRes, Colliders, CurrentInterior, Game, Messages, Prompt, RngRes, SfxQueue, Transition, WeatherRes};
use crate::sim::synth::Sound;
use crate::world::{flicker_light, glow, ground, mat};

/// How close (metres) you must be to use a door, bunk, stove or terminal.
const REACH: f32 = 1.9;

#[derive(Component)]
struct InteriorRoot(Interior);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Door {
    Enter(Interior),
    Leave(Interior),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FixtureKind {
    Door(Door),
    Bunk,
    Stove,
    /// A survivor to talk to, or the Overseer's terminal.
    Talk(Npc),
    /// A place the quest line asks you to use, out in the world.
    Spot(Spot),
}

/// Something you can use with E. `space` is where it is: a room, or `None`
/// for out in the world.
#[derive(Component)]
pub(crate) struct Fixture {
    pub(crate) kind: FixtureKind,
    pub(crate) space: Option<Interior>,
}

/// Put something usable in the world (not in a room) at `pos`.
pub(crate) fn spawn_spot(commands: &mut Commands, pos: Vec3, spot: Spot) {
    commands.spawn((Transform::from_translation(pos), Fixture { kind: FixtureKind::Spot(spot), space: None }));
}

/// What a fade in progress is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Trip {
    Enter(Interior),
    Leave(Interior),
    Sleep(Interior),
}

#[derive(Resource, Default)]
struct Moving(Option<(Fade, Trip)>);

#[derive(Component)]
struct FadeOverlay;
#[derive(Component)]
struct FadeText;

pub struct InteriorPlugin;

impl Plugin for InteriorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Moving>()
            .add_systems(Startup, build_fade_overlay)
            .add_systems(
                Update,
                (
                    use_fixtures.run_if(alive).after(crate::interact::reset_prompt).before(crate::interact::interact),
                    run_trips,
                    show_current_room,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Building the rooms
// ---------------------------------------------------------------------------

/// Materials shared by the rooms.
struct Kit {
    planks: Handle<StandardMaterial>,
    concrete: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    rust: Handle<StandardMaterial>,
    pole: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    wool_green: Handle<StandardMaterial>,
    wool_red: Handle<StandardMaterial>,
    pillow: Handle<StandardMaterial>,
    yellow: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    lamp: Handle<StandardMaterial>,
    strip: Handle<StandardMaterial>,
    screen: Handle<StandardMaterial>,
    cold_light: Handle<StandardMaterial>,
}

fn make_kit(materials: &mut Assets<StandardMaterial>, assets: &GameAssets) -> Kit {
    Kit {
        planks: assets.planks_dark.clone(),
        concrete: assets.concrete.clone(),
        metal: assets.vault_metal.clone(),
        rust: assets.rust.clone(),
        pole: assets.pole_wood.clone(),
        dark: mat(materials, Color::srgb(0.07, 0.06, 0.05)),
        black: mat(materials, Color::srgb(0.02, 0.02, 0.025)),
        wool_green: mat(materials, Color::srgb(0.2, 0.28, 0.18)),
        wool_red: mat(materials, Color::srgb(0.5, 0.13, 0.1)),
        pillow: mat(materials, Color::srgb(0.75, 0.72, 0.66)),
        yellow: mat(materials, Color::srgb(0.85, 0.66, 0.1)),
        window: glow(materials, Color::srgb(0.5, 0.6, 0.7), LinearRgba::rgb(0.5, 0.65, 0.85)),
        lamp: glow(materials, Color::srgb(1.0, 0.8, 0.45), LinearRgba::rgb(5.0, 3.0, 0.9)),
        strip: glow(materials, Color::srgb(0.9, 0.95, 1.0), LinearRgba::rgb(3.0, 3.4, 4.0)),
        screen: glow(materials, Color::srgb(1.0, 0.6, 0.2), LinearRgba::rgb(2.4, 1.2, 0.3)),
        cold_light: glow(materials, Color::srgb(0.7, 0.8, 0.95), LinearRgba::rgb(0.9, 1.2, 1.8)),
    }
}

/// A box centred at `at` (local to the room's centre, y up from the floor).
fn block(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, at: [f32; 3], size: [f32; 3]) {
    r.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid(size, 1.5)))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(at[0], at[1], at[2]),
    ));
}

/// A box that gives off light and casts no shadow.
fn glowing(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, at: [f32; 3], size: [f32; 3]) {
    r.spawn((Mesh3d(meshes.add(Cuboid::new(size[0], size[1], size[2]))), MeshMaterial3d(material.clone()), Transform::from_xyz(at[0], at[1], at[2]), NotShadowCaster));
}

fn cylinder(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, at: [f32; 3], radius: f32, height: f32) {
    r.spawn((Mesh3d(meshes.add(Cylinder::new(radius, height))), MeshMaterial3d(material.clone()), Transform::from_xyz(at[0], at[1], at[2])));
}

fn light(r: &mut ChildSpawnerCommands, color: Color, intensity: f32, range: f32, at: [f32; 3]) {
    r.spawn((PointLight { color, intensity, range, ..default() }, Transform::from_xyz(at[0], at[1], at[2])));
}

fn flicker(r: &mut ChildSpawnerCommands, color: Color, intensity: f32, range: f32, at: [f32; 3], seed: f32) {
    r.spawn((flicker_light(color, intensity, range, false, seed), Transform::from_xyz(at[0], at[1], at[2])));
}

/// The four walls, floor and ceiling of a room, with colliders. `door_wall`
/// is the south wall. Returns nothing: the rest is furniture.
#[allow(clippy::too_many_arguments)]
fn shell(
    r: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    solid: &mut Vec<Shape>,
    room: Interior,
    floor: &Handle<StandardMaterial>,
    walls: &Handle<StandardMaterial>,
    ceiling: &Handle<StandardMaterial>,
) {
    let (hw, hd) = room.half();
    let h = room.height();
    let (ox, oz) = room.origin();
    let t = 0.2;
    block(r, meshes, floor, [0.0, -0.05, 0.0], [hw * 2.0 + 0.8, 0.1, hd * 2.0 + 0.8]);
    block(r, meshes, ceiling, [0.0, h + 0.05, 0.0], [hw * 2.0 + 0.8, 0.1, hd * 2.0 + 0.8]);
    for (x, z, w, d) in [
        (0.0, -hd - t / 2.0, hw * 2.0 + 0.8, t),
        (0.0, hd + t / 2.0, hw * 2.0 + 0.8, t),
        (-hw - t / 2.0, 0.0, t, hd * 2.0),
        (hw + t / 2.0, 0.0, t, hd * 2.0),
    ] {
        block(r, meshes, walls, [x, h / 2.0, z], [w, h, d]);
        solid.push(Shape::rect_centered(ox + x, oz + z, w + 0.6, d + 0.6));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_interiors(
    mut commands: Commands,
    assets: Res<GameAssets>,
    containers: Res<ContainerAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
    people: Res<PersonKit>,
) {
    let solid = &mut colliders.0;
    let kit = make_kit(&mut materials, &assets);
    for room in interiors::ALL {
        let (ox, oz) = room.origin();
        // Rooms are hidden until you're in one (see `show_current_room`).
        let root = commands.spawn((Transform::from_xyz(ox, 0.0, oz), Visibility::Hidden, InteriorRoot(room))).id();
        commands.entity(root).with_children(|r| match room {
            Interior::FishHouse(i) => fish_house(r, &mut meshes, &kit, &people, &assets, solid, i),
            Interior::VaultLobby => vault_lobby(r, &mut meshes, &mut materials, &kit, &assets, solid),
            Interior::Mart => mart(r, &mut meshes, &kit, &assets, solid),
        });
        // Things that live in the world rather than under the room (loot
        // containers and the stove fire) are spawned beside it.
        match room {
            Interior::FishHouse(i) => fish_house_extras(&mut commands, &assets, &containers, &mut meshes, &mut materials, &mut rng, solid, i),
            Interior::VaultLobby => vault_extras(&mut commands, &assets, &containers, &mut rng, solid),
            Interior::Mart => mart_extras(&mut commands, &assets, &containers, &mut rng, solid),
        }
        // The door out.
        let (dx, dz) = room.exit_door();
        commands.spawn((Transform::from_xyz(dx, 1.0, dz), Fixture { kind: FixtureKind::Door(Door::Leave(room)), space: Some(room) }));
        // The door in, out in the world.
        let (wx, wz) = room.outdoor_door();
        commands.spawn((Transform::from_xyz(wx, ground(wx, wz) + 1.0, wz), Fixture { kind: FixtureKind::Door(Door::Enter(room)), space: None }));
    }
}

// ---- Fish houses ----

fn fish_house(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, k: &Kit, people: &PersonKit, assets: &GameAssets, solid: &mut Vec<Shape>, i: u8) {
    let room = Interior::FishHouse(i);
    let (ox, oz) = room.origin();
    let (hw, hd) = room.half();
    let h = room.height();
    shell(r, meshes, solid, room, &k.planks, &k.planks, &k.planks);
    // Ceiling beams.
    for z in [-1.0f32, 1.0] {
        block(r, meshes, &k.pole, [0.0, h - 0.08, z], [hw * 2.0, 0.14, 0.16]);
    }
    // The door out, with cold light leaking under it.
    block(r, meshes, &k.pole, [0.0, 1.0, hd - 0.04], [0.95, 2.0, 0.07]);
    block(r, meshes, &k.dark, [0.34, 0.95, hd - 0.09], [0.06, 0.12, 0.05]);
    glowing(r, meshes, &k.cold_light, [0.0, 0.015, hd - 0.1], [0.9, 0.025, 0.03]);
    // A window on the north wall, and a stripe of day through it.
    block(r, meshes, &k.pole, [1.0, 1.55, -hd + 0.03], [0.8, 0.6, 0.05]);
    glowing(r, meshes, &k.window, [1.0, 1.55, -hd + 0.06], [0.66, 0.46, 0.03]);
    // Bunk against the west wall: two decks, posts, wool and a pillow.
    let bx = -hw + 0.5;
    for (y, wool) in [(0.45f32, &k.wool_red), (1.4, &k.wool_green)] {
        block(r, meshes, &k.pole, [bx, y - 0.05, -1.0], [0.95, 0.06, 2.1]);
        block(r, meshes, wool, [bx, y + 0.04, -0.85], [0.85, 0.12, 1.75]);
        block(r, meshes, &k.pillow, [bx, y + 0.12, -1.8], [0.5, 0.1, 0.3]);
    }
    for (dx, dz) in [(-0.45f32, -2.0f32), (0.45, -2.0), (-0.45, 0.0), (0.45, 0.0)] {
        block(r, meshes, &k.pole, [bx + dx, 0.95, dz], [0.07, 1.9, 0.07]);
    }
    solid.push(Shape::rect_centered(ox + bx, oz - 1.0, 1.1, 2.3));
    r.spawn((Transform::from_xyz(bx + 0.9, 0.7, -0.4), Fixture { kind: FixtureKind::Bunk, space: Some(room) }));
    // The stove: a barrel stove with a flue to the roof, in the north-east corner.
    let (sx, sz) = (hw - 0.75, -hd + 0.8);
    r.spawn((SceneRoot(assets.barrel_stove.clone()), Transform::from_xyz(sx, 0.0, sz).with_scale(Vec3::splat(1.0))));
    cylinder(r, meshes, &k.dark, [sx, 1.7, sz], 0.07, 1.5);
    glowing(r, meshes, &k.lamp, [sx, 0.9, sz + 0.4], [0.18, 0.1, 0.02]);
    solid.push(Shape::Circle { x: ox + sx, z: oz + sz, r: 0.55 });
    r.spawn((Transform::from_xyz(sx - 0.2, 0.9, sz + 0.6), Fixture { kind: FixtureKind::Stove, space: Some(room) }));
    // A table with a lantern, and a stool.
    block(r, meshes, &k.pole, [0.4, 0.78, -0.1], [1.3, 0.07, 0.8]);
    for (dx, dz) in [(-0.55f32, -0.3f32), (0.55, -0.3), (-0.55, 0.3), (0.55, 0.3)] {
        block(r, meshes, &k.pole, [0.4 + dx, 0.38, -0.1 + dz], [0.07, 0.76, 0.07]);
    }
    solid.push(Shape::rect_centered(ox + 0.4, oz - 0.1, 1.5, 1.0));
    cylinder(r, meshes, &k.lamp, [0.2, 0.9, -0.1], 0.07, 0.16);
    flicker(r, Color::srgb(1.0, 0.8, 0.45), 90_000.0, 6.0, [0.2, 1.05, -0.1], 5.0 + i as f32);
    cylinder(r, meshes, &k.pole, [1.2, 0.25, 0.8], 0.2, 0.5);
    // A shelf of tins on the north wall, a rug, and fishing gear on the east wall.
    block(r, meshes, &k.pole, [-0.4, 1.35, -hd + 0.15], [1.4, 0.05, 0.28]);
    for dx in [-0.8f32, -0.5, -0.2, 0.0] {
        r.spawn((SceneRoot(assets.can.clone()), Transform::from_xyz(-0.4 + dx + 0.3, 1.38, -hd + 0.15).with_scale(Vec3::splat(1.2))));
    }
    let rug = if i.is_multiple_of(2) { &k.wool_red } else { &k.wool_green };
    r.spawn((Mesh3d(meshes.add(Cylinder::new(1.0, 0.02))), MeshMaterial3d(rug.clone()), Transform::from_xyz(0.2, 0.012, 0.9)));
    for (n, z) in [-0.9f32, -0.5, -0.1].into_iter().enumerate() {
        let tilt = 0.05 * n as f32;
        r.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.012, 1.7))),
            MeshMaterial3d(k.dark.clone()),
            Transform::from_xyz(hw - 0.07, 1.2, z).with_rotation(Quat::from_rotation_z(0.04 + tilt)),
        ));
    }
    cylinder(r, meshes, &k.rust, [hw - 0.15, 0.55, 0.9], 0.04, 1.1);
    light(r, Color::srgb(1.0, 0.85, 0.6), 60_000.0, 6.0, [0.0, 2.2, 1.2]);
    // The survivor who lives here, by the table and facing the door.
    survivor(r, people, i, [-0.95, 0.0, 0.3], 0.5);
    solid.push(Shape::Circle { x: ox - 0.95, z: oz + 0.3, r: 0.4 });
    r.spawn((Transform::from_xyz(-0.95, 1.0, 0.3), Fixture { kind: FixtureKind::Talk(Npc::for_house(i)), space: Some(room) }));
}

/// A person in winter gear, standing at `at` (the floor), turned `yaw` radians
/// (0 faces +z, the south wall and its door).
fn survivor(r: &mut ChildSpawnerCommands, kit: &PersonKit, house: u8, at: [f32; 3], yaw: f32) {
    let look = &kit.survivors[house as usize % 4];
    r.spawn((Transform::from_xyz(at[0], at[1], at[2]).with_rotation(Quat::from_rotation_y(yaw)), Visibility::default())).with_children(|p| {
        for x in [-0.1f32, 0.1] {
            p.spawn((Transform::from_xyz(x, 0.82, 0.0), Visibility::default())).with_children(|l| kit.dress_leg(l));
        }
        for x in [-0.29f32, 0.29] {
            // Arms hanging, one a little forward as if warming at the stove.
            let swing = if x > 0.0 { 0.25 } else { -0.1 };
            p.spawn((Transform::from_xyz(x, 1.42, 0.0).with_rotation(Quat::from_rotation_x(swing)), Visibility::default())).with_children(|l| kit.dress_arm(l, look, x.signum()));
        }
        kit.dress_torso(p, look);
    });
}

#[allow(clippy::too_many_arguments)]
fn fish_house_extras(
    commands: &mut Commands,
    assets: &GameAssets,
    containers: &ContainerAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut RngRes,
    solid: &mut Vec<Shape>,
    i: u8,
) {
    let room = Interior::FishHouse(i);
    let (cx, cz) = room.at(-0.5, -1.55);
    let stash = loot::roll_cache(&mut rng.0);
    spawn_container(commands, containers, solid, assets.crate_wood.clone(), Vec3::new(cx, interiors::FLOOR_Y + 0.05, cz), 0.1, 1.0, "stash", stash);
    // A small flame in the stove's mouth.
    let (sx, sz) = room.at(room.half().0 - 0.75, -room.half().1 + 0.8);
    crate::particles::spawn_indoor_fire(commands, meshes, materials, &assets.soft, Vec3::new(sx, interiors::FLOOR_Y + 0.95, sz), 0.28, 20.0 + i as f32);
}

// ---- The Vault 143 lobby ----

fn vault_lobby(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, k: &Kit, assets: &GameAssets, solid: &mut Vec<Shape>) {
    let room = Interior::VaultLobby;
    let (ox, oz) = room.origin();
    let (hw, hd) = room.half();
    let h = room.height();
    shell(r, meshes, solid, room, &k.concrete, &k.metal, &k.concrete);
    // Yellow lane markings on the floor and a stripe round the walls.
    for x in [-1.6f32, 1.6] {
        glowing(r, meshes, &k.yellow, [x, 0.005, 0.0], [0.12, 0.01, hd * 2.0 - 1.0]);
    }
    for z in [-hd + 0.06, hd - 0.06] {
        glowing(r, meshes, &k.yellow, [0.0, 1.0, z], [hw * 2.0, 0.14, 0.02]);
    }
    for x in [-hw + 0.06, hw - 0.06] {
        glowing(r, meshes, &k.yellow, [x, 1.0, 0.0], [0.02, 0.14, hd * 2.0]);
    }
    // Ceiling light strips and the pools of light under them.
    for (n, z) in [-3.6f32, -1.2, 1.2, 3.6].into_iter().enumerate() {
        for x in [-3.8f32, 3.8] {
            glowing(r, meshes, &k.strip, [x, h - 0.04, z], [0.25, 0.05, 1.6]);
        }
        let _ = n;
    }
    for (x, z) in [(-3.8f32, -2.4f32), (3.8, -2.4), (-3.8, 2.4), (3.8, 2.4)] {
        light(r, Color::srgb(0.9, 0.95, 1.0), 260_000.0, 12.0, [x, h - 0.5, z]);
    }
    // Four concrete pillars.
    for (x, z) in [(-3.0f32, -2.2f32), (3.0, -2.2), (-3.0, 2.2), (3.0, 2.2)] {
        block(r, meshes, &k.concrete, [x, h / 2.0, z], [0.7, h, 0.7]);
        solid.push(Shape::rect_centered(ox + x, oz + z, 1.0, 1.0));
    }
    // The blast door out: a great cog set in the south wall with a yellow ring.
    let (dx, dz) = (0.0, hd - 0.1);
    let door = meshgen::gear(2.3, 0.3, 10, 0.4, 2.0);
    r.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&door))),
        MeshMaterial3d(k.metal.clone()),
        Transform::from_xyz(dx, 2.2, dz - 0.15).with_rotation(Quat::from_rotation_y(PI)),
    ));
    r.spawn((Mesh3d(meshes.add(Torus::new(2.65, 2.85))), MeshMaterial3d(k.yellow.clone()), Transform::from_xyz(dx, 2.2, dz - 0.3).with_rotation(Quat::from_rotation_x(FRAC_PI_2))));
    for n in 0..12 {
        let a = n as f32 / 12.0 * TAU;
        cylinder(r, meshes, &k.yellow, [a.cos() * 2.0, 2.2 + a.sin() * 2.0, dz - 0.4], 0.07, 0.08);
    }
    light(r, Color::srgb(1.0, 0.85, 0.5), 120_000.0, 7.0, [0.0, 1.0, hd - 1.4]);
    // The Overseer's desk and terminal, against the north wall.
    block(r, meshes, &k.metal, [0.0, 0.45, -hd + 1.0], [3.4, 0.9, 1.0]);
    block(r, meshes, &k.dark, [0.0, 0.92, -hd + 1.0], [3.5, 0.06, 1.1]);
    block(r, meshes, &k.black, [0.0, 1.25, -hd + 0.85], [0.8, 0.6, 0.6]);
    r.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.64, 0.46, 0.02))),
        MeshMaterial3d(k.screen.clone()),
        Transform::from_xyz(0.0, 1.27, -hd + 1.16).with_rotation(Quat::from_rotation_x(-0.12)),
        NotShadowCaster,
    ));
    light(r, Color::srgb(1.0, 0.65, 0.3), 90_000.0, 6.0, [0.0, 1.5, -hd + 2.0]);
    solid.push(Shape::rect_centered(ox, oz - hd + 1.0, 3.6, 1.2));
    r.spawn((Transform::from_xyz(0.0, 1.0, -hd + 1.9), Fixture { kind: FixtureKind::Talk(Npc::Overseer), space: Some(room) }));
    // An office chair, two benches along the east wall.
    block(r, meshes, &k.dark, [0.0, 0.5, -hd + 2.2], [0.5, 0.08, 0.5]);
    block(r, meshes, &k.dark, [0.0, 0.85, -hd + 2.42], [0.5, 0.6, 0.07]);
    for z in [-1.5f32, 1.5] {
        block(r, meshes, &k.metal, [hw - 0.6, 0.45, z], [0.5, 0.08, 1.8]);
        block(r, meshes, &k.metal, [hw - 0.4, 0.8, z], [0.08, 0.7, 1.8]);
        solid.push(Shape::rect_centered(ox + hw - 0.6, oz + z, 0.7, 2.0));
    }
    // The vault's cog on the west wall.
    let cog = meshgen::gear(1.5, 0.22, 10, 0.25, 2.0);
    r.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&cog))),
        MeshMaterial3d(k.metal.clone()),
        Transform::from_xyz(-hw + 0.2, 2.4, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
    ));
    for n in 0..10 {
        let a = n as f32 / 10.0 * TAU;
        cylinder_x(r, meshes, &k.yellow, [-hw + 0.38, 2.4 + a.sin() * 1.0, a.cos() * 1.0]);
    }
    // Stacked supply crates in the south-west corner.
    for (n, (x, y, z)) in [(-hw + 1.0, 0.0f32, hd - 1.2), (-hw + 2.2, 0.0, hd - 1.2), (-hw + 1.1, 0.8, hd - 1.2)].into_iter().enumerate() {
        r.spawn((SceneRoot(assets.crate_military.clone()), Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(0.2 * n as f32)).with_scale(Vec3::splat(1.3))));
    }
    solid.push(Shape::rect_centered(ox - hw + 1.6, oz + hd - 1.2, 3.0, 1.2));
    let _ = materials;
}

/// A short yellow bolt sticking out of a wall facing +x.
fn cylinder_x(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, material: &Handle<StandardMaterial>, at: [f32; 3]) {
    r.spawn((Mesh3d(meshes.add(Cylinder::new(0.07, 0.1))), MeshMaterial3d(material.clone()), Transform::from_xyz(at[0], at[1], at[2]).with_rotation(Quat::from_rotation_z(FRAC_PI_2))));
}

fn vault_extras(commands: &mut Commands, assets: &GameAssets, containers: &ContainerAssets, rng: &mut RngRes, solid: &mut Vec<Shape>) {
    let room = Interior::VaultLobby;
    let (hw, hd) = room.half();
    let (x, z) = room.at(hw - 1.5, -hd + 1.0);
    let cache = loot::roll_cache(&mut rng.0);
    spawn_container(commands, containers, solid, assets.crate_military.clone(), Vec3::new(x, interiors::FLOOR_Y + 0.05, z), PI, 1.1, "vault locker", cache);
}

// ---- The Bullseye-Mart stockroom ----

fn mart(r: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, k: &Kit, assets: &GameAssets, solid: &mut Vec<Shape>) {
    let room = Interior::Mart;
    let (ox, oz) = room.origin();
    let (hw, hd) = room.half();
    let h = room.height();
    shell(r, meshes, solid, room, &k.concrete, &k.concrete, &k.concrete);
    // Exposed girders overhead.
    for z in [-5.0f32, -1.7, 1.6, 4.9] {
        block(r, meshes, &k.rust, [0.0, h - 0.2, z], [hw * 2.0, 0.3, 0.3]);
    }
    // A skylight punched through the roof: cold light, and the snow that fell through.
    r.spawn((
        SpotLight {
            color: Color::srgb(0.7, 0.82, 1.0),
            intensity: 3_000_000.0,
            range: 14.0,
            outer_angle: 0.7,
            inner_angle: 0.4,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.0, h - 0.1, -1.5).looking_to(Vec3::NEG_Y, Vec3::Z),
    ));
    glowing(r, meshes, &k.cold_light, [2.0, h - 0.02, -1.5], [2.6, 0.03, 2.6]);
    r.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::blob(1.6, 0.35, 0.25, 77, 1.5)))),
        MeshMaterial3d(assets.snow.clone()),
        Transform::from_xyz(2.0, 0.1, -1.5),
    ));
    // Metal shelving in three aisles; the middle one has come down.
    for (n, z) in [-5.2f32, -2.4, 0.4].into_iter().enumerate() {
        for x in [-6.4f32, -3.9, -1.4, 4.4, 6.9] {
            if n == 1 && (x > 3.0) {
                continue;
            }
            let fallen = n == 1 && x < -3.0;
            let (y, tilt) = if fallen { (0.55, 1.1) } else { (0.0, 0.0) };
            r.spawn((SceneRoot(assets.rack.clone()), Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_z(tilt))));
            solid.push(Shape::rect_centered(ox + x, oz + z, 2.2, 0.9));
        }
    }
    // Checkout counters by the loading dock, and boxes on the floor.
    for x in [-5.0f32, -2.5] {
        block(r, meshes, &k.dark, [x, 0.5, hd - 2.4], [1.9, 1.0, 0.8]);
        block(r, meshes, &k.black, [x - 0.4, 1.12, hd - 2.4], [0.4, 0.25, 0.4]);
        solid.push(Shape::rect_centered(ox + x, oz + hd - 2.4, 2.0, 0.9));
    }
    for (n, (x, z)) in [(6.4f32, 3.4f32), (7.4, 2.4), (-7.6, 3.6), (4.2, 4.6), (-0.5, 5.0)].into_iter().enumerate() {
        let scene = [&assets.crate_wood, &assets.ammo_box, &assets.jerrycan, &assets.food_cans, &assets.can][n % 5];
        r.spawn((SceneRoot(scene.clone()), Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(n as f32)).with_scale(Vec3::splat(1.2))));
        solid.push(Shape::Circle { x: ox + x, z: oz + z, r: 0.5 });
    }
    // The loading dock's roller door.
    let (dx, dz) = (0.0f32, hd - 0.1);
    block(r, meshes, &k.rust, [dx, 1.4, dz - 0.05], [3.2, 2.8, 0.1]);
    for n in 0..7 {
        block(r, meshes, &k.dark, [dx, 0.2 + n as f32 * 0.4, dz - 0.12], [3.2, 0.05, 0.06]);
    }
    glowing(r, meshes, &k.cold_light, [dx, 0.015, dz - 0.14], [3.1, 0.03, 0.03]);
    // Strip lights, some of them dying.
    for (n, (x, z)) in [(-6.0f32, -4.0f32), (-1.5, -4.0), (5.5, -4.0), (-6.0, 2.5), (-1.0, 2.5), (6.0, 2.5)].into_iter().enumerate() {
        glowing(r, meshes, &k.strip, [x, h - 0.06, z], [0.2, 0.05, 1.8]);
        flicker(r, Color::srgb(0.75, 0.85, 1.0), if n % 2 == 0 { 200_000.0 } else { 120_000.0 }, 9.5, [x, h - 0.6, z], n as f32 * 1.7);
    }
}

fn mart_extras(commands: &mut Commands, assets: &GameAssets, containers: &ContainerAssets, rng: &mut RngRes, solid: &mut Vec<Shape>) {
    let room = Interior::Mart;
    let (hw, hd) = room.half();
    // The shotgun, which used to sit out in the ruin.
    let (x, z) = room.at(hw - 2.0, -hd + 0.9);
    spawn_container(
        commands,
        containers,
        solid,
        assets.crate_military.clone(),
        Vec3::new(x, interiors::FLOOR_Y + 0.1, z),
        0.4,
        1.3,
        "military footlocker",
        loot::weapon_cache(WeaponKind::ScrapShotgun),
    );
    for (lx, lz, name) in [(-hw + 1.2, -hd + 1.0, "supply crate"), (-1.0, -hd + 1.0, "stock cage"), (hw - 1.5, hd - 3.0, "supply crate")] {
        let (x, z) = room.at(lx, lz);
        let cache = loot::roll_cache(&mut rng.0);
        spawn_container(commands, containers, solid, assets.crate_wood.clone(), Vec3::new(x, interiors::FLOOR_Y + 0.05, z), lx, 1.2, name, cache);
    }
}

// ---------------------------------------------------------------------------
// Using things, and moving between rooms
// ---------------------------------------------------------------------------

/// Show what the nearest bunk, stove, terminal or door offers, and use it on E.
#[allow(clippy::too_many_arguments)]
fn use_fixtures(
    controls: crate::keybind::Controls,
    current: Res<CurrentInterior>,
    mut moving: ResMut<Moving>,
    mut transition: ResMut<Transition>,
    mut claim: ResMut<FixtureClaim>,
    mut prompt: ResMut<Prompt>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    flags: Res<StoryFlags>,
    perks: Res<Perks>,
    mut talks: EventWriter<StartTalk>,
    mut spots: EventWriter<UseSpot>,
    player: Query<&Transform, With<Player>>,
    fixtures: Query<(&GlobalTransform, &Fixture)>,
) {
    claim.0 = false;
    if moving.0.is_some() {
        return;
    }
    let Ok(p) = player.single() else { return };
    let here = current.0;
    let nearest = fixtures
        .iter()
        .filter(|(_, f)| f.space == here)
        .map(|(gt, f)| (((gt.translation().x - p.translation.x).powi(2) + (gt.translation().z - p.translation.z).powi(2)).sqrt(), f))
        .filter(|(d, _)| *d < REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, fixture)) = nearest else { return };
    claim.0 = true;
    let line = match fixture.kind {
        FixtureKind::Door(Door::Enter(room)) => room.enter_prompt(),
        FixtureKind::Door(Door::Leave(room)) => format!("[E] Leave {}", room.name()),
        FixtureKind::Bunk => "[E] Sleep until morning (saves your game)".to_string(),
        FixtureKind::Stove => format!("[E] Heat a hotdish on the stove ({} left)", game.inv.hotdish),
        FixtureKind::Talk(Npc::Overseer) => match quest::stage(&flags.0) {
            Stage::Unstarted => "[E] Play the Overseer's recording",
            Stage::Active => "[E] Check the Overseer's log",
            Stage::ReadyToReport => "[E] Report to the Overseer",
            Stage::Complete => "[E] Use the Overseer's terminal",
        }
        .to_string(),
        FixtureKind::Talk(npc) => format!("[E] Talk to {}", npc.name()),
        FixtureKind::Spot(spot) => spot.prompt(&flags.0, game.inv.scrap),
    };
    prompt.0.push(line);
    if !controls.just_pressed(Bind::Interact) {
        return;
    }
    match fixture.kind {
        FixtureKind::Door(Door::Enter(room)) => start(&mut moving, &mut transition, &mut sfx, Trip::Enter(room), 0.0),
        FixtureKind::Door(Door::Leave(room)) => start(&mut moving, &mut transition, &mut sfx, Trip::Leave(room), 0.0),
        FixtureKind::Bunk => {
            if let Some(room) = here {
                start(&mut moving, &mut transition, &mut sfx, Trip::Sleep(room), 1.4);
            }
        }
        FixtureKind::Stove => {
            let Game { inv, survival, .. } = &mut *game;
            let before = survival.health;
            let result = interiors::cook_hotdish(inv, survival);
            if result.used() && perks.0.aid_heal > 1.0 {
                survival.heal((survival.health - before) * (perks.0.aid_heal - 1.0));
            }
            msgs.show(result.message(), 3.0);
            sfx.play(if result.used() { Sound::PickupFood } else { Sound::DryClick });
        }
        FixtureKind::Talk(npc) => {
            talks.write(StartTalk(npc));
        }
        FixtureKind::Spot(spot) => {
            spots.write(UseSpot(spot));
        }
    }
}

fn start(moving: &mut Moving, transition: &mut Transition, sfx: &mut SfxQueue, trip: Trip, hold: f32) {
    moving.0 = Some((Fade::new(hold), trip));
    transition.active = true;
    sfx.play(if matches!(trip, Trip::Sleep(_)) { Sound::PipOff } else { Sound::ContainerOpen });
}

/// Run the fade and, when the screen is black, make the move.
#[allow(clippy::too_many_arguments)]
fn run_trips(
    time: Res<Time<Real>>,
    mut moving: ResMut<Moving>,
    mut transition: ResMut<Transition>,
    mut current: ResMut<CurrentInterior>,
    mut game: ResMut<Game>,
    mut clock: ResMut<ClockRes>,
    mut weather: ResMut<WeatherRes>,
    mut msgs: ResMut<Messages>,
    mut autosave: EventWriter<AutosaveRequest>,
    mut player: Query<(&mut Transform, &mut Player)>,
    mut overlay: Query<&mut BackgroundColor, With<FadeOverlay>>,
    mut text: Query<(&mut Text, &mut TextColor), With<FadeText>>,
) {
    let Some((fade, trip)) = moving.0.as_mut() else {
        // Nothing under way: make sure the screen is clear.
        if let Ok(mut bg) = overlay.single_mut() {
            bg.set_if_neq(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)));
        }
        if let Ok((mut t, _)) = text.single_mut() {
            t.set_if_neq(Text::new(""));
        }
        return;
    };
    let trip = *trip;
    let step = fade.step(time.delta_secs().min(0.1));
    if let Ok(mut bg) = overlay.single_mut() {
        bg.set_if_neq(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, step.alpha)));
    }
    let mut report = None;
    if step.switch_now {
        let (x, z, yaw, on_map) = match trip {
            Trip::Enter(room) | Trip::Sleep(room) => {
                let (x, z, yaw) = room.entry();
                (x, z, yaw, false)
            }
            Trip::Leave(room) => {
                let (x, z, yaw) = room.outside();
                (x, z, yaw, true)
            }
        };
        match trip {
            Trip::Enter(room) => current.0 = Some(room),
            Trip::Leave(_) => current.0 = None,
            Trip::Sleep(_) => {
                let r = interiors::sleep(&mut clock.0, &mut game.survival, &mut weather.weather);
                weather.just_changed = None;
                report = Some(format!("You sleep for {:.0} hours.\nDay {}  {}\n\nHealth restored: +{:.0}", r.hours, clock.0.day, clock.0.label(), r.healed));
            }
        }
        if let Ok((mut tf, mut p)) = player.single_mut() {
            // Sleeping keeps you where you lay down: only a door moves you.
            if !matches!(trip, Trip::Sleep(_)) {
                p.yaw = yaw;
                p.pitch = 0.0;
                p.vel_y = 0.0;
                p.grounded = true;
                let floor = if on_map { terrain::walk_height(x, z) } else { interiors::FLOOR_Y };
                tf.translation = Vec3::new(x, floor + EYE_HEIGHT, z);
                tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
            }
        }
        info!("trip: {trip:?}");
    }
    if let Ok((mut t, mut c)) = text.single_mut() {
        if let Some(r) = report {
            t.set_if_neq(Text::new(r));
        }
        c.set_if_neq(TextColor(crate::theme::FROST.with_alpha(step.alpha)));
    }
    if step.done {
        moving.0 = None;
        transition.active = false;
        match trip {
            Trip::Enter(Interior::FishHouse(_)) => {
                autosave.write(AutosaveRequest);
            }
            Trip::Sleep(_) => {
                msgs.show("You wake rested.", 3.0);
                autosave.write(AutosaveRequest);
            }
            _ => {}
        }
    }
}

/// Only the room you're in is drawn.
fn show_current_room(current: Res<CurrentInterior>, mut roots: Query<(&InteriorRoot, &mut Visibility)>) {
    for (root, mut vis) in &mut roots {
        let want = if current.0 == Some(root.0) { Visibility::Inherited } else { Visibility::Hidden };
        vis.set_if_neq(want);
    }
}

fn build_fade_overlay(mut commands: Commands, assets: Res<GameAssets>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            GlobalZIndex(140),
            FadeOverlay,
        ))
        .with_children(|o| {
            o.spawn((
                Text::new(""),
                TextFont { font: assets.font.clone(), font_size: 30.0, ..default() },
                TextColor(crate::theme::FROST.with_alpha(0.0)),
                TextLayout::new_with_justify(JustifyText::Center),
                FadeText,
            ));
        });
}
