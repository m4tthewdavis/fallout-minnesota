//! Builds the map: textured snowy terrain, glowing cracked nuclear-ice lakes,
//! radiation craters and loot. Landmarks live in `landmarks.rs` and trees,
//! rocks and junk in `nature.rs`. Also animates the lights that make the
//! world feel alive: flickering fires, pulsing radiation and blinking lamps.

use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::player::Player;
use crate::sim::collision;
use crate::sim::meshgen::{self, MeshData};
use crate::sim::survival::Item;
use crate::sim::sfx;
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

/// A point light that flickers like fire.
#[derive(Component)]
pub struct FireLight {
    pub base: f32,
    pub seed: f32,
}

/// A point light that slowly breathes (radiation glow).
#[derive(Component)]
pub struct PulseLight {
    pub base: f32,
    pub speed: f32,
}

/// Toggles visibility on and off (warning lamps).
#[derive(Component)]
pub struct Blinker {
    pub period: f32,
    pub offset: f32,
}

/// The nuclear-ice material, whose cracks pulse with a sickly glow.
#[derive(Resource)]
struct IceGlow(Handle<StandardMaterial>);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                build_world,
                crate::landmarks::spawn_landmarks,
                crate::nature::spawn_nature,
                spawn_loot,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                animate_pickups,
                collect_pickups.run_if(alive),
                flicker_fires,
                pulse_lights,
                blink,
                pulse_ice,
            ),
        );
    }
}

pub fn ground(x: f32, z: f32) -> f32 {
    terrain::height(x, z)
}

pub fn mat(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    })
}

pub fn glow(materials: &mut Assets<StandardMaterial>, color: Color, emissive: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        emissive,
        perceptual_roughness: 0.4,
        ..default()
    })
}

