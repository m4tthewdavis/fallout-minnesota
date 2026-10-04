//! The sky: a gradient dome that follows the day/night cycle and the weather,
//! a slowly wheeling starfield, the northern lights on clear nights, and
//! sun and moon discs. Everything here ignores fog and follows the camera.

use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;

use crate::assets::GameAssets;
use crate::meshes::to_mesh;
use crate::player::Player;
use crate::sim::meshgen::MeshData;
use crate::sim::weather::Phase;
use crate::state::{ClockRes, WeatherRes};

const DOME_RADIUS: f32 = 900.0;

#[derive(Component)]
struct SkyDome;
#[derive(Component)]
struct Stars;
#[derive(Component)]
struct Aurora;
#[derive(Component)]
struct SunDisc;
#[derive(Component)]
struct MoonDisc;

/// Everything that should stay centred on the camera.
#[derive(Component)]
struct FollowCamera;

#[derive(Resource)]
struct SkyHandles {
    dome: Handle<Mesh>,
    stars: Handle<StandardMaterial>,
    aurora: Vec<Handle<StandardMaterial>>,
    sun: Handle<StandardMaterial>,
    moon: Handle<StandardMaterial>,
}

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_sky)
            .add_systems(PostUpdate, follow_camera.before(bevy::transform::TransformSystem::TransformPropagate))
            .add_systems(Update, update_sky);
    }
}

fn sky_material(materials: &mut Assets<StandardMaterial>, texture: Option<Handle<Image>>, alpha: AlphaMode) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: texture,
        unlit: true,
        fog_enabled: false,
        alpha_mode: alpha,
        cull_mode: None,
        double_sided: true,
        ..default()
    })
}

/// An upright curtain along an arc, `radius` from the centre (aurora ribbon).
fn curtain(radius: f32, from: f32, to: f32, bottom: f32, top: f32, seed: f32) -> MeshData {
    let mut m = MeshData::default();
    let n = 48;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = from + (to - from) * t;
        let r = radius + 40.0 * (t * 9.0 + seed).sin();
        let fold = 25.0 * (t * 23.0 + seed * 2.0).sin();
        let (x, z) = ((r + fold) * a.cos(), -(r + fold) * a.sin());
        let lift = 20.0 * (t * 5.0 + seed).sin();
        let n_out = [a.cos(), 0.0, -a.sin()];
        m.vertex([x, bottom + lift, z], n_out, [t * 6.0, 1.0], [1.0; 4]);
        m.vertex([x, top + lift * 1.5, z], n_out, [t * 6.0, 0.0], [1.0; 4]);
    }
    for i in 0..n as u32 {
        let (a, b, c, d) = (i * 2, i * 2 + 2, i * 2 + 3, i * 2 + 1);
        m.quad(a, b, c, d);
    }
    m
}

fn spawn_sky(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Gradient dome: vertex colours are rewritten every frame.
    let mut dome_mesh: Mesh = Sphere::new(DOME_RADIUS).mesh().uv(48, 24);
    let count = dome_mesh.count_vertices();
    dome_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.6f32, 0.65, 0.7, 1.0]; count]);
    let dome = meshes.add(dome_mesh);
    let dome_mat = sky_material(&mut materials, None, AlphaMode::Opaque);
    commands.spawn((
        Mesh3d(dome.clone()),
        MeshMaterial3d(dome_mat),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
        SkyDome,
        FollowCamera,
    ));

    let stars = sky_material(&mut materials, Some(assets.stars.clone()), AlphaMode::Add);
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(DOME_RADIUS * 0.97).mesh().uv(48, 24))),
        MeshMaterial3d(stars.clone()),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
        Stars,
        FollowCamera,
    ));

    // Three aurora ribbons hanging in the northern sky (-Z is north).
    let mut aurora = Vec::new();
    for (i, (from, to, bottom, top)) in [(1.2f32, 2.2, 150.0, 380.0), (1.5, 2.6, 200.0, 420.0), (0.8, 1.6, 170.0, 330.0)]
        .into_iter()
        .enumerate()
    {
        let mut material = sky_material(&mut materials, Some(assets.aurora.clone()), AlphaMode::Add);
        if i == 2 {
            // The third ribbon is a fainter violet-tinged one.
            material = materials.add(StandardMaterial {
                base_color: Color::srgba(0.8, 0.6, 1.0, 1.0),
                base_color_texture: Some(assets.aurora.clone()),
                unlit: true,
                fog_enabled: false,
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                double_sided: true,
                ..default()
            });
        }
        aurora.push(material.clone());
        commands.spawn((
            Mesh3d(meshes.add(to_mesh(&curtain(650.0, from, to, bottom, top, i as f32 * 2.3)))),
            MeshMaterial3d(material),
            Transform::default(),
            NotShadowCaster,
            NotShadowReceiver,
            Aurora,
            FollowCamera,
        ));
    }

    let sun = sky_material(&mut materials, Some(assets.soft.clone()), AlphaMode::Add);
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(90.0, 90.0))),
        MeshMaterial3d(sun.clone()),
        Transform::default(),
        NotShadowCaster,
        SunDisc,
        FollowCamera,
    ));
    let moon = sky_material(&mut materials, None, AlphaMode::Blend);
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(14.0))),
        MeshMaterial3d(moon.clone()),
        Transform::default(),
        NotShadowCaster,
        MoonDisc,
        FollowCamera,
    ));

    commands.insert_resource(SkyHandles {
        dome,
        stars,
        aurora,
        sun,
        moon,
    });
}

