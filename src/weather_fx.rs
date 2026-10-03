//! Drives the weather cycle, the day/night clock and their visuals: fog, a
//! moving sun and moon, ambient light, and wind-blown snow that turns into a
//! sickly green whiteout during rad-blizzards.

use bevy::pbr::{DistanceFog, FogFalloff, NotShadowCaster};
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::player::Player;
use crate::sim::weather::{Phase, Weather};
use crate::state::{ClockRes, Messages, RngRes, WeatherRes};
use crate::world::Sun;

const FLAKES: usize = 900;
const BOX_HALF: f32 = 25.0;

#[derive(Component)]
struct Flake {
    index: usize,
    fall: f32,
}

/// The snowflake material, tinted green during rad-blizzards.
#[derive(Resource, Default)]
struct FlakeMaterial(Handle<StandardMaterial>);

/// Smoothed visual weather values so changes fade in instead of popping.
#[derive(Resource)]
struct VisualWeather {
    fog: f32,
    snow: f32,
    wind: f32,
    light: f32,
    /// 0 = clean white fog, 1 = radioactive green.
    sick: f32,
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        let calm = Weather::conditions_for(Phase::Calm);
        app.insert_resource(VisualWeather {
            fog: calm.fog_density,
            snow: calm.snow,
            wind: calm.wind,
            light: calm.light,
            sick: 0.0,
        })
        .add_systems(Startup, spawn_flakes)
        .init_resource::<FlakeMaterial>()
        .add_systems(
            Update,
            (update_weather, smooth_visuals, apply_atmosphere, move_flakes).chain(),
        );
    }
}

fn spawn_flakes(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
    mut flake_mat: ResMut<FlakeMaterial>,
) {
    // Soft round flakes; they turn to face the camera as they fall.
    let mesh = meshes.add(Rectangle::new(0.11, 0.11));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.97, 1.0, 0.9),
        base_color_texture: Some(assets.soft.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    flake_mat.0 = material.clone();
    let (sx, sz) = crate::sim::terrain::PLAYER_SPAWN;
    for index in 0..FLAKES {
        let pos = Vec3::new(
            sx + rng.0.range(-BOX_HALF, BOX_HALF),
            rng.0.range(-4.0, 18.0),
            sz + rng.0.range(-BOX_HALF, BOX_HALF),
        );
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos),
            Visibility::Visible,
            NotShadowCaster,
            Flake {
                index,
                fall: rng.0.range(2.0, 4.0),
            },
        ));
    }
}

fn update_weather(
    time: Res<Time>,
    mut weather: ResMut<WeatherRes>,
    mut clock: ResMut<ClockRes>,
    mut rng: ResMut<RngRes>,
    mut msgs: ResMut<Messages>,
    mut was_night: Local<bool>,
) {
    clock.0.advance(time.delta_secs());
    let night = clock.0.is_night();
    if night != *was_night {
        *was_night = night;
        if night {
            msgs.show("Night falls over Mille Lacs. The cold bites harder.", 4.0);
        } else {
            msgs.show("Dawn. A pale sun rises over the ice.", 3.5);
        }
    }

    let changed = weather.weather.update(time.delta_secs(), &mut rng.0);
    weather.just_changed = changed;
    match changed {
        Some(Phase::Warning) => msgs.show(
            "SIREN: An Alberta Clipper is rolling in from the northwest. Rad-blizzard in 20 seconds - find shelter!",
            6.0,
        ),
        Some(Phase::Blizzard) => msgs.show(
            "RAD-BLIZZARD! Radiation and wind chill rising. Frostfang packs are on the hunt.",
            5.0,
        ),
        Some(Phase::Calm) => msgs.show("The blizzard passes. The snow stops glowing... mostly.", 4.0),
        None => {}
    }
}

fn smooth_visuals(time: Res<Time>, weather: Res<WeatherRes>, mut vis: ResMut<VisualWeather>) {
    let target = weather.weather.conditions();
    let k = (time.delta_secs() * 0.6).min(1.0);
    let sick_target = if weather.weather.phase == Phase::Blizzard {
        1.0
    } else {
        0.0
    };
    vis.fog += (target.fog_density - vis.fog) * k;
    vis.snow += (target.snow - vis.snow) * k;
    vis.wind += (target.wind - vis.wind) * k;
    vis.light += (target.light - vis.light) * k;
    vis.sick += (sick_target - vis.sick) * k;
}

