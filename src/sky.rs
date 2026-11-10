//! The sky: a dome that follows the day/night cycle and the weather. Its
//! vertex colours are the scattering model's gradient; its shader
//! (`shaders/sky.wgsl`) adds drifting sunlit clouds, the sun's disc and
//! glare, the moon, a slowly wheeling starfield and ranges of far wooded
//! hills. The northern lights hang in front on clear nights. Everything here
//! ignores fog and follows the camera.

use bevy::asset::embedded_asset;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::mesh::{MeshVertexBufferLayoutRef, VertexAttributeValues};
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};

use crate::assets::GameAssets;
use crate::meshes::to_mesh;
use crate::player::Player;
use crate::sim::meshgen::MeshData;
use crate::sim::atmosphere;
use crate::changed::{set_base_color, set_uv_transform};
use crate::sim::weather::WIND_DIR;
use crate::state::{ClockRes, CurrentInterior};
use crate::weather_fx::{SkyLook, VisualWeather};

const DOME_RADIUS: f32 = 900.0;

#[derive(Component)]
struct SkyDome;
#[derive(Component)]
struct Aurora;

/// Everything that should stay centred on the camera.
#[derive(Component)]
struct FollowCamera;

#[derive(Resource)]
struct SkyHandles {
    dome: Handle<Mesh>,
    sky: Handle<SkyMaterial>,
    aurora: Vec<Handle<StandardMaterial>>,
}

/// What `shaders/sky.wgsl` needs to know each frame (see the comments there).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct SkyParams {
    sun: Vec4,
    light: Vec4,
    light_dir: Vec4,
    moon: Vec4,
    horizon: Vec4,
    zenith: Vec4,
    cloud: Vec4,
    misc: Vec4,
}

/// The dome's material: unlit, drawn from the inside.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct SkyMaterial {
    #[uniform(0)]
    sun: Vec4,
    #[uniform(0)]
    light: Vec4,
    #[uniform(0)]
    light_dir: Vec4,
    #[uniform(0)]
    moon: Vec4,
    #[uniform(0)]
    horizon: Vec4,
    #[uniform(0)]
    zenith: Vec4,
    #[uniform(0)]
    cloud: Vec4,
    #[uniform(0)]
    misc: Vec4,
    #[texture(1)]
    #[sampler(2)]
    stars: Handle<Image>,
}

impl SkyMaterial {
    fn params(&self) -> SkyParams {
        SkyParams { sun: self.sun, light: self.light, light_dir: self.light_dir, moon: self.moon, horizon: self.horizon, zenith: self.zenith, cloud: self.cloud, misc: self.misc }
    }

    fn set(&mut self, p: SkyParams) {
        (self.sun, self.light, self.light_dir, self.moon) = (p.sun, p.light, p.light_dir, p.moon);
        (self.horizon, self.zenith, self.cloud, self.misc) = (p.horizon, p.zenith, p.cloud, p.misc);
    }
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://fallout_minnesota/shaders/sky.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // We are inside the sphere.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Where the moon hangs (it does not move).
const MOON_DIR: Vec3 = Vec3::new(-0.3, 0.8, -0.5);

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/sky.wgsl");
        app.add_plugins(MaterialPlugin::<SkyMaterial> { prepass_enabled: false, shadows_enabled: false, ..default() })
            .add_systems(Startup, spawn_sky)
            .add_systems(PostUpdate, follow_camera.before(bevy::transform::TransformSystem::TransformPropagate))
            .add_systems(Update, update_sky.after(crate::weather_fx::AtmosphereSet));
    }
}

/// Elevations (degrees) of the dome's rings: close together near the horizon,
/// where the sky's colour changes fastest, and a few below it to close the bowl.
const DOME_RINGS: [f32; 21] = [-90.0, -30.0, -8.0, -2.0, 0.0, 1.0, 2.0, 3.5, 5.5, 8.0, 11.0, 15.0, 20.0, 26.0, 33.0, 41.0, 50.0, 60.0, 72.0, 82.0, 90.0];
const DOME_SEGMENTS: usize = 72;

