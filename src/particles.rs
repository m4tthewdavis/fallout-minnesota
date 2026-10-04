//! Billboard particle effects: snow kicked up by boots, breath vapour in the
//! cold, chimney smoke, fire flames and embers, muzzle flashes and smoke,
//! ejected brass, fur and blood from wolf hits (blood stains the snow),
//! shattering nuclear ice and drifting radioactive motes.
//!
//! Each particle is a camera-facing quad. Fading swaps between a few
//! pre-built materials of decreasing opacity instead of giving every particle
//! its own material.

use std::collections::HashMap;

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::synth::Sound;
use crate::sim::terrain;
use crate::state::{alive, ClockRes, Fx, FxQueue, Game, Lifetime, RngRes, SfxQueue, WeatherRes};

const FADE_STEPS: usize = 6;
/// Emitters further than this from the camera don't spawn anything.
const EMIT_RANGE: f32 = 90.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Look {
    Snow,
    Smoke,
    Breath,
    Ember,
    Fur,
    Blood,
    Mote,
    Ice,
    Flash,
    Flame,
    WolfBreath,
    Spark,
    FireSmoke,
}

#[derive(Resource)]
struct ParticleAssets {
    quad: Handle<Mesh>,
    casing: Handle<Mesh>,
    casing_mat: Handle<StandardMaterial>,
    stain: Handle<Mesh>,
    stain_mat: Handle<StandardMaterial>,
    /// For each look, materials from full strength to faint.
    looks: HashMap<Look, Vec<Handle<StandardMaterial>>>,
}

#[derive(Component)]
struct Particle {
    look: Look,
    vel: Vec3,
    age: f32,
    life: f32,
    size: (f32, f32),
    gravity: f32,
    drag: f32,
    /// Blown along by the wind.
    windy: bool,
    step: usize,
    /// Leaves a blood stain where it lands.
    stains: bool,
}

