//! Drives the weather cycle, the day/night clock and their visuals: fog, a
//! moving sun and moon, ambient light, and wind-blown snow that turns into a
//! sickly green whiteout during rad-blizzards.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{DistanceFog, FogFalloff, FogVolume, NotShadowCaster};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::player::Player;
use crate::sim::atmosphere::{self, Conditions};
use crate::sim::weather::{Phase, Weather};
use crate::state::{ClockRes, CurrentInterior, Messages, RngRes, WeatherRes};
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
        .add_systems(Startup, (spawn_flakes.after(crate::state::WorldGen), spawn_fog_volume))
        .init_resource::<FlakeMaterial>()
        .init_resource::<SkyLook>()
        .add_systems(
            Update,
            (update_weather, smooth_visuals, apply_atmosphere.in_set(AtmosphereSet), update_fog_volume, move_flakes).chain(),
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

/// The big box of fog that follows the player: banks that thicken in
/// blizzards, hug the ground and drift with the wind.
#[derive(Component)]
struct BlizzardFog;

const FOG_BOX: Vec3 = Vec3::new(180.0, 50.0, 180.0);

fn spawn_fog_volume(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let (w, h, d) = (32usize, 16usize, 32usize);
    let mut image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: d as u32 },
        TextureDimension::D3,
        atmosphere::fog_density_field(w, h, d),
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    commands.spawn((
        FogVolume { fog_color: Color::WHITE, density_factor: 0.0, density_texture: Some(images.add(image)), scattering: 0.35, absorption: 0.08, ..default() },
        Transform::from_scale(FOG_BOX),
        BlizzardFog,
    ));
}

/// Fit the fog box round the player, scroll it with the wind and set how
/// thick it is: a thin haze in calm weather, heavy banks in a blizzard.
fn update_fog_volume(
    time: Res<Time>,
    vis: Res<VisualWeather>,
    look: Res<SkyLook>,
    interior: Res<CurrentInterior>,
    settings: Res<crate::menu::GameSettings>,
    player: Query<&Transform, With<Player>>,
    mut volume: Query<(&mut FogVolume, &mut Transform), (With<BlizzardFog>, Without<Player>)>,
) {
    let (Ok(p), Ok((mut fog, mut tf))) = (player.single(), volume.single_mut()) else { return };
    let ground = crate::sim::terrain::walk_height(p.translation.x, p.translation.z);
    // Centre the box so its floor sits a little under the snow.
    tf.translation = Vec3::new(p.translation.x, ground - 2.0 + FOG_BOX.y * 0.5, p.translation.z);
    let t = time.elapsed_secs();
    // Gusts: the banks pulse as they roll past.
    let gust = 0.8 + 0.2 * (t * 0.37).sin() + 0.12 * (t * 0.91 + 1.3).sin();
    let storm = ((vis.fog - 0.0075) / 0.03).clamp(0.0, 1.0);
    let on = settings.0.volumetrics && interior.0.is_none();
    fog.density_factor = if on { (0.0016 + 0.03 * storm * gust) * (1.0 + vis.snow * 0.5) } else { 0.0 };
    // Lit by the day's own light: pale blue-white, green in a rad-blizzard.
    let c = (look.horizon * 0.6 + look.zenith * 0.4).lerp(Vec3::new(0.45, 0.62, 0.42), vis.sick * 0.5);
    let m = c / c.max_element().max(1e-3);
    fog.fog_color = Color::linear_rgb(m.x, m.y, m.z);
    // Scroll with the wind (the field tiles, so it never runs out).
    let drift = Vec3::new(vis.wind, 0.0, vis.wind * 0.4) * t * 0.004;
    fog.density_texture_offset = drift;
}

/// Runs once the sky's look for the frame is known.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AtmosphereSet;

/// Exposure for the scattering model's colours.
pub const SKY_EXPOSURE: f32 = 0.8;

/// What the sky looks like this frame: worked out once here from the
/// scattering model and the weather, then used by the sky dome, fog, sun and
/// ambient light so they all agree.
#[derive(Resource, Clone)]
pub struct SkyLook {
    pub conditions: Conditions,
    /// The sky colour at the horizon (what the fog fades to), linear.
    pub horizon: Vec3,
    /// The sky colour overhead, linear.
    pub zenith: Vec3,
    /// Colour of direct sunlight (linear; black at night).
    pub sun_light: Vec3,
    /// 0 = clear, 1 = socked in.
    pub overcast: f32,
    /// 0..=1 radioactive green in the air.
    pub sick: f32,
}

impl Default for SkyLook {
    fn default() -> Self {
        SkyLook {
            conditions: Conditions { sun: [0.0, 1.0, 0.0], haze: 1.0, moon: 0.0 },
            horizon: Vec3::splat(0.5),
            zenith: Vec3::splat(0.3),
            sun_light: Vec3::ONE,
            overcast: 0.0,
            sick: 0.0,
        }
    }
}

