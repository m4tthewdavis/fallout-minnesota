//! Turns the procedural geometry from `sim::meshgen` into Bevy meshes.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::sim::meshgen::{MeshData, WHITE};

/// A Bevy mesh with positions, normals, UVs, triangle indices and (when any
/// vertex isn't white) vertex colours.
pub fn to_mesh(d: &MeshData) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, d.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, d.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, d.uvs.clone())
        .with_inserted_indices(Indices::U32(d.indices.clone()));
    if d.colors.iter().any(|c| *c != WHITE) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, d.colors.clone());
    }
    mesh
}

/// Like [`to_mesh`], plus tangents so normal-mapped materials light correctly.
pub fn to_mesh_tangents(d: &MeshData) -> Mesh {
    let mut mesh = to_mesh(d);
    if let Err(e) = mesh.generate_tangents() {
        warn!("could not generate tangents: {e}");
    }
    mesh
}
