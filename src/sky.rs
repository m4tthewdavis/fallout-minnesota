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
use crate::sim::atmosphere;
use crate::changed::{set_base_color, set_uv_transform};
use crate::state::{ClockRes, CurrentInterior};
use crate::weather_fx::{SkyLook, SKY_EXPOSURE};

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
            .add_systems(Update, update_sky.after(crate::weather_fx::AtmosphereSet));
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
    let mut dome_mesh: Mesh = Sphere::new(DOME_RADIUS).mesh().uv(72, 40);
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
    interior: Res<CurrentInterior>,
    handles: Option<Res<SkyHandles>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut painted: Local<Option<[u32; 13]>>,
    cam: Query<&Transform, With<Player>>,
    mut layers: Query<(&mut Visibility, Has<SkyDome>, Has<Stars>, Has<Aurora>, Has<SunDisc>, Has<MoonDisc>), With<FollowCamera>>,
    mut stars: Query<&mut Transform, (With<Stars>, Without<Player>)>,
    mut discs: ParamSet<(
        Query<&mut Transform, (With<SunDisc>, Without<Player>, Without<Stars>)>,
        Query<&mut Transform, (With<MoonDisc>, Without<Player>, Without<Stars>)>,
    )>,
) {
    let Some(h) = handles else { return };
    let Ok(cam) = cam.single() else { return };
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
    let aurora_alpha = |i: usize| clear_night * (0.55 + 0.45 * (t * 0.13 + i as f32 * 1.7).sin()) * 0.9;
    let show_stars = sky_layer_shows(outdoors, clear_night);
    let show_aurora = sky_layer_shows(outdoors, clear_night);
    let show_sun = sky_layer_shows(outdoors, sun_alpha);
    let show_moon = sky_layer_shows(outdoors, moon_alpha);
    for (mut vis, dome, star, aurora, sun, moon) in &mut layers {
        let on = if dome {
            outdoors
        } else if star {
            show_stars
        } else if aurora {
            show_aurora
        } else if sun {
            show_sun
        } else if moon {
            show_moon
        } else {
            outdoors
        };
        show(&mut vis, on);
    }

    // Repaint the dome from the scattering model (storms flatten it towards
    // the weather colours the fog already uses), when what it shows has changed.
    let key = dome_key(&look);
    if outdoors && *painted != Some(key) {
        if let Some(mesh) = meshes.get_mut(&h.dome) {
            let positions: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
                _ => Vec::new(),
            };
            let green = Vec3::new(0.45, 0.62, 0.42);
            let colors: Vec<[f32; 4]> = positions
                .iter()
                .map(|p| {
                    let d = Vec3::from_array(*p) / DOME_RADIUS;
                    if d.y < 0.0 {
                        return [look.horizon.x, look.horizon.y, look.horizon.z, 1.0];
                    }
                    let model = Vec3::from_array(atmosphere::expose(atmosphere::sky_radiance(d.to_array(), &look.conditions), SKY_EXPOSURE));
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
    if show_stars {
        set_base_color(&mut materials, &h.stars, Color::srgba(1.0, 1.0, 1.0, clear_night));
    }
    if show_aurora {
        for (i, handle) in h.aurora.iter().enumerate() {
            let Some(base) = materials.get(handle).map(|m| m.base_color.to_srgba()) else { continue };
            set_base_color(&mut materials, handle, Color::srgba(base.red, base.green, base.blue, aurora_alpha(i)));
            set_uv_transform(&mut materials, handle, bevy::math::Affine2::from_translation(Vec2::new(t * 0.01 * (i as f32 + 1.0), 0.0)));
        }
    }
    for mut tf in &mut stars {
        tf.rotation = Quat::from_rotation_y(t * 0.002) * Quat::from_rotation_x(0.35);
    }

    // Sun and moon discs, facing the camera.
    if show_sun {
        // The disc takes the colour of the light that survives the air: white-gold
        // high up, orange then red as it sinks.
        let t = look.sun_light;
        let k = 5.0 / t.max_element().max(1e-3);
        set_base_color(&mut materials, &h.sun, Color::LinearRgba(LinearRgba::new(t.x * k, t.y * k, t.z * k, sun_alpha)));
    }
    for mut tf in &mut discs.p0() {
        let p = cam.translation + sun_dir * DOME_RADIUS * 0.9;
        *tf = Transform::from_translation(p).looking_at(cam.translation, Vec3::Y);
    }
    let moon_dir = Vec3::new(-0.3, 0.8, -0.5).normalize();
    if show_moon {
        set_base_color(&mut materials, &h.moon, Color::LinearRgba(LinearRgba::new(2.2, 2.3, 2.6, moon_alpha)));
    }
    for mut tf in &mut discs.p1() {
        let p = cam.translation + moon_dir * DOME_RADIUS * 0.9;
        *tf = Transform::from_translation(p).looking_at(cam.translation, Vec3::Y);
    }
}

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
