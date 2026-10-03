//! The first-person pipe rifle: a scrap-built view model drawn by its own
//! camera (so it never clips into walls or trees), animated from the pure
//! pose maths in `sim::viewmodel` - idle sway, walk/sprint bob, look lag,
//! recoil with a falling hammer and pulled trigger, magazine-out reloads,
//! bolt-racking unjams, lowering against walls - and aim-down-sights on the
//! right mouse button.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::camera::ClearColorConfig;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;

use crate::assets::GameAssets;
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::player::{cursor_locked, Player};
use crate::sim::collision;
use crate::sim::meshgen::{self, sec, MeshData};
use crate::sim::viewmodel::{self, Inputs, SCALE, SIGHT_Y};
use crate::state::{Colliders, Game};

/// Render layer for the view model and its camera.
pub const VIEW_LAYER: usize = 1;
/// Muzzle position in the gun model's local space.
pub fn muzzle_local(_kind: crate::sim::combat::WeaponKind) -> Vec3 {
    Vec3::new(0.0, 0.012, -0.81)
}
const HIP_FOV: f32 = 75.0;
const ADS_FOV: f32 = 52.0;
/// Barrel axis height in gun space.
const BORE_Y: f32 = 0.012;

/// The gun root (child of the camera).
#[derive(Component)]
pub struct GunModel;
#[derive(Component)]
struct ViewModelCamera;
#[derive(Component)]
struct GunBolt(Vec3);
#[derive(Component)]
struct GunMag(Vec3);
#[derive(Component)]
struct GunHammer(Vec3);
#[derive(Component)]
struct GunTrigger(Vec3);

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

pub struct GunPlugin;

impl Plugin for GunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GunState>()
            .init_resource::<ForceAim>()
            .init_resource::<AimAmount>()
            .add_systems(Update, (aim_down_sights, pose_gun).chain());
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
    let h = viewmodel::HIP;
    cam.spawn((
        Transform::from_xyz(h[0], h[1], h[2]).with_scale(Vec3::splat(SCALE)),
        Visibility::default(),
        GunModel,
    ))
    .with_children(|gun| build_pipe_rifle(gun, meshes, materials, assets, &layer));
}

/// A part of the rifle on the view-model layer.
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