fn apply_atmosphere(
    vis: Res<VisualWeather>,
    clock: Res<ClockRes>,
    mut fog: Query<&mut DistanceFog>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut ambient: ResMut<AmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let sky = clock.0.sky();
    // 0 at deepest night, 1 in full day.
    let day = ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0);

    // Sky / fog colour: weather tint, darkened towards a deep-blue night,
    // and warmed at sunrise and sunset.
    let white = Vec3::new(0.70, 0.74, 0.78);
    let green = Vec3::new(0.45, 0.62, 0.42);
    let weather_tint = white.lerp(green, vis.sick) * (0.55 + 0.45 * vis.light);
    let night_tint = Vec3::new(0.04, 0.05, 0.09) + green * 0.08 * vis.sick;
    let mut c = night_tint.lerp(weather_tint, day);
    c = c.lerp(
        Vec3::new(0.85, 0.55, 0.42) * (0.5 + 0.5 * vis.light),
        sky.warmth * 0.35 * day.max(0.3),
    );
    let color = Color::srgb(c.x, c.y, c.z);
    for mut f in &mut fog {
        f.color = color;
        f.falloff = FogFalloff::Exponential { density: vis.fog };
    }
    clear.0 = color;

    // Sun by day, moon by night.
    let (dir, strength, light_color) = if sky.sun >= sky.moon * 0.3 {
        let warm = Vec3::new(1.0, 0.62, 0.38);
        let noon = Vec3::new(0.95, 0.96, 1.0);
        let lc = noon.lerp(warm, sky.warmth);
        (Vec3::from_array(sky.sun_pos), 6_000.0 * sky.sun, lc)
    } else {
        (
            Vec3::new(-0.3, 0.8, -0.5).normalize(),
            350.0 * sky.moon,
            Vec3::new(0.6, 0.7, 1.0),
        )
    };
    for (mut light, mut tf) in &mut sun {
        light.illuminance = strength * vis.light;
        light.color = Color::srgb(light_color.x, light_color.y, light_color.z);
        *tf = Transform::from_translation(Vec3::ZERO).looking_at(-dir, Vec3::Y);
    }

    let amb = Vec3::new(0.35, 0.42, 0.65).lerp(Vec3::new(0.75, 0.80, 0.90), day);
    ambient.color = Color::srgb(amb.x, amb.y, amb.z);
    ambient.brightness = ((200.0 + 300.0 * vis.light) * sky.daylight).max(60.0);
}

fn move_flakes(
    time: Res<Time>,
    vis: Res<VisualWeather>,
    mut rng: ResMut<RngRes>,
    flake_mat: Res<FlakeMaterial>,
    clock: Res<ClockRes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
    mut flakes: Query<(&mut Transform, &mut Visibility, &Flake), Without<Player>>,
) {
    let Ok(ptf) = player.single() else { return };
    let cam = ptf.translation;
    let facing = ptf.rotation;
    if let Some(m) = materials.get_mut(&flake_mat.0) {
        // Radioactive snow glows faintly green.
        // Unlit, so dim them by hand at night.
        let light = 0.2 + 0.8 * clock.0.sky().daylight.clamp(0.0, 1.0);
        let c = Vec3::new(0.95, 0.97, 1.0).lerp(Vec3::new(0.7, 1.3, 0.7), vis.sick) * light;
        m.base_color = Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, 0.9));
    }
    let dt = time.delta_secs();
    let active = (vis.snow * FLAKES as f32) as usize;
    let speed_mul = 1.0 + vis.snow * 1.5;

    for (mut tf, mut visibility, flake) in &mut flakes {
        let want = if flake.index < active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if flake.index >= active {
            continue;
        }
        tf.rotation = facing;
        tf.translation.y -= flake.fall * speed_mul * dt;
        tf.translation.x += vis.wind * dt;
        tf.translation.z += vis.wind * 0.4 * dt;

        // Keep every flake inside a box that follows the camera.
        let rel = tf.translation - cam;
        if rel.x > BOX_HALF {
            tf.translation.x -= BOX_HALF * 2.0;
        } else if rel.x < -BOX_HALF {
            tf.translation.x += BOX_HALF * 2.0;
        }
        if rel.z > BOX_HALF {
            tf.translation.z -= BOX_HALF * 2.0;
        } else if rel.z < -BOX_HALF {
            tf.translation.z += BOX_HALF * 2.0;
        }
        if rel.y < -6.0 {
            tf.translation.y = cam.y + rng.0.range(10.0, 16.0);
            tf.translation.x = cam.x + rng.0.range(-BOX_HALF, BOX_HALF);
            tf.translation.z = cam.z + rng.0.range(-BOX_HALF, BOX_HALF);
        } else if rel.y > 18.0 {
            tf.translation.y = cam.y + rng.0.range(-4.0, 16.0);
        }
    }
}
