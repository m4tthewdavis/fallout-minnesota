//! Fallout: Minnesota - first-person cold-survival prototype.
//!
//! Milestone 1: leave Vault 143, survive the Long Winter (Body Heat, radiation,
//! rad-blizzards, nuclear ice), and fight Frostfang wolf packs.
//! Milestone 2: collision, synthesised audio, day/night cycle, animated wolves.
//! Milestone 3: real art - CC0 models and PBR textures, procedural pines,
//! wolves, cars and buildings, a sky with stars and aurora, particles, bloom,
//! a Pip-Boy HUD, music and a Geiger counter.

mod assets;
mod audio;
mod combat;
mod devshot;
mod hud;
mod landmarks;
mod meshes;
mod nature;
mod particles;
mod player;
mod sim;
mod sky;
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
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Fallout: Minnesota - Prototype".into(),
                        resolution: (1280.0_f32, 720.0_f32).into(),
                        ..default()
                    }),
                    ..default()
                })
                // Find the assets folder wherever the game was launched from.
                .set(AssetPlugin {
                    file_path: assets::asset_root(),
                    ..default()
                }),
        )
        .add_plugins((
            assets::AssetsPlugin,
            state::StatePlugin,
            audio::SoundPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            sky::SkyPlugin,
            weather_fx::WeatherPlugin,
            wolves::WolfPlugin,
            combat::CombatPlugin,
            particles::ParticlePlugin,
            hud::HudPlugin,
            devshot::DevShotPlugin,
        ))
        .run();
}
