//! The winter clothes the game's people wear: a quilted parka over a belt,
//! fur-trimmed hood, cuffs and hem, knit hats, mittens, boots, packs, scarves
//! and a dusting of frost. Survivors and raiders share the same pieces and
//! differ in colours, headgear and gear. Shapes are in `sim::outfit`; the
//! cloth, fur and leather are real scanned textures (Poly Haven, CC0).
//!
//! Everybody is built standing at the origin facing +Z. Raiders animate their
//! legs and arms, so legs and arms are dressed on their own pivots.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::meshes::to_mesh_tangents;
use crate::sim::outfit;

/// What's on a person's head.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hat {
    /// Just the parka's hood, fur-trimmed.
    Hood,
    Beanie,
    /// A trapper hat with ear flaps and a fur band.
    EarFlap,
    /// A balaclava with goggles.
    Balaclava,
}

/// One person's outfit: colours and gear.
#[derive(Clone)]
pub struct Look {
    pub parka: Handle<StandardMaterial>,
    pub fur: Handle<StandardMaterial>,
    pub knit: Handle<StandardMaterial>,
    pub hat: Hat,
    pub pack: bool,
    pub bandolier: bool,
    /// How many patches of frost cling to the shoulders, hood and pack.
    pub frost: usize,
}

#[derive(Resource)]
pub struct PersonKit {
    parka: Handle<Mesh>,
    belt: Handle<Mesh>,
    quilting: Handle<Mesh>,
    hem_fur: Handle<Mesh>,
    hood: Handle<Mesh>,
    hood_fur: Handle<Mesh>,
    cuff_fur: Handle<Mesh>,
    beanie: Handle<Mesh>,
    earflap: Handle<Mesh>,
    balaclava: Handle<Mesh>,
    scarf: Handle<Mesh>,
    mitten: Handle<Mesh>,
    boot: Handle<Mesh>,
    pack: Handle<Mesh>,
    bandolier: Handle<Mesh>,
    frost: Handle<Mesh>,
    head: Handle<Mesh>,
    goggles: Handle<Mesh>,
    eye: Handle<Mesh>,
    nose: Handle<Mesh>,
    sleeve: Handle<Mesh>,
    leg: Handle<Mesh>,
    skin: Handle<StandardMaterial>,
    pants: Handle<StandardMaterial>,
    leather: Handle<StandardMaterial>,
    canvas: Handle<StandardMaterial>,
    rime: Handle<StandardMaterial>,
    lens: Handle<StandardMaterial>,
    /// The four fish-house survivors' outfits, by house.
    pub survivors: [Look; 4],
    /// The raiders' outfits.
    pub raiders: [Look; 3],
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_kit.before(crate::state::WorldGen));
    }
}

/// A tinted copy of one of the shared cloth materials.
fn tinted(materials: &mut Assets<StandardMaterial>, base: &Handle<StandardMaterial>, tint: Color, rough: f32) -> Handle<StandardMaterial> {
    match materials.get(base).cloned() {
        Some(m) => materials.add(StandardMaterial { base_color: tint, perceptual_roughness: rough, ..m }),
        None => materials.add(StandardMaterial { base_color: tint, perceptual_roughness: rough, ..default() }),
    }
}

/// Multiply a colour's channels (a tint correction).
fn scale(c: Color, k: [f32; 3]) -> Color {
    let l = c.to_linear();
    Color::linear_rgb(l.red * k[0], l.green * k[1], l.blue * k[2])
}

