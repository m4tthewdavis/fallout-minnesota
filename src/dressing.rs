//! Outdoor set dressing from the CC0 library: glacial boulders in the
//! fields, fallen logs, stumps and deadfall in the woods, rock outcrops
//! flanking the vault portal, propane tanks and generators at the shelters
//! and jersey barriers round the raider camps. Placed with its own seeded
//! random numbers, after the rest of the world, so adding it moved nothing
//! that was already there (saves key loot by position).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::interiors::Fixture;
use crate::library::{Library, Weathered};
use crate::perf::Lod;
use crate::sim::collision::{self, Shape};
use crate::sim::rng::Rng;
use crate::sim::terrain::{self, SHELTERS, VAULT_POS};
use crate::state::{Colliders, RngRes, TreePositions};
use crate::landmarks::RUINS;
use crate::sim::weather::WIND_DIR;
use crate::world::{ground, open_spot, spawn_contact_shadow, spawn_drift, Pickup, SCATTER};

const SEED: u64 = 0x5EED_0010;
/// Cold grey granite (on a colour map already drained grey, see
/// `library::GREYED`), with snow on its top.
pub const GRANITE: Weathered = Weathered { tint: [0.58, 0.62, 0.68], snow: 0.45 };
/// The vault's outcrops: the same stone, more snow on their ledges.
const OUTCROP: Weathered = Weathered { tint: [0.6, 0.64, 0.7], snow: 0.6 };
/// Weathered grey concrete, a line of snow along the top.
const CONCRETE: Weathered = Weathered { tint: [0.9, 0.93, 0.98], snow: 0.4 };
/// Bark and cut wood keep their colour; snow lies along logs and on stumps.
const SNOWY_WOOD: Weathered = Weathered { tint: [1.0, 1.0, 1.0], snow: 0.5 };
/// Keep this far from pickups and quest fixtures (they have no colliders).
const FIXTURE_CLEAR: f32 = 2.0;
const BOULDERS: usize = 14;
const LOGS: usize = 12;
const STUMPS: usize = 10;
const DEADFALL: usize = 14;

pub struct DressingPlugin;

impl Plugin for DressingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, dress_world.after(crate::state::WorldGen).after(crate::quest::QuestWorld));
    }
}

fn place(commands: &mut Commands, lib: &Library, id: &str, pos: Vec3, rot: Quat, scale: f32, lod: bool) -> Entity {
    let mut e = commands.spawn((lib.scene(id), Transform::from_translation(pos).with_rotation(rot).with_scale(Vec3::splat(scale))));
    if lod {
        e.insert(Lod::Prop);
    }
    e.id()
}

/// A random spot within `r0..r1` of (x, z) where `ok` allows it and clear of
/// everything solid and of `keep_clear` (pickups and quest fixtures).
#[allow(clippy::too_many_arguments)]
fn near(rng: &mut RngRes, solid: &[Shape], keep_clear: &[Vec2], x: f32, z: f32, r0: f32, r1: f32, clearance: f32, ok: impl Fn(f32, f32) -> bool) -> Option<(f32, f32)> {
    for _ in 0..20 {
        let a = rng.0.range(0.0, TAU);
        let d = rng.0.range(r0, r1);
        let (px, pz) = (x + a.cos() * d, z + a.sin() * d);
        let by_fixture = keep_clear.iter().any(|p| p.distance(Vec2::new(px, pz)) < FIXTURE_CLEAR + clearance);
        if ok(px, pz) && !by_fixture && !collision::blocked(px, pz, clearance, solid) {
            return Some((px, pz));
        }
    }
    None
}

/// Out in the woods: open ground, not in a ruin.
fn wild(x: f32, z: f32) -> bool {
    terrain::is_open_ground(x, z) && !RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r + 2.0)
}

/// Beside a building: anywhere off the lakes and the road (open-ground rules
/// keep 10 m clear round the shelters, which is where these belong).
fn yard(x: f32, z: f32) -> bool {
    terrain::lake_at(x, z).is_none() && terrain::road_distance(x, z) > terrain::ROAD_HALF_WIDTH + 1.0
}

