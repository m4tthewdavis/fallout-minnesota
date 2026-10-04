//! Frostfang wolf packs: translucent, glowing-eyed timber wolves that wander in
//! calm weather and hunt in packs during rad-blizzards. Every wolf looks a
//! little different (four fur patterns, torn or tall ears, three coats, odd
//! proportions). They trot, break into a bounding gallop when they chase,
//! lunge as they bite, flinch when hit, fall over when killed, exhale glowing
//! vapour, steer around trees and buildings, and howl at night and in storms.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::enemy::{Body, Dying, Frozen, Species};
use crate::meshes::to_mesh;
use crate::player::Player;
use crate::sim::meshgen;
use crate::sim::collision;
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::sim::weather::Phase;
use crate::sim::wolf::{self, WolfMode, BITE_RANGE};
use crate::state::{alive, random_point_around, ClockRes, Colliders, Fx, FxQueue, Game, Hostile, Messages, RngRes, SfxQueue, WeatherRes};

const MAX_WOLVES: usize = 16;
const BITE_COOLDOWN: f32 = 1.3;
/// Radians of leg swing per metre travelled.
const STRIDE_PER_METRE: f32 = 1.7;

#[derive(Component)]
pub struct Wolf {
    pub alpha: bool,
    /// Visual and hitbox scale.
    pub size: f32,
    bite_cd: f32,
    howl_cd: f32,
    wander_target: Vec2,
    wander_timer: f32,
    speed_jitter: f32,
    /// Current ground speed, m/s (drives the leg animation).
    gait: f32,
    /// Accumulated walk-cycle phase.
    stride: f32,
    /// 0 = trotting, 1 = full gallop (smoothed).
    gallop: f32,
    /// 1 right after a bite, falling to 0: the lunge.
    lunge: f32,
    breath_cd: f32,
}


/// A leg (or tail) pivot belonging to a wolf.
#[derive(Component)]
struct WolfLimb {
    owner: Entity,
    /// Phase when trotting (diagonal pairs) and when galloping (fore and hind pairs).
    trot_phase: f32,
    gallop_phase: f32,
    tail: bool,
}

/// The wolf's whole body, so it can bound, lunge and flinch without fighting
/// the AI for control of the root transform.
#[derive(Component)]
struct WolfRig {
    owner: Entity,
}

#[derive(Resource)]
pub(crate) struct WolfAssets {
    bodies: Vec<Handle<Mesh>>,
    heads: Vec<Handle<Mesh>>,
    leg: Handle<Mesh>,
    tail: Handle<Mesh>,
    eye: Handle<Mesh>,
    furs: Vec<Handle<StandardMaterial>>,
    alpha_fur: Handle<StandardMaterial>,
    eye_mat: Handle<StandardMaterial>,
    alpha_eye_mat: Handle<StandardMaterial>,
}

/// Countdown to the next distant howl.
#[derive(Resource)]
struct DistantHowl(f32);

pub struct WolfPlugin;

impl Plugin for WolfPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DistantHowl(20.0))
            .add_systems(Startup, (setup_wolf_assets, spawn_initial_packs).chain())
            .add_systems(
                Update,
                (
                    (blizzard_packs, wolf_ai, distant_howls).chain().run_if(alive),
                    animate_wolves,
                ),
            );
    }
}

fn setup_wolf_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // "Translucent fur that blends into snow." Three coats: arctic white,
    // ash grey and dusky brown-grey.
    let fur = |materials: &mut Assets<StandardMaterial>, c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };
    let furs = vec![
        fur(&mut materials, Color::srgba(0.9, 0.93, 0.97, 0.72)),
        fur(&mut materials, Color::srgba(0.68, 0.72, 0.78, 0.78)),
        fur(&mut materials, Color::srgba(0.56, 0.52, 0.5, 0.8)),
    ];
    let alpha_fur = fur(&mut materials, Color::srgba(0.5, 0.55, 0.64, 0.86));
    // Glowing eyes are the only tell in a whiteout; an alpha's burn amber.
    let eye = |materials: &mut Assets<StandardMaterial>, c: Color, e: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: c,
            emissive: e,
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(WolfAssets {
        // Smooth procedural bodies (see sim::meshgen), countershaded with
        // vertex colours and marked per variant.
        bodies: (0..meshgen::WOLF_VARIANTS).map(|v| meshes.add(to_mesh(&meshgen::wolf_body_variant(v)))).collect(),
        heads: (0..meshgen::WOLF_VARIANTS).map(|v| meshes.add(to_mesh(&meshgen::wolf_head_variant(v)))).collect(),
        leg: meshes.add(to_mesh(&meshgen::wolf_leg())),
        tail: meshes.add(to_mesh(&meshgen::wolf_tail())),
        eye: meshes.add(Sphere::new(0.032)),
        furs,
        alpha_fur,
        eye_mat: eye(&mut materials, Color::srgb(0.4, 0.9, 1.0), LinearRgba::rgb(4.0, 16.0, 22.0)),
        alpha_eye_mat: eye(&mut materials, Color::srgb(1.0, 0.7, 0.3), LinearRgba::rgb(24.0, 11.0, 2.0)),
    });
}

