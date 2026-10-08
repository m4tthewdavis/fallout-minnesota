//! The first-person weapons: the pipe rifle, scrap shotgun, frontier revolver
//! and ice axe, all scrap-built view models drawn by their own camera (so they
//! never clip into walls or trees) and animated from the pure pose maths in
//! `sim::viewmodel` - idle sway, walk/sprint bob, look lag, recoil, a falling
//! hammer, pulled trigger, reloads that drop magazines, break open shotgun
//! barrels and swing out revolver cylinders, axe swings, drawing, lowering
//! against walls - and aim-down-sights on the right mouse button. Fitted
//! upgrades show up on the models.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::camera::ClearColorConfig;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;

use crate::assets::GameAssets;
use crate::meshes::to_mesh_tangents;
use crate::player::{cursor_locked, Player};
use crate::sim::collision;
use crate::sim::meshgen::{self, sec, MeshData};
use crate::sim::combat::{Upgrade, WeaponKind};
use crate::sim::viewmodel::{self, Held, Inputs, Pose, SCALE};
use crate::state::{Colliders, Game};

/// Render layer for the view model and its camera.
pub const VIEW_LAYER: usize = 1;
/// Muzzle position in the gun model's local space.
pub fn muzzle_local(kind: WeaponKind) -> Vec3 {
    match kind {
        WeaponKind::PipeRifle => Vec3::new(0.0, 0.012, -0.81),
        WeaponKind::ScrapShotgun => Vec3::new(0.0, 0.012, -0.79),
        WeaponKind::Revolver => Vec3::new(0.0, 0.012, -0.29),
        WeaponKind::IceAxe => Vec3::new(0.0, 0.2, -0.25),
    }
}

/// Where spent brass leaves the weapon during a reload (gun space).
pub fn breech_local(kind: WeaponKind) -> Vec3 {
    match kind {
        WeaponKind::ScrapShotgun => Vec3::new(0.0, 0.0, -0.03),
        WeaponKind::Revolver => Vec3::new(0.0, -0.02, -0.05),
        _ => Vec3::new(0.037, 0.02, 0.0),
    }
}
const HIP_FOV: f32 = 75.0;
const ADS_FOV: f32 = 52.0;
/// Barrel axis height in gun space.
const BORE_Y: f32 = 0.012;
/// The rifle's sight line (see `viewmodel::sight_y`).
const RIFLE_SIGHT: f32 = 0.075;

/// The gun root (child of the camera).
#[derive(Component)]
pub struct GunModel;
#[derive(Component)]
pub struct ViewModelCamera;
#[derive(Component)]
struct GunBolt(Vec3);
#[derive(Component)]
struct GunMag(Vec3);
#[derive(Component)]
struct GunHammer(Vec3);
#[derive(Component)]
struct GunTrigger(Vec3);
/// One weapon's model; only the weapon in hand is visible.
#[derive(Component)]
struct WeaponModel(WeaponKind);
/// The shotgun's hinged barrel assembly (rest position = the hinge).
#[derive(Component)]
struct BreakGroup(Vec3);
/// The revolver's cylinder (rest position).
#[derive(Component)]
struct CylinderGroup(Vec3);
/// Rounds sitting in the chambers, hidden while a reload has them out.
#[derive(Component)]
struct ChamberRounds;
/// A part that only shows once an upgrade is fitted to that weapon.
#[derive(Component)]
struct UpgradeVisual(WeaponKind, Upgrade);

/// Screenshot mode can hold the aim button down (see `devshot.rs`).
#[derive(Resource, Default)]
pub struct ForceAim(pub bool);

/// How far the player is aimed down the sights, 0..1 (the HUD hides the
/// crosshair when aiming).
#[derive(Resource, Default)]
pub struct AimAmount(pub f32);

/// Smoothed inputs to the pose.
#[derive(Resource, Default)]
struct GunState {
    ads: f32,
    blocked: f32,
    look: Vec2,
    stride: f32,
    last_pos: Option<Vec3>,
}

/// The first-person hands and sleeves (hidden with the gun by the Pip-Boy).
#[derive(Component)]
pub struct ArmsRig;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Left,
    Right,
}

/// A hand's root: placed every frame from the pose.
#[derive(Component)]
struct HandRoot(Side);
/// One of a hand's shapes (open, half, fist); the nearest to the grip shows.
#[derive(Component)]
struct HandShape(f32);
/// A sleeve from the wrist to the (off-screen) elbow.
#[derive(Component)]
struct Sleeve(Side);
/// Shells or rounds carried in the left hand.
#[derive(Component)]
struct HeldItem(Held);

/// The pose worked out this frame, for the hands.
#[derive(Resource, Default)]
struct LastPose(Option<Pose>);

pub struct GunPlugin;

impl Plugin for GunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GunState>()
            .init_resource::<ForceAim>()
            .init_resource::<AimAmount>()
            .init_resource::<LastPose>()
            .add_systems(Update, (aim_down_sights, pose_gun, pose_hands, show_current_weapon, show_upgrades).chain())
            .add_systems(Update, update_gun_frost);
    }
}

/// Spawns the view-model camera and the rifle under the player camera.
pub fn spawn_view_model(
    cam: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &GameAssets,
) {
    let layer = RenderLayers::layer(VIEW_LAYER);
    cam.spawn((
        Camera3d::default(),
        Camera {
            // Drawn after the world, over it, with its own depth buffer.
            order: 1,
            hdr: true,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Projection::from(PerspectiveProjection {
            fov: HIP_FOV.to_radians(),
            near: 0.01,
            ..default()
        }),
        layer.clone(),
        ViewModelCamera,
    ));
    let h = viewmodel::hip(WeaponKind::PipeRifle);
    let mats = make_mats(materials, assets);
    cam.commands().insert_resource(GunFrost(mats.rime.clone()));
    spawn_arms(cam, meshes, materials, &mats, &layer);
    cam.spawn((
        Transform::from_xyz(h[0], h[1], h[2]).with_scale(Vec3::splat(SCALE)),
        Visibility::default(),
        GunModel,
    ))
    .with_children(|gun| {
        for kind in WeaponKind::ALL {
            gun.spawn((Transform::default(), Visibility::Hidden, WeaponModel(kind)))
                .with_children(|g| match kind {
                    WeaponKind::PipeRifle => build_rifle(g, meshes, &mats, &layer),
                    WeaponKind::ScrapShotgun => build_shotgun(g, meshes, &mats, &layer),
                    WeaponKind::Revolver => build_revolver(g, meshes, &mats, &layer),
                    WeaponKind::IceAxe => build_axe(g, meshes, &mats, &layer),
                })
                .with_children(|g| frost_patches(g, meshes, &mats, &layer, kind));
        }
    });
}

/// Materials shared by every weapon model.
struct Mats {
    steel: Handle<StandardMaterial>,
    pipe: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    tape: Handle<StandardMaterial>,
    copper: Handle<StandardMaterial>,
    brass: Handle<StandardMaterial>,
    weld: Handle<StandardMaterial>,
    leather: Handle<StandardMaterial>,
    rubber: Handle<StandardMaterial>,
    fleece: Handle<StandardMaterial>,
    frost: Handle<StandardMaterial>,
    /// Hoar frost that builds up on cold metal (see `update_gun_frost`).
    rime: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
}

fn make_mats(materials: &mut Assets<StandardMaterial>, assets: &GameAssets) -> Mats {
    let tex = |name: &str| assets.gun_textures.get(name).cloned();
    let steel = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: tex("steel_diff"),
        normal_map_texture: tex("steel_nor"),
        metallic_roughness_texture: tex("steel_arm"),
        occlusion_texture: tex("steel_arm"),
        metallic: 1.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    // Pipes and tubing are painted steel plate (a real scanned metal: scuffed, chipped).
    let pipe = match materials.get(&assets.plate).cloned() {
        Some(plate) => materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.64, 0.7), ..plate }),
        None => assets.plate.clone(),
    };
    let rime = materials.add(StandardMaterial {
        base_color: Color::srgba(0.9, 0.96, 1.0, 0.0),
        base_color_texture: tex("rime"),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.5,
        emissive: LinearRgba::rgb(0.1, 0.14, 0.18),
        ..default()
    });
    let wood = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: tex("wood_diff"),
        normal_map_texture: tex("wood_nor"),
        perceptual_roughness: 0.62,
        ..default()
    });
    let mut flat = |c: Color, metal: f32, rough: f32| {
        materials.add(StandardMaterial {
            base_color: c,
            metallic: metal,
            perceptual_roughness: rough,
            ..default()
        })
    };
    Mats {
        steel,
        pipe,
        wood,
        black: flat(Color::srgb(0.02, 0.02, 0.02), 0.0, 0.9),
        tape: flat(Color::srgb(0.24, 0.26, 0.22), 0.0, 0.85),
        copper: flat(Color::srgb(0.75, 0.42, 0.24), 1.0, 0.35),
        brass: flat(Color::srgb(0.8, 0.62, 0.28), 1.0, 0.3),
        weld: flat(Color::srgb(0.2, 0.18, 0.17), 0.7, 0.6),
        leather: flat(Color::srgb(0.26, 0.15, 0.08), 0.0, 0.75),
        rubber: flat(Color::srgb(0.05, 0.05, 0.05), 0.0, 0.95),
        fleece: flat(Color::srgb(0.55, 0.58, 0.62), 0.0, 0.97),
        frost: flat(Color::srgb(0.88, 0.93, 1.0), 0.0, 0.8),
        rime,
        red: flat(Color::srgb(0.7, 0.1, 0.08), 0.0, 0.5),
    }
}