/// Spawns a glTF model.
pub fn prop(commands: &mut Commands, scene: &Handle<Scene>, pos: Vec3, yaw: f32, scale: f32) -> Entity {
    commands
        .spawn((
            SceneRoot(scene.clone()),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .id()
}

/// Snow tint for the terrain: bluer in hollows, trampled near shelters, slushy
/// along the highway and sickly green around the radiation craters.
fn terrain_tint(x: f32, z: f32, h: f32) -> [f32; 4] {
    let mut c = [0.97f32, 0.98, 1.0];
    let hollow = ((-h - 0.5) / 3.0).clamp(0.0, 1.0);
    c = [c[0] - 0.08 * hollow, c[1] - 0.05 * hollow, c[2]];
    let ripple = 0.03 * ((x * 0.31).sin() * (z * 0.27).cos() + (x * 0.07 + z * 0.05).sin());
    c = [c[0] + ripple, c[1] + ripple, c[2] + ripple * 0.5];
    for &(sx, sz) in &SHELTERS {
        let t = (1.0 - (x - sx).hypot(z - sz) / 9.0).clamp(0.0, 1.0);
        c = [c[0] - 0.18 * t, c[1] - 0.2 * t, c[2] - 0.22 * t];
    }
    let road = (1.0 - (terrain::road_distance(x, z) - terrain::ROAD_HALF_WIDTH) / 4.0).clamp(0.0, 1.0);
    c = [c[0] - 0.15 * road, c[1] - 0.15 * road, c[2] - 0.13 * road];
    for &(cx, cz, r, _) in &RAD_SOURCES {
        let t = (1.0 - (x - cx).hypot(z - cz) / (r * 1.6)).clamp(0.0, 1.0);
        c = [c[0] - 0.35 * t, c[1] - 0.12 * t, c[2] - 0.4 * t];
    }
    [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), 1.0]
}

fn build_world(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<Colliders>,
) {
    let solid = &mut colliders.0;

    // ---------- Terrain ----------
    let terrain_data = meshgen::ground_patch(0.0, 0.0, HALF_SIZE, 200, 0.0, Some(4.0), &terrain::height);
    let mut terrain_data = MeshData {
        normals: terrain_data
            .positions
            .iter()
            .map(|p| terrain::normal(p[0], p[2]))
            .collect(),
        ..terrain_data
    };
    for (i, p) in terrain_data.positions.iter().enumerate() {
        terrain_data.colors[i] = terrain_tint(p[0], p[2], p[1]);
    }
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&terrain_data))),
        MeshMaterial3d(assets.snow.clone()),
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
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: 14.0,
            maximum_distance: 140.0,
            ..default()
        }
        .build(),
        // The sun lights the world and the first-person rifle.
        bevy::render::view::RenderLayers::from_layers(&[0, crate::gun::VIEW_LAYER]),
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.4, -1.0, -0.3), Vec3::Y),
        Sun,
    ));

    // ---------- Nuclear ice lakes ----------
    let ice = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 1.0, 0.95),
        base_color_texture: Some(assets.ice_diff.clone()),
        emissive: LinearRgba::rgb(0.6, 1.6, 0.8),
        emissive_texture: Some(assets.ice_emissive.clone()),
        perceptual_roughness: 0.08,
        reflectance: 0.7,
        ..default()
    });
    commands.insert_resource(IceGlow(ice.clone()));
    for (lx, lz, r) in LAKES {
        commands.spawn((
            Mesh3d(meshes.add(to_mesh(&meshgen::disc(r * ICE_FRACTION, 64, 9.0)))),
            MeshMaterial3d(ice.clone()),
            Transform::from_xyz(lx, ICE_LEVEL, lz),
            NotShadowCaster,
        ));
        // Snowy rim where the ice meets the shore.
        let rim: Vec<(f32, f32)> = (0..=48)
            .map(|i| {
                let a = i as f32 / 48.0 * std::f32::consts::TAU;
                let rr = r * ICE_FRACTION + 0.4 + 0.5 * (a * 7.0).sin();
                (lx + rr * a.cos(), lz - rr * a.sin())
            })
            .collect();
        let rim_mesh = meshgen::ground_strip(&rim, 2.2, 0.05, 3.0, &|x, z| terrain::height(x, z).max(ICE_LEVEL));
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&rim_mesh))),
            MeshMaterial3d(assets.snow.clone()),
            Transform::default(),
            NotShadowCaster,
        ));
    }

    // ---------- Radiation hot spots ----------
    let scorch = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(assets.scorch.clone()),
        emissive: LinearRgba::rgb(1.5, 3.0, 1.5),
        emissive_texture: Some(assets.scorch.clone()),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        depth_bias: 10.0,
        ..default()
    });
    for (cx, cz, r, _) in RAD_SOURCES {
        let patch = meshgen::ground_patch(cx, cz, r * 1.1, 24, 0.06, None, &terrain::height);
        commands.spawn((
            Mesh3d(meshes.add(to_mesh(&patch))),
            MeshMaterial3d(scorch.clone()),
            Transform::default(),
            NotShadowCaster,
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.3, 1.0, 0.3),
                intensity: 600_000.0,
                range: r * 1.6,
                ..default()
            },
            Transform::from_xyz(cx, ground(cx, cz) + 3.0, cz),
            PulseLight {
                base: 600_000.0,
                speed: 0.8 + r * 0.02,
            },
        ));
    }

    // A half-buried dud warhead in the first crater: body, nose cone and fins.
    let (cx, cz, _, _) = RAD_SOURCES[0];
    let warhead_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.48, 0.38),
        emissive: LinearRgba::rgb(0.05, 0.6, 0.08),
        metallic: 0.6,
        perceptual_roughness: 0.5,
        ..default()
    });
    let band_mat = glow(&mut materials, Color::srgb(0.8, 0.7, 0.1), LinearRgba::rgb(0.3, 0.25, 0.0));
    let core = glow(&mut materials, Color::srgb(0.4, 1.0, 0.4), LinearRgba::rgb(2.0, 14.0, 2.5));
    let body = meshgen::lathe(
        &[(0.0, -2.2), (0.6, -2.0), (0.85, -1.4), (0.9, 1.0), (0.75, 1.6), (0.35, 2.2), (0.0, 2.4)],
        20,
        1.0,
        false,
        false,
    );
    commands
        .spawn((
            Transform::from_xyz(cx, ground(cx, cz) + 0.6, cz)
                .with_rotation(Quat::from_rotation_z(1.1) * Quat::from_rotation_x(0.2)),
            Visibility::default(),
        ))
        .with_children(|w| {
            w.spawn((Mesh3d(meshes.add(to_mesh(&body))), MeshMaterial3d(warhead_mat.clone())));
            w.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.92, 0.25))),
                MeshMaterial3d(band_mat),
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
            for k in 0..4 {
                let a = k as f32 * std::f32::consts::FRAC_PI_2;
                w.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.08, 0.9, 0.7))),
                    MeshMaterial3d(warhead_mat.clone()),
                    Transform::from_xyz(a.cos() * 0.9, -1.7, a.sin() * 0.9).with_rotation(Quat::from_rotation_y(-a)),
                ));
            }
            // A crack in the casing leaking light.
            w.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 1.2, 0.05))),
                MeshMaterial3d(core),
                Transform::from_xyz(0.0, -0.3, 0.88),
                NotShadowCaster,
            ));
        });
    solid.push(collision::Shape::Circle { x: cx, z: cz, r: 1.4 });
}

