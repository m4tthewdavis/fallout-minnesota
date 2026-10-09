//! Performance: a frame-rate display (Settings > Graphics > Show Frame Rate),
//! a profile log for real hardware (`--profile` or `FMN_PROFILE=1`: a line
//! every five seconds in the log and in `profile.csv` in the save folder), a
//! repeatable benchmark (`--benchmark` or `FMN_BENCH=<seconds>`: the camera
//! flies a fixed loop over the map, then writes `benchmark.txt` and quits),
//! and level of detail: small props and the fine detail on people stop being
//! drawn at a distance.

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::transform::TransformSystem;

use crate::assets::GameAssets;
use crate::menu::GameSettings;
use crate::player::{Player, EYE_HEIGHT};
use crate::sim::lod::{self, keep_visible};
use crate::sim::perf::{FrameStats, CSV_HEADER};
use crate::sim::terrain;
use crate::state::{Game, Paused, TitleScreen};
use crate::storage;
use crate::theme::{FROST, HUD_PANEL};

/// What a thing is, for how far away it stops being drawn.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lod {
    /// A small prop: follows the view distance setting.
    Prop,
    /// A boulder or big rock: goes with the trees (never on Far).
    Large,
    /// Fur tufts, frost, faces and the like on a person.
    PersonDetail,
    Crow,
}

/// How many LOD-managed things are hidden right now.
#[derive(Resource, Default)]
struct LodStats {
    total: usize,
    hidden: usize,
}

#[derive(Resource)]
struct PerfState {
    /// The last ten seconds or so, for the display.
    recent: FrameStats,
    /// Everything since the last profile line (or the whole benchmark).
    window: FrameStats,
    profile: bool,
    since_line: f32,
}

/// The benchmark's progress: seconds flown and how long to fly.
#[derive(Resource)]
pub struct Benchmark {
    pub t: f32,
    pub secs: f32,
    frames: FrameStats,
}

impl Benchmark {
    /// Asked for on the command line or in the environment?
    pub fn requested() -> Option<f32> {
        if let Some(secs) = std::env::var("FMN_BENCH").ok().and_then(|v| v.parse::<f32>().ok()) {
            return Some(secs.clamp(5.0, 600.0));
        }
        std::env::args().any(|a| a == "--benchmark").then_some(60.0)
    }
}

#[derive(Component)]
struct FpsText;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        let profile = std::env::var("FMN_PROFILE").is_ok() || std::env::args().any(|a| a == "--profile");
        app.insert_resource(PerfState { recent: FrameStats::new(600), window: FrameStats::new(100_000), profile, since_line: 0.0 })
            .init_resource::<LodStats>()
            .add_systems(Startup, build_overlay)
            .add_systems(Update, (measure, apply_lod, show_overlay));
        if let Some(secs) = Benchmark::requested() {
            info!("benchmark: flying for {secs:.0} s");
            app.insert_resource(Benchmark { t: 0.0, secs, frames: FrameStats::new(200_000) })
                .add_systems(PostUpdate, bench_camera.before(TransformSystem::TransformPropagate));
        }
    }
}

fn build_overlay(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, right: Val::Px(12.0), top: Val::Px(108.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), display: Display::None, ..default() },
        BackgroundColor(HUD_PANEL),
        Text::new(""),
        TextFont { font: assets.font.clone(), font_size: 13.0, ..default() },
        TextColor(FROST),
        GlobalZIndex(70),
        FpsText,
    ));
}

/// Record every frame; write a profile line every five seconds if asked.
fn measure(real: Res<Time<Real>>, mut perf: ResMut<PerfState>, bench: Option<ResMut<Benchmark>>, lods: Res<LodStats>, entities: Query<Entity>) {
    let dt = real.delta_secs();
    perf.recent.push(dt);
    perf.window.push(dt);
    if let Some(mut b) = bench {
        // The first two seconds are loading and shader warm-up: not counted.
        if b.t > 2.0 {
            b.frames.push(dt);
        }
    }
    if !perf.profile {
        return;
    }
    perf.since_line += dt;
    if perf.since_line < 5.0 {
        return;
    }
    perf.since_line = 0.0;
    if let Some(s) = perf.window.summary() {
        let count = entities.iter().count();
        info!("perf: {}  entities {count}  lod hidden {}/{}", s.line(), lods.hidden, lods.total);
        let dir = storage::data_dir();
        let old = storage::read(&dir, "profile.csv").unwrap_or_else(|| format!("{CSV_HEADER},entities\n"));
        let _ = storage::write(&dir, "profile.csv", &format!("{old}{},{count}\n", s.csv()));
    }
    perf.window = FrameStats::new(100_000);
}

