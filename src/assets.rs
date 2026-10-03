//! Loads the CC0 Poly Haven models and textures, the generated textures and
//! the HUD font from `assets/`, builds the shared PBR materials, and gives
//! loaded textures proper mipmaps so tiled snow and rust don't shimmer.
//!
//! It also keeps the game playable when files are missing: the `assets`
//! folder is searched for in several places, the HUD font, HUD images and
//! particle sprites are built into the executable, any material whose
//! texture fails to load falls back to its plain colour, and missing files
//! are counted so the HUD can warn the player.

use std::path::{Path, PathBuf};

use bevy::asset::{AssetEvent, AssetLoadFailedEvent, UntypedAssetLoadFailedEvent};
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
    ImageType,
};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::TextureFormat;

use crate::sim::mipmaps;

/// Handles to every model, material and sprite the world uses.
#[derive(Resource)]
pub struct GameAssets {
    /// Pipe-rifle textures: steel_diff, steel_arm, wood_diff, wood_nor.
    pub gun_textures: std::collections::HashMap<&'static str, Handle<Image>>,
    pub font: Handle<Font>,

    // Poly Haven models (glTF scenes).
    pub barrel_stove: Handle<Scene>,
    pub barrel: Handle<Scene>,
    pub tyre: Handle<Scene>,
    pub rim: Handle<Scene>,
    pub jerrycan: Handle<Scene>,
    pub crate_wood: Handle<Scene>,
    pub crate_military: Handle<Scene>,
    pub utility_box: Handle<Scene>,
    pub covered_car: Handle<Scene>,
    pub ammo_box: Handle<Scene>,
    pub medical_box: Handle<Scene>,
    pub can: Handle<Scene>,
    pub food_cans: Handle<Scene>,
    pub rock: Handle<Scene>,
    pub stump: Handle<Scene>,

    // Tiled PBR materials.
    pub snow: Handle<StandardMaterial>,
    pub bark: Handle<StandardMaterial>,
    pub rust: Handle<StandardMaterial>,
    pub corrugated: Handle<StandardMaterial>,
    pub planks_red: Handle<StandardMaterial>,
    pub planks_dark: Handle<StandardMaterial>,
    pub concrete: Handle<StandardMaterial>,
    pub rock_wall: Handle<StandardMaterial>,
    pub vault_metal: Handle<StandardMaterial>,
    pub roof_metal: Handle<StandardMaterial>,
    pub pole_wood: Handle<StandardMaterial>,
    pub asphalt: Handle<StandardMaterial>,
    /// Rust textures on their own, for car bodies in different paint colours.
    pub rust_diff: Handle<Image>,
    pub rust_normal: Handle<Image>,

    // Generated textures.
    pub ice_diff: Handle<Image>,
    pub ice_emissive: Handle<Image>,
    pub scorch: Handle<Image>,
    pub soft: Handle<Image>,
    pub flash: Handle<Image>,
    pub stars: Handle<Image>,
    pub aurora: Handle<Image>,
    pub sign_bullseye: Handle<Image>,
    pub sign_mille_lacs: Handle<Image>,
    pub sign_golden_atomic: Handle<Image>,
    pub sign_shelter: Handle<Image>,

    // HUD images.
    pub icon_hp: Handle<Image>,
    pub icon_heat: Handle<Image>,
    pub icon_rads: Handle<Image>,
    pub scanlines: Handle<Image>,
    pub vignette: Handle<Image>,
    pub frost: Handle<Image>,
}

/// A file every complete `assets` folder contains.
const MARKER: &str = "textures/snow_02/diff_clean.jpg";

/// Where to look for the `assets` folder, best first: next to the
/// executable, one or two folders up (a `cargo build` puts the executable in
/// `target/release`), the working directory, and the source checkout.
fn candidate_roots(exe_dir: Option<&Path>, cwd: Option<&Path>, manifest: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = exe_dir {
        out.push(dir.join("assets"));
        out.extend(dir.parent().map(|p| p.join("assets")));
        out.extend(dir.parent().and_then(Path::parent).map(|p| p.join("assets")));
    }
    out.extend(cwd.map(|d| d.join("assets")));
    out.extend(manifest.map(|d| d.join("assets")));
    out
}

