//! Outdoor set dressing from the CC0 library: glacial boulders in the
//! fields, fallen logs, stumps and deadfall in the woods, rock outcrops
//! flanking the vault portal, propane tanks and generators at the shelters
//! and jersey barriers round the raider camps. Placed with its own seeded
//! random numbers, after the rest of the world, so adding it moved nothing
//! that was already there (saves key loot by position).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::library::{Library, Tint};
use crate::meshes::to_mesh_tangents;
use crate::perf::Lod;
use crate::sim::collision::{self, Shape};
use crate::sim::meshgen;
use crate::sim::rng::Rng;
use crate::sim::terrain::{self, SHELTERS, VAULT_POS};
use crate::state::{Colliders, RngRes, TreePositions};
use crate::world::{ground, open_spot, spawn_contact_shadow, SCATTER};

const SEED: u64 = 0x5EED_0010;
/// Pulls the warm scanned stone towards the map's grey granite.
const GRANITE: [f32; 3] = [0.62, 0.66, 0.74];
/// Weathered grey concrete.
const CONCRETE: [f32; 3] = [0.8, 0.84, 0.9];
const BOULDERS: usize = 14;
const LOGS: usize = 12;
const STUMPS: usize = 10;
const DEADFALL: usize = 14;

/// The raider camps (x, z), from `raiders.rs`.
const CAMPS: [(f32, f32); 2] = [(-110.0, 15.0), (160.0, 55.0)];

pub struct DressingPlugin;

impl Plugin for DressingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, dress_world.after(crate::state::WorldGen));
    }
}

fn place(commands: &mut Commands, lib: &Library, id: &str, pos: Vec3, rot: Quat, scale: f32, lod: bool) -> Entity {
    let mut e = commands.spawn((lib.scene(id), Transform::from_translation(pos).with_rotation(rot).with_scale(Vec3::splat(scale))));
    if lod {
        e.insert(Lod::Prop);
    }
    e.id()
}

