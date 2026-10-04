//! Plays the synthesised sounds. The rules for what plays, how often and how
//! loud live in `sim::sfx` (unit-tested); this module is the Bevy glue:
//!
//! * the sound bank is generated on background threads, so the game starts
//!   at once and sounds appear a moment later;
//! * every one-shot goes through `sim::sfx::Player` (random variant that
//!   never repeats, pitch and volume wobble, cooldowns, repeat fatigue) and
//!   the player's volume sliders;
//! * positioned sounds (wolves, fires, shell casings) are spatial: they pan
//!   left and right and fade with distance;
//! * wind (three loops of different lengths plus random gusts), the siren,
//!   fire crackle and three moods of music are loops whose volumes follow the
//!   weather, the player and the music director;
//! * a Geiger counter that clicks gently, and keys to change the volume.

use std::collections::HashMap;
use std::sync::{mpsc, Mutex};

use bevy::audio::{AudioSinkPlayback, SpatialAudioSink, SpatialScale, Volume};
use bevy::prelude::*;

use crate::player::Player;
use crate::sim::rng::Rng;
use crate::sim::sfx::{self, approach, Mix, Mood, MusicContext, MusicDirector};
use crate::sim::synth::{Bus, Sound};
use crate::sim::terrain::{self, SHELTERS};
use crate::sim::weather::Phase;
use crate::state::{alive, Hostile, Messages, SfxQueue, WeatherRes};

/// The player's volume sliders and mute switch (shown in the Pip-Boy).
#[derive(Resource, Default)]
pub struct AudioSettings(pub Mix);

/// Whether the Geiger counter clicks (press G).
#[derive(Resource)]
pub struct GeigerOn(pub bool);

/// Every generated clip: (sound, variant) -> handle.
#[derive(Resource, Default)]
struct SoundBank(HashMap<(Sound, usize), Handle<AudioSource>>);

impl SoundBank {
    fn get(&self, sound: Sound, variant: usize) -> Option<Handle<AudioSource>> {
        self.0.get(&(sound, variant)).cloned()
    }
    fn has_all(&self, sounds: &[Sound]) -> bool {
        sounds.iter().all(|s| (0..s.variants()).all(|v| self.0.contains_key(&(*s, v))))
    }
}

/// Clips arriving from the generator threads.
#[derive(Resource)]
struct BankLoader(Mutex<mpsc::Receiver<(Sound, usize, Vec<u8>)>>);

/// Everything the audio systems remember between frames.
#[derive(Resource)]
struct AudioState {
    player: sfx::Player,
    rng: Rng,
    director: MusicDirector,
    /// Smoothed 0..1 wind strength.
    wind: f32,
    gust_timer: f32,
    /// Smoothed volume of each steered loop.
    levels: HashMap<LoopKind, f32>,
    loops_started: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum LoopKind {
    WindLow,
    WindMid,
    WindHigh,
    Siren,
    Music(Mood),
    PipHum,
}

impl LoopKind {
    const ALL: [LoopKind; 8] = [
        LoopKind::PipHum,
        LoopKind::WindLow,
        LoopKind::WindMid,
        LoopKind::WindHigh,
        LoopKind::Siren,
        LoopKind::Music(Mood::Calm),
        LoopKind::Music(Mood::Tense),
        LoopKind::Music(Mood::Danger),
    ];

