//! First-person controller, survival ticking, item use and respawning.

use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use crate::sim::keys::Bind;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::sim::collision;
use crate::sim::daynight::Clock;
use crate::sim::survival::{self, Aid, Exposure, Survival};
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, HALF_SIZE, PLAYER_SPAWN};
use crate::sim::weather::Weather;
use crate::assets::GameAssets;
use crate::state::{alive, ClockRes, Colliders, Fx, FxQueue, Game, Messages, RngRes, SfxQueue, WeatherRes};
use crate::wolves::Wolf;

pub const EYE_HEIGHT: f32 = 1.7;
const WALK_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.5;
const JUMP_SPEED: f32 = 5.5;
const GRAVITY: f32 = 15.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;
/// Seconds of sprinting on nuclear ice before it gives way.
const ICE_CRACK_SECS: f32 = 2.0;
/// Collision radius of the player's body.
const BODY_RADIUS: f32 = 0.35;
const STEP_WALK_SECS: f32 = 0.48;
const STEP_SPRINT_SECS: f32 = 0.32;

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub vel_y: f32,
    pub grounded: bool,
    pub sprinting: bool,
    pub moving: bool,
    pub ice_strain: f32,
    pub cold_warned: bool,
    step_timer: f32,
}

impl Player {
    pub fn new() -> Self {
        Player {
            yaw: 0.0,
            pitch: -0.05,
            vel_y: 0.0,
            grounded: true,
            sprinting: false,
            moving: false,
            ice_strain: 0.0,
            cold_warned: false,
            step_timer: 0.0,
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_player, grab_cursor)).add_systems(
            Update,
            (
                toggle_cursor,
                (look, move_player, survival_tick, use_items).chain().run_if(alive),
                god_mode,
                start_new_game,
                respawn,
            ),
        );
    }
}

fn spawn_point() -> Vec3 {
    let (x, z) = PLAYER_SPAWN;
    Vec3::new(x, terrain::walk_height(x, z) + EYE_HEIGHT, z)
}

fn spawn_player(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let player = Player::new();

    commands
        .spawn((
            Camera3d::default(),
            // HDR so emissive glows (eyes, fires, the warhead) can bloom.
            Camera {
                hdr: true,
                ..default()
            },
            Bloom::NATURAL,
            // Ears for positioned sounds (wolves, fires).
            SpatialListener::new(0.3),
            Tonemapping::TonyMcMapface,
            Projection::from(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                ..default()
            }),
            Transform::from_translation(spawn_point()).with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                player.yaw,
                player.pitch,
                0.0,
            )),
            DistanceFog {
                color: Color::srgba(0.70, 0.74, 0.78, 1.0),
                falloff: FogFalloff::Exponential { density: 0.0075 },
                ..default()
            },
            player,
        ))
        .with_children(|cam| {
            crate::gun::spawn_view_model(cam, &mut meshes, &mut materials, &assets);
        });
}

pub fn set_grab(window: &mut Window, locked: bool) {
    window.cursor_options.grab_mode = if locked {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    window.cursor_options.visible = !locked;
}

fn grab_cursor(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        set_grab(&mut window, true);
    }
}

/// Click to take the mouse back (after alt-tabbing, say). The Pip-Boy and the
/// pause menu own the mouse while they are open.
fn toggle_cursor(
    pip: Res<crate::state::PipOpen>,
    paused: Res<crate::state::Paused>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if pip.0 || paused.0 {
        return;
    }
    let Ok(mut window) = windows.single_mut() else { return };
    if mouse.just_pressed(MouseButton::Left) && window.cursor_options.grab_mode == CursorGrabMode::None {
        set_grab(&mut window, true);
    }
}

pub fn cursor_locked(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    windows
        .single()
        .map(|w| w.cursor_options.grab_mode != CursorGrabMode::None)
        .unwrap_or(false)
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q: Query<(&mut Transform, &mut Player)>,
    settings: Res<crate::menu::GameSettings>,
) {
    if !cursor_locked(&windows) {
        return;
    }
    let Ok((mut tf, mut p)) = q.single_mut() else { return };
    let speed = MOUSE_SENSITIVITY * settings.0.mouse_sensitivity;
    let invert = if settings.0.invert_y { -1.0 } else { 1.0 };
    p.yaw -= motion.delta.x * speed;
    p.pitch = (p.pitch - motion.delta.y * speed * invert).clamp(-1.5, 1.5);
    tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
}

