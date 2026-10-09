//! Hand-placed and scattered props that make the map feel lived in (and
//! then abandoned): shelter workbenches, the three weapon finds, supply
//! caches, camps with tents and campfires (one still burning), snowmobiles, sleds, shopping
//! carts, vending machines, mailboxes, lamp posts, chain-link fences,
//! ice-fishing sets and more road signs.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::snow::{RockSnowExt, RockSnowMaterial, ROCK_SNOW};
use crate::interact::{spawn_container, ContainerAssets, Workbench};
use crate::landmarks::{sign, RUINS};
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::sim::collision::{self, Shape};
use crate::sim::combat::WeaponKind;
use crate::sim::loot;
use crate::sim::meshgen::{self, sec, MeshData};
use crate::sim::terrain::{self, LAKES, RAD_SOURCES, ROAD_HALF_WIDTH, SHELTERS, VAULT_POS};
use crate::state::{Colliders, RngRes};
use crate::world::{flicker_light, ground, mat, open_spot, prop, PLACED};

const CACHES: usize = 12;

/// A fence line of chain-link panels between two points (axis-aligned so it
/// can be a simple collider).
fn fence(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &GameAssets,
    post_mat: &Handle<StandardMaterial>,
    wire_mat: &Handle<StandardMaterial>,
    solid: &mut Vec<Shape>,
    from: Vec2,
    to: Vec2,
) {
    let len = from.distance(to);
    let panels = (len / 2.5).round().max(1.0) as usize;
    let step = (to - from) / panels as f32;
    let yaw = (-step.y).atan2(step.x);
    let panel_len = step.length();
    let panel = meshes.add(Rectangle::new(panel_len, 1.9));
    let post = meshes.add(Cylinder::new(0.045, 2.1));
    let rail = meshes.add(Cuboid::new(panel_len, 0.05, 0.05));
    for i in 0..=panels {
        let p = from + step * i as f32;
        commands.spawn((
            Mesh3d(post.clone()),
            MeshMaterial3d(post_mat.clone()),
            Transform::from_xyz(p.x, ground(p.x, p.y) + 1.0, p.y),
        ));
    }
    let _ = assets;
    for i in 0..panels {
        let c = from + step * (i as f32 + 0.5);
        let gy = ground(c.x, c.y);
        commands.spawn((
            Mesh3d(panel.clone()),
            MeshMaterial3d(wire_mat.clone()),
            Transform::from_xyz(c.x, gy + 1.0, c.y).with_rotation(Quat::from_rotation_y(yaw)),
            NotShadowCaster,
        ));
        commands.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(post_mat.clone()),
            Transform::from_xyz(c.x, gy + 1.98, c.y).with_rotation(Quat::from_rotation_y(yaw)),
        ));
        let (w, d) = if step.x.abs() > step.y.abs() { (panel_len, 0.3) } else { (0.3, panel_len) };
        solid.push(Shape::rect_centered(c.x, c.y, w, d));
    }
}