/// An ejected casing: tumbles, bounces once with a tink, then lies there.
#[derive(Component)]
struct Casing {
    vel: Vec3,
    spin: Vec3,
    bounced: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmitterKind {
    ChimneySmoke,
    Embers,
    RadMotes,
    /// Thick, dark smoke rolling off a fire.
    FireSmoke,
    /// Bright sparks, with the odd pop of a whole shower.
    Sparks,
}

/// Spawns particles every `every` seconds while the camera is near.
#[derive(Component)]
pub struct Emitter {
    kind: EmitterKind,
    every: f32,
    timer: f32,
}

impl Emitter {
    pub fn new(kind: EmitterKind, every: f32) -> Self {
        Emitter { kind, every, timer: 0.0 }
    }
}

/// One tongue of flame: an animated flipbook quad standing on its base,
/// turned to face you, flickering and leaning with the wind.
#[derive(Component)]
pub struct Flame {
    pub seed: f32,
    /// Width and height in metres.
    pub size: Vec2,
}

/// The warm patch of light a fire throws on the snow round it.
#[derive(Component)]
pub struct FireGlow {
    seed: f32,
    strength: f32,
}

/// How bright a fire is right now (about 0.7..1.15): flames, their light and
/// the glow on the ground all follow this, so they pulse together.
pub fn fire_flicker(t: f32, seed: f32) -> f32 {
    0.9 + 0.1 * (t * 7.3 + seed).sin() + 0.07 * (t * 13.1 + seed * 2.0).sin() + 0.05 * (t * 23.7 + seed * 3.0).sin() + 0.04 * (t * 41.0 + seed * 5.0).sin()
}

/// Frames in the flame flipbook (textures/generated/flame_sheet.png).
const FLAME_FRAMES: usize = 8;

#[derive(Resource)]
struct FlameSheet {
    quad: Handle<Mesh>,
    frames: Vec<Handle<StandardMaterial>>,
}

/// A whole fire at `at` (the base of the flames): several tongues, embers,
/// sparks, smoke, a flickering light and a glow on the ground. `scale` 1 is
/// a fire-barrel fire.
pub fn spawn_fire(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, soft: &Handle<Image>, at: Vec3, scale: f32, seed: f32, shadows: bool) {
    // A big central tongue and smaller ones round it.
    let tongues = [(0.0f32, 0.0f32, 0.55f32, 1.05f32), (0.14, 0.05, 0.38, 0.75), (-0.13, -0.04, 0.36, 0.7), (0.03, -0.13, 0.32, 0.62), (-0.04, 0.13, 0.3, 0.55)];
    for (k, (dx, dz, w, h)) in tongues.into_iter().enumerate() {
        commands.spawn((
            Transform::from_translation(at + Vec3::new(dx, 0.0, dz) * scale),
            Visibility::default(),
            Flame { seed: seed + k as f32 * 1.7, size: Vec2::new(w, h) * scale },
        ));
    }
    commands.spawn((Transform::from_translation(at + Vec3::Y * 0.15 * scale), Emitter::new(EmitterKind::Embers, 0.12 / scale)));
    commands.spawn((Transform::from_translation(at + Vec3::Y * 0.3 * scale), Emitter::new(EmitterKind::Sparks, 0.09 / scale)));
    commands.spawn((Transform::from_translation(at + Vec3::Y * 0.9 * scale), Emitter::new(EmitterKind::FireSmoke, 0.22 / scale)));
    let base = 400_000.0 * scale;
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.2),
            intensity: base,
            range: 18.0 * scale.max(0.7),
            shadows_enabled: shadows,
            ..default()
        },
        Transform::from_translation(at + Vec3::Y * 0.6),
        crate::world::FireLight { base, seed },
    ));
    // Warm glow on the snow, following the ground.
    let r = 3.2 * scale;
    let mut m = crate::sim::meshgen::MeshData::default();
    let n = 6;
    for i in 0..=n {
        for j in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let (x, z) = (at.x + (u * 2.0 - 1.0) * r, at.z + (v * 2.0 - 1.0) * r);
            m.vertex([x, terrain::mesh_height(x, z).max(terrain::walk_height(x, z)) + 0.03, z], [0.0, 1.0, 0.0], [u, v], crate::sim::meshgen::WHITE);
        }
    }
    let row = n as u32 + 1;
    for i in 0..n as u32 {
        for j in 0..n as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m.recompute_normals();
    if m.normals[0][1] < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
    }
    let glow = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(1.6, 0.6, 0.15, 0.5)),
        base_color_texture: Some(soft.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        depth_bias: 6.0,
        ..default()
    });
    commands.spawn((Mesh3d(meshes.add(crate::meshes::to_mesh(&m))), MeshMaterial3d(glow), NotShadowCaster, FireGlow { seed, strength: 0.5 }));
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    spawn_requested_fx,
                    run_emitters,
                    breath.run_if(alive),
                    init_flames,
                    animate_flames,
                    pulse_glows,
                    update_particles,
                    update_casings,
                ),
            );
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<GameAssets>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // (look, colour, emissive glow, additive, texture)
    let specs: [(Look, LinearRgba, bool, &Handle<Image>); 13] = [
        (Look::Snow, LinearRgba::new(0.95, 0.97, 1.0, 0.85), false, &assets.soft),
        (Look::Smoke, LinearRgba::new(0.32, 0.32, 0.33, 0.5), false, &assets.soft),
        (Look::Breath, LinearRgba::new(0.95, 0.97, 1.0, 0.35), false, &assets.soft),
        (Look::Ember, LinearRgba::new(6.0, 2.0, 0.3, 1.0), true, &assets.soft),
        (Look::Fur, LinearRgba::new(0.8, 0.85, 0.92, 0.95), false, &assets.soft),
        (Look::Blood, LinearRgba::new(0.3, 0.01, 0.01, 0.95), false, &assets.soft),
        (Look::Mote, LinearRgba::new(0.6, 4.0, 0.8, 1.0), true, &assets.soft),
        (Look::Ice, LinearRgba::new(1.0, 3.0, 2.2, 1.0), true, &assets.soft),
        (Look::Flash, LinearRgba::new(9.0, 6.0, 2.5, 1.0), true, &assets.flash),
        (Look::Flame, LinearRgba::new(5.0, 1.8, 0.35, 1.0), true, &assets.flash),
        (Look::WolfBreath, LinearRgba::new(0.4, 1.6, 2.0, 0.55), true, &assets.soft),
        (Look::Spark, LinearRgba::new(9.0, 4.0, 0.9, 1.0), true, &assets.soft),
        (Look::FireSmoke, LinearRgba::new(0.13, 0.12, 0.11, 0.55), false, &assets.soft),
    ];
    let mut looks = HashMap::new();
    for (look, color, additive, tex) in specs {
        let steps = (0..FADE_STEPS)
            .map(|k| {
                let fade = 1.0 - k as f32 / FADE_STEPS as f32;
                let mut c = color;
                c.alpha *= fade;
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(c),
                    base_color_texture: Some(tex.clone()),
                    unlit: true,
                    alpha_mode: if additive { AlphaMode::Add } else { AlphaMode::Blend },
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .collect();
        looks.insert(look, steps);
    }
    // The flame flipbook: one material per frame, each showing its slice of the sheet.
    let sheet = server.load::<Image>("textures/generated/flame_sheet.png");
    let frames = (0..FLAME_FRAMES)
        .map(|k| {
            materials.add(StandardMaterial {
                base_color: Color::LinearRgba(LinearRgba::new(3.4, 2.5, 1.9, 1.0)),
                base_color_texture: Some(sheet.clone()),
                uv_transform: bevy::math::Affine2::from_scale_angle_translation(Vec2::new(1.0 / FLAME_FRAMES as f32, 1.0), 0.0, Vec2::new(k as f32 / FLAME_FRAMES as f32, 0.0)),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .collect();
    // A quad standing on its bottom edge, so flames grow upwards from the fire.
    let mut flame_quad = Mesh::from(Rectangle::new(1.0, 1.0));
    flame_quad.translate_by(Vec3::Y * 0.5);
    commands.insert_resource(FlameSheet { quad: meshes.add(flame_quad), frames });
    commands.insert_resource(ParticleAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        casing: meshes.add(Cylinder::new(0.009, 0.045)),
        casing_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.62, 0.25),
            metallic: 1.0,
            perceptual_roughness: 0.3,
            ..default()
        }),
        stain: meshes.add(Circle::new(1.0)),
        stain_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.02, 0.02, 0.85),
            base_color_texture: Some(assets.soft.clone()),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.6,
            depth_bias: 20.0,
            ..default()
        }),
        looks,
    });
}

