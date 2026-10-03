//! Loads the CC0 Poly Haven models and textures, the generated textures and
//! the HUD font from `assets/`, builds the shared PBR materials, and gives
//! loaded textures proper mipmaps so tiled snow and rust don't shimmer.

use bevy::asset::AssetEvent;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::sim::mipmaps;

/// Handles to every model, material and sprite the world uses.
#[derive(Resource)]
pub struct GameAssets {
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

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_assets).add_systems(Update, add_mipmaps);
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

fn load_assets(mut commands: Commands, server: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let s = &*server;
    let m = &mut *materials;
    let generated = |name: &str| s.load::<Image>(format!("textures/generated/{name}"));
    let ui = |name: &str| s.load::<Image>(format!("ui/{name}"));

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

    commands.insert_resource(GameAssets {
        font: s.load("fonts/ShareTechMono-Regular.ttf"),

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
        soft: generated("soft.png"),
        flash: generated("flash.png"),
        stars: generated("stars.png"),
        aurora: tiled(s, "textures/generated/aurora.png".into(), true),
        sign_bullseye: generated("sign_bullseye.png"),
        sign_mille_lacs: generated("sign_mille_lacs.png"),
        sign_golden_atomic: generated("sign_golden_atomic.png"),
        sign_shelter: generated("sign_fallout_shelter.png"),

        icon_hp: ui("icon_hp.png"),
        icon_heat: ui("icon_heat.png"),
        icon_rads: ui("icon_rads.png"),
        scanlines: ui("scanlines.png"),
        vignette: ui("vignette.png"),
        frost: ui("frost.png"),
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