/// A part that only shows once `up` is fitted to `kind`.
fn upgrade_part(
    p: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
    material: &Handle<StandardMaterial>,
    tf: Transform,
    layer: &RenderLayers,
    kind: WeaponKind,
    up: Upgrade,
) {
    p.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone()),
        tf,
        Visibility::Hidden,
        NotShadowCaster,
        layer.clone(),
        UpgradeVisual(kind, up),
    ));
}

/// A row of brass cartridges on a stock or grip ("heavy loads" upgrade).
fn bandolier(p: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers, kind: WeaponKind, from: Vec3, step: Vec3) {
    for k in 0..6 {
        let at = from + step * k as f32;
        upgrade_part(p, meshes, Cylinder::new(0.0075, 0.04).into(), &m.brass, Transform::from_translation(at).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), layer, kind, Upgrade::HeavyLoads);
        upgrade_part(p, meshes, Cylinder::new(0.0078, 0.012).into(), &m.red, Transform::from_translation(at + Vec3::Z * -0.016).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), layer, kind, Upgrade::HeavyLoads);
    }
}

/// A part of the rifle on the view-model layer.
/// Mirror a right-hand mesh into a left hand.
fn mirrored(m: MeshData) -> MeshData {
    let mut m = m.scaled([-1.0, 1.0, 1.0]);
    for t in m.indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    m
}

/// Gloved hands with three grip shapes each, sleeves, and the shells and
/// rounds the left hand carries while loading.
fn spawn_arms(cam: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, m: &Mats, layer: &RenderLayers) {
    let glove = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.72,
        ..default()
    });
    let cloth = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    });
    let shapes = [0.0f32, 0.55, 1.0];
    for side in [Side::Left, Side::Right] {
        cam.spawn((Transform::default(), Visibility::default(), HandRoot(side), ArmsRig)).with_children(|h| {
            for g in shapes {
                let mesh = meshgen::glove(g);
                let mesh = if side == Side::Left { mirrored(mesh) } else { mesh };
                h.spawn((Mesh3d(meshes.add(to_mesh_tangents(&mesh))), MeshMaterial3d(glove.clone()), Transform::default(), Visibility::Hidden, HandShape(g), NotShadowCaster, layer.clone()));
            }
            if side == Side::Left {
                // Two shotgun shells, and three revolver rounds, held in the fingers.
                let fist = Vec3::from_array(viewmodel::FIST_CENTRE);
                h.spawn((Transform::from_translation(fist), Visibility::Hidden, HeldItem(Held::Shells))).with_children(|i| {
                    for dz in [-0.012f32, 0.012] {
                        part(i, meshes, Cylinder::new(0.0115, 0.07).into(), &m.red, Transform::from_xyz(0.0, 0.0, dz).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
                        part(i, meshes, Cylinder::new(0.012, 0.016).into(), &m.brass, Transform::from_xyz(0.035, 0.0, dz).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
                    }
                });
                h.spawn((Transform::from_translation(fist), Visibility::Hidden, HeldItem(Held::Rounds))).with_children(|i| {
                    for dz in [-0.012f32, 0.0, 0.012] {
                        part(i, meshes, Cylinder::new(0.0055, 0.04).into(), &m.brass, Transform::from_xyz(0.0, 0.0, dz).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
                    }
                });
            }
        });
        cam.spawn((Mesh3d(meshes.add(to_mesh_tangents(&meshgen::sleeve()))), MeshMaterial3d(cloth.clone()), Transform::default(), Visibility::default(), Sleeve(side), ArmsRig, NotShadowCaster, layer.clone()));
    }
}

/// Place the hands on the weapon, pick each hand's shape, stretch the
/// sleeves back to the elbows, and show what the left hand is carrying.
fn pose_hands(
    last: Res<LastPose>,
    mut roots: Query<(&HandRoot, &mut Transform, &Children), Without<Sleeve>>,
    mut shapes: Query<(&HandShape, &mut Visibility), Without<HeldItem>>,
    mut held: Query<(&HeldItem, &mut Visibility), Without<HandShape>>,
    mut sleeves: Query<(&Sleeve, &mut Transform), Without<HandRoot>>,
) {
    let Some(pose) = last.0 else { return };
    let gun = Transform::from_translation(Vec3::from_array(pose.pos))
        .with_rotation(Quat::from_euler(EulerRot::YXZ, pose.rot[1], pose.rot[0], pose.rot[2]))
        .with_scale(Vec3::splat(SCALE));
    let mut wrists = [Vec3::ZERO; 2];
    for (root, mut tf, children) in &mut roots {
        let hand = if root.0 == Side::Left { pose.left } else { pose.right };
        let (x, y) = (Vec3::from_array(hand.x), Vec3::from_array(hand.y));
        let local = Transform::from_translation(Vec3::from_array(hand.pos)).with_rotation(Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))));
        *tf = gun.mul_transform(local);
        wrists[root.0 as usize] = tf.transform_point(Vec3::new(0.0, 0.0, 0.1));
        // Show the shape closest to the grip.
        let mut best = (f32::MAX, 0.0);
        for &c in children {
            if let Ok((shape, _)) = shapes.get(c) {
                let d = (shape.0 - hand.grip).abs();
                if d < best.0 {
                    best = (d, shape.0);
                }
            }
        }
        for &c in children {
            if let Ok((shape, mut vis)) = shapes.get_mut(c) {
                let want = if shape.0 == best.1 { Visibility::Inherited } else { Visibility::Hidden };
                if *vis != want {
                    *vis = want;
                }
            }
        }
    }
    for (item, mut vis) in &mut held {
        let want = if item.0 == pose.held { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    // Forearms hang down and back from the wrists, out past the screen edge.
    let hang = [Vec3::new(-0.35, -0.8, 0.45).normalize(), Vec3::new(0.3, -0.8, 0.5).normalize()];
    for (sleeve, mut tf) in &mut sleeves {
        let i = sleeve.0 as usize;
        let (wrist, elbow) = (wrists[i], wrists[i] + hang[i] * 0.5);
        let along = elbow - wrist;
        let len = along.length().max(0.05);
        *tf = Transform::from_translation(wrist).with_rotation(Quat::from_rotation_arc(Vec3::Z, along / len)).with_scale(Vec3::new(SCALE, SCALE, len));
    }
}

fn part(
    p: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
    material: &Handle<StandardMaterial>,
    tf: Transform,
    layer: &RenderLayers,
) {
    p.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material.clone()),
        tf,
        NotShadowCaster,
        layer.clone(),
    ));
}

