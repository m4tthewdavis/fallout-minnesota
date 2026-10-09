//! Plants the woods: a mixed Minnesota forest (conifer groves of balsam fir,
//! white spruce and white and red pine; stands of paper birch and clonal
//! quaking aspen; tamarack and spruce down by the lakes), plus red osier
//! dogwood, staghorn sumac, juniper and prairie grass in the open, and
//! cattails and reeds round the lake shores. Shapes come from
//! [`crate::sim::flora`].

use std::collections::HashMap;

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::{tiled, GameAssets};
use crate::meshes::{to_mesh, to_mesh_tangents};
use crate::sim::collision::{self, Shape};
use crate::sim::flora::{self, Bark, Card, Species};
use crate::sim::meshgen::MeshData;
use crate::sim::rng::Rng;
use crate::sim::terrain::{self, HALF_SIZE, ICE_FRACTION, ICE_LEVEL, LAKES};
use crate::sim::weather::WIND_DIR;
use crate::world::{ground, spawn_contact_shadow, spawn_drift};

const TREES: usize = 300;

/// Beyond this distance (metres) a tree swaps to its cheap model. Shadows and
/// the draw cutoff follow the player's settings. (A little hysteresis keeps
/// trees at an edge from flickering.)
pub const TREE_FAR: f32 = 60.0;
const TREE_HYSTERESIS: f32 = 4.0;

/// A tree that switches between detailed and cheap models by distance.
#[derive(Component)]
struct TreeLod {
    bark: Entity,
    foliage: Entity,
    near: (Handle<Mesh>, Handle<Mesh>),
    far: (Handle<Mesh>, Handle<Mesh>),
    far_now: bool,
    /// Currently casting shadows.
    shadowed: bool,
    /// Currently hidden because it's past the view distance.
    hidden: bool,
}

/// Is `distance` beyond `limit`, given whether it was already beyond it? Near
/// the limit the answer sticks, so trees don't flicker back and forth.
pub fn beyond(distance: f32, limit: f32, was_beyond: bool) -> bool {
    if was_beyond {
        distance > limit - TREE_HYSTERESIS
    } else {
        distance > limit + TREE_HYSTERESIS
    }
}

/// Whether a tree at `distance` should be in its far state, given its current one.
pub fn is_far(distance: f32, was_far: bool) -> bool {
    beyond(distance, TREE_FAR, was_far)
}

pub struct FloraPlugin;

impl Plugin for FloraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tree_lod);
    }
}

/// Four times a second, move trees between their near and far versions, and
/// apply the shadow and view-distance settings.
fn tree_lod(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<crate::menu::GameSettings>,
    mut since: Local<f32>,
    cam: Query<&GlobalTransform, With<crate::player::Player>>,
    mut trees: Query<(&GlobalTransform, &mut TreeLod, &mut Visibility)>,
) {
    *since += time.delta_secs();
    // A settings change should show at once, not a quarter-second later.
    if *since < 0.25 && !settings.is_changed() {
        return;
    }
    *since = 0.0;
    let Ok(cam) = cam.single() else { return };
    let eye = cam.translation();
    let shadow_limit = settings.0.shadows.tree_shadow_distance();
    let cull = settings.0.view.tree_cull();
    for (tf, mut tree, mut vis) in &mut trees {
        let d = (tf.translation() - eye).xz().length();
        // Past the view distance a tree isn't drawn at all.
        let hidden = cull.is_some_and(|limit| beyond(d, limit, tree.hidden));
        if hidden != tree.hidden {
            tree.hidden = hidden;
            *vis = if hidden { Visibility::Hidden } else { Visibility::Inherited };
        }
        let far = is_far(d, tree.far_now);
        let shadowed = !beyond(d, shadow_limit, !tree.shadowed);
        if far == tree.far_now && shadowed == tree.shadowed {
            continue;
        }
        tree.far_now = far;
        tree.shadowed = shadowed;
        let (bark, foliage) = if far { tree.far.clone() } else { tree.near.clone() };
        for (part, mesh) in [(tree.bark, bark), (tree.foliage, foliage)] {
            let mut e = commands.entity(part);
            e.insert(Mesh3d(mesh));
            if shadowed {
                e.remove::<NotShadowCaster>();
            } else {
                e.insert(NotShadowCaster);
            }
        }
    }
}