#[derive(Clone, Copy)]
struct Spec {
    look: Look,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    size: (f32, f32),
    gravity: f32,
    drag: f32,
    windy: bool,
    stains: bool,
}

impl Spec {
    fn new(look: Look, pos: Vec3, vel: Vec3, life: f32, size: (f32, f32)) -> Self {
        Spec {
            look,
            pos,
            vel,
            life,
            size,
            gravity: 0.0,
            drag: 0.0,
            windy: false,
            stains: false,
        }
    }
    fn gravity(mut self, g: f32) -> Self {
        self.gravity = g;
        self
    }
    fn drag(mut self, d: f32) -> Self {
        self.drag = d;
        self
    }
    fn windy(mut self) -> Self {
        self.windy = true;
        self
    }
    fn stains(mut self) -> Self {
        self.stains = true;
        self
    }
}

fn emit(commands: &mut Commands, pa: &ParticleAssets, s: Spec) {
    commands.spawn((
        Mesh3d(pa.quad.clone()),
        MeshMaterial3d(pa.looks[&s.look][0].clone()),
        Transform::from_translation(s.pos).with_scale(Vec3::splat(s.size.0)),
        NotShadowCaster,
        Particle {
            look: s.look,
            vel: s.vel,
            age: 0.0,
            life: s.life,
            size: s.size,
            gravity: s.gravity,
            drag: s.drag,
            windy: s.windy,
            step: 0,
            stains: s.stains,
        },
    ));
}

fn rand_dir(rng: &mut RngRes) -> Vec3 {
    Vec3::new(rng.0.range(-1.0, 1.0), rng.0.range(-1.0, 1.0), rng.0.range(-1.0, 1.0)).normalize_or_zero()
}

