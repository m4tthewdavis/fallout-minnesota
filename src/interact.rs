//! Things you do with your hands: open containers (E) and fit weapon
//! upgrades at a shelter workbench (B). Shows a prompt near the crosshair
//! when something can be used.

use bevy::ecs::schedule::common_conditions::not;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

use crate::sim::keys::Bind;

use crate::player::Player;
use crate::sim::collision::Shape;
use crate::sim::loot::{self, Loot};
use crate::sim::lootmenu;
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
    /// Which entries of `loot` have been taken.
    pub taken: Vec<bool>,
    /// Everything has been taken.
    pub opened: bool,
    ring: Entity,
}

impl Container {
    /// The green ring that marks it while it still holds something.
    pub fn ring(&self) -> Entity {
        self.ring
    }

    /// Indices into `loot` of what's still inside.
    pub fn remaining(&self) -> Vec<usize> {
        lootmenu::remaining(&self.taken)
    }

    /// Take entry `i` (None if it's gone or doesn't exist). Emptying the
    /// container marks it opened.
    pub fn take(&mut self, i: usize) -> Option<Loot> {
        if *self.taken.get(i)? {
            return None;
        }
        self.taken[i] = true;
        self.opened = self.taken.iter().all(|t| *t);
        self.loot.get(i).copied()
    }

    /// Which entries are gone, for the save.
    pub fn taken_indices(&self) -> Vec<u8> {
        self.taken.iter().enumerate().filter(|(_, t)| **t).map(|(i, _)| i as u8).collect()
    }

    /// Put it back as a save says: emptied, partly looted or untouched.
    pub fn restore(&mut self, opened: bool, taken: &[u8]) {
        self.taken = (0..self.loot.len()).map(|i| opened || taken.contains(&(i as u8))).collect();
        self.opened = self.taken.iter().all(|t| *t);
    }
}

/// The container you're looking at, and which of its entries is highlighted.
#[derive(Resource, Default)]
pub struct LootMenu {
    pub target: Option<Entity>,
    pub selected: usize,
}

#[derive(Component)]
pub struct Workbench;

/// Shared look of the green ring that marks unopened containers.
#[derive(Resource)]
pub struct ContainerAssets {
    ring_mesh: Handle<Mesh>,
    ring_mat: Handle<StandardMaterial>,
}

/// Set while a door, bunk or stove is in reach: it takes E, so a stash
/// beside it stays shut.
#[derive(Resource, Default)]
pub struct FixtureClaim(pub bool);

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FixtureClaim>()
            .init_resource::<LootMenu>()
            .add_systems(Update, forget_loot_target.run_if(not(alive)))
            .add_systems(PreStartup, setup_assets)
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
                taken: vec![false; loot.len()],
                loot,
                opened: false,
                ring,
            },
        ))
        .id()
}

/// No container is targeted while a menu or conversation has the screen.
fn forget_loot_target(mut menu: ResMut<LootMenu>) {
    menu.target = None;
}

pub(crate) fn reset_prompt(mut prompt: ResMut<Prompt>) {
    prompt.0.clear();
}

