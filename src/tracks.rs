//! Marks in the snow: footprints you leave behind you, paw and hoof prints
//! behind wolves and moose, old animal trails crossing the map, and soft
//! contact shadows that settle trees, rocks and props into the ground.
//! Prints fade (sink into fresh snow) after a while, faster in a blizzard.

use std::collections::{HashMap, VecDeque};

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::enemy::{Body, Dying, Species};
use crate::meshes::to_mesh;
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::meshgen;
use crate::sim::rng::Rng;
use crate::sim::terrain::{self, Surface, HALF_SIZE};
use crate::sim::weather::Phase;
use crate::state::WeatherRes;

/// Most prints alive at once; the oldest go first.
const MAX_PRINTS: usize = 260;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Boot,
    Paw,
    Hoof,
}

#[derive(Resource)]
struct PrintKit {
    boot: (Handle<Mesh>, Handle<StandardMaterial>),
    paw: (Handle<Mesh>, Handle<StandardMaterial>),
    hoof: (Handle<Mesh>, Handle<StandardMaterial>),
}

impl PrintKit {
    fn get(&self, kind: Kind) -> &(Handle<Mesh>, Handle<StandardMaterial>) {
        match kind {
            Kind::Boot => &self.boot,
            Kind::Paw => &self.paw,
            Kind::Hoof => &self.hoof,
        }
    }
}

/// A fresh print: seconds left before it's snowed over.
#[derive(Component)]
struct Print {
    life: f32,
    base_y: f32,
}

#[derive(Resource, Default)]
struct Prints(VecDeque<Entity>);

pub struct TracksPlugin;

impl Plugin for TracksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Prints>()
            .add_systems(Startup, (load_kit, old_trails).chain())
            .add_systems(Update, (player_prints, creature_prints, weather_prints));
    }
}

fn load_kit(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut kit = |file: &str, w: f32, l: f32| {
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(server.load(format!("textures/generated/{file}"))),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            depth_bias: 2.0,
            ..default()
        });
        (meshes.add(Plane3d::new(Vec3::Y, Vec2::new(w * 0.5, l * 0.5))), mat)
    };
    let kit = PrintKit {
        boot: kit("print_boot.png", 0.13, 0.3),
        paw: kit("print_paw.png", 0.11, 0.11),
        hoof: kit("print_hoof.png", 0.13, 0.15),
    };
    commands.insert_resource(kit);
}

/// Place one print flat on the visible snow, facing `yaw`.
fn place(commands: &mut Commands, kit: &PrintKit, prints: &mut Prints, kind: Kind, x: f32, z: f32, yaw: f32, life: f32) {
    let y = terrain::mesh_height(x, z) + 0.012;
    let n = Vec3::from_array(terrain::normal(x, z));
    let tilt = Quat::from_rotation_arc(Vec3::Y, n);
    let (mesh, mat) = kit.get(kind);
    let e = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(x, y, z).with_rotation(tilt * Quat::from_rotation_y(yaw)),
            NotShadowCaster,
            Print { life, base_y: y },
        ))
        .id();
    prints.0.push_back(e);
    while prints.0.len() > MAX_PRINTS {
        if let Some(old) = prints.0.pop_front() {
            commands.entity(old).despawn();
        }
    }
}

fn snowy(x: f32, z: f32) -> bool {
    terrain::surface_at(x, z) == Surface::Snow
}

/// Your boots: one print per stride, left and right.
fn player_prints(
    mut commands: Commands,
    kit: Option<Res<PrintKit>>,
    mut prints: ResMut<Prints>,
    player: Query<(&Transform, &Player)>,
    mut last: Local<Option<(Vec2, bool)>>,
) {
    let Some(kit) = kit else { return };
    let Ok((tf, p)) = player.single() else { return };
    let feet = tf.translation - Vec3::Y * EYE_HEIGHT;
    let here = Vec2::new(feet.x, feet.z);
    let Some((prev, left)) = *last else {
        *last = Some((here, false));
        return;
    };
    let step = here - prev;
    if !p.grounded || step.length() < 0.72 {
        if step.length() > 3.0 {
            *last = Some((here, left)); // teleported
        }
        return;
    }
    let dir = step.normalize();
    let side = Vec2::new(-dir.y, dir.x) * if left { 0.13 } else { -0.13 };
    let at = here + side;
    if snowy(at.x, at.y) {
        let yaw = (-dir.x).atan2(-dir.y);
        place(&mut commands, &kit, &mut prints, Kind::Boot, at.x, at.y, yaw, 150.0);
    }
    *last = Some((here, !left));
}