fn v3(c: [f32; 3]) -> Vec3 {
    Vec3::from_array(c)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn apply_atmosphere(
    vis: Res<VisualWeather>,
    clock: Res<ClockRes>,
    interior: Res<CurrentInterior>,
    mut look: ResMut<SkyLook>,
    mut fog: Query<&mut DistanceFog>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut ambient: ResMut<AmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let sky = clock.0.sky();
    // 0 at deepest night, 1 in full day.
    let day = ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0);
    let overcast = ((vis.fog - 0.01) / 0.03).clamp(0.0, 1.0);

    // The sky itself: scattering through winter air, hazier in a storm.
    let conditions = Conditions { sun: sky.sun_pos, haze: 1.0 + 2.5 * overcast + 0.6 * vis.snow, moon: sky.moon * (1.0 - sky.sun) };
    let clear_horizon = v3(atmosphere::horizon_color(&conditions, SKY_EXPOSURE));
    let clear_zenith = v3(atmosphere::zenith_color(&conditions, SKY_EXPOSURE));
    // A storm flattens it to a dull grey, lit from above; the blizzard adds a sick green.
    let flat = |c: Vec3| Vec3::splat(luma(c)) * Vec3::new(0.93, 0.97, 1.0) * (0.45 + 0.55 * vis.light);
    let green = Vec3::new(0.45, 0.62, 0.42);
    let mut horizon = clear_horizon.lerp(flat(clear_horizon), overcast * 0.85);
    let mut zenith = clear_zenith.lerp(flat(clear_horizon) * 0.8, overcast * 0.85);
    horizon = horizon.lerp(green * luma(horizon) / luma(green), vis.sick * 0.55);
    zenith = zenith.lerp(green * luma(zenith) / luma(green), vis.sick * 0.4);
    let sun_light = v3(atmosphere::sun_transmittance(sky.sun_pos, conditions.haze));
    *look = SkyLook { conditions, horizon, zenith, sun_light, overcast, sick: vis.sick };

    let color = Color::linear_rgb(horizon.x, horizon.y, horizon.z);
    for mut f in &mut fog {
        f.color = color;
        f.falloff = FogFalloff::Exponential { density: vis.fog };
        // Fog toward the sun glows with its light (warm at sunrise and sunset).
        let glare = sun_light * 0.55 * (1.0 - overcast);
        f.directional_light_color = Color::linear_rgb(glare.x, glare.y, glare.z);
        f.directional_light_exponent = 30.0;
    }
    clear.0 = color;

    // Indoors the weather, sun and sky don't reach you: just the room's own light.
    if let Some(room) = interior.0 {
        for mut f in &mut fog {
            f.color = Color::srgb(0.02, 0.02, 0.025);
            f.falloff = FogFalloff::Exponential { density: 0.004 };
            f.directional_light_color = Color::NONE;
        }
        clear.0 = Color::BLACK;
        for (mut light, _) in &mut sun {
            light.illuminance = 0.0;
        }
        let (c, brightness) = room.ambient();
        ambient.color = Color::srgb(c[0], c[1], c[2]);
        ambient.brightness = brightness;
        return;
    }

    // Sun by day (warm and low), moon by night.
    let (dir, strength, light_color) = if sky.sun >= sky.moon * 0.3 {
        // Illuminance follows the light that survives the atmosphere.
        let lum = luma(sun_light).max(0.0).powf(0.8);
        (Vec3::from_array(sky.sun_pos), 15_000.0 * lum * (1.0 - overcast * 0.3), sun_light)
    } else {
        (Vec3::new(-0.3, 0.8, -0.5).normalize(), 350.0 * sky.moon, Vec3::new(0.6, 0.7, 1.0))
    };
    for (mut light, mut tf) in &mut sun {
        light.illuminance = strength * vis.light;
        let m = light_color / light_color.max_element().max(1e-4);
        light.color = Color::linear_rgb(m.x, m.y, m.z);
        *tf = Transform::from_translation(Vec3::ZERO).looking_at(-dir, Vec3::Y);
    }

    // Ambient light is the sky's own: blue from above, pale at the horizon.
    let amb = zenith.lerp(horizon, 0.45);
    let m = amb / amb.max_element().max(1e-4);
    ambient.color = Color::linear_rgb(m.x, m.y, m.z);
    // Less fill than sun so snow drifts, ripples and trees keep their shape.
    ambient.brightness = ((170.0 + 230.0 * vis.light) * sky.daylight).max(60.0) * (0.75 + 0.5 * day);
}

fn move_flakes(
    time: Res<Time>,
    vis: Res<VisualWeather>,
    interior: Res<CurrentInterior>,
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
    let active = if interior.0.is_some() { 0 } else { (vis.snow * FLAKES as f32) as usize };
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
