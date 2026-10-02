#![allow(dead_code)]

//! Pure game simulation for Fallout: Minnesota.
//!
//! Nothing in this module depends on Bevy, so all of the rules (cold, radiation,
//! weather, weapons, wolf behaviour, terrain) can be unit-tested with plain
//! `cargo test` and reused if the renderer ever changes.

pub mod collision;
pub mod combat;
pub mod daynight;
pub mod rng;
pub mod survival;
pub mod synth;
pub mod terrain;
pub mod weather;
pub mod wolf;
