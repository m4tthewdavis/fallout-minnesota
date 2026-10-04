//! The Glowmoose: a mutated bull moose the size of a van, with bioluminescent
//! fungus on its back and glowing antler tips. It grazes until you come close,
//! stares, paws the snow and bellows, then charges in a straight line at
//! where you were standing. Dodge sideways, or lead it into a tree: a crash
//! leaves it stunned. The rules live in `sim::moose`; this is the body, the
//! animation and the damage it does.

use bevy::prelude::*;

use crate::enemy::{Body, Dying, Frozen, Species};
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::collision;
use crate::sim::meshgen;
use crate::sim::moose::{self as brain, Event, Mode, Moose};
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::state::{alive, Colliders, Game, Hostile, Messages, RngRes, SfxQueue};

const MOOSE_COUNT: usize = 2;
/// Pivot of the neck and head: the top of the shoulders.
const NECK_PIVOT: Vec3 = Vec3::new(0.0, 1.72, 1.0);
const BODY_RADIUS: f32 = 1.0;
const TURN_RATE: f32 = 3.5;

#[derive(Component)]
pub struct MooseAi {
    brain: Moose,
    /// Accumulated walk-cycle phase and current ground speed.
    stride: f32,
    gait: f32,
    /// A charge step ended inside something solid.
    crashed: bool,
    /// Which half-stride last thumped, to time hoofbeats.
    last_beat: i32,
    /// Smoothed head angle (radians down) and the pawing amount.
    head: f32,
}

#[derive(Component)]
struct MooseLimb {
    owner: Entity,
    phase: f32,
    /// Front-left leg: the one that paws.
    paws: bool,
}

#[derive(Component)]
struct MooseRig {
    owner: Entity,
}

#[derive(Component)]
struct MooseHead {
    owner: Entity,
}

#[derive(Resource)]
pub(crate) struct MooseAssets {
    body: Handle<Mesh>,
    head: Handle<Mesh>,
    leg: Handle<Mesh>,
    antlers: [Handle<Mesh>; 2],
    tips: [Vec<Vec3>; 2],
    tip: Handle<Mesh>,
    fungus: Handle<Mesh>,
    eye: Handle<Mesh>,
    fur: Handle<StandardMaterial>,
    antler_mat: Handle<StandardMaterial>,
    glow_mat: Handle<StandardMaterial>,
    eye_mat: Handle<StandardMaterial>,
}

pub struct MoosePlugin;

impl Plugin for MoosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_assets, spawn_initial).chain())
            .add_systems(Update, (moose_ai.run_if(alive), animate_moose));
    }
}

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (left, left_tips) = meshgen::moose_antler(-1.0);
    let (right, right_tips) = meshgen::moose_antler(1.0);
    let v3 = |t: Vec<[f32; 3]>| t.into_iter().map(Vec3::from_array).collect::<Vec<_>>();
    commands.insert_resource(MooseAssets {
        body: meshes.add(to_mesh_tangents(&meshgen::moose_body())),
        head: meshes.add(to_mesh_tangents(&meshgen::moose_head())),
        leg: meshes.add(to_mesh(&meshgen::moose_leg())),
        antlers: [meshes.add(to_mesh_tangents(&left)), meshes.add(to_mesh_tangents(&right))],
        tips: [v3(left_tips), v3(right_tips)],
        tip: meshes.add(Sphere::new(0.055)),
        fungus: meshes.add(to_mesh(&meshgen::blob(0.16, 0.7, 0.3, 91, 0.5))),
        eye: meshes.add(Sphere::new(0.04)),
        fur: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            ..default()
        }),
        antler_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.8, 0.7),
            perceptual_roughness: 0.7,
            ..default()
        }),
        glow_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.9, 0.5),
            emissive: LinearRgba::rgb(0.8, 7.0, 1.6),
            unlit: true,
            ..default()
        }),
        eye_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 1.0, 0.4),
            emissive: LinearRgba::rgb(9.0, 14.0, 2.0),
            unlit: true,
            ..default()
        }),
    });
}

