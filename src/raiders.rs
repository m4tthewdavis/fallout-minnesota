//! Frozen Raiders: the people who've held the old camps through the Long
//! Winter. Frost-rimed parkas, scarves, a rifle, revolver or scrap shotgun
//! each. They shoot what they can see (trees and walls hide you, a blizzard or
//! the dark shortens their sight), their guns jam in the cold, they fall back
//! to the fire when badly hurt, and a shot from you brings the camp to look.
//! They take hits, flinch, die and leave tracks like every other enemy; the
//! rules are in `sim::raider`.

use std::f32::consts::{PI, TAU};

use bevy::ecs::event::EventCursor;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::enemy::{Body, Dying, Frozen, Species};
use crate::meshes::to_mesh;
use crate::player::Player;
use crate::sim::collision::{self, segment_blocked};
use crate::sim::meshgen;
use crate::sim::raider::{Gun, Mode, Raider, Senses};
use crate::sim::survival::DeathCause;
use crate::sim::synth::Sound;
use crate::sim::terrain;
use crate::sim::weather::Phase;
use crate::state::{alive, outdoors, ClockRes, Colliders, Fx, FxQueue, Game, Gunshot, Hostile, Lifetime, Messages, RngRes, SfxQueue, SfxReq, WeatherRes};

/// Where the camps' fires are, and who's there. A camp's fire pit is 3.6 m
/// east and 0.8 m south of its tent, turned by the camp's yaw (see `props.rs`).
const CAMPS: [(f32, f32, f32); 2] = [(-110.0, 15.0, 0.4), (160.0, 55.0, -0.9)];
/// Hit sphere: torso height and size.
const BODY_CENTER: f32 = 1.1;
const BODY_RADIUS: f32 = 0.5;
const HEALTH: f32 = 75.0;
/// Radians of leg swing per metre walked.
const STRIDE_PER_METRE: f32 = 2.2;
/// How far a raider's shout carries: friends this close hear that it saw you.
const SHOUT_RANGE: f32 = 30.0;

#[derive(Component)]
pub struct RaiderAi {
    brain: Raider,
    stride: f32,
    gait: f32,
    /// 0 = gun low, 1 = shouldered.
    aim: f32,
    yaw: f32,
}

#[derive(Component)]
struct Rig {
    owner: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LimbKind {
    LegL,
    LegR,
    ArmL,
    ArmR,
}

#[derive(Component)]
struct Limb {
    owner: Entity,
    kind: LimbKind,
}

/// The gun in the raider's hands: carried low, raised to aim.
#[derive(Component)]
struct HeldGun {
    owner: Entity,
}

#[derive(Resource)]
pub(crate) struct RaiderAssets {
    leg: Handle<Mesh>,
    /// A 1 m cube, scaled for the gun parts.
    unit: Handle<Mesh>,
    torso: Handle<Mesh>,
    arm: Handle<Mesh>,
    head: Handle<Mesh>,
    hood: Handle<Mesh>,
    scarf: Handle<Mesh>,
    goggles: Handle<Mesh>,
    frost: Handle<Mesh>,
    parka: Handle<StandardMaterial>,
    pants: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    scarves: [Handle<StandardMaterial>; 3],
    lens: Handle<StandardMaterial>,
    rime: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    tracer: Handle<Mesh>,
    tracer_mat: Handle<StandardMaterial>,
}

pub struct RaiderPlugin;

impl Plugin for RaiderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_assets, spawn_initial).chain().after(crate::state::WorldGen))
            .add_systems(Update, (raider_ai.run_if(alive.and(outdoors)), animate_raiders, raider_loot).chain());
    }
}

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, assets: Res<GameAssets>) {
    let mat = |m: &mut Assets<StandardMaterial>, c: Color, rough: f32| m.add(StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() });
    commands.insert_resource(RaiderAssets {
        leg: meshes.add(Cuboid::new(0.15, 0.82, 0.19)),
        unit: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        torso: meshes.add(Capsule3d::new(0.23, 0.42)),
        arm: meshes.add(Capsule3d::new(0.075, 0.42)),
        head: meshes.add(Sphere::new(0.12)),
        hood: meshes.add(to_mesh(&meshgen::blob(0.17, 0.9, 0.06, 7, 1.0))),
        scarf: meshes.add(Cylinder::new(0.15, 0.1)),
        goggles: meshes.add(Cuboid::new(0.2, 0.06, 0.04)),
        frost: meshes.add(Sphere::new(0.06)),
        // Parkas the colour of old snow, stiff with ice.
        parka: mat(&mut materials, Color::srgb(0.6, 0.64, 0.68), 0.85),
        pants: mat(&mut materials, Color::srgb(0.12, 0.12, 0.13), 0.9),
        skin: mat(&mut materials, Color::srgb(0.7, 0.55, 0.47), 0.8),
        scarves: [
            mat(&mut materials, Color::srgb(0.55, 0.12, 0.1), 0.9),
            mat(&mut materials, Color::srgb(0.12, 0.25, 0.45), 0.9),
            mat(&mut materials, Color::srgb(0.7, 0.55, 0.12), 0.9),
        ],
        lens: materials.add(StandardMaterial { base_color: Color::srgb(0.9, 0.6, 0.15), emissive: LinearRgba::rgb(1.2, 0.5, 0.05), ..default() }),
        rime: materials.add(StandardMaterial { base_color: Color::srgb(0.9, 0.96, 1.0), emissive: LinearRgba::rgb(0.15, 0.2, 0.25), perceptual_roughness: 0.4, ..default() }),
        metal: assets.rust.clone(),
        wood: assets.pole_wood.clone(),
        tracer: meshes.add(Cuboid::new(0.025, 0.025, 1.0)),
        tracer_mat: materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.7, 0.3), emissive: LinearRgba::rgb(6.0, 3.0, 0.6), unlit: true, ..default() }),
    });
}

