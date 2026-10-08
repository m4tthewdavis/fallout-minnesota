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
use std::sync::{mpsc, Arc, Mutex};

use bevy::audio::{AudioSinkPlayback, SpatialAudioSink, SpatialScale, Volume};
use bevy::prelude::*;

use crate::sim::keys::Bind;

use crate::player::Player;
use crate::sim::collision::segment_cover;
use crate::sim::interiors::{self, Interior};
use crate::sim::rng::Rng;
use crate::sim::sfx::{self, approach, Mix, Mood, MusicContext, MusicDirector};
use crate::sim::soundscape::{self, Shelter};
use crate::sim::recorded::{self, Takes};
use crate::sim::synth::{Bus, Sound};
use crate::sim::terrain::{self, SHELTERS};
use crate::sim::weather::Phase;
use crate::state::{alive, Colliders, CurrentInterior, Hostile, Messages, SfxQueue, SfxReq, WeatherRes};

/// The player's volume sliders and mute switch (shown in the Pip-Boy).
#[derive(Resource, Default)]
pub struct AudioSettings(pub Mix);

/// Whether the Geiger counter clicks (press G).
#[derive(Resource)]
pub struct GeigerOn(pub bool);

/// Every generated clip: (sound, variant, heard through something) -> handle.
/// Gunfire, voices and boots also have a muffled version for when walls or
/// trees are in the way.
#[derive(Resource, Default)]
struct SoundBank(HashMap<(Sound, usize, bool), Handle<AudioSource>>);

impl SoundBank {
    fn get(&self, sound: Sound, variant: usize, muffled: bool) -> Option<Handle<AudioSource>> {
        // Until the muffled clip has been generated, play the clear one.
        self.0.get(&(sound, variant, muffled)).or_else(|| self.0.get(&(sound, variant, false))).cloned()
    }
    fn has_all(&self, sounds: &[Sound]) -> bool {
        sounds.iter().all(|s| (0..s.variants()).all(|v| self.0.contains_key(&(*s, v, false))))
    }
}

/// Clips arriving from the generator threads.
#[derive(Resource)]
struct BankLoader(Mutex<mpsc::Receiver<(Sound, usize, bool, Vec<u8>)>>);

/// Sounds waiting out their delay (the echo of a shot, a bolt's clack).
#[derive(Resource, Default)]
struct PendingSfx(Vec<(f32, SfxReq)>);

/// Seconds until the room's next creak or drip.
#[derive(Resource, Default)]
struct RoomNoiseTimer(f32);

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
    /// FMN_AUDIO_LOG=1: say what plays, and why it sounds the way it does.
    log: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum LoopKind {
    WindBreeze,
    WindLow,
    WindMid,
    WindHigh,
    WindHowl,
    RoomHum,
    Siren,
    Music(Mood),
    PipHum,
}

impl LoopKind {
    const ALL: [LoopKind; 11] = [
        LoopKind::PipHum,
        LoopKind::WindBreeze,
        LoopKind::WindLow,
        LoopKind::WindMid,
        LoopKind::WindHigh,
        LoopKind::WindHowl,
        LoopKind::RoomHum,
        LoopKind::Siren,
        LoopKind::Music(Mood::Calm),
        LoopKind::Music(Mood::Tense),
        LoopKind::Music(Mood::Danger),
    ];

    fn sound(self) -> Sound {
        match self {
            LoopKind::WindBreeze => Sound::WindBreeze,
            LoopKind::WindLow => Sound::WindLow,
            LoopKind::WindMid => Sound::WindMid,
            LoopKind::WindHigh => Sound::WindHigh,
            LoopKind::WindHowl => Sound::WindHowl,
            LoopKind::RoomHum => Sound::RoomHum,
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
            .init_resource::<PendingSfx>()
            .init_resource::<RoomNoiseTimer>()
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
                log: std::env::var("FMN_AUDIO_LOG").is_ok(),
            })
            .add_systems(Startup, start_generators)
            .add_systems(
                Update,
                (
                    receive_sounds,
                    audio_controls,
                    room_noises,
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
    if std::env::var("FMN_SHOT").is_ok() && std::env::var("FMN_AUDIO_LOG").is_err() {
        commands.insert_resource(BankLoader(Mutex::new(rx)));
        return;
    }
    // Recordings in assets/sounds stand in for the synthesised clips.
    let recorded = Arc::new(find_recordings());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 3);
    for t in 0..threads {
        let tx = tx.clone();
        let recorded = recorded.clone();
        std::thread::spawn(move || {
            let mut i = 0;
            for &s in Sound::ALL {
                let takes = recorded.iter().find(|(r, _)| *r == s).map(|(_, t)| t);
                for v in 0..s.variants() {
                    if i % threads == t {
                        let clip = |muffled: bool| {
                            takes.and_then(|t| t.pick(v, muffled)).and_then(|path| std::fs::read(&path).ok()).unwrap_or_else(|| if muffled { s.wav_muffled(v) } else { s.wav(v) })
                        };
                        if tx.send((s, v, false, clip(false))).is_err() {
                            return;
                        }
                        // The same clip as heard through a wall or a thicket.
                        if s.muffleable() && tx.send((s, v, true, clip(true))).is_err() {
                            return;
                        }
                    }
                    i += 1;
                }
            }
        });
    }
    commands.insert_resource(BankLoader(Mutex::new(rx)));
}

/// The recorded clips in `assets/sounds`, by sound.
fn find_recordings() -> Vec<(Sound, Takes<std::path::PathBuf>)> {
    let dir = std::path::Path::new(&crate::assets::asset_root()).join("sounds");
    let files = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()));
    let found = recorded::gather(files);
    let n: usize = found.iter().map(|(_, t)| t.plain.len() + t.muffled.len()).sum();
    info!("{n} recorded clips for {} sounds in {}", found.len(), dir.display());
    found
}