fn move_player(
    controls: crate::keybind::Controls,
    time: Res<Time>,
    colliders: Res<Colliders>,
    mut sfx: ResMut<SfxQueue>,
    mut fx: ResMut<FxQueue>,
    mut rng: ResMut<RngRes>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    mut q: Query<(&mut Transform, &mut Player)>,
) {
    let dt = time.delta_secs();
    let Ok((mut tf, mut p)) = q.single_mut() else { return };

    let forward = Vec3::new(-p.yaw.sin(), 0.0, -p.yaw.cos());
    let right = Vec3::new(p.yaw.cos(), 0.0, -p.yaw.sin());
    let mut wish = Vec3::ZERO;
    if controls.pressed(Bind::Forward) {
        wish += forward;
    }
    if controls.pressed(Bind::Back) {
        wish -= forward;
    }
    if controls.pressed(Bind::Right) {
        wish += right;
    }
    if controls.pressed(Bind::Left) {
        wish -= right;
    }
    let moving = wish.length_squared() > 0.0;
    p.moving = moving;
    p.sprinting = moving && controls.pressed(Bind::Sprint);
    let speed = if p.sprinting { SPRINT_SPEED } else { WALK_SPEED };
    if moving {
        tf.translation += wish.normalize() * speed * dt;
    }
    // The map's edge doesn't apply inside a room, which is built beyond it.
    if crate::sim::interiors::zone_at(tf.translation.x, tf.translation.z).is_none() {
        let limit = HALF_SIZE - 2.0;
        tf.translation.x = tf.translation.x.clamp(-limit, limit);
        tf.translation.z = tf.translation.z.clamp(-limit, limit);
    }

    // Slide around trees, walls and buildings.
    let (cx, cz) = collision::push_out(tf.translation.x, tf.translation.z, BODY_RADIUS, &colliders.0);
    tf.translation.x = cx;
    tf.translation.z = cz;

    // Footsteps: a different sound for snow, ice, road, concrete and decks,
    // at slightly uneven intervals so the rhythm never sounds mechanical.
    if moving && p.grounded {
        p.step_timer -= dt;
        if p.step_timer <= 0.0 {
            let base = if p.sprinting { STEP_SPRINT_SECS } else { STEP_WALK_SECS };
            p.step_timer = base * rng.0.range(0.9, 1.1);
            let surface = terrain::surface_at(tf.translation.x, tf.translation.z);
            // Running crunches harder than walking, and in deep cold dry snow squeaks.
            let temp = weather.weather.conditions().air_temp_f + clock.0.temp_offset_f();
            let (sound, gain) = crate::sim::soundscape::footstep(surface, p.sprinting, temp, rng.0.f32());
            sfx.play_gain(sound, gain);
            fx.spawn(Fx::Footstep(crate::particles::feet(tf.translation) + wish.normalize() * 0.3));
        }
    } else {
        p.step_timer = 0.0;
    }

    // Gravity and jumping over the height field.
    let floor = terrain::walk_height(tf.translation.x, tf.translation.z) + EYE_HEIGHT;
    if p.grounded && controls.just_pressed(Bind::Jump) {
        p.vel_y = JUMP_SPEED;
        p.grounded = false;
    }
    p.vel_y -= GRAVITY * dt;
    tf.translation.y += p.vel_y * dt;
    if tf.translation.y <= floor {
        tf.translation.y = floor;
        p.vel_y = 0.0;
        p.grounded = true;
    } else if tf.translation.y - floor > 0.3 {
        p.grounded = false;
    }
}

/// New Game from the title screen: back to the vault door, facing out.
fn start_new_game(mut events: EventReader<crate::state::StartNewGame>, mut q: Query<(&mut Transform, &mut Player)>) {
    if events.read().count() == 0 {
        return;
    }
    if let Ok((mut tf, mut p)) = q.single_mut() {
        *p = Player::new();
        tf.translation = spawn_point();
        tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
    }
}

/// F3 toggles god mode (nothing hurts). `FMN_GOD=1` starts with it on.
fn god_mode(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>, mut msgs: ResMut<Messages>, mut started: Local<bool>) {
    if !std::mem::replace(&mut *started, true) && std::env::var("FMN_GOD").is_ok() {
        game.survival.god = true;
    }
    if keys.just_pressed(KeyCode::F3) {
        game.survival.god = !game.survival.god;
        msgs.show(if game.survival.god { "God mode ON: nothing can hurt you (F3 to turn off)" } else { "God mode off" }, 2.5);
    }
    // No red flash, and no death screen carrying over, while it's on.
    if game.survival.god {
        game.hurt_flash = 0.0;
    }
}