    fn sound(self) -> Sound {
        match self {
            LoopKind::WindLow => Sound::WindLow,
            LoopKind::WindMid => Sound::WindMid,
            LoopKind::WindHigh => Sound::WindHigh,
            LoopKind::Siren => Sound::Siren,
            LoopKind::PipHum => Sound::PipHum,
            LoopKind::Music(m) => m.sound(),
        }
    }
}

/// A spatial crackle loop at one fire barrel.
#[derive(Component)]
struct FireLoop(usize);

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(143);
        let mut rng = Rng::new(seed ^ 0xA0D10);
        let director = MusicDirector::new(&mut rng);
        app.init_resource::<SoundBank>()
            .init_resource::<AudioSettings>()
            .insert_resource(GeigerOn(true))
            .insert_resource(AudioState {
                player: sfx::Player::new(),
                rng,
                director,
                wind: 0.0,
                gust_timer: 8.0,
                levels: HashMap::new(),
                loops_started: false,
            })
            .add_systems(Startup, start_generators)
            .add_systems(
                Update,
                (
                    receive_sounds,
                    audio_controls,
                    play_queued,
                    start_loops,
                    steer_loops,
                    gusts,
                    fire_loops,
                    geiger.run_if(alive),
                )
                    .chain(),
            );
    }
}

/// Generate every clip on background threads, a few at a time.
fn start_generators(mut commands: Commands) {
    let (tx, rx) = mpsc::channel();
    // Screenshot mode has no audio device and wants every core for rendering.
    if std::env::var("FMN_SHOT").is_ok() {
        commands.insert_resource(BankLoader(Mutex::new(rx)));
        return;
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 3);
    for t in 0..threads {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut i = 0;
            for &s in Sound::ALL {
                for v in 0..s.variants() {
                    if i % threads == t && tx.send((s, v, s.wav(v))).is_err() {
                        return;
                    }
                    i += 1;
                }
            }
        });
    }
    commands.insert_resource(BankLoader(Mutex::new(rx)));
}

fn receive_sounds(loader: Res<BankLoader>, mut bank: ResMut<SoundBank>, mut sources: ResMut<Assets<AudioSource>>) {
    let Ok(rx) = loader.0.lock() else { return };
    let total: usize = Sound::ALL.iter().map(|s| s.variants()).sum();
    for _ in 0..12 {
        match rx.try_recv() {
            Ok((sound, variant, wav)) => {
                bank.0.insert((sound, variant), sources.add(AudioSource { bytes: wav.into() }));
                if bank.0.len() == total {
                    info!("sound bank ready ({total} clips)");
                }
            }
            Err(_) => break,
        }
    }
}

/// F5/F6 effects, F7/F8 music, F10/F11 master volume, F9 mute, G Geiger.
fn audio_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<AudioSettings>,
    mut geiger: ResMut<GeigerOn>,
    mut msgs: ResMut<Messages>,
) {
    let m = &mut settings.0;
    let mut say: Option<String> = None;
    let pct = |v: f32| (v * 100.0).round() as i32;
    if keys.just_pressed(KeyCode::F5) || keys.just_pressed(KeyCode::F6) {
        Mix::nudge(&mut m.sfx, if keys.just_pressed(KeyCode::F6) { 0.1 } else { -0.1 });
        say = Some(format!("Effects volume {}%", pct(m.sfx)));
    }
    if keys.just_pressed(KeyCode::F7) || keys.just_pressed(KeyCode::F8) {
        Mix::nudge(&mut m.music, if keys.just_pressed(KeyCode::F8) { 0.1 } else { -0.1 });
        say = Some(format!("Music volume {}%", pct(m.music)));
    }
    if keys.just_pressed(KeyCode::F10) || keys.just_pressed(KeyCode::F11) {
        Mix::nudge(&mut m.master, if keys.just_pressed(KeyCode::F11) { 0.1 } else { -0.1 });
        m.muted = false;
        say = Some(format!("Master volume {}%", pct(m.master)));
    }
    if keys.just_pressed(KeyCode::F9) {
        m.muted = !m.muted;
        say = Some(if m.muted { "Sound muted (F9 to unmute)".into() } else { "Sound on".into() });
    }
    if keys.just_pressed(KeyCode::KeyG) {
        geiger.0 = !geiger.0;
        say = Some(format!("Geiger counter {}", if geiger.0 { "ON" } else { "OFF" }));
    }
    if let Some(text) = say {
        msgs.show(text, 1.8);
    }
}