fn build_pipe_rifle(
    gun: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &GameAssets,
    layer: &RenderLayers,
) {
    let tex = |name: &str| assets.gun_textures.get(name).cloned();
    let steel = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: tex("steel_diff"),
        metallic_roughness_texture: tex("steel_arm"),
        occlusion_texture: tex("steel_arm"),
        metallic: 1.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    let pipe = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.9),
        base_color_texture: tex("steel_diff"),
        metallic_roughness_texture: tex("steel_arm"),
        metallic: 1.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    let wood = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: tex("wood_diff"),
        normal_map_texture: tex("wood_nor"),
        perceptual_roughness: 0.62,
        ..default()
    });
    let flat = |materials: &mut Assets<StandardMaterial>, c: Color, metal: f32, rough: f32| {
        materials.add(StandardMaterial {
            base_color: c,
            metallic: metal,
            perceptual_roughness: rough,
            ..default()
        })
    };
    let black = flat(materials, Color::srgb(0.02, 0.02, 0.02), 0.0, 0.9);
    let tape = flat(materials, Color::srgb(0.24, 0.26, 0.22), 0.0, 0.85);
    let copper = flat(materials, Color::srgb(0.75, 0.42, 0.24), 1.0, 0.35);
    let brass = flat(materials, Color::srgb(0.8, 0.62, 0.28), 1.0, 0.3);
    let weld = flat(materials, Color::srgb(0.2, 0.18, 0.17), 0.7, 0.6);
    let leather = flat(materials, Color::srgb(0.26, 0.15, 0.08), 0.0, 0.75);
    let rubber = flat(materials, Color::srgb(0.05, 0.05, 0.05), 0.0, 0.95);
    let t0 = Transform::default();

    // ---- Receiver: one solid block every other part bolts on to ----
    part(gun, meshes, to_mesh_tangents(&meshgen::cuboid([0.072, 0.095, 0.3], 0.12)), &steel, Transform::from_xyz(0.0, 0.0, -0.02), layer);
    part(gun, meshes, to_mesh_tangents(&meshgen::cuboid([0.06, 0.012, 0.22], 0.12)), &steel, Transform::from_xyz(0.0, 0.0535, -0.03), layer);
    // Ejection port and bolt slot on the right.
    part(gun, meshes, Cuboid::new(0.002, 0.03, 0.075).into(), &black, Transform::from_xyz(0.0365, 0.015, -0.02), layer);
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
        let a = i as f32 / 14.0 * 2.0 * PI;
        beads.append(&meshgen::blob(0.006, 1.0, 0.35, 100 + i, 0.05).translated([a.cos() * 0.034, BORE_Y + a.sin() * 0.034, -0.168]));
    }
    part(gun, meshes, to_mesh(&beads), &weld, t0, layer);
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
    part(gun, meshes, to_mesh(&screws), &brass, t0, layer);

    // ---- Barrel: plumbing pipe with hex couplings and a muzzle cap ----
    let barrel_at = |m: MeshData, z: f32| m.translated([0.0, BORE_Y, z]);
    part(gun, meshes, to_mesh(&barrel_at(along_z(&[(0.034, 0.0), (0.034, 0.05)], 6), -0.165)), &steel, t0, layer);
    part(gun, meshes, to_mesh(&barrel_at(along_z(&[(0.022, 0.0), (0.022, 0.585)], 16), -0.2)), &pipe, t0, layer);
    part(gun, meshes, to_mesh(&barrel_at(along_z(&[(0.03, 0.0), (0.031, 0.04)], 6), -0.47)), &steel, t0, layer);
    part(gun, meshes, to_mesh(&barrel_at(along_z(&[(0.029, 0.0), (0.029, 0.035), (0.017, 0.042)], 12), -0.765)), &steel, t0, layer);
    part(gun, meshes, Circle::new(0.012).into(), &black, Transform::from_xyz(0.0, BORE_Y, -0.8075).with_rotation(Quat::from_rotation_y(PI)), layer);

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
    part(gun, meshes, to_mesh_tangents(&handguard), &wood, t0, layer);
    for z in [-0.29f32, -0.6] {
        part(gun, meshes, to_mesh(&band(-0.009, 0.033, 0.05, 0.012).translated([0.0, 0.0, z])), &steel, t0, layer);
        part(gun, meshes, Cuboid::new(0.012, 0.016, 0.016).into(), &steel, Transform::from_xyz(0.035, -0.009, z), layer);
        part(gun, meshes, Cylinder::new(0.004, 0.012).into(), &brass, Transform::from_xyz(0.035, 0.002, z).with_rotation(Quat::from_rotation_x(0.0)), layer);
    }
    // Copper wire binding: a helix around barrel and handguard.
    let helix: Vec<[f32; 3]> = (0..=90)
        .map(|i| {
            let t = i as f32 / 90.0;
            let a = t * 6.0 * 2.0 * PI;
            [a.cos() * 0.032, -0.009 + a.sin() * 0.049, -0.4 - t * 0.04]
        })
        .collect();
    part(gun, meshes, to_mesh(&meshgen::tube(&helix, 0.0022, 5)), &copper, t0, layer);

    // ---- Sights: rear peep on the cover, front post with guard wings ----
    part(gun, meshes, Cuboid::new(0.024, 0.006, 0.02).into(), &steel, Transform::from_xyz(0.0, 0.0625, 0.09), layer);
    part(gun, meshes, Cuboid::new(0.006, 0.008, 0.004).into(), &steel, Transform::from_xyz(0.0, 0.066, 0.09), layer);
    part(
        gun,
        meshes,
        Torus::new(0.0055, 0.0105).into(),
        &steel,
        Transform::from_xyz(0.0, SIGHT_Y, 0.09).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        layer,
    );
    part(gun, meshes, Cuboid::new(0.02, 0.012, 0.02).into(), &steel, Transform::from_xyz(0.0, 0.04, -0.74), layer);
    part(gun, meshes, Cuboid::new(0.005, 0.03, 0.005).into(), &steel, Transform::from_xyz(0.0, SIGHT_Y - 0.015, -0.74), layer);
    for x in [-0.012f32, 0.012] {
        part(gun, meshes, Cuboid::new(0.004, 0.028, 0.012).into(), &steel, Transform::from_xyz(x, 0.058, -0.74), layer);
    }

    // ---- Lower: trigger guard, trigger, grip, magazine ----
    part(
        gun,
        meshes,
        Torus::new(0.016, 0.021).into(),
        &steel,
        Transform::from_xyz(0.0, -0.066, 0.03).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        layer,
    );
    let trigger_rest = Vec3::new(0.0, -0.048, 0.024);
    gun.spawn((Transform::from_translation(trigger_rest), Visibility::default(), GunTrigger(trigger_rest)))
        .with_children(|t| {
            part(t, meshes, Cuboid::new(0.005, 0.022, 0.006).into(), &steel, Transform::from_xyz(0.0, -0.011, 0.0).with_rotation(Quat::from_rotation_x(-0.25)), layer);
        });
    part(
        gun,
        meshes,
        to_mesh_tangents(&meshgen::cuboid([0.034, 0.11, 0.046], 0.12)),
        &wood,
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
                &steel,
                Transform::from_xyz(0.0, -0.058, 0.004).with_rotation(Quat::from_rotation_x(-0.12)),
                layer,
            );
            part(m, meshes, Cuboid::new(0.034, 0.008, 0.056).into(), &steel, Transform::from_xyz(0.0, -0.118, 0.011).with_rotation(Quat::from_rotation_x(-0.12)), layer);
            // The top round peeking out of the feed lips.
            part(m, meshes, Cylinder::new(0.0045, 0.03).into(), &brass, Transform::from_xyz(0.0, 0.002, -0.004).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), layer);
        });

    // ---- Action: hammer and bolt handle ----
    let hammer_rest = Vec3::new(0.0, 0.036, 0.128);
    gun.spawn((Transform::from_translation(hammer_rest), Visibility::default(), GunHammer(hammer_rest)))
        .with_children(|hm| {
            part(hm, meshes, Cuboid::new(0.01, 0.026, 0.01).into(), &steel, Transform::from_xyz(0.0, 0.013, 0.0), layer);
            part(hm, meshes, Cuboid::new(0.01, 0.006, 0.018).into(), &steel, Transform::from_xyz(0.0, 0.025, 0.007), layer);
        });
    let bolt_rest = Vec3::new(0.037, 0.02, 0.0);
    gun.spawn((Transform::from_translation(bolt_rest), Visibility::default(), GunBolt(bolt_rest)))
        .with_children(|b| {
            part(b, meshes, Cylinder::new(0.0055, 0.045).into(), &steel, Transform::from_xyz(0.022, 0.0, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)), layer);
            part(b, meshes, Sphere::new(0.012).into(), &steel, Transform::from_xyz(0.047, 0.0, 0.0), layer);
        });

    // ---- Stock: carved wood, taped wrist, rubber butt pad ----
    let stock_secs = [
        sec(0.125, -0.005, 0.033, 0.045, 3.0),
        sec(0.2, -0.025, 0.028, 0.04, 3.0),
        sec(0.3, -0.05, 0.031, 0.06, 3.0),
        sec(0.41, -0.065, 0.034, 0.078, 3.0),
        sec(0.43, -0.066, 0.034, 0.079, 3.0),
    ];
    part(gun, meshes, to_mesh_tangents(&meshgen::loft_z(&stock_secs, 16, 0.12)), &wood, t0, layer);
    for z in [0.18f32, 0.2, 0.22] {
        // Interpolate the stock's cross-section at this point.
        let t = (z - 0.125) / (0.2 - 0.125);
        let (y, rx, ry) = if z <= 0.2 {
            (-0.005 - 0.02 * t, 0.033 - 0.005 * t, 0.045 - 0.005 * t)
        } else {
            let t = (z - 0.2) / 0.1;
            (-0.025 - 0.025 * t, 0.028 + 0.003 * t, 0.04 + 0.02 * t)
        };
        part(gun, meshes, to_mesh(&band(y, rx + 0.002, ry + 0.002, 0.018).translated([0.0, 0.0, z])), &tape, t0, layer);
    }
    part(gun, meshes, Cuboid::new(0.07, 0.16, 0.018).into(), &rubber, Transform::from_xyz(0.0, -0.066, 0.44), layer);

    // ---- Sling with swivels ----
    let rear = Vec3::new(0.0, -0.152, 0.37);
    let front = Vec3::new(0.0, -0.064, -0.48);
    for p in [rear, front] {
        part(
            gun,
            meshes,
            Torus::new(0.006, 0.01).into(),
            &steel,
            Transform::from_translation(p).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            layer,
        );
    }
    let strap = meshgen::catenary((rear - Vec3::Y * 0.01).to_array(), (front - Vec3::Y * 0.01).to_array(), 0.05, 24);
    part(gun, meshes, to_mesh(&meshgen::tube(&strap, 0.005, 6).scaled([2.4, 1.0, 1.0])), &leather, t0, layer);
}

