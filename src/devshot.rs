//! Screenshot mode for automated previews (used to check art changes on a
//! headless machine). Does nothing unless `FMN_SHOT` is set:
//!
//! ```text
//! FMN_SHOT="x,z,yaw_deg,pitch_deg,hour[,blizzard]" FMN_SHOT_OUT=shot.png cargo run
//! ```
//!
//! Teleports the player, sets the time of day (and optionally starts a
//! rad-blizzard), waits for assets to load, saves a screenshot and exits.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::player::{Player, EYE_HEIGHT};
use crate::sim::terrain;
use crate::sim::weather::Phase;
use crate::state::{ClockRes, WeatherRes};

#[derive(Resource)]
struct Shot {
    x: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    hour: f32,
    blizzard: bool,
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
            out,
            wait: std::env::var("FMN_SHOT_WAIT").ok().and_then(|w| w.parse().ok()).unwrap_or(20.0),
            taken: None,
        })
        .add_systems(Update, take_shot);
    }
}

fn take_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut shot: ResMut<Shot>,
    mut clock: ResMut<ClockRes>,
    mut weather: ResMut<WeatherRes>,
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
