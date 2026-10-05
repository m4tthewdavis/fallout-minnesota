//! Rad-crows: glowing black crows that wheel over their roosts in flocks.
//! A gunshot scatters them, and if it was close they come down on you one
//! after another: peck, climb, circle, dive again, until they lose interest.
//! They're fragile (one hit kills) and fall out of the sky when shot. The
//! rules are in `sim::crow`; this is the bird, its flight and the damage.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::enemy::{Body, Dying, Frozen, Species};
use crate::meshes::to_mesh;
use crate::player::Player;
use crate::sim::crow::{self as brain, Crow, Mode};
use crate::sim::meshgen;
use crate::sim::survival::DeathCause;
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::state::{alive, outdoors, Game, Gunshot, Hostile, Messages, RngRes, SfxQueue};

/// Where the flocks roost (x, z) and how many birds.
const ROOSTS: [(f32, f32, usize); 4] = [(-60.0, 30.0, 5), (90.0, 20.0, 5), (30.0, -100.0, 4), (135.0, -118.0, 4)];

#[derive(Component)]
pub struct CrowAi {
    brain: Crow,
    vel: Vec3,
    flap: f32,
    caw_cd: f32,
}

/// A wing pivot: flaps while flying, folds in a dive.
#[derive(Component)]
struct Wing {
    owner: Entity,
    /// +1 left, -1 right.
    side: f32,
}

#[derive(Resource)]
pub(crate) struct CrowAssets {
    body: Handle<Mesh>,
    head: Handle<Mesh>,
    beak: Handle<Mesh>,
    wing: Handle<Mesh>,
    tail: Handle<Mesh>,
    eye: Handle<Mesh>,
    feathers: Handle<StandardMaterial>,
    beak_mat: Handle<StandardMaterial>,
    eye_mat: Handle<StandardMaterial>,
}

pub struct CrowPlugin;

impl Plugin for CrowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_assets, spawn_initial).chain().after(crate::state::WorldGen))
            .add_systems(Update, (crow_ai.run_if(alive.and(outdoors)), animate_crows).chain());
    }
}

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(CrowAssets {
        body: meshes.add(to_mesh(&meshgen::blob(0.2, 0.55, 0.05, 31, 1.0).scaled([0.8, 0.8, 1.6]))),
        head: meshes.add(Sphere::new(0.085)),
        beak: meshes.add(Cone::new(0.035, 0.16)),
        wing: meshes.add(Cuboid::new(0.6, 0.014, 0.24)),
        tail: meshes.add(Cuboid::new(0.14, 0.012, 0.26)),
        eye: meshes.add(Sphere::new(0.016)),
        // Black feathers with a sickly glow in the creases.
        feathers: materials.add(StandardMaterial { base_color: Color::srgb(0.045, 0.05, 0.06), emissive: LinearRgba::rgb(0.0, 0.006, 0.002), perceptual_roughness: 0.7, ..default() }),
        beak_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.1, 0.08), perceptual_roughness: 0.6, ..default() }),
        eye_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.4, 1.0, 0.4), emissive: LinearRgba::rgb(0.5, 6.0, 0.8), unlit: true, ..default() }),
    });
}

pub(crate) fn spawn_initial(mut commands: Commands, assets: Res<CrowAssets>, mut rng: ResMut<RngRes>) {
    for (x, z, n) in ROOSTS {
        for _ in 0..n {
            let a = rng.0.range(0.0, TAU);
            let r = rng.0.range(brain::CIRCLE_RADIUS * 0.6, brain::CIRCLE_RADIUS * 1.2);
            let alt = rng.0.range(brain::CIRCLE_ALT.0, brain::CIRCLE_ALT.1);
            spawn_crow(&mut commands, &assets, Vec3::new(x + a.cos() * r, terrain::walk_height(x, z) + alt, z + a.sin() * r), [x, z], &mut rng);
        }
    }
}

/// Screenshot helper: three crows hanging in the air a few metres ahead of
/// the player's spawn, wings out.
pub fn spawn_lineup(mut commands: Commands, assets: Res<CrowAssets>, mut rng: ResMut<RngRes>) {
    let (sx, sz) = terrain::PLAYER_SPAWN;
    for (i, (dx, dy, dz)) in [(-1.5f32, 0.0f32, 4.0f32), (0.3, 0.4, 6.0), (1.8, -0.2, 5.0)].into_iter().enumerate() {
        let (x, z) = (sx + dx, sz - dz);
        let y = terrain::walk_height(x, z) + 1.9 + dy;
        let id = spawn_crow(&mut commands, &assets, Vec3::new(x, y, z), [x, z], &mut rng);
        commands.entity(id).insert((Frozen, Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(PI * 0.5 + 0.8 * i as f32))));
    }
}

