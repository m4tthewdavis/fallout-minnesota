//! Shared game resources: the player's stats, the weather, RNG and on-screen messages.

use bevy::prelude::*;

use crate::sim::collision::Shape;
use crate::sim::combat::Weapon;
use crate::sim::daynight::Clock;
use crate::sim::rng::Rng;
use crate::sim::survival::{DeathCause, Inventory, Survival};
use crate::sim::synth::Sound;
use crate::sim::weather::{Phase, Weather};

#[derive(Resource)]
pub struct Game {
    pub survival: Survival,
    pub inv: Inventory,
    pub weapon: Weapon,
    pub death: Option<DeathCause>,
    /// 0..=1, drives the red damage overlay.
    pub hurt_flash: f32,
    /// 0..=1, drives the gun's recoil kick.
    pub recoil: f32,
    pub kills: u32,
}

impl Game {
    pub fn new() -> Self {
        Game {
            survival: Survival::new(),
            inv: Inventory::starting_kit(),
            weapon: Weapon::pipe_rifle(),
            death: None,
            hurt_flash: 0.0,
            recoil: 0.0,
            kills: 0,
        }
    }
}

#[derive(Resource)]
pub struct WeatherRes {
    pub weather: Weather,
    /// Set for exactly one frame when the phase changes.
    pub just_changed: Option<Phase>,
}

#[derive(Resource)]
pub struct RngRes(pub Rng);

/// Time of day.
#[derive(Resource, Default)]
pub struct ClockRes(pub Clock);

/// Solid obstacles (trees, walls, buildings) for players and wolves.
#[derive(Resource, Default)]
pub struct Colliders(pub Vec<Shape>);

/// Sound effects requested this frame; the audio plugin plays and clears them.
#[derive(Resource, Default)]
pub struct SfxQueue(pub Vec<Sound>);

impl SfxQueue {
    pub fn play(&mut self, sound: Sound) {
        self.0.push(sound);
    }
}

/// A visual effect requested by gameplay code; the particle plugin spawns it.
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    /// Snow kicked up by a footstep.
    Footstep(Vec3),
    /// Muzzle flash, smoke and an ejected casing: (muzzle, aim direction, gun right).
    Muzzle(Vec3, Vec3, Vec3),
    /// A bullet striking a Frostfang: (hit point, bullet direction).
    WolfHit(Vec3, Vec3),
    /// A Frostfang dropping dead: (body centre, size).
    WolfDeath(Vec3, f32),
    /// A bullet kicking up snow where it lands.
    Ricochet(Vec3),
    /// Nuclear ice giving way.
    IceBreak(Vec3),
}

/// Effects requested this frame; the particle plugin spawns and clears them.
#[derive(Resource, Default)]
pub struct FxQueue(pub Vec<Fx>);

impl FxQueue {
    pub fn spawn(&mut self, fx: Fx) {
        self.0.push(fx);
    }
}

/// One line of text shown at the top of the screen for a few seconds.
#[derive(Resource, Default)]
pub struct Messages {
    pub text: String,
    pub timer: f32,
}

impl Messages {
    pub fn show(&mut self, text: impl Into<String>, secs: f32) {
        self.text = text.into();
        self.timer = secs;
    }
}

/// Run condition: gameplay systems only run while the player is alive.
pub fn alive(game: Res<Game>) -> bool {
    game.death.is_none()
}

/// Despawns short-lived effects (muzzle flashes, tracers).
#[derive(Component)]
pub struct Lifetime(pub f32);

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(2077);

        let mut messages = Messages::default();
        messages.show(
            "VAULT 143: The door has opened for the first time since 2077. Spring has... ended?",
            8.0,
        );

        app.insert_resource(Game::new())
            .insert_resource(WeatherRes {
                weather: Weather::new(),
                just_changed: None,
            })
            .insert_resource(RngRes(Rng::new(seed)))
            .insert_resource(messages)
            .init_resource::<ClockRes>()
            .init_resource::<Colliders>()
            .init_resource::<SfxQueue>()
            .init_resource::<FxQueue>()
            .add_systems(Update, (tick_messages, tick_lifetimes));
    }
}

fn tick_messages(time: Res<Time>, mut msgs: ResMut<Messages>) {
    if msgs.timer > 0.0 {
        msgs.timer -= time.delta_secs();
    }
}

fn tick_lifetimes(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Lifetime)>) {
    for (entity, mut life) in &mut q {
        life.0 -= time.delta_secs();
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