/// Lathe along +Y turned to point down -Z (barrel pieces).
fn along_z(profile: &[(f32, f32)], segments: usize) -> MeshData {
    meshgen::lathe(profile, segments, 0.12, true, true).rotated_x(-FRAC_PI_2)
}

/// An elliptical band around the gun (hose clamps, tape wraps).
fn band(y: f32, rx: f32, ry: f32, width: f32) -> MeshData {
    meshgen::loft_z(&[sec(-width * 0.5, y, rx, ry, 2.6), sec(width * 0.5, y, rx, ry, 2.6)], 16, 0.05)
}

fn build_rifle(gun: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers) {
    let Mats { steel, pipe, wood, black, tape, copper, brass, weld, leather, rubber, fleece, .. } = m;
    let t0 = Transform::default();

    // ---- Receiver: one solid block every other part bolts on to ----
    part(gun, meshes, to_mesh_tangents(&meshgen::cuboid([0.072, 0.095, 0.3], 0.12)), steel, Transform::from_xyz(0.0, 0.0, -0.02), layer);
    part(gun, meshes, to_mesh_tangents(&meshgen::cuboid([0.06, 0.012, 0.22], 0.12)), steel, Transform::from_xyz(0.0, 0.0535, -0.03), layer);
    // Ejection port and bolt slot on the right.
    part(gun, meshes, Cuboid::new(0.002, 0.03, 0.075).into(), black, Transform::from_xyz(0.0365, 0.015, -0.02), layer);
    // Weld beads along the lower seams and around the barrel joint.
    let mut beads = MeshData::default();
    let mut k = 0;
    let mut z = -0.15;
    while z < 0.12 {
        for x in [-0.036f32, 0.036] {
            k += 1;
            beads.append(&meshgen::blob(0.0055, 0.8, 0.35, k, 0.05).translated([x, -0.047, z]));
        }
        z += 0.016;
    }
    for i in 0..14 {
        let a = i as f32 / 14.0 * TAU;
        beads.append(&meshgen::blob(0.006, 1.0, 0.35, 100 + i, 0.05).translated([a.cos() * 0.034, BORE_Y + a.sin() * 0.034, -0.168]));
    }
    part(gun, meshes, to_mesh_tangents(&beads), weld, t0, layer);
    // Brass screws holding the side plates and the stock.
    let mut screws = MeshData::default();
    for (y, z) in [(0.025, -0.13), (0.025, 0.08), (-0.028, 0.08), (-0.028, -0.13), (0.0, 0.12)] {
        for x in [-1.0f32, 1.0] {
            let head = meshgen::lathe(&[(0.006, 0.0), (0.006, 0.002), (0.004, 0.004), (0.0, 0.0045)], 8, 0.05, false, false)
                .rotated_z(-x * FRAC_PI_2)
                .translated([x * 0.036, y, z]);
            screws.append(&head);
        }
    }
    part(gun, meshes, to_mesh_tangents(&screws), brass, t0, layer);

    // ---- Barrel: plumbing pipe with hex couplings and a muzzle cap ----
    let barrel_at = |m: MeshData, z: f32| m.translated([0.0, BORE_Y, z]);
    part(gun, meshes, to_mesh_tangents(&barrel_at(along_z(&[(0.034, 0.0), (0.034, 0.05)], 6), -0.165)), steel, t0, layer);
    part(gun, meshes, to_mesh_tangents(&barrel_at(along_z(&[(0.022, 0.0), (0.022, 0.585)], 16), -0.2)), pipe, t0, layer);
    part(gun, meshes, to_mesh_tangents(&barrel_at(along_z(&[(0.03, 0.0), (0.031, 0.04)], 6), -0.47)), steel, t0, layer);
    part(gun, meshes, to_mesh_tangents(&barrel_at(along_z(&[(0.029, 0.0), (0.029, 0.035), (0.017, 0.042)], 12), -0.765)), steel, t0, layer);
    part(gun, meshes, Circle::new(0.012).into(), black, Transform::from_xyz(0.0, BORE_Y, -0.8075).with_rotation(Quat::from_rotation_y(PI)), layer);

    // ---- Wooden handguard, lashed on with wire and hose clamps ----
    let handguard = meshgen::loft_z(
        &[
            sec(-0.53, -0.03, 0.02, 0.018, 3.0),
            sec(-0.52, -0.03, 0.028, 0.026, 3.0),
            sec(-0.2, -0.03, 0.03, 0.028, 3.0),
            sec(-0.185, -0.028, 0.026, 0.024, 3.0),
        ],
        14,
        0.12,
    );
    part(gun, meshes, to_mesh_tangents(&handguard), wood, t0, layer);
    for z in [-0.29f32, -0.6] {
        part(gun, meshes, to_mesh_tangents(&band(-0.009, 0.033, 0.05, 0.012).translated([0.0, 0.0, z])), steel, t0, layer);
        part(gun, meshes, Cuboid::new(0.012, 0.016, 0.016).into(), steel, Transform::from_xyz(0.035, -0.009, z), layer);
        part(gun, meshes, Cylinder::new(0.004, 0.012).into(), brass, Transform::from_xyz(0.035, 0.002, z).with_rotation(Quat::from_rotation_x(0.0)), layer);
    }
    // Copper wire binding: a helix around barrel and handguard.
    let helix: Vec<[f32; 3]> = (0..=90)
        .map(|i| {
            let t = i as f32 / 90.0;
            let a = t * 6.0 * TAU;
            [a.cos() * 0.032, -0.009 + a.sin() * 0.049, -0.4 - t * 0.04]
        })
        .collect();
    part(gun, meshes, to_mesh_tangents(&meshgen::tube(&helix, 0.0022, 5)), copper, t0, layer);

    // ---- Sights: rear peep on the cover, front post with guard wings ----
    part(gun, meshes, Cuboid::new(0.024, 0.006, 0.02).into(), steel, Transform::from_xyz(0.0, 0.0625, 0.09), layer);
    part(gun, meshes, Cuboid::new(0.006, 0.008, 0.004).into(), steel, Transform::from_xyz(0.0, 0.066, 0.09), layer);
    part(
        gun,
        meshes,
        Torus::new(0.0055, 0.0105).into(),
        steel,
        Transform::from_xyz(0.0, RIFLE_SIGHT, 0.09).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        layer,
    );
    part(gun, meshes, Cuboid::new(0.02, 0.012, 0.02).into(), steel, Transform::from_xyz(0.0, 0.04, -0.74), layer);
    part(gun, meshes, Cuboid::new(0.005, 0.03, 0.005).into(), steel, Transform::from_xyz(0.0, RIFLE_SIGHT - 0.015, -0.74), layer);
    for x in [-0.012f32, 0.012] {
        part(gun, meshes, Cuboid::new(0.004, 0.028, 0.012).into(), steel, Transform::from_xyz(x, 0.058, -0.74), layer);
    }

    // ---- Lower: trigger guard, trigger, grip, magazine ----
    part(
        gun,
        meshes,
        Torus::new(0.016, 0.021).into(),
        steel,
        Transform::from_xyz(0.0, -0.066, 0.03).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        layer,
    );
    let trigger_rest = Vec3::new(0.0, -0.048, 0.024);
    gun.spawn((Transform::from_translation(trigger_rest), Visibility::default(), GunTrigger(trigger_rest)))
        .with_children(|t| {
            part(t, meshes, Cuboid::new(0.005, 0.022, 0.006).into(), steel, Transform::from_xyz(0.0, -0.011, 0.0).with_rotation(Quat::from_rotation_x(-0.25)), layer);
        });
    part(
        gun,
        meshes,
        to_mesh_tangents(&meshgen::cuboid([0.034, 0.11, 0.046], 0.12)),
        wood,
        Transform::from_xyz(0.0, -0.1, 0.088).with_rotation(Quat::from_rotation_x(0.35)),
        layer,
    );
    let mag_rest = Vec3::new(0.0, -0.0475, -0.075);
    gun.spawn((Transform::from_translation(mag_rest), Visibility::default(), GunMag(mag_rest)))
        .with_children(|m| {
            part(
                m,
                meshes,
                to_mesh_tangents(&meshgen::cuboid([0.03, 0.12, 0.05], 0.12)),
                steel,
                Transform::from_xyz(0.0, -0.058, 0.004).with_rotation(Quat::from_rotation_x(-0.12)),
                layer,
            );
            part(m, meshes, Cuboid::new(0.034, 0.008, 0.056).into(), steel, Transform::from_xyz(0.0, -0.118, 0.011).with_rotation(Quat::from_rotation_x(-0.12)), layer);
            // The top round peeking out of the feed lips.
            part(m, meshes, Cylinder::new(0.0045, 0.03).into(), brass, Transform::from_xyz(0.0, 0.002, -0.004).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), layer);
            // Extended magazine: a longer body bolted under the floorplate.
            upgrade_part(
                m,
                meshes,
                to_mesh_tangents(&meshgen::cuboid([0.03, 0.07, 0.05], 0.12)),
                steel,
                Transform::from_xyz(0.0, -0.158, 0.015).with_rotation(Quat::from_rotation_x(-0.12)),
                layer,
                WeaponKind::PipeRifle,
                Upgrade::ExtendedMag,
            );
        });

    // ---- Action: hammer and bolt handle ----
    let hammer_rest = Vec3::new(0.0, 0.036, 0.128);
    gun.spawn((Transform::from_translation(hammer_rest), Visibility::default(), GunHammer(hammer_rest)))
        .with_children(|hm| {
            part(hm, meshes, Cuboid::new(0.01, 0.026, 0.01).into(), steel, Transform::from_xyz(0.0, 0.013, 0.0), layer);
            part(hm, meshes, Cuboid::new(0.01, 0.006, 0.018).into(), steel, Transform::from_xyz(0.0, 0.025, 0.007), layer);
        });
    let bolt_rest = Vec3::new(0.037, 0.02, 0.0);
    gun.spawn((Transform::from_translation(bolt_rest), Visibility::default(), GunBolt(bolt_rest)))
        .with_children(|b| {
            part(b, meshes, Cylinder::new(0.0055, 0.045).into(), steel, Transform::from_xyz(0.022, 0.0, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
            part(b, meshes, Sphere::new(0.012).into(), steel, Transform::from_xyz(0.047, 0.0, 0.0), layer);
        });

    // ---- Stock: carved wood, taped wrist, rubber butt pad ----
    let stock_secs = [
        sec(0.125, -0.005, 0.033, 0.045, 3.0),
        sec(0.2, -0.025, 0.028, 0.04, 3.0),
        sec(0.3, -0.05, 0.031, 0.06, 3.0),
        sec(0.41, -0.065, 0.034, 0.078, 3.0),
        sec(0.43, -0.066, 0.034, 0.079, 3.0),
    ];
    part(gun, meshes, to_mesh_tangents(&meshgen::loft_z(&stock_secs, 16, 0.12)), wood, t0, layer);
    for z in [0.18f32, 0.2, 0.22] {
        // Interpolate the stock's cross-section at this point.
        let t = (z - 0.125) / (0.2 - 0.125);
        let (y, rx, ry) = if z <= 0.2 {
            (-0.005 - 0.02 * t, 0.033 - 0.005 * t, 0.045 - 0.005 * t)
        } else {
            let t = (z - 0.2) / 0.1;
            (-0.025 - 0.025 * t, 0.028 + 0.003 * t, 0.04 + 0.02 * t)
        };
        part(gun, meshes, to_mesh_tangents(&band(y, rx + 0.002, ry + 0.002, 0.018).translated([0.0, 0.0, z])), tape, t0, layer);
    }
    part(gun, meshes, Cuboid::new(0.07, 0.16, 0.018).into(), rubber, Transform::from_xyz(0.0, -0.066, 0.44), layer);

    // ---- Sling with swivels ----
    let rear = Vec3::new(0.0, -0.152, 0.37);
    let front = Vec3::new(0.0, -0.064, -0.48);
    for p in [rear, front] {
        part(
            gun,
            meshes,
            Torus::new(0.006, 0.01).into(),
            steel,
            Transform::from_translation(p).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            layer,
        );
    }
    let strap = meshgen::catenary((rear - Vec3::Y * 0.01).to_array(), (front - Vec3::Y * 0.01).to_array(), 0.05, 24);
    part(gun, meshes, to_mesh_tangents(&meshgen::tube(&strap, 0.005, 6).scaled([2.4, 1.0, 1.0])), leather, t0, layer);

    // ---- Upgrade visuals ----
    // Insulated action: a quilted fleece wrap taped round the receiver.
    let k = WeaponKind::PipeRifle;
    upgrade_part(gun, meshes, to_mesh_tangents(&meshgen::cuboid([0.082, 0.104, 0.11], 0.1)), fleece, Transform::from_xyz(0.0, 0.0, 0.04), layer, k, Upgrade::InsulatedAction);
    for z in [-0.005f32, 0.085] {
        upgrade_part(gun, meshes, Cuboid::new(0.086, 0.108, 0.012).into(), tape, Transform::from_xyz(0.0, 0.0, z), layer, k, Upgrade::InsulatedAction);
    }
    bandolier(gun, meshes, m, layer, k, Vec3::new(0.038, -0.03, 0.2), Vec3::new(0.0, 0.0, 0.035));
}

/// The scrap shotgun: two plumbing pipes clamped together on a hinge, so the
/// whole barrel assembly (with its forend) tips open for loading.
fn build_shotgun(w: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers) {
    let Mats { steel, pipe, wood, black, copper, brass, weld, leather, rubber, fleece, .. } = m;
    let t0 = Transform::default();
    let k = WeaponKind::ScrapShotgun;

    // ---- Receiver: one steel block with a top plate and rear sight ----
    part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.078, 0.095, 0.12], 0.12)), steel, Transform::from_xyz(0.0, 0.0, 0.035), layer);
    part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.07, 0.01, 0.1], 0.12)), steel, Transform::from_xyz(0.0, 0.052, 0.04), layer);
    // A shallow sighting groove along the top plate: the only rear sight a scrap gun gets.
    part(w, meshes, Cuboid::new(0.004, 0.002, 0.1).into(), black, Transform::from_xyz(0.0, 0.0575, 0.04), layer);
    // Lever on the right side of the receiver: a bent bar you thumb over to break the gun open.
    part(w, meshes, Cuboid::new(0.006, 0.006, 0.05).into(), steel, Transform::from_xyz(0.0435, 0.03, 0.07).with_rotation(Quat::from_rotation_x(-0.12)), layer);
    part(w, meshes, Sphere::new(0.007).into(), steel, Transform::from_xyz(0.0435, 0.03, 0.098), layer);
    // Brass screws and weld beads.
    let mut screws = MeshData::default();
    for (y, z) in [(0.0, 0.0), (0.0, 0.07), (-0.03, 0.035)] {
        for x in [-1.0f32, 1.0] {
            screws.append(&meshgen::lathe(&[(0.006, 0.0), (0.006, 0.002), (0.004, 0.004), (0.0, 0.0045)], 8, 0.05, false, false).rotated_z(-x * FRAC_PI_2).translated([x * 0.039, y, z]));
        }
    }
    part(w, meshes, to_mesh_tangents(&screws), brass, t0, layer);
    let mut beads = MeshData::default();
    let mut z = -0.02;
    let mut n = 0;
    while z < 0.09 {
        n += 1;
        for x in [-0.039f32, 0.039] {
            beads.append(&meshgen::blob(0.0055, 0.8, 0.35, 300 + n, 0.05).translated([x, -0.047, z]));
        }
        z += 0.016;
    }
    part(w, meshes, to_mesh_tangents(&beads), weld, t0, layer);

    // ---- Hammers and triggers ----
    for x in [-0.02f32, 0.02] {
        // Low-profile hammers, so they don't block the sight line.
        let rest = Vec3::new(x, 0.034, 0.088);
        w.spawn((Transform::from_translation(rest), Visibility::default(), GunHammer(rest))).with_children(|hm| {
            part(hm, meshes, Cuboid::new(0.009, 0.018, 0.01).into(), steel, Transform::from_xyz(0.0, 0.009, 0.0), layer);
            part(hm, meshes, Cuboid::new(0.009, 0.005, 0.016).into(), steel, Transform::from_xyz(0.0, 0.017, 0.006), layer);
        });
    }
    for x in [-0.011f32, 0.011] {
        let rest = Vec3::new(x, -0.046, 0.03);
        w.spawn((Transform::from_translation(rest), Visibility::default(), GunTrigger(rest))).with_children(|t| {
            part(t, meshes, Cuboid::new(0.005, 0.022, 0.006).into(), steel, Transform::from_xyz(0.0, -0.011, 0.0).with_rotation(Quat::from_rotation_x(-0.25)), layer);
        });
    }
    part(w, meshes, Torus::new(0.017, 0.023).into(), steel, Transform::from_xyz(0.0, -0.064, 0.03).with_rotation(Quat::from_rotation_z(FRAC_PI_2)).with_scale(Vec3::new(1.0, 1.0, 1.4)), layer);
    // Shell rims seen at the breech when the gun is open and loaded.
    for x in [-0.0235f32, 0.0235] {
        w.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.0185, 0.006))),
            MeshMaterial3d(brass.clone()),
            Transform::from_xyz(x, BORE_Y, -0.029).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            NotShadowCaster,
            layer.clone(),
            ChamberRounds,
        ));
    }

    // ---- The hinged barrel assembly ----
    let hinge = Vec3::new(0.0, 0.0, -0.025);
    w.spawn((Transform::from_translation(hinge), Visibility::default(), BreakGroup(hinge))).with_children(|b| {
        for x in [-0.0235f32, 0.0235] {
            part(b, meshes, to_mesh_tangents(&along_z(&[(0.021, 0.0), (0.021, 0.765)], 16)), pipe, Transform::from_xyz(x, BORE_Y, 0.0), layer);
            // Welded muzzle ring.
            part(b, meshes, to_mesh_tangents(&along_z(&[(0.0235, 0.0), (0.0235, 0.012)], 16)), steel, Transform::from_xyz(x, BORE_Y, -0.754), layer);
            // Choke: a threaded tube screwed into each muzzle.
            upgrade_part(b, meshes, to_mesh_tangents(&along_z(&[(0.0245, 0.0), (0.0245, 0.05), (0.018, 0.056)], 16)), steel, Transform::from_xyz(x, BORE_Y, -0.76), layer, k, Upgrade::Choke);
        }
        // Breech block, rib and bead.
        part(b, meshes, to_mesh_tangents(&meshgen::cuboid([0.096, 0.052, 0.04], 0.12)), steel, Transform::from_xyz(0.0, 0.01, -0.02), layer);
        part(b, meshes, to_mesh_tangents(&meshgen::cuboid([0.012, 0.007, 0.74], 0.12)), steel, Transform::from_xyz(0.0, 0.0395, -0.39), layer);
        part(b, meshes, Cuboid::new(0.005, 0.014, 0.005).into(), steel, Transform::from_xyz(0.0, 0.0485, -0.745), layer);
        part(b, meshes, Sphere::new(0.0045).into(), brass, Transform::from_xyz(0.0, 0.0575, -0.745), layer);
        // Hose clamps and copper binding wire hold the pipes together.
        for z in [-0.16f32, -0.4, -0.62] {
            part(b, meshes, to_mesh_tangents(&band(BORE_Y, 0.0505, 0.0285, 0.014).translated([0.0, 0.0, z])), steel, t0, layer);
            part(b, meshes, Cuboid::new(0.014, 0.014, 0.016).into(), steel, Transform::from_xyz(0.0, BORE_Y - 0.0285, z), layer);
            part(b, meshes, Cylinder::new(0.004, 0.012).into(), brass, Transform::from_xyz(0.0, BORE_Y - 0.0345, z), layer);
        }
        let helix: Vec<[f32; 3]> = (0..=72)
            .map(|i| {
                let t = i as f32 / 72.0;
                let a = t * 5.0 * TAU;
                [a.cos() * 0.0505, BORE_Y + a.sin() * 0.0285, -0.52 - t * 0.05]
            })
            .collect();
        part(b, meshes, to_mesh_tangents(&meshgen::tube(&helix, 0.0022, 5)), copper, t0, layer);
        // Wooden forend lashed under the barrels.
        let forend = meshgen::loft_z(&[sec(-0.5, -0.012, 0.024, 0.02, 3.0), sec(-0.49, -0.012, 0.032, 0.026, 3.0), sec(-0.15, -0.012, 0.034, 0.028, 3.0), sec(-0.14, -0.01, 0.03, 0.025, 3.0)], 14, 0.12);
        part(b, meshes, to_mesh_tangents(&forend), wood, t0, layer);
    });

    // ---- Stock with a taped wrist and rubber butt pad ----
    let stock = meshgen::loft_z(
        &[sec(0.09, -0.005, 0.036, 0.05, 3.0), sec(0.18, -0.022, 0.03, 0.046, 3.0), sec(0.3, -0.055, 0.032, 0.066, 3.0), sec(0.45, -0.076, 0.036, 0.086, 3.0), sec(0.47, -0.077, 0.036, 0.087, 3.0)],
        16,
        0.12,
    );
    part(w, meshes, to_mesh_tangents(&stock), wood, t0, layer);
    for z in [0.14f32, 0.16, 0.18] {
        part(w, meshes, to_mesh_tangents(&band(-0.014, 0.034, 0.05, 0.016).translated([0.0, 0.0, z])), &m.tape, t0, layer);
    }
    part(w, meshes, Cuboid::new(0.076, 0.18, 0.016).into(), rubber, Transform::from_xyz(0.0, -0.077, 0.478), layer);
    // Sling.
    let (rear, front) = (Vec3::new(0.0, -0.17, 0.42), Vec3::new(0.0, -0.05, -0.42));
    for p in [rear, front] {
        part(w, meshes, Torus::new(0.006, 0.01).into(), steel, Transform::from_translation(p).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
    }
    let strap = meshgen::catenary((rear - Vec3::Y * 0.01).to_array(), (front - Vec3::Y * 0.01).to_array(), 0.06, 24);
    part(w, meshes, to_mesh_tangents(&meshgen::tube(&strap, 0.005, 6).scaled([2.4, 1.0, 1.0])), leather, t0, layer);

    // ---- Upgrade visuals ----
    upgrade_part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.088, 0.103, 0.09], 0.1)), fleece, Transform::from_xyz(0.0, 0.0, 0.045), layer, k, Upgrade::InsulatedAction);
    bandolier(w, meshes, m, layer, k, Vec3::new(0.04, -0.03, 0.22), Vec3::new(0.0, 0.0, 0.034));
}