/// Every tree shape and plant material, kept for screenshot lineups.
#[derive(Resource, Clone)]
pub struct FloraKit {
    models: HashMap<Species, Vec<TreeModel>>,
    mats: Materials,
}
const VARIANTS: usize = 3;

#[derive(Clone)]
struct TreeModel {
    bark: Handle<Mesh>,
    foliage: Handle<Mesh>,
    /// Cheaper meshes for when the tree is far away.
    bark_far: Handle<Mesh>,
    foliage_far: Handle<Mesh>,
    snow: Option<Handle<Mesh>>,
    height: f32,
}

#[derive(Clone)]
struct Materials {
    cards: HashMap<Card, Handle<StandardMaterial>>,
    barks: HashMap<Bark, Handle<StandardMaterial>>,
    snow: Handle<StandardMaterial>,
    plain: Handle<StandardMaterial>,
    grass: Handle<StandardMaterial>,
    plume: Handle<StandardMaterial>,
}

fn card_material(materials: &mut Assets<StandardMaterial>, server: &AssetServer, file: &str, base: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: base,
        base_color_texture: Some(server.load(format!("textures/generated/{file}"))),
        alpha_mode: AlphaMode::Mask(0.35),
        perceptual_roughness: 0.85,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

fn make_materials(materials: &mut Assets<StandardMaterial>, server: &AssetServer, assets: &GameAssets) -> Materials {
    let mut cards = HashMap::new();
    for (card, file) in [(Card::Pine, "spray_pine.png"), (Card::Fir, "spray_fir_photo.png"), (Card::Twigs, "twigs.png"), (Card::Tamarack, "spray_tamarack.png")] {
        cards.insert(card, card_material(materials, server, file, Color::WHITE));
    }
    let mut barks = HashMap::new();
    barks.insert(Bark::Pine, assets.bark.clone());
    barks.insert(
        Bark::RedPine,
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.7, 0.58),
            base_color_texture: Some(tiled(server, "textures/pine_bark/diff.jpg".into(), true)),
            normal_map_texture: Some(tiled(server, "textures/pine_bark/nor.jpg".into(), false)),
            perceptual_roughness: 0.95,
            ..default()
        }),
    );
    barks.insert(
        Bark::Birch,
        materials.add(StandardMaterial {
            base_color_texture: Some(tiled(server, "textures/generated/bark_birch.jpg".into(), true)),
            normal_map_texture: Some(tiled(server, "textures/generated/bark_birch_nor.jpg".into(), false)),
            perceptual_roughness: 0.8,
            ..default()
        }),
    );
    barks.insert(
        Bark::Aspen,
        materials.add(StandardMaterial {
            base_color_texture: Some(tiled(server, "textures/generated/bark_aspen.jpg".into(), true)),
            perceptual_roughness: 0.7,
            ..default()
        }),
    );
    Materials {
        cards,
        barks,
        snow: card_material(materials, server, "snow_clumps.png", Color::srgb(0.93, 0.95, 0.99)),
        // Vertex-coloured stems, cones and cattail leaves (seen from both sides).
        plain: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.8,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        grass: card_material(materials, server, "grass_tuft.png", Color::WHITE),
        plume: card_material(materials, server, "reed_plume.png", Color::WHITE),
    }
}

fn mesh_or_none(meshes: &mut Assets<Mesh>, m: &MeshData) -> Option<Handle<Mesh>> {
    (!m.positions.is_empty()).then(|| meshes.add(to_mesh(m)))
}