fn spawn_requested_fx(
    mut commands: Commands,
    pa: Res<ParticleAssets>,
    mut queue: ResMut<FxQueue>,
    mut rng: ResMut<RngRes>,
) {
    for fx in queue.0.drain(..) {
        match fx {
            Fx::Footstep(p) => {
                for _ in 0..3 {
                    let v = Vec3::new(rng.0.range(-0.6, 0.6), rng.0.range(0.3, 0.7), rng.0.range(-0.6, 0.6));
                    emit(&mut commands, &pa, Spec::new(Look::Snow, p + Vec3::Y * 0.05, v, rng.0.range(0.5, 0.8), (0.1, 0.4)).gravity(1.0).drag(2.0));
                }
            }
            Fx::Muzzle(p, dir, right, brass) => {
                emit(&mut commands, &pa, Spec::new(Look::Flash, p, dir * 2.0, 0.06, (0.35, 0.5)));
                for k in 0..3 {
                    let v = dir * (1.0 + k as f32 * 0.6) + Vec3::Y * 0.3 + rand_dir(&mut rng) * 0.2;
                    emit(&mut commands, &pa, Spec::new(Look::Smoke, p + dir * 0.05 * k as f32, v, rng.0.range(0.8, 1.3), (0.08, 0.7)).drag(2.5).windy());
                }
                if brass {
                    // Brass flies out of the ejection port to the right.
                    let port = p - dir * 0.5 + Vec3::Y * 0.03;
                    commands.spawn((
                        Mesh3d(pa.casing.clone()),
                        MeshMaterial3d(pa.casing_mat.clone()),
                        Transform::from_translation(port),
                        NotShadowCaster,
                        Casing {
                            vel: right * rng.0.range(2.0, 3.0) + Vec3::Y * rng.0.range(1.2, 2.0) - dir * 0.5,
                            spin: Vec3::new(rng.0.range(8.0, 20.0), rng.0.range(-5.0, 5.0), rng.0.range(8.0, 20.0)),
                            bounced: false,
                        },
                        Lifetime(12.0),
                    ));

                }
            }
            Fx::Brass(p, count) => {
                for _ in 0..count {
                    commands.spawn((
                        Mesh3d(pa.casing.clone()),
                        MeshMaterial3d(pa.casing_mat.clone()),
                        Transform::from_translation(p + rand_dir(&mut rng) * 0.03),
                        NotShadowCaster,
                        Casing {
                            vel: Vec3::new(rng.0.range(-0.5, 0.5), rng.0.range(-0.2, 0.4), rng.0.range(-0.5, 0.5)),
                            spin: Vec3::new(rng.0.range(6.0, 16.0), rng.0.range(-5.0, 5.0), rng.0.range(6.0, 16.0)),
                            bounced: false,
                        },
                        Lifetime(12.0),
                    ));
                }
            }
            Fx::WolfHit(p, dir) => {
                for _ in 0..8 {
                    let v = dir * rng.0.range(0.5, 2.5) + rand_dir(&mut rng) * 1.5 + Vec3::Y;
                    emit(&mut commands, &pa, Spec::new(Look::Fur, p, v, rng.0.range(0.7, 1.4), (0.08, 0.03)).gravity(2.0).drag(1.5).windy());
                }
                for _ in 0..7 {
                    let v = dir * rng.0.range(1.0, 3.0) + rand_dir(&mut rng) * 1.2 + Vec3::Y * 1.5;
                    emit(&mut commands, &pa, Spec::new(Look::Blood, p, v, 1.5, (0.06, 0.04)).gravity(9.0).stains());
                }
            }
            Fx::WolfDeath(p, size) => {
                for _ in 0..24 {
                    let v = rand_dir(&mut rng) * rng.0.range(1.0, 3.0) + Vec3::Y * 1.5;
                    emit(&mut commands, &pa, Spec::new(Look::Fur, p, v, rng.0.range(1.0, 2.0), (0.12 * size, 0.04)).gravity(1.5).drag(1.2).windy());
                }
                for _ in 0..10 {
                    let v = Vec3::new(rng.0.range(-1.5, 1.5), rng.0.range(0.5, 1.5), rng.0.range(-1.5, 1.5));
                    let at = Vec3::new(p.x, terrain::walk_height(p.x, p.z) + 0.1, p.z);
                    emit(&mut commands, &pa, Spec::new(Look::Snow, at, v, 1.0, (0.2, 0.9)).gravity(1.0).drag(2.0));
                }
                for _ in 0..8 {
                    let v = rand_dir(&mut rng) * 1.5 + Vec3::Y * 2.0;
                    emit(&mut commands, &pa, Spec::new(Look::Blood, p, v, 1.5, (0.07, 0.05)).gravity(9.0).stains());
                }
            }
            Fx::WolfBreath(p, dir) => {
                for k in 0..2 {
                    let v = dir * (0.6 + 0.3 * k as f32) + Vec3::Y * 0.25 + rand_dir(&mut rng) * 0.1;
                    emit(&mut commands, &pa, Spec::new(Look::WolfBreath, p, v, rng.0.range(0.9, 1.5), (0.05, 0.4)).drag(1.4).windy());
                }
            }
            Fx::Ricochet(p) => {
                for _ in 0..5 {
                    let v = Vec3::new(rng.0.range(-1.0, 1.0), rng.0.range(1.0, 2.5), rng.0.range(-1.0, 1.0));
                    emit(&mut commands, &pa, Spec::new(Look::Snow, p, v, 0.7, (0.08, 0.35)).gravity(4.0).drag(1.5));
                }
            }
            Fx::IceBreak(p) => {
                for _ in 0..30 {
                    let v = Vec3::new(rng.0.range(-3.0, 3.0), rng.0.range(3.0, 7.0), rng.0.range(-3.0, 3.0));
                    emit(&mut commands, &pa, Spec::new(Look::Ice, p, v, rng.0.range(0.8, 1.6), (0.12, 0.05)).gravity(9.0));
                }
                for _ in 0..16 {
                    let v = Vec3::new(rng.0.range(-2.0, 2.0), rng.0.range(1.0, 3.0), rng.0.range(-2.0, 2.0));
                    emit(&mut commands, &pa, Spec::new(Look::Snow, p, v, 1.2, (0.3, 1.2)).gravity(2.0).drag(1.5));
                }
            }
        }
    }
}