/// The frontier revolver: swing-out cylinder, single-action hammer, plow-handle grip.
fn build_revolver(w: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers) {
    let Mats { steel, pipe, wood, black, brass, weld, leather, fleece, .. } = m;
    let t0 = Transform::default();
    let k = WeaponKind::Revolver;

    // ---- Frame, top strap and sights ----
    part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.036, 0.07, 0.1], 0.1)), steel, Transform::from_xyz(0.0, 0.0, 0.01), layer);
    part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.026, 0.012, 0.16], 0.1)), steel, Transform::from_xyz(0.0, 0.0405, -0.02), layer);
    // Rear sight: a notch cut in a block on the strap.
    for x in [-0.0085f32, 0.0085] {
        part(w, meshes, Cuboid::new(0.009, 0.014, 0.014).into(), steel, Transform::from_xyz(x, 0.059, 0.065), layer);
    }
    // Front blade on the barrel.
    part(w, meshes, Cuboid::new(0.004, 0.026, 0.012).into(), steel, Transform::from_xyz(0.0, 0.053, -0.275), layer);
    // ---- Barrel, ejector rod ----
    part(w, meshes, to_mesh_tangents(&along_z(&[(0.0165, 0.0), (0.0165, 0.2), (0.0145, 0.206)], 8)), pipe, Transform::from_xyz(0.0, BORE_Y, -0.085), layer);
    part(w, meshes, to_mesh_tangents(&along_z(&[(0.0075, 0.0), (0.0075, 0.14)], 8)), steel, Transform::from_xyz(0.0, -0.016, -0.085), layer);
    part(w, meshes, Sphere::new(0.0085).into(), steel, Transform::from_xyz(0.0, -0.016, -0.228), layer);
    part(w, meshes, to_mesh_tangents(&along_z(&[(0.019, 0.0), (0.019, 0.02)], 8)), steel, Transform::from_xyz(0.0, BORE_Y, -0.1), layer);
    // Weld beads where the barrel meets the frame.
    let mut beads = MeshData::default();
    for i in 0..10 {
        let a = i as f32 / 10.0 * TAU;
        beads.append(&meshgen::blob(0.0045, 1.0, 0.35, 400 + i, 0.05).translated([a.cos() * 0.0185, BORE_Y + a.sin() * 0.0185, -0.088]));
    }
    part(w, meshes, to_mesh_tangents(&beads), weld, t0, layer);
    // Screws.
    let mut screws = MeshData::default();
    for (y, z) in [(0.015, 0.045), (-0.02, 0.03)] {
        for x in [-1.0f32, 1.0] {
            screws.append(&meshgen::lathe(&[(0.0055, 0.0), (0.0055, 0.002), (0.0035, 0.004), (0.0, 0.0045)], 8, 0.05, false, false).rotated_z(-x * FRAC_PI_2).translated([x * 0.0185, y, z]));
        }
    }
    part(w, meshes, to_mesh_tangents(&screws), brass, t0, layer);

    // ---- The cylinder (swings out to the left to load) ----
    let rest = Vec3::new(0.0, 0.004, -0.05);
    w.spawn((Transform::from_translation(rest), Visibility::default(), CylinderGroup(rest))).with_children(|c| {
        part(c, meshes, to_mesh_tangents(&along_z(&[(0.0295, -0.035), (0.0295, 0.035), (0.026, 0.037)], 24)), steel, t0, layer);
        // Flutes between the chambers.
        for i in 0..6 {
            let a = (i as f32 + 0.5) / 6.0 * TAU;
            part(c, meshes, Cuboid::new(0.0035, 0.006, 0.05).into(), black, Transform::from_xyz(a.cos() * 0.0295, a.sin() * 0.0295, 0.0).with_rotation(Quat::from_rotation_z(a)), layer);
        }
        // Brass cartridge heads on the back face.
        for i in 0..6 {
            let a = i as f32 / 6.0 * TAU;
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.0068, 0.006))),
                MeshMaterial3d(brass.clone()),
                Transform::from_xyz(a.cos() * 0.0185, a.sin() * 0.0185, 0.036).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                NotShadowCaster,
                layer.clone(),
                ChamberRounds,
            ));
        }
    });

    // ---- Hammer, trigger, guard, grip ----
    let hammer_rest = Vec3::new(0.0, 0.032, 0.062);
    w.spawn((Transform::from_translation(hammer_rest), Visibility::default(), GunHammer(hammer_rest))).with_children(|hm| {
        part(hm, meshes, Cuboid::new(0.008, 0.026, 0.012).into(), steel, Transform::from_xyz(0.0, 0.013, 0.0), layer);
        part(hm, meshes, Cuboid::new(0.008, 0.006, 0.018).into(), steel, Transform::from_xyz(0.0, 0.026, 0.008), layer);
    });
    let trigger_rest = Vec3::new(0.0, -0.03, 0.03);
    w.spawn((Transform::from_translation(trigger_rest), Visibility::default(), GunTrigger(trigger_rest))).with_children(|t| {
        part(t, meshes, Cuboid::new(0.005, 0.02, 0.006).into(), steel, Transform::from_xyz(0.0, -0.01, 0.0).with_rotation(Quat::from_rotation_x(-0.3)), layer);
    });
    part(w, meshes, Torus::new(0.014, 0.019).into(), steel, Transform::from_xyz(0.0, -0.05, 0.03).with_rotation(Quat::from_rotation_z(FRAC_PI_2)).with_scale(Vec3::new(1.0, 1.0, 1.4)), layer);
    part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.03, 0.105, 0.042], 0.1)), wood, Transform::from_xyz(0.0, -0.078, 0.085).with_rotation(Quat::from_rotation_x(0.32)), layer);
    part(w, meshes, Cuboid::new(0.012, 0.11, 0.006).into(), brass, Transform::from_xyz(0.0, -0.075, 0.108).with_rotation(Quat::from_rotation_x(0.32)), layer);
    part(w, meshes, Torus::new(0.004, 0.009).into(), steel, Transform::from_xyz(0.0, -0.132, 0.1), layer);
    // Lanyard.
    let lanyard = meshgen::catenary([0.0, -0.138, 0.1], [0.0, -0.22, 0.14], 0.03, 10);
    part(w, meshes, to_mesh_tangents(&meshgen::tube(&lanyard, 0.003, 5)), leather, t0, layer);

    // ---- Upgrade visuals ----
    upgrade_part(w, meshes, to_mesh_tangents(&meshgen::cuboid([0.042, 0.076, 0.07], 0.1)), fleece, Transform::from_xyz(0.0, 0.0, 0.035), layer, k, Upgrade::InsulatedAction);
    bandolier(w, meshes, m, layer, k, Vec3::new(0.0185, -0.09, 0.07), Vec3::new(0.0, -0.0, 0.0));
}