/// A random spot within `r` of (x, z) on open ground clear of everything.
fn near(rng: &mut RngRes, solid: &[Shape], x: f32, z: f32, r0: f32, r1: f32, clearance: f32) -> Option<(f32, f32)> {
    for _ in 0..20 {
        let a = rng.0.range(0.0, TAU);
        let d = rng.0.range(r0, r1);
        let (px, pz) = (x + a.cos() * d, z + a.sin() * d);
        if terrain::is_open_ground(px, pz) && !collision::blocked(px, pz, clearance, solid) {
            return Some((px, pz));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn dress_world(
    mut commands: Commands,
    lib: Res<Library>,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut colliders: ResMut<Colliders>,
    trees: Res<TreePositions>,
) {
    let solid = &mut colliders.0;
    let mut rng = RngRes(Rng::new(SEED));
    let cap = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.3, 0.25, 77, 1.0)));

    // ---- Glacial boulders out in the open, dusted with snow ----
    for _ in 0..BOULDERS {
        let Some((x, z)) = open_spot(&mut rng, solid, 3.0, &SCATTER) else { continue };
        let s = rng.0.range(1.4, 3.0);
        let yaw = rng.0.range(0.0, TAU);
        let gy = ground(x, z);
        debug!("boulder at {x:.0},{z:.0}");
        // Cold grey granite rather than the scan's warm sandstone.
        let b = place(&mut commands, &lib, "boulder_01", Vec3::new(x, gy - 0.18 * s, z), Quat::from_rotation_y(yaw), s, false);
        commands.entity(b).insert(Tint(GRANITE));
        commands.spawn((
            Mesh3d(cap.clone()),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, gy + 0.6 * s, z).with_rotation(Quat::from_rotation_y(yaw)).with_scale(Vec3::new(0.42 * s, 0.16 * s, 0.62 * s)),
        ));
        solid.push(Shape::Circle { x, z, r: 0.75 * s });
        spawn_contact_shadow(&mut commands, &mut meshes, &assets, x, z, 0.9 * s, 1.2 * s, yaw);
    }

    // ---- The woods: fallen logs, stumps and deadfall among the trees ----
    let woods: Vec<(f32, f32)> = trees.0.clone();
    let pick_tree = |rng: &mut RngRes| (!woods.is_empty()).then(|| woods[(rng.0.f32() * woods.len() as f32) as usize % woods.len()]);
    for _ in 0..LOGS {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, tx, tz, 2.0, 4.0, 1.6) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        let s = rng.0.range(1.0, 1.4);
        // Half sunk in the snow, a little tilted to follow the ground.
        let tilt = rng.0.range(-0.06, 0.06);
        let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(tilt);
        debug!("log at {x:.0},{z:.0}");
        place(&mut commands, &lib, "dead_tree_trunk", Vec3::new(x, ground(x, z) - 0.06 * s, z), rot, s, true);
        commands.spawn((
            Mesh3d(cap.clone()),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, ground(x, z) + 0.2 * s, z).with_rotation(rot).with_scale(Vec3::new(1.35 * s, 0.07 * s, 0.11 * s)),
            Lod::Prop,
        ));
        // The log is a line of small circles to walk round.
        let dir = Vec2::new(yaw.cos(), -yaw.sin());
        for t in [-1.1f32, 0.0, 1.1] {
            solid.push(Shape::Circle { x: x + dir.x * t * s, z: z + dir.y * t * s, r: 0.3 });
        }
    }
    for _ in 0..STUMPS {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, tx, tz, 1.5, 3.5, 1.0) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        place(&mut commands, &lib, "tree_stump_02", Vec3::new(x, ground(x, z) + 0.12, z), Quat::from_rotation_y(yaw), rng.0.range(0.7, 1.0), true);
        solid.push(Shape::Circle { x, z, r: 0.5 });
    }
    for _ in 0..DEADFALL {
        let Some((tx, tz)) = pick_tree(&mut rng) else { break };
        let Some((x, z)) = near(&mut rng, solid, tx, tz, 1.0, 3.0, 0.8) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        place(&mut commands, &lib, "dry_branches_medium_01", Vec3::new(x, ground(x, z) - 0.04, z), Quat::from_rotation_y(yaw), rng.0.range(0.9, 1.4), true);
    }

    // ---- Rock outcrops either side of the vault portal ----
    let (vx, vz) = VAULT_POS;
    for (dx, dz, yaw, s) in [(-15.5f32, -3.5f32, PI + 0.4, 2.6f32), (15.5, -3.5, PI - 0.4, 2.4), (-23.0, 2.0, PI + 0.9, 2.2), (23.5, 2.5, PI - 1.0, 2.3)] {
        let (x, z) = (vx + dx, vz + dz);
        let r = place(&mut commands, &lib, "rock_face_02", Vec3::new(x, ground(x, z) - 0.3, z), Quat::from_rotation_y(yaw), s, false);
        commands.entity(r).insert(Tint(GRANITE));
        solid.push(Shape::Circle { x, z, r: 1.6 * s });
    }

    // ---- The shelters: a propane tank and a generator by each ----
    for (i, &(sx, sz)) in SHELTERS.iter().enumerate() {
        let Some((x, z)) = near(&mut rng, solid, sx, sz, 3.5, 5.5, 0.8) else { continue };
        let yaw = rng.0.range(0.0, TAU);
        place(&mut commands, &lib, "propane_tank", Vec3::new(x, ground(x, z), z), Quat::from_rotation_y(yaw), 1.3, true);
        if let Some((gx, gz)) = near(&mut rng, solid, x, z, 0.7, 1.2, 0.5) {
            place(&mut commands, &lib, "propane_tank", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(yaw + 1.0), 1.3, true);
        }
        solid.push(Shape::Circle { x, z, r: 0.5 });
        if i % 2 == 0 {
            if let Some((gx, gz)) = near(&mut rng, solid, sx, sz, 3.5, 5.5, 1.0) {
                place(&mut commands, &lib, "portable_generator", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(rng.0.range(0.0, TAU)), 1.1, true);
                solid.push(Shape::Circle { x: gx, z: gz, r: 0.6 });
            }
        }
    }

    // ---- The raider camps: a ring of jersey barriers and a generator ----
    for &(cx, cz) in &CAMPS {
        for k in 0..5 {
            let a = k as f32 / 5.0 * TAU + rng.0.range(-0.2, 0.2);
            let d = rng.0.range(9.0, 11.0);
            let (x, z) = (cx + a.cos() * d, cz + a.sin() * d);
            if !terrain::is_open_ground(x, z) || collision::blocked(x, z, 1.2, solid) {
                continue;
            }
            // Side-on to the camp, as cover.
            let yaw = -a + PI / 2.0 + rng.0.range(-0.25, 0.25);
            let b = place(&mut commands, &lib, "concrete_road_barrier", Vec3::new(x, ground(x, z) - 0.03, z), Quat::from_rotation_y(yaw), 1.4, false);
            commands.entity(b).insert(Tint(CONCRETE));
            let along = Vec2::new(yaw.cos(), -yaw.sin()) * 0.6;
            for t in [-1.0f32, 1.0] {
                solid.push(Shape::Circle { x: x + along.x * t, z: z + along.y * t, r: 0.55 });
            }
        }
        if let Some((gx, gz)) = near(&mut rng, solid, cx, cz, 4.0, 6.0, 1.0) {
            place(&mut commands, &lib, "portable_generator", Vec3::new(gx, ground(gx, gz), gz), Quat::from_rotation_y(rng.0.range(0.0, TAU)), 1.1, true);
            solid.push(Shape::Circle { x: gx, z: gz, r: 0.6 });
        }
    }
}
