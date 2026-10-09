//! The furniture and set-dressing library: the CC0 models and surfaces
//! fetched by `tools/fetch_assets.py` for Milestone 10, looked up by their
//! Poly Haven / ambientCG id. Some models are kits (a dozen duct or pipe
//! pieces laid out side by side in one file); `Library::piece` spawns just
//! one of them, by node name, once the file has loaded.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use crate::snow::{RockSnowExt, RockSnowMaterial, ROCK_SNOW};

/// Whole models, spawned as scenes.
const SCENES: &[&str] = &[
    "metal_office_desk",
    "drawer_cabinet",
    "Television_01",
    "vintage_radio_transceiver",
    "SchoolChair_01",
    "caged_hanging_light",
    "power_box_01",
    "old_military_compressor",
    "vintage_spacecraft_instrument",
    "hanging_industrial_lamp",
    "metal_tool_chest",
    "Barrel_01",
    "old_gas_mask",
    "CashRegister_01",
    "painted_wooden_shelves",
    "cardboard_box_01",
    "trashbag",
    "long_life_food",
    "Lantern_01",
    "life_jacket",
    "Rockingchair_01",
    "boulder_01",
    "rock_face_02",
    "dead_tree_trunk",
    "tree_stump_02",
    "dry_branches_medium_01",
    "propane_tank",
    "portable_generator",
    "concrete_road_barrier",
];

/// Kits: pieces are picked out by node name.
const KITS: &[&str] = &["mounted_fluorescent_lights", "modular_airduct_rectangular_01", "modular_industrial_pipes_01"];

/// Tiling PBR surfaces (diff / nor / arm): a tint, and how metallic the
/// metal channel is allowed to be (bare metal reflects only the room's
/// little light, so it reads near-black without an environment map).
const SURFACES: &[(&str, [f32; 3], f32)] = &[
    ("Tiles140", [0.9, 0.9, 0.9], 0.0),
    ("PaintedMetal006", [0.9, 0.95, 0.95], 0.0),
    ("PaintedMetal016", [1.0, 1.0, 1.0], 0.0),
    ("Concrete031", [0.95, 0.95, 0.95], 0.0),
    ("MetalPlates013", [1.0, 1.0, 1.0], 0.35),
    ("OfficeCeiling003", [0.85, 0.85, 0.85], 0.0),
    ("concrete_floor_worn_001", [0.9, 0.9, 0.9], 0.0),
    ("damaged_concrete_floor_02", [0.85, 0.85, 0.85], 0.0),
    ("dirty_tiles", [0.9, 0.9, 0.9], 0.0),
    ("dark_wooden_planks", [0.85, 0.8, 0.75], 0.0),
    ("metal_grate_rusty", [1.0, 1.0, 1.0], 0.5),
];

#[derive(Resource)]
pub struct Library {
    scenes: HashMap<&'static str, Handle<Scene>>,
    kits: HashMap<&'static str, Handle<Gltf>>,
    surfaces: HashMap<&'static str, Handle<StandardMaterial>>,
}

impl Library {
    /// A whole model. Panics on an id that isn't in the list (a typo in code).
    pub fn scene(&self, id: &str) -> SceneRoot {
        SceneRoot(self.scenes.get(id).unwrap_or_else(|| panic!("no model {id} in the library")).clone())
    }

    /// One piece of a kit, by node name; it appears once the file has loaded.
    pub fn piece(&self, kit: &str, node: &'static str) -> KitPiece {
        KitPiece { gltf: self.kits.get(kit).unwrap_or_else(|| panic!("no kit {kit} in the library")).clone(), node }
    }

    pub fn surface(&self, id: &str) -> Handle<StandardMaterial> {
        self.surfaces.get(id).unwrap_or_else(|| panic!("no surface {id} in the library")).clone()
    }
}

/// Becomes the meshes of one named node of a kit (the node's own position in
/// the kit is dropped: the piece sits at this entity's origin).
#[derive(Component, Clone)]
pub struct KitPiece {
    gltf: Handle<Gltf>,
    node: &'static str,
}

/// Weathers a model's materials once its scene has spawned: a colour
/// multiply (cold granite, grey concrete) and snow lying on its upward faces
/// (see `shaders/rock_snow.wgsl`). Materials are shared by every model with
/// the same weathering. The multiply alone can't turn the scans' warm
/// sandstone grey, so their colour maps are drained first (see [`GREYED`]).
#[derive(Component, Clone, Copy)]
pub struct Weathered {
    pub tint: [f32; 3],
    /// 0..1: how much snow lies on it.
    pub snow: f32,
    /// 0..1: fine stone grain added up close (see `RockSnowExt::detail`).
    pub grain: f32,
}

/// Colour maps drained of most of their colour as they load (path, how much
/// saturation to keep), so the stone takes the cold tint it is given.
pub const GREYED: &[(&str, f32)] = &[
    ("models/boulder_01/textures/boulder_01_diff_1k.jpg", 0.12),
    ("models/rock_face_02/textures/rock_face_02_diff_1k.jpg", 0.12),
    ("models/rock_07/textures/rock_07_diff_1k.jpg", 0.15),
    ("models/concrete_road_barrier/textures/concrete_road_barrier_diff_1k.jpg", 0.2),
    // The pine trunks read as flat saturated orange; red pine is only reddish.
    ("textures/pine_bark/diff.jpg", 0.35),
];

