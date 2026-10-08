//! The furniture and set-dressing library: the CC0 models and surfaces
//! fetched by `tools/fetch_assets.py` for Milestone 10, looked up by their
//! Poly Haven / ambientCG id. Some models are kits (a dozen duct or pipe
//! pieces laid out side by side in one file); `Library::piece` spawns just
//! one of them, by node name, once the file has loaded.

use std::collections::HashMap;

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

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

pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_library).add_systems(Update, build_pieces);
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