/// The ice axe: a hickory haft with a leather-wrapped grip, a steel head with
/// a pick on one side and an adze on the other, a wrist loop, and rime.
#[allow(clippy::approx_constant)] // 0.318 m is the axe head's height, not 1/pi
fn build_axe(w: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers) {
    let Mats { steel, wood, leather, frost, .. } = m;
    let t0 = Transform::default();
    // Held leaning forward; everything below is built with the haft along +Y.
    w.spawn((Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(Quat::from_rotation_x(-0.55)), Visibility::default()))
        .with_children(|a| {
            let haft = meshgen::lathe(&[(0.0, -0.14), (0.03, -0.14), (0.027, -0.08), (0.022, 0.1), (0.026, 0.3), (0.0, 0.31)], 12, 0.12, false, false);
            part(a, meshes, to_mesh_tangents(&haft), wood, t0, layer);
            // Steel butt cap and leather grip wraps.
            part(a, meshes, to_mesh_tangents(&meshgen::lathe(&[(0.0325, -0.155), (0.0325, -0.135), (0.0, -0.135)], 12, 0.1, true, false)), steel, t0, layer);
            for k in 0..7 {
                let y = -0.12 + k as f32 * 0.024;
                part(a, meshes, to_mesh_tangents(&meshgen::lathe(&[(0.0325, y), (0.0325, y + 0.016)], 12, 0.05, true, true)), leather, t0, layer);
            }
            // Wrist loop.
            part(a, meshes, Torus::new(0.0045, 0.045).into(), leather, Transform::from_xyz(0.0, -0.09, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2 - 0.3)), layer);
            // Head: socket, curved pick forward (-Z), broad adze back (+Z).
            part(a, meshes, to_mesh_tangents(&meshgen::cuboid([0.05, 0.09, 0.07], 0.12)), steel, Transform::from_xyz(0.0, 0.31, 0.0), layer);
            let pick = meshgen::loft_z(
                &[sec(-0.3, 0.255, 0.003, 0.004, 2.5), sec(-0.22, 0.272, 0.008, 0.012, 2.5), sec(-0.13, 0.298, 0.014, 0.022, 2.5), sec(-0.05, 0.315, 0.018, 0.03, 2.5), sec(0.0, 0.318, 0.02, 0.036, 2.5)],
                12,
                0.12,
            );
            part(a, meshes, to_mesh_tangents(&pick), steel, t0, layer);
            let adze = meshgen::loft_z(&[sec(0.02, 0.318, 0.016, 0.034, 2.5), sec(0.1, 0.312, 0.006, 0.055, 2.5), sec(0.17, 0.302, 0.0025, 0.066, 2.5)], 12, 0.12);
            part(a, meshes, to_mesh_tangents(&adze), steel, t0, layer);
            // Rime frozen onto the head and haft.
            for (i, (pos, r)) in [([0.0, 0.365, 0.0], 0.04f32), ([0.0, 0.32, -0.16], 0.026), ([0.0, 0.31, 0.12], 0.03), ([0.03, 0.22, 0.0], 0.02)].into_iter().enumerate() {
                part(a, meshes, to_mesh_tangents(&meshgen::blob(r, 0.7, 0.3, 500 + i as u64, 0.05).translated(pos)), frost, t0, layer);
            }
        });
}