/// Fire pit position for a camp.
fn fire_pit(camp: (f32, f32, f32)) -> Vec2 {
    let (cx, cz, yaw) = camp;
    let (dx, dz) = (3.6f32, 0.8f32);
    Vec2::new(cx + yaw.cos() * dx + yaw.sin() * dz, cz - yaw.sin() * dx + yaw.cos() * dz)
}

pub(crate) fn spawn_initial(mut commands: Commands, assets: Res<RaiderAssets>, mut rng: ResMut<RngRes>) {
    let convoy = Vec2::new(crate::quest::CONVOY_AT.0 + 1.0, crate::quest::CONVOY_AT.1 - 3.4);
    // (where the fire is, who stands round it)
    for (fire, guns) in [
        (fire_pit(CAMPS[0]), &[Gun::Rifle, Gun::Revolver, Gun::Shotgun][..]),
        (fire_pit(CAMPS[1]), &[Gun::Revolver, Gun::Rifle][..]),
        (convoy, &[Gun::Shotgun, Gun::Revolver][..]),
    ] {
        for (i, gun) in guns.iter().enumerate() {
            let a = i as f32 / guns.len() as f32 * TAU + rng.0.range(0.0, 0.8);
            let at = fire + Vec2::new(a.cos(), a.sin()) * rng.0.range(1.8, 3.0);
            spawn_raider(&mut commands, &assets, at, *gun, [fire.x, fire.y], i, 0.0, &mut rng);
        }
    }
}

