//! Shooting and fighting: hitscan firearms (the pipe rifle's single round, the
//! shotgun's spread of pellets, the revolver's heavy shot), melee swings of
//! the ice axe, tracers, muzzle flashes, recoil, reloads and cold-weather
//! jams. Works on anything with a [`Body`], and switches weapons on the
//! number keys and the scroll wheel.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::enemy::{Body, Dying, Species};
use crate::gun::{breech_local, muzzle_local, GunModel, VIEW_LAYER};
use crate::particles::ground_hit;
use crate::player::{cursor_locked, Player};
use crate::sim::combat::{ray_sphere, FireResult, WeaponKind};
use crate::sim::rng::Rng;
use crate::sim::sfx;
use crate::sim::synth::Sound;
use crate::state::{alive, ClockRes, Fx, FxQueue, Game, Hostile, Lifetime, Messages, RngRes, SfxQueue, WeatherRes};

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
            (weapon_timers, switch_weapons, reload, reload_cues, fire).chain().run_if(alive),
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
    let dt = time.delta_secs();
    let was_jammed = game.weapon().jammed;
    game.arsenal.tick(dt);
    if game.weapon_mut().tick(dt) && was_jammed {
        msgs.show("Jam cleared.", 1.5);
    }
    game.hurt_flash = (game.hurt_flash - dt * 1.5).max(0.0);
}

/// Number keys pick a weapon; the scroll wheel cycles through the ones you own.
fn switch_weapons(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut game: ResMut<Game>,
    mut sfx: ResMut<SfxQueue>,
    mut msgs: ResMut<Messages>,
) {
    let mut switched = false;
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].into_iter().enumerate() {
        if keys.just_pressed(key) {
            if !game.arsenal.owned[i] {
                msgs.show(format!("You haven't found the {} yet.", WeaponKind::ALL[i].name()), 2.0);
            } else {
                switched |= game.arsenal.select(i);
            }
        }
    }
    if cursor_locked(&windows) && scroll.delta.y != 0.0 {
        switched |= game.arsenal.cycle(if scroll.delta.y > 0.0 { -1 } else { 1 });
    }
    if switched {
        sfx.play_gain(Sound::ClunkIn, 0.5);
        let name = game.weapon().kind.name();
        msgs.show(name, 1.2);
    }
}

fn reload(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>, mut msgs: ResMut<Messages>) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Game { arsenal, inv, .. } = &mut *game;
    let weapon = arsenal.current_mut();
    let Some(ammo) = weapon.kind.ammo() else { return };
    let jammed = weapon.jammed;
    if weapon.start_reload(inv.reserve_mut(ammo)) {
        if jammed {
            msgs.show("Working the frozen action loose...", 1.2);
        }
    } else if weapon.mag == 0 && inv.reserve(ammo) == 0 {
        msgs.show(format!("Out of ammo. Search the snow for {}.", ammo.name()), 2.0);
    }
}

/// Plays the clunks and clacks of a reload (or unjam) as the animation
/// reaches them.
fn reload_cues(
    game: Res<Game>,
    mut sfx: ResMut<SfxQueue>,
    mut fxq: ResMut<FxQueue>,
    gun: Query<&GlobalTransform, With<GunModel>>,
    mut last: Local<Option<(f32, bool, WeaponKind)>>,
) {
    let w = game.weapon();
    // Revolvers and shotguns dump their spent brass when the first clunk sounds.
    let dump = |sounds: &[Sound], kind: WeaponKind, unjam: bool, fxq: &mut FxQueue| {
        if unjam || !matches!(kind, WeaponKind::ScrapShotgun | WeaponKind::Revolver) {
            return;
        }
        if sounds.contains(&Sound::ClunkOut) {
            if let Ok(g) = gun.single() {
                let spent = (w.mag_size - w.mag).min(6);
                fxq.spawn(Fx::Brass(g.transform_point(breech_local(kind)), spent));
            }
        }
    };
    match (w.reload_progress(), *last) {
        // Switching weapons mid-reload isn't allowed, so the kind never changes here.
        (Some(p), prev) => {
            let (from, unjam, kind) = prev.unwrap_or((-0.001, w.jammed, w.kind));
            let sounds = sfx::cues_between(&sfx::cues(kind, unjam), from, p);
            dump(&sounds, kind, unjam, &mut fxq);
            for sound in sounds {
                sfx.play(sound);
            }
            *last = Some((p, unjam, kind));
        }
        // Finished since last frame: play whatever cues remained.
        (None, Some((from, unjam, kind))) => {
            let sounds = sfx::cues_between(&sfx::cues(kind, unjam), from, 1.0);
            dump(&sounds, kind, unjam, &mut fxq);
            for sound in sounds {
                sfx.play(sound);
            }
            *last = None;
        }
        (None, None) => {}
    }
}