pub(crate) fn spawn_moose(commands: &mut Commands, a: &MooseAssets, pos: Vec2, rng: &mut RngRes) -> Entity {
    let y = terrain::walk_height(pos.x, pos.y);
    let yaw = rng.0.range(0.0, std::f32::consts::TAU);
    let id = commands
        .spawn((
            Transform::from_xyz(pos.x, y, pos.y).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Hostile,
            Body::new(Species::Moose, 220.0, 1.6, 1.25),
            MooseAi {
                brain: Moose::new(&mut rng.0),
                stride: rng.0.range(0.0, std::f32::consts::TAU),
                gait: 0.0,
                crashed: false,
                last_beat: 0,
                head: 0.0,
            },
        ))
        .id();
    commands.entity(id).with_children(|m| {
        m.spawn((Transform::default(), Visibility::default(), MooseRig { owner: id })).with_children(|rig| {
            rig.spawn((Mesh3d(a.body.clone()), MeshMaterial3d(a.fur.clone())));
            // Fungus growing along the spine, glowing.
            for (i, (z, x)) in [(-0.7f32, 0.05f32), (-0.3, -0.12), (0.1, 0.1), (0.5, -0.05), (0.85, 0.12), (-0.95, -0.06), (0.3, 0.2)].into_iter().enumerate() {
                let y = 2.18 + 0.1 * (i as f32 * 1.7).sin() + if z > 0.5 { 0.1 } else { 0.0 };
                rig.spawn((
                    Mesh3d(a.fungus.clone()),
                    MeshMaterial3d(a.glow_mat.clone()),
                    Transform::from_xyz(x, y, z).with_scale(Vec3::splat(0.8 + 0.3 * (i % 3) as f32)),
                ));
            }
            rig.spawn((
                PointLight {
                    color: Color::srgb(0.3, 1.0, 0.5),
                    intensity: 70_000.0,
                    range: 9.0,
                    ..default()
                },
                Transform::from_xyz(0.0, 2.6, 0.2),
            ));
            // Neck, head and antlers swing from the shoulders.
            rig.spawn((Transform::from_translation(NECK_PIVOT), Visibility::default(), MooseHead { owner: id })).with_children(|h| {
                h.spawn((Mesh3d(a.head.clone()), MeshMaterial3d(a.fur.clone()), Transform::from_translation(-NECK_PIVOT)));
                for side in 0..2 {
                    h.spawn((Mesh3d(a.antlers[side].clone()), MeshMaterial3d(a.antler_mat.clone()), Transform::from_translation(-NECK_PIVOT)));
                    for tip in &a.tips[side] {
                        h.spawn((Mesh3d(a.tip.clone()), MeshMaterial3d(a.glow_mat.clone()), Transform::from_translation(*tip - NECK_PIVOT)));
                    }
                }
                for x in [-0.17f32, 0.17] {
                    h.spawn((Mesh3d(a.eye.clone()), MeshMaterial3d(a.eye_mat.clone()), Transform::from_translation(Vec3::new(x, 1.8, 2.0) - NECK_PIVOT)));
                }
            });
            // Four long legs hang from the hips and shoulders.
            let pi = std::f32::consts::PI;
            for (x, z, phase, paws) in [(-0.3f32, 0.75f32, 0.0f32, true), (0.3, 0.75, pi, false), (-0.3, -0.8, pi, false), (0.3, -0.8, 0.0, false)] {
                rig.spawn((Transform::from_xyz(x, 1.15, z), Visibility::default(), MooseLimb { owner: id, phase, paws })).with_children(|l| {
                    l.spawn((Mesh3d(a.leg.clone()), MeshMaterial3d(a.fur.clone())));
                });
            }
        });
    });
    id
}

fn spawn_initial(mut commands: Commands, assets: Res<MooseAssets>, mut rng: ResMut<RngRes>, colliders: Res<Colliders>) {
    let mut placed = 0;
    for _ in 0..400 {
        if placed >= MOOSE_COUNT {
            break;
        }
        let (x, z) = (rng.0.range(-HALF_SIZE + 25.0, HALF_SIZE - 25.0), rng.0.range(-HALF_SIZE + 25.0, HALF_SIZE - 25.0));
        if !terrain::is_open_ground(x, z) || terrain::dist_to_vault(x, z) < 85.0 || collision::blocked(x, z, 2.0, &colliders.0) {
            continue;
        }
        spawn_moose(&mut commands, &assets, Vec2::new(x, z), &mut rng);
        placed += 1;
    }
}

