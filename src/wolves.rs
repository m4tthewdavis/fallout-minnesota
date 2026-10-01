//! Frostfang wolf packs: translucent, glowing-eyed timber wolves that wander in
//! calm weather and hunt in packs during rad-blizzards.

use bevy::prelude::*;

use crate::player::Player;
use crate::sim::terrain::{self, HALF_SIZE};
use crate::sim::wolf::{self, WolfMode, BITE_RANGE};
use crate::state::{alive, Game, Messages, RngRes, WeatherRes};

const MAX_WOLVES: usize = 16;
const BITE_COOLDOWN: f32 = 1.3;

#[derive(Component)]
pub struct Wolf {
    pub health: f32,
    pub max_health: f32,
    pub alpha: bool,
    /// Visual and hitbox scale.
    pub size: f32,
    bite_cd: f32,
    wander_target: Vec2,
    wander_timer: f32,
    speed_jitter: f32,
}

impl Wolf {
    /// Centre of the body for hit detection.
    pub fn hit_center(&self, tf: &Transform) -> Vec3 {
        tf.translation + Vec3::Y * 0.75 * self.size
    }

    pub fn hit_radius(&self) -> f32 {
        0.8 * self.size
    }
}

#[derive(Resource)]
struct WolfAssets {
    body: Handle<Mesh>,
    head: Handle<Mesh>,
    snout: Handle<Mesh>,
    leg: Handle<Mesh>,
    tail: Handle<Mesh>,
    eye: Handle<Mesh>,
    fur: Handle<StandardMaterial>,
    alpha_fur: Handle<StandardMaterial>,
    eye_mat: Handle<StandardMaterial>,
}

pub struct WolfPlugin;

impl Plugin for WolfPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_wolf_assets, spawn_initial_packs).chain())
            .add_systems(Update, (blizzard_packs, wolf_ai).chain().run_if(alive));
    }
}

fn setup_wolf_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(WolfAssets {
        body: meshes.add(Cuboid::new(0.6, 0.6, 1.4)),
        head: meshes.add(Cuboid::new(0.42, 0.42, 0.5)),
        snout: meshes.add(Cuboid::new(0.22, 0.2, 0.3)),
        leg: meshes.add(Cuboid::new(0.14, 0.6, 0.14)),
        tail: meshes.add(Cuboid::new(0.12, 0.12, 0.6)),
        eye: meshes.add(Sphere::new(0.05)),
        // "Translucent fur that blends into snow."
        fur: materials.add(StandardMaterial {
            base_color: Color::srgba(0.86, 0.90, 0.95, 0.72),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            ..default()
        }),
        alpha_fur: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.60, 0.68, 0.85),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            ..default()
        }),
        // Glowing eyes are the only tell in a whiteout.
        eye_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.9, 1.0),
            emissive: LinearRgba::rgb(1.5, 6.0, 8.0),
            unlit: true,
            ..default()
        }),
    });
}

fn spawn_wolf(commands: &mut Commands, assets: &WolfAssets, pos: Vec2, alpha: bool, rng: &mut RngRes) {
    let size = if alpha { 1.3 } else { rng.0.range(0.9, 1.1) };
    let health = if alpha { 100.0 } else { 60.0 };
    let fur = if alpha {
        assets.alpha_fur.clone()
    } else {
        assets.fur.clone()
    };
    let y = terrain::walk_height(pos.x, pos.y);

    commands
        .spawn((
            Transform::from_xyz(pos.x, y, pos.y).with_scale(Vec3::splat(size)),
            Visibility::default(),
            Wolf {
                health,
                max_health: health,
                alpha,
                size,
                bite_cd: 0.0,
                wander_target: pos,
                wander_timer: 0.0,
                speed_jitter: rng.0.range(0.85, 1.05),
            },
        ))
        .with_children(|w| {
            w.spawn((
                Mesh3d(assets.body.clone()),
                MeshMaterial3d(fur.clone()),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ));
            w.spawn((
                Mesh3d(assets.head.clone()),
                MeshMaterial3d(fur.clone()),
                Transform::from_xyz(0.0, 1.0, 0.85),
            ));
            w.spawn((
                Mesh3d(assets.snout.clone()),
                MeshMaterial3d(fur.clone()),
                Transform::from_xyz(0.0, 0.92, 1.2),
            ));
            w.spawn((
                Mesh3d(assets.tail.clone()),
                MeshMaterial3d(fur.clone()),
                Transform::from_xyz(0.0, 0.9, -0.9).with_rotation(Quat::from_rotation_x(0.5)),
            ));
            for (x, z) in [(-0.2, 0.5), (0.2, 0.5), (-0.2, -0.5), (0.2, -0.5)] {
                w.spawn((
                    Mesh3d(assets.leg.clone()),
                    MeshMaterial3d(fur.clone()),
                    Transform::from_xyz(x, 0.3, z),
                ));
            }
            for x in [-0.12, 0.12] {
                w.spawn((
                    Mesh3d(assets.eye.clone()),
                    MeshMaterial3d(assets.eye_mat.clone()),
                    Transform::from_xyz(x, 1.08, 1.11),
                ));
            }
        });
}