fn spawn_wolf(commands: &mut Commands, assets: &WolfAssets, pos: Vec2, alpha: bool, rng: &mut RngRes) -> Entity {
    spawn_wolf_with(commands, assets, pos, alpha, None, None, rng)
}

/// Spawn a wolf, optionally forcing its fur pattern and coat.
fn spawn_wolf_with(
    commands: &mut Commands,
    assets: &WolfAssets,
    pos: Vec2,
    alpha: bool,
    variant: Option<u32>,
    coat: Option<usize>,
    rng: &mut RngRes,
) -> Entity {
    let size = if alpha { 1.3 } else { rng.0.range(0.9, 1.1) };
    let health = if alpha { 100.0 } else { 60.0 };
    let variant = variant.unwrap_or((rng.0.f32() * meshgen::WOLF_VARIANTS as f32) as u32) % meshgen::WOLF_VARIANTS;
    let fur = if alpha {
        assets.alpha_fur.clone()
    } else {
        assets.furs[coat.unwrap_or((rng.0.f32() * assets.furs.len() as f32) as usize) % assets.furs.len()].clone()
    };
    let eye_mat = if alpha { assets.alpha_eye_mat.clone() } else { assets.eye_mat.clone() };
    // Slightly different proportions: broader or leaner, longer or shorter.
    let scale = Vec3::new(size * rng.0.range(0.94, 1.08), size, size * rng.0.range(0.95, 1.06));
    let y = terrain::walk_height(pos.x, pos.y);

    let id = commands
        .spawn((
            Transform::from_xyz(pos.x, y, pos.y).with_scale(scale),
            Visibility::default(),
            Hostile,
            Body::new(Species::Wolf { alpha }, health, 0.75, 0.8),
            Wolf {
                alpha,
                size,
                bite_cd: 0.0,
                howl_cd: rng.0.range(5.0, 20.0),
                wander_target: pos,
                wander_timer: 0.0,
                speed_jitter: rng.0.range(0.85, 1.05),
                gait: 0.0,
                stride: rng.0.range(0.0, std::f32::consts::TAU),
                gallop: 0.0,
                lunge: 0.0,
                breath_cd: rng.0.range(0.0, 2.0),
            },
        ))
        .id();

    commands.entity(id).with_children(|w| {
        w.spawn((Transform::default(), Visibility::default(), WolfRig { owner: id })).with_children(|rig| {
            rig.spawn((Mesh3d(assets.bodies[variant as usize].clone()), MeshMaterial3d(fur.clone())));
            rig.spawn((Mesh3d(assets.heads[variant as usize].clone()), MeshMaterial3d(fur.clone())));
            for x in [-0.075, 0.075] {
                rig.spawn((Mesh3d(assets.eye.clone()), MeshMaterial3d(eye_mat.clone()), Transform::from_xyz(x, 1.11, 1.02)));
            }

            // Tail pivots at the rump.
            rig.spawn((
                Transform::from_xyz(0.0, 0.95, -0.7),
                Visibility::default(),
                WolfLimb {
                    owner: id,
                    trot_phase: 0.0,
                    gallop_phase: 0.0,
                    tail: true,
                },
            ))
            .with_children(|t| {
                t.spawn((Mesh3d(assets.tail.clone()), MeshMaterial3d(fur.clone())));
            });

            // Legs pivot at the hip/shoulder. Trotting, diagonal pairs move
            // together; galloping, the two forelegs reach together and the two
            // hindlegs push together a beat later.
            let pi = std::f32::consts::PI;
            for (x, z, trot, gallop) in [(-0.2, 0.5, 0.0, 0.0), (0.2, 0.5, pi, 0.25), (-0.2, -0.5, pi, 2.5), (0.2, -0.5, 0.0, 2.75)] {
                rig.spawn((
                    Transform::from_xyz(x, 0.6, z),
                    Visibility::default(),
                    WolfLimb {
                        owner: id,
                        trot_phase: trot,
                        gallop_phase: gallop,
                        tail: false,
                    },
                ))
                .with_children(|leg| {
                    leg.spawn((Mesh3d(assets.leg.clone()), MeshMaterial3d(fur.clone())));
                });
            }
        });
    });
    id
}