/// `FMN_DUMP_SYNTH=<folder>`: write every synthesised clip there as a WAV
/// (named like the recordings that would replace it) and quit. For matching
/// the loudness of new recordings to the mix.
pub fn dump_synth_if_asked() -> bool {
    let Ok(dir) = std::env::var("FMN_DUMP_SYNTH") else { return false };
    let dir = std::path::PathBuf::from(dir);
    let _ = std::fs::create_dir_all(&dir);
    for &s in Sound::ALL {
        for v in 0..s.variants() {
            let _ = std::fs::write(dir.join(format!("{}_{v}.wav", recorded::stem(s))), s.wav(v));
            if s.muffleable() {
                let _ = std::fs::write(dir.join(format!("{}_{v}_muffled.wav", recorded::stem(s))), s.wav_muffled(v));
            }
        }
    }
    println!("synthesised clips written to {}", dir.display());
    true
}

fn receive_sounds(loader: Res<BankLoader>, mut bank: ResMut<SoundBank>, mut sources: ResMut<Assets<AudioSource>>) {
    let Ok(rx) = loader.0.lock() else { return };
    let total: usize = Sound::ALL.iter().map(|s| s.variants() * if s.muffleable() { 2 } else { 1 }).sum();
    for _ in 0..16 {
        match rx.try_recv() {
            Ok((sound, variant, muffled, wav)) => {
                bank.0.insert((sound, variant, muffled), sources.add(AudioSource { bytes: wav.into() }));
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
    controls: crate::keybind::Controls,
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
    if controls.just_pressed(Bind::Geiger) {
        geiger.0 = !geiger.0;
        say = Some(format!("Geiger counter {}", if geiger.0 { "ON" } else { "OFF" }));
    }
    if let Some(text) = say {
        msgs.show(text, 1.8);
    }
}

/// Play what gameplay code asked for: work out how much of each positioned
/// sound gets through the trees and walls between it and you (and use the
/// dull version of the clip if not much), give gunshots the echo of the place
/// they were fired in, and hold back anything with a delay.
#[allow(clippy::too_many_arguments)]
fn play_queued(
    mut commands: Commands,
    real: Res<Time<Real>>,
    bank: Res<SoundBank>,
    settings: Res<AudioSettings>,
    colliders: Res<Colliders>,
    interior: Res<CurrentInterior>,
    mut queue: ResMut<SfxQueue>,
    mut pending: ResMut<PendingSfx>,
    mut state: ResMut<AudioState>,
    listener: Query<&Transform, With<Player>>,
) {
    let now = real.elapsed_secs();
    let state = &mut *state;
    let here = listener.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let listener_room = interior.0;

    // Anything whose time has come joins this frame's requests.
    let mut due: Vec<SfxReq> = Vec::new();
    pending.0.retain(|(at, req)| {
        if *at <= now {
            due.push(*req);
            false
        } else {
            true
        }
    });
    let mut echoes: Vec<SfxReq> = Vec::new();
    for req in queue.0.drain(..).chain(due) {
        if req.delay > 0.0 {
            pending.0.push((now + req.delay, SfxReq { delay: 0.0, ..req }));
            continue;
        }
        // What stands between the sound and you?
        let source_room = req.pos.and_then(|p| interiors::zone_at(p.x, p.z));
        let (heard, muffled) = match req.pos {
            Some(p) if p.distance(here) < 160.0 || source_room != listener_room => {
                let (trees, walls) = segment_cover((here.x, here.z), (p.x, p.z), &colliders.0);
                let h = soundscape::heard(soundscape::occlusion(source_room, listener_room, trees, walls));
                (h.gain, h.muffled)
            }
            _ => (1.0, false),
        };
        let Some(play) = state.player.request(req.sound, now, req.gain * heard, &mut state.rng) else {
            continue;
        };
        let muffled = muffled && req.sound.muffleable();
        let Some(handle) = bank.get(req.sound, play.variant, muffled) else {
            continue; // still being generated
        };
        let prof = req.sound.profile();
        let volume = play.volume * settings.0.gain(prof.bus);
        if volume < 0.002 {
            continue;
        }
        // A gunshot leaves an echo: long and rolling in the open, short and bright in a room.
        if soundscape::is_gunshot(req.sound) {
            let room = if req.pos.is_some() { source_room } else { listener_room };
            let gain = req.gain * soundscape::tail_gain(req.sound);
            let tail = SfxReq { sound: soundscape::shot_tail(room), gain, delay: if room.is_some() { 0.025 } else { 0.07 }, ..req };
            echoes.push(tail);
        }
        if state.log {
            info!("sfx: {:?} v{} muffled={muffled} heard={heard:.2} volume={volume:.2} speed={:.2} spatial={:?} room={:?}->{:?} at={:?}", req.sound, play.variant, play.speed, prof.spatial_ref.or(req.reference), source_room, listener_room, req.pos);
        }
        let playback = PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(play.speed);
        match (req.pos, prof.spatial_ref.or(req.reference)) {
            (Some(pos), Some(reference)) => {
                commands.spawn((
                    AudioPlayer::new(handle),
                    playback.with_spatial(true).with_spatial_scale(SpatialScale::new(1.0 / reference)),
                    Transform::from_translation(pos),
                ));
            }
            _ => {
                commands.spawn((AudioPlayer::new(handle), playback));
            }
        }
    }
    for e in echoes {
        pending.0.push((now + e.delay, SfxReq { delay: 0.0, ..e }));
    }
}

/// The random small noises a room makes: timber creaking in a fish house,
/// drips in the stockroom.
fn room_noises(real: Res<Time<Real>>, interior: Res<CurrentInterior>, mut timer: ResMut<RoomNoiseTimer>, mut state: ResMut<AudioState>, mut sfx: ResMut<SfxQueue>) {
    let Some((sound, lo, hi)) = soundscape::room_noise(interior.0) else {
        timer.0 = 3.0;
        return;
    };
    timer.0 -= real.delta_secs();
    if timer.0 <= 0.0 {
        timer.0 = state.rng.range(lo, hi);
        sfx.play_gain(sound, state.rng.range(0.6, 1.0));
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
        if let Some(handle) = bank.get(kind.sound(), 0, false) {
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
fn steer_loops(
    real: Res<Time<Real>>,
    weather: Res<WeatherRes>,
    settings: Res<AudioSettings>,
    mut state: ResMut<AudioState>,
    player: Query<&Transform, With<Player>>,
    hostiles: Query<&Transform, (With<Hostile>, Without<Player>)>,
    mut sinks: Query<(&mut AudioSink, &LoopKind)>,
    pip: Res<crate::state::PipOpen>,
    interior: Res<CurrentInterior>,
) {
    let dt = real.delta_secs().min(0.25);
    let phase = weather.weather.phase;
    let wind_now = (weather.weather.conditions().wind / 14.0).clamp(0.0, 1.0);
    state.wind = approach(state.wind, wind_now, 0.08, dt);
    let w = state.wind;

    let ppos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    // Layers of wind: breeze, rumble, rush, hiss and (in a blizzard) a howl; behind
    // walls only the dull rumble gets through.
    let mix = soundscape::wind_mix(w, Shelter::of(interior.0));
    let hum = soundscape::room_hum_level(interior.0);

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
            LoopKind::WindBreeze => (mix.breeze, Bus::Ambience),
            LoopKind::WindLow => (mix.low, Bus::Ambience),
            LoopKind::WindMid => (mix.mid, Bus::Ambience),
            LoopKind::WindHigh => (mix.high, Bus::Ambience),
            LoopKind::WindHowl => (mix.howl, Bus::Ambience),
            LoopKind::RoomHum => (hum, Bus::Ambience),
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
    // Fires 0..4 are the barrels beside the fish houses; 4..8 are the stoves inside them.
    let fire_pos = |i: usize| {
        if i < SHELTERS.len() {
            let (sx, sz) = SHELTERS[i];
            Vec3::new(sx + 1.5, terrain::walk_height(sx + 1.5, sz) + 1.1, sz)
        } else {
            let room = Interior::FishHouse((i - SHELTERS.len()) as u8);
            let (hw, hd) = room.half();
            let (x, z) = room.at(hw - 0.75, -hd + 0.8);
            Vec3::new(x, interiors::FLOOR_Y + 1.0, z)
        }
    };

    for (entity, fire, mut sink) in &mut fires {
        if fire_pos(fire.0).distance(p) > 60.0 {
            commands.entity(entity).despawn();
        } else {
            sink.set_volume(Volume::Linear(gain));
        }
    }
    let running: Vec<usize> = fires.iter().map(|(_, f, _)| f.0).chain(pending.iter().map(|f| f.0)).collect();
    for i in 0..SHELTERS.len() * 2 {
        if running.contains(&i) || fire_pos(i).distance(p) > 45.0 {
            continue;
        }
        let Some(handle) = bank.get(Sound::Fire, i % Sound::Fire.variants(), false) else { continue };
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
    let sheltered = terrain::cover_at(x, z).sheltered();
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