fn spawn_raider(commands: &mut Commands, a: &RaiderAssets, at: Vec2, gun: Gun, home: [f32; 2], look: usize, aim: f32, rng: &mut RngRes) -> Entity {
    let yaw = rng.0.range(0.0, TAU);
    let id = commands
        .spawn((
            Transform::from_xyz(at.x, terrain::walk_height(at.x, at.y), at.y).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Hostile,
            Body::new(Species::Raider, HEALTH, BODY_CENTER, BODY_RADIUS),
            RaiderAi { brain: Raider::new(gun, home), stride: rng.0.range(0.0, TAU), gait: 0.0, aim, yaw },
        ))
        .id();
    commands.entity(id).with_children(|r| {
        r.spawn((Transform::default(), Visibility::default(), Rig { owner: id })).with_children(|rig| {
            // Legs and arms pivot at hip and shoulder.
            for (x, kind) in [(-0.1f32, LimbKind::LegL), (0.1, LimbKind::LegR)] {
                rig.spawn((Transform::from_xyz(x, 0.82, 0.0), Visibility::default(), Limb { owner: id, kind })).with_children(|l| {
                    l.spawn((Mesh3d(a.leg.clone()), MeshMaterial3d(a.pants.clone()), Transform::from_xyz(0.0, -0.41, 0.0)));
                });
            }
            for (x, kind) in [(-0.3f32, LimbKind::ArmL), (0.3, LimbKind::ArmR)] {
                rig.spawn((Transform::from_xyz(x, 1.42, 0.0), Visibility::default(), Limb { owner: id, kind })).with_children(|l| {
                    l.spawn((Mesh3d(a.arm.clone()), MeshMaterial3d(a.parka.clone()), Transform::from_xyz(0.0, -0.3, 0.0)));
                });
            }
            rig.spawn((Mesh3d(a.torso.clone()), MeshMaterial3d(a.parka.clone()), Transform::from_xyz(0.0, 1.12, 0.0).with_scale(Vec3::new(1.0, 1.0, 0.8))));
            // Head: face, goggles, hood, scarf, and rime on the shoulders.
            rig.spawn((Mesh3d(a.head.clone()), MeshMaterial3d(a.skin.clone()), Transform::from_xyz(0.0, 1.68, 0.02)));
            rig.spawn((Mesh3d(a.goggles.clone()), MeshMaterial3d(a.lens.clone()), Transform::from_xyz(0.0, 1.71, 0.12), NotShadowCaster));
            rig.spawn((Mesh3d(a.hood.clone()), MeshMaterial3d(a.parka.clone()), Transform::from_xyz(0.0, 1.7, -0.05)));
            rig.spawn((Mesh3d(a.scarf.clone()), MeshMaterial3d(a.scarves[look % 3].clone()), Transform::from_xyz(0.0, 1.5, 0.02)));
            for (x, y, z, s) in [(-0.25f32, 1.5f32, 0.0f32, 1.0f32), (0.25, 1.5, 0.0, 1.0), (0.0, 1.82, -0.05, 0.9), (0.0, 1.35, -0.2, 1.3)] {
                rig.spawn((Mesh3d(a.frost.clone()), MeshMaterial3d(a.rime.clone()), Transform::from_xyz(x, y, z).with_scale(Vec3::new(s, 0.5 * s, s)), NotShadowCaster));
            }
            // The gun.
            rig.spawn((Transform::default(), Visibility::default(), HeldGun { owner: id })).with_children(|g| {
                // (position, size, wooden?) of each part, muzzle towards +z.
                let parts: &[([f32; 3], [f32; 3], bool)] = match gun {
                    Gun::Rifle => &[([0.0, 0.0, 0.45], [0.045, 0.055, 0.9], false), ([0.0, -0.03, -0.14], [0.06, 0.11, 0.32], true)],
                    Gun::Shotgun => &[([-0.025, 0.0, 0.32], [0.04, 0.045, 0.65], false), ([0.025, 0.0, 0.32], [0.04, 0.045, 0.65], false), ([0.0, -0.03, -0.14], [0.06, 0.11, 0.3], true)],
                    Gun::Revolver => &[([0.0, 0.0, 0.1], [0.04, 0.05, 0.2], false), ([0.0, -0.07, -0.03], [0.04, 0.12, 0.05], true)],
                };
                for (at, size, wooden) in parts {
                    let material = if *wooden { a.wood.clone() } else { a.metal.clone() };
                    g.spawn((Mesh3d(a.unit.clone()), MeshMaterial3d(material), Transform::from_translation(Vec3::from_array(*at)).with_scale(Vec3::from_array(*size))));
                }
            });
        });
    });
    id
}

fn gun_sound(g: Gun) -> Sound {
    match g {
        Gun::Rifle => Sound::RifleShot,
        Gun::Shotgun => Sound::ShotgunShot,
        Gun::Revolver => Sound::RevolverShot,
    }
}

/// Screenshot helper: three raiders in a row, standing still, guns raised,
/// facing the player's spawn.
pub fn spawn_lineup(mut commands: Commands, assets: Res<RaiderAssets>, mut rng: ResMut<RngRes>) {
    let (sx, sz) = terrain::PLAYER_SPAWN;
    let z = sz - 7.0;
    for (i, gun) in [Gun::Rifle, Gun::Shotgun, Gun::Revolver].into_iter().enumerate() {
        let x = sx - 2.4 + i as f32 * 2.4;
        let id = spawn_raider(&mut commands, &assets, Vec2::new(x, z), gun, [x, z], i, 1.0, &mut rng);
        commands.entity(id).insert((Frozen, Transform::from_xyz(x, terrain::walk_height(x, z), z).with_rotation(Quat::from_rotation_y(PI + 0.1 - 0.1 * i as f32))));
    }
}

