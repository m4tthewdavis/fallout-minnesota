//! Plays the synthesised sound effects and keeps the ambient loops (wind,
//! blizzard gale, air-raid siren, fire crackle, music) in step with the world.
//! Also runs the Geiger counter, which clicks faster the more rads you take.

use std::collections::HashMap;

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;

use crate::player::Player;
use crate::sim::synth::Sound;
use crate::sim::terrain::{self, SHELTERS};
use crate::sim::weather::Phase;
use crate::state::{alive, RngRes, SfxQueue, WeatherRes};

/// A looping sound whose volume is steered every frame.
#[derive(Component)]
enum Steered {
    Fire,
    Music,
}

#[derive(Resource)]
struct SoundBank(HashMap<Sound, Handle<AudioSource>>);

/// The ambient loops currently playing.
#[derive(Resource, Default)]
struct AmbientLoops {
    wind: Option<(Sound, Entity)>,
    siren: Option<Entity>,
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AmbientLoops>()
            .add_systems(Startup, build_sound_bank)
            .add_systems(
                Update,
                (play_queued, ambient_loops, start_steered_loops, steer_loops, geiger.run_if(alive)),
            );
    }
}

fn build_sound_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let bank = Sound::ALL
        .iter()
        .map(|&s| {
            let handle = sources.add(AudioSource { bytes: s.wav().into() });
            (s, handle)
        })
        .collect();
    commands.insert_resource(SoundBank(bank));
}

fn play_queued(mut commands: Commands, bank: Option<Res<SoundBank>>, mut queue: ResMut<SfxQueue>) {
    let Some(bank) = bank else { return };
    // Never play the same effect twice in one frame (e.g. two wolves biting).
    queue.0.sort_by_key(|s| *s as u8);
    queue.0.dedup();
    for sound in queue.0.drain(..) {
        if let Some(handle) = bank.0.get(&sound) {
            commands.spawn((AudioPlayer::new(handle.clone()), PlaybackSettings::DESPAWN));
        }
    }
}

fn ambient_loops(
    mut commands: Commands,
    bank: Option<Res<SoundBank>>,
    weather: Res<WeatherRes>,
    mut loops: ResMut<AmbientLoops>,
) {
    let Some(bank) = bank else { return };
    let phase = weather.weather.phase;

    let want_wind = if phase == Phase::Blizzard {
        Sound::WindStorm
    } else {
        Sound::WindCalm
    };
    if loops.wind.map(|(s, _)| s) != Some(want_wind) {
        if let Some((_, e)) = loops.wind.take() {
            commands.entity(e).despawn();
        }
        if let Some(handle) = bank.0.get(&want_wind) {
            let e = commands
                .spawn((AudioPlayer::new(handle.clone()), PlaybackSettings::LOOP))
                .id();
            loops.wind = Some((want_wind, e));
        }
    }

    let want_siren = phase == Phase::Warning;
    match (want_siren, loops.siren) {
        (true, None) => {
            if let Some(handle) = bank.0.get(&Sound::Siren) {
                let e = commands
                    .spawn((AudioPlayer::new(handle.clone()), PlaybackSettings::LOOP))
                    .id();
                loops.siren = Some(e);
            }
        }
        (false, Some(e)) => {
            commands.entity(e).despawn();
            loops.siren = None;
        }
        _ => {}
    }
}

fn start_steered_loops(mut commands: Commands, bank: Option<Res<SoundBank>>, mut started: Local<bool>) {
    let Some(bank) = bank else { return };
    if *started {
        return;
    }
    *started = true;
    for (sound, steered) in [(Sound::FireCrackle, Steered::Fire), (Sound::Music, Steered::Music)] {
        if let Some(handle) = bank.0.get(&sound) {
            commands.spawn((
                AudioPlayer::new(handle.clone()),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                steered,
            ));
        }
    }
}

/// Fire crackle swells as you approach a fire barrel; the music ducks under
/// the storm.
fn steer_loops(
    time: Res<Time>,
    weather: Res<WeatherRes>,
    player: Query<&Transform, With<Player>>,
    mut sinks: Query<(&mut AudioSink, &Steered)>,
    mut levels: Local<[f32; 2]>,
) {
    let Ok(ptf) = player.single() else { return };
    let (x, z) = (ptf.translation.x, ptf.translation.z);
    let nearest = SHELTERS
        .iter()
        .map(|&(sx, sz)| (sx + 1.5 - x).hypot(sz - z))
        .fold(f32::MAX, f32::min);
    let fire = (1.0 - nearest / 20.0).max(0.0).powi(2) * 0.9;
    let music = match weather.weather.phase {
        Phase::Calm => 0.32,
        Phase::Warning => 0.2,
        Phase::Blizzard => 0.1,
    };
    let k = (time.delta_secs() * 1.5).min(1.0);
    levels[0] += (fire - levels[0]) * k;
    levels[1] += (music - levels[1]) * k * 0.5;
    for (mut sink, steered) in &mut sinks {
        let v = match steered {
            Steered::Fire => levels[0],
            Steered::Music => levels[1],
        };
        sink.set_volume(Volume::Linear(v));
    }
}

/// Clicks at a rate proportional to the rads you're soaking up right now.
fn geiger(
    time: Res<Time>,
    weather: Res<WeatherRes>,
    mut rng: ResMut<RngRes>,
    mut sfx: ResMut<SfxQueue>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(ptf) = player.single() else { return };
    let (x, z) = (ptf.translation.x, ptf.translation.z);
    let sheltered = terrain::shelter_at(x, z).is_some();
    let rate = terrain::ambient_rads(x, z) + if sheltered { 0.0 } else { weather.weather.conditions().rads_per_sec };
    if rate <= 0.0 {
        return;
    }
    let clicks_per_sec = (rate * 2.5).min(40.0);
    let p = 1.0 - (-clicks_per_sec * time.delta_secs()).exp();
    if rng.0.chance(p) {
        sfx.play(if rng.0.chance(0.5) { Sound::Geiger1 } else { Sound::Geiger2 });
    }
}
