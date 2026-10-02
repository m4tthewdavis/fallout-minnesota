//! Plays the synthesised sound effects and keeps the ambient loops (wind,
//! blizzard gale, air-raid siren) in step with the weather.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::sim::synth::Sound;
use crate::sim::weather::Phase;
use crate::state::{SfxQueue, WeatherRes};

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
            .add_systems(Update, (play_queued, ambient_loops));
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
