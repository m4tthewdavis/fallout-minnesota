//! The snow ground material: Bevy's StandardMaterial extended with a shader
//! (`shaders/snow.wgsl`) that adds powder, wind-packed and icy-crust areas,
//! wind ripples, scattered pits, a second texture scale to hide tiling, and
//! sun glints. The sun and wind it uses are kept up to date here.

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

use crate::sim::weather::{Phase, WIND_DIR};
use crate::state::WeatherRes;
use crate::world::Sun;

pub type SnowMaterial = ExtendedMaterial<StandardMaterial, SnowExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SnowExt {
    /// Direction towards the sun (xyz) and its strength (w).
    #[uniform(100)]
    pub sun: Vec4,
    /// Wind direction (xy), ripple strength (z) and glint strength (w).
    #[uniform(100)]
    pub wind: Vec4,
}

impl Default for SnowExt {
    fn default() -> Self {
        Self {
            sun: Vec4::new(0.4, 1.0, 0.3, 1.0),
            wind: Vec4::new(WIND_DIR[0], WIND_DIR[1], 1.0, 1.0),
        }
    }
}

impl MaterialExtension for SnowExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://fallout_minnesota/shaders/snow.wgsl".into()
    }
}

pub struct SnowPlugin;

impl Plugin for SnowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/snow.wgsl");
        app.add_plugins(MaterialPlugin::<SnowMaterial>::default()).add_systems(Update, follow_sun);
    }
}

/// Glints follow the sun, fade at night and dull in a blizzard.
fn follow_sun(sun: Query<(&DirectionalLight, &Transform), With<Sun>>, weather: Res<WeatherRes>, mut materials: ResMut<Assets<SnowMaterial>>, mut last: Local<Vec4>) {
    let Ok((light, tf)) = sun.single() else { return };
    let towards = tf.back();
    let blizzard = matches!(weather.weather.phase, Phase::Blizzard);
    let strength = (light.illuminance / 9000.0).clamp(0.0, 1.2) * if blizzard { 0.25 } else { 1.0 };
    let value = towards.extend(strength);
    // Only touch the materials when something visibly changed.
    if (value - *last).length() < 0.01 {
        return;
    }
    *last = value;
    for (_, m) in materials.iter_mut() {
        m.extension.sun = value;
    }
}
