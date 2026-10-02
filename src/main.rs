//! Fallout: Minnesota - first-person cold-survival prototype.
//!
//! Milestone 1: leave Vault 143, survive the Long Winter (Body Heat, radiation,
//! rad-blizzards, nuclear ice), and fight Frostfang wolf packs.
//! Milestone 2: collision, synthesised audio, day/night cycle, animated wolves.

mod audio;
mod combat;
mod hud;
mod player;
mod sim;
mod state;
mod weather_fx;
mod wolves;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.62, 0.66, 0.70)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.75, 0.80, 0.90),
            brightness: 500.0,
            ..default()
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Fallout: Minnesota - Prototype".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            state::StatePlugin,
            audio::SoundPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            weather_fx::WeatherPlugin,
            wolves::WolfPlugin,
            combat::CombatPlugin,
            hud::HudPlugin,
        ))
        .run();
}