/// Wolves leave paw prints and moose leave hoof prints as they move.
fn creature_prints(
    mut commands: Commands,
    kit: Option<Res<PrintKit>>,
    mut prints: ResMut<Prints>,
    creatures: Query<(Entity, &Transform, &Body), Without<Dying>>,
    mut last: Local<HashMap<Entity, (Vec2, bool)>>,
) {
    let Some(kit) = kit else { return };
    let mut seen = Vec::new();
    for (e, tf, body) in &creatures {
        seen.push(e);
        let here = Vec2::new(tf.translation.x, tf.translation.z);
        let (kind, stride, gauge) = match body.species {
            Species::Wolf { .. } => (Kind::Paw, 0.9, 0.09),
            Species::Moose => (Kind::Hoof, 1.4, 0.2),
            Species::Raider => (Kind::Boot, 0.8, 0.13),
            // Birds fly.
            Species::Crow => continue,
        };
        let entry = last.entry(e).or_insert((here, false));
        let step = here - entry.0;
        if step.length() < stride {
            continue;
        }
        if step.length() > stride * 4.0 {
            *entry = (here, entry.1);
            continue;
        }
        let dir = step.normalize();
        let side = Vec2::new(-dir.y, dir.x) * if entry.1 { gauge } else { -gauge };
        let at = here + side;
        if snowy(at.x, at.y) {
            place(&mut commands, &kit, &mut prints, kind, at.x, at.y, (-dir.x).atan2(-dir.y), 120.0);
        }
        *entry = (here, !entry.1);
    }
    last.retain(|e, _| seen.contains(e));
}

/// Fresh snow fills prints in: they sink out of sight as they expire, much
/// sooner in a rad-blizzard.
fn weather_prints(
    mut commands: Commands,
    time: Res<Time>,
    weather: Res<WeatherRes>,
    mut prints: ResMut<Prints>,
    mut q: Query<(Entity, &mut Print, &mut Transform)>,
) {
    let rate = match weather.weather.phase {
        Phase::Blizzard => 6.0,
        Phase::Warning => 2.0,
        Phase::Calm => 1.0,
    };
    let dt = time.delta_secs() * rate;
    for (e, mut p, mut tf) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            prints.0.retain(|&x| x != e);
        } else if p.life < 8.0 {
            tf.translation.y = p.base_y - (8.0 - p.life) / 8.0 * 0.05;
        }
    }
}

/// Old wolf and moose trails wandering across open snow. These never fade.
fn old_trails(mut commands: Commands, kit: Res<PrintKit>, mut meshes: ResMut<Assets<Mesh>>) {
    let mut rng = Rng::new(1143);
    for t in 0..14 {
        let kind = if t % 5 == 4 { Kind::Hoof } else { Kind::Paw };
        let (stride, gauge, w, l) = if kind == Kind::Hoof { (1.4, 0.2, 0.13, 0.15) } else { (0.85, 0.09, 0.11, 0.11) };
        // Merge each trail into one mesh so hundreds of prints cost one draw.
        let mut m = meshgen::MeshData::default();
        let mut p = Vec2::new(rng.range(-HALF_SIZE + 20.0, HALF_SIZE - 20.0), rng.range(-HALF_SIZE + 20.0, HALF_SIZE - 20.0));
        let mut heading = rng.range(0.0, std::f32::consts::TAU);
        let mut left = false;
        for _ in 0..rng.range(30.0, 70.0) as usize {
            heading += rng.range(-0.25, 0.25);
            let dir = Vec2::new(heading.cos(), heading.sin());
            p += dir * stride;
            let side = Vec2::new(-dir.y, dir.x) * if left { gauge } else { -gauge };
            left = !left;
            let at = p + side;
            if !snowy(at.x, at.y) || at.x.abs() > HALF_SIZE - 5.0 || at.y.abs() > HALF_SIZE - 5.0 {
                continue;
            }
            let yaw = (-dir.x).atan2(-dir.y);
            // One flat quad per print, following the snow.
            let mut top = meshgen::MeshData::default();
            for (u, v) in [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
                let lx = (u - 0.5) * w;
                let lz = (0.5 - v) * l;
                let (s, c) = yaw.sin_cos();
                let (x, z) = (at.x + lx * c + lz * s, at.y - lx * s + lz * c);
                top.vertex([x, terrain::mesh_height(x, z) + 0.012, z], [0.0, 1.0, 0.0], [u, v], meshgen::WHITE);
            }
            top.quad(0, 1, 2, 3);
            if top.outwardness([at.x, -100.0, at.y]) < 0.0 {
                top.indices = vec![0, 2, 1, 0, 3, 2];
            }
            m.append(&top);
        }
        if m.positions.is_empty() {
            continue;
        }
        let (_, mat) = kit.get(kind);
        commands.spawn((Mesh3d(meshes.add(to_mesh(&m))), MeshMaterial3d(mat.clone()), Transform::default(), NotShadowCaster));
    }
}