/// Think, move, shoot.
#[allow(clippy::too_many_arguments)]
fn raider_ai(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    colliders: Res<Colliders>,
    assets: Res<RaiderAssets>,
    // Both reads and writes gunshots, so it holds the event queue itself.
    mut gunshots: ResMut<Events<Gunshot>>,
    mut cursor: Local<EventCursor<Gunshot>>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    mut fxq: ResMut<FxQueue>,
    mut warned: Local<bool>,
    player: Query<(&Transform, &Player)>,
    mut raiders: Query<(&mut Transform, &mut RaiderAi, &Body), (Without<Player>, Without<Dying>, Without<Frozen>)>,
) {
    let dt = time.delta_secs();
    let Ok((ptf, p)) = player.single() else { return };
    let shots: Vec<Vec3> = cursor.read(&gunshots).filter(|s| s.by_player).map(|s| s.pos).collect();
    let player_xz = Vec2::new(ptf.translation.x, ptf.translation.z);
    let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();
    let blizzard = weather.weather.phase == Phase::Blizzard;
    let night = clock.0.is_night();
    let mut seers: Vec<Vec2> = Vec::new();
    let mut any_fighting = false;

    for (mut tf, mut r, body) in &mut raiders {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        let los = !segment_blocked((pos.x, pos.y), (player_xz.x, player_xz.y), &colliders.0);
        let nearest_shot = shots.iter().map(|s| Vec2::new(s.x, s.z)).min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)));
        let senses = Senses {
            pos: pos.to_array(),
            player: player_xz.to_array(),
            los,
            night,
            blizzard,
            temp_f: temp,
            health_frac: body.health_fraction(),
            heard: nearest_shot.map(|s| s.to_array()),
            player_moving: p.moving,
            player_sprinting: p.sprinting,
        };
        let act = r.brain.think(dt, &senses, &mut rng.0);
        if r.brain.mode == Mode::Fight {
            seers.push(pos);
            any_fighting = true;
        }

        // Walk, around trees and walls.
        let step = Vec2::from_array(act.dir) * act.speed * dt;
        let (nx, nz) = collision::push_out(pos.x + step.x, pos.y + step.y, 0.45, &colliders.0);
        tf.translation = Vec3::new(nx, terrain::walk_height(nx, nz), nz);
        let moved = Vec2::new(nx, nz).distance(pos);
        r.gait = if dt > 0.0 { moved / dt } else { 0.0 };
        r.stride += moved * STRIDE_PER_METRE;
        // Turn towards where it wants to look.
        if act.face[0].abs() + act.face[1].abs() > 1e-4 {
            let want = act.face[0].atan2(act.face[1]);
            let mut diff = (want - r.yaw + PI).rem_euclid(TAU) - PI;
            diff = diff.clamp(-6.0 * dt, 6.0 * dt);
            r.yaw += diff;
        }
        tf.rotation = Quat::from_rotation_y(r.yaw);
        let aim_target = if act.aiming { 1.0 } else { 0.0 };
        r.aim += (aim_target - r.aim) * (dt * 7.0).min(1.0);

        if act.reloading {
            sfx.0.push(SfxReq { sound: Sound::ClunkOut, pos: Some(tf.translation + Vec3::Y), gain: gain_for(pos.distance(player_xz)) });
        }
        let Some(v) = act.volley else { continue };

        // The shot: flash, tracer, noise, and what it did to you.
        let muzzle = tf.translation + tf.rotation * Vec3::new(0.12, 1.4, 0.9);
        let target = ptf.translation - Vec3::Y * 0.4;
        let to_target = (target - muzzle).normalize_or_zero();
        let right = tf.rotation * Vec3::X;
        fxq.spawn(Fx::Muzzle(muzzle, to_target, right, false));
        commands.spawn((
            PointLight { color: Color::srgb(1.0, 0.8, 0.4), intensity: 150_000.0, range: 12.0, ..default() },
            Transform::from_translation(muzzle),
            Lifetime(0.07),
        ));
        let aim_point = if v.hits > 0 {
            target
        } else {
            // A miss lands near you, in the snow.
            let miss = Vec3::new(rng.0.range(-2.0, 2.0), 0.0, rng.0.range(-2.0, 2.0));
            let gx = ptf.translation.x + miss.x;
            let gz = ptf.translation.z + miss.z;
            let ground = Vec3::new(gx, terrain::walk_height(gx, gz), gz);
            fxq.spawn(Fx::Ricochet(ground));
            ground
        };
        let len = muzzle.distance(aim_point).max(0.5);
        commands.spawn((
            Mesh3d(assets.tracer.clone()),
            MeshMaterial3d(assets.tracer_mat.clone()),
            Transform::from_translation((muzzle + aim_point) * 0.5).looking_at(aim_point, Vec3::Y).with_scale(Vec3::new(1.0, 1.0, len)),
            NotShadowCaster,
            Lifetime(0.05),
        ));
        sfx.0.push(SfxReq { sound: gun_sound(v.gun), pos: Some(muzzle), gain: gain_for(muzzle.distance(ptf.translation)) });
        if v.jammed {
            sfx.0.push(SfxReq { sound: Sound::Jam, pos: Some(muzzle), gain: gain_for(muzzle.distance(ptf.translation)) });
        }
        gunshots.send(Gunshot { pos: muzzle, by_player: false });
        if v.hits > 0 && game.death.is_none() {
            game.hurt_flash = 1.0;
            msgs.show(format!("A Frozen Raider's {} hits you for {:.0}!", v.gun.name(), v.damage), 1.8);
            if let Some(cause) = game.survival.damage_by(v.damage, DeathCause::Shot) {
                game.death = Some(cause);
                msgs.show(cause.describe(), f32::MAX);
            }
        }
    }

    // Whoever saw you shouts, and friends nearby come looking.
    for (tf, mut r, _) in &mut raiders {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        if seers.iter().any(|s| s.distance(pos) < SHOUT_RANGE && s.distance(pos) > 0.01) {
            r.brain.warn(player_xz.to_array());
        }
    }
    if any_fighting && !*warned {
        *warned = true;
        msgs.show("Frozen Raiders! Get behind something solid.", 3.0);
    } else if !any_fighting {
        *warned = false;
    }
}

