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
use crate::state::{ClockRes, CurrentInterior, Game, WeatherRes};

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
    /// `FMN_FREE=1`: pose the player once, then let the game run (to test doors).
    free: bool,
    placed: bool,
    /// Real seconds to wait for assets to load before shooting.
    wait: f32,
    taken: Option<f32>,
}

/// `FMN_KEYS="Escape,ArrowDown,Enter"` presses those keys one after another,
/// a few frames apart, before the screenshot is taken (to drive menus).
#[derive(Resource)]
struct KeyScript {
    steps: std::collections::VecDeque<KeyCode>,
    wait: u32,
    release: Option<KeyCode>,
    /// Frames left to settle once every key has been pressed.
    settle: u32,
}

fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Escape" => KeyCode::Escape,
        "Enter" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Space,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "KeyW" => KeyCode::KeyW,
        "KeyA" => KeyCode::KeyA,
        "KeyS" => KeyCode::KeyS,
        "KeyD" => KeyCode::KeyD,
        "KeyQ" => KeyCode::KeyQ,
        "KeyE" => KeyCode::KeyE,
        "KeyR" => KeyCode::KeyR,
        "KeyL" => KeyCode::KeyL,
        "KeyF" => KeyCode::KeyF,
        "KeyH" => KeyCode::KeyH,
        "KeyX" => KeyCode::KeyX,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F9" => KeyCode::F9,
        "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2,
        "Digit3" => KeyCode::Digit3,
        _ => return None,
    })
}

/// Press the next scripted key, right after Bevy has read the real input.
fn inject_keys(mut script: ResMut<KeyScript>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    if let Some(k) = script.release.take() {
        keys.release(k);
    }
    if script.wait > 0 {
        script.wait -= 1;
        return;
    }
    if let Some(k) = script.steps.pop_front() {
        keys.press(k);
        script.release = Some(k);
        script.wait = 4;
    } else if script.settle > 0 {
        script.settle -= 1;
    }
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
        if let Ok(list) = std::env::var("FMN_KEYS") {
            let steps: Vec<KeyCode> = list.split(',').filter_map(|n| parse_key(n.trim())).collect();
            // Start well after the world has loaded.
            app.insert_resource(KeyScript { steps: steps.into(), wait: 12, release: None, settle: 6 });
        }
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
            free: std::env::var("FMN_FREE").is_ok(),
            placed: false,
            wait: std::env::var("FMN_SHOT_WAIT").ok().and_then(|w| w.parse().ok()).unwrap_or(20.0),
            taken: None,
        })
        .add_systems(Update, (dev_story, dev_slay, dev_bang, dev_fire, dev_sfx, dev_kills, take_shot).chain())
        .add_systems(PreUpdate, inject_keys.run_if(resource_exists::<KeyScript>).after(bevy::input::InputSystem))
        .add_systems(PostStartup, spawn_extras)
        .add_systems(Startup, resize_window);
    }
}