fn play_queued(
    mut commands: Commands,
    real: Res<Time<Real>>,
    bank: Res<SoundBank>,
    settings: Res<AudioSettings>,
    mut queue: ResMut<SfxQueue>,
    mut state: ResMut<AudioState>,
) {
    let now = real.elapsed_secs();
    let state = &mut *state;
    for req in queue.0.drain(..) {
        let Some(play) = state.player.request(req.sound, now, req.gain, &mut state.rng) else {
            continue;
        };
        let Some(handle) = bank.get(req.sound, play.variant) else {
            continue; // still being generated
        };
        let prof = req.sound.profile();
        let volume = play.volume * settings.0.gain(prof.bus);
        if volume < 0.002 {
            continue;
        }
        let settings = PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_speed(play.speed);
        match (req.pos, prof.spatial_ref) {
            (Some(pos), Some(reference)) => {
                commands.spawn((
                    AudioPlayer::new(handle),
                    settings.with_spatial(true).with_spatial_scale(SpatialScale::new(1.0 / reference)),
                    Transform::from_translation(pos),
                ));
            }
            _ => {
                commands.spawn((AudioPlayer::new(handle), settings));
            }
        }
    }
}

/// Start the wind, siren and music loops (silent) once they are generated.
fn start_loops(mut commands: Commands, bank: Res<SoundBank>, mut state: ResMut<AudioState>) {
    if state.loops_started {
        return;
    }
    let needed: Vec<Sound> = LoopKind::ALL.iter().map(|k| k.sound()).collect();
    if !bank.has_all(&needed) {
        return;
    }
    state.loops_started = true;
    for kind in LoopKind::ALL {
        if let Some(handle) = bank.get(kind.sound(), 0) {
            commands.spawn((
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                kind,
            ));
        }
    }
}

/// Steer the loops: wind follows the weather (muffled near a shelter), the
/// siren swells during the warning, and the music director picks the mood.
#[allow(clippy::too_many_arguments)]
fn steer_loops(
    real: Res<Time<Real>>,
    weather: Res<WeatherRes>,
    settings: Res<AudioSettings>,
    mut state: ResMut<AudioState>,
    player: Query<&Transform, With<Player>>,
    hostiles: Query<&Transform, (With<Hostile>, Without<Player>)>,
    mut sinks: Query<(&mut AudioSink, &LoopKind)>,
    pip: Res<crate::state::PipOpen>,
) {
    let dt = real.delta_secs().min(0.25);
    let phase = weather.weather.phase;
    let wind_now = (weather.weather.conditions().wind / 14.0).clamp(0.0, 1.0);
    state.wind = approach(state.wind, wind_now, 0.08, dt);
    let w = state.wind;

    let ppos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let sheltered = terrain::shelter_at(ppos.x, ppos.z).is_some();
    let muffle = if sheltered { 0.45 } else { 1.0 };

    let enemy_dist = hostiles
        .iter()
        .map(|t| t.translation.distance(ppos))
        .fold(None, |best: Option<f32>, d| Some(best.map_or(d, |b| b.min(d))));
    let ctx = MusicContext {
        blizzard: phase == Phase::Blizzard,
        siren: phase == Phase::Warning,
        enemy_dist,
    };
    let state = &mut *state;
    let music = state.director.update(dt, &ctx, &mut state.rng);

    for (mut sink, kind) in &mut sinks {
        let (target, bus) = match kind {
            LoopKind::WindLow => ((0.45 + 0.5 * w) * 0.6, Bus::Ambience),
            LoopKind::WindMid => ((0.08 + 0.9 * w) * 0.55 * muffle, Bus::Ambience),
            LoopKind::WindHigh => ((0.03 + 0.95 * w * w) * 0.5 * muffle, Bus::Ambience),
            LoopKind::Siren => (if phase == Phase::Warning { 1.0 } else { 0.0 } * Sound::Siren.profile().volume, Bus::Ambience),
            LoopKind::Music(m) => (music[*m as usize], Bus::Music),
            LoopKind::PipHum => (if pip.0 { Sound::PipHum.profile().volume } else { 0.0 }, Bus::Sfx),
        };
        let level = state.levels.entry(*kind).or_insert(0.0);
        // The siren fades in over two seconds and out over about one.
        let rate = if *kind == LoopKind::Siren { 0.8 } else { 2.0 };
        *level = approach(*level, target, rate, dt);
        sink.set_volume(Volume::Linear(*level * settings.0.gain(bus)));
    }
}