/// Where hoar frost gathers on each weapon: (position, size) in gun space,
/// along the barrel, on the receiver and on the cold grips.
fn frost_spots(kind: WeaponKind) -> &'static [([f32; 3], [f32; 3])] {
    match kind {
        WeaponKind::PipeRifle => &[
            ([0.0, 0.034, -0.45], [0.026, 0.006, 0.17]),
            ([0.0, 0.034, -0.72], [0.022, 0.005, 0.08]),
            ([0.0, 0.062, -0.03], [0.03, 0.007, 0.12]),
            ([0.0, 0.035, 0.2], [0.03, 0.007, 0.1]),
        ],
        WeaponKind::ScrapShotgun => &[
            ([0.0, 0.034, -0.5], [0.03, 0.006, 0.2]),
            ([0.0, 0.05, -0.12], [0.034, 0.007, 0.1]),
            ([0.0, 0.03, 0.22], [0.03, 0.007, 0.11]),
        ],
        WeaponKind::Revolver => &[([0.0, 0.042, -0.17], [0.02, 0.005, 0.09]), ([0.0, 0.066, 0.0], [0.024, 0.006, 0.05])],
        WeaponKind::IceAxe => &[],
    }
}

/// Soft, flat blobs of rime laid on the metal. They're invisible until the
/// cold gets at them.
fn frost_patches(g: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, m: &Mats, layer: &RenderLayers, kind: WeaponKind) {
    let patch = meshes.add(to_mesh_tangents(&meshgen::blob(1.0, 0.45, 0.3, 700 + kind.slot() as u64, 2.0)));
    for (pos, size) in frost_spots(kind) {
        g.spawn((
            Mesh3d(patch.clone()),
            MeshMaterial3d(m.rime.clone()),
            Transform::from_translation(Vec3::from_array(*pos) + Vec3::Y * 0.004).with_scale(Vec3::from_array(*size) * Vec3::new(1.7, 2.2, 1.5)),
            NotShadowCaster,
            layer.clone(),
        ));
    }
}