/// Gunfire is quieter the further away it is.
fn gain_for(dist: f32) -> f32 {
    (1.0 / (1.0 + dist / 35.0)).clamp(0.2, 1.0)
}

/// Animate gait, aim and flinch.
fn animate_raiders(
    time: Res<Time>,
    raiders: Query<(&RaiderAi, &Body), Without<Dying>>,
    mut parts: ParamSet<(
        Query<(&Rig, &mut Transform)>,
        Query<(&Limb, &mut Transform)>,
        Query<(&HeldGun, &mut Transform)>,
    )>,
) {
    for (rig, mut tf) in &mut parts.p0() {
        if let Ok((_, body)) = raiders.get(rig.owner) {
            // Rock back from a hit.
            tf.rotation = Quat::from_rotation_x(-body.flinch * 0.35);
        } else {
            tf.rotation = Quat::IDENTITY;
        }
    }
    for (limb, mut tf) in &mut parts.p1() {
        let Ok((r, _)) = raiders.get(limb.owner) else { continue };
        let walk = (r.gait / 2.6).clamp(0.0, 1.0);
        let swing = r.stride.sin() * 0.75 * walk;
        let busy = if r.brain.is_busy() { 0.12 * (time.elapsed_secs() * 14.0).sin() } else { 0.0 };
        tf.rotation = match limb.kind {
            LimbKind::LegL => Quat::from_rotation_x(swing),
            LimbKind::LegR => Quat::from_rotation_x(-swing),
            // Arms swing with the stride, then come up to hold the gun.
            LimbKind::ArmL => Quat::from_rotation_x((-swing * 0.6) * (1.0 - r.aim) - 1.25 * r.aim + busy) * Quat::from_rotation_y(0.35 * r.aim),
            LimbKind::ArmR => Quat::from_rotation_x((swing * 0.6) * (1.0 - r.aim) - 1.4 * r.aim) * Quat::from_rotation_y(-0.1 * r.aim),
        };
    }
    for (gun, mut tf) in &mut parts.p2() {
        let Ok((r, _)) = raiders.get(gun.owner) else { continue };
        let low = (Vec3::new(0.2, 0.95, 0.25), Quat::from_rotation_x(0.9));
        let up = (Vec3::new(0.13, 1.4, 0.35), Quat::IDENTITY);
        tf.translation = low.0.lerp(up.0, r.aim);
        tf.rotation = low.1.slerp(up.1, r.aim);
    }
}

/// A dead raider's pockets: rounds for its gun and some scrap.
fn raider_loot(mut game: ResMut<Game>, mut msgs: ResMut<Messages>, mut sfx: ResMut<SfxQueue>, mut rng: ResMut<RngRes>, fallen: Query<&RaiderAi, Added<Dying>>) {
    for r in &fallen {
        let rounds = r.brain.gun.loot_rounds() + (rng.0.f32() * 4.0) as u32;
        let scrap = 2 + (rng.0.f32() * 3.0) as u32;
        let inv = &mut game.inv;
        let what = match r.brain.gun {
            Gun::Rifle => {
                inv.ammo_reserve += rounds;
                "pipe rounds"
            }
            Gun::Shotgun => {
                inv.shells += rounds;
                "shells"
            }
            Gun::Revolver => {
                inv.revolver_rounds += rounds;
                "revolver rounds"
            }
        };
        inv.scrap += scrap;
        msgs.show(format!("Frozen Raider down: +{rounds} {what}, +{scrap} scrap"), 3.5);
        sfx.play(Sound::PickupAmmo);
    }
}
