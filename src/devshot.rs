//! Screenshot mode for automated previews (used to check art changes on a
//! headless machine). Does nothing unless `FMN_SHOT` is set:
//!
//! ```text
//! FMN_SHOT="x,z,yaw_deg,pitch_deg,hour[,blizzard[,aim[,reload[,weapon[,upgrades[,recoil[,swing]]]]]]]]" FMN_SHOT_OUT=shot.png cargo run
//! ```
//!
//! `FMN_SHOT_SIZE="2461,1154"` also resizes the window first.
//!
//! Teleports the player, sets the time of day (and optionally starts a
//! rad-blizzard, aims down the sights, or freezes a reload at the given
//! progress 0..1), waits for assets to load, saves a screenshot and exits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::gun::ForceAim;
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::terrain;
use crate::sim::weather::Phase;
use crate::sim::combat::{Upgrade, WeaponKind};
use crate::state::{ClockRes, Game, WeatherRes};

#[derive(Resource)]
struct Shot {
    x: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    hour: f32,
    blizzard: bool,
    aim: bool,
    reload: Option<f32>,
    /// Weapon slot to hold (0-3); all weapons are unlocked in screenshot mode.
    weapon: Option<usize>,
    upgrades: bool,
    /// Freeze a pose: recoil strength 0..1, or an axe swing progress.
    recoil: f32,
    swing: Option<f32>,
    out: String,
    /// Real seconds to wait for assets to load before shooting.
    wait: f32,
    taken: Option<f32>,
}

pub struct DevShotPlugin;

impl Plugin for DevShotPlugin {
    fn build(&self, app: &mut App) {
        let Ok(spec) = std::env::var("FMN_SHOT") else { return };
        let v: Vec<f32> = spec.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() < 5 {
            warn!("FMN_SHOT needs x,z,yaw,pitch,hour");
            return;
        }
        let out = std::env::var("FMN_SHOT_OUT").unwrap_or_else(|_| "shot.png".into());
        // A stale file would make us quit before the new shot is written.
        let _ = std::fs::remove_file(&out);
        app.insert_resource(Shot {
            x: v[0],
            z: v[1],
            yaw: v[2].to_radians(),
            pitch: v[3].to_radians(),
            hour: v[4],
            blizzard: v.get(5).is_some_and(|b| *b > 0.0),
            aim: v.get(6).is_some_and(|b| *b > 0.0),
            reload: v.get(7).copied().filter(|r| *r > 0.0),
            weapon: v.get(8).map(|w| *w as usize),
            upgrades: v.get(9).is_some_and(|u| *u > 0.0),
            recoil: v.get(10).copied().unwrap_or(0.0),
            swing: v.get(11).copied().filter(|r| *r > 0.0),
            out,
            wait: std::env::var("FMN_SHOT_WAIT").ok().and_then(|w| w.parse().ok()).unwrap_or(20.0),
            taken: None,
        })
        .add_systems(Update, take_shot)
        .add_systems(PostStartup, spawn_extras)
        .add_systems(Startup, resize_window);
    }
}

/// FMN_LINEUP=wolves|moose spawns a row of stationary creatures to look at.
/// Screenshot mode also turns off anti-aliasing: software rendering at
/// 4x MSAA takes gigabytes on a small machine.
fn spawn_extras(mut commands: Commands, cameras: Query<Entity, With<Camera3d>>) {
    for e in &cameras {
        commands.entity(e).insert(Msaa::Off);
    }
    match std::env::var("FMN_LINEUP").as_deref() {
        Ok("wolves") => {
            commands.run_system_cached(crate::wolves::spawn_lineup);
        }
        Ok("moose") => {
            commands.run_system_cached(crate::moose::spawn_lineup);
        }
        _ => {}
    }
}

fn resize_window(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    let Ok(size) = std::env::var("FMN_SHOT_SIZE") else { return };
    let v: Vec<f32> = size.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if let (Some(&w), Some(&h), Ok(mut window)) = (v.first(), v.get(1), windows.single_mut()) {
        window.resolution.set(w, h);
    }
}

fn take_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut shot: ResMut<Shot>,
    mut clock: ResMut<ClockRes>,
    mut weather: ResMut<WeatherRes>,
    mut game: ResMut<Game>,
    mut aim: ResMut<ForceAim>,
    mut player: Query<(&mut Transform, &mut Player)>,
    mut exit: EventWriter<AppExit>,
) {
    clock.0.hours = shot.hour;
    if shot.blizzard {
        weather.weather.phase = Phase::Blizzard;
        weather.weather.timer = 30.0;
    } else {
        weather.weather.timer = 60.0;
    }
    aim.0 = shot.aim;
    if let Some(slot) = shot.weapon {
        for k in WeaponKind::ALL {
            game.arsenal.unlock(k);
        }
        if game.arsenal.current != slot.min(3) {
            game.arsenal.current = slot.min(3);
        }
        game.arsenal.draw = 0.0;
        if shot.upgrades {
            for w in game.arsenal.weapons.iter_mut() {
                for up in Upgrade::ALL {
                    let _ = w.apply_upgrade(up);
                }
            }
        }
    }
    if let Some(r) = shot.reload {
        let w = game.weapon_mut();
        w.busy = w.reload_time * (1.0 - r);
    }
    if shot.recoil > 0.0 {
        game.recoil = shot.recoil;
    }
    if let Some(p) = shot.swing {
        let w = game.weapon_mut();
        w.cooldown = w.fire_interval * (1.0 - p);
    }
    if let Ok((mut tf, mut p)) = player.single_mut() {
        p.yaw = shot.yaw;
        p.pitch = shot.pitch;
        tf.translation = Vec3::new(shot.x, terrain::walk_height(shot.x, shot.z) + EYE_HEIGHT, shot.z);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
    }
    let now = time.elapsed_secs();
    match shot.taken {
        None if now > shot.wait => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot.out.clone()));
            shot.taken = Some(now);
        }
        // Software renderers can take seconds per frame; wait for the file.
        Some(at) if std::path::Path::new(&shot.out).exists() || now > at + 90.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