/// The weapons' frost material; its opacity is how frosted over they are.
#[derive(Resource)]
struct GunFrost(Handle<StandardMaterial>);

/// Cold metal grows hoar frost outdoors (faster the colder it is) and loses
/// it in a warm room; firing warms the barrel and knocks some off.
fn update_gun_frost(
    time: Res<Time>,
    weather: Res<crate::state::WeatherRes>,
    clock: Res<crate::state::ClockRes>,
    interior: Res<crate::state::CurrentInterior>,
    game: Res<Game>,
    frost: Option<Res<GunFrost>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut amount: Local<f32>,
    mut seeded: Local<bool>,
) {
    let Some(frost) = frost else { return };
    if !std::mem::replace(&mut *seeded, true) {
        // Screenshots can start with the guns already frosted: FMN_GUNFROST=0..1.
        *amount = std::env::var("FMN_GUNFROST").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    }
    let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();
    let target = if interior.0.is_some() { 0.0 } else { ((12.0 - temp) / 45.0).clamp(0.0, 1.0) };
    let dt = time.delta_secs();
    let rate = if target > *amount { 0.02 } else { 0.12 };
    *amount += (target - *amount).clamp(-rate * dt, rate * dt);
    if game.recoil > 0.9 {
        *amount = (*amount - 0.02).max(0.0);
    }
    if let Some(m) = materials.get_mut(&frost.0) {
        let a = (*amount).clamp(0.0, 1.0);
        if (m.base_color.alpha() - a).abs() > 0.004 {
            m.base_color = Color::srgba(0.9, 0.96, 1.0, a);
        }
    }
}