/// FMN_LINEUP=wolves|moose|trees spawns a row of stationary creatures to look at.
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
        Ok("crows") => {
            commands.run_system_cached(crate::crows::spawn_lineup);
        }
        Ok("raiders") => {
            commands.run_system_cached(crate::raiders::spawn_lineup);
        }
        Ok("trees") => {
            commands.run_system_cached(crate::flora::spawn_lineup);
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

/// `FMN_FLAGS=quest.started,found.convoy` sets story flags on the first frame,
/// and `FMN_TALK=<npc|perks>` opens that conversation.
fn dev_story(mut done: Local<bool>, mut flags: ResMut<crate::saves::StoryFlags>, mut talk: ResMut<crate::quest::Talk>, mut talking: ResMut<crate::state::Talking>) {
    if std::mem::replace(&mut *done, true) {
        return;
    }
    if let Ok(list) = std::env::var("FMN_FLAGS") {
        flags.0.extend(list.split(',').map(str::trim).filter(|f| !f.is_empty()).map(String::from));
    }
    if let Ok(name) = std::env::var("FMN_TALK") {
        crate::quest::open_dev_talk(&name, &flags.0, &mut talk, &mut talking);
    }
}

/// `FMN_SLAY=1`: a few seconds in, fell Sven's Glowmoose.
fn dev_slay(mut commands: Commands, time: Res<Time<Real>>, mut done: Local<bool>) {
    if !*done && time.elapsed_secs() > 6.0 && std::env::var("FMN_SLAY").is_ok() {
        *done = true;
        commands.run_system_cached(crate::quest::dev_slay);
    }
}

/// `FMN_BANG=1`: fire a (silent) gunshot from the player's position a few
/// seconds in, to see what the crows and raiders do about it.
fn dev_bang(mut shots: EventWriter<crate::state::Gunshot>, time: Res<Time<Real>>, mut done: Local<bool>, player: Query<&Transform, With<Player>>) {
    if !*done && time.elapsed_secs() > 8.0 && std::env::var("FMN_BANG").is_ok() {
        *done = true;
        if let Ok(p) = player.single() {
            shots.write(crate::state::Gunshot { pos: p.translation, by_player: true });
        }
    }
}

/// `FMN_FIRE=1`: keep firing the held weapon's muzzle effects (no ammo spent),
/// so a screenshot catches the flash and smoke.
fn dev_fire(mut fx: ResMut<crate::state::FxQueue>, mut tick: Local<u32>, game: Res<Game>, gun: Query<&GlobalTransform, With<crate::gun::GunModel>>, cam: Query<&GlobalTransform, With<Player>>) {
    if std::env::var("FMN_FIRE").is_err() {
        return;
    }
    *tick += 1;
    if !(*tick).is_multiple_of(3) {
        return;
    }
    let (Ok(g), Ok(c)) = (gun.single(), cam.single()) else { return };
    let kind = game.weapon().kind;
    let muzzle = g.transform_point(crate::gun::muzzle_local(kind));
    let dir = c.forward().as_vec3();
    fx.spawn(crate::state::Fx::Muzzle(muzzle, dir, c.right().as_vec3(), false));
    fx.spawn(crate::state::Fx::Blast(muzzle, dir, if kind == WeaponKind::ScrapShotgun { 1.6 } else { 1.0 }));
}

/// `FMN_SFXTEST=1`: a few seconds in, fire some test sounds (with
/// `FMN_AUDIO_LOG=1` the audio system says how each one was heard).
fn dev_sfx(mut done: Local<bool>, time: Res<Time<Real>>, mut sfx: ResMut<crate::state::SfxQueue>, player: Query<&Transform, With<Player>>) {
    if *done || time.elapsed_secs() < 12.0 || std::env::var("FMN_SFXTEST").is_err() {
        return;
    }
    *done = true;
    let Ok(p) = player.single() else { return };
    use crate::sim::synth::Sound;
    let at = p.translation;
    let vault = crate::sim::interiors::Interior::VaultLobby.origin();
    // Your own shot; a raider's shot 40 m off in the open; the same shot in the vault next door; footsteps; a bark.
    sfx.play(Sound::RifleShot);
    sfx.push(crate::state::SfxReq::new(Sound::ShotgunShot).at(at + Vec3::new(40.0, 0.0, 0.0)).carrying(26.0));
    sfx.push(crate::state::SfxReq::new(Sound::RifleShot).at(Vec3::new(vault.0, 1.0, vault.1)).carrying(26.0));
    sfx.push(crate::state::SfxReq::new(Sound::StepSnowRun).at(at + Vec3::new(0.0, 0.0, 12.0)).carrying(10.0));
    sfx.push(crate::state::SfxReq::new(Sound::RaiderShout).at(at + Vec3::new(-20.0, 1.6, 0.0)));
    sfx.push(crate::state::SfxReq::new(Sound::BoltClack).after(0.4));
}

/// `FMN_KILLS=n`: a few seconds in, award n kills (25 XP each) to show the XP bar.
fn dev_kills(mut done: Local<bool>, time: Res<Time<Real>>, mut game: ResMut<Game>) {
    // Just before the screenshot, so the bar is still up when it's taken.
    let wait: f32 = std::env::var("FMN_SHOT_WAIT").ok().and_then(|w| w.parse().ok()).unwrap_or(20.0);
    if *done || time.elapsed_secs() < wait - 2.5 {
        return;
    }
    if let Some(n) = std::env::var("FMN_KILLS").ok().and_then(|v| v.parse::<u32>().ok()) {
        game.kills += n;
    }
    *done = true;
}

fn take_shot(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut shot: ResMut<Shot>,
    mut clock: ResMut<ClockRes>,
    mut weather: ResMut<WeatherRes>,
    mut interior: ResMut<CurrentInterior>,
    mut game: ResMut<Game>,
    mut aim: ResMut<ForceAim>,
    mut player: Query<(&mut Transform, &mut Player)>,
    mut exit: EventWriter<AppExit>,
    script: Option<Res<KeyScript>>,
) {
    if !(shot.free && shot.placed) {
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
            // Off-map coordinates inside a room's floor put you in that room.
            interior.set_if_neq(CurrentInterior(crate::sim::interiors::zone_at(shot.x, shot.z)));
            tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
        }
        shot.placed = true;
    }
    let now = time.elapsed_secs();
    match shot.taken {
        // With a key script, wait until every key has been pressed and things settled.
        None if now > shot.wait && script.is_none_or(|s| s.steps.is_empty() && s.settle == 0 && s.release.is_none()) => {
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
