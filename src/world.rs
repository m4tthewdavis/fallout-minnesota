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
use crate::state::{alive, Colliders, Game, RngRes, SfxQueue};

/// The directional "sun" light, dimmed by the weather.
#[derive(Component)]
pub struct Sun;

/// A pickup the player has taken. It stays in the world, hidden, so that
/// loading an earlier save can bring it back.
#[derive(Component)]
pub struct Collected;

#[derive(Component)]
pub struct Pickup {
    pub item: Item,
    pub base_y: f32,
}

/// Where [`open_spot`] may put things: how far in from the map edge, how many
/// tries, and how far to stay from the vault, ruins and cars.
pub struct SpotRules {
    pub edge: f32,
    pub tries: usize,
    pub vault_clear: f32,
    pub ruin_pad: f32,
    pub car_clear: f32,
}

/// Trees, rocks and drifts: anywhere on open ground not in a ruin or on a car.
pub const SCATTER: SpotRules = SpotRules { edge: 0.0, tries: 40, vault_clear: 0.0, ruin_pad: 0.0, car_clear: 5.0 };
/// Loot and camps: well inside the map and clear of the vault, with margins round landmarks.
pub const PLACED: SpotRules = SpotRules { edge: 12.0, tries: 60, vault_clear: 30.0, ruin_pad: 4.0, car_clear: 6.0 };

/// A random point on open ground that obeys `rules` and is clear of `solid`.
pub fn open_spot(rng: &mut RngRes, solid: &[collision::Shape], clearance: f32, rules: &SpotRules) -> Option<(f32, f32)> {
    use crate::landmarks::{CARS, RUINS};
    for _ in 0..rules.tries {
        let x = rng.0.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
        let z = rng.0.range(-HALF_SIZE + rules.edge, HALF_SIZE - rules.edge);
        if !terrain::is_open_ground(x, z) || terrain::dist_to_vault(x, z) < rules.vault_clear {
            continue;
        }
        if RUINS.iter().any(|&(rx, rz, r)| (x - rx).hypot(z - rz) < r + rules.ruin_pad)
            || CARS.iter().any(|&(cx, cz, _)| (x - cx).hypot(z - cz) < rules.car_clear)
            || collision::blocked(x, z, clearance, solid)
        {
            continue;
        }
        return Some((x, z));
    }
    None
}

/// A fire light that casts shadows when the shadow quality is high enough.
#[derive(Component)]
pub struct PointShadows;

/// A point light that flickers like fire.
#[derive(Component)]
pub struct FireLight {
    pub base: f32,
    pub seed: f32,
}

/// A warm point light that flickers with [`crate::particles::fire_flicker`]:
/// add a `Transform` to place it.
pub fn flicker_light(color: Color, intensity: f32, range: f32, shadows: bool, seed: f32) -> (PointLight, FireLight) {
    (
        PointLight { color, intensity, range, shadows_enabled: shadows, ..default() },
        FireLight { base: intensity, seed },
    )
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
                crate::vehicles::spawn_vehicles,
                crate::props::spawn_props,
                crate::nature::spawn_nature,
                spawn_loot,
                crate::interiors::spawn_interiors,
            )
                .chain()
                .in_set(crate::state::WorldGen),
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

/// Height of the ground you can see: the rendered terrain's triangles, not
/// the smooth field they approximate, so props rest on the snow exactly.
pub fn ground(x: f32, z: f32) -> f32 {
    terrain::mesh_height(x, z)
}

/// Weathering laid over every plain-coloured material (see [`mat`]).
pub static GRIME: std::sync::OnceLock<Handle<Image>> = std::sync::OnceLock::new();

