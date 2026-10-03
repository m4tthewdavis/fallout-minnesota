//! Hand-placed landmarks: Vault 143's hillside entrance, the fish-house
//! shelters, the Golden Atomic Mills silos, the Bullseye-Mart ruin, rusted
//! cars on the old US-169 highway, power lines and road signs.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::particles::{Emitter, EmitterKind, Flame};
use crate::sim::collision::{self, Shape};
use crate::sim::meshgen;
use crate::sim::terrain::{self, RAD_SOURCES, ROAD, ROAD_HALF_WIDTH, SHELTERS, VAULT_POS};
use crate::state::{Colliders, RngRes};
use crate::world::{glow, ground, mat, prop, Blinker, FireLight};

/// Pre-war landmarks trees should stay away from: (x, z, clear radius).
pub const RUINS: [(f32, f32, f32); 2] = [(-40.0, -110.0, 20.0), (140.0, -140.0, 18.0)];
pub const CARS: [(f32, f32, f32); 6] = [
    (-150.0, 95.0, 1.62),
    (-128.0, 92.0, 1.4),
    (-60.0, 98.0, 4.6),
    (60.0, 140.0, 2.6),
    (150.0, 100.0, 1.75),
    (165.0, 20.0, 1.7),
];

/// A flat sign showing `image`, facing +Z, with a plain backing board.
pub fn sign(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: &Handle<Image>,
    size: Vec2,
    tf: Transform,
    glowing: f32,
) -> Entity {
    let face = materials.add(StandardMaterial {
        base_color_texture: Some(image.clone()),
        emissive: LinearRgba::rgb(glowing, glowing, glowing),
        emissive_texture: (glowing > 0.0).then(|| image.clone()),
        perceptual_roughness: 0.7,
        ..default()
    });
    let back = mat(materials, Color::srgb(0.3, 0.3, 0.3));
    commands
        .spawn((tf, Visibility::default()))
        .with_children(|s| {
            s.spawn((
                Mesh3d(meshes.add(Rectangle::new(size.x, size.y))),
                MeshMaterial3d(face),
                Transform::from_xyz(0.0, 0.0, 0.03),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cuboid::new(size.x + 0.08, size.y + 0.08, 0.05))),
                MeshMaterial3d(back),
            ));
        })
        .id()
}

