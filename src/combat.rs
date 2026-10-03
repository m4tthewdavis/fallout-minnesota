//! Pipe rifle shooting: hitscan against Frostfangs, tracers, muzzle flash,
//! recoil, reloads and cold-weather jams.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use std::f32::consts::FRAC_PI_2;

use crate::assets::GameAssets;
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::particles::ground_hit;
use crate::player::{cursor_locked, GunModel, Player};
use crate::sim::combat::{ray_sphere, FireResult};
use crate::sim::meshgen::{self, sec};
use crate::sim::synth::Sound;
use crate::state::{alive, ClockRes, Fx, FxQueue, Game, Lifetime, Messages, RngRes, SfxQueue, WeatherRes};
use crate::wolves::Wolf;

/// Muzzle position in the gun model's local space.
const MUZZLE: Vec3 = Vec3::new(0.0, 0.02, -0.78);

/// The rifle's bolt handle, which snaps back when firing and reloading.
#[derive(Component)]
pub struct GunBolt {
    rest: Vec3,
}

/// Builds the pipe rifle view model: a scrap receiver, a plumbing-pipe
/// barrel with couplings and tape, a carved wooden stock and handguard,
/// magazine, sights and a working bolt. Faces -Z.
pub fn build_pipe_rifle(
    gun: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    assets: &GameAssets,
) {
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.19, 0.11),
        perceptual_roughness: 0.6,
        ..default()
    });
    let blued = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.18, 0.2),
        metallic: 0.9,
        perceptual_roughness: 0.35,
        ..default()
    });
    let receiver = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.4, 0.38),
        base_color_texture: Some(assets.rust_diff.clone()),
        normal_map_texture: Some(assets.rust_normal.clone()),
        metallic: 0.6,
        perceptual_roughness: 0.55,
        ..default()
    });
    let tape = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.27, 0.25),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mut part = |mesh: Mesh, material: &Handle<StandardMaterial>, tf: Transform| {
        gun.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), tf, NotShadowCaster));
    };
    // Receiver.
    part(to_mesh_tangents(&meshgen::cuboid([0.07, 0.09, 0.27], 0.6)), &receiver, Transform::default());
    // Barrel: lathe along +Y, turned to point down -Z.
    let barrel = meshgen::lathe(
        &[
            (0.0, -0.02),
            (0.034, -0.02),
            (0.034, 0.05),
            (0.024, 0.06),
            (0.024, 0.28),
            (0.033, 0.29),
            (0.033, 0.35),
            (0.024, 0.36),
            (0.024, 0.585),
            (0.03, 0.59),
            (0.03, 0.64),
            (0.0, 0.64),
        ],
        14,
        0.2,
        false,
        false,
    )
    .rotated_x(-FRAC_PI_2);
    part(to_mesh(&barrel), &blued, Transform::from_xyz(0.0, 0.02, -0.13));
    for z in [-0.3f32, -0.58] {
        let band = meshgen::lathe(&[(0.027, 0.0), (0.027, 0.035)], 14, 0.2, false, false).rotated_x(-FRAC_PI_2);
        part(to_mesh(&band), &tape, Transform::from_xyz(0.0, 0.02, z));
    }
    // Wooden handguard under the barrel and the stock behind the receiver.
    let handguard = meshgen::loft_z(
        &[sec(-0.46, -0.035, 0.022, 0.018, 3.0), sec(-0.44, -0.035, 0.032, 0.028, 3.0), sec(-0.15, -0.035, 0.034, 0.03, 3.0)],
        12,
        0.3,
    );
    part(to_mesh(&handguard), &wood, Transform::default());
    let stock = meshgen::loft_z(
        &[
            sec(0.12, -0.005, 0.03, 0.04, 3.0),
            sec(0.24, -0.035, 0.028, 0.045, 3.0),
            sec(0.38, -0.065, 0.032, 0.075, 3.0),
            sec(0.43, -0.07, 0.033, 0.08, 3.0),
            sec(0.44, -0.07, 0.03, 0.075, 3.0),
        ],
        12,
        0.3,
    );
    part(to_mesh(&stock), &wood, Transform::default());
    // Pistol grip, trigger guard and magazine.
    part(
        Cuboid::new(0.038, 0.11, 0.05).into(),
        &wood,
        Transform::from_xyz(0.0, -0.085, 0.075).with_rotation(Quat::from_rotation_x(0.35)),
    );
    part(
        Torus::new(0.018, 0.024).into(),
        &blued,
        Transform::from_xyz(0.0, -0.05, 0.02).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    part(
        Cuboid::new(0.034, 0.11, 0.055).into(),
        &blued,
        Transform::from_xyz(0.0, -0.09, -0.06).with_rotation(Quat::from_rotation_x(-0.15)),
    );
    // Sights.
    part(Cuboid::new(0.008, 0.028, 0.01).into(), &blued, Transform::from_xyz(0.0, 0.055, -0.74));
    part(Cuboid::new(0.03, 0.022, 0.012).into(), &blued, Transform::from_xyz(0.0, 0.055, -0.06));
    // Bolt handle.
    let rest = Vec3::new(0.05, 0.025, 0.04);
    gun.spawn((Transform::from_translation(rest), Visibility::default(), GunBolt { rest }))
        .with_children(|b| {
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.008, 0.05))),
                MeshMaterial3d(blued.clone()),
                Transform::from_xyz(0.02, 0.0, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                NotShadowCaster,
            ));
            b.spawn((
                Mesh3d(meshes.add(Sphere::new(0.014))),
                MeshMaterial3d(blued.clone()),
                Transform::from_xyz(0.045, 0.0, 0.0),
                NotShadowCaster,
            ));
        });
}