pub(crate) fn interact(
    controls: crate::keybind::Controls,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut prompt: ResMut<Prompt>,
    claim: Res<FixtureClaim>,
    perks: Res<crate::quest::Perks>,
    scroll: Res<AccumulatedMouseScroll>,
    mut menu: ResMut<LootMenu>,
    mut feed: ResMut<crate::fo4ui::Feed>,
    cam: Query<&GlobalTransform, With<Player>>,
    player: Query<&Transform, With<Player>>,
    mut containers: Query<(Entity, &Transform, &mut Container), Without<Player>>,
    benches: Query<&Transform, (With<Workbench>, Without<Player>)>,
) {
    let Ok(ptf) = player.single() else { return };
    let p = ptf.translation;
    let flat = |t: Vec3| Vec2::new(t.x - p.x, t.z - p.z).length();

    // ---- Containers: look at one to see what's in it ----
    // The one in reach that you're facing most squarely.
    let look = cam.single().map(|g| g.forward().as_vec3()).unwrap_or(Vec3::NEG_Z);
    let target = containers
        .iter()
        .filter(|(_, _, c)| !c.opened && !claim.0)
        .filter_map(|(e, tf, _)| {
            let to = Vec3::new(tf.translation.x - p.x, 0.0, tf.translation.z - p.z);
            let dist = to.length();
            let facing = if dist < 0.6 { 1.0 } else { to.normalize_or_zero().dot(Vec3::new(look.x, 0.0, look.z).normalize_or_zero()) };
            (dist < REACH && facing > 0.35).then_some((e, dist * (2.0 - facing)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e);
    if menu.target != target {
        menu.target = target;
        menu.selected = 0;
    }
    if let Some((_, tf, mut c)) = target.and_then(|e| containers.get_mut(e).ok()) {
        let left = c.remaining();
        let step = (controls.raw().just_pressed(KeyCode::ArrowDown) as i32 - controls.raw().just_pressed(KeyCode::ArrowUp) as i32) - scroll.delta.y.signum() as i32 * (scroll.delta.y != 0.0) as i32;
        if step != 0 {
            menu.selected = lootmenu::step(menu.selected, step, left.len());
            sfx.play(Sound::UiTab);
        }
        menu.selected = menu.selected.min(left.len().saturating_sub(1));
        let take_all = controls.just_pressed(Bind::TakeAll);
        if controls.just_pressed(Bind::Interact) || take_all {
            let picks: Vec<usize> = if take_all { left.clone() } else { left.get(menu.selected).copied().into_iter().collect() };
            let first_take = c.taken.iter().all(|t| !*t);
            for i in picks {
                let Some(item) = c.take(i) else { continue };
                let Game { inv, arsenal, .. } = &mut *game;
                let scrap_before = inv.scrap;
                let gained = loot::grant(&[item], inv, arsenal);
                if inv.scrap > scrap_before {
                    // Scrounger.
                    inv.scrap += perks.0.extra_scrap;
                }
                feed.push(format!("{} added", gained.first().cloned().unwrap_or_else(|| lootmenu::label(&item))));
                if let Loot::Item(it, _) = item {
                    sfx.play(crate::sim::sfx::pickup_sound(it));
                } else {
                    sfx.play(Sound::PickupAmmo);
                }
            }
            if first_take {
                sfx.play_at(Sound::ContainerOpen, tf.translation + Vec3::Y * 0.5);
            }
            if c.opened {
                commands.entity(c.ring).insert(Visibility::Hidden);
                menu.target = None;
            }
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
                if controls.just_pressed(Bind::Workbench) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::survival::Item;

    fn crate_with(n: usize) -> Container {
        Container {
            name: "crate",
            loot: (0..n).map(|_| Loot::Item(Item::Scrap, 1)).collect(),
            taken: vec![false; n],
            opened: false,
            ring: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn taking_things_one_at_a_time_empties_the_container() {
        let mut c = crate_with(3);
        assert_eq!(c.remaining(), vec![0, 1, 2]);
        assert!(c.take(1).is_some());
        assert_eq!(c.remaining(), vec![0, 2]);
        assert!(!c.opened);
        assert!(c.take(1).is_none(), "can't take it twice");
        assert!(c.take(9).is_none());
        c.take(0);
        c.take(2);
        assert!(c.opened && c.remaining().is_empty());
    }

    #[test]
    fn a_partly_looted_container_comes_back_as_it_was_left() {
        let mut c = crate_with(4);
        c.take(0);
        c.take(3);
        let gone = c.taken_indices();
        assert_eq!(gone, vec![0, 3]);
        let mut fresh = crate_with(4);
        fresh.restore(false, &gone);
        assert_eq!(fresh.remaining(), vec![1, 2], "what you took stays taken");
        assert!(!fresh.opened);
        let mut emptied = crate_with(4);
        emptied.restore(true, &[]);
        assert!(emptied.opened && emptied.remaining().is_empty());
    }
}