/// A plain painted or bare material, with a little grime so flat colours
/// don't look like plastic.
pub fn mat(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        base_color_texture: GRIME.get().cloned(),
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

/// A snowdrift growing out of the ground (see [`meshgen::drift_patch`]),
/// shaded with the same snow shader and tint as the terrain so it blends in.
/// `along` is the direction its long axis and gentle slope face into.
pub fn spawn_drift(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &GameAssets,
    x: f32,
    z: f32,
    length: f32,
    width: f32,
    peak: f32,
    along: [f32; 2],
    seed: u64,
) {
    let mut m = meshgen::drift_patch(x, z, length, width, peak, along, seed, 4.0, &terrain::mesh_height);
    for i in 0..m.positions.len() {
        let p = m.positions[i];
        m.colors[i] = terrain_tint(p[0], p[2], p[1]);
    }
    commands.spawn((Mesh3d(meshes.add(to_mesh_tangents(&m))), MeshMaterial3d(assets.snow_ground.clone())));
}

/// A soft shadow pooled on the snow under an object (`rx` by `rz` metres,
/// turned by `yaw`): cheap ambient occlusion where things meet the ground.
pub fn spawn_contact_shadow(commands: &mut Commands, meshes: &mut Assets<Mesh>, assets: &GameAssets, x: f32, z: f32, rx: f32, rz: f32, yaw: f32) {
    let n = 4;
    let (s, c) = yaw.sin_cos();
    let mut m = MeshData::default();
    for i in 0..=n {
        for j in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let (lx, lz) = ((u * 2.0 - 1.0) * rx, (v * 2.0 - 1.0) * rz);
            let (wx, wz) = (x + lx * c + lz * s, z - lx * s + lz * c);
            m.vertex([wx, terrain::mesh_height(wx, wz).max(terrain::walk_height(wx, wz)) + 0.02, wz], [0.0, 1.0, 0.0], [u, v], meshgen::WHITE);
        }
    }
    let row = n as u32 + 1;
    for i in 0..n as u32 {
        for j in 0..n as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m.recompute_normals();
    if m.normals[0][1] < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
    }
    commands.spawn((Mesh3d(meshes.add(to_mesh(&m))), MeshMaterial3d(assets.contact_shadow.clone()), bevy::pbr::NotShadowCaster));
}

/// Spawns a glTF model.
pub fn prop(commands: &mut Commands, scene: &Handle<Scene>, pos: Vec3, yaw: f32, scale: f32) -> Entity {
    commands
        .spawn((
            SceneRoot(scene.clone()),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
            // Not drawn beyond the view distance's prop range.
            crate::perf::Lod::Prop,
        ))
        .id()
}