/// Distance from (x, z) to the nearest lake shore, scaled so 1 is "far".
fn near_water(x: f32, z: f32) -> f32 {
    LAKES.iter().map(|&(lx, lz, r)| (((x - lx).hypot(z - lz) - r) / 30.0).clamp(0.0, 1.0)).fold(1.0, f32::min)
}

/// Is (x, z) somewhere a plant can go?
fn free(x: f32, z: f32, clearance: f32, solid: &[Shape], avoid: &dyn Fn(f32, f32) -> bool) -> bool {
    terrain::is_open_ground(x, z) && !avoid(x, z) && !collision::blocked(x, z, clearance, solid)
}

pub fn plant_forest(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    server: &AssetServer,
    assets: &GameAssets,
    solid: &mut Vec<Shape>,
    trees_out: &mut Vec<(f32, f32)>,
    avoid: &dyn Fn(f32, f32) -> bool,
    seed: u64,
) {
    let mut rng = Rng::new(seed);
    let mats = make_materials(materials, server, assets);

    // A few shapes of each species, reused at different sizes and turns.
    let mut models: HashMap<Species, Vec<TreeModel>> = HashMap::new();
    for (si, species) in Species::ALL.into_iter().enumerate() {
        let (lo, hi) = species.heights();
        let list = (0..VARIANTS)
            .map(|v| {
                let height = lo + (hi - lo) * (v as f32 + 0.5) / VARIANTS as f32;
                let seed = 1000 + si as u64 * 17 + v as u64;
                let t = flora::tree(species, height, 0.9, seed);
                let far = flora::tree_lod(species, height, 0.9, seed);
                TreeModel {
                    bark: meshes.add(to_mesh_tangents(&t.bark)),
                    foliage: meshes.add(to_mesh(&t.foliage)),
                    bark_far: meshes.add(to_mesh_tangents(&far.bark)),
                    foliage_far: meshes.add(to_mesh(&far.foliage)),
                    snow: mesh_or_none(meshes, &t.snow),
                    height,
                }
            })
            .collect();
        models.insert(species, list);
    }
    commands.insert_resource(FloraKit { models: models.clone(), mats: mats.clone() });

    let plant = |commands: &mut Commands, meshes: &mut Assets<Mesh>, rng: &mut Rng, solid: &mut Vec<Shape>, trees_out: &mut Vec<(f32, f32)>, species: Species, x: f32, z: f32| {
        let list = &models[&species];
        let model = &list[(rng.f32() * list.len() as f32) as usize % list.len()];
        let (lo, hi) = species.heights();
        let scale = rng.range(lo, hi) / model.height;
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let tf = Transform::from_xyz(x, terrain::mesh_height(x, z) - 0.05, z)
            .with_scale(Vec3::splat(scale))
            .with_rotation(Quat::from_rotation_y(yaw));
        let mut parts = None;
        let tree = commands
            .spawn((tf, Visibility::default()))
            .with_children(|t| {
                let bark = t.spawn((Mesh3d(model.bark.clone()), MeshMaterial3d(mats.barks[&species.bark()].clone()))).id();
                let foliage = t.spawn((Mesh3d(model.foliage.clone()), MeshMaterial3d(mats.cards[&species.card()].clone()))).id();
                if let Some(snow) = &model.snow {
                    t.spawn((Mesh3d(snow.clone()), MeshMaterial3d(mats.snow.clone()), NotShadowCaster));
                }
                parts = Some((bark, foliage));
            })
            .id();
        let (bark, foliage) = parts.expect("tree parts are spawned above");
        commands.entity(tree).insert(TreeLod {
            bark,
            foliage,
            near: (model.bark.clone(), model.foliage.clone()),
            far: (model.bark_far.clone(), model.foliage_far.clone()),
            far_now: false,
            shadowed: true,
            hidden: false,
        });
        let trunk = species.trunk_radius(model.height) * scale;
        solid.push(Shape::Circle { x, z, r: trunk + 0.15 });
        trees_out.push((x, z));
        let shade = (model.height * scale * 0.12).clamp(0.8, 2.2);
        spawn_contact_shadow(commands, meshes, assets, x, z, shade, shade, yaw);
        if rng.chance(0.35) {
            let d = 0.5 + trunk * 3.0;
            spawn_drift(commands, meshes, assets, x + WIND_DIR[0] * d, z + WIND_DIR[1] * d, 2.6, 1.5, 0.3, WIND_DIR, (x * 31.0 + z) as u64);
        }
    };

    // ---- Trees ----
    let mut placed = 0;
    let mut tries = 0;
    while placed < TREES && tries < 4000 {
        tries += 1;
        let (x, z) = (rng.range(-HALF_SIZE, HALF_SIZE), rng.range(-HALF_SIZE, HALF_SIZE));
        if !free(x, z, 1.2, solid, avoid) {
            continue;
        }
        let species = flora::pick_species(x, z, near_water(x, z), rng.f32());
        plant(commands, meshes, &mut rng, solid, trees_out, species, x, z);
        placed += 1;
        // Birch grows in small stands and aspen in clonal groves.
        let extra = match species {
            Species::QuakingAspen => 3 + (rng.f32() * 5.0) as usize,
            Species::PaperBirch => 1 + (rng.f32() * 3.0) as usize,
            Species::BalsamFir | Species::WhiteSpruce => (rng.f32() * 3.0) as usize,
            _ => 0,
        };
        for _ in 0..extra {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = rng.range(2.5, 6.0);
            let (sx, sz) = (x + a.cos() * d, z + a.sin() * d);
            if placed < TREES + 60 && free(sx, sz, 1.0, solid, avoid) {
                plant(commands, meshes, &mut rng, solid, trees_out, species, sx, sz);
                placed += 1;
            }
        }
    }

    // ---- Undergrowth ----
    let spot = |rng: &mut Rng, solid: &[Shape]| -> Option<(f32, f32)> {
        for _ in 0..30 {
            let (x, z) = (rng.range(-HALF_SIZE, HALF_SIZE), rng.range(-HALF_SIZE, HALF_SIZE));
            if free(x, z, 0.6, solid, avoid) {
                return Some((x, z));
            }
        }
        None
    };
    let dogwoods: Vec<Handle<Mesh>> = (0..4).map(|k| meshes.add(to_mesh(&flora::dogwood(70 + k)))).collect();
    let sumacs: Vec<(Handle<Mesh>, Handle<Mesh>)> = (0..3)
        .map(|k| {
            let (s, c) = flora::sumac(80 + k);
            (meshes.add(to_mesh(&s)), meshes.add(to_mesh(&c)))
        })
        .collect();
    let junipers: Vec<(Handle<Mesh>, Handle<Mesh>)> = (0..3)
        .map(|k| {
            let (f, s) = flora::juniper(90 + k);
            (meshes.add(to_mesh(&f)), meshes.add(to_mesh(&s)))
        })
        .collect();
    let tufts: Vec<Handle<Mesh>> = (0..6).map(|k| meshes.add(to_mesh(&flora::grass_tuft(100 + k)))).collect();
    let fir_cards = mats.cards[&Card::Fir].clone();

    let place = |rng: &mut Rng, x: f32, z: f32, s: f32| {
        Transform::from_xyz(x, ground(x, z), z)
            .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU)))
            .with_scale(Vec3::splat(s))
    };
    for k in 0..45 {
        // Dogwood likes damp ground: half of them go near water.
        let pos = if k % 2 == 0 {
            let (lx, lz, r) = LAKES[k % LAKES.len()];
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = r + rng.range(1.0, 9.0);
            let (x, z) = (lx + a.cos() * d, lz + a.sin() * d);
            free(x, z, 0.6, solid, avoid).then_some((x, z))
        } else {
            spot(&mut rng, solid)
        };
        let Some((x, z)) = pos else { continue };
        let sc = rng.range(0.8, 1.2);
        let tf = place(&mut rng, x, z, sc);
        commands.spawn((Mesh3d(dogwoods[k % dogwoods.len()].clone()), MeshMaterial3d(mats.plain.clone()), tf));
    }
    for k in 0..30 {
        let Some((x, z)) = spot(&mut rng, solid) else { continue };
        let sc = rng.range(0.8, 1.3);
        let tf = place(&mut rng, x, z, sc);
        let (stems, cones) = &sumacs[k % sumacs.len()];
        commands.spawn((tf, Visibility::default())).with_children(|p| {
            p.spawn((Mesh3d(stems.clone()), MeshMaterial3d(mats.plain.clone())));
            p.spawn((Mesh3d(cones.clone()), MeshMaterial3d(mats.plain.clone())));
        });
    }
    for k in 0..35 {
        let Some((x, z)) = spot(&mut rng, solid) else { continue };
        let sc = rng.range(0.8, 1.5);
        let tf = place(&mut rng, x, z, sc);
        let (foliage, snow) = &junipers[k % junipers.len()];
        commands.spawn((tf, Visibility::default())).with_children(|p| {
            p.spawn((Mesh3d(foliage.clone()), MeshMaterial3d(fir_cards.clone())));
            p.spawn((Mesh3d(snow.clone()), MeshMaterial3d(mats.snow.clone()), NotShadowCaster));
        });
        solid.push(Shape::Circle { x, z, r: 0.6 });
    }
    // Prairie grass in patches on the open ground.
    for _ in 0..70 {
        let Some((cx, cz)) = spot(&mut rng, solid) else { continue };
        for _ in 0..(3 + (rng.f32() * 6.0) as usize) {
            let (x, z) = (cx + rng.range(-2.5, 2.5), cz + rng.range(-2.5, 2.5));
            if !terrain::is_open_ground(x, z) || avoid(x, z) {
                continue;
            }
            let sc = rng.range(0.7, 1.3);
        let tf = place(&mut rng, x, z, sc);
            commands.spawn((Mesh3d(tufts[(rng.f32() * 6.0) as usize % 6].clone()), MeshMaterial3d(mats.grass.clone()), tf, NotShadowCaster));
        }
    }

    // ---- Cattails and reeds round every lake ----
    let cattail_set: Vec<(Handle<Mesh>, Handle<Mesh>)> = (0..4)
        .map(|k| {
            let (s, h) = flora::cattails(120 + k);
            (meshes.add(to_mesh(&s)), meshes.add(to_mesh(&h)))
        })
        .collect();
    let reed_set: Vec<(Handle<Mesh>, Handle<Mesh>)> = (0..3)
        .map(|k| {
            let (s, p) = flora::reeds(130 + k);
            (meshes.add(to_mesh(&s)), meshes.add(to_mesh(&p)))
        })
        .collect();
    for (li, &(lx, lz, r)) in LAKES.iter().enumerate() {
        let shore = r * ICE_FRACTION;
        let count = (std::f32::consts::TAU * shore / 3.5) as usize;
        for k in 0..count {
            if !rng.chance(0.55) {
                continue;
            }
            let a = k as f32 / count as f32 * std::f32::consts::TAU + rng.range(-0.05, 0.05);
            let d = shore + rng.range(0.2, 1.4);
            let (x, z) = (lx + a.cos() * d, lz - a.sin() * d);
            if collision::blocked(x, z, 0.5, solid) || avoid(x, z) {
                continue;
            }
            let y = terrain::mesh_height(x, z).max(ICE_LEVEL);
            let tf = Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU))).with_scale(Vec3::splat(rng.range(0.85, 1.15)));
            let (stalks, tops, top_mat) = if rng.chance(0.7) {
                let (s, h) = &cattail_set[(li + k) % cattail_set.len()];
                (s, h, &mats.plain)
            } else {
                let (s, p) = &reed_set[(li + k) % reed_set.len()];
                (s, p, &mats.plume)
            };
            commands.spawn((tf, Visibility::default())).with_children(|p| {
                p.spawn((Mesh3d(stalks.clone()), MeshMaterial3d(mats.plain.clone())));
                p.spawn((Mesh3d(tops.clone()), MeshMaterial3d(top_mat.clone())));
            });
        }
    }
}