/// The glass parts of these models are exported as blended but with no
/// alpha anywhere, so they drew as opaque metal in the transparent pass:
/// (file, glTF material index). They become thin, glossy, see-through glass.
const GLASS: &[(&str, usize)] = &[
    ("models/Lantern_01/Lantern_01.gltf", 1),
    ("models/old_military_compressor/old_military_compressor.gltf", 1),
    ("models/street_lamp_01/street_lamp_01.gltf", 1),
    ("models/portable_generator/portable_generator.gltf", 1),
];

pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherCache>()
            .add_systems(PreStartup, load_library)
            .add_systems(Update, (build_pieces, fix_glass))
            .add_observer(weather_scene);
    }
}

fn load_library(mut commands: Commands, server: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let scenes = SCENES.iter().map(|&id| (id, server.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{id}/{id}.gltf"))))).collect();
    let kits = KITS.iter().map(|&id| (id, server.load(format!("models/{id}/{id}.gltf")))).collect();
    let surfaces = SURFACES
        .iter()
        .map(|&(id, [r, g, b], metallic)| {
            let dir = format!("textures/{id}");
            let t = |name: &str, srgb: bool| Some(crate::assets::tiled(&server, format!("{dir}/{name}"), srgb));
            let m = StandardMaterial {
                base_color: Color::srgb(r, g, b),
                base_color_texture: t("diff.jpg", true),
                normal_map_texture: t("nor.jpg", false),
                metallic_roughness_texture: t("arm.jpg", false),
                occlusion_texture: t("arm.jpg", false),
                metallic,
                perceptual_roughness: 1.0,
                ..default()
            };
            (id, materials.add(m))
        })
        .collect();
    commands.insert_resource(Library { scenes, kits, surfaces });
}

/// Fill in kit pieces whose file has finished loading.
fn build_pieces(
    mut commands: Commands,
    pieces: Query<(Entity, &KitPiece)>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    for (e, piece) in &pieces {
        let Some(gltf) = gltfs.get(&piece.gltf) else { continue };
        commands.entity(e).remove::<KitPiece>();
        let Some(node) = gltf.named_nodes.get(piece.node).and_then(|h| nodes.get(h)) else {
            warn!("kit piece {} not found", piece.node);
            continue;
        };
        // Keep the node's turn and scale, not where it sat in the kit.
        let root = Transform { translation: Vec3::ZERO, ..node.transform };
        commands.entity(e).with_children(|c| spawn_node(c, node, root, &nodes, &gltf_meshes));
    }
}

fn spawn_node(c: &mut ChildSpawnerCommands, node: &GltfNode, tf: Transform, nodes: &Assets<GltfNode>, gltf_meshes: &Assets<GltfMesh>) {
    c.spawn((tf, Visibility::default())).with_children(|n| {
        if let Some(mesh) = node.mesh.as_ref().and_then(|h| gltf_meshes.get(h)) {
            for p in &mesh.primitives {
                let material = p.material.clone().unwrap_or_default();
                n.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(material)));
            }
        }
        for child in node.children.iter().filter_map(|h| nodes.get(h)) {
            spawn_node(n, child, child.transform, nodes, gltf_meshes);
        }
    });
}


/// Weathered copies of materials, so a hundred boulders share one.
#[derive(Resource, Default)]
struct WeatherCache(HashMap<(AssetId<StandardMaterial>, [u32; 5]), Handle<RockSnowMaterial>>);

fn weather_scene(
    trigger: Trigger<bevy::scene::SceneInstanceReady>,
    mut commands: Commands,
    weathered: Query<&Weathered>,
    children: Query<&Children>,
    mats: Query<&MeshMaterial3d<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
    mut rock_materials: ResMut<Assets<RockSnowMaterial>>,
    mut cache: ResMut<WeatherCache>,
) {
    let root = trigger.target();
    let Ok(&Weathered { tint: t, snow, grain }) = weathered.get(root) else { return };
    let key = [t[0], t[1], t[2], snow, grain].map(f32::to_bits);
    for e in children.iter_descendants(root) {
        let Ok(m) = mats.get(e) else { continue };
        let id = m.0.id();
        let handle = match cache.0.get(&(id, key)) {
            Some(h) => h.clone(),
            None => {
                let Some(base) = materials.get(id).cloned() else {
                    warn!("weathering: material not loaded yet");
                    continue;
                };
                let c = base.base_color.to_linear();
                let base = StandardMaterial { base_color: LinearRgba::new(c.red * t[0], c.green * t[1], c.blue * t[2], c.alpha).into(), ..base };
                let h = rock_materials.add(RockSnowMaterial { base, extension: RockSnowExt { snow: ROCK_SNOW.extend(snow), detail: Vec4::new(grain, 0.0, 0.0, 0.0) } });
                cache.0.insert((id, key), h.clone());
                h
            }
        };
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle));
    }
}

/// Turns the mis-exported glass (see [`GLASS`]) into glass as it loads.
fn fix_glass(mut events: EventReader<AssetEvent<StandardMaterial>>, server: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for event in events.read() {
        let AssetEvent::Added { id } = event else { continue };
        let Some(path) = server.get_path(*id) else { continue };
        let Some(label) = path.label() else { continue };
        let file = path.path().to_string_lossy().replace('\\', "/");
        let glass = GLASS.iter().any(|&(f, i)| file == f && (label == format!("Material{i}") || label == format!("Material{i} (inverted)")));
        if !glass {
            continue;
        }
        if let Some(m) = materials.get_mut(*id) {
            m.alpha_mode = AlphaMode::Blend;
            m.base_color = m.base_color.with_alpha(0.22);
            m.perceptual_roughness = 0.08;
            m.metallic = 0.0;
            m.metallic_roughness_texture = None;
            m.reflectance = 0.5;
        }
    }
}