fn run_emitters(
    mut commands: Commands,
    time: Res<Time>,
    pa: Res<ParticleAssets>,
    mut rng: ResMut<RngRes>,
    cam: Query<&GlobalTransform, With<Player>>,
    mut emitters: Query<(&GlobalTransform, &mut Emitter)>,
) {
    let Ok(cam) = cam.single() else { return };
    let dt = time.delta_secs();
    for (gt, mut e) in &mut emitters {
        let p = gt.translation();
        if p.distance(cam.translation()) > EMIT_RANGE {
            continue;
        }
        e.timer -= dt;
        while e.timer <= 0.0 {
            e.timer += e.every * rng.0.range(0.6, 1.4);
            let spec = match e.kind {
                EmitterKind::ChimneySmoke => Spec::new(
                    Look::Smoke,
                    p,
                    Vec3::new(rng.0.range(-0.1, 0.1), rng.0.range(0.7, 1.0), rng.0.range(-0.1, 0.1)),
                    rng.0.range(3.5, 5.0),
                    (0.35, 1.8),
                )
                .windy(),
                EmitterKind::Embers => Spec::new(
                    Look::Ember,
                    p + Vec3::new(rng.0.range(-0.2, 0.2), 0.0, rng.0.range(-0.2, 0.2)),
                    Vec3::new(rng.0.range(-0.3, 0.3), rng.0.range(1.2, 2.4), rng.0.range(-0.3, 0.3)),
                    rng.0.range(0.8, 1.8),
                    (0.05, 0.015),
                )
                .windy(),
                EmitterKind::FireSmoke => Spec::new(
                    Look::FireSmoke,
                    p + Vec3::new(rng.0.range(-0.1, 0.1), 0.0, rng.0.range(-0.1, 0.1)),
                    Vec3::new(rng.0.range(-0.15, 0.15), rng.0.range(0.6, 1.0), rng.0.range(-0.15, 0.15)),
                    rng.0.range(3.0, 4.5),
                    (0.3, 1.6),
                )
                .windy(),
                EmitterKind::Sparks => {
                    // Now and then a log shifts and throws a shower.
                    if rng.0.chance(0.06) {
                        for _ in 0..8 {
                            let v = Vec3::new(rng.0.range(-1.2, 1.2), rng.0.range(2.5, 5.0), rng.0.range(-1.2, 1.2));
                            emit(&mut commands, &pa, Spec::new(Look::Spark, p, v, rng.0.range(0.5, 1.1), (0.04, 0.008)).gravity(-2.0).drag(0.6).windy());
                        }
                    }
                    Spec::new(
                        Look::Spark,
                        p + Vec3::new(rng.0.range(-0.15, 0.15), 0.0, rng.0.range(-0.15, 0.15)),
                        Vec3::new(rng.0.range(-0.4, 0.4), rng.0.range(2.0, 3.5), rng.0.range(-0.4, 0.4)),
                        rng.0.range(0.6, 1.4),
                        (0.035, 0.008),
                    )
                    .drag(0.8)
                    .windy()
                }
                EmitterKind::RadMotes => {
                    let a = rng.0.range(0.0, std::f32::consts::TAU);
                    let r = rng.0.range(0.0, 12.0);
                    let (x, z) = (p.x + a.cos() * r, p.z + a.sin() * r);
                    Spec::new(
                        Look::Mote,
                        Vec3::new(x, terrain::walk_height(x, z) + rng.0.range(0.1, 1.5), z),
                        Vec3::new(rng.0.range(-0.15, 0.15), rng.0.range(0.2, 0.5), rng.0.range(-0.15, 0.15)),
                        rng.0.range(2.5, 4.5),
                        (0.07, 0.02),
                    )
                }
            };
            emit(&mut commands, &pa, spec);
        }
    }
}