/// Random one-shot gusts on top of the wind loops.
fn gusts(real: Res<Time<Real>>, mut state: ResMut<AudioState>, mut sfx: ResMut<SfxQueue>) {
    let dt = real.delta_secs();
    state.gust_timer -= dt;
    if state.gust_timer > 0.0 {
        return;
    }
    let w = state.wind;
    state.gust_timer = state.rng.range(14.0 - 9.0 * w, 30.0 - 21.0 * w);
    sfx.play_gain(Sound::Gust, 0.35 + 0.65 * w);
}

/// Crackle loops at the fire barrels near the player, one per fire, each
/// using a different loop so they never line up.
fn fire_loops(
    mut commands: Commands,
    bank: Res<SoundBank>,
    settings: Res<AudioSettings>,
    player: Query<&Transform, With<Player>>,
    mut fires: Query<(Entity, &FireLoop, &mut SpatialAudioSink)>,
    pending: Query<&FireLoop, Without<SpatialAudioSink>>,
) {
    let Ok(ptf) = player.single() else { return };
    let p = ptf.translation;
    let prof = Sound::Fire.profile();
    let gain = prof.volume * settings.0.gain(prof.bus);
    let fire_pos = |i: usize| {
        let (sx, sz) = SHELTERS[i];
        Vec3::new(sx + 1.5, terrain::walk_height(sx + 1.5, sz) + 1.1, sz)
    };

    for (entity, fire, mut sink) in &mut fires {
        if fire_pos(fire.0).distance(p) > 60.0 {
            commands.entity(entity).despawn();
        } else {
            sink.set_volume(Volume::Linear(gain));
        }
    }
    let running: Vec<usize> = fires.iter().map(|(_, f, _)| f.0).chain(pending.iter().map(|f| f.0)).collect();
    for i in 0..SHELTERS.len() {
        if running.contains(&i) || fire_pos(i).distance(p) > 45.0 {
            continue;
        }
        let Some(handle) = bank.get(Sound::Fire, i % Sound::Fire.variants()) else { continue };
        let reference = prof.spatial_ref.unwrap_or(5.0);
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::LOOP
                .with_volume(Volume::Linear(gain))
                .with_spatial(true)
                .with_spatial_scale(SpatialScale::new(1.0 / reference)),
            Transform::from_translation(fire_pos(i)),
            FireLoop(i),
        ));
    }
}

/// Clicks at a gentle rate that grows with the rads you are taking.
fn geiger(
    real: Res<Time<Real>>,
    on: Res<GeigerOn>,
    weather: Res<WeatherRes>,
    mut state: ResMut<AudioState>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
) {
    if !on.0 {
        return;
    }
    let Ok(ptf) = player.single() else { return };
    let (x, z) = (ptf.translation.x, ptf.translation.z);
    let sheltered = terrain::shelter_at(x, z).is_some();
    let rate = terrain::ambient_rads(x, z) + if sheltered { 0.0 } else { weather.weather.conditions().rads_per_sec };
    let cps = sfx::geiger_clicks_per_sec(rate);
    if cps <= 0.0 {
        return;
    }
    let p = 1.0 - (-cps * real.delta_secs()).exp();
    if state.rng.chance(p) {
        sfx.play(Sound::Geiger);
    }
}