#[derive(Resource)]
struct FxAssets {
    tracer: Handle<Mesh>,
    tracer_mat: Handle<StandardMaterial>,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_fx).add_systems(
            Update,
            ((weapon_timers, reload, fire).chain().run_if(alive), animate_gun),
        );
    }
}

fn setup_fx(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(FxAssets {
        tracer: meshes.add(Cuboid::new(0.02, 0.02, 1.0)),
        tracer_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.5),
            emissive: LinearRgba::rgb(6.0, 4.0, 1.0),
            unlit: true,
            ..default()
        }),
    });
}

fn weapon_timers(time: Res<Time>, mut game: ResMut<Game>, mut msgs: ResMut<Messages>) {
    let was_jammed = game.weapon.jammed;
    if game.weapon.tick(time.delta_secs()) && was_jammed {
        msgs.show("Jam cleared.", 1.5);
    }
    game.hurt_flash = (game.hurt_flash - time.delta_secs() * 1.5).max(0.0);
}

fn reload(
    keys: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Game { weapon, inv, .. } = &mut *game;
    let jammed = weapon.jammed;
    if weapon.start_reload(&mut inv.ammo_reserve) {
        sfx.play(if jammed { Sound::Jam } else { Sound::Reload });
        if jammed {
            msgs.show("Working the frozen bolt loose...", 1.2);
        }
    } else if inv.ammo_reserve == 0 && weapon.mag == 0 {
        msgs.show("Out of ammo. Search the snow for pipe rounds.", 2.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn fire(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    fx: Res<FxAssets>,
    mut game: ResMut<Game>,
    weather: Res<WeatherRes>,
    mut rng: ResMut<RngRes>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut fxq: ResMut<FxQueue>,
    clock: Res<ClockRes>,
    cam: Query<&GlobalTransform, With<Player>>,
    gun: Query<&GlobalTransform, With<GunModel>>,
    mut wolves: Query<(Entity, &Transform, &mut Wolf), Without<Player>>,
) {
    if !mouse.pressed(MouseButton::Left) || !cursor_locked(&windows) {
        return;
    }
    let Ok(cam) = cam.single() else { return };
    let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();

    match game.weapon.try_fire(temp, &mut rng.0) {
        FireResult::Busy => return,
        FireResult::Empty => {
            if mouse.just_pressed(MouseButton::Left) {
                sfx.play(Sound::DryClick);
                msgs.show("*click* Empty. Press R to reload.", 1.5);
            }
            return;
        }
        FireResult::Jammed => {
            if mouse.just_pressed(MouseButton::Left) {
                sfx.play(Sound::DryClick);
                msgs.show("JAMMED - the cold seized the action. Press R to clear it.", 2.0);
            }
            return;
        }
        FireResult::FiredAndJammed => {
            sfx.play(Sound::Jam);
            msgs.show("The pipe rifle jams in the cold! Press R to clear it.", 2.5);
        }
        FireResult::Fired => {}
    }
    game.recoil = 1.0;
    sfx.play(Sound::Gunshot);

    let origin = cam.translation();
    let dir = cam.forward().as_vec3();
    let range = game.weapon.range;

    // Find the nearest wolf on the ray.
    let mut best: Option<(Entity, f32)> = None;
    for (entity, tf, wolf) in &wolves {
        let c = wolf.hit_center(tf);
        if let Some(t) = ray_sphere(origin.to_array(), dir.to_array(), c.to_array(), wolf.hit_radius()) {
            if t <= range && best.is_none_or(|(_, bt)| t < bt) {
                best = Some((entity, t));
            }
        }
    }

    let hit_dist = best.map(|(_, t)| t).unwrap_or(range);
    let muzzle = gun
        .single()
        .map(|g| g.transform_point(MUZZLE))
        .unwrap_or(origin + cam.right().as_vec3() * 0.25 - cam.up().as_vec3() * 0.18 + dir * 0.8);
    let end = origin + dir * hit_dist;
    fxq.spawn(Fx::Muzzle(muzzle, dir, cam.right().as_vec3()));
    if best.is_none() {
        if let Some(p) = ground_hit(origin, dir, range) {
            fxq.spawn(Fx::Ricochet(p));
        }
    }

    // Tracer and muzzle flash.
    let len = muzzle.distance(end).max(0.1);
    commands.spawn((
        Mesh3d(fx.tracer.clone()),
        MeshMaterial3d(fx.tracer_mat.clone()),
        Transform::from_translation((muzzle + end) * 0.5)
            .looking_at(end, Vec3::Y)
            .with_scale(Vec3::new(1.0, 1.0, len)),
        NotShadowCaster,
        Lifetime(0.05),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.8, 0.4),
            intensity: 150_000.0,
            range: 10.0,
            ..default()
        },
        Transform::from_translation(muzzle),
        Lifetime(0.06),
    ));

    if let Some((entity, t)) = best {
        if let Ok((_, wtf, mut wolf)) = wolves.get_mut(entity) {
            let damage = game.weapon.damage;
            wolf.health -= damage;
            sfx.play(Sound::Yelp);
            fxq.spawn(Fx::WolfHit(origin + dir * t, dir));
            if wolf.health <= 0.0 {
                fxq.spawn(Fx::WolfDeath(wolf.hit_center(wtf), wolf.size));
                let alpha = wolf.alpha;
                commands.entity(entity).despawn();
                game.inv.pelts += 1;
                game.kills += 1;
                let name = if alpha { "Frostfang alpha" } else { "Frostfang" };
                let pelts = game.inv.pelts;
                msgs.show(format!("{name} killed. +1 Frostfang pelt ({pelts}/3 for a coat)"), 2.5);
            }
        }
    }
}

fn animate_gun(
    time: Res<Time>,
    mut game: ResMut<Game>,
    player: Query<&Player>,
    mut gun: Query<(&mut Transform, &GunModel), (Without<Player>, Without<Wolf>, Without<GunBolt>)>,
    mut bolt: Query<(&mut Transform, &GunBolt), (Without<Player>, Without<Wolf>, Without<GunModel>)>,
) {
    let dt = time.delta_secs();
    game.recoil = (game.recoil - dt * 8.0).max(0.0);
    let t = time.elapsed_secs();
    let sprinting = player.single().map(|p| p.sprinting).unwrap_or(false);
    let reloading = game.weapon.is_reloading();

    for (mut tf, model) in &mut gun {
        let bob = if sprinting {
            (t * 12.0).sin() * 0.02
        } else {
            (t * 2.0).sin() * 0.004
        };
        let mut pos = model.rest + Vec3::new(0.0, bob, game.recoil * 0.08);
        let mut rot = Quat::from_rotation_x(game.recoil * 0.12);
        if reloading {
            pos.y -= 0.12;
            rot *= Quat::from_rotation_z(0.6);
        }
        tf.translation = pos;
        tf.rotation = rot;
    }
    // The bolt kicks back with each shot and cycles while reloading.
    let cycle = if reloading { (t * 6.0).sin().max(0.0) } else { 0.0 };
    for (mut tf, b) in &mut bolt {
        tf.translation = b.rest + Vec3::Z * (game.recoil.max(cycle) * 0.06);
    }
}