/// Hide what's too far away to matter. A quarter of the things are checked
/// each frame, so the cost stays flat however many there are.
fn apply_lod(
    settings: Res<GameSettings>,
    mut frame: Local<u32>,
    mut stats: ResMut<LodStats>,
    cam: Query<&GlobalTransform, With<Player>>,
    mut things: Query<(Entity, &GlobalTransform, &Lod, &mut Visibility)>,
) {
    let Ok(cam) = cam.single() else { return };
    let here = cam.translation();
    *frame = frame.wrapping_add(1);
    let slice = *frame % 4;
    let prop_cut = lod::prop_cutoff(settings.0.view);
    let large_cut = lod::large_cutoff(settings.0.view);
    let (mut total, mut hidden) = (0, 0);
    for (e, tf, lod, mut vis) in &mut things {
        total += 1;
        let shown = *vis != Visibility::Hidden;
        if e.index() % 4 == slice {
            let cutoff = match lod {
                Lod::Prop => prop_cut,
                Lod::Large => large_cut,
                Lod::PersonDetail => lod::PERSON_DETAIL,
                Lod::Crow => lod::CROW,
            };
            let want = keep_visible(tf.translation().distance(here), cutoff, shown);
            if want != shown {
                *vis = if want { Visibility::Inherited } else { Visibility::Hidden };
            }
            if !want {
                hidden += 1;
            }
        } else if !shown {
            hidden += 1;
        }
    }
    *stats = LodStats { total, hidden };
}

fn show_overlay(
    real: Res<Time<Real>>,
    settings: Res<GameSettings>,
    perf: Res<PerfState>,
    lods: Res<LodStats>,
    bench: Option<Res<Benchmark>>,
    mut since: Local<f32>,
    mut q: Query<(&mut Node, &mut Text), With<FpsText>>,
) {
    let Ok((mut node, mut text)) = q.single_mut() else { return };
    let show = settings.0.show_fps || perf.profile || bench.is_some();
    let d = if show { Display::Flex } else { Display::None };
    if node.display != d {
        node.display = d;
    }
    *since += real.delta_secs();
    if !show || *since < 0.5 {
        return;
    }
    *since = 0.0;
    if let Some(s) = perf.recent.summary() {
        let bench_line = bench.map(|b| format!("\nBENCHMARK {:.0} / {:.0} s", b.t, b.secs)).unwrap_or_default();
        text.set_if_neq(Text::new(format!("{}\nfar detail hidden {}/{}{bench_line}", s.line(), lods.hidden, lods.total)));
    }
}

/// The benchmark's flight: a slow loop over the woods, lakes and landmarks
/// at head height, the same every run. Writes the result and quits at the end.
#[allow(clippy::too_many_arguments)]
fn bench_camera(
    real: Res<Time<Real>>,
    mut bench: ResMut<Benchmark>,
    mut game: ResMut<Game>,
    mut title: ResMut<TitleScreen>,
    mut paused: ResMut<Paused>,
    mut vtime: ResMut<Time<Virtual>>,
    settings: Res<GameSettings>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut cam: Query<(&mut Transform, &mut Player)>,
    mut exit: EventWriter<AppExit>,
) {
    // No title screen, nothing can kill the camera.
    if title.0 || paused.0 {
        title.0 = false;
        paused.0 = false;
        vtime.unpause();
    }
    game.survival.god = true;
    // Real seconds: the run lasts the same time however slow the frames are.
    bench.t += real.delta_secs();
    let Ok((mut tf, mut p)) = cam.single_mut() else { return };
    // A loop of radius 130 m round the middle of the map, once a minute.
    let a = bench.t / 60.0 * std::f32::consts::TAU;
    let pos = Vec2::new(a.cos() * 130.0, a.sin() * 130.0);
    let ahead = Vec2::new((a + 0.1).cos() * 130.0, (a + 0.1).sin() * 130.0);
    let y = terrain::walk_height(pos.x, pos.y) + EYE_HEIGHT + 1.0;
    tf.translation = Vec3::new(pos.x, y, pos.y);
    tf.look_at(Vec3::new(ahead.x, y - 0.5, ahead.y), Vec3::Y);
    let (yaw, pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
    (p.yaw, p.pitch) = (yaw, pitch);
    if bench.t < bench.secs {
        return;
    }
    let s = &settings.0;
    let gpu = adapter.map(|a| format!("{} ({:?}, {:?})", a.name, a.device_type, a.backend)).unwrap_or_else(|| "unknown".into());
    let result = match bench.frames.summary() {
        Some(r) => r.line(),
        None => "no frames measured".to_string(),
    };
    let report = format!(
        "Fallout: Minnesota {} benchmark ({:.0} s)\nGPU: {gpu}\nSettings: shadows {}, view {}, volumetric fog {}, ambient occlusion {}\nResult: {result}\n",
        env!("CARGO_PKG_VERSION"),
        bench.secs,
        s.shadows.label(),
        s.view.label(),
        if s.volumetrics { "on" } else { "off" },
        if s.ambient_occlusion { "on" } else { "off" },
    );
    info!("benchmark: {}", report.replace('\n', " | "));
    let dir = storage::data_dir();
    match storage::write(&dir, "benchmark.txt", &report) {
        Ok(()) => info!("benchmark: written to {}", dir.join("benchmark.txt").display()),
        Err(e) => warn!("benchmark: could not write the report: {e}"),
    }
    exit.write(AppExit::Success);
}