/// Breath vapour puffs in front of the face; quicker when sprinting.
fn breath(
    mut commands: Commands,
    time: Res<Time>,
    pa: Res<ParticleAssets>,
    mut rng: ResMut<RngRes>,
    mut timer: Local<f32>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    game: Res<Game>,
    cam: Query<(&GlobalTransform, &Player)>,
) {
    let Ok((gt, p)) = cam.single() else { return };
    let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();
    if temp > 40.0 || game.death.is_some() {
        return;
    }
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = if p.sprinting { 1.4 } else { 3.2 } + rng.0.range(-0.3, 0.3);
    let fwd = gt.forward().as_vec3();
    let mouth = gt.translation() + fwd * 0.35 - Vec3::Y * 0.15;
    for _ in 0..5 {
        let v = fwd * rng.0.range(0.4, 0.8) + Vec3::Y * 0.15 + rand_dir(&mut rng) * 0.1;
        emit(&mut commands, &pa, Spec::new(Look::Breath, mouth, v, rng.0.range(1.0, 1.6), (0.05, 0.35)).drag(1.2).windy());
    }
}

fn init_flames(mut commands: Commands, sheet: Res<FlameSheet>, flames: Query<Entity, Added<Flame>>) {
    for e in &flames {
        commands.entity(e).insert((Mesh3d(sheet.quad.clone()), MeshMaterial3d(sheet.frames[0].clone()), NotShadowCaster));
    }
}

/// Turn each tongue to face you (staying upright), run its flipbook, and
/// make it flicker and lean with the wind.
fn animate_flames(
    time: Res<Time>,
    sheet: Res<FlameSheet>,
    weather: Res<WeatherRes>,
    cam: Query<&GlobalTransform, With<Player>>,
    mut flames: Query<(&mut Transform, &mut MeshMaterial3d<StandardMaterial>, &Flame)>,
) {
    let Ok(cam) = cam.single() else { return };
    let eye = cam.translation();
    let t = time.elapsed_secs();
    let wind_dir = Vec3::new(crate::sim::weather::WIND_DIR[0], 0.0, crate::sim::weather::WIND_DIR[1]);
    let wind = (weather.weather.conditions().wind / 14.0).clamp(0.0, 1.0);
    for (mut tf, mut mat, f) in &mut flames {
        let to_eye = eye - tf.translation;
        let yaw = to_eye.x.atan2(to_eye.z);
        let right = Quat::from_rotation_y(yaw) * Vec3::X;
        let lean = -right.dot(wind_dir) * (0.12 + 0.35 * wind) + (t * 2.3 + f.seed).sin() * 0.07;
        tf.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(lean);
        let flick = fire_flicker(t * 1.3, f.seed);
        let breathe = 1.0 + 0.12 * (t * 5.1 + f.seed * 1.3).sin();
        tf.scale = Vec3::new(f.size.x * (0.9 + 0.1 * flick), f.size.y * flick * breathe, 1.0);
        let frame = ((t * 14.0 + f.seed * 3.7) as usize) % FLAME_FRAMES;
        if mat.0 != sheet.frames[frame] {
            mat.0 = sheet.frames[frame].clone();
        }
    }
}