/// The sky dome: a sphere of rings at [`DOME_RINGS`], seen from inside.
fn dome_mesh() -> MeshData {
    let mut m = MeshData::default();
    for (r, elev) in DOME_RINGS.iter().enumerate() {
        let (sy, cy) = elev.to_radians().sin_cos();
        for s in 0..=DOME_SEGMENTS {
            let a = s as f32 / DOME_SEGMENTS as f32 * std::f32::consts::TAU;
            let d = [cy * a.cos(), sy, cy * a.sin()];
            m.vertex([d[0] * DOME_RADIUS, d[1] * DOME_RADIUS, d[2] * DOME_RADIUS], [-d[0], -d[1], -d[2]], [s as f32 / DOME_SEGMENTS as f32, r as f32 / (DOME_RINGS.len() - 1) as f32], [0.6, 0.65, 0.7, 1.0]);
        }
    }
    let row = DOME_SEGMENTS as u32 + 1;
    for r in 0..DOME_RINGS.len() as u32 - 1 {
        for s in 0..DOME_SEGMENTS as u32 {
            let a = r * row + s;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m
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
    mut skies: ResMut<Assets<SkyMaterial>>,
) {
    // The dome: its vertex colours are rewritten as the light changes.
    let dome = meshes.add(to_mesh(&dome_mesh()));
    let sky = skies.add(SkyMaterial { stars: assets.stars.clone(), ..default() });
    commands.spawn((
        Mesh3d(dome.clone()),
        MeshMaterial3d(sky.clone()),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
        SkyDome,
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

    commands.insert_resource(SkyHandles { dome, sky, aurora });
}

fn follow_camera(cam: Query<&Transform, With<Player>>, mut q: Query<&mut Transform, (With<FollowCamera>, Without<Player>)>) {
    let Ok(cam) = cam.single() else { return };
    for mut tf in &mut q {
        tf.translation = cam.translation;
    }
}

/// What the dome was last painted from, as raw bits: it is repainted only
/// when one of these changes (not while paused, on the title screen, or in a
/// frozen screenshot).
fn dome_key(look: &SkyLook) -> [u32; 13] {
    let c = &look.conditions;
    [
        c.sun[0].to_bits(),
        c.sun[1].to_bits(),
        c.sun[2].to_bits(),
        c.haze.to_bits(),
        c.moon.to_bits(),
        look.horizon.x.to_bits(),
        look.horizon.y.to_bits(),
        look.horizon.z.to_bits(),
        look.zenith.x.to_bits(),
        look.zenith.y.to_bits(),
        look.zenith.z.to_bits(),
        look.overcast.to_bits(),
        look.sick.to_bits(),
    ]
}

/// Hidden or shown, written only on change.
fn show(vis: &mut Mut<Visibility>, on: bool) {
    vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
}

/// An additive or blended sky layer with no alpha draws nothing at all, so it
/// can be hidden (and its material left alone) without changing a pixel.
fn sky_layer_shows(outdoors: bool, alpha: f32) -> bool {
    outdoors && (alpha > 0.0 || alpha.is_nan())
}

fn update_sky(
    time: Res<Time>,
    clock: Res<ClockRes>,
    look: Res<SkyLook>,
    vis: Res<VisualWeather>,
    interior: Res<CurrentInterior>,
    handles: Option<Res<SkyHandles>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut skies: ResMut<Assets<SkyMaterial>>,
    mut painted: Local<Option<[u32; 13]>>,
    mut scroll: Local<Vec2>,
    mut layers: Query<(&mut Visibility, Has<SkyDome>, Has<Aurora>), With<FollowCamera>>,
) {
    let Some(h) = handles else { return };
    let sky = clock.0.sky();
    let day = ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0);
    let overcast = look.overcast;
    let sun_dir = Vec3::from_array(look.conditions.sun).normalize_or_zero();
    // Indoors the room's walls hide the whole sky.
    let outdoors = interior.0.is_none();

    // Stars and aurora only on clear nights.
    let night = 1.0 - day;
    let clear_night = night * (1.0 - overcast);
    let sun_alpha = (sky.sun * 1.5).min(1.0) * (1.0 - overcast * 0.9);
    let moon_alpha = (night * 1.2).min(1.0) * (1.0 - overcast * 0.85);
    let t = time.elapsed_secs();
    let cover = atmosphere::cloud_cover(clock.0.day as f32 * 24.0 + clock.0.hours, overcast);
    // The northern lights show through the gaps: fainter the more cloud there is.
    // Not at dusk, while the sky still has colour: only once it is properly dark.
    let deep_night = clear_night * crate::sim::mathx::smoothstep(0.55, 0.9, night);
    let aurora_alpha = |i: usize| deep_night * (0.55 + 0.45 * (t * 0.13 + i as f32 * 1.7).sin()) * 0.9 * (1.0 - cover * 0.6);
    let show_aurora = sky_layer_shows(outdoors, deep_night);
    for (mut vis, dome, aurora) in &mut layers {
        let on = if dome {
            outdoors
        } else if aurora {
            show_aurora
        } else {
            outdoors
        };
        show(&mut vis, on);
    }

    // Repaint the dome from the sky's palette (storms flatten it towards
    // the weather colours the fog already uses), when what it shows has changed.
    let key = dome_key(&look);
    if outdoors && *painted != Some(key) {
        if let Some(mesh) = meshes.get_mut(&h.dome) {
            let positions: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
                _ => Vec::new(),
            };
            let green = Vec3::new(0.45, 0.62, 0.42);
            // Every vertex of a ring at or below the horizon is the same colour,
            // and the model's sums are costly: work each direction out once.
            let colors: Vec<[f32; 4]> = positions
                .iter()
                .map(|p| {
                    let d = Vec3::from_array(*p) / DOME_RADIUS;
                    if d.y < 0.0 {
                        return [look.horizon.x, look.horizon.y, look.horizon.z, 1.0];
                    }
                    let model = Vec3::from_array(atmosphere::sky_color(d.to_array(), &look.conditions));
                    // Under cloud the sky is a smooth ramp from horizon to zenith.
                    let overcast_c = look.horizon.lerp(look.zenith, d.y.max(0.0).powf(0.5));
                    let mut c = model.lerp(overcast_c, overcast * 0.9);
                    if look.sick > 0.0 {
                        let l = c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
                        c = c.lerp(green * l / 0.6, look.sick * 0.4);
                    }
                    [c.x, c.y, c.z, 1.0]
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            *painted = Some(key);
        }
    }

    // Materials of hidden layers are left as they are: they're rewritten
    // (from the current time) the frame they show again.
    if show_aurora {
        for (i, handle) in h.aurora.iter().enumerate() {
            let Some(base) = materials.get(handle).map(|m| m.base_color.to_srgba()) else { continue };
            set_base_color(&mut materials, handle, Color::srgba(base.red, base.green, base.blue, aurora_alpha(i)));
            set_uv_transform(&mut materials, handle, bevy::math::Affine2::from_translation(Vec2::new(t * 0.01 * (i as f32 + 1.0), 0.0)));
        }
    }

    if !outdoors {
        return;
    }
    // The clouds ride the wind (faster in a storm), high enough to be slow.
    *scroll -= Vec2::from_array(WIND_DIR) * time.delta_secs() * (0.0022 + 0.0006 * vis.wind);
    // Keep the numbers small: the pattern only needs to be continuous.
    if scroll.length() > 4096.0 {
        *scroll = Vec2::ZERO;
    }
    // What lights the clouds: the sun (they stay lit a little after the
    // ground has lost it), or the moon.
    let sun_light = look.sun_light.max(Vec3::ZERO);
    let moon_dir = MOON_DIR.normalize();
    let moonlight = Vec3::new(0.016, 0.021, 0.034) * sky.moon * (1.0 - sky.sun);
    let by_sun = sun_light.max_element() > 0.02;
    let (light, light_dir) = if by_sun {
        // Clouds stand in clearer air than the ground: the light on them is
        // brighter and less red than what reaches the snow.
        (Vec3::new(sun_light.x.powf(0.6), sun_light.y.powf(0.6), sun_light.z.powf(0.6)) * (0.35 + 0.65 * vis.light) + moonlight, sun_dir)
    } else {
        (moonlight, moon_dir)
    };
    let params = SkyParams {
        sun: sun_dir.extend(sun_alpha),
        light: light.extend(0.0),
        light_dir: light_dir.extend(0.0),
        moon: moon_dir.extend(moon_alpha),
        horizon: look.horizon.extend(overcast),
        zenith: look.zenith.extend(look.sick),
        cloud: Vec4::new(cover, atmosphere::cirrus_amount(clock.0.day as f32 * 24.0 + clock.0.hours), scroll.x, scroll.y),
        misc: Vec4::new(clear_night * crate::sim::mathx::smoothstep(0.3, 0.8, night), t * 0.002, CLOUD_BRIGHTNESS, (overcast * 0.6).min(1.0)),
    };
    if skies.get(&h.sky).is_some_and(|m| m.params() != params) {
        if let Some(m) = skies.get_mut(&h.sky) {
            m.set(params);
        }
    }
}

/// How bright sunlit cloud is next to the sky's own gradient.
const CLOUD_BRIGHTNESS: f32 = 1.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_layers_with_some_alpha_show_and_never_indoors() {
        assert!(sky_layer_shows(true, 0.3));
        assert!(sky_layer_shows(true, 1e-9));
        assert!(!sky_layer_shows(true, 0.0));
        assert!(!sky_layer_shows(true, -0.0));
        assert!(!sky_layer_shows(false, 1.0));
        // Garbage keeps drawing, as it always did.
        assert!(sky_layer_shows(true, f32::NAN));
    }

    #[test]
    fn the_dome_is_a_closed_bowl_with_most_rings_near_the_horizon() {
        let m = dome_mesh();
        assert_eq!(m.positions.len(), DOME_RINGS.len() * (DOME_SEGMENTS + 1));
        assert_eq!(m.indices.len(), (DOME_RINGS.len() - 1) * DOME_SEGMENTS * 6);
        assert!(DOME_RINGS.windows(2).all(|w| w[0] < w[1]), "rings climb");
        assert_eq!((DOME_RINGS[0], DOME_RINGS[DOME_RINGS.len() - 1]), (-90.0, 90.0));
        let low = DOME_RINGS.iter().filter(|e| (0.0..=15.0).contains(*e)).count();
        assert!(low >= 8, "{low} rings in the first 15 degrees");
        for p in &m.positions {
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - DOME_RADIUS).abs() < 0.01);
        }
    }

    #[test]
    fn the_dome_key_sees_every_input() {
        let look = SkyLook::default();
        let k = dome_key(&look);
        for i in 0..13 {
            let mut l = look.clone();
            let bump = |v: &mut f32| *v = f32::from_bits(v.to_bits() ^ 1);
            match i {
                0..=2 => bump(&mut l.conditions.sun[i]),
                3 => bump(&mut l.conditions.haze),
                4 => bump(&mut l.conditions.moon),
                5 => bump(&mut l.horizon.x),
                6 => bump(&mut l.horizon.y),
                7 => bump(&mut l.horizon.z),
                8 => bump(&mut l.zenith.x),
                9 => bump(&mut l.zenith.y),
                10 => bump(&mut l.zenith.z),
                11 => bump(&mut l.overcast),
                _ => bump(&mut l.sick),
            }
            assert_ne!(dome_key(&l), k, "input {i}");
        }
        // sun_light doesn't colour the dome.
        let mut l = look.clone();
        l.sun_light.x += 1.0;
        assert_eq!(dome_key(&l), k);
    }
}