/// Screenshot helper: one moose standing 15 m ahead of the player's spawn.
pub fn spawn_lineup(mut commands: Commands, assets: Res<MooseAssets>, mut rng: ResMut<RngRes>) {
    let (sx, sz) = terrain::PLAYER_SPAWN;
    let (x, z) = (sx + 1.5, sz - 15.0);
    let id = spawn_moose(&mut commands, &assets, Vec2::new(x, z), &mut rng);
    commands
        .entity(id)
        .insert((Frozen, Transform::from_xyz(x, terrain::walk_height(x, z), z).with_rotation(Quat::from_rotation_y(std::f32::consts::PI + 0.35))));
}

fn moose_ai(
    time: Res<Time>,
    colliders: Res<Colliders>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    mut player: Query<&mut Transform, (With<Player>, Without<MooseAi>)>,
    mut moose: Query<(&mut Transform, &mut MooseAi, &Body), (Without<Dying>, Without<Frozen>)>,
) {
    let dt = time.delta_secs();
    let Ok(mut ptf) = player.single_mut() else { return };
    for (mut tf, mut m, body) in &mut moose {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        let ppos = Vec2::new(ptf.translation.x, ptf.translation.z);
        let to_player = ppos - pos;
        let dist = to_player.length();
        let hurt = body.flinch > 0.95;
        let crashed = std::mem::take(&mut m.crashed);
        let step = m.brain.update(dt, dist, to_player.to_array(), hurt, crashed, &mut rng.0);

        // Move, sliding around (or crashing into) trees and buildings.
        let dir = Vec2::from_array(step.dir);
        let wanted = pos + dir * step.speed * dt;
        let (cx, cz) = collision::push_out(wanted.x, wanted.y, BODY_RADIUS, &colliders.0);
        let mut new_pos = Vec2::new(cx, cz).clamp(Vec2::splat(-HALF_SIZE + 3.0), Vec2::splat(HALF_SIZE - 3.0));
        if m.brain.mode == Mode::Charge && step.speed > 0.0 && new_pos.distance(wanted) > 0.12 {
            m.crashed = true;
        }
        if step.speed == 0.0 {
            new_pos = pos;
        }
        tf.translation = Vec3::new(new_pos.x, terrain::walk_height(new_pos.x, new_pos.y), new_pos.y);
        let moved = new_pos.distance(pos);
        m.gait = if dt > 0.0 { moved / dt } else { 0.0 };
        m.stride += moved * 1.0;

        // Turn to face where it means to go (or glare at you).
        let face = Vec2::from_array(step.face);
        if face.length_squared() > 1e-6 {
            let target = Quat::from_rotation_y(face.x.atan2(face.y));
            let rate = if m.brain.mode == Mode::Windup { 8.0 } else { TURN_RATE };
            tf.rotation = tf.rotation.slerp(target, (rate * dt).min(1.0));
        }

        // Trample the player once per charge.
        if m.brain.strikes(dist) {
            game.hurt_flash = 1.0;
            let push = if dir.length_squared() > 0.0 { dir.normalize() } else { Vec2::X };
            let knock = ppos + push * 3.5;
            let (kx, kz) = collision::push_out(knock.x, knock.y, 0.4, &colliders.0);
            ptf.translation.x = kx;
            ptf.translation.z = kz;
            ptf.translation.y = terrain::walk_height(kx, kz) + EYE_HEIGHT;
            msgs.show("The Glowmoose tramples you!", 2.5);
            if let Some(cause) = game.survival.damage(brain::HIT_DAMAGE) {
                game.death = Some(cause);
                msgs.show(cause.describe(), f32::MAX);
            }
        }

        // Sounds.
        let at = tf.translation + Vec3::Y * 2.0;
        for e in &step.events {
            match e {
                Event::Bellow => sfx.play_at(Sound::MooseBellow, at),
                Event::Charged => sfx.play_at(Sound::Hoof, tf.translation),
                Event::Crashed => {
                    sfx.play_at(Sound::MeleeHit, tf.translation + Vec3::Y);
                    sfx.play_at(Sound::MooseGrunt, at);
                    msgs.show("The Glowmoose crashes headlong and reels, stunned!", 3.0);
                }
                Event::ChargeEnded | Event::Grunt => sfx.play_at(Sound::MooseGrunt, at),
            }
        }
        // Hoofbeats while it is moving fast.
        let beat = (m.stride / std::f32::consts::PI * 1.4) as i32;
        if beat != m.last_beat {
            m.last_beat = beat;
            if m.gait > 3.0 {
                sfx.play_at(Sound::Hoof, tf.translation);
            }
        }
    }
}

