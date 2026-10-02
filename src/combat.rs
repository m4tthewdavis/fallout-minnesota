//! Pipe rifle shooting: hitscan against Frostfangs, tracers, muzzle flash,
//! recoil, reloads and cold-weather jams.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::player::{cursor_locked, GunModel, Player};
use crate::sim::combat::{ray_sphere, FireResult};
use crate::sim::synth::Sound;
use crate::state::{alive, ClockRes, Game, Lifetime, Messages, RngRes, SfxQueue, WeatherRes};
use crate::wolves::Wolf;

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
    clock: Res<ClockRes>,
    cam: Query<&GlobalTransform, With<Player>>,
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
    let muzzle = origin + cam.right().as_vec3() * 0.25 - cam.up().as_vec3() * 0.18 + dir * 0.8;
    let end = origin + dir * hit_dist;

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
        Lifetime(0.05),
    ));

    if let Some((entity, _)) = best {
        if let Ok((_, _, mut wolf)) = wolves.get_mut(entity) {
            let damage = game.weapon.damage;
            wolf.health -= damage;
            sfx.play(Sound::Yelp);
            if wolf.health <= 0.0 {
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
    mut gun: Query<(&mut Transform, &GunModel), (Without<Player>, Without<Wolf>)>,
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
}
