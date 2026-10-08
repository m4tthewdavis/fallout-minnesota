//! The colour grade and lens: hands `sim::grade`'s numbers to Bevy's
//! `ColorGrading` and `ChromaticAberration` on the cameras every frame,
//! easing between looks so dusk, storms and getting hurt roll in rather than
//! snap. Going through a door snaps (the fade hides it).

use bevy::core_pipeline::post_process::ChromaticAberration;
use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

use crate::gun::ViewModelCamera;
use crate::player::Player;
use crate::sim::grade::{self, Band, Grade, Mood};
use crate::sim::survival::Survival;
use crate::state::{ClockRes, CurrentInterior, Game};
use crate::weather_fx::{AtmosphereSet, SkyLook};

/// The grade on screen now (eased towards the wanted one).
#[derive(Resource)]
struct Current {
    grade: Grade,
    room: Option<crate::sim::interiors::Interior>,
}

pub struct GradePlugin;

impl Plugin for GradePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Current { grade: Grade::NEUTRAL, room: None }).add_systems(Update, apply_grade.after(AtmosphereSet));
    }
}

fn section(b: Band) -> ColorGradingSection {
    ColorGradingSection { saturation: b.saturation, contrast: b.contrast, gamma: b.gamma, gain: b.gain, lift: b.lift }
}

fn to_bevy(g: &Grade) -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal {
            exposure: g.exposure,
            temperature: g.temperature,
            tint: g.tint,
            post_saturation: g.saturation,
            ..default()
        },
        shadows: section(g.shadows),
        midtones: section(g.midtones),
        highlights: section(g.highlights),
    }
}

#[allow(clippy::type_complexity)]
fn apply_grade(
    mut commands: Commands,
    real: Res<Time<Real>>,
    look: Res<SkyLook>,
    clock: Res<ClockRes>,
    interior: Res<CurrentInterior>,
    game: Res<Game>,
    mut current: ResMut<Current>,
    mut world_cam: Query<(Entity, Option<&mut ColorGrading>, Option<&mut ChromaticAberration>), (With<Player>, Without<ViewModelCamera>)>,
    mut gun_cam: Query<(Entity, Option<&mut ColorGrading>), (With<ViewModelCamera>, Without<Player>)>,
) {
    let sky = clock.0.sky();
    let s = &game.survival;
    let mood = Mood {
        day: ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0),
        warmth: sky.warmth * sky.sun,
        overcast: look.overcast,
        sick: look.sick,
        interior: interior.0,
        health: (s.health / Survival::BASE_MAX_HEALTH).clamp(0.0, 1.0),
        body_heat: s.body_heat,
        rads: s.rads / Survival::MAX_RADS,
        hurt: game.hurt_flash,
    };
    let want = grade::grade(&mood);
    // Weather and light drift over a few seconds; a hit lands at once.
    let k = if current.room != interior.0 { 1.0 } else { 1.0 - (-real.delta_secs() * 1.5).exp() };
    current.room = interior.0;
    let mut g = current.grade.approach(want, k);
    g.aberration = g.aberration.max(want.aberration);
    current.grade = g;
    let cg = to_bevy(&g);

    for (cam, grading, fringe) in &mut world_cam {
        match grading {
            Some(mut c) => *c = cg.clone(),
            None => {
                commands.entity(cam).insert(cg.clone());
            }
        }
        // The fringe costs a pass, so it's only there when it shows.
        match (fringe, g.aberration > 0.0005) {
            (Some(mut f), true) => f.intensity = g.aberration,
            (None, true) => {
                commands.entity(cam).insert(ChromaticAberration { intensity: g.aberration, ..default() });
            }
            (Some(_), false) => {
                commands.entity(cam).remove::<ChromaticAberration>();
            }
            (None, false) => {}
        }
    }
    // The rifle is drawn by its own camera: graded the same so it sits in the world.
    for (cam, grading) in &mut gun_cam {
        match grading {
            Some(mut c) => *c = cg.clone(),
            None => {
                commands.entity(cam).insert(cg.clone());
            }
        }
    }
}