#[allow(clippy::too_many_arguments)]
fn dress_world(
    mut commands: Commands,
    lib: Res<Library>,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut colliders: ResMut<Colliders>,
    trees: Res<TreePositions>,
    fixtures: Query<(&Transform, Option<&Fixture>), Or<(With<Pickup>, With<Fixture>)>>,
) {
    let solid = &mut colliders.0;
    let mut rng = RngRes(Rng::new(SEED));
    // Outdoor ones only: a room's fixtures sit in its own local space.
    let keep_clear: Vec<Vec2> = fixtures.iter().filter(|(_, f)| f.is_none_or(|f| f.space.is_none())).map(|(t, _)| t.translation.xz()).collect();
    let kc = &keep_clear[..];

    // ---- Glacial boulders out in the open, snow lying on their tops ----
    for _ in 0..BOULDERS {
        let Some((x, z)) = open_spot(&mut rng, solid, 3.0, &SCATTER) else { continue };
        if kc.iter().any(|p| p.distance(Vec2::new(x, z)) < FIXTURE_CLEAR + 3.0) {
            continue;
        }
        let s = rng.0.range(1.4, 3.0);
        let yaw = rng.0.range(0.0, TAU);
        let gy = ground(x, z);
        // Sunk well into the snow, as if it had been there all winter.
        let b = place(&mut commands, &lib, "boulder_01", Vec3::new(x, gy - 0.22 * s, z), Quat::from_rotation_y(yaw), s, false);
        commands.entity(b).insert((GRANITE, Lod::Large));
        solid.push(Shape::Circle { x, z, r: 0.75 * s });
        spawn_contact_shadow(&mut commands, &mut meshes, &assets, x, z, 0.9 * s, 1.2 * s, yaw);
        // Snow banked up on its lee side.
        let d = 0.55 * s;
        spawn_drift(&mut commands, &mut meshes, &assets, x + WIND_DIR[0] * d, z + WIND_DIR[1] * d, 1.8 * s, 1.0 * s, 0.22 * s, WIND_DIR, 700 + (x.abs() * 7.0 + z.abs()) as u64);
    }

    // ---- The woods: fallen logs, stumps and deadfall among the trees ----
    let woods: Vec<(f32, f32)> = trees.0.clone();
    let pick_tree = |rng: &mut RngRes| (!woods.is_empty()).then(|| woods[(rng.0.f32() * woods.len() as f32) as usize % woods.len()]);
    for _ in 0..LOGS {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, kc, tx, tz, 2.0, 4.0, 1.6, wild) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        let s = rng.0.range(1.0, 1.4);
        // Half sunk in the snow, a little tilted to follow the ground.
        let tilt = rng.0.range(-0.06, 0.06);
        let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(tilt);
        let log = place(&mut commands, &lib, "dead_tree_trunk", Vec3::new(x, ground(x, z) - 0.06 * s, z), rot, s, true);
        commands.entity(log).insert(SNOWY_WOOD);
        // The log is a line of small circles to walk round.
        let dir = Vec2::new(yaw.cos(), -yaw.sin());
        for t in [-1.1f32, 0.0, 1.1] {
            solid.push(Shape::Circle { x: x + dir.x * t * s, z: z + dir.y * t * s, r: 0.3 });
        }
    }
    for _ in 0..STUMPS {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, kc, tx, tz, 1.5, 3.5, 1.0, wild) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        let stump = place(&mut commands, &lib, "tree_stump_02", Vec3::new(x, ground(x, z) + 0.12, z), Quat::from_rotation_y(yaw), rng.0.range(0.7, 1.0), true);
        commands.entity(stump).insert(SNOWY_WOOD);
        solid.push(Shape::Circle { x, z, r: 0.5 });
    }
    for _ in 0..DEADFALL {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, kc, tx, tz, 1.0, 3.0, 0.8, wild) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        place(&mut commands, &lib, "dry_branches_medium_01", Vec3::new(x, ground(x, z) - 0.04, z), Quat::from_rotation_y(yaw), rng.0.range(0.9, 1.4), true);
    }

    // ---- Rock outcrops either side of the vault portal ----
    let (vx, vz) = VAULT_POS;
    for (dx, dz, yaw, s) in [(-15.5f32, -3.5f32, PI + 0.4, 2.6f32), (15.5, -3.5, PI - 0.4, 2.4), (-23.0, 2.0, PI + 0.9, 2.2), (23.5, 2.5, PI - 1.0, 2.3)] {
        let (x, z) = (vx + dx, vz + dz);
        let r = place(&mut commands, &lib, "rock_face_02", Vec3::new(x, ground(x, z) - 0.3, z), Quat::from_rotation_y(yaw), s, false);
        commands.entity(r).insert((OUTCROP, Lod::Large));
        solid.push(Shape::Circle { x, z, r: 1.6 * s });
    }

    // ---- The shelters: a pair of propane tanks and a generator by each ----
    for (i, &(sx, sz)) in SHELTERS.iter().enumerate() {
        let Some((x, z)) = near(&mut rng, solid, kc, sx, sz, 3.5, 5.5, 0.8, yard) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        place(&mut commands, &lib, "propane_tank", Vec3::new(x, ground(x, z), z), Quat::from_rotation_y(yaw), 1.3, true);
        solid.push(Shape::Circle { x, z, r: 0.5 });
        if let Some((gx, gz)) = near(&mut rng, solid, kc, x, z, 0.9, 1.3, 0.4, yard) {
            place(&mut commands, &lib, "propane_tank", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(yaw + 1.0), 1.3, true);
            solid.push(Shape::Circle { x: gx, z: gz, r: 0.5 });
        }
        if i % 2 == 0 {
            if let Some((gx, gz)) = near(&mut rng, solid, kc, sx, sz, 3.5, 5.5, 1.0, yard) {
                place(&mut commands, &lib, "portable_generator", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(rng.0.range(0.0, TAU)), 1.1, true);
                solid.push(Shape::Circle { x: gx, z: gz, r: 0.6 });
            }
        }
    }

    // ---- The raider camps: a ring of jersey barriers and a generator ----
    for &(cx, cz, _) in &crate::raiders::CAMPS {
        for k in 0..5 {
            let a = k as f32 / 5.0 * TAU + rng.0.range(-0.2, 0.2);
            let d = rng.0.range(9.0, 11.0);
            let (x, z) = (cx + a.cos() * d, cz + a.sin() * d);
            let by_fixture = kc.iter().any(|p| p.distance(Vec2::new(x, z)) < FIXTURE_CLEAR + 1.2);
            if !terrain::is_open_ground(x, z) || by_fixture || collision::blocked(x, z, 1.2, solid) {
                continue;
            }
            // Side-on to the camp, as cover.
            let yaw = -a + PI / 2.0 + rng.0.range(-0.25, 0.25);
            let b = place(&mut commands, &lib, "concrete_road_barrier", Vec3::new(x, ground(x, z) - 0.03, z), Quat::from_rotation_y(yaw), 1.4, true);
            commands.entity(b).insert(CONCRETE);
            let along = Vec2::new(yaw.cos(), -yaw.sin()) * 0.6;
            for t in [-1.0f32, 1.0] {
                solid.push(Shape::Circle { x: x + along.x * t, z: z + along.y * t, r: 0.55 });
            }
        }
        if let Some((gx, gz)) = near(&mut rng, solid, kc, cx, cz, 4.0, 6.0, 1.0, terrain::is_open_ground) {
            place(&mut commands, &lib, "portable_generator", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(rng.0.range(0.0, TAU)), 1.1, true);
            solid.push(Shape::Circle { x: gx, z: gz, r: 0.6 });
        }
    }
}