/// The first candidate that really is a complete `assets` folder.
fn pick_root(candidates: &[PathBuf], is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    candidates.iter().find(|c| is_file(&c.join(MARKER))).cloned()
}

/// Absolute path of the `assets` folder for Bevy's `AssetPlugin`. Falls back
/// to the folder next to the executable (which the HUD then reports as missing).
pub fn asset_root() -> String {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let cwd = std::env::current_dir().ok();
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    let candidates = candidate_roots(exe_dir.as_deref(), cwd.as_deref(), manifest.as_deref());
    let root = pick_root(&candidates, |p| p.is_file()).unwrap_or_else(|| candidates.first().cloned().unwrap_or_else(|| "assets".into()));
    root.to_string_lossy().into_owned()
}

/// Files that failed to load, for the HUD warning.
#[derive(Resource, Default)]
pub struct MissingAssets {
    pub count: usize,
    pub first: Option<String>,
    pub root: String,
}

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MissingAssets {
            root: asset_root(),
            ..default()
        })
        .add_systems(PreStartup, load_assets)
        .add_systems(Update, (add_mipmaps, fall_back_to_plain_colours, count_missing));
    }
}

fn repeat_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    })
}

/// A tiling texture; `srgb` is false for normal and roughness/metal maps.
fn tiled(server: &AssetServer, path: String, srgb: bool) -> Handle<Image> {
    server.load_with_settings(path, move |s: &mut ImageLoaderSettings| {
        s.is_srgb = srgb;
        s.sampler = repeat_sampler();
    })
}

/// A Poly Haven PBR set: `diff.jpg`, `nor.jpg` and `arm.jpg` (AO / rough /
/// metal, the same channel layout glTF uses).
fn pbr(server: &AssetServer, materials: &mut Assets<StandardMaterial>, set: &str, tint: Color) -> Handle<StandardMaterial> {
    let dir = format!("textures/{set}");
    materials.add(StandardMaterial {
        base_color: tint,
        base_color_texture: Some(tiled(server, format!("{dir}/diff.jpg"), true)),
        normal_map_texture: Some(tiled(server, format!("{dir}/nor.jpg"), false)),
        metallic_roughness_texture: Some(tiled(server, format!("{dir}/arm.jpg"), false)),
        occlusion_texture: Some(tiled(server, format!("{dir}/arm.jpg"), false)),
        metallic: 1.0,
        perceptual_roughness: 1.0,
        ..default()
    })
}

fn scene(server: &AssetServer, id: &str) -> Handle<Scene> {
    server.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{id}/{id}.gltf")))
}

/// A PNG compiled into the executable.
fn embedded_png(images: &mut Assets<Image>, bytes: &[u8]) -> Handle<Image> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::default(),
    )
    .expect("embedded PNG is valid");
    images.add(image)
}