pub fn spawn_landmarks(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;
    let snow_cap = |r: f32, seed: u64| meshgen::blob(r, 0.22, 0.18, seed, 2.0);

    // ================= Vault 143 =================
    let (vx, vz) = VAULT_POS;
    let vy = ground(vx, vz - 8.0);
    // A poured-concrete portal set into a rocky, snow-capped hillside.
    let facade = meshgen::cuboid([24.0, 11.0, 3.0], 2.5);
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&facade))),
        MeshMaterial3d(assets.concrete.clone()),
        Transform::from_xyz(vx, vy + 4.7, vz + 0.5),
    ));
    // Pilasters and a lintel overhang give the face some depth.
    for x in [-11.4f32, 11.4] {
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([1.4, 11.0, 0.8], 2.5)))),
            MeshMaterial3d(assets.concrete.clone()),
            Transform::from_xyz(vx + x, vy + 4.7, vz - 1.3),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([26.0, 1.0, 3.4], 2.5)))),
        MeshMaterial3d(assets.concrete.clone()),
        Transform::from_xyz(vx, vy + 10.6, vz - 0.8),
    ));
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.25, 0.2, 9, 2.0).scaled([13.2, 1.0, 1.9])))),
        MeshMaterial3d(assets.snow.clone()),
        Transform::from_xyz(vx, vy + 11.1, vz - 0.8),
    ));
    // Boulders heaped around and over the portal, capped with snow.
    for (i, (x, y, z, r)) in [
        (-17.0, 3.0, 5.0, 7.0),
        (17.0, 3.0, 5.0, 7.0),
        (-20.0, 1.0, 10.0, 7.0),
        (20.0, 1.0, 10.0, 7.0),
        (-8.0, 11.5, 6.0, 7.5),
        (8.0, 11.5, 6.0, 7.5),
        (0.0, 12.5, 8.0, 8.0),
        (0.0, 6.0, 14.0, 11.0),
        (-13.0, 7.0, 11.0, 8.0),
        (13.0, 7.0, 11.0, 8.0),
    ]
    .into_iter()
    .enumerate()
    {
        let rock = meshgen::blob(r, 0.75, 0.22, 40 + i as u64, 3.0);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&rock))),
            MeshMaterial3d(assets.rock_wall.clone()),
            Transform::from_xyz(vx + x, vy + y, vz + z),
        ));
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&snow_cap(r * 0.85, 60 + i as u64)))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(vx + x, vy + y + r * 0.5, vz + z),
        ));
    }
    solid.push(Shape::rect_centered(vx, vz + 4.0, 26.0, 10.0));
    solid.push(Shape::Circle { x: vx - 17.0, z: vz + 5.0, r: 6.0 });
    solid.push(Shape::Circle { x: vx + 17.0, z: vz + 5.0, r: 6.0 });
    solid.push(Shape::Rect {
        x0: vx + 4.5,
        z0: vz - 1.9,
        x1: vx + 12.5,
        z1: vz - 0.9,
    });

    // The cog-shaped gear door, rolled aside to the right, with its hub.
    let door = meshgen::gear(4.0, 0.45, 10, 0.9, 2.0);
    let vault_yellow = glow(&mut materials, Color::srgb(0.85, 0.68, 0.10), LinearRgba::rgb(0.25, 0.18, 0.0));
    commands
        .spawn((
            Transform::from_xyz(vx + 8.5, vy + 4.2, vz - 1.4).with_rotation(Quat::from_rotation_y(PI)),
            Visibility::default(),
        ))
        .with_children(|d| {
            d.spawn((
                Mesh3d(meshes.add(to_mesh_tangents(&door))),
                MeshMaterial3d(assets.vault_metal.clone()),
            ));
            let hub = meshgen::lathe(&[(1.2, 0.0), (1.2, 0.3), (0.9, 0.55), (0.0, 0.6)], 24, 1.0, false, false)
                .rotated_x(FRAC_PI_2);
            d.spawn((
                Mesh3d(meshes.add(to_mesh(&hub))),
                MeshMaterial3d(assets.vault_metal.clone()),
                Transform::from_xyz(0.0, 0.0, 0.45),
            ));
            // "143" stencil ring of yellow bolts.
            for k in 0..14 {
                let a = k as f32 / 14.0 * TAU;
                d.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.12, 0.2))),
                    MeshMaterial3d(vault_yellow.clone()),
                    Transform::from_xyz(a.cos() * 2.9, a.sin() * 2.9, 0.5).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                ));
            }
        });
    // Yellow frame around the open doorway and the dark tunnel beyond it.
    let door_rot = Quat::from_rotation_x(FRAC_PI_2);
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(4.0, 4.9))),
        MeshMaterial3d(vault_yellow.clone()),
        Transform::from_xyz(vx, vy + 4.2, vz - 1.0).with_rotation(door_rot),
    ));
    let tunnel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.02, 0.025),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.05))),
        MeshMaterial3d(tunnel),
        Transform::from_xyz(vx, vy + 4.2, vz - 1.04).with_rotation(Quat::from_rotation_y(PI)),
    ));
    // The tunnel beyond: a floor ramp and the faint glow of the lit
    // atrium at the far end, low in the opening as perspective would put it.
    let corridor_floor = mat(&mut materials, Color::srgb(0.16, 0.15, 0.13));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(5.6, 0.7, 0.05))),
        MeshMaterial3d(corridor_floor),
        Transform::from_xyz(vx, vy + 0.6, vz - 1.05),
    ));
    let far_end = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.045, 0.04),
        emissive: LinearRgba::rgb(0.09, 0.07, 0.04),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Ellipse::new(1.3, 0.9))),
        MeshMaterial3d(far_end),
        Transform::from_xyz(vx, vy + 2.4, vz - 1.045).with_rotation(Quat::from_rotation_y(PI)),
        NotShadowCaster,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.5),
            intensity: 250_000.0,
            range: 14.0,
            ..default()
        },
        Transform::from_xyz(vx, vy + 3.0, vz - 3.0),
    ));
    // Floodlight over the door.
    commands.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.92, 0.75),
            intensity: 2_500_000.0,
            range: 40.0,
            outer_angle: 0.75,
            inner_angle: 0.45,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(vx, vy + 9.6, vz - 2.6).looking_at(Vec3::new(vx, vy, vz - 14.0), Vec3::Y),
    ));
    let lamp_housing = mat(&mut materials, Color::srgb(0.2, 0.2, 0.22));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 0.4, 0.6))),
        MeshMaterial3d(lamp_housing.clone()),
        Transform::from_xyz(vx, vy + 9.9, vz - 2.2),
    ));
    sign(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets.sign_shelter,
        Vec2::splat(2.0),
        Transform::from_xyz(vx - 7.5, vy + 5.5, vz - 1.1).with_rotation(Quat::from_rotation_y(PI)),
        0.0,
    );
    // Door control panel with a blinking status lamp.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.7, 1.3, 0.45))),
        MeshMaterial3d(assets.vault_metal.clone()),
        Transform::from_xyz(vx - 5.6, vy + 0.65, vz - 1.5),
    ));
    solid.push(Shape::Circle { x: vx - 5.6, z: vz - 1.5, r: 0.5 });
    let red_lamp = glow(&mut materials, Color::srgb(1.0, 0.1, 0.05), LinearRgba::rgb(12.0, 0.6, 0.2));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.07))),
        MeshMaterial3d(red_lamp.clone()),
        Transform::from_xyz(vx - 5.6, vy + 1.15, vz - 1.75),
        Blinker { period: 1.2, offset: 0.0 },
        NotShadowCaster,
    ));

    // ================= Fish-house shelters =================
    let window = glow(&mut materials, Color::srgb(1.0, 0.7, 0.35), LinearRgba::rgb(4.0, 2.2, 0.8));
    let pipe = mat(&mut materials, Color::srgb(0.12, 0.12, 0.12));
    let fire_core = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.5, 0.1),
        emissive: LinearRgba::rgb(10.0, 3.5, 0.5),
        unlit: true,
        ..default()
    });
    let (w, h, d, gable) = (3.0, 2.4, 3.0, 1.0);
    let walls = meshes.add(to_mesh_tangents(&meshgen::gable_walls(w, h, d, gable, 1.5)));
    let slope = (gable / (w * 0.5)).atan();
    let slab_len = ((w * 0.5).powi(2) + gable * gable).sqrt() + 0.35;
    let roof_slab = meshes.add(to_mesh_tangents(&meshgen::cuboid([slab_len, 0.08, d + 0.5], 1.5)));
    let roof_snow = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.12, 0.25, 77, 1.5).scaled([slab_len * 0.48, 1.0, (d + 0.3) * 0.5])));
    let log = meshes.add(to_mesh_tangents(&meshgen::lathe(&[(0.13, -0.6), (0.14, 0.6)], 8, 0.6, true, true)));
    for (i, &(sx, sz)) in SHELTERS.iter().enumerate() {
        let gy = ground(sx, sz);
        let (hx, hz) = (sx - 1.5, sz);
        commands
            .spawn((Transform::from_xyz(hx, gy, hz), Visibility::default()))
            .with_children(|house| {
                house.spawn((Mesh3d(walls.clone()), MeshMaterial3d(assets.planks_red.clone())));
                for side in [-1.0f32, 1.0] {
                    let rot = Quat::from_rotation_z(-side * slope);
                    let mid = Vec3::new(side * w * 0.25, h + gable * 0.5, 0.0) + rot * Vec3::Y * 0.06;
                    house.spawn((
                        Mesh3d(roof_slab.clone()),
                        MeshMaterial3d(assets.roof_metal.clone()),
                        Transform::from_translation(mid).with_rotation(rot),
                    ));
                    house.spawn((
                        Mesh3d(roof_snow.clone()),
                        MeshMaterial3d(assets.snow.clone()),
                        Transform::from_translation(mid + rot * Vec3::Y * 0.07).with_rotation(rot),
                    ));
                }
                // Door facing the fire barrel, and a warm window.
                house.spawn((
                    Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([0.08, 1.85, 0.9], 1.5)))),
                    MeshMaterial3d(assets.planks_dark.clone()),
                    Transform::from_xyz(w * 0.5 + 0.04, 0.93, 0.0),
                ));
                house.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.55, 0.45, 0.04))),
                    MeshMaterial3d(window.clone()),
                    Transform::from_xyz(0.0, 1.55, d * 0.5 + 0.02),
                    NotShadowCaster,
                ));
                house.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.7, 0.08, 0.1))),
                    MeshMaterial3d(assets.planks_dark.clone()),
                    Transform::from_xyz(0.0, 1.29, d * 0.5 + 0.05),
                ));
                // Stovepipe through the roof, puffing smoke.
                house.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.09, 1.6))),
                    MeshMaterial3d(pipe.clone()),
                    Transform::from_xyz(-0.7, h + gable * 0.5 + 0.3, -0.7),
                ));
                house.spawn((
                    Transform::from_xyz(-0.7, h + gable * 0.5 + 1.15, -0.7),
                    Emitter::new(EmitterKind::ChimneySmoke, 0.35),
                ));
                // Woodpile against the back wall.
                for k in 0..6 {
                    let (row, col) = (k / 3, k % 3);
                    house.spawn((
                        Mesh3d(log.clone()),
                        MeshMaterial3d(assets.bark.clone()),
                        Transform::from_xyz(-0.8 + col as f32 * 0.29 + row as f32 * 0.14, 0.14 + row as f32 * 0.25, -d * 0.5 - 0.3)
                            .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                    ));
                }
            });
        solid.push(Shape::rect_centered(hx, hz, w, d));
        solid.push(Shape::Circle { x: sx + 1.5, z: sz, r: 0.45 });

        // Fire barrel: the Poly Haven barrel stove with live flames.
        prop(&mut commands, &assets.barrel_stove, Vec3::new(sx + 1.5, gy, sz), i as f32 * 1.3, 1.15);
        commands.spawn((
            Mesh3d(meshes.add(Circle::new(0.3))),
            MeshMaterial3d(fire_core.clone()),
            Transform::from_xyz(sx + 1.5, gy + 0.97, sz).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            NotShadowCaster,
        ));
        for k in 0..3 {
            commands.spawn((
                Transform::from_xyz(sx + 1.5 + (k as f32 - 1.0) * 0.1, gy + 1.2, sz + (k as f32 % 2.0) * 0.08),
                Visibility::default(),
                Flame {
                    seed: i as f32 * 3.1 + k as f32 * 1.7,
                    size: 0.55 - k as f32 * 0.1,
                },
            ));
        }
        commands.spawn((
            Transform::from_xyz(sx + 1.5, gy + 1.3, sz),
            Emitter::new(EmitterKind::Embers, 0.12),
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.2),
                intensity: 400_000.0,
                range: 18.0,
                shadows_enabled: i == 0,
                ..default()
            },
            Transform::from_xyz(sx + 1.5, gy + 1.8, sz),
            FireLight {
                base: 400_000.0,
                seed: i as f32 * 2.7,
            },
        ));
        // Cut stumps and a crate of supplies nearby.
        prop(&mut commands, &assets.stump, Vec3::new(sx + 3.5, gy - 0.1, sz - 3.2), i as f32, 0.9);
        solid.push(Shape::Circle { x: sx + 3.5, z: sz - 3.2, r: 0.6 });
    }

    // ================= Golden Atomic Mills =================
    let (gx, gz, _, _) = RAD_SOURCES[1];
    let gzz = gz + 9.0;
    let band = mat(&mut materials, Color::srgb(0.35, 0.22, 0.14));
    for (i, off) in [-5.0_f32, 0.0, 5.0].iter().enumerate() {
        let h = 16.0 + i as f32 * 3.0;
        let x = gx + off;
        let by = ground(x, gzz) - 0.3;
        let body = meshgen::lathe(&[(2.3, 0.0), (2.3, h)], 28, 2.5, false, false);
        let roof = meshgen::lathe(&[(2.45, h - 0.05), (2.45, h + 0.15), (0.35, h + 2.3), (0.25, h + 2.6), (0.0, h + 2.6)], 28, 2.0, false, false);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&body))),
            MeshMaterial3d(assets.corrugated.clone()),
            Transform::from_xyz(x, by, gzz),
        ));
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&roof))),
            MeshMaterial3d(assets.rust.clone()),
            Transform::from_xyz(x, by, gzz),
        ));
        // Snow on the cone.
        commands.spawn((
            Mesh3d(meshes.add(to_mesh(&meshgen::lathe(&[(2.2, 0.0), (0.4, 1.9), (0.0, 2.05)], 20, 2.0, false, false)))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, by + h + 0.45, gzz),
        ));
        let mut y = 3.0;
        while y < h {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(2.36, 0.22))),
                MeshMaterial3d(band.clone()),
                Transform::from_xyz(x, by + y, gzz),
            ));
            y += 4.0;
        }
        solid.push(Shape::Circle { x, z: gzz, r: 2.3 });
    }
    // Catwalk, ladder and the bucket-elevator tower.
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.3, 0.28),
        metallic: 0.8,
        perceptual_roughness: 0.6,
        ..default()
    });
    let gy0 = ground(gx, gzz);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(12.0, 0.15, 1.0))),
        MeshMaterial3d(steel.clone()),
        Transform::from_xyz(gx, gy0 + 15.6, gzz - 2.6),
    ));
    for x in [-6.0f32, 6.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.06, 1.0, 1.0))),
            MeshMaterial3d(steel.clone()),
            Transform::from_xyz(gx + x, gy0 + 16.1, gzz - 2.6),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(12.0, 0.06, 0.06))),
        MeshMaterial3d(steel.clone()),
        Transform::from_xyz(gx, gy0 + 16.6, gzz - 3.1),
    ));
    for side in [-0.25f32, 0.25] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.06, 15.6, 0.06))),
            MeshMaterial3d(steel.clone()),
            Transform::from_xyz(gx - 5.0 + side, gy0 + 7.8, gzz - 2.4),
        ));
    }
    let mut ry = 0.4;
    while ry < 15.5 {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.04, 0.04))),
            MeshMaterial3d(steel.clone()),
            Transform::from_xyz(gx - 5.0, gy0 + ry, gzz - 2.4),
        ));
        ry += 0.4;
    }
    let tower_x = gx + 9.0;
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([1.8, 27.0, 1.8], 2.0)))),
        MeshMaterial3d(assets.rust.clone()),
        Transform::from_xyz(tower_x, ground(tower_x, gzz) + 13.2, gzz),
    ));
    solid.push(Shape::rect_centered(tower_x, gzz, 1.8, 1.8));
    let chute = meshgen::tube(&[[tower_x, gy0 + 25.0, gzz], [gx + 2.0, gy0 + 20.5, gzz], [gx, gy0 + 19.3, gzz]], 0.28, 8);
    commands.spawn((Mesh3d(meshes.add(to_mesh(&chute))), MeshMaterial3d(steel.clone())));
    sign(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets.sign_golden_atomic,
        Vec2::new(8.0, 2.0),
        Transform::from_xyz(gx, gy0 + 10.0, gzz + 2.45),
        0.15,
    );
    // Leaking, glowing drums and old military crates.
    let leak = glow(&mut materials, Color::srgb(0.3, 1.0, 0.3), LinearRgba::rgb(1.0, 8.0, 1.2));
    for (k, (dx, dz)) in [(-3.0, -5.0), (-2.2, -5.6), (2.5, -5.2), (6.5, -3.8), (-7.5, -2.5)].into_iter().enumerate() {
        let (x, z) = (gx + dx, gzz + dz);
        let gy = ground(x, z);
        let tipped = k % 2 == 1;
        let tf = if tipped {
            Transform::from_xyz(x, gy + 0.32, z).with_rotation(Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(k as f32))
        } else {
            Transform::from_xyz(x, gy, z).with_rotation(Quat::from_rotation_y(k as f32))
        };
        commands.spawn((SceneRoot(assets.barrel.clone()), tf));
        commands.spawn((
            Mesh3d(meshes.add(Circle::new(if tipped { 1.1 } else { 0.5 }))),
            MeshMaterial3d(leak.clone()),
            Transform::from_xyz(x, gy + 0.04, z + if tipped { 0.9 } else { 0.0 })
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            NotShadowCaster,
        ));
        solid.push(Shape::Circle { x, z, r: 0.4 });
    }
    for (k, (dx, dz)) in [(4.0, -7.0), (-6.0, -6.5), (8.5, -6.0)].into_iter().enumerate() {
        if k == 0 {
            continue; // the first one is the lootable armoury crate (props.rs)
        }
        let (x, z) = (gx + dx, gzz + dz);
        prop(&mut commands, &assets.crate_military, Vec3::new(x, ground(x, z) + 0.1, z), k as f32 * 0.8, 1.2);
        solid.push(Shape::Circle { x, z, r: 0.7 });
    }
    commands.spawn((Transform::from_xyz(gx, ground(gx, gz), gz), Emitter::new(EmitterKind::RadMotes, 0.05)));
    let (cx, cz, _, _) = RAD_SOURCES[0];
    commands.spawn((Transform::from_xyz(cx, ground(cx, cz), cz), Emitter::new(EmitterKind::RadMotes, 0.05)));

    // ================= Bullseye-Mart ruin =================
    let (bx, bz, _) = RUINS[0];
    let by = ground(bx, bz);
    let wall = |w: f32, h: f32, d: f32, x: f32, z: f32| (Vec3::new(w, h, d), Vec3::new(bx + x, by + h / 2.0 - 0.3, bz + z));
    for (size, pos) in [
        wall(24.0, 6.0, 1.0, 0.0, -8.0),
        wall(1.0, 5.0, 16.0, -12.0, 0.0),
        wall(1.0, 3.5, 10.0, 12.0, -3.0),
        wall(8.0, 6.0, 1.0, -8.0, 8.0),
        wall(5.0, 2.5, 1.0, 9.5, 8.0),
    ] {
        solid.push(Shape::rect_centered(pos.x, pos.z, size.x, size.z));
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid(size.to_array(), 2.5)))),
            MeshMaterial3d(assets.concrete.clone()),
            Transform::from_translation(pos),
        ));
        // Snow on top of the wall and drifted against its base.
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([size.x + 0.1, 0.18, size.z + 0.1], 2.0)))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_translation(pos + Vec3::Y * (size.y / 2.0 + 0.09)),
        ));
    }
    // Collapsed roof slab and twisted steel beams.
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([9.0, 0.5, 7.0], 2.5)))),
        MeshMaterial3d(assets.concrete.clone()),
        Transform::from_xyz(bx - 5.0, by + 2.4, bz - 4.0).with_rotation(Quat::from_rotation_z(0.42) * Quat::from_rotation_x(0.1)),
    ));
    solid.push(Shape::rect_centered(bx - 5.0, bz - 4.0, 8.0, 7.0));
    for (k, (x, z, lean)) in [(3.0, -6.0, 0.3), (6.0, -2.0, -0.5), (-2.0, 4.0, 0.7)].into_iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::cuboid([0.3, 7.0, 0.3], 1.0)))),
            MeshMaterial3d(assets.rust.clone()),
            Transform::from_xyz(bx + x, by + 3.0, bz + z).with_rotation(Quat::from_rotation_z(lean) * Quat::from_rotation_y(k as f32)),
        ));
        solid.push(Shape::Circle { x: bx + x, z: bz + z, r: 0.35 });
    }
    // Rubble piles.
    for k in 0..7 {
        let x = bx + rng.0.range(-9.0, 9.0);
        let z = bz + rng.0.range(-6.0, 6.0);
        let r = rng.0.range(0.8, 1.6);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::blob(r, 0.5, 0.35, 90 + k, 2.0)))),
            MeshMaterial3d(assets.concrete.clone()),
            Transform::from_xyz(x, ground(x, z), z).with_rotation(Quat::from_rotation_y(k as f32)),
        ));
    }
    // The sign and its target logo.
    sign(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets.sign_bullseye,
        Vec2::new(6.0, 3.0),
        Transform::from_xyz(bx - 8.0, by + 7.3, bz + 8.56).with_rotation(Quat::from_rotation_x(-0.08)),
        0.0,
    );
    let red = glow(&mut materials, Color::srgb(0.8, 0.05, 0.05), LinearRgba::rgb(0.6, 0.0, 0.0));
    let white = mat(&mut materials, Color::srgb(0.95, 0.95, 0.95));
    for (radius, material, dz) in [(1.1, red.clone(), 0.0), (0.75, white, 0.03), (0.38, red, 0.06)] {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(radius, 0.05))),
            MeshMaterial3d(material),
            Transform::from_xyz(bx - 8.0, by + 10.0, bz + 8.6 + dz).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ));
    }
    // A tarp-covered car left in the parking lot.
    let (px, pz) = (bx + 5.0, bz + 15.0);
    prop(&mut commands, &assets.covered_car, Vec3::new(px, ground(px, pz) + 0.3, pz), 0.4, 1.0);
    for s in [-1.2f32, 1.2] {
        solid.push(Shape::Circle {
            x: px + 0.4f32.sin() * s,
            z: pz + 0.4f32.cos() * s,
            r: 1.0,
        });
    }

    // ================= US-169 highway =================
    let mut path = Vec::new();
    for w in ROAD.windows(2) {
        let ((ax, az), (bx, bz)) = (w[0], w[1]);
        let n = (((bx - ax).powi(2) + (bz - az).powi(2)).sqrt() / 4.0).ceil() as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            path.push((ax + (bx - ax) * t, az + (bz - az) * t));
        }
    }
    path.push(ROAD[ROAD.len() - 1]);
    let road = meshgen::ground_strip(&path, ROAD_HALF_WIDTH * 2.0, 0.07, 7.0, &terrain::height);
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&road))),
        MeshMaterial3d(assets.asphalt.clone()),
        Transform::default(),
        NotShadowCaster,
    ));
    // Plough-banks of snow along the shoulders.
    let mut along = 0.0;
    for i in 1..path.len() {
        let (ax, az) = path[i - 1];
        let (bx2, bz2) = path[i];
        along += ((bx2 - ax).powi(2) + (bz2 - az).powi(2)).sqrt();
        if along < 9.0 {
            continue;
        }
        along = 0.0;
        let side = if rng.0.chance(0.5) { 1.0 } else { -1.0 };
        let (x, z) = (bx2, bz2 + side * (ROAD_HALF_WIDTH + 1.3));
        let r = rng.0.range(1.5, 3.0);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::blob(r, 0.3, 0.2, i as u64, 2.0).scaled([1.6, 1.0, 0.8])))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::from_xyz(x, ground(x, z) - 0.15, z),
            NotShadowCaster,
        ));
    }

    // Rusted 1950s sedans.
    let body = meshes.add(to_mesh_tangents(&meshgen::car_body(1.5)));
    let cabin = meshes.add(to_mesh(&meshgen::car_cabin(1.5)));
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.1, 0.12),
        metallic: 0.2,
        perceptual_roughness: 0.15,
        reflectance: 0.8,
        ..default()
    });
    let chrome = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.5, 0.45),
        metallic: 0.9,
        perceptual_roughness: 0.45,
        ..default()
    });
    let car_snow = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.18, 0.2, 5, 1.5)));
    let paints = [
        Color::srgb(0.75, 0.45, 0.35),
        Color::srgb(0.45, 0.6, 0.65),
        Color::srgb(0.8, 0.72, 0.5),
        Color::srgb(0.55, 0.65, 0.5),
    ];
    for (i, (x, z, yaw)) in CARS.iter().enumerate() {
        let gy = ground(*x, *z);
        let (ax, az) = (yaw.sin() * 1.2, yaw.cos() * 1.2);
        solid.push(Shape::Circle { x: x + ax, z: z + az, r: 1.0 });
        solid.push(Shape::Circle { x: x - ax, z: z - az, r: 1.0 });
        let paint = materials.add(StandardMaterial {
            base_color: paints[i % paints.len()],
            base_color_texture: Some(assets.rust_diff.clone()),
            normal_map_texture: Some(assets.rust_normal.clone()),
            perceptual_roughness: 0.8,
            metallic: 0.3,
            ..default()
        });
        let on_rims = i % 3 == 2;
        let sink = if on_rims { 0.22 } else { 0.0 };
        commands
            .spawn((
                Transform::from_xyz(*x, gy - sink, *z).with_rotation(Quat::from_rotation_y(*yaw) * Quat::from_rotation_z(if on_rims { 0.04 } else { 0.0 })),
                Visibility::default(),
            ))
            .with_children(|car| {
                car.spawn((Mesh3d(body.clone()), MeshMaterial3d(paint.clone())));
                car.spawn((Mesh3d(cabin.clone()), MeshMaterial3d(glass.clone())));
                // Snow piled on the roof, hood and trunk.
                for (pos, s) in [
                    (Vec3::new(0.0, 1.55, -0.12), Vec3::new(0.72, 1.0, 0.95)),
                    (Vec3::new(0.0, 1.07, 1.6), Vec3::new(0.85, 0.8, 0.7)),
                    (Vec3::new(0.0, 1.09, -1.75), Vec3::new(0.85, 0.8, 0.55)),
                ] {
                    car.spawn((
                        Mesh3d(car_snow.clone()),
                        MeshMaterial3d(assets.snow.clone()),
                        Transform::from_translation(pos).with_scale(s),
                    ));
                }
                for zb in [-2.33f32, 2.33] {
                    car.spawn((
                        Mesh3d(meshes.add(Cuboid::new(1.95, 0.14, 0.12))),
                        MeshMaterial3d(chrome.clone()),
                        Transform::from_xyz(0.0, 0.58, zb),
                    ));
                }
                for xh in [-0.62f32, 0.62] {
                    car.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.12, 0.05))),
                        MeshMaterial3d(chrome.clone()),
                        Transform::from_xyz(xh, 0.78, 2.29).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                    ));
                }
                for (wx, wz) in [(-0.86, 1.45), (0.86, 1.45), (-0.86, -1.35), (0.86, -1.35)] {
                    let scene = if on_rims { assets.rim.clone() } else { assets.tyre.clone() };
                    let s = if on_rims { 1.3 } else { 1.0 };
                    car.spawn((
                        SceneRoot(scene),
                        Transform::from_xyz(wx, 0.3, wz)
                            .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
                            .with_scale(Vec3::splat(s)),
                    ));
                }
            });
    }

    // Road sign pointing travellers to Mille Lacs and the vault.
    let sx = -14.0;
    let sz = terrain::road_z(sx) + ROAD_HALF_WIDTH + 2.0;
    let sy = ground(sx, sz);
    let post = mat(&mut materials, Color::srgb(0.45, 0.45, 0.45));
    for dx in [-1.2f32, 1.2] {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.05, 3.4))),
            MeshMaterial3d(post.clone()),
            Transform::from_xyz(sx + dx, sy + 1.7, sz - 0.05),
        ));
    }
    solid.push(Shape::Circle { x: sx, z: sz, r: 0.3 });
    sign(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets.sign_mille_lacs,
        Vec2::new(3.2, 1.6),
        Transform::from_xyz(sx, sy + 2.8, sz).with_rotation(Quat::from_rotation_z(0.06)),
        0.0,
    );

    // ================= Power lines =================
    let pole_mesh = meshes.add(to_mesh_tangents(&meshgen::lathe(&[(0.17, -0.5), (0.12, 9.0), (0.0, 9.05)], 8, 1.0, false, false)));
    let arm_mesh = meshes.add(Cuboid::new(2.6, 0.14, 0.14));
    let insulator = meshes.add(to_mesh(&meshgen::lathe(&[(0.06, 0.0), (0.09, 0.08), (0.05, 0.12), (0.08, 0.2), (0.0, 0.24)], 8, 1.0, false, false)));
    let insulator_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 0.4),
        perceptual_roughness: 0.15,
        ..default()
    });
    let wire_mat = mat(&mut materials, Color::srgb(0.06, 0.06, 0.06));
    let mut prev: Option<[Vec3; 3]> = None;
    let mut x = -190.0;
    let mut k = 0;
    while x < 195.0 {
        let z = terrain::road_z(x) + ROAD_HALF_WIDTH + 5.5;
        let blocked = collision::blocked(x, z, 0.8, solid) || terrain::lake_at(x, z).is_some();
        if blocked {
            prev = None;
            x += 30.0;
            continue;
        }
        let gy = ground(x, z);
        // Every few poles leans after decades of frost heave.
        let lean = if k % 5 == 3 { 0.16 } else { rng.0.range(-0.03, 0.03) };
        let tf = Transform::from_xyz(x, gy, z).with_rotation(Quat::from_rotation_x(lean));
        commands.spawn((Mesh3d(pole_mesh.clone()), MeshMaterial3d(assets.pole_wood.clone()), tf));
        commands.spawn((
            Mesh3d(arm_mesh.clone()),
            MeshMaterial3d(assets.pole_wood.clone()),
            tf * Transform::from_xyz(0.0, 8.4, 0.0),
        ));
        let mut tips = [Vec3::ZERO; 3];
        for (j, ax) in [-1.15f32, 0.0, 1.15].into_iter().enumerate() {
            let local = Transform::from_xyz(ax, 8.47, 0.0);
            commands.spawn((Mesh3d(insulator.clone()), MeshMaterial3d(insulator_mat.clone()), tf * local));
            tips[j] = (tf * Transform::from_xyz(ax, 8.7, 0.0)).translation;
        }
        if let Some(p) = prev {
            for j in 0..3 {
                let pts: Vec<[f32; 3]> = meshgen::catenary(p[j].to_array(), tips[j].to_array(), 1.1, 16);
                commands.spawn((
                    Mesh3d(meshes.add(to_mesh(&meshgen::tube(&pts, 0.018, 4)))),
                    MeshMaterial3d(wire_mat.clone()),
                    NotShadowCaster,
                ));
            }
        }
        if k % 4 == 1 {
            prop(&mut commands, &assets.utility_box, Vec3::new(x + 0.8, gy, z + 0.3), 0.0, 1.0);
        }
        solid.push(Shape::Circle { x, z, r: 0.3 });
        prev = Some(tips);
        x += 30.0;
        k += 1;
    }
}