/// Right mouse: aim down the sights (zooms both cameras). Sprinting, reloading
/// and being dead cancel it.
#[allow(clippy::too_many_arguments)]
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
    mut projections: Query<&mut Projection, Or<(With<Player>, With<ViewModelCamera>)>>,
) {
    let dt = time.delta_secs().max(1e-4);
    let Ok((tf, p)) = player.single() else { return };
    let want = (force.0 || (mouse.pressed(MouseButton::Right) && cursor_locked(&windows))) && game.death.is_none() && !p.sprinting && !game.weapon().is_reloading();
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

    let fov = (HIP_FOV + (ADS_FOV - HIP_FOV) * state.ads).to_radians();
    for mut proj in &mut projections {
        if let Projection::Perspective(pp) = proj.as_mut() {
            if (pp.fov - fov).abs() > 1e-4 {
                pp.fov = fov;
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn pose_gun(
    time: Res<Time>,
    mut game: ResMut<Game>,
    state: Res<GunState>,
    player: Query<&Player>,
    mut parts: ParamSet<(
        Query<&mut Transform, With<GunModel>>,
        Query<(&mut Transform, &GunBolt)>,
        Query<(&mut Transform, &GunMag)>,
        Query<(&mut Transform, &GunHammer)>,
        Query<(&mut Transform, &GunTrigger)>,
    )>,
) {
    let dt = time.delta_secs();
    game.recoil = (game.recoil - dt * 6.0).max(0.0);
    let (moving, sprinting) = player.single().map(|p| (p.moving, p.sprinting)).unwrap_or((false, false));
    let pose = viewmodel::pose(&Inputs {
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
        tf.translation = m.0 - Vec3::Y * pose.mag_drop;
        tf.rotation = Quat::from_rotation_x(pose.mag_drop * 0.8);
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
}

/// Keeps the hip-fire constants honest: the sights sit on the gun's top.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muzzle_is_in_front_of_the_barrel() {
        let m = muzzle_local(crate::sim::combat::WeaponKind::PipeRifle);
        assert!(m.z < -0.8 && (m.y - BORE_Y).abs() < 1e-6);
        assert!(SIGHT_Y > BORE_Y + 0.05);
    }
}