fn build_kit(mut commands: Commands, assets: Res<GameAssets>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut m = |d: crate::sim::meshgen::MeshData| meshes.add(to_mesh_tangents(&d));
    let rime = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.97, 1.0, 0.85),
        base_color_texture: assets.gun_textures.get("rime").cloned(),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.45,
        emissive: LinearRgba::rgb(0.08, 0.11, 0.14),
        ..default()
    });
    let c = |r: f32, g: f32, b: f32| Color::srgb(r, g, b);
    // A parka, its fur trim and a knit accessory for each person.
    let look = |materials: &mut Assets<StandardMaterial>, parka: Color, fur: Color, knit: Color, hat: Hat, pack: bool, bandolier: bool, frost: usize| Look {
        // The scanned cloth is beige, so each tint is first corrected towards neutral
        // (and the dark wool lifted) to land on the colour asked for.
        parka: tinted(materials, &assets.fleece, scale(parka, [0.84, 1.01, 1.33]), 0.9),
        fur: tinted(materials, &assets.fur, scale(fur, [0.92, 1.01, 1.15]), 0.95),
        knit: tinted(materials, &assets.wool, scale(knit, [1.4, 1.5, 1.7]), 0.97),
        hat,
        pack,
        bandolier,
        frost,
    };
    let survivors = [
        look(&mut materials, c(0.95, 0.5, 0.2), c(0.95, 0.9, 0.8), c(0.7, 0.15, 0.12), Hat::EarFlap, false, false, 2),
        look(&mut materials, c(0.35, 0.5, 0.3), c(0.55, 0.42, 0.3), c(0.85, 0.8, 0.65), Hat::Beanie, true, false, 3),
        look(&mut materials, c(0.28, 0.42, 0.7), c(0.8, 0.82, 0.85), c(0.85, 0.7, 0.2), Hat::Hood, false, false, 2),
        look(&mut materials, c(0.65, 0.55, 0.38), c(0.97, 0.97, 0.95), c(0.3, 0.3, 0.35), Hat::Beanie, true, false, 3),
    ];
    // Raiders: dirty white and grey parkas, ragged dark fur, caked with frost.
    let raiders = [
        look(&mut materials, c(0.78, 0.8, 0.82), c(0.3, 0.27, 0.24), c(0.5, 0.12, 0.1), Hat::Balaclava, true, true, 5),
        look(&mut materials, c(0.62, 0.66, 0.7), c(0.42, 0.38, 0.33), c(0.12, 0.25, 0.45), Hat::Hood, false, true, 6),
        look(&mut materials, c(0.7, 0.72, 0.66), c(0.25, 0.23, 0.22), c(0.7, 0.55, 0.12), Hat::EarFlap, true, false, 5),
    ];
    commands.insert_resource(PersonKit {
        parka: m(outfit::parka_body()),
        belt: m(outfit::belt()),
        quilting: m(outfit::quilting()),
        hem_fur: m(outfit::fur_ring(0.31, 0.042, 18, 2, false).translated([0.0, 0.67, 0.0]).scaled([1.0, 1.0, 0.78])),
        hood: m(outfit::hood_shell()),
        hood_fur: m(outfit::fur_ring(0.145, 0.04, 14, 1, true).translated([0.0, 1.66, 0.12])),
        cuff_fur: m(outfit::fur_ring(0.08, 0.03, 8, 3, false)),
        beanie: m(outfit::beanie()),
        earflap: m(outfit::earflap_hat()),
        balaclava: m(outfit::balaclava()),
        scarf: m(outfit::scarf()),
        mitten: m(outfit::mitten()),
        boot: m(outfit::boot()),
        pack: m(outfit::backpack()),
        bandolier: m(outfit::bandolier()),
        frost: m(outfit::frost_patch(0.09, 6)),
        head: meshes.add(Sphere::new(0.12)),
        goggles: meshes.add(Cuboid::new(0.2, 0.06, 0.04)),
        eye: meshes.add(Sphere::new(0.0135)),
        nose: meshes.add(Sphere::new(0.026)),
        sleeve: meshes.add(Capsule3d::new(0.08, 0.38)),
        leg: meshes.add(Cuboid::new(0.15, 0.62, 0.19)),
        skin: materials.add(StandardMaterial { base_color: c(0.72, 0.56, 0.47), perceptual_roughness: 0.8, ..default() }),
        pants: materials.add(StandardMaterial { base_color: c(0.14, 0.14, 0.16), perceptual_roughness: 0.9, ..default() }),
        leather: tinted(&mut materials, &assets.leather, c(0.75, 0.62, 0.52), 0.8),
        canvas: tinted(&mut materials, &assets.wool, c(0.45, 0.4, 0.28), 0.95),
        rime,
        lens: materials.add(StandardMaterial { base_color: c(0.9, 0.6, 0.15), emissive: LinearRgba::rgb(1.2, 0.5, 0.05), ..default() }),
        survivors,
        raiders,
    });
}

fn piece(p: &mut ChildSpawnerCommands, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) {
    p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
}

fn at(p: &mut ChildSpawnerCommands, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, t: Transform) {
    p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), t));
}