/// Screenshot helper: every wolf look in a row, standing still, facing south
/// (towards the player's spawn): four patterns across, then an alpha.
pub fn spawn_lineup(mut commands: Commands, assets: Res<WolfAssets>, mut rng: ResMut<RngRes>) {
    let (sx, sz) = terrain::PLAYER_SPAWN;
    let z = sz - 9.0;
    for (i, (variant, coat, alpha)) in [(0, 0, false), (1, 1, false), (2, 0, false), (3, 2, false), (2, 1, true)].into_iter().enumerate() {
        let x = sx - 6.0 + i as f32 * 3.0;
        let id = spawn_wolf_with(&mut commands, &assets, Vec2::new(x, z), alpha, Some(variant), Some(coat), &mut rng);
        commands.entity(id).insert((Frozen, Transform::from_xyz(x, terrain::walk_height(x, z), z).with_scale(Vec3::splat(if alpha { 1.3 } else { 1.0 })).with_rotation(Quat::from_rotation_y(std::f32::consts::PI))));
    }
}

fn spawn_pack(commands: &mut Commands, assets: &WolfAssets, center: Vec2, count: usize, rng: &mut RngRes) {
    for i in 0..count {
        let offset = Vec2::new(rng.0.range(-6.0, 6.0), rng.0.range(-6.0, 6.0));
        let p = (center + offset).clamp(Vec2::splat(-HALF_SIZE + 5.0), Vec2::splat(HALF_SIZE - 5.0));
        let _ = spawn_wolf(commands, assets, p, i == 0, rng);
    }
}

fn spawn_initial_packs(mut commands: Commands, assets: Res<WolfAssets>, mut rng: ResMut<RngRes>) {
    for center in [
        Vec2::new(-130.0, -20.0),
        Vec2::new(120.0, -90.0),
        Vec2::new(-10.0, -10.0),
    ] {
        spawn_pack(&mut commands, &assets, center, 3, &mut rng);
    }
}

/// Each new blizzard brings a fresh pack in from the treeline.
fn blizzard_packs(
    mut commands: Commands,
    assets: Res<WolfAssets>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    weather: Res<WeatherRes>,
    player: Query<&Transform, With<Player>>,
    wolves: Query<(), (With<Wolf>, Without<Dying>)>,
) {
    if weather.just_changed != Some(Phase::Blizzard) {
        return;
    }
    let count = wolves.iter().count();
    if count >= MAX_WOLVES {
        return;
    }
    let Ok(ptf) = player.single() else { return };
    let angle = rng.0.range(0.0, std::f32::consts::TAU);
    let center = Vec2::new(ptf.translation.x, ptf.translation.z) + Vec2::new(angle.cos(), angle.sin()) * 55.0;
    let size = (4 + (rng.0.f32() * 3.0) as usize).min(MAX_WOLVES - count);
    spawn_pack(&mut commands, &assets, center, size, &mut rng);
    // The pack announces itself from the treeline.
    let from = Vec3::new(center.x, 1.0, center.y);
    sfx.play_at(Sound::HowlFar, from);
}