fn spawn_pack(commands: &mut Commands, assets: &WolfAssets, center: Vec2, count: usize, rng: &mut RngRes) {
    for i in 0..count {
        let offset = Vec2::new(rng.0.range(-6.0, 6.0), rng.0.range(-6.0, 6.0));
        let p = (center + offset).clamp(Vec2::splat(-HALF_SIZE + 5.0), Vec2::splat(HALF_SIZE - 5.0));
        spawn_wolf(commands, assets, p, i == 0, rng);
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
    weather: Res<WeatherRes>,
    player: Query<&Transform, With<Player>>,
    wolves: Query<(), With<Wolf>>,
) {
    if weather.just_changed != Some(crate::sim::weather::Phase::Blizzard) {
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
}

fn wolf_ai(
    time: Res<Time>,
    weather: Res<WeatherRes>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut rng: ResMut<RngRes>,
    player: Query<&Transform, With<Player>>,
    mut wolves: Query<(Entity, &mut Transform, &mut Wolf), Without<Player>>,
) {
    let dt = time.delta_secs();
    let Ok(ptf) = player.single() else { return };
    let player_xz = Vec2::new(ptf.translation.x, ptf.translation.z);
    let hunting = weather.weather.wolves_hunting();

    // Snapshot positions for pack separation.
    let positions: Vec<(Entity, Vec2)> = wolves
        .iter()
        .map(|(e, tf, _)| (e, Vec2::new(tf.translation.x, tf.translation.z)))
        .collect();

    for (entity, mut tf, mut w) in &mut wolves {
        let pos = Vec2::new(tf.translation.x, tf.translation.z);
        let to_player = player_xz - pos;
        let dist = to_player.length();
        let mode = wolf::decide(dist, hunting, w.health / w.max_health);
        w.bite_cd = (w.bite_cd - dt).max(0.0);

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
        let new_pos = (pos + step).clamp(Vec2::splat(-HALF_SIZE + 2.0), Vec2::splat(HALF_SIZE - 2.0));
        tf.translation.x = new_pos.x;
        tf.translation.z = new_pos.y;
        tf.translation.y = terrain::walk_height(new_pos.x, new_pos.y);

        let face = if mode == WolfMode::Chase || mode == WolfMode::Stalk {
            to_player
        } else {
            step
        };
        if face.length_squared() > 1e-6 {
            tf.rotation = Quat::from_rotation_y(face.x.atan2(face.y));
        }

        // Bite.
        if mode == WolfMode::Chase && dist < BITE_RANGE && w.bite_cd <= 0.0 && game.death.is_none() {
            w.bite_cd = BITE_COOLDOWN;
            let dmg = if w.alpha { 18.0 } else { 12.0 };
            game.hurt_flash = 1.0;
            if let Some(cause) = game.survival.damage(dmg) {
                game.death = Some(cause);
                msgs.show(cause.describe(), f32::MAX);
            }
        }
    }
}