/// Screenshot mode (`FMN_LINEUP=trees`): one of each tree in a row in front
/// of the player's spawn point, biggest variant, for comparing species.
pub fn spawn_lineup(mut commands: Commands, kit: Res<FloraKit>, mut meshes: ResMut<Assets<Mesh>>) {
    let (sx, sz) = terrain::PLAYER_SPAWN;
    // Undergrowth in a closer row.
    let (cs, ch) = flora::cattails(120);
    let (rs, rp) = flora::reeds(130);
    let (ss, sc) = flora::sumac(80);
    let (jf, js) = flora::juniper(90);
    let rows: [(MeshData, &Handle<StandardMaterial>); 10] = [
        (flora::dogwood(70), &kit.mats.plain),
        (ss, &kit.mats.plain),
        (sc, &kit.mats.plain),
        (jf, &kit.mats.cards[&Card::Fir]),
        (js, &kit.mats.snow),
        (flora::grass_tuft(100), &kit.mats.grass),
        (cs, &kit.mats.plain),
        (ch, &kit.mats.plain),
        (rs, &kit.mats.plain),
        (rp, &kit.mats.plume),
    ];
    let slots = [0, 1, 1, 2, 2, 3, 4, 4, 5, 5];
    for ((m, mat), slot) in rows.into_iter().zip(slots) {
        let (x, z) = (sx - 5.0 + slot as f32 * 2.0, sz - 6.0);
        commands.spawn((Mesh3d(meshes.add(to_mesh(&m))), MeshMaterial3d(mat.clone()), Transform::from_xyz(x, terrain::mesh_height(x, z), z)));
    }
    for (i, species) in Species::ALL.into_iter().enumerate() {
        let x = sx - 18.0 + i as f32 * 6.0;
        let z = sz - 16.0 - (i % 2) as f32 * 3.0;
        let model = &kit.models[&species][VARIANTS - 1];
        commands.spawn((Transform::from_xyz(x, terrain::mesh_height(x, z) - 0.05, z), Visibility::default())).with_children(|t| {
            t.spawn((Mesh3d(model.bark.clone()), MeshMaterial3d(kit.mats.barks[&species.bark()].clone())));
            t.spawn((Mesh3d(model.foliage.clone()), MeshMaterial3d(kit.mats.cards[&species.card()].clone())));
            if let Some(snow) = &model.snow {
                t.spawn((Mesh3d(snow.clone()), MeshMaterial3d(kit.mats.snow.clone())));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_switch_at_sixty_metres_with_hysteresis() {
        assert!(!is_far(30.0, false));
        assert!(is_far(100.0, false));
        assert!(!is_far(TREE_FAR + 1.0, false), "stays near until clearly past the line");
        assert!(is_far(TREE_FAR + 1.0, true), "and stays far until clearly inside it");
        assert!(!is_far(TREE_FAR - 10.0, true));
    }

    #[test]
    fn the_cutoff_sticks_near_its_edge_for_any_limit() {
        for limit in [30.0, 60.0, 140.0] {
            assert!(!beyond(limit - 10.0, limit, false));
            assert!(beyond(limit + 10.0, limit, false));
            assert!(beyond(limit + 1.0, limit, true), "already beyond: stays beyond just inside the line");
            assert!(!beyond(limit + 1.0, limit, false), "inside: stays inside just past the line");
        }
        assert!(beyond(10.0, 0.0, false), "a limit of zero (shadows off) puts every nearby tree beyond it");
    }
}