fn wolf_ai(
    time: Res<Time>,
    weather: Res<WeatherRes>,
    colliders: Res<Colliders>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    mut fxq: ResMut<FxQueue>,
    player: Query<&Transform, With<Player>>,
    mut wolves: Query<(Entity, &mut Transform, &mut Wolf, &Body), (Without<Player>, Without<Dying>, Without<Frozen>)>,
) {
    let dt = time.delta_secs();
    let Ok(ptf) = player.single() else { return };
    let player_xz = Vec2::new(ptf.translation.x, ptf.translation.z);
    let hunting = weather.weather.wolves_hunting();

    // Snapshot positions for pack separation.
    let positions: Vec<(Entity, Vec2)> = wolves
        .iter()
        .map(|(e, tf, _, _)| (e, Vec2::new(tf.translation.x, tf.translation.z)))
        .collect();

    for (entity, mut tf, mut w, body) in &mut wolves {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        let to_player = player_xz - pos;
        let dist = to_player.length();
        let mode = wolf::decide(dist, hunting, body.health_fraction());
        w.bite_cd = (w.bite_cd - dt).max(0.0);
        w.howl_cd -= dt;

        let mut dir = match mode {
            WolfMode::Chase => to_player.normalize_or_zero(),
            WolfMode::Flee => -to_player.normalize_or_zero(),
            WolfMode::Stalk => {
                // Circle at ~25 m, drifting closer.
                let radial = to_player.normalize_or_zero();
                let tangent = Vec2::new(-radial.y, radial.x);
                if dist > 25.0 {
                    (radial * 0.8 + tangent * 0.6).normalize_or_zero()
                } else {
                    tangent
                }
            }
            WolfMode::Wander => {
                w.wander_timer -= dt;
                if w.wander_timer <= 0.0 || pos.distance(w.wander_target) < 1.5 {
                    w.wander_timer = rng.0.range(4.0, 9.0);
                    w.wander_target = (pos + Vec2::new(rng.0.range(-20.0, 20.0), rng.0.range(-20.0, 20.0)))
                        .clamp(Vec2::splat(-HALF_SIZE + 5.0), Vec2::splat(HALF_SIZE - 5.0));
                }
                (w.wander_target - pos).normalize_or_zero()
            }
        };

        // Keep a little space between pack members.
        for (other, opos) in &positions {
            if *other == entity {
                continue;
            }
            let away = pos - *opos;
            let d = away.length();
            if d > 0.01 && d < 2.0 {
                dir += away / d * (2.0 - d) * 0.8;
            }
        }

        // Don't climb onto the player; stop at biting range.
        let mut speed = wolf::speed(mode) * w.speed_jitter;
        if mode == WolfMode::Chase && dist < BITE_RANGE * 0.8 {
            speed = 0.0;
        }

        let step = dir.normalize_or_zero() * speed * dt;
        let mut new_pos = (pos + step).clamp(Vec2::splat(-HALF_SIZE + 2.0), Vec2::splat(HALF_SIZE - 2.0));
        let (cx, cz) = collision::push_out(new_pos.x, new_pos.y, 0.45 * w.size, &colliders.0);
        new_pos = Vec2::new(cx, cz);
        tf.translation.x = new_pos.x;
        tf.translation.z = new_pos.y;
        tf.translation.y = terrain::walk_height(new_pos.x, new_pos.y);

        // Animation inputs: real distance covered this frame.
        let moved = new_pos.distance(pos);
        w.gait = if dt > 0.0 { moved / dt } else { 0.0 };
        w.stride += moved * STRIDE_PER_METRE / w.size;
        // Break into a gallop when running hard; lunge decays after a bite.
        let target = ((w.gait / wolf::CHASE_SPEED - 0.65) / 0.25).clamp(0.0, 1.0);
        w.gallop += (target - w.gallop) * (dt * 5.0).min(1.0);
        w.lunge = (w.lunge - dt * 3.0).max(0.0);
        // Breath: glowing vapour in the cold, faster when running.
        w.breath_cd -= dt;
        if w.breath_cd <= 0.0 && dist < 50.0 {
            w.breath_cd = (if w.gait > 5.0 { 0.55 } else { 1.9 }) * rng.0.range(0.8, 1.25);
            let mouth = tf.translation + tf.rotation * Vec3::new(0.0, 0.98 * w.size, 1.3 * w.size);
            fxq.spawn(Fx::WolfBreath(mouth, tf.rotation * Vec3::Z));
        }

        let face = if mode == WolfMode::Chase || mode == WolfMode::Stalk {
            to_player
        } else {
            step
        };
        if face.length_squared() > 1e-6 {
            tf.rotation = Quat::from_rotation_y(face.x.atan2(face.y));
        }

        // Pack calls while hunting.
        if (mode == WolfMode::Stalk || (mode == WolfMode::Chase && dist > 15.0)) && w.howl_cd <= 0.0 {
            w.howl_cd = rng.0.range(14.0, 28.0);
            let at = tf.translation + Vec3::Y;
            sfx.play_at(
                if dist < 18.0 {
                    Sound::Growl
                } else if dist < 45.0 {
                    Sound::HowlNear
                } else {
                    Sound::HowlFar
                },
                at,
            );
        }

        // Bite.
        if mode == WolfMode::Chase && dist < BITE_RANGE && w.bite_cd <= 0.0 && game.death.is_none() {
            w.bite_cd = BITE_COOLDOWN;
            w.lunge = 1.0;
            let dmg = if w.alpha { 18.0 } else { 12.0 };
            game.hurt_flash = 1.0;
            sfx.play_at(Sound::Snarl, tf.translation + Vec3::Y);
            if let Some(cause) = game.survival.damage(dmg) {
                game.death = Some(cause);
                msgs.show(cause.describe(), f32::MAX);
            }
        }
    }
}

