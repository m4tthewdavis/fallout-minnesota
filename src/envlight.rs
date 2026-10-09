//! Image-based lighting: winter sky cubemaps (baked from CC0 Poly Haven
//! HDRIs by `tools/bake_ibl.py`) give the world its ambient fill and the
//! reflections on ice, metal and guns. The rules for which map, how bright
//! and how much of the old flat fill to keep are in `sim::envlight`; this
//! hands them to Bevy's `EnvironmentMapLight` on both cameras every frame,
//! fading between maps when the weather turns.

use bevy::pbr::environment_map::EnvironmentMapLight;
use bevy::prelude::*;

use crate::gun::ViewModelCamera;
use crate::player::Player;
use crate::sim::envlight::{self, EnvFade, EnvMap};
use crate::state::{ClockRes, CurrentInterior};
use crate::weather_fx::{AtmosphereSet, VisualWeather};

/// The least sky light the view-model camera gets (in the same units as
/// `AmbientLight::brightness`).
const GUN_FILL: f32 = 160.0;

#[derive(Resource)]
struct EnvMaps {
    /// Index 0 is the overcast map, 1 the clear one.
    diffuse: [Handle<Image>; 2],
    specular: [Handle<Image>; 2],
    fade: EnvFade,
}

fn index(m: EnvMap) -> usize {
    if m == EnvMap::Clear {
        1
    } else {
        0
    }
}

pub struct EnvLightPlugin;

impl Plugin for EnvLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_maps).add_systems(Update, apply_env_light.after(AtmosphereSet));
    }
}

fn load_maps(mut commands: Commands, server: Res<AssetServer>) {
    let load = |p: &'static str| server.load::<Image>(p);
    commands.insert_resource(EnvMaps {
        diffuse: [load(EnvMap::Overcast.diffuse_path()), load(EnvMap::Clear.diffuse_path())],
        specular: [load(EnvMap::Overcast.specular_path()), load(EnvMap::Clear.specular_path())],
        fade: EnvFade::new(EnvMap::Clear),
    });
}

/// After `apply_atmosphere` has set the flat ambient light for the frame:
/// swap most of it for the sky map.
#[allow(clippy::type_complexity)]
fn apply_env_light(
    mut commands: Commands,
    time: Res<Time<Real>>,
    clock: Res<ClockRes>,
    vis: Res<VisualWeather>,
    interior: Res<CurrentInterior>,
    mut maps: ResMut<EnvMaps>,
    mut ambient: ResMut<AmbientLight>,
    mut cams: Query<(Entity, Option<&mut EnvironmentMapLight>, Has<ViewModelCamera>), Or<(With<Player>, With<ViewModelCamera>)>>,
) {
    let sky = clock.0.sky();
    let weather = envlight::Weather { overcast: ((vis.fog - 0.01) / 0.03).clamp(0.0, 1.0), sick: vis.sick, light: vis.light };
    let shown = maps.fade.shown;
    let want = envlight::evaluate(shown, &sky, &weather, interior.0);
    // A door's fade to black hides a sudden change: snap to the right map.
    if interior.is_changed() {
        maps.fade = EnvFade::new(want.map);
    }
    let level = maps.fade.step(time.delta_secs(), want.map);
    let i = index(maps.fade.shown);
    let light = EnvironmentMapLight {
        diffuse_map: maps.diffuse[i].clone(),
        specular_map: maps.specular[i].clone(),
        intensity: want.intensity * level,
        rotation: Quat::from_rotation_y(envlight::map_yaw(maps.fade.shown, sky.sun_pos)),
        ..default()
    };
    for (cam, env, gun) in &mut cams {
        let mut light = light.clone();
        // The rifle is bare metal: with only the dim fill of a room or a
        // night it went black. Like any shooter, keep it readable.
        if gun {
            light.intensity = light.intensity.max(GUN_FILL);
        }
        match env {
            Some(mut e) => *e = light,
            None => {
                commands.entity(cam).insert(light);
            }
        }
    }
    // The map now does much of the fill the flat ambient light used to; while
    // maps swap (the map fades out and back in) the flat light covers for it,
    // so the total never dips. `apply_atmosphere` resets the ambient light
    // every frame, so this never compounds.
    ambient.brightness *= 1.0 - (1.0 - want.ambient_keep) * level;
    let c = ambient.color.to_linear();
    ambient.color = Color::linear_rgb(c.red * want.tint[0], c.green * want.tint[1], c.blue * want.tint[2]);
}
