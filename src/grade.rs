//! The colour grade and lens: hands `sim::grade`'s numbers to Bevy's
//! `ColorGrading` and `ChromaticAberration` on the last camera every frame,
//! easing between looks so dusk, storms and getting hurt roll in rather than
//! snap. Going through a door snaps (the fade hides it).

use bevy::core_pipeline::post_process::ChromaticAberration;
use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

use crate::gun::ViewModelCamera;
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
    mut gun_cam: Query<(Entity, Option<&mut ColorGrading>, Option<&mut ChromaticAberration>), With<ViewModelCamera>>,
) {
    let sky = clock.0.sky();
    let s = &game.survival;
    let mood = Mood {
        day: ((sky.daylight - 0.12) / 0.88).clamp(0.0, 1.0),
        warmth: sky.warmth * sky.sun,
        overcast: look.overcast,
        sick: look.sick,
        interior: interior.0,
        // Of the maximum radiation has left, so a full bar never looks like dying.
        health: (s.health / s.max_health().max(1.0)).clamp(0.0, 1.0),
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

    // Both HDR cameras draw into one shared frame; the view-model camera is
    // drawn last and tonemaps the lot, so the grade and the lens fringe go
    // on it alone (on the world camera too, the world was graded twice).
    for (cam, grading, fringe) in &mut gun_cam {
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
}
