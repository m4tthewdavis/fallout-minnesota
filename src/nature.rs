//! Scatters the Northwoods over the map: snow-laden pines, dead snags, bare
//! shrubs, boulders, snowdrifts and pre-war junk.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::landmarks::{CARS, RUINS};
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::sim::collision::{self, Shape};
use crate::sim::meshgen::{self, MeshData};
use crate::sim::terrain::{self, HALF_SIZE};
use crate::state::{Colliders, RngRes};
use crate::world::{ground, prop};

const PINES: usize = 260;
const ROCKS: usize = 30;
const DRIFTS: usize = 50;
const SHRUBS: usize = 70;
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;

    // ---------- Pines ----------
    let needles = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let variants: Vec<(Handle<Mesh>, Handle<Mesh>, f32)> = [(8.0, 7, 1), (9.5, 8, 2), (7.0, 6, 3), (10.5, 9, 4)]
        .into_iter()
        .map(|(h, tiers, seed)| {
            let foliage = meshes.add(to_mesh(&meshgen::pine(h, tiers, 1.0, seed)));
            let trunk = meshgen::lathe(&[(0.26, -0.3), (0.22, h * 0.25), (0.12, h * 0.7), (0.04, h * 0.95)], 8, 1.2, false, false);
            (foliage, meshes.add(to_mesh_tangents(&trunk)), h)
        })
        .collect();
    let snag_variants: Vec<Handle<Mesh>> = (0..3)
        .map(|seed| {
            let h = 7.0 + seed as f32;
            let mut m = meshgen::lathe(&[(0.24, -0.3), (0.18, h * 0.3), (0.08, h * 0.85), (0.02, h)], 7, 1.2, false, false);
            m.append(&meshgen::dead_branches(h, 20 + seed as u64));
            meshes.add(to_mesh_tangents(&m))
        })
        .collect();

    let mut placed = 0;
    let mut tries = 0;
    while placed < PINES && tries < 3_000 {
        tries += 1;
        let Some((x, z)) = open_spot(&mut rng, solid, 1.0) else { continue };
        placed += 1;
        let yaw = rng.0.range(0.0, std::f32::consts::TAU);
        let snag = rng.0.chance(0.12);
        let scale = rng.0.range(0.75, 1.35);
        let tf = Transform::from_xyz(x, ground(x, z) - 0.1, z)
            .with_scale(Vec3::splat(scale))
            .with_rotation(Quat::from_rotation_y(yaw));
        solid.push(Shape::Circle { x, z, r: 0.4 * scale });
        if snag {
            let mesh = snag_variants[(rng.0.f32() * snag_variants.len() as f32) as usize % snag_variants.len()].clone();
            commands.spawn((Mesh3d(mesh), MeshMaterial3d(assets.bark.clone()), tf));
        } else {
            let (foliage, trunk, _) = &variants[(rng.0.f32() * variants.len() as f32) as usize % variants.len()];
            commands.spawn((tf, Visibility::default())).with_children(|tree| {
                tree.spawn((Mesh3d(trunk.clone()), MeshMaterial3d(assets.bark.clone())));
                tree.spawn((Mesh3d(foliage.clone()), MeshMaterial3d(needles.clone())));
            });
        }
    }

    // ---------- Boulders poking through the snow ----------
    let cap = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.3, 0.25, 31, 1.0)));
    for _ in 0..ROCKS {
        let Some((x, z)) = open_spot(&mut rng, solid, 1.5) else { continue };
        let s = rng.0.range(8.0, 16.0);
        let yaw = rng.0.range(0.0, 6.28);
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
    }

    // ---------- Snowdrifts ----------
    for i in 0..DRIFTS {
        let Some((x, z)) = open_spot(&mut rng, solid, 0.5) else { continue };
        let r = rng.0.range(2.0, 5.5);
        let drift = meshgen::blob(r, 0.2, 0.25, 200 + i as u64, 3.0).scaled([1.0, 1.0, rng.0.range(0.4, 0.8)]);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&drift))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, ground(x, z) - 0.12, z).with_rotation(Quat::from_rotation_y(rng.0.range(0.0, 6.28))),
            NotShadowCaster,
        ));
    }

    // ---------- Bare shrubs ----------
    let shrub_variants: Vec<Handle<Mesh>> = (0..4)
        .map(|seed| {
            let mut m = MeshData::default();
            m.append(&meshgen::dead_branches(1.6, 300 + seed).translated([0.0, -0.7, 0.0]));
            meshes.add(to_mesh(&m))
        })
        .collect();
    let twigs = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.12, 0.09),
        perceptual_roughness: 1.0,
        ..default()
    });
    for _ in 0..SHRUBS {
        let Some((x, z)) = open_spot(&mut rng, solid, 0.5) else { continue };
        let mesh = shrub_variants[(rng.0.f32() * 4.0) as usize % 4].clone();
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(twigs.clone()),
            Transform::from_xyz(x, ground(x, z), z)
                .with_rotation(Quat::from_rotation_y(rng.0.range(0.0, 6.28)))
                .with_scale(Vec3::splat(rng.0.range(0.7, 1.3))),
        ));
    }

    // ---------- Pre-war junk ----------
    // Clusters by the cars and shelters, then a sprinkle over open ground.
    let junk_at = |commands: &mut Commands, rng: &mut RngRes, solid: &mut Vec<Shape>, x: f32, z: f32| {
        if collision::blocked(x, z, 0.6, solid) || terrain::lake_at(x, z).is_some() {
            return;
        }
        let gy = ground(x, z);
        let yaw = rng.0.range(0.0, 6.28);
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
            let a = rng.0.range(0.0, 6.28);
            let d = rng.0.range(3.0, 5.0);
            junk_at(&mut commands, &mut rng, solid, cx + a.cos() * d, cz + a.sin() * d);
        }
    }
    for &(sx, sz) in &terrain::SHELTERS {
        let a = rng.0.range(0.0, 6.28);
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