fn spawn_crow(commands: &mut Commands, a: &CrowAssets, pos: Vec3, home: [f32; 2], rng: &mut RngRes) -> Entity {
    let id = commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            Hostile,
            Body::new(Species::Crow, 8.0, 0.0, 0.42),
            CrowAi { brain: Crow::new(home, &mut rng.0), vel: Vec3::ZERO, flap: rng.0.range(0.0, TAU), caw_cd: rng.0.range(2.0, 12.0) },
        ))
        .id();
    commands.entity(id).with_children(|c| {
        c.spawn((Mesh3d(a.body.clone()), MeshMaterial3d(a.feathers.clone())));
        c.spawn((Mesh3d(a.head.clone()), MeshMaterial3d(a.feathers.clone()), Transform::from_xyz(0.0, 0.06, 0.3)));
        c.spawn((
            Mesh3d(a.beak.clone()),
            MeshMaterial3d(a.beak_mat.clone()),
            Transform::from_xyz(0.0, 0.05, 0.4).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ));
        for x in [-0.045f32, 0.045] {
            c.spawn((Mesh3d(a.eye.clone()), MeshMaterial3d(a.eye_mat.clone()), Transform::from_xyz(x, 0.09, 0.34), NotShadowCaster));
        }
        c.spawn((Mesh3d(a.tail.clone()), MeshMaterial3d(a.feathers.clone()), Transform::from_xyz(0.0, 0.02, -0.38).with_rotation(Quat::from_rotation_x(-0.15))));
        for side in [1.0f32, -1.0] {
            c.spawn((Transform::from_xyz(side * 0.07, 0.04, 0.04), Visibility::default(), Wing { owner: id, side })).with_children(|w| {
                w.spawn((Mesh3d(a.wing.clone()), MeshMaterial3d(a.feathers.clone()), Transform::from_xyz(side * 0.3, 0.0, 0.0)));
            });
        }
    });
    id
}

/// Fly: hear gunshots, steer, peck.
#[allow(clippy::too_many_arguments)]
fn crow_ai(
    time: Res<Time>,
    mut shots: EventReader<Gunshot>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
    mut crows: Query<(&mut Transform, &mut CrowAi), (Without<Player>, Without<Dying>, Without<Frozen>)>,
) {
    let dt = time.delta_secs();
    let Ok(ptf) = player.single() else { return };
    let head = ptf.translation.to_array();
    let heard: Vec<Gunshot> = shots.read().copied().collect();
    for (mut tf, mut crow) in &mut crows {
        let pos = tf.translation.to_array();
        let before = crow.brain.mode;
        for s in &heard {
            crow.brain.hear_shot(pos, s.pos.to_array(), &mut rng.0);
        }
        let step = crow.brain.update(dt, pos, head, &mut rng.0);
        // Ease towards the wanted velocity so turns are wide and swoops arc.
        let want = Vec3::from_array(step.vel);
        let k = (dt * 3.5).min(1.0);
        crow.vel = crow.vel.lerp(want, k);
        tf.translation += crow.vel * dt;
        // Stay over the map and off the ground.
        tf.translation.x = tf.translation.x.clamp(-HALF_SIZE + 3.0, HALF_SIZE - 3.0);
        tf.translation.z = tf.translation.z.clamp(-HALF_SIZE + 3.0, HALF_SIZE - 3.0);
        let floor = terrain::walk_height(tf.translation.x, tf.translation.z) + 0.7;
        if tf.translation.y < floor {
            tf.translation.y = floor;
            crow.vel.y = crow.vel.y.max(0.0);
        }
        // Face the way it's flying, nose up when climbing.
        if crow.vel.length_squared() > 0.5 {
            let yaw = crow.vel.x.atan2(crow.vel.z);
            let pitch = (crow.vel.y / crow.vel.length()).asin();
            tf.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
        }
        crow.flap += dt * if matches!(crow.brain.mode, Mode::Scatter | Mode::Climb) { 17.0 } else { 10.0 };

        // Caws: when startled, and now and then while hunting.
        crow.caw_cd -= dt;
        let startled = before != Mode::Scatter && crow.brain.mode == Mode::Scatter;
        let hunting = crow.brain.anger() > 0.0 && crow.caw_cd <= 0.0;
        if startled || hunting {
            crow.caw_cd = rng.0.range(2.5, 6.0);
            sfx.play_at(Sound::Caw, tf.translation);
        }

        if step.peck && game.death.is_none() {
            game.hurt_flash = 0.7;
            sfx.play_at(Sound::Caw, tf.translation);
            msgs.show("A rad-crow pecks you!", 1.2);
            if let Some(cause) = game.survival.damage_by(brain::PECK_DAMAGE, DeathCause::Pecked) {
                game.death = Some(cause);
                msgs.show(cause.describe(), f32::MAX);
            }
        }
    }
}

/// Flap the wings, fold them in a dive, let them hang when dead.
fn animate_crows(crows: Query<&CrowAi, Without<Dying>>, mut wings: Query<(&Wing, &mut Transform)>) {
    for (wing, mut tf) in &mut wings {
        let (angle, sweep) = match crows.get(wing.owner) {
            Ok(c) if c.brain.mode == Mode::Dive => (0.9, -0.5),
            Ok(c) => (0.15 + 0.75 * c.flap.sin(), 0.0),
            Err(_) => (-0.9, 0.0),
        };
        tf.rotation = Quat::from_rotation_z(angle * wing.side) * Quat::from_rotation_y(sweep * wing.side);
    }
}