fn load_assets(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let s = &*server;
    let m = &mut *materials;
    let generated = |name: &str| s.load::<Image>(format!("textures/generated/{name}"));
    // The HUD and particle sprites are small, so they are built into the
    // executable: the HUD always works even if the assets folder is missing.
    let mut png = |bytes: &[u8]| embedded_png(&mut images, bytes);
    let font = Font::try_from_bytes(include_bytes!("../assets/fonts/ShareTechMono-Regular.ttf").to_vec())
        .expect("embedded font is valid");
    let font = fonts.add(font);
    let icon_hp = png(include_bytes!("../assets/ui/icon_hp.png"));
    let icon_heat = png(include_bytes!("../assets/ui/icon_heat.png"));
    let icon_rads = png(include_bytes!("../assets/ui/icon_rads.png"));
    let scanlines = png(include_bytes!("../assets/ui/scanlines.png"));
    let vignette = png(include_bytes!("../assets/ui/vignette.png"));
    let frost = png(include_bytes!("../assets/ui/frost.png"));
    let soft = png(include_bytes!("../assets/textures/generated/soft.png"));
    let flash = png(include_bytes!("../assets/textures/generated/flash.png"));

    // Terrain snow: a cleaned-up diffuse (see tools/gen_textures.py) and no
    // baked AO, which made the whole map look dirty when tiled.
    let snow = m.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.98, 1.0),
        base_color_texture: Some(tiled(s, "textures/snow_02/diff_clean.jpg".into(), true)),
        normal_map_texture: Some(tiled(s, "textures/snow_02/nor.jpg".into(), false)),
        metallic_roughness_texture: Some(tiled(s, "textures/snow_02/arm.jpg".into(), false)),
        metallic: 0.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    let bark = m.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.8, 0.75),
        base_color_texture: Some(tiled(s, "textures/pine_bark/diff.jpg".into(), true)),
        normal_map_texture: Some(tiled(s, "textures/pine_bark/nor.jpg".into(), false)),
        perceptual_roughness: 0.95,
        ..default()
    });

    let gun_textures = [
        ("steel_diff", "gun_steel_diff.png", true),
        ("steel_arm", "gun_steel_arm.png", false),
        ("wood_diff", "gun_wood_diff.png", true),
        ("wood_nor", "gun_wood_nor.png", false),
    ]
    .into_iter()
    .map(|(key, file, srgb)| (key, tiled(s, format!("textures/generated/{file}"), srgb)))
    .collect();

    commands.insert_resource(GameAssets {
        gun_textures,
        font,

        barrel_stove: scene(s, "barrel_stove"),
        barrel: scene(s, "barrel_03"),
        tyre: scene(s, "old_tyre"),
        rim: scene(s, "rusted_wheel_rim_01"),
        jerrycan: scene(s, "metal_jerrycan"),
        crate_wood: scene(s, "wooden_crate_02"),
        crate_military: scene(s, "old_military_crate"),
        utility_box: scene(s, "utility_box_01"),
        covered_car: scene(s, "covered_car"),
        ammo_box: scene(s, "ammo_box"),
        medical_box: scene(s, "medical_box"),
        can: scene(s, "can_rusted"),
        food_cans: scene(s, "russian_food_cans_01"),
        rock: scene(s, "rock_07"),
        stump: scene(s, "tree_stump_01"),

        snow,
        bark,
        rust: pbr(s, m, "rusty_metal_02", Color::WHITE),
        corrugated: pbr(s, m, "rusty_corrugated_iron", Color::WHITE),
        planks_red: pbr(s, m, "weathered_plank_siding", Color::srgb(0.95, 0.38, 0.3)),
        planks_dark: pbr(s, m, "weathered_plank_siding", Color::srgb(0.45, 0.38, 0.32)),
        concrete: pbr(s, m, "cracked_concrete_wall", Color::srgb(0.85, 0.85, 0.85)),
        rock_wall: pbr(s, m, "rock_wall_02", Color::srgb(0.8, 0.8, 0.82)),
        vault_metal: pbr(s, m, "metal_plate", Color::srgb(0.75, 0.75, 0.7)),
        roof_metal: pbr(s, m, "rusty_corrugated_iron", Color::srgb(0.45, 0.42, 0.4)),
        pole_wood: pbr(s, m, "weathered_plank_siding", Color::srgb(0.4, 0.3, 0.22)),
        asphalt: pbr(s, m, "asphalt_snow", Color::WHITE),
        rust_diff: tiled(s, "textures/rusty_metal_02/diff.jpg".into(), true),
        rust_normal: tiled(s, "textures/rusty_metal_02/nor.jpg".into(), false),

        ice_diff: tiled(s, "textures/generated/ice_diff.png".into(), true),
        ice_emissive: tiled(s, "textures/generated/ice_emissive.png".into(), true),
        scorch: generated("scorch.png"),
        soft,
        flash,
        stars: generated("stars.png"),
        aurora: tiled(s, "textures/generated/aurora.png".into(), true),
        sign_bullseye: generated("sign_bullseye.png"),
        sign_mille_lacs: generated("sign_mille_lacs.png"),
        sign_golden_atomic: generated("sign_golden_atomic.png"),
        sign_shelter: generated("sign_fallout_shelter.png"),

        icon_hp,
        icon_heat,
        icon_rads,
        scanlines,
        vignette,
        frost,
    });
}