/// A point inside the cone of half-angle `spread` around `dir`.
fn jitter_dir(dir: Vec3, right: Vec3, up: Vec3, spread: f32, rng: &mut Rng) -> Vec3 {
    if spread <= 0.0 {
        return dir;
    }
    let r = spread * rng.f32().sqrt();
    let a = rng.range(0.0, std::f32::consts::TAU);
    (dir + right * (a.cos() * r) + up * (a.sin() * r)).normalize()
}

/// Everything one volley did to one creature.
struct Strike {
    entity: Entity,
    damage: f32,
    nearest: f32,
    point: Vec3,
    dir: Vec3,
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
    mut bodies: Query<(Entity, &Transform, &mut Body), (Without<Player>, Without<Dying>)>,
) {
    if !mouse.pressed(MouseButton::Left) || !cursor_locked(&windows) {
        return;
    }
    let kind = game.weapon().kind;
    // Revolvers and shotguns fire once per click.
    if matches!(kind, WeaponKind::Revolver | WeaponKind::ScrapShotgun) && !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(cam) = cam.single() else { return };
    let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();

    match game.weapon_mut().try_fire(temp, &mut rng.0) {
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
            msgs.show(format!("The {} jams in the cold! Press R to clear it.", kind.name().to_lowercase()), 2.5);
        }
        FireResult::Fired => {}
    }
    game.recoil = 1.0;
    sfx.play(sfx::fire_sound(kind));

    let origin = cam.translation();
    let dir = cam.forward().as_vec3();
    let (right, up) = (cam.right().as_vec3(), cam.up().as_vec3());
    let weapon = game.weapon().clone();

    // ---- Melee: hit the nearest creature in front of you ----
    if weapon.melee {
        let mut best: Option<(Entity, f32, Vec3)> = None;
        for (entity, tf, body) in &bodies {
            let centre = body.center(tf);
            let to = centre - origin;
            let reach = to.length() - body.hit_radius(tf);
            if reach <= weapon.range && to.normalize_or_zero().dot(dir) > 0.5 && best.is_none_or(|(_, d, _)| reach < d) {
                best = Some((entity, reach, centre - dir * body.hit_radius(tf)));
            }
        }
        if let Some((entity, reach, point)) = best {
            let damage = weapon.damage_at(reach.max(0.0));
            sfx.play_at(Sound::MeleeHit, point);
            fxq.spawn(Fx::WolfHit(point, dir));
            strike(
                &mut commands,
                &mut game,
                &mut msgs,
                &mut fxq,
                &mut bodies,
                &[Strike {
                    entity,
                    damage,
                    nearest: reach,
                    point,
                    dir,
                }],
            );
        }
        return;
    }

    // ---- Firearms: one ray per pellet ----
    let muzzle = gun
        .single()
        .map(|g| g.transform_point(muzzle_local(kind)))
        .unwrap_or(origin + right * 0.25 - up * 0.18 + dir * 0.8);
    fxq.spawn(Fx::Muzzle(muzzle, dir, right, kind == WeaponKind::PipeRifle));

    let mut strikes: Vec<Strike> = Vec::new();
    let mut ricochets = 0;
    for _ in 0..weapon.pellets {
        let d = jitter_dir(dir, right, up, weapon.spread, &mut rng.0);
        let mut best: Option<(Entity, f32)> = None;
        for (entity, tf, body) in &bodies {
            let c = body.center(tf);
            if let Some(t) = ray_sphere(origin.to_array(), d.to_array(), c.to_array(), body.hit_radius(tf)) {
                if t <= weapon.range && best.is_none_or(|(_, bt)| t < bt) {
                    best = Some((entity, t));
                }
            }
        }
        let t = best.map(|(_, t)| t).unwrap_or(weapon.range);
        let end = origin + d * t;
        match best {
            Some((entity, t)) => {
                let damage = weapon.damage_at(t);
                match strikes.iter_mut().find(|s| s.entity == entity) {
                    Some(s) => {
                        s.damage += damage;
                        if t < s.nearest {
                            s.nearest = t;
                            s.point = end;
                        }
                    }
                    None => strikes.push(Strike {
                        entity,
                        damage,
                        nearest: t,
                        point: end,
                        dir: d,
                    }),
                }
            }
            None if ricochets < 3 => {
                if let Some(p) = ground_hit(origin, d, weapon.range) {
                    fxq.spawn(Fx::Ricochet(p));
                    ricochets += 1;
                }
            }
            None => {}
        }
        // A tracer for each pellet (shotgun tracers are short-lived and fainter).
        let len = muzzle.distance(end).max(0.1);
        let thin = if weapon.pellets > 1 { 0.5 } else { 1.0 };
        commands.spawn((
            Mesh3d(fx.tracer.clone()),
            MeshMaterial3d(fx.tracer_mat.clone()),
            Transform::from_translation((muzzle + end) * 0.5)
                .looking_at(end, Vec3::Y)
                .with_scale(Vec3::new(thin, thin, len)),
            NotShadowCaster,
            Lifetime(if weapon.pellets > 1 { 0.035 } else { 0.05 }),
        ));
    }
    let flash = if kind == WeaponKind::ScrapShotgun { 300_000.0 } else { 150_000.0 };
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.8, 0.4),
            intensity: flash,
            range: 10.0,
            ..default()
        },
        // Lights the world and the weapon itself.
        bevy::render::view::RenderLayers::from_layers(&[0, VIEW_LAYER]),
        Transform::from_translation(muzzle),
        Lifetime(0.06),
    ));

    for s in &strikes {
        sfx.play_at(Sound::Yelp, s.point);
        fxq.spawn(Fx::WolfHit(s.point, s.dir));
    }
    strike(&mut commands, &mut game, &mut msgs, &mut fxq, &mut bodies, &strikes);
}

