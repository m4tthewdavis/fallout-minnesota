//! Builds the map: snowy terrain, nuclear ice lakes, Vault 143, fish-house
//! shelters, pines, pre-war ruins, radiation hot spots and loot.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::player::Player;
use crate::sim::collision::{self, Shape};
use crate::sim::survival::Item;
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE, ICE_FRACTION, ICE_LEVEL, LAKES, RAD_SOURCES, SHELTERS, VAULT_POS};
use crate::state::{alive, Colliders, Game, Messages, RngRes, SfxQueue};

/// The directional "sun" light, dimmed by the weather.
#[derive(Component)]
pub struct Sun;

#[derive(Component)]
pub struct Pickup {
    pub item: Item,
    pub base_y: f32,
}

/// Pre-war landmarks trees should stay away from: (x, z, clear radius).
const RUINS: [(f32, f32, f32); 2] = [(-40.0, -110.0, 20.0), (140.0, -140.0, 18.0)];
const CARS: [(f32, f32, f32); 6] = [
    (-150.0, 95.0, 0.2),
    (-128.0, 92.0, 1.4),
    (-60.0, 98.0, 0.1),
    (60.0, 140.0, 2.6),
    (150.0, 100.0, 0.9),
    (165.0, 20.0, 1.7),
];

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_world)
            .add_systems(Update, (animate_pickups, collect_pickups.run_if(alive)));
    }
}

fn mat(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    })
}

fn glow(materials: &mut Assets<StandardMaterial>, color: Color, emissive: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        emissive,
        perceptual_roughness: 0.4,
        ..default()
    })
}

fn ground(x: f32, z: f32) -> f32 {
    terrain::height(x, z)
}