/// Far-off howls at night and during blizzards, as long as wolves remain.
fn distant_howls(
    time: Res<Time>,
    clock: Res<ClockRes>,
    weather: Res<WeatherRes>,
    mut timer: ResMut<DistantHowl>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    wolves: Query<(), (With<Wolf>, Without<Dying>)>,
    player: Query<&Transform, With<Player>>,
) {
    if !(clock.0.is_night() || weather.weather.phase == Phase::Blizzard) || wolves.is_empty() {
        return;
    }
    timer.0 -= time.delta_secs();
    if timer.0 <= 0.0 {
        timer.0 = rng.0.range(25.0, 55.0);
        // Somewhere out in the dark, 70-110 m away.
        if let Ok(p) = player.single() {
            let dist = rng.0.range(70.0, 110.0);
            sfx.play_at(Sound::HowlFar, random_point_around(p.translation, dist, &mut rng.0));
        }
    }
}

/// What the animation needs from each wolf.
struct Pose {
    stride: f32,
    gait: f32,
    gallop: f32,
    lunge: f32,
    flinch: f32,
    /// Which side a hit came from relative to the wolf (+1 right, -1 left).
    hit_side: f32,
    dead: bool,
}

fn animate_wolves(
    time: Res<Time>,
    wolves: Query<(Entity, &Wolf, &Body, &Transform, Has<Dying>)>,
    mut limbs: Query<(&mut Transform, &WolfLimb), (Without<Wolf>, Without<WolfRig>)>,
    mut rigs: Query<(&mut Transform, &WolfRig), (Without<Wolf>, Without<WolfLimb>)>,
) {
    let state: HashMap<Entity, Pose> = wolves
        .iter()
        .map(|(e, w, body, tf, dead)| {
            let right = tf.rotation * Vec3::X;
            (
                e,
                Pose {
                    stride: w.stride,
                    gait: if dead { 0.0 } else { w.gait },
                    gallop: if dead { 0.0 } else { w.gallop },
                    lunge: w.lunge,
                    flinch: body.flinch,
                    hit_side: if body.hit_dir.dot(right) >= 0.0 { 1.0 } else { -1.0 },
                    dead,
                },
            )
        })
        .collect();
    let t = time.elapsed_secs();
    for (mut tf, limb) in &mut limbs {
        let Some(p) = state.get(&limb.owner) else { continue };
        let effort = (p.gait / wolf::CHASE_SPEED).clamp(0.0, 1.0);
        if limb.tail {
            let wag = (t * (3.0 + 6.0 * effort)).sin() * (0.15 + 0.35 * effort);
            // Streams out behind a galloping wolf, tucks when it is hit.
            let carry = 0.5 - 0.4 * effort - 0.35 * p.gallop + 0.9 * p.flinch;
            tf.rotation = Quat::from_rotation_x(carry) * Quat::from_rotation_y(wag);
        } else {
            let phase = limb.trot_phase + (limb.gallop_phase - limb.trot_phase) * p.gallop;
            let amp = (0.15 + 0.55 * effort) * (1.0 + 0.4 * p.gallop);
            let swing = if p.gait > 0.2 { (p.stride + phase).sin() * amp } else { 0.0 };
            // Forelegs reach forward in a lunge.
            tf.rotation = Quat::from_rotation_x(swing - p.lunge * 0.6 * if limb.gallop_phase < 1.0 { 1.0 } else { -0.3 });
        }
    }
    for (mut tf, rig) in &mut rigs {
        let Some(p) = state.get(&rig.owner) else { continue };
        if p.dead {
            tf.translation = Vec3::ZERO;
            tf.rotation = Quat::IDENTITY;
            continue;
        }
        // Bounding: the body rises and dips, the nose rocks, once per stride.
        let bob = p.gallop * 0.07 * (0.5 + 0.5 * (p.stride * 2.0).sin());
        let rock = p.gallop * 0.1 * (p.stride + 0.8).sin();
        // A lunge drives the head forward and down; a hit rears it back and
        // twists it away from the blow.
        let pitch = rock + p.lunge * 0.32 - p.flinch * 0.35;
        let roll = p.flinch * 0.22 * p.hit_side;
        let breathe = 1.0 + 0.012 * (t * 2.4 + p.stride).sin();
        tf.translation = Vec3::new(0.0, bob, p.lunge * 0.45 - p.flinch * 0.18);
        tf.rotation = Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll);
        tf.scale = Vec3::new(1.0, breathe, 1.0);
    }
}