/// Fine detail (fur tufts, frost, eyes, buckles): not drawn far away.
fn detail(p: &mut ChildSpawnerCommands, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, t: Transform) {
    p.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), t, crate::perf::Lod::PersonDetail, NotShadowCaster));
}

impl PersonKit {
    /// Body, head, hat, trim, pack and frost: everything above the hips that
    /// doesn't move on its own.
    pub fn dress_torso(&self, rig: &mut ChildSpawnerCommands, look: &Look) {
        piece(rig, &self.parka, &look.parka);
        detail(rig, &self.quilting, &look.parka, Transform::IDENTITY);
        piece(rig, &self.belt, &self.leather);
        detail(rig, &self.hem_fur, &look.fur, Transform::IDENTITY);
        // The head, with its fur-trimmed hood or hat.
        at(rig, &self.head, &self.skin, Transform::from_xyz(0.0, 1.68, 0.03));
        // A face: eyes and a nose (a balaclava's goggles cover the eyes).
        if look.hat != Hat::Balaclava {
            for x in [-0.047f32, 0.047] {
                detail(rig, &self.eye, &self.pants, Transform::from_xyz(x, 1.7, 0.139));
            }
        }
        detail(rig, &self.nose, &self.skin, Transform::from_xyz(0.0, 1.67, 0.15).with_scale(Vec3::new(0.8, 1.0, 1.1)));
        match look.hat {
            Hat::Hood => {
                piece(rig, &self.hood, &look.parka);
                detail(rig, &self.hood_fur, &look.fur, Transform::IDENTITY);
            }
            Hat::Beanie => {
                piece(rig, &self.beanie, &look.knit);
                piece(rig, &self.hood, &look.parka);
            }
            Hat::EarFlap => {
                piece(rig, &self.earflap, &look.knit);
                detail(rig, &self.hood_fur, &look.fur, Transform::IDENTITY);
            }
            Hat::Balaclava => {
                piece(rig, &self.balaclava, &look.knit);
                at(rig, &self.goggles, &self.lens, Transform::from_xyz(0.0, 1.71, 0.115));
                piece(rig, &self.hood, &look.parka);
                detail(rig, &self.hood_fur, &look.fur, Transform::IDENTITY);
            }
        }
        piece(rig, &self.scarf, &look.knit);
        if look.pack {
            piece(rig, &self.pack, &self.canvas);
        }
        if look.bandolier {
            piece(rig, &self.bandolier, &self.leather);
        }
        // Frost caught on the shoulders, the crown of the hood and the pack.
        let spots = [
            (-0.2, 1.5, 0.0, 1.0),
            (0.2, 1.5, 0.02, 1.0),
            (0.0, 1.88, -0.03, 0.9),
            (0.0, 1.5, -0.24, 1.3),
            (0.0, 1.47, -0.25, 1.2),
            (-0.1, 1.35, -0.2, 1.0),
        ];
        for (x, y, z, s) in spots.into_iter().take(look.frost) {
            detail(rig, &self.frost, &self.rime, Transform::from_xyz(x, y, z).with_scale(Vec3::splat(s)));
        }
    }

    /// A leg from the hip pivot (origin at the hip, the leg hanging down).
    pub fn dress_leg(&self, leg: &mut ChildSpawnerCommands) {
        at(leg, &self.leg, &self.pants, Transform::from_xyz(0.0, -0.31, 0.0));
        at(leg, &self.boot, &self.leather, Transform::from_xyz(0.0, -0.82, 0.0));
    }

    /// An arm from the shoulder pivot: sleeve, fur cuff, mitten.
    pub fn dress_arm(&self, arm: &mut ChildSpawnerCommands, look: &Look, side: f32) {
        at(arm, &self.sleeve, &look.parka, Transform::from_xyz(0.0, -0.28, 0.0));
        detail(arm, &self.cuff_fur, &look.fur, Transform::from_xyz(0.0, -0.5, 0.0));
        at(arm, &self.mitten, &self.leather, Transform::from_xyz(0.0, -0.58, 0.02).with_scale(Vec3::new(side, 1.0, 1.0)));
    }

    /// A frost patch for an arm or leg (used by raiders).
    pub fn rime(&self, p: &mut ChildSpawnerCommands, t: Transform) {
        p.spawn((Mesh3d(self.frost.clone()), MeshMaterial3d(self.rime.clone()), t, NotShadowCaster, crate::perf::Lod::PersonDetail));
    }
}
