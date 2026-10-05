//! Things you do with your hands: open containers (E) and fit weapon
//! upgrades at a shelter workbench (B). Shows a prompt near the crosshair
//! when something can be used.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::player::Player;
use crate::sim::collision::Shape;
use crate::sim::loot::{self, Loot};
use crate::sim::synth::Sound;
use crate::state::{alive, Game, Messages, Prompt, SfxQueue};

/// How close you must be to open a container.
const REACH: f32 = 2.6;
/// How close you must be to use a workbench.
const BENCH_REACH: f32 = 3.4;

#[derive(Component)]
pub struct Container {
    pub name: &'static str,
    pub loot: Vec<Loot>,
    pub opened: bool,
    ring: Entity,
}

impl Container {
    /// The green ring that marks it while it's unopened.
    pub fn ring(&self) -> Entity {
        self.ring
    }
}

#[derive(Component)]
pub struct Workbench;

/// Shared look of the green ring that marks unopened containers.
#[derive(Resource)]
pub struct ContainerAssets {
    ring_mesh: Handle<Mesh>,
    ring_mat: Handle<StandardMaterial>,
}

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_assets)
            .add_systems(Update, (reset_prompt, interact).chain().run_if(alive));
    }
}

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(ContainerAssets {
        ring_mesh: meshes.add(Annulus::new(0.7, 0.8)),
        ring_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.4, 1.0, 0.4, 0.45),
            emissive: LinearRgba::rgb(0.5, 2.0, 0.5),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    });
}

/// Spawn a container: a model with loot inside, a collider and a marker ring.
pub fn spawn_container(
    commands: &mut Commands,
    assets: &ContainerAssets,
    solid: &mut Vec<Shape>,
    scene: Handle<Scene>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    name: &'static str,
    loot: Vec<Loot>,
) -> Entity {
    let ring = commands
        .spawn((
            Mesh3d(assets.ring_mesh.clone()),
            MeshMaterial3d(assets.ring_mat.clone()),
            Transform::from_xyz(pos.x, pos.y + 0.06, pos.z).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
        ))
        .id();
    solid.push(Shape::Circle {
        x: pos.x,
        z: pos.z,
        r: 0.6 * scale.max(1.0),
    });
    commands
        .spawn((
            SceneRoot(scene),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
            Container {
                name,
                loot,
                opened: false,
                ring,
            },
        ))
        .id()
}

fn reset_prompt(mut prompt: ResMut<Prompt>) {
    prompt.0.clear();
}

fn interact(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut prompt: ResMut<Prompt>,
    player: Query<&Transform, With<Player>>,
    mut containers: Query<(&Transform, &mut Container), Without<Player>>,
    benches: Query<&Transform, (With<Workbench>, Without<Player>)>,
) {
    let Ok(ptf) = player.single() else { return };
    let p = ptf.translation;
    let flat = |t: Vec3| Vec2::new(t.x - p.x, t.z - p.z).length();

    // ---- Containers (E) ----
    let nearest = containers
        .iter_mut()
        .filter(|(_, c)| !c.opened)
        .map(|(tf, c)| (flat(tf.translation), tf, c))
        .filter(|(d, _, _)| *d < REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, tf, mut c)) = nearest {
        prompt.0.push(format!("[E] Open {}", c.name));
        if keys.just_pressed(KeyCode::KeyE) {
            c.opened = true;
            let Game { inv, arsenal, .. } = &mut *game;
            let gained = loot::grant(&c.loot, inv, arsenal);
            msgs.show(format!("{}: {}", c.name, gained.join(", ")), 5.0);
            sfx.play_at(Sound::ContainerOpen, tf.translation + Vec3::Y * 0.5);
            commands.entity(c.ring).insert(Visibility::Hidden);
        }
    }

    // ---- Workbench (B) ----
    if benches.iter().any(|t| flat(t.translation) < BENCH_REACH) {
        let Game { inv, arsenal, .. } = &mut *game;
        let weapon = arsenal.current_mut();
        match weapon.next_upgrade() {
            Some(up) => {
                prompt.0.push(format!(
                    "[B] Fit {} to the {}: {} ({} scrap, you have {})",
                    up.name(),
                    weapon.name,
                    up.describe(),
                    up.scrap_cost(),
                    inv.scrap
                ));
                if keys.just_pressed(KeyCode::KeyB) {
                    match inv.craft_upgrade(weapon, up) {
                        Ok(()) => {
                            sfx.play(Sound::Craft);
                            msgs.show(format!("Fitted {} to the {}: {}.", up.name(), weapon.name, up.describe()), 4.0);
                        }
                        Err(e) => msgs.show(
                            format!("{e} {} needs {} scrap (you have {}).", up.name(), up.scrap_cost(), inv.scrap),
                            3.0,
                        ),
                    }
                }
            }
            None => {
                let line = if weapon.melee {
                    "Workbench: the ice axe can't be improved. Hold a firearm to upgrade it.".to_string()
                } else {
                    format!("Workbench: the {} is fully upgraded.", weapon.name)
                };
                prompt.0.push(line);
            }
        }
    }
}
