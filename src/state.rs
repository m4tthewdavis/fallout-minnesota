//! Shared game resources: the player's stats, the weather, RNG and on-screen messages.

use bevy::prelude::*;

use crate::sim::collision::Shape;
use crate::sim::combat::{Arsenal, Weapon};
use crate::sim::daynight::Clock;
use crate::sim::rng::Rng;
use crate::sim::survival::{DeathCause, Inventory, Survival};
use crate::sim::synth::Sound;
use crate::sim::weather::{Phase, Weather};

#[derive(Resource)]
pub struct Game {
    pub survival: Survival,
    pub inv: Inventory,
    pub arsenal: Arsenal,
    pub death: Option<DeathCause>,
    /// 0..=1, drives the red damage overlay.
    pub hurt_flash: f32,
    /// 0..=1, drives the gun's recoil kick.
    pub recoil: f32,
    pub kills: u32,
}

impl Game {
    /// The weapon in your hands.
    pub fn weapon(&self) -> &Weapon {
        self.arsenal.current()
    }

    pub fn weapon_mut(&mut self) -> &mut Weapon {
        self.arsenal.current_mut()
    }

    pub fn new() -> Self {
        Game {
            survival: Survival::new(),
            inv: Inventory::starting_kit(),
            arsenal: Arsenal::starting(),
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

/// One sound requested by gameplay code.
#[derive(Clone, Copy, Debug)]
pub struct SfxReq {
    pub sound: Sound,
    /// Where it comes from, for positioned sounds (they pan and fade).
    pub pos: Option<Vec3>,
    /// Extra volume scale (a soft footstep, a muffled shot).
    pub gain: f32,
}

/// Sound effects requested this frame; the audio plugin plays and clears them.
#[derive(Resource, Default)]
pub struct SfxQueue(pub Vec<SfxReq>);

impl SfxQueue {
    pub fn play(&mut self, sound: Sound) {
        self.0.push(SfxReq { sound, pos: None, gain: 1.0 });
    }
    pub fn play_gain(&mut self, sound: Sound, gain: f32) {
        self.0.push(SfxReq { sound, pos: None, gain });
    }
    /// A sound that comes from a place in the world.
    pub fn play_at(&mut self, sound: Sound, pos: Vec3) {
        self.0.push(SfxReq { sound, pos: Some(pos), gain: 1.0 });
    }
}

/// Context hints shown near the crosshair ("[E] Open crate"). Systems set the
/// lines each frame; the HUD shows them.
#[derive(Resource, Default)]
pub struct Prompt(pub Vec<String>);

/// Marks anything that wants to hurt the player (wolves, the moose); the
/// music turns tense when one is near.
#[derive(Component)]
pub struct Hostile;

/// A point `dist` metres from `from` in a random direction (distant howls).
pub fn random_point_around(from: Vec3, dist: f32, rng: &mut Rng) -> Vec3 {
    let a = rng.range(0.0, std::f32::consts::TAU);
    from + Vec3::new(a.cos() * dist, 0.0, a.sin() * dist)
}

/// A visual effect requested by gameplay code; the particle plugin spawns it.
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    /// Snow kicked up by a footstep.
    Footstep(Vec3),
    /// Muzzle flash and smoke, plus an ejected casing if the last field is true:
    /// (muzzle, aim direction, gun right, ejects brass).
    Muzzle(Vec3, Vec3, Vec3, bool),
    /// Spent casings dumped during a reload: (breech position, how many).
    Brass(Vec3, u32),
    /// A bullet striking a Frostfang: (hit point, bullet direction).
    WolfHit(Vec3, Vec3),
    /// A Frostfang dropping dead: (body centre, size).
    WolfDeath(Vec3, f32),
    /// A wolf exhaling glowing vapour: (mouth, direction).
    WolfBreath(Vec3, Vec3),
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

/// True while the Pip-Boy is open (the game is paused).
#[derive(Resource, Default)]
pub struct PipOpen(pub bool);

/// Where the pines stand, for the Pip-Boy map's forest.
#[derive(Resource, Default)]
pub struct TreePositions(pub Vec<(f32, f32)>);

/// Run condition: gameplay systems only run while the player is alive and the
/// Pip-Boy is closed.
pub fn alive(game: Res<Game>, pip: Res<PipOpen>) -> bool {
    game.death.is_none() && !pip.0
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
            .init_resource::<Prompt>()
            .init_resource::<PipOpen>()
            .init_resource::<TreePositions>()
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