fn spawn_loot(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    colliders: Res<Colliders>,
) {
    // RadAway: a glowing orange IV bag. Hotdish: a covered casserole dish.
    let bag = meshgen::blob(0.2, 1.3, 0.05, 5, 1.0).scaled([1.0, 1.0, 0.35]);
    let bag_mesh = meshes.add(to_mesh(&bag));
    let bag_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.55, 0.1, 0.85),
        emissive: LinearRgba::rgb(1.2, 0.45, 0.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.2,
        ..default()
    });
    let dish = meshgen::lathe(
        &[(0.0, 0.0), (0.26, 0.0), (0.3, 0.12), (0.32, 0.13), (0.26, 0.2), (0.12, 0.25), (0.05, 0.3), (0.0, 0.3)],
        16,
        1.0,
        false,
        false,
    );
    let dish_mesh = meshes.add(to_mesh(&dish));
    let dish_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.45, 0.2),
        emissive: LinearRgba::rgb(0.25, 0.12, 0.02),
        perceptual_roughness: 0.3,
        ..default()
    });
    // A faint Pip-Boy-green ring marks every pickup so it can be spotted in snow.
    let ring_mesh = meshes.add(Annulus::new(0.45, 0.55));
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 1.0, 0.4, 0.5),
        emissive: LinearRgba::rgb(0.6, 2.5, 0.6),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });

    let spawn = |commands: &mut Commands, item: Item, x: f32, z: f32| {
        let floor = terrain::walk_height(x, z);
        let base_y = floor + 0.6;
        let id = commands
            .spawn((Transform::from_xyz(x, base_y, z), Visibility::default(), Pickup { item, base_y }))
            .id();
        commands.entity(id).with_children(|p| match item {
            Item::Stimpak => {
                p.spawn((
                    SceneRoot(assets.medical_box.clone()),
                    Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::splat(0.9)),
                ));
            }
            Item::RadAway => {
                p.spawn((Mesh3d(bag_mesh.clone()), MeshMaterial3d(bag_mat.clone()), Transform::default()));
            }
            Item::Ammo => {
                p.spawn((
                    SceneRoot(assets.ammo_box.clone()),
                    Transform::from_xyz(0.0, -0.15, 0.0).with_scale(Vec3::splat(2.0)),
                ));
            }
            Item::Hotdish => {
                p.spawn((Mesh3d(dish_mesh.clone()), MeshMaterial3d(dish_mat.clone()), Transform::default()));
            }
        });
        commands.spawn((
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_xyz(x, floor + 0.05, z).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
            LootRing(id),
        ));
    };

    let loot_table = [Item::Stimpak, Item::RadAway, Item::Ammo, Item::Ammo, Item::Hotdish];
    let mut spawned = 0;
    let mut attempts = 0;
    while spawned < 34 && attempts < 2_000 {
        attempts += 1;
        let x = rng.0.range(-HALF_SIZE + 10.0, HALF_SIZE - 10.0);
        let z = rng.0.range(-HALF_SIZE + 10.0, HALF_SIZE - 10.0);
        if terrain::lake_at(x, z).is_some() || collision::blocked(x, z, 0.6, &colliders.0) {
            continue;
        }
        let item = loot_table[(rng.0.f32() * loot_table.len() as f32) as usize % loot_table.len()];
        spawn(&mut commands, item, x, z);
        spawned += 1;
    }
    // A guaranteed welcome kit outside the vault door.
    for (i, item) in [Item::Ammo, Item::Hotdish].into_iter().enumerate() {
        spawn(&mut commands, item, VAULT_POS.0 - 4.0 + i as f32 * 8.0, VAULT_POS.1 - 18.0);
    }
}

/// The ground marker under a pickup; despawned with it.
#[derive(Component)]
struct LootRing(Entity);

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
    rings: Query<(Entity, &LootRing)>,
) {
    let Ok(ptf) = player.single() else { return };
    for (entity, tf, pickup) in &pickups {
        let d = tf.translation - ptf.translation;
        if d.x * d.x + d.z * d.z < 2.2 * 2.2 && d.y.abs() < 3.0 {
            game.inv.add(pickup.item);
            sfx.play(sfx::pickup_sound(pickup.item));
            let extra = if pickup.item == Item::Ammo { " (+12)" } else { "" };
            msgs.show(format!("Picked up: {}{}", pickup.item.name(), extra), 2.5);
            commands.entity(entity).despawn();
            for (ring, owner) in &rings {
                if owner.0 == entity {
                    commands.entity(ring).despawn();
                }
            }
        }
    }
}

fn flicker_fires(time: Res<Time>, mut q: Query<(&mut PointLight, &FireLight)>) {
    let t = time.elapsed_secs();
    for (mut light, fire) in &mut q {
        let s = fire.seed;
        let f = 0.78
            + 0.12 * (t * 7.3 + s).sin()
            + 0.07 * (t * 13.1 + s * 2.0).sin()
            + 0.05 * (t * 23.7 + s * 3.0).sin();
        light.intensity = fire.base * f;
    }
}

fn pulse_lights(time: Res<Time>, mut q: Query<(&mut PointLight, &PulseLight)>) {
    let t = time.elapsed_secs();
    for (mut light, pulse) in &mut q {
        light.intensity = pulse.base * (0.75 + 0.25 * (t * pulse.speed).sin());
    }
}

fn blink(time: Res<Time>, mut q: Query<(&mut Visibility, &Blinker)>) {
    let t = time.elapsed_secs();
    for (mut vis, b) in &mut q {
        let on = ((t + b.offset) / b.period).fract() < 0.5;
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

fn pulse_ice(time: Res<Time>, ice: Option<Res<IceGlow>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let Some(ice) = ice else { return };
    let t = time.elapsed_secs();
    if let Some(m) = materials.get_mut(&ice.0) {
        let k = 0.8 + 0.2 * (t * 0.7).sin() + 0.08 * (t * 2.3).sin();
        m.emissive = LinearRgba::rgb(0.6 * k, 1.6 * k, 0.8 * k);
    }
}