/// The glow on the snow pulses with its fire.
fn pulse_glows(time: Res<Time>, glows: Query<(&FireGlow, &MeshMaterial3d<StandardMaterial>)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let t = time.elapsed_secs();
    for (g, mat) in &glows {
        if let Some(m) = materials.get_mut(&mat.0) {
            let k = fire_flicker(t, g.seed);
            m.base_color = Color::LinearRgba(LinearRgba::new(1.6 * k, 0.6 * k, 0.15 * k, g.strength * k));
        }
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    pa: Res<ParticleAssets>,
    weather: Res<WeatherRes>,
    cam: Query<&GlobalTransform, With<Player>>,
    mut parts: Query<(Entity, &mut Transform, &mut Particle, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Ok(cam) = cam.single() else { return };
    let rot = cam.compute_transform().rotation;
    let dt = time.delta_secs();
    let wind = weather.weather.conditions().wind;
    let wind_v = Vec3::new(wind, 0.0, wind * 0.4) * 0.35;
    for (e, mut tf, mut p, mut mat) in &mut parts {
        p.age += dt;
        if p.age >= p.life {
            commands.entity(e).despawn();
            continue;
        }
        let k = p.age / p.life;
        p.vel.y -= p.gravity * dt;
        let damp = (1.0 - p.drag * dt).max(0.0);
        p.vel *= damp;
        let drift = if p.windy { wind_v } else { Vec3::ZERO };
        tf.translation += (p.vel + drift) * dt;
        let floor = terrain::walk_height(tf.translation.x, tf.translation.z) + 0.02;
        if tf.translation.y < floor && p.vel.y < 0.0 {
            if p.stains {
                let r = 0.06 + 0.1 * (tf.translation.x * 13.0).sin().abs();
                commands.spawn((
                    Mesh3d(pa.stain.clone()),
                    MeshMaterial3d(pa.stain_mat.clone()),
                    Transform::from_xyz(tf.translation.x, floor, tf.translation.z)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::splat(r)),
                    NotShadowCaster,
                    Lifetime(45.0),
                ));
                commands.entity(e).despawn();
                continue;
            }
            tf.translation.y = floor;
            p.vel = Vec3::ZERO;
        }
        tf.rotation = rot;
        tf.scale = Vec3::splat(p.size.0 + (p.size.1 - p.size.0) * k);
        // Fade out over the last 60% of the particle's life.
        let step = (((k - 0.4) / 0.6).max(0.0) * FADE_STEPS as f32) as usize;
        let step = step.min(FADE_STEPS - 1);
        if step != p.step {
            p.step = step;
            mat.0 = pa.looks[&p.look][step].clone();
        }
    }
}

fn update_casings(time: Res<Time>, mut sfx: ResMut<SfxQueue>, mut q: Query<(&mut Transform, &mut Casing)>) {
    let dt = time.delta_secs();
    for (mut tf, mut c) in &mut q {
        if c.vel == Vec3::ZERO {
            continue;
        }
        c.vel.y -= 9.8 * dt;
        tf.translation += c.vel * dt;
        let spin = c.spin * dt;
        tf.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
        let floor = terrain::walk_height(tf.translation.x, tf.translation.z) + 0.01;
        if tf.translation.y < floor {
            tf.translation.y = floor;
            if c.bounced {
                c.vel = Vec3::ZERO;
                tf.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) * Quat::from_rotation_y(tf.translation.x);
            } else {
                c.bounced = true;
                c.vel = Vec3::new(c.vel.x * 0.3, -c.vel.y * 0.25, c.vel.z * 0.3);
                c.spin *= 0.4;
                sfx.play_at(Sound::ShellTink, tf.translation);
            }
        }
    }
}

/// Where a shot that missed every wolf hits the snow, if within `range`.
pub fn ground_hit(origin: Vec3, dir: Vec3, range: f32) -> Option<Vec3> {
    let mut t = 0.5;
    while t < range {
        let p = origin + dir * t;
        if p.y <= terrain::walk_height(p.x, p.z) {
            return Some(p);
        }
        t += 0.75;
    }
    None
}

/// Footstep position under the player's camera.
pub fn feet(cam: Vec3) -> Vec3 {
    cam - Vec3::Y * EYE_HEIGHT
}