/// Snow tint for the terrain: bluer in hollows, trampled near shelters, slushy
/// along the highway and sickly green around the radiation craters.
pub fn terrain_tint(x: f32, z: f32, h: f32) -> [f32; 4] {
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
    let terrain_data = meshgen::ground_patch(0.0, 0.0, HALF_SIZE, terrain::MESH_RES, 0.0, Some(4.0), &terrain::height);
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
        MeshMaterial3d(assets.snow_ground.clone()),
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
    // Clear black-green ice full of bubbles and cracks (ambientCG Ice003),
    // the reactor's glow seeping up through it (ice_emissive).
    let ice = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 1.0, 1.0),
        base_color_texture: Some(assets.ice_diff.clone()),
        normal_map_texture: Some(assets.ice_nor.clone()),
        metallic_roughness_texture: Some(assets.ice_arm.clone()),
        emissive: LinearRgba::rgb(0.6, 1.6, 0.8),
        emissive_texture: Some(assets.ice_emissive.clone()),
        metallic: 0.0,
        // The scan's roughness: glassy, with frosted cracks.
        perceptual_roughness: 1.0,
        reflectance: 0.7,
        ..default()
    });
    commands.insert_resource(IceGlow(ice.clone()));
    for (lx, lz, r) in LAKES {
        commands.spawn((
            Mesh3d(meshes.add(to_mesh_tangents(&meshgen::disc(r * ICE_FRACTION, 64, 9.0)))),
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
    // Shotgun shells: three red cartridges with brass bases. Revolver rounds: a
    // small brass box. Scrap: a little heap of rusted metal.
    let mut shell_data = meshgen::MeshData::default();
    for (i, dx) in [-0.07f32, 0.0, 0.07].into_iter().enumerate() {
        let shell = meshgen::lathe(&[(0.034, 0.0), (0.034, 0.12), (0.03, 0.125), (0.0, 0.125)], 10, 0.2, true, false)
            .rotated_z(std::f32::consts::FRAC_PI_2)
            .translated([0.0, 0.034, dx * 1.0 + i as f32 * 0.0]);
        shell_data.append(&shell);
    }
    let shell_mesh = meshes.add(to_mesh(&shell_data));
    let shell_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.12, 0.1),
        emissive: LinearRgba::rgb(0.35, 0.03, 0.02),
        perceptual_roughness: 0.4,
        ..default()
    });
    let brass_mesh = meshes.add(Cuboid::new(0.26, 0.12, 0.18));
    let brass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.6, 0.25),
        emissive: LinearRgba::rgb(0.3, 0.2, 0.03),
        metallic: 0.8,
        perceptual_roughness: 0.35,
        ..default()
    });
    let mut scrap_data = meshgen::MeshData::default();
    for (i, (size, pos, rot)) in [
        ([0.3, 0.04, 0.2], [0.0, 0.03, 0.0], 0.3f32),
        ([0.22, 0.05, 0.12], [0.05, 0.08, 0.03], 1.1),
        ([0.16, 0.14, 0.04], [-0.06, 0.1, -0.05], 0.7),
        ([0.1, 0.1, 0.1], [0.1, 0.06, -0.08], 0.2),
    ]
    .into_iter()
    .enumerate()
    {
        scrap_data.append(&meshgen::cuboid(size, 0.3).rotated_y(rot + i as f32 * 0.4).rotated_z(0.1 * i as f32).translated(pos));
    }
    let scrap_mesh = meshes.add(to_mesh_tangents(&scrap_data));
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
            Item::Shells => {
                p.spawn((Mesh3d(shell_mesh.clone()), MeshMaterial3d(shell_mat.clone()), Transform::from_xyz(0.0, -0.1, 0.0)));
            }
            Item::RevolverRounds => {
                p.spawn((Mesh3d(brass_mesh.clone()), MeshMaterial3d(brass_mat.clone()), Transform::default()));
            }
            Item::Scrap => {
                p.spawn((
                    Mesh3d(scrap_mesh.clone()),
                    MeshMaterial3d(assets.rust.clone()),
                    Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::splat(1.3)),
                ));
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

    let loot_table = [
        Item::Stimpak,
        Item::RadAway,
        Item::Ammo,
        Item::Ammo,
        Item::Hotdish,
        Item::Shells,
        Item::RevolverRounds,
        Item::Scrap,
        Item::Scrap,
        Item::Scrap,
    ];
    let mut spawned = 0;
    let mut attempts = 0;
    while spawned < 42 && attempts < 2_000 {
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

/// The ground marker under a pickup; hidden with it.
#[derive(Component)]
pub(crate) struct LootRing(pub Entity);

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
    mut feed: ResMut<crate::fo4ui::Feed>,
    mut sfx: ResMut<SfxQueue>,
    perks: Res<crate::quest::Perks>,
    player: Query<&Transform, With<Player>>,
    pickups: Query<(Entity, &Transform, &Pickup), (Without<Player>, Without<Collected>)>,
    rings: Query<(Entity, &LootRing)>,
) {
    let Ok(ptf) = player.single() else { return };
    for (entity, tf, pickup) in &pickups {
        let d = tf.translation - ptf.translation;
        if d.x * d.x + d.z * d.z < 2.2 * 2.2 && d.y.abs() < 3.0 {
            game.inv.add(pickup.item);
            if pickup.item == crate::sim::survival::Item::Scrap {
                // Scrounger.
                game.inv.scrap += perks.0.extra_scrap;
            }
            sfx.play(sfx::pickup_sound(pickup.item));
            let n = pickup.item.amount();
            let extra = if n > 1 { format!(" (+{n})") } else { String::new() };
            feed.push(format!("{}{} added", pickup.item.name(), extra));
            commands.entity(entity).insert((Collected, Visibility::Hidden));
            for (ring, owner) in &rings {
                if owner.0 == entity {
                    commands.entity(ring).insert(Visibility::Hidden);
                }
            }
        }
    }
}

fn flicker_fires(time: Res<Time>, mut q: Query<(&mut PointLight, &FireLight)>) {
    let t = time.elapsed_secs();
    for (mut light, fire) in &mut q {
        // The same flicker as the flames and their glow.
        light.intensity = fire.base * crate::particles::fire_flicker(t, fire.seed) * 0.85;
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
