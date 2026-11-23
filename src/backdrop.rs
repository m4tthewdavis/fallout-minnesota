//! The land beyond the map (shapes and rules in [`crate::sim::backdrop`]):
//! a ring of snow-covered hills and lake ice that carries the terrain on to
//! the horizon, and the woods standing on it, drawn as flat pictures of
//! whole trees. None of it can be reached; it is there to be looked at.

use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::sim::backdrop::{self, Card, BANDS, SECTORS};
use crate::sim::settings::ViewDistance;
use crate::state::CurrentInterior;

/// The seed the backdrop's woods grow from (its own: nothing on the map moves).
const SEED: u64 = 1949;

/// A piece of the backdrop: which belt of trees (the ground is belt 0).
#[derive(Component)]
struct Backdrop {
    band: u8,
}

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_backdrop.after(crate::state::WorldGen)).add_systems(Update, show_backdrop);
    }
}

/// How many belts of trees are drawn at a view distance: the far ones are
/// lost in the haze first.
pub fn bands_shown(view: ViewDistance) -> u8 {
    match view {
        ViewDistance::Near => 1,
        ViewDistance::Medium => 2,
        ViewDistance::Far => BANDS.len() as u8,
    }
}

fn spawn_backdrop(
    mut commands: Commands,
    assets: Res<GameAssets>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The ground: the terrain's own snow material, so the join doesn't show.
    commands.spawn((
        Mesh3d(meshes.add(to_mesh_tangents(&backdrop::skirt_mesh()))),
        MeshMaterial3d(assets.snow_ground.clone()),
        Transform::default(),
        NotShadowCaster,
        Backdrop { band: 0 },
    ));

    // The woods. Not double-sided: both faces of a card are lit by the one
    // normal it was given, the side of the tree that faces the map.
    let trees = materials.add(StandardMaterial {
        base_color_texture: Some(server.load("textures/generated/tree_cards.png")),
        alpha_mode: AlphaMode::Mask(0.4),
        perceptual_roughness: 0.95,
        reflectance: 0.2,
        cull_mode: None,
        ..default()
    });
    let cards = backdrop::tree_cards(SEED);
    for band in 0..BANDS.len() as u8 {
        let mut slices: Vec<Vec<Card>> = vec![Vec::new(); SECTORS];
        for c in cards.iter().filter(|c| c.band == band) {
            slices[backdrop::sector(c.x, c.z)].push(*c);
        }
        for slice in slices.iter().filter(|s| !s.is_empty()) {
            commands.spawn((
                Mesh3d(meshes.add(to_mesh(&backdrop::cards_mesh(slice, band == 0)))),
                MeshMaterial3d(trees.clone()),
                Transform::default(),
                NotShadowCaster,
                NotShadowReceiver,
                Backdrop { band },
            ));
        }
    }
}

/// Hidden indoors (the rooms have no windows on it), and thinned to the
/// view distance setting.
fn show_backdrop(settings: Res<crate::menu::GameSettings>, interior: Res<CurrentInterior>, mut parts: Query<(&Backdrop, &mut Visibility)>, added: Query<(), Added<Backdrop>>) {
    if !settings.is_changed() && !interior.is_changed() && added.is_empty() {
        return;
    }
    let shown = bands_shown(settings.0.view);
    for (part, mut vis) in &mut parts {
        let on = interior.0.is_none() && part.band < shown;
        vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_longer_view_shows_more_of_the_woods() {
        assert!(bands_shown(ViewDistance::Near) >= 1, "the nearest belt hides the map's edge at every setting");
        assert!(bands_shown(ViewDistance::Near) < bands_shown(ViewDistance::Medium));
        assert!(bands_shown(ViewDistance::Medium) < bands_shown(ViewDistance::Far));
        assert_eq!(bands_shown(ViewDistance::Far) as usize, BANDS.len());
    }
}