fn follow_camera(cam: Query<&Transform, With<Player>>, mut q: Query<&mut Transform, (With<FollowCamera>, Without<Player>, Without<SunDisc>, Without<MoonDisc>)>) {
    let Ok(cam) = cam.single() else { return };
    for mut tf in &mut q {
        tf.translation = cam.translation;
    }
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn update_sky(
    time: Res<Time>,
    clock: Res<ClockRes>,
    weather: Res<WeatherRes>,
    clear: Res<ClearColor>,
    handles: Option<Res<SkyHandles>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam: Query<&Transform, With<Player>>,
    mut stars: Query<&mut Transform, (With<Stars>, Without<Player>)>,
    mut discs: ParamSet<(
        Query<&mut Transform, (With<SunDisc>, Without<Player>, Without<Stars>)>,
        Query<&mut Transform, (With<MoonDisc>, Without<Player>, Without<Stars>)>,
    )>,
    mut overcast_smooth: Local<f32>,
) {
    let Some(h) = handles else { return };
    let Ok(cam) = cam.single() else { return };
    let sky = clock.0.sky();
    let day = ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0);
    let cond = weather.weather.conditions();
    // 0 = clear, 1 = socked in by the storm.
    let overcast_target = ((cond.fog_density - 0.01) / 0.03).clamp(0.0, 1.0);
    let k = (time.delta_secs() * 0.5).min(1.0);
    *overcast_smooth += (overcast_target - *overcast_smooth) * k;
    let overcast = *overcast_smooth;
    let sick = if weather.weather.phase == Phase::Blizzard { overcast } else { 0.0 };

    // The horizon matches the fog colour so distant terrain melts into the sky.
    let horizon = clear.0.to_srgba();
    let horizon = Vec3::new(horizon.red, horizon.green, horizon.blue);
    let day_zenith = Vec3::new(0.30, 0.45, 0.72);
    let night_zenith = Vec3::new(0.005, 0.01, 0.03);
    let zenith = lerp3(lerp3(night_zenith, day_zenith, day), horizon, overcast * 0.85);
    let zenith = lerp3(zenith, Vec3::new(0.25, 0.4, 0.2), sick * 0.3);
    let sun_dir = Vec3::from_array(sky.sun_pos).normalize_or_zero();
    let glow = Vec3::new(1.0, 0.55, 0.3);

    if let Some(mesh) = meshes.get_mut(&h.dome) {
        let positions: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => Vec::new(),
        };
        let colors: Vec<[f32; 4]> = positions
            .iter()
            .map(|p| {
                let d = Vec3::from_array(*p) / DOME_RADIUS;
                let up = d.y.max(0.0);
                let t = up.powf(0.45);
                let mut c = lerp3(horizon, zenith, t);
                // Warm halo around the low sun at dawn and dusk.
                let near_sun = d.dot(sun_dir).max(0.0).powf(6.0);
                c += glow * near_sun * sky.warmth * (1.0 - overcast) * 0.6;
                if d.y < 0.0 {
                    c = horizon;
                }
                [c.x, c.y, c.z, 1.0]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }

    // Stars and aurora only on clear nights.
    let night = 1.0 - day;
    let clear_night = night * (1.0 - overcast);
    if let Some(m) = materials.get_mut(&h.stars) {
        m.base_color = Color::srgba(1.0, 1.0, 1.0, clear_night);
    }
    let t = time.elapsed_secs();
    for (i, handle) in h.aurora.iter().enumerate() {
        if let Some(m) = materials.get_mut(handle) {
            let wave = 0.55 + 0.45 * (t * 0.13 + i as f32 * 1.7).sin();
            let base = m.base_color.to_srgba();
            m.base_color = Color::srgba(base.red, base.green, base.blue, clear_night * wave * 0.9);
            m.uv_transform = bevy::math::Affine2::from_translation(Vec2::new(t * 0.01 * (i as f32 + 1.0), 0.0));
        }
    }
    for mut tf in &mut stars {
        tf.rotation = Quat::from_rotation_y(t * 0.002) * Quat::from_rotation_x(0.35);
    }

    // Sun and moon discs, facing the camera.
    let sun_alpha = (sky.sun * 1.5).min(1.0) * (1.0 - overcast * 0.9);
    if let Some(m) = materials.get_mut(&h.sun) {
        let warm = lerp3(Vec3::new(4.0, 3.8, 3.3), Vec3::new(5.0, 2.2, 0.8), sky.warmth);
        m.base_color = Color::LinearRgba(LinearRgba::new(warm.x, warm.y, warm.z, sun_alpha));
    }
    for mut tf in &mut discs.p0() {
        let p = cam.translation + sun_dir * DOME_RADIUS * 0.9;
        *tf = Transform::from_translation(p).looking_at(cam.translation, Vec3::Y);
    }
    let moon_dir = Vec3::new(-0.3, 0.8, -0.5).normalize();
    let moon_alpha = (night * 1.2).min(1.0) * (1.0 - overcast * 0.85);
    if let Some(m) = materials.get_mut(&h.moon) {
        m.base_color = Color::LinearRgba(LinearRgba::new(2.2, 2.3, 2.6, moon_alpha));
    }
    for mut tf in &mut discs.p1() {
        let p = cam.translation + moon_dir * DOME_RADIUS * 0.9;
        *tf = Transform::from_translation(p).looking_at(cam.translation, Vec3::Y);
    }
}