/// Builds a full mip chain for every 8-bit RGBA texture as it finishes loading
/// (including the textures inside the glTF models).
fn add_mipmaps(mut events: EventReader<AssetEvent<Image>>, mut images: ResMut<Assets<Image>>) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        let Some(image) = images.get_mut(*id) else { continue };
        let desc = &image.texture_descriptor;
        let srgb = match desc.format {
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Unorm => false,
            _ => continue,
        };
        let (w, h) = (desc.size.width, desc.size.height);
        if desc.mip_level_count != 1 || desc.size.depth_or_array_layers != 1 || w.min(h) < 16 {
            continue;
        }
        let Some(data) = image.data.as_ref() else { continue };
        if data.len() != (w * h * 4) as usize {
            continue;
        }
        let (chain, levels) = mipmaps::build_chain(w, h, data, srgb);
        image.data = Some(chain);
        image.texture_descriptor.mip_level_count = levels;
        // Make sure the sampler actually blends between the new levels.
        if let ImageSampler::Descriptor(d) = &mut image.sampler {
            d.mipmap_filter = ImageFilterMode::Linear;
        } else {
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                mipmap_filter: ImageFilterMode::Linear,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        }
    }
}

/// A material whose texture failed to load is never drawn at all (that made
/// the whole terrain invisible). Drop the missing texture so the material
/// falls back to its plain colour: white snow, grey rock and so on.
fn fall_back_to_plain_colours(
    mut failed: EventReader<AssetLoadFailedEvent<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in failed.read() {
        let id = event.id;
        let uses = |slot: &Option<Handle<Image>>| slot.as_ref().is_some_and(|h| h.id() == id);
        let affected: Vec<AssetId<StandardMaterial>> = materials
            .iter()
            .filter(|(_, m)| {
                uses(&m.base_color_texture)
                    || uses(&m.normal_map_texture)
                    || uses(&m.metallic_roughness_texture)
                    || uses(&m.occlusion_texture)
                    || uses(&m.emissive_texture)
            })
            .map(|(mid, _)| mid)
            .collect();
        for mid in affected {
            let Some(m) = materials.get_mut(mid) else { continue };
            for slot in [
                &mut m.base_color_texture,
                &mut m.normal_map_texture,
                &mut m.metallic_roughness_texture,
                &mut m.occlusion_texture,
                &mut m.emissive_texture,
            ] {
                if slot.as_ref().is_some_and(|h| h.id() == id) {
                    *slot = None;
                }
            }
            // Without its metal/roughness map a material would turn into
            // shiny chrome (both factors are 1.0 for textured materials).
            if m.metallic_roughness_texture.is_none() && m.metallic > 0.99 {
                m.metallic = 0.0;
                m.perceptual_roughness = 0.9;
            }
        }
    }
}

fn count_missing(mut failed: EventReader<UntypedAssetLoadFailedEvent>, mut missing: ResMut<MissingAssets>) {
    for event in failed.read() {
        missing.count += 1;
        if missing.first.is_none() {
            missing.first = Some(event.path.to_string());
        }
        warn!("missing game file: {} ({})", event.path, event.error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_assets_next_to_the_exe_first() {
        let exe = Path::new("/game");
        let c = candidate_roots(Some(exe), Some(Path::new("/home/me")), None);
        assert_eq!(c[0], Path::new("/game/assets"));
        let found = pick_root(&c, |p| p.starts_with("/home/me/assets"));
        assert_eq!(found.as_deref(), Some(Path::new("/home/me/assets")));
    }

    #[test]
    fn finds_the_checkout_from_target_release() {
        let exe = Path::new("/src/fallout-minnesota/target/release");
        let c = candidate_roots(Some(exe), None, None);
        let found = pick_root(&c, |p| p == Path::new("/src/fallout-minnesota/assets").join(MARKER));
        assert_eq!(found.as_deref(), Some(Path::new("/src/fallout-minnesota/assets")));
    }

    #[test]
    fn nothing_found_means_none() {
        let c = candidate_roots(Some(Path::new("/tmp/x")), Some(Path::new("/tmp")), None);
        assert!(pick_root(&c, |_| false).is_none());
    }
}
