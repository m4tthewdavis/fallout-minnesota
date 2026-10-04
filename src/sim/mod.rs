// TEMP

//! Pure game simulation for Fallout: Minnesota.
//!
//! Nothing in this module depends on Bevy, so all of the rules (cold, radiation,
//! weather, weapons, wolf behaviour, terrain) can be unit-tested with plain
//! `cargo test` and reused if the renderer ever changes.

pub mod collision;
pub mod combat;
pub mod daynight;
pub mod flora;
pub mod loot;
pub mod mathx;
pub mod mapdata;
pub mod meshgen;
pub mod mipmaps;
pub mod moose;
pub mod pipnav;
pub mod progress;
pub mod rng;
pub mod sfx;
pub mod survival;
pub mod synth;
pub mod viewmodel;
pub mod terrain;
pub mod vehicles;
pub mod weather;
pub mod wolf;