fn build_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;

    // ---------- Terrain ----------
    let mut terrain_mesh: Mesh = Plane3d::default()
        .mesh()
        .size(HALF_SIZE * 2.0, HALF_SIZE * 2.0)
        .subdivisions(160)
        .into();
    let flat: Vec<[f32; 3]> = terrain_mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|a| a.as_float3())
        .map(|p| p.to_vec())
        .unwrap_or_default();
    let positions: Vec<[f32; 3]> = flat.iter().map(|p| [p[0], terrain::height(p[0], p[2]), p[2]]).collect();
    let normals: Vec<[f32; 3]> = flat.iter().map(|p| terrain::normal(p[0], p[2])).collect();
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);

    let snow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.92, 0.96),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh)),
        MeshMaterial3d(snow),
        Transform::default(),
    ));

    // ---------- Sun ----------
    commands.spawn((
        DirectionalLight {
            illuminance: 6_000.0,
            shadows_enabled: true,
            color: Color::srgb(0.95, 0.96, 1.0),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.4, -1.0, -0.3), Vec3::Y),
        Sun,
    ));

    // ---------- Nuclear ice lakes ----------
    let ice = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.85, 0.72),
        emissive: LinearRgba::rgb(0.04, 0.45, 0.15),
        perceptual_roughness: 0.12,
        reflectance: 0.6,
        ..default()
    });
    for (lx, lz, r) in LAKES {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(r * ICE_FRACTION, 0.1))),
            MeshMaterial3d(ice.clone()),
            Transform::from_xyz(lx, ICE_LEVEL - 0.05, lz),
        ));
    }

    // ---------- Vault 143 ----------
    let (vx, vz) = VAULT_POS;
    let vy = ground(vx, vz - 8.0);
    let rock = mat(&mut materials, Color::srgb(0.42, 0.43, 0.45));
    let vault_grey = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.56, 0.52),
        metallic: 0.7,
        perceptual_roughness: 0.45,
        ..default()
    });
    let vault_yellow = glow(
        &mut materials,
        Color::srgb(0.85, 0.68, 0.10),
        LinearRgba::rgb(0.25, 0.18, 0.0),
    );
    let door_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    // Hillside the door is cut into.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(26.0, 14.0, 10.0))),
        MeshMaterial3d(rock.clone()),
        Transform::from_xyz(vx, vy + 5.0, vz + 4.0),
    ));
    solid.push(Shape::rect_centered(vx, vz + 4.0, 26.0, 10.0));
    solid.push(Shape::Rect {
        x0: vx + 4.5,
        z0: vz - 1.9,
        x1: vx + 12.5,
        z1: vz - 0.9,
    });
    // The gear door, rolled aside to the right.
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(4.0, 0.9))),
        MeshMaterial3d(vault_grey.clone()),
        Transform::from_xyz(vx + 8.5, vy + 4.2, vz - 1.4).with_rotation(door_rot),
    ));
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.8, 0.8, 1.0))),
            MeshMaterial3d(vault_grey.clone()),
            Transform::from_xyz(vx + 8.5 + a.cos() * 4.2, vy + 4.2 + a.sin() * 4.2, vz - 1.4),
        ));
    }
    // Yellow frame around the open doorway, plus a dark tunnel.
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(4.0, 4.9))),
        MeshMaterial3d(vault_yellow.clone()),
        Transform::from_xyz(vx, vy + 4.2, vz - 1.0).with_rotation(door_rot),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(4.0, 0.2))),
        MeshMaterial3d(mat(&mut materials, Color::srgb(0.05, 0.05, 0.06))),
        Transform::from_xyz(vx, vy + 4.2, vz - 0.7).with_rotation(door_rot),
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

    // ---------- Fish-house shelters with fire barrels ----------
    let house = mat(&mut materials, Color::srgb(0.62, 0.16, 0.12));
    let roof = mat(&mut materials, Color::srgb(0.20, 0.18, 0.17));
    let barrel = mat(&mut materials, Color::srgb(0.30, 0.22, 0.15));
    let fire = glow(
        &mut materials,
        Color::srgb(1.0, 0.5, 0.1),
        LinearRgba::rgb(8.0, 3.0, 0.4),
    );
    for (sx, sz) in SHELTERS {
        let gy = ground(sx, sz);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(3.0, 2.4, 3.0))),
            MeshMaterial3d(house.clone()),
            Transform::from_xyz(sx - 1.5, gy + 1.2, sz),
        ));
        solid.push(Shape::rect_centered(sx - 1.5, sz, 3.0, 3.0));
        solid.push(Shape::Circle {
            x: sx + 1.5,
            z: sz,
            r: 0.45,
        });
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(3.5, 0.3, 3.5))),
            MeshMaterial3d(roof.clone()),
            Transform::from_xyz(sx - 1.5, gy + 2.55, sz),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.4, 1.0))),
            MeshMaterial3d(barrel.clone()),
            Transform::from_xyz(sx + 1.5, gy + 0.5, sz),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.35, 0.05))),
            MeshMaterial3d(fire.clone()),
            Transform::from_xyz(sx + 1.5, gy + 1.02, sz),
            NotShadowCaster,
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.2),
                intensity: 400_000.0,
                range: 18.0,
                ..default()
            },
            Transform::from_xyz(sx + 1.5, gy + 1.8, sz),
        ));
    }

    // ---------- Radiation hot spots ----------
    let scorched = glow(
        &mut materials,
        Color::srgb(0.10, 0.14, 0.08),
        LinearRgba::rgb(0.02, 0.25, 0.03),
    );
    let warhead = glow(
        &mut materials,
        Color::srgb(0.35, 0.45, 0.30),
        LinearRgba::rgb(0.2, 2.5, 0.3),
    );
    for (cx, cz, r, _) in RAD_SOURCES {
        let gy = ground(cx, cz);
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(r * 0.8, 0.12))),
            MeshMaterial3d(scorched.clone()),
            Transform::from_xyz(cx, gy + 0.02, cz),
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.3, 1.0, 0.3),
                intensity: 600_000.0,
                range: r * 1.6,
                ..default()
            },
            Transform::from_xyz(cx, gy + 3.0, cz),
        ));
    }
    // A half-buried dud warhead in the first crater.
    let (cx, cz, _, _) = RAD_SOURCES[0];
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(0.9, 3.5))),
        MeshMaterial3d(warhead),
        Transform::from_xyz(cx, ground(cx, cz) + 0.8, cz).with_rotation(Quat::from_rotation_z(1.1)),
    ));
    solid.push(Shape::Circle { x: cx, z: cz, r: 1.4 });
    // Golden Atomic Mills grain silos, leaking at the second hot spot.
    let (gx, gz, _, _) = RAD_SOURCES[1];
    let rust = mat(&mut materials, Color::srgb(0.55, 0.38, 0.25));
    for (i, off) in [-5.0_f32, 0.0, 5.0].iter().enumerate() {
        let h = 16.0 + i as f32 * 3.0;
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(2.3, h))),
            MeshMaterial3d(rust.clone()),
            Transform::from_xyz(gx + off, ground(gx + off, gz + 9.0) + h / 2.0, gz + 9.0),
        ));
        solid.push(Shape::Circle {
            x: gx + off,
            z: gz + 9.0,
            r: 2.3,
        });
    }

    // ---------- Bullseye-Mart ruin ----------
    let (bx, bz, _) = RUINS[0];
    let by = ground(bx, bz);
    let concrete = mat(&mut materials, Color::srgb(0.58, 0.56, 0.53));
    let wall = |w: f32, h: f32, d: f32, x: f32, z: f32| {
        (
            Cuboid::new(w, h, d),
            Transform::from_xyz(bx + x, by + h / 2.0 - 0.3, bz + z),
        )
    };
    for (shape, tf) in [
        wall(24.0, 6.0, 1.0, 0.0, -8.0),
        wall(1.0, 5.0, 16.0, -12.0, 0.0),
        wall(1.0, 3.5, 10.0, 12.0, -3.0),
        wall(8.0, 6.0, 1.0, -8.0, 8.0),
        wall(5.0, 2.5, 1.0, 9.5, 8.0),
    ] {
        let size = shape.half_size * 2.0;
        solid.push(Shape::rect_centered(tf.translation.x, tf.translation.z, size.x, size.z));
        commands.spawn((Mesh3d(meshes.add(shape)), MeshMaterial3d(concrete.clone()), tf));
    }
    let red = glow(
        &mut materials,
        Color::srgb(0.8, 0.05, 0.05),
        LinearRgba::rgb(0.6, 0.0, 0.0),
    );
    let white = mat(&mut materials, Color::srgb(0.95, 0.95, 0.95));
    let sign_rot = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    for (radius, material, dz) in [(2.2, red.clone(), 0.0), (1.5, white, 0.06), (0.75, red, 0.12)] {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(radius, 0.1))),
            MeshMaterial3d(material),
            Transform::from_xyz(bx - 8.0, by + 7.6, bz + 8.6 + dz).with_rotation(sign_rot),
        ));
    }

    // ---------- Rusted pre-war cars ----------
    let car_colors = [
        Color::srgb(0.45, 0.30, 0.22),
        Color::srgb(0.30, 0.40, 0.45),
        Color::srgb(0.50, 0.45, 0.30),
    ];
    for (i, (x, z, yaw)) in CARS.iter().enumerate() {
        let gy = ground(*x, *z);
        // Two circles along the car's length approximate its rotated body.
        let (ax, az) = (yaw.sin() * 1.2, yaw.cos() * 1.2);
        solid.push(Shape::Circle {
            x: x + ax,
            z: z + az,
            r: 1.0,
        });
        solid.push(Shape::Circle {
            x: x - ax,
            z: z - az,
            r: 1.0,
        });
        let paint = mat(&mut materials, car_colors[i % car_colors.len()]);
        commands
            .spawn((
                Transform::from_xyz(*x, gy, *z).with_rotation(Quat::from_rotation_y(*yaw)),
                Visibility::default(),
            ))
            .with_children(|car| {
                car.spawn((
                    Mesh3d(meshes.add(Cuboid::new(1.9, 1.0, 4.4))),
                    MeshMaterial3d(paint.clone()),
                    Transform::from_xyz(0.0, 0.7, 0.0),
                ));
                car.spawn((
                    Mesh3d(meshes.add(Cuboid::new(1.7, 0.8, 2.0))),
                    MeshMaterial3d(paint.clone()),
                    Transform::from_xyz(0.0, 1.55, -0.3),
                ));
            });
    }

    // ---------- Northwoods pines ----------
    let trunk_mesh = meshes.add(Cylinder::new(0.25, 2.0));
    let low_cone = meshes.add(Cone {
        radius: 1.7,
        height: 3.2,
    });
    let high_cone = meshes.add(Cone {
        radius: 1.1,
        height: 2.6,
    });
    let bark = mat(&mut materials, Color::srgb(0.25, 0.17, 0.11));
    let needles = mat(&mut materials, Color::srgb(0.12, 0.24, 0.17));
    let mut placed = 0;
    let mut attempts = 0;
    while placed < 260 && attempts < 5_000 {
        attempts += 1;
        let x = rng.0.range(-HALF_SIZE, HALF_SIZE);
        let z = rng.0.range(-HALF_SIZE, HALF_SIZE);
        if !terrain::is_open_ground(x, z) {
            continue;
        }
        if RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r)
            || CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < 5.0)
        {
            continue;
        }
        placed += 1;
        let scale = rng.0.range(0.8, 1.7);
        solid.push(Shape::Circle { x, z, r: 0.4 * scale });
        commands
            .spawn((
                Transform::from_xyz(x, ground(x, z) - 0.1, z)
                    .with_scale(Vec3::splat(scale))
                    .with_rotation(Quat::from_rotation_y(rng.0.range(0.0, 6.28))),
                Visibility::default(),
            ))
            .with_children(|tree| {
                tree.spawn((
                    Mesh3d(trunk_mesh.clone()),
                    MeshMaterial3d(bark.clone()),
                    Transform::from_xyz(0.0, 1.0, 0.0),
                ));
                tree.spawn((
                    Mesh3d(low_cone.clone()),
                    MeshMaterial3d(needles.clone()),
                    Transform::from_xyz(0.0, 3.2, 0.0),
                ));
                tree.spawn((
                    Mesh3d(high_cone.clone()),
                    MeshMaterial3d(needles.clone()),
                    Transform::from_xyz(0.0, 4.9, 0.0),
                ));
            });
    }

    // ---------- Loot ----------
    let stim_mesh = meshes.add(Cylinder::new(0.12, 0.5));
    let bag_mesh = meshes.add(Cuboid::new(0.35, 0.45, 0.15));
    let ammo_mesh = meshes.add(Cuboid::new(0.5, 0.3, 0.3));
    let dish_mesh = meshes.add(Cylinder::new(0.3, 0.18));
    let stim_mat = glow(
        &mut materials,
        Color::srgb(0.9, 0.2, 0.2),
        LinearRgba::rgb(0.6, 0.05, 0.05),
    );
    let bag_mat = glow(
        &mut materials,
        Color::srgb(0.95, 0.55, 0.1),
        LinearRgba::rgb(0.6, 0.3, 0.0),
    );
    let ammo_mat = glow(
        &mut materials,
        Color::srgb(0.35, 0.40, 0.2),
        LinearRgba::rgb(0.1, 0.15, 0.0),
    );
    let dish_mat = glow(
        &mut materials,
        Color::srgb(0.6, 0.45, 0.2),
        LinearRgba::rgb(0.35, 0.2, 0.05),
    );

    let loot_table = [Item::Stimpak, Item::RadAway, Item::Ammo, Item::Ammo, Item::Hotdish];
    let mut spawned = 0;
    attempts = 0;
    while spawned < 34 && attempts < 2_000 {
        attempts += 1;
        let x = rng.0.range(-HALF_SIZE + 10.0, HALF_SIZE - 10.0);
        let z = rng.0.range(-HALF_SIZE + 10.0, HALF_SIZE - 10.0);
        if terrain::lake_at(x, z).is_some() || collision::blocked(x, z, 0.6, solid) {
            continue;
        }
        let item = loot_table[(rng.0.f32() * loot_table.len() as f32) as usize % loot_table.len()];
        let (mesh, material) = match item {
            Item::Stimpak => (stim_mesh.clone(), stim_mat.clone()),
            Item::RadAway => (bag_mesh.clone(), bag_mat.clone()),
            Item::Ammo => (ammo_mesh.clone(), ammo_mat.clone()),
            Item::Hotdish => (dish_mesh.clone(), dish_mat.clone()),
        };
        let base_y = terrain::walk_height(x, z) + 0.6;
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(x, base_y, z),
            Pickup { item, base_y },
        ));
        spawned += 1;
    }
    // A guaranteed welcome kit outside the vault door.
    for (i, item) in [Item::Ammo, Item::Hotdish].into_iter().enumerate() {
        let (x, z) = (VAULT_POS.0 - 4.0 + i as f32 * 8.0, VAULT_POS.1 - 18.0);
        let base_y = ground(x, z) + 0.6;
        let (mesh, material) = match item {
            Item::Ammo => (ammo_mesh.clone(), ammo_mat.clone()),
            _ => (dish_mesh.clone(), dish_mat.clone()),
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(x, base_y, z),
            Pickup { item, base_y },
        ));
    }
}

fn animate_pickups(time: Res<Time>, mut q: Query<(&mut Transform, &Pickup)>) {
    let t = time.elapsed_secs();
    for (mut tf, pickup) in &mut q {
        tf.rotation = Quat::from_rotation_y(t * 1.5);
        tf.translation.y = pickup.base_y + (t * 2.0 + tf.translation.x).sin() * 0.12;
    }
}

fn collect_pickups(
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
    pickups: Query<(Entity, &Transform, &Pickup), Without<Player>>,
) {
    let Ok(ptf) = player.single() else { return };
    for (entity, tf, pickup) in &pickups {
        let d = tf.translation - ptf.translation;
        if d.x * d.x + d.z * d.z < 2.2 * 2.2 && d.y.abs() < 3.0 {
            game.inv.add(pickup.item);
            sfx.play(Sound::Pickup);
            let extra = if pickup.item == Item::Ammo { " (+12)" } else { "" };
            msgs.show(format!("Picked up: {}{}", pickup.item.name(), extra), 2.5);
            commands.entity(entity).despawn();
        }
    }
}