/// Apply a volley's damage, and handle anything it killed.
fn strike(
    commands: &mut Commands,
    game: &mut Game,
    msgs: &mut Messages,
    fxq: &mut FxQueue,
    bodies: &mut Query<(Entity, &Transform, &mut Body), (Without<Player>, Without<Dying>)>,
    strikes: &[Strike],
) {
    for s in strikes {
        let Ok((_, tf, mut body)) = bodies.get_mut(s.entity) else { continue };
        if !body.hurt(s.damage, s.dir) {
            continue;
        }
        fxq.spawn(Fx::WolfDeath(body.center(tf), tf.scale.x));
        game.kills += 1;
        match body.species {
            Species::Wolf { .. } => {
                game.inv.pelts += 1;
                let pelts = game.inv.pelts;
                msgs.show(
                    format!("{} killed. +1 Frostfang pelt ({pelts}/3 for a coat)", body.species.name()),
                    2.5,
                );
            }
            Species::Moose => {
                msgs.show(format!("{} down. It won't be charging anyone again.", body.species.name()), 3.0);
            }
        }
        // Fall away from the shot.
        let side = tf.rotation.mul_vec3(Vec3::X).dot(s.dir);
        commands
            .entity(s.entity)
            .remove::<Hostile>()
            .insert(Dying::new(tf, &body, side));
    }
}