pub fn spawn_props(
    mut commands: Commands,
    assets: Res<GameAssets>,
    containers: Res<ContainerAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
    server: Res<AssetServer>,
    mut rock_materials: ResMut<Assets<RockSnowMaterial>>,
) {
    let solid = &mut colliders.0;
    let snow_blob = |meshes: &mut Assets<Mesh>, r: f32, seed: u64| meshes.add(to_mesh_tangents(&meshgen::blob(r, 0.3, 0.2, seed, 1.5)));
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.4, 0.42),
        metallic: 0.8,
        perceptual_roughness: 0.55,
        ..default()
    });
    let black = mat(&mut materials, Color::srgb(0.04, 0.04, 0.045));
    let paint = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            base_color_texture: Some(assets.rust_diff.clone()),
            normal_map_texture: Some(assets.rust_normal.clone()),
            perceptual_roughness: 0.75,
            metallic: 0.3,
            ..default()
        })
    };

    // ================= Workbenches at every shelter =================
    for &(sx, sz) in SHELTERS.iter() {
        let (bx, bz) = (sx - 3.9, sz);
        let gy = ground(bx, bz);
        prop(&mut commands, &assets.tool_cart, Vec3::new(bx, gy, bz), FRAC_PI_2, 1.25);
        commands.spawn((
            SceneRoot(assets.toolbox.clone()),
            Transform::from_xyz(bx, gy + 1.05, bz + 0.3).with_scale(Vec3::splat(1.3)),
        ));
        commands.spawn((Transform::from_xyz(bx, gy, bz), Workbench));
        solid.push(Shape::rect_centered(bx, bz, 1.0, 1.7));
        // A stash by the door of every fish house.
        let stash = loot::roll_cache(&mut rng.0);
        spawn_container(
            &mut commands,
            &containers,
            solid,
            assets.crate_wood.clone(),
            Vec3::new(sx - 1.5, ground(sx - 1.5, sz + 2.6) + 0.05, sz + 2.6),
            0.2,
            1.1,
            "shelter stash",
            stash,
        );
        // A pegboard of tools behind the bench, against the house wall.
        let board = meshgen::cuboid([0.06, 1.0, 1.6], 1.0);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&board))),
            MeshMaterial3d(assets.planks_dark.clone()),
            Transform::from_xyz(sx - 3.2, gy + 1.5, sz),
        ));
        for (k, z) in [-0.5f32, -0.1, 0.3, 0.6].into_iter().enumerate() {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.03, 0.28 - 0.04 * k as f32, 0.05))),
                MeshMaterial3d(metal.clone()),
                Transform::from_xyz(sx - 3.12, gy + 1.5, sz + z),
            ));
        }
    }

    // ================= The three weapon finds =================
    let (bx, bz, _) = RUINS[0];
    let (gx, gz, _, _) = RAD_SOURCES[1];
    let gzz = gz + 9.0;
    let finds = [
        (WeaponKind::Revolver, assets.crate_military.clone(), Vec3::new(gx + 4.0, ground(gx + 4.0, gzz - 7.0) + 0.1, gzz - 7.0), 2.6, 1.3, "armoury crate"),
        (WeaponKind::IceAxe, assets.toolbox.clone(), {
            let (sx, sz) = SHELTERS[1];
            Vec3::new(sx + 3.0, ground(sx + 3.0, sz - 2.6) + 0.02, sz - 2.6)
        }, 0.3, 1.8, "tackle box"),
    ];
    for (kind, scene, pos, yaw, scale, name) in finds {
        spawn_container(&mut commands, &containers, solid, scene, pos, yaw, scale, name, loot::weapon_cache(kind));
    }

    // ================= Supply caches scattered about =================
    let cache_models = [
        (assets.crate_wood.clone(), 1.2f32, "supply crate"),
        (assets.crate_military.clone(), 1.1, "footlocker"),
        (assets.toolbox.clone(), 1.7, "tool chest"),
    ];
    let mut placed = 0;
    for i in 0..60 {
        if placed >= CACHES {
            break;
        }
        let Some((x, z)) = open_spot(&mut rng, solid, 1.5, &PLACED) else { continue };
        let (scene, scale, name) = cache_models[i % cache_models.len()].clone();
        let pos = Vec3::new(x, ground(x, z) + 0.05, z);
        let yaw = rng.0.range(0.0, std::f32::consts::TAU);
        let cache = loot::roll_cache(&mut rng.0);
        spawn_container(&mut commands, &containers, solid, scene, pos, yaw, scale, name, cache);
        placed += 1;
    }

    // ================= Camps: tent, cold fire pit, sled, gear =================
    let tent_walls = meshes.add(to_mesh_tangents(&meshgen::tent(2.4, 1.1, 3.0, 0.9, 0.15, 1.5)));
    // Old army canvas, faded to a dirty olive-tan by sun and snow (CC0
    // hessian weave, tinted), not a bright nylon tent.
    let canvas = |name: &str, srgb: bool| Some(crate::assets::tiled(&server, format!("textures/hessian_230/{name}"), srgb));
    let tent_cloth = rock_materials.add(RockSnowMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.5, 0.53, 0.4),
            base_color_texture: canvas("diff.jpg", true),
            normal_map_texture: canvas("nor.jpg", false),
            metallic_roughness_texture: canvas("arm.jpg", false),
            occlusion_texture: canvas("arm.jpg", false),
            metallic: 0.0,
            perceptual_roughness: 1.0,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(2.5)),
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        // Snow lying in patches on the roof (see shaders/rock_snow.wgsl).
        extension: RockSnowExt { snow: ROCK_SNOW.extend(0.6), detail: Vec4::ZERO },
    });

    let tent_dark = mat(&mut materials, Color::srgb(0.02, 0.02, 0.02));
    let char_log = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.045, 0.035),
        perceptual_roughness: 1.0,
        ..default()
    });
    let log_mesh = meshes.add(to_mesh_tangents(&meshgen::lathe(&[(0.07, -0.45), (0.08, 0.45)], 8, 0.4, true, true)));
    let sled_mesh = {
        let board = meshgen::loft_z(
            &[sec(-0.95, 0.07, 0.2, 0.012, 4.0), sec(0.4, 0.07, 0.2, 0.012, 4.0), sec(0.75, 0.11, 0.2, 0.012, 4.0), sec(0.98, 0.24, 0.2, 0.012, 4.0)],
            10,
            0.6,
        );
        meshes.add(to_mesh_tangents(&board))
    };
    let rope = {
        let pts = meshgen::catenary([0.0, 0.3, 0.98], [0.0, 0.35, 1.9], 0.25, 10);
        meshes.add(to_mesh(&meshgen::tube(&pts, 0.012, 5)))
    };
    let rope_mat = mat(&mut materials, Color::srgb(0.55, 0.45, 0.3));
    let ash = meshes.add(Circle::new(0.75));
    let ash_mat = mat(&mut materials, Color::srgb(0.14, 0.13, 0.13));
    let camps = [(-110.0f32, 15.0f32, 0.4f32), (160.0, 55.0, -0.9)];
    for (ci, &(cx, cz, yaw)) in camps.iter().enumerate() {
        if !terrain::is_open_ground(cx, cz) {
            continue;
        }
        let rot = Quat::from_rotation_y(yaw);
        let at = |dx: f32, dz: f32| {
            let v = rot * Vec3::new(dx, 0.0, dz);
            (cx + v.x, cz + v.z)
        };
        // Tent.
        let (tx, tz) = at(0.0, 0.0);
        let gy = ground(tx, tz);
        commands
            .spawn((Transform::from_xyz(tx, gy, tz).with_rotation(rot), Visibility::default()))
            .with_children(|t| {
                t.spawn((Mesh3d(tent_walls.clone()), MeshMaterial3d(tent_cloth.clone())));
                t.spawn((Mesh3d(meshes.add(Cuboid::new(0.9, 0.9, 0.05))), MeshMaterial3d(tent_dark.clone()), Transform::from_xyz(0.0, 0.5, 1.52)));
                for x in [-1.4f32, 1.4] {
                    for z in [-1.7f32, 1.7] {
                        t.spawn((Mesh3d(meshes.add(Cylinder::new(0.02, 0.5))), MeshMaterial3d(rope_mat.clone()), Transform::from_xyz(x, 0.15, z).with_rotation(Quat::from_rotation_z(-x.signum() * 0.5))));
                    }
                }
            });
        for dz in [-1.0f32, 0.0, 1.0] {
            let (x, z) = at(0.0, dz);
            solid.push(Shape::Circle { x, z, r: 1.15 });
        }
        // Cold fire pit with charred logs.
        let (fx, fz) = at(3.6, 0.8);
        let fgy = ground(fx, fz);
        prop(&mut commands, &assets.fire_pit, Vec3::new(fx, fgy, fz), yaw, 1.0);
        commands.spawn((
            Mesh3d(ash.clone()),
            MeshMaterial3d(ash_mat.clone()),
            Transform::from_xyz(fx, fgy + 0.07, fz).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            NotShadowCaster,
        ));
        for k in 0..3 {
            let a = k as f32 * 2.1 + ci as f32;
            commands.spawn((
                Mesh3d(log_mesh.clone()),
                MeshMaterial3d(char_log.clone()),
                Transform::from_xyz(fx + a.cos() * 0.15, fgy + 0.18, fz + a.sin() * 0.15)
                    .with_rotation(Quat::from_rotation_y(a) * Quat::from_rotation_z(FRAC_PI_2 - 0.2)),
            ));
        }
        solid.push(Shape::Circle { x: fx, z: fz, r: 0.9 });
        // Someone was here recently: the first camp's fire is still burning.
        if ci == 0 {
            crate::particles::spawn_fire(&mut commands, &mut meshes, &mut materials, &assets.soft, Vec3::new(fx, fgy + 0.12, fz), 0.9, 9.1, true);
        }
        // Sled, bucket, spade, boombox.
        let (sx, sz) = at(-3.0, 0.5);
        commands
            .spawn((Transform::from_xyz(sx, ground(sx, sz), sz).with_rotation(rot * Quat::from_rotation_y(0.4)), Visibility::default()))
            .with_children(|s| {
                s.spawn((Mesh3d(sled_mesh.clone()), MeshMaterial3d(assets.planks_dark.clone())));
                s.spawn((Mesh3d(rope.clone()), MeshMaterial3d(rope_mat.clone())));
                for x in [-0.17f32, 0.17] {
                    s.spawn((Mesh3d(meshes.add(Cuboid::new(0.03, 0.06, 1.7))), MeshMaterial3d(metal.clone()), Transform::from_xyz(x, 0.03, 0.0)));
                }
            });
        let (bx2, bz2) = at(4.6, -0.6);
        prop(&mut commands, &assets.bucket, Vec3::new(bx2, ground(bx2, bz2), bz2), 0.3, 1.0);
        let (px, pz) = at(-1.8, -2.2);
        commands.spawn((
            SceneRoot(assets.spade.clone()),
            Transform::from_xyz(px, ground(px, pz) + 0.45, pz).with_rotation(Quat::from_rotation_y(1.0) * Quat::from_rotation_z(0.18)),
        ));
        let (mx, mz) = at(2.0, 2.6);
        prop(&mut commands, &assets.boombox, Vec3::new(mx, ground(mx, mz), mz), yaw + 2.0, 1.0);
        // A cache of its own.
        let (kx, kz) = at(-2.2, -3.4);
        let cache = loot::roll_cache(&mut rng.0);
        spawn_container(
            &mut commands,
            &containers,
            solid,
            assets.crate_wood.clone(),
            Vec3::new(kx, ground(kx, kz) + 0.05, kz),
            0.8,
            1.2,
            "camp footlocker",
            cache,
        );
    }
    // ================= Shopping carts around the Bullseye-Mart =================
    let cart_mesh = {
        let mut m = MeshData::default();
        let (hw, hl) = (0.3, 0.46);
        // Bottom grid and walls: thin wires.
        let mut x = -hw;
        while x <= hw + 1e-3 {
            m.append(&meshgen::tube(&[[x, 0.42, -hl], [x, 0.42, hl]], 0.006, 4));
            x += 0.1;
        }
        let mut z = -hl;
        while z <= hl + 1e-3 {
            m.append(&meshgen::tube(&[[-hw, 0.42, z], [hw, 0.42, z]], 0.006, 4));
            z += 0.115;
        }
        for (a, b) in [(-hw, hl), (hw, hl), (-hw, -hl), (hw, -hl)] {
            m.append(&meshgen::tube(&[[a, 0.42, b], [a * 1.3, 0.86, b * 1.2]], 0.01, 5));
        }
        for y in [0.55, 0.7, 0.86] {
            let k = 1.0 + (y - 0.42) / 0.44 * 0.3;
            let (w2, l2) = (hw * k, hl * (1.0 + (y - 0.42) / 0.44 * 0.2));
            m.append(&meshgen::tube(&[[-w2, y, -l2], [w2, y, -l2], [w2, y, l2], [-w2, y, l2], [-w2, y, -l2]], 0.008, 4));
        }
        // Handle and legs.
        m.append(&meshgen::tube(&[[-hw * 1.3, 0.86, -hl * 1.2], [-hw * 1.3, 0.98, -hl * 1.25], [hw * 1.3, 0.98, -hl * 1.25], [hw * 1.3, 0.86, -hl * 1.2]], 0.012, 5));
        for (a, b) in [(-0.27, -0.4), (0.27, -0.4), (-0.27, 0.42), (0.27, 0.42)] {
            m.append(&meshgen::tube(&[[a, 0.42, b], [a, 0.08, b]], 0.01, 5));
        }
        meshes.add(to_mesh(&m))
    };
    let wheel = meshes.add(Cylinder::new(0.045, 0.03));
    let cart_spots = [(bx + 2.0f32, bz + 15.0f32, 0.3f32, 0.0f32), (bx + 7.0, bz + 17.0, 2.0, 0.0), (bx - 6.0, bz + 13.0, 1.2, 1.35), (bx + 12.5, bz + 14.0, -0.4, 0.0), (bx - 14.0, bz + 4.0, 0.8, 0.0)];
    for &(x, z, yaw, tip) in &cart_spots {
        if collision::blocked(x, z, 0.8, solid) {
            continue;
        }
        let gy = ground(x, z);
        commands
            .spawn((
                Transform::from_xyz(x, gy + if tip > 0.0 { 0.35 } else { 0.0 }, z)
                    .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(tip)),
                Visibility::default(),
            ))
            .with_children(|c| {
                c.spawn((Mesh3d(cart_mesh.clone()), MeshMaterial3d(metal.clone())));
                for (a, b) in [(-0.27, -0.4), (0.27, -0.4), (-0.27, 0.42), (0.27, 0.42)] {
                    c.spawn((Mesh3d(wheel.clone()), MeshMaterial3d(black.clone()), Transform::from_xyz(a, 0.045, b).with_rotation(Quat::from_rotation_z(FRAC_PI_2))));
                }
                c.spawn((Mesh3d(snow_blob(&mut meshes, 0.3, 9)), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 0.5, 0.0)));
            });
        solid.push(Shape::Circle { x, z, r: 0.55 });
    }
    // Dumpsters and bins by the Mart, shelving inside it.
    for (k, &(dx, dz, yaw)) in [(16.0f32, 3.0f32, 1.57f32), (16.0, 6.2, 1.5), (-15.0, 11.0, 0.2)].iter().enumerate() {
        let (x, z) = (bx + dx, bz + dz);
        if collision::blocked(x, z, 1.0, solid) {
            continue;
        }
        prop(&mut commands, &assets.trash_can, Vec3::new(x, ground(x, z), z), yaw + k as f32 * 0.1, 1.4);
        solid.push(Shape::Circle { x, z, r: 1.0 });
    }
    for (k, &(dx, dz)) in [(-8.5f32, -6.6f32), (-4.5, -6.6), (2.0, -6.6)].iter().enumerate() {
        let (x, z) = (bx + dx, bz + dz);
        let tipped = k == 1;
        commands.spawn((
            SceneRoot(assets.rack.clone()),
            Transform::from_xyz(x, ground(x, z) + if tipped { 0.6 } else { 0.0 }, z)
                .with_rotation(Quat::from_rotation_y(0.05 * k as f32) * Quat::from_rotation_z(if tipped { 1.1 } else { 0.0 }))
                .with_scale(Vec3::splat(1.2)),
        ));
        solid.push(Shape::Circle { x, z, r: 0.7 });
    }

    // ================= Vending machines =================
    let vend_body = meshes.add(to_mesh_tangents(&meshgen::cuboid([0.92, 1.84, 0.78], 1.5)));
    let vend_front = meshes.add(Rectangle::new(0.82, 1.64));
    let vend_red = paint(&mut materials, Color::srgb(0.7, 0.1, 0.1));
    let vend_face = materials.add(StandardMaterial {
        base_color_texture: Some(assets.vending_front.clone()),
        emissive_texture: Some(assets.vending_glow.clone()),
        emissive: LinearRgba::rgb(1.6, 1.6, 1.6),
        perceptual_roughness: 0.3,
        ..default()
    });
    let road_n = |x: f32| terrain::road_z(x);
    let vend_spots = [
        (bx + 14.0, bz + 9.5, PI, true),
        (VAULT_POS.0 - 11.5, VAULT_POS.1 - 9.0, PI * 0.5, false),
        (-30.0, road_n(-30.0) + ROAD_HALF_WIDTH + 2.4, PI, false),
        (VAULT_POS.0 + 24.0, VAULT_POS.1 - 28.0, PI, false),
    ];
    for &(x, z, yaw, flicker) in &vend_spots {
        if collision::blocked(x, z, 0.8, solid) || terrain::lake_at(x, z).is_some() {
            continue;
        }
        let gy = ground(x, z);
        commands
            .spawn((Transform::from_xyz(x, gy, z).with_rotation(Quat::from_rotation_y(yaw)), Visibility::default()))
            .with_children(|v| {
                v.spawn((Mesh3d(vend_body.clone()), MeshMaterial3d(vend_red.clone()), Transform::from_xyz(0.0, 0.92, 0.0)));
                v.spawn((Mesh3d(vend_front.clone()), MeshMaterial3d(vend_face.clone()), Transform::from_xyz(0.0, 0.95, 0.392), NotShadowCaster));
                v.spawn((Mesh3d(snow_blob(&mut meshes, 0.5, 3)), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 1.88, 0.0).with_scale(Vec3::new(1.0, 0.8, 0.9)), NotShadowCaster));
                if flicker {
                    v.spawn((flicker_light(Color::srgb(0.85, 1.0, 0.9), 90_000.0, 9.0, false, 4.4), Transform::from_xyz(0.0, 1.0, 1.0)));
                }
            });
        solid.push(Shape::rect_centered(x, z, 1.0, 0.9));
    }

    // ================= Mailboxes along the highway =================
    let mailbox = {
        let m = meshgen::loft_z(&[sec(-0.22, 0.0, 0.07, 0.07, 3.0), sec(-0.21, 0.0, 0.1, 0.1, 3.0), sec(0.2, 0.0, 0.1, 0.1, 3.0), sec(0.22, 0.0, 0.07, 0.07, 3.0)], 12, 0.4);
        meshes.add(to_mesh_tangents(&m))
    };
    let post_mesh = meshes.add(Cylinder::new(0.045, 1.1));
    let flag = meshes.add(Cuboid::new(0.02, 0.1, 0.16));
    let red_flag = mat(&mut materials, Color::srgb(0.75, 0.1, 0.08));
    for (k, &x) in [-172.0f32, -104.0, -44.0, 38.0, 112.0, 176.0].iter().enumerate() {
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let z = road_n(x) + side * (ROAD_HALF_WIDTH + 2.2);
        if collision::blocked(x, z, 0.4, solid) {
            continue;
        }
        let gy = ground(x, z);
        let knocked = k == 3;
        commands
            .spawn((
                Transform::from_xyz(x, gy, z).with_rotation(Quat::from_rotation_y(if side < 0.0 { 0.0 } else { PI }) * Quat::from_rotation_z(if knocked { 0.5 } else { 0.0 })),
                Visibility::default(),
            ))
            .with_children(|m| {
                m.spawn((Mesh3d(post_mesh.clone()), MeshMaterial3d(assets.pole_wood.clone()), Transform::from_xyz(0.0, 0.55, 0.0)));
                m.spawn((Mesh3d(mailbox.clone()), MeshMaterial3d(metal.clone()), Transform::from_xyz(0.0, 1.17, 0.0)));
                m.spawn((Mesh3d(flag.clone()), MeshMaterial3d(red_flag.clone()), Transform::from_xyz(0.11, 1.28, -0.1)));
                m.spawn((Mesh3d(snow_blob(&mut meshes, 0.14, 21 + k as u64)), MeshMaterial3d(assets.snow.clone()), Transform::from_xyz(0.0, 1.3, 0.0).with_scale(Vec3::new(1.0, 0.5, 1.4)), NotShadowCaster));
            });
        solid.push(Shape::Circle { x, z, r: 0.25 });
    }

    // ================= Lamp posts =================
    let mut lamp_spots: Vec<(f32, f32, f32, bool)> = Vec::new();
    for x in [-150.0f32, -110.0, -70.0, 10.0, 50.0, 90.0, 130.0] {
        lamp_spots.push((x, road_n(x) - ROAD_HALF_WIDTH - 1.8, FRAC_PI_2, false));
    }
    for (dx, dz) in [(-7.5f32, -12.0f32), (7.5, -12.0), (-7.5, -30.0), (7.5, -30.0)] {
        lamp_spots.push((VAULT_POS.0 + dx, VAULT_POS.1 + dz, if dx < 0.0 { 0.0 } else { PI }, dz == -12.0));
    }
    for &(x, z, yaw, lit) in &lamp_spots {
        if collision::blocked(x, z, 0.4, solid) {
            continue;
        }
        let gy = ground(x, z);
        prop(&mut commands, &assets.street_lamp, Vec3::new(x, gy, z), yaw, 1.0);
        solid.push(Shape::Circle { x, z, r: 0.3 });
        if lit {
            // The vault's own lamps still run off the geothermal tap.
            commands.spawn((flicker_light(Color::srgb(1.0, 0.9, 0.7), 220_000.0, 16.0, false, x * 0.3), Transform::from_xyz(x, gy + 3.6, z)));
        }
    }

    // ================= Chain-link fences =================
    let post_mat = metal.clone();
    let wire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.78, 0.8),
        base_color_texture: Some(assets.chainlink.clone()),
        alpha_mode: AlphaMode::Mask(0.4),
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.6,
        metallic: 0.5,
        unlit: false,
        ..default()
    });
    let (vx, vz) = VAULT_POS;
    let mut fences = vec![
        (Vec2::new(vx - 14.0, vz - 4.0), Vec2::new(vx - 14.0, vz - 26.0)),
        (Vec2::new(vx - 14.0, vz - 26.0), Vec2::new(vx - 28.0, vz - 26.0)),
        (Vec2::new(vx + 14.0, vz - 4.0), Vec2::new(vx + 14.0, vz - 14.0)),
        (Vec2::new(vx + 14.0, vz - 20.0), Vec2::new(vx + 14.0, vz - 26.0)),
        (Vec2::new(vx + 14.0, vz - 26.0), Vec2::new(vx + 28.0, vz - 26.0)),
    ];
    let (gx, gzz) = (gx, gz + 9.0);
    fences.push((Vec2::new(gx - 15.0, gzz - 12.0), Vec2::new(gx - 3.0, gzz - 12.0)));
    fences.push((Vec2::new(gx + 5.0, gzz - 12.0), Vec2::new(gx + 15.0, gzz - 12.0)));
    for (from, to) in fences {
        fence(&mut commands, &mut meshes, &assets, &post_mat, &wire_mat, solid, from, to);
    }

    // ================= Ice-fishing sets on the lakes =================
    let hole = meshes.add(Circle::new(0.24));
    let hole_ring = meshes.add(Annulus::new(0.24, 0.4));
    let hole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.01, 0.03, 0.05),
        perceptual_roughness: 0.05,
        reflectance: 0.9,
        ..default()
    });
    let ring_mat = mat(&mut materials, Color::srgb(0.85, 0.92, 0.95));
    let stick = meshes.add(Cuboid::new(0.025, 0.025, 0.7));
    for (li, &(lx, lz, r)) in LAKES.iter().enumerate() {
        for k in 0..3 {
            let a = (li as f32 * 1.7 + k as f32 * 2.1) % std::f32::consts::TAU;
            let (x, z) = (lx + a.cos() * r * 0.78, lz + a.sin() * r * 0.78);
            if terrain::lake_at(x, z).is_none() {
                continue;
            }
            let y = terrain::ICE_LEVEL + 0.012;
            let flat = Quat::from_rotation_x(-FRAC_PI_2);
            commands.spawn((Mesh3d(hole_ring.clone()), MeshMaterial3d(ring_mat.clone()), Transform::from_xyz(x, y, z).with_rotation(flat), NotShadowCaster));
            commands.spawn((Mesh3d(hole.clone()), MeshMaterial3d(hole_mat.clone()), Transform::from_xyz(x, y + 0.004, z).with_rotation(flat), NotShadowCaster));
            // Tip-up: two crossed sticks and a flag on a string.
            let yaw = a + 1.0;
            commands.spawn((Mesh3d(stick.clone()), MeshMaterial3d(assets.pole_wood.clone()), Transform::from_xyz(x, y + 0.02, z).with_rotation(Quat::from_rotation_y(yaw))));
            commands.spawn((Mesh3d(stick.clone()), MeshMaterial3d(assets.pole_wood.clone()), Transform::from_xyz(x, y + 0.2, z).with_rotation(Quat::from_rotation_y(yaw + FRAC_PI_2) * Quat::from_rotation_x(0.0)).with_scale(Vec3::new(1.0, 1.0, 0.5))));
            commands.spawn((Mesh3d(flag.clone()), MeshMaterial3d(red_flag.clone()), Transform::from_xyz(x + 0.1 * yaw.cos(), y + 0.5, z - 0.1 * yaw.sin())));
            if k == 0 {
                prop(&mut commands, &assets.bucket, Vec3::new(x + 0.9, terrain::ICE_LEVEL, z + 0.4), a, 1.0);
            }
        }
    }

    // ================= More road signs =================
    let post_gray = mat(&mut materials, Color::srgb(0.4, 0.4, 0.4));
    let road_sign = |commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, solid: &mut Vec<Shape>, image: &Handle<Image>, size: Vec2, x: f32, side: f32, yaw: f32| {
        let z = road_n(x) + side * (ROAD_HALF_WIDTH + 1.8);
        let gy = ground(x, z);
        let h = 2.2 + size.y * 0.5;
        let rot = Quat::from_rotation_y(yaw);
        // Posts stand behind the board: one for a small sign, two at the
        // sides of a wide one.
        let back = rot * Vec3::new(0.0, 0.0, -0.085);
        let posts: &[f32] = if size.x > 1.5 { &[-0.36, 0.36] } else { &[0.0] };
        for &u in posts {
            let p = Vec3::new(x, gy + h * 0.5, z) + back + rot * Vec3::X * (u * size.x);
            commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.045, h))), MeshMaterial3d(post_gray.clone()), Transform::from_translation(p)));
        }
        sign(commands, meshes, materials, image, size, Transform::from_xyz(x, gy + 2.2, z).with_rotation(rot), 0.0);
        solid.push(Shape::Circle { x, z, r: 0.25 });
    };
    road_sign(&mut commands, &mut meshes, &mut materials, solid, &assets.sign_speed, Vec2::new(0.8, 1.0), -100.0, -1.0, FRAC_PI_2);
    road_sign(&mut commands, &mut meshes, &mut materials, solid, &assets.sign_speed, Vec2::new(0.8, 1.0), 122.0, 1.0, -FRAC_PI_2);
    road_sign(&mut commands, &mut meshes, &mut materials, solid, &assets.sign_welcome, Vec2::new(3.2, 1.6), -176.0, 1.0, FRAC_PI_2);
    {
        // The bait shop's board, propped against the third fish house.
        let (sx, sz) = SHELTERS[2];
        let (x, z) = (sx - 1.5, sz + 1.62);
        sign(&mut commands, &mut meshes, &mut materials, &assets.sign_bait, Vec2::new(1.4, 0.7), Transform::from_xyz(x, ground(x, z) + 1.35, z), 0.0);
    }
}