/// Show only the weapon in hand.
fn show_current_weapon(game: Res<Game>, mut q: Query<(&mut Visibility, &WeaponModel)>) {
    let current = game.weapon().kind;
    for (mut vis, model) in &mut q {
        let want = if model.0 == current { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Show the parts for upgrades that have been fitted.
fn show_upgrades(game: Res<Game>, mut q: Query<(&mut Visibility, &UpgradeVisual)>) {
    for (mut vis, UpgradeVisual(kind, up)) in &mut q {
        let want = if game.arsenal.get(*kind).has_upgrade(*up) { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Right mouse: aim down the sights (zooms both cameras). Sprinting, reloading
/// and being dead cancel it.
fn aim_down_sights(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    game: Res<Game>,
    motion: Res<AccumulatedMouseMotion>,
    colliders: Res<Colliders>,
    mut state: ResMut<GunState>,
    force: Res<ForceAim>,
    mut aim: ResMut<AimAmount>,
    player: Query<(&Transform, &Player)>,
    mut projections: Query<(&mut Projection, Has<Player>), Or<(With<Player>, With<ViewModelCamera>)>>,
    settings: Res<crate::menu::GameSettings>,
) {
    let dt = time.delta_secs().max(1e-4);
    let Ok((tf, p)) = player.single() else { return };
    let want = (force.0 || (mouse.pressed(MouseButton::Right) && cursor_locked(&windows)))
        && game.death.is_none()
        && !p.sprinting
        && !game.weapon().is_reloading()
        && viewmodel::can_aim(game.weapon().kind);
    let target = if want { 1.0 } else { 0.0 };
    state.ads += (target - state.ads) * (dt * 12.0).min(1.0);
    aim.0 = state.ads;

    // Mouse-look velocity, smoothed, for sway.
    let look = if cursor_locked(&windows) { motion.delta / dt * 0.0022 } else { Vec2::ZERO };
    state.look = state.look.lerp(look, (dt * 10.0).min(1.0));

    // Muzzle near a wall or tree?
    let fwd = Vec2::new(-p.yaw.sin(), -p.yaw.cos());
    let pos = Vec2::new(tf.translation.x, tf.translation.z);
    let blocked_at = |d: f32| {
        let q = pos + fwd * d;
        collision::blocked(q.x, q.y, 0.12, &colliders.0)
    };
    let blocked = if blocked_at(0.55) {
        1.0
    } else if blocked_at(0.8) {
        0.6
    } else {
        0.0
    };
    state.blocked += (blocked - state.blocked) * (dt * 8.0).min(1.0);

    // Walk cycle from real distance covered.
    if let Some(last) = state.last_pos {
        let moved = Vec2::new(tf.translation.x - last.x, tf.translation.z - last.z).length();
        if moved < 2.0 {
            state.stride += moved * 2.4;
        }
    }
    state.last_pos = Some(tf.translation);

    // The world uses the player's field of view; the weapon keeps its own so it
    // looks the same at any setting. Aiming narrows both by the same ratio.
    let zoom = 1.0 + (ADS_FOV / HIP_FOV - 1.0) * state.ads;
    for (mut proj, world) in &mut projections {
        let fov = (if world { settings.0.fov } else { HIP_FOV } * zoom).to_radians();
        if let Projection::Perspective(pp) = proj.as_mut() {
            if (pp.fov - fov).abs() > 1e-4 {
                pp.fov = fov;
            }
        }
    }
}

fn pose_gun(
    time: Res<Time>,
    mut game: ResMut<Game>,
    state: Res<GunState>,
    mut last: ResMut<LastPose>,
    player: Query<&Player>,
    mut parts: ParamSet<(
        Query<&mut Transform, With<GunModel>>,
        Query<(&mut Transform, &GunBolt)>,
        Query<(&mut Transform, &GunMag)>,
        Query<(&mut Transform, &GunHammer)>,
        Query<(&mut Transform, &GunTrigger)>,
        Query<(&mut Transform, &BreakGroup)>,
        Query<(&mut Transform, &CylinderGroup)>,
        Query<&mut Visibility, With<ChamberRounds>>,
    )>,
) {
    let dt = time.delta_secs();
    game.recoil = (game.recoil - dt * 6.0).max(0.0);
    let (moving, sprinting) = player.single().map(|p| (p.moving, p.sprinting)).unwrap_or((false, false));
    let weapon = game.weapon();
    // The axe's swing runs on its recovery timer (but not while drawing it).
    let swing = (weapon.melee && game.arsenal.draw <= 0.0 && weapon.cooldown > 0.0)
        .then(|| 1.0 - weapon.cooldown / weapon.fire_interval);
    let pose = viewmodel::pose(&Inputs {
        kind: weapon.kind,
        swing,
        draw: game.arsenal.draw / crate::sim::combat::DRAW_SECS,
        time: time.elapsed_secs(),
        stride: state.stride,
        moving,
        sprinting,
        recoil: game.recoil,
        reload: game.weapon().reload_progress(),
        unjamming: game.weapon().jammed,
        ads: state.ads,
        blocked: state.blocked,
        look: state.look.to_array(),
    });
    for mut tf in &mut parts.p0() {
        tf.translation = Vec3::from_array(pose.pos);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, pose.rot[1], pose.rot[0], pose.rot[2]);
    }
    for (mut tf, b) in &mut parts.p1() {
        tf.translation = b.0 + Vec3::Z * pose.bolt * 0.07;
    }
    for (mut tf, m) in &mut parts.p2() {
        match pose.mag_pos {
            // Out of the gun, in the left hand.
            Some(at) => {
                tf.translation = Vec3::from_array(at);
                tf.rotation = Quat::from_rotation_x(0.25);
            }
            None => {
                tf.translation = m.0 - Vec3::Y * pose.mag_drop;
                tf.rotation = Quat::from_rotation_x(pose.mag_drop * 0.8);
            }
        }
    }
    for (mut tf, hm) in &mut parts.p3() {
        tf.translation = hm.0;
        // Cocked leans back; fallen stands upright against the firing pin.
        tf.rotation = Quat::from_rotation_x(0.75 * (1.0 - pose.hammer));
    }
    for (mut tf, tr) in &mut parts.p4() {
        tf.translation = tr.0;
        tf.rotation = Quat::from_rotation_x(-0.35 * pose.trigger);
    }
    // Shotgun barrels tip down about the hinge.
    for (mut tf, b) in &mut parts.p5() {
        tf.translation = b.0;
        tf.rotation = Quat::from_rotation_x(-0.62 * pose.open);
    }
    // The revolver cylinder swings out to the left and spins.
    for (mut tf, c) in &mut parts.p6() {
        tf.translation = c.0 + Vec3::new(-0.065 * pose.open, -0.012 * pose.open, 0.0);
        tf.rotation = Quat::from_rotation_y(-0.6 * pose.open) * Quat::from_rotation_z(pose.spin);
    }
    for mut vis in &mut parts.p7() {
        let want = if pose.chambers_loaded { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    last.0 = Some(pose);
}

/// Keeps the hip-fire constants honest: the sights sit on the gun's top.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rifle_sight_height_matches_the_pose_maths() {
        assert_eq!(RIFLE_SIGHT, viewmodel::sight_y(WeaponKind::PipeRifle));
        for kind in [WeaponKind::PipeRifle, WeaponKind::ScrapShotgun, WeaponKind::Revolver] {
            assert!(viewmodel::sight_y(kind) > BORE_Y + 0.04, "{kind:?}: sights sit above the bore");
            let m = muzzle_local(kind);
            assert!(m.z < -0.25 && (m.y - BORE_Y).abs() < 1e-6, "{kind:?} muzzle");
        }
        // Brass leaves from behind the muzzle on every firearm.
        for kind in [WeaponKind::PipeRifle, WeaponKind::ScrapShotgun, WeaponKind::Revolver] {
            assert!(breech_local(kind).z > muzzle_local(kind).z);
        }
    }

    #[test]
    fn muzzle_is_in_front_of_the_barrel() {
        let m = muzzle_local(crate::sim::combat::WeaponKind::PipeRifle);
        assert!(m.z < -0.8 && (m.y - BORE_Y).abs() < 1e-6);
        assert!(RIFLE_SIGHT > BORE_Y + 0.05);
    }
}
