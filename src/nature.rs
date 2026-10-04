//! Scatters the Northwoods over the map: the forest and undergrowth (see
//! `flora.rs`), dead snags, boulders, snowdrifts and pre-war junk.

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::landmarks::{CARS, RUINS};
use crate::meshes::to_mesh_tangents;
use crate::sim::collision::{self, Shape};
use crate::sim::meshgen;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::state::{Colliders, RngRes};
use crate::sim::weather::WIND_DIR;
use crate::world::{ground, prop, spawn_contact_shadow, spawn_drift};

const SNAGS: usize = 24;
const ROCKS: usize = 30;
const DRIFTS: usize = 50;
const JUNK: usize = 36;

/// A random point on open ground away from the ruins and cars, if one is found.
fn open_spot(rng: &mut RngRes, solid: &[Shape], clearance: f32) -> Option<(f32, f32)> {
    for _ in 0..40 {
        let x = rng.0.range(-HALF_SIZE, HALF_SIZE);
        let z = rng.0.range(-HALF_SIZE, HALF_SIZE);
        if !terrain::is_open_ground(x, z) {
            continue;
        }
        if RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r)
            || CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < 5.0)
            || collision::blocked(x, z, clearance, solid)
        {
            continue;
        }
        return Some((x, z));
    }
    None
}