fn survival_tick(
    time: Res<Time>,
    mut game: ResMut<Game>,
    weather: Res<WeatherRes>,
    clock: Res<ClockRes>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    mut fx: ResMut<FxQueue>,
    perks: Res<crate::quest::Perks>,
    mut q: Query<(&mut Transform, &mut Player)>,
) {
    let dt = time.delta_secs();
    let Ok((mut tf, mut p)) = q.single_mut() else { return };
    let (x, z) = (tf.translation.x, tf.translation.z);
    let cond = weather.weather.conditions();
    let cover = terrain::cover_at(x, z);
    let sheltered = cover.sheltered();

    // Nuclear ice: sprinting on it too long cracks it.
    if let Some(lake) = terrain::lake_at(x, z) {
        if p.sprinting {
            p.ice_strain += dt;
        } else {
            p.ice_strain = (p.ice_strain - dt * 0.5).max(0.0);
        }
        if p.ice_strain >= ICE_CRACK_SECS {
            p.ice_strain = 0.0;
            fx.spawn(Fx::IceBreak(Vec3::new(x, terrain::ICE_LEVEL, z)));
            game.survival.warm(-45.0);
            game.survival.rads = (game.survival.rads + 80.0).min(Survival::MAX_RADS);
            let (sx, sz) = terrain::shore_point(lake, x, z);
            tf.translation = Vec3::new(sx, terrain::walk_height(sx, sz) + EYE_HEIGHT, sz);
            p.vel_y = 0.0;
            sfx.play(Sound::IceCrack);
            msgs.show(
                "The nuclear ice cracks! You plunge into glowing water and claw your way ashore.",
                4.0,
            );
        } else if p.ice_strain > 0.9 && msgs.timer <= 0.0 {
            sfx.play(Sound::IceCreak);
            msgs.show("The ice groans under your boots... stop sprinting!", 1.5);
        }
    } else {
        p.ice_strain = 0.0;
    }

    let exposure = Exposure {
        air_temp_f: cond.air_temp_f + clock.0.temp_offset_f(),
        wind_chill_f: cond.wind_chill_f,
        sheltered,
        near_heat: cover.warm(),
        rads_per_sec: terrain::ambient_rads(x, z) + if sheltered { 0.0 } else { cond.rads_per_sec },
        sprinting: p.sprinting,
        insulation: game.inv.insulation() + perks.0.insulation,
        rad_resist: perks.0.rad_resist,
    };
    if let Some(cause) = game.survival.tick(&exposure, dt) {
        game.death = Some(cause);
        msgs.show(cause.describe(), f32::MAX);
        return;
    }

    if game.survival.body_heat < 30.0 && !p.cold_warned {
        p.cold_warned = true;
        msgs.show("You're freezing. Find a fish house fire, or eat some hotdish [F].", 4.0);
    } else if game.survival.body_heat > 50.0 {
        p.cold_warned = false;
    }
}

/// Use one aid item and tell the player what happened. Returns true if it was used.
pub fn use_aid(aid: Aid, game: &mut Game, msgs: &mut Messages, heal_mult: f32) -> bool {
    let before = game.survival.health;
    let result = survival::use_aid(aid, &mut game.inv, &mut game.survival);
    if result.used() && heal_mult > 1.0 {
        // Field Medic: the healing goes further.
        let healed = game.survival.health - before;
        game.survival.heal(healed * (heal_mult - 1.0));
    }
    let secs = if result.used() && aid == Aid::Hotdish { 3.0 } else if result.used() { 2.0 } else { 1.5 };
    msgs.show(result.message(), secs);
    result.used()
}

fn use_items(
    controls: crate::keybind::Controls,
    mut game: ResMut<Game>,
    mut msgs: ResMut<Messages>,
    mut sfx: ResMut<SfxQueue>,
    perks: Res<crate::quest::Perks>,
    q: Query<&Transform, With<Player>>,
) {
    for (key, aid) in [(Bind::Stimpak, Aid::Stimpak), (Bind::RadAway, Aid::RadAway), (Bind::Hotdish, Aid::Hotdish)] {
        if controls.just_pressed(key) {
            use_aid(aid, &mut game, &mut msgs, perks.0.aid_heal);
        }
    }
    let Game { inv, .. } = &mut *game;
    if controls.just_pressed(Bind::Craft) {
        let at_shelter = q
            .single()
            .map(|t| terrain::cover_at(t.translation.x, t.translation.z).warm())
            .unwrap_or(false);
        if !at_shelter {
            msgs.show("You need a fish house workbench to craft. Find a shelter.", 2.5);
        } else {
            match inv.craft_coat() {
                Ok(()) => {
                    sfx.play(Sound::Craft);
                    msgs.show("Crafted a Frostfang coat! Insulation greatly increased.", 3.5)
                }
                Err(e) => msgs.show(e, 2.5),
            }
        }
    }
}

fn respawn(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut weather: ResMut<WeatherRes>,
    mut clock: ResMut<ClockRes>,
    mut msgs: ResMut<Messages>,
    mut interior: ResMut<crate::state::CurrentInterior>,
    mut player: Query<(&mut Transform, &mut Player)>,
    wolves: Query<(Entity, &Transform), (With<Wolf>, Without<Player>)>,
) {
    if game.death.is_none() || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let spawn = spawn_point();
    // Back outdoors, whichever room you died in.
    interior.0 = None;
    if let Ok((mut tf, mut p)) = player.single_mut() {
        *p = Player::new();
        tf.translation = spawn;
        tf.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
    }
    for (entity, tf) in &wolves {
        if tf.translation.distance(spawn) < 60.0 {
            commands.entity(entity).despawn();
        }
    }
    *game = Game::new();
    weather.weather = Weather::new();
    weather.just_changed = None;
    clock.0 = Clock::new();
    msgs.show(
        "The Overseer drags you back inside and patches you up. Try again, Thawborn.",
        5.0,
    );
}