fn animate_moose(
    time: Res<Time>,
    mut moose: Query<(Entity, &mut MooseAi, &Body, Has<Dying>)>,
    mut limbs: Query<(&mut Transform, &MooseLimb), (Without<MooseAi>, Without<MooseRig>, Without<MooseHead>)>,
    mut rigs: Query<(&mut Transform, &MooseRig), (Without<MooseAi>, Without<MooseLimb>, Without<MooseHead>)>,
    mut heads: Query<(&mut Transform, &MooseHead), (Without<MooseAi>, Without<MooseLimb>, Without<MooseRig>)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    struct S {
        stride: f32,
        gait: f32,
        windup: f32,
        mode: Mode,
        flinch: f32,
        dead: bool,
        head: f32,
    }
    let mut state = std::collections::HashMap::new();
    for (e, mut m, body, dead) in &mut moose {
        // Head target: up and watching, down pawing, locked low charging, hanging when stunned.
        let target = match m.brain.mode {
            Mode::Graze => 0.35 + 0.1 * (t * 0.6 + e.index() as f32).sin(),
            Mode::Alert => -0.12,
            Mode::Windup => 0.2 + 0.75 * m.brain.windup_progress(),
            Mode::Charge => 0.85,
            Mode::Recover => {
                if m.brain.stunned {
                    1.0 + 0.1 * (t * 3.0).sin()
                } else {
                    0.5
                }
            }
        };
        let target = if dead { 1.2 } else { target };
        m.head += (target - m.head) * (dt * 5.0).min(1.0);
        state.insert(
            e,
            S {
                stride: m.stride,
                gait: if dead { 0.0 } else { m.gait },
                windup: m.brain.windup_progress(),
                mode: m.brain.mode,
                flinch: body.flinch,
                dead,
                head: m.head,
            },
        );
    }
    for (mut tf, limb) in &mut limbs {
        let Some(s) = state.get(&limb.owner) else { continue };
        let effort = (s.gait / brain::CHARGE_SPEED).clamp(0.0, 1.0);
        let amp = 0.18 + 0.7 * effort;
        let swing = if s.gait > 0.2 { (s.stride * 1.4 + limb.phase).sin() * amp } else { 0.0 };
        let paw = if limb.paws && s.mode == Mode::Windup { -0.7 * s.windup * (t * 13.0).sin().max(0.0) - 0.25 * s.windup } else { 0.0 };
        tf.rotation = Quat::from_rotation_x(swing + paw);
    }
    for (mut tf, rig) in &mut rigs {
        let Some(s) = state.get(&rig.owner) else { continue };
        if s.dead {
            tf.translation = Vec3::ZERO;
            tf.rotation = Quat::IDENTITY;
            continue;
        }
        let effort = (s.gait / brain::CHARGE_SPEED).clamp(0.0, 1.0);
        let bob = 0.12 * effort * (0.5 + 0.5 * (s.stride * 2.8).sin());
        let rock = 0.05 * effort * (s.stride * 1.4 + 0.7).sin();
        tf.translation = Vec3::new(0.0, bob, -s.flinch * 0.12);
        tf.rotation = Quat::from_rotation_x(rock - s.flinch * 0.06) * Quat::from_rotation_z(s.flinch * 0.04);
    }
    for (mut tf, head) in &mut heads {
        let Some(s) = state.get(&head.owner) else { continue };
        // Bellowing throws the head up at the start of the windup.
        let toss = if s.mode == Mode::Windup && s.windup < 0.25 { -0.5 * (1.0 - s.windup * 4.0) } else { 0.0 };
        tf.translation = NECK_PIVOT;
        tf.rotation = Quat::from_rotation_x(s.head + toss - s.flinch * 0.25);
    }
}
