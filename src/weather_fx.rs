//! Drives the weather cycle and its visuals: fog, sunlight, ambient light and
//! wind-blown snow that turns into a sickly green whiteout during rad-blizzards.

use bevy::pbr::{DistanceFog, FogFalloff, NotShadowCaster};
use bevy::prelude::*;

use crate::player::Player;
use crate::sim::weather::{Phase, Weather};
use crate::state::{Messages, RngRes, WeatherRes};
use crate::world::Sun;

const FLAKES: usize = 900;
const BOX_HALF: f32 = 25.0;

#[derive(Component)]
struct Flake {
    index: usize,
    fall: f32,
}

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
        .add_systems(
            Update,
            (update_weather, smooth_visuals, apply_atmosphere, move_flakes).chain(),
        );
    }
}

fn spawn_flakes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<RngRes>,
) {
    let mesh = meshes.add(Cuboid::new(0.07, 0.07, 0.07));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.97, 1.0),
        unlit: true,
        ..default()
    });
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
    mut rng: ResMut<RngRes>,
    mut msgs: ResMut<Messages>,
) {
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
    mut fog: Query<&mut DistanceFog>,
    mut sun: Query<&mut DirectionalLight, With<Sun>>,
    mut ambient: ResMut<AmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let white = Vec3::new(0.70, 0.74, 0.78);
    let green = Vec3::new(0.45, 0.62, 0.42);
    let c = white.lerp(green, vis.sick) * (0.55 + 0.45 * vis.light);
    let color = Color::srgb(c.x, c.y, c.z);
    for mut f in &mut fog {
        f.color = color;
        f.falloff = FogFalloff::Exponential { density: vis.fog };
    }
    for mut light in &mut sun {
        light.illuminance = 6_000.0 * vis.light;
    }
    ambient.brightness = 200.0 + 300.0 * vis.light;
    clear.0 = color;
}

fn move_flakes(
    time: Res<Time>,
    vis: Res<VisualWeather>,
    mut rng: ResMut<RngRes>,
    player: Query<&Transform, With<Player>>,
    mut flakes: Query<(&mut Transform, &mut Visibility, &Flake), Without<Player>>,
) {
    let Ok(ptf) = player.single() else { return };
    let cam = ptf.translation;
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