pub fn spawn_nature(
    mut commands: Commands,
    assets: Res<GameAssets>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
    mut tree_positions: ResMut<crate::state::TreePositions>,
) {
    let solid = &mut colliders.0;

    // ---------- The forest, undergrowth and lake shores ----------
    let avoid = |x: f32, z: f32| {
        RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r) || CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < 5.0)
    };
    crate::flora::plant_forest(&mut commands, &mut meshes, &mut materials, &server, &assets, solid, &mut tree_positions.0, &avoid, 143);

    // ---------- Dead snags ----------
    let snag_variants: Vec<Handle<Mesh>> = (0..3)
        .map(|seed| {
            let h = 7.0 + seed as f32;
            let mut m = meshgen::lathe(&[(0.24, -0.3), (0.18, h * 0.3), (0.08, h * 0.85), (0.02, h)], 7, 1.2, false, false);
            m.append(&meshgen::dead_branches(h, 20 + seed as u64));
            meshes.add(to_mesh_tangents(&m))
        })
        .collect();
    for k in 0..SNAGS {
        let Some((x, z)) = open_spot(&mut rng, solid, 1.0) else { continue };
        let yaw = rng.0.range(0.0, std::f32::consts::TAU);
        let tf = Transform::from_xyz(x, ground(x, z) - 0.1, z)
            .with_scale(Vec3::splat(rng.0.range(0.8, 1.3)))
            .with_rotation(Quat::from_rotation_y(yaw));
        commands.spawn((Mesh3d(snag_variants[k % 3].clone()), MeshMaterial3d(assets.bark.clone()), tf));
        solid.push(Shape::Circle { x, z, r: 0.35 });
        tree_positions.0.push((x, z));
    }

    // ---------- Boulders poking through the snow ----------
    let cap = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.3, 0.25, 31, 1.0)));
    for _ in 0..ROCKS {
        let Some((x, z)) = open_spot(&mut rng, solid, 1.5) else { continue };
        let s = rng.0.range(8.0, 16.0);
        let yaw = rng.0.range(0.0, std::f32::consts::TAU);
        let gy = ground(x, z);
        prop(&mut commands, &assets.rock, Vec3::new(x, gy - 0.04 * s, z), yaw, s);
        commands.spawn((
            Mesh3d(cap.clone()),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, gy + 0.09 * s, z)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::new(0.09 * s, 0.09 * s, 0.13 * s)),
        ));
        solid.push(Shape::Circle { x, z, r: 0.12 * s });
        spawn_contact_shadow(&mut commands, &mut meshes, &assets, x, z, 0.2 * s, 0.2 * s, yaw);
        let d = 0.12 * s;
        spawn_drift(&mut commands, &mut meshes, &assets, x + WIND_DIR[0] * d, z + WIND_DIR[1] * d, 0.4 * s, 0.22 * s, 0.05 * s, WIND_DIR, 500 + (s * 10.0) as u64);
    }

    // ---------- Snowdrifts ----------
    for i in 0..DRIFTS {
        let Some((x, z)) = open_spot(&mut rng, solid, 0.5) else { continue };
        let length = rng.0.range(4.0, 11.0);
        let width = length * rng.0.range(0.35, 0.6);
        let peak = rng.0.range(0.45, 1.2);
        // Mostly lined up with the wind, a little scattered.
        let a = WIND_DIR[1].atan2(WIND_DIR[0]) + rng.0.range(-0.35, 0.35);
        spawn_drift(&mut commands, &mut meshes, &assets, x, z, length, width, peak, [a.cos(), a.sin()], 200 + i as u64);
    }

    // ---------- Pre-war junk ----------
    // Clusters by the cars and shelters, then a sprinkle over open ground.
    let junk_at = |commands: &mut Commands, rng: &mut RngRes, solid: &mut Vec<Shape>, x: f32, z: f32| {
        if collision::blocked(x, z, 0.6, solid) || terrain::lake_at(x, z).is_some() {
            return;
        }
        let gy = ground(x, z);
        let yaw = rng.0.range(0.0, std::f32::consts::TAU);
        match (rng.0.f32() * 7.0) as u32 {
            0 => {
                prop(commands, &assets.barrel, Vec3::new(x, gy, z), yaw, 1.0);
                solid.push(Shape::Circle { x, z, r: 0.35 });
            }
            1 => {
                // Tyre lying flat, half buried.
                commands.spawn((
                    SceneRoot(assets.tyre.clone()),
                    Transform::from_xyz(x, gy + 0.05, z).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(1.5)),
                ));
            }
            2 => {
                prop(commands, &assets.jerrycan, Vec3::new(x, gy, z), yaw, 1.0);
            }
            3 => {
                prop(commands, &assets.crate_wood, Vec3::new(x, gy, z), yaw, 1.0);
                solid.push(Shape::Circle { x, z, r: 0.55 });
            }
            4 => {
                for k in 0..3 {
                    let (cx, cz) = (x + k as f32 * 0.25, z + (k as f32 * 1.7).sin() * 0.3);
                    commands.spawn((
                        SceneRoot(assets.can.clone()),
                        Transform::from_xyz(cx, gy, cz).with_rotation(Quat::from_rotation_y(yaw + k as f32) * Quat::from_rotation_x(if k == 1 { 1.5 } else { 0.0 })),
                    ));
                }
            }
            5 => {
                commands.spawn((
                    SceneRoot(assets.rim.clone()),
                    Transform::from_xyz(x, gy + 0.05, z).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(1.45)),
                ));
            }
            _ => {
                prop(commands, &assets.food_cans, Vec3::new(x, gy, z), yaw, 1.5);
            }
        }
    };
    for &(cx, cz, _) in &CARS {
        for _ in 0..2 {
            let a = rng.0.range(0.0, std::f32::consts::TAU);
            let d = rng.0.range(3.0, 5.0);
            junk_at(&mut commands, &mut rng, solid, cx + a.cos() * d, cz + a.sin() * d);
        }
    }
    for &(sx, sz) in &terrain::SHELTERS {
        let a = rng.0.range(0.0, std::f32::consts::TAU);
        junk_at(&mut commands, &mut rng, solid, sx + a.cos() * 4.0, sz + a.sin() * 4.0);
    }
    let (bx, bz, _) = RUINS[0];
    for _ in 0..6 {
        let (x, z) = (bx + rng.0.range(-10.0, 10.0), bz + rng.0.range(-6.0, 14.0));
        junk_at(&mut commands, &mut rng, solid, x, z);
    }
    for _ in 0..JUNK {
        if let Some((x, z)) = open_spot(&mut rng, solid, 1.0) {
            junk_at(&mut commands, &mut rng, solid, x, z);
        }
    }
}
