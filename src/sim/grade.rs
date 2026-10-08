//! The camera's colour grade: how the picture is tinted, how much contrast
//! and colour it has, and how much the lens fringes. Worked out from the
//! light (day, golden hour, night, storm), where you are (each room has its
//! own look) and how you are (cold, irradiated, hurt, dying), so the whole
//! frame tells you something. Pure numbers; `grade.rs` hands them to Bevy.

use crate::sim::interiors::Interior;
use crate::sim::mathx::{lerp, smoothstep};

/// What the grade is worked out from.
#[derive(Clone, Copy, Debug)]
pub struct Mood {
    /// 0 at deepest night, 1 in full day.
    pub day: f32,
    /// 0..=1 how orange the light is (sunrise and sunset).
    pub warmth: f32,
    /// 0 clear, 1 socked in.
    pub overcast: f32,
    /// 0..=1 radioactive green in the air (a rad-blizzard).
    pub sick: f32,
    pub interior: Option<Interior>,
    /// Health as a fraction of its maximum.
    pub health: f32,
    /// Body heat, 0..=100.
    pub body_heat: f32,
    /// Rads as a fraction of the maximum.
    pub rads: f32,
    /// The red flash after a hit, 0..=1.
    pub hurt: f32,
}

impl Default for Mood {
    fn default() -> Self {
        Mood { day: 1.0, warmth: 0.0, overcast: 0.0, sick: 0.0, interior: None, health: 1.0, body_heat: 100.0, rads: 0.0, hurt: 0.0 }
    }
}

/// One band of the picture (shadows, midtones or highlights).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub saturation: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub gain: f32,
    pub lift: f32,
}

impl Band {
    pub const NEUTRAL: Band = Band { saturation: 1.0, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 };

    fn lerp(self, o: Band, t: f32) -> Band {
        Band {
            saturation: lerp(self.saturation, o.saturation, t),
            contrast: lerp(self.contrast, o.contrast, t),
            gamma: lerp(self.gamma, o.gamma, t),
            gain: lerp(self.gain, o.gain, t),
            lift: lerp(self.lift, o.lift, t),
        }
    }
}

/// The finished grade. Temperature and tint shift Bevy's white point (CIE xy):
/// tiny numbers, since 0.02 already takes a sixth of the red out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// Stops of exposure.
    pub exposure: f32,
    /// Warmer (+) or bluer (-).
    pub temperature: f32,
    /// More magenta (+) or greener (-).
    pub tint: f32,
    /// Saturation after tonemapping.
    pub saturation: f32,
    pub shadows: Band,
    pub midtones: Band,
    pub highlights: Band,
    /// Colour fringing towards the edges of the screen.
    pub aberration: f32,
}

impl Grade {
    pub const NEUTRAL: Grade = Grade {
        exposure: 0.0,
        temperature: 0.0,
        tint: 0.0,
        saturation: 1.0,
        shadows: Band::NEUTRAL,
        midtones: Band::NEUTRAL,
        highlights: Band::NEUTRAL,
        aberration: 0.0,
    };

    /// Ease towards another grade; `k` is the fraction of the way to go.
    pub fn approach(self, to: Grade, k: f32) -> Grade {
        let k = k.clamp(0.0, 1.0);
        Grade {
            exposure: lerp(self.exposure, to.exposure, k),
            temperature: lerp(self.temperature, to.temperature, k),
            tint: lerp(self.tint, to.tint, k),
            saturation: lerp(self.saturation, to.saturation, k),
            shadows: self.shadows.lerp(to.shadows, k),
            midtones: self.midtones.lerp(to.midtones, k),
            highlights: self.highlights.lerp(to.highlights, k),
            aberration: lerp(self.aberration, to.aberration, k),
        }
    }

    fn blend(self, to: Grade, t: f32) -> Grade {
        self.approach(to, t)
    }
}

/// A clear winter day: crisp, a little extra contrast, cool shadows, snow
/// that stays white without blowing out.
fn clear_day() -> Grade {
    Grade {
        exposure: 0.15,
        temperature: -0.004,
        tint: 0.0,
        saturation: 1.06,
        shadows: Band { saturation: 1.1, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 },
        midtones: Band { saturation: 1.04, contrast: 1.05, gamma: 1.0, gain: 1.02, lift: 0.0 },
        highlights: Band { saturation: 0.9, contrast: 1.04, gamma: 1.0, gain: 1.0, lift: 0.0 },
        aberration: 0.0,
    }
}

/// Sunrise and sunset: warm, rich and a touch more contrast.
fn golden() -> Grade {
    Grade {
        exposure: 0.1,
        temperature: 0.016,
        tint: 0.002,
        saturation: 1.14,
        shadows: Band { saturation: 1.15, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 },
        midtones: Band { saturation: 1.12, contrast: 1.05, gamma: 1.0, gain: 1.03, lift: 0.0 },
        highlights: Band { saturation: 1.0, contrast: 1.05, gamma: 1.0, gain: 1.02, lift: 0.0 },
        aberration: 0.0,
    }
}

/// Night: blue, washed of colour, with deep but not crushed blacks.
fn night() -> Grade {
    Grade {
        exposure: 0.35,
        temperature: -0.022,
        tint: 0.002,
        saturation: 0.72,
        shadows: Band { saturation: 0.8, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.004 },
        midtones: Band { saturation: 0.8, contrast: 1.05, gamma: 1.0, gain: 1.0, lift: 0.0 },
        highlights: Band { saturation: 0.85, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 },
        aberration: 0.0,
    }
}

/// A storm or heavy overcast: flat, cold, grey.
fn storm() -> Grade {
    Grade {
        exposure: 0.1,
        temperature: -0.008,
        tint: 0.0,
        saturation: 0.82,
        shadows: Band { saturation: 0.85, contrast: 1.0, gamma: 1.0, gain: 1.0, lift: 0.0 },
        midtones: Band { saturation: 0.85, contrast: 1.04, gamma: 1.0, gain: 1.0, lift: 0.0 },
        highlights: Band { saturation: 0.85, contrast: 1.0, gamma: 1.0, gain: 0.98, lift: 0.0 },
        aberration: 0.0,
    }
}

/// Each room's own look.
fn room(r: Interior) -> Grade {
    let base = Grade { shadows: Band { contrast: 1.0, lift: 0.0, ..Band::NEUTRAL }, midtones: Band { contrast: 1.04, ..Band::NEUTRAL }, ..Grade::NEUTRAL };
    match r {
        // Lamp-lit plank walls: warm and close.
        Interior::FishHouse(_) => Grade { temperature: 0.006, saturation: 1.0, ..base },
        // Clean, blue and clinical.
        Interior::VaultLobby => Grade { temperature: -0.008, saturation: 0.95, ..base },
        // A dead shop: drained of colour.
        Interior::Mart => Grade { temperature: -0.004, saturation: 0.8, ..base },
        // Teal machine light, hard contrast.
        Interior::Reactor => Grade {
            temperature: -0.01,
            tint: -0.004,
            saturation: 1.05,
            shadows: Band { contrast: 1.0, lift: 0.0, ..Band::NEUTRAL },
            midtones: Band { contrast: 1.05, ..Band::NEUTRAL },
            ..base
        },
    }
}

/// The grade for this moment.
pub fn grade(m: &Mood) -> Grade {
    let mut g = match m.interior {
        Some(r) => room(r),
        None => {
            let day = clear_day().blend(golden(), smoothstep(0.1, 0.8, m.warmth) * 0.9);
            let lit = night().blend(day, smoothstep(0.0, 0.6, m.day));
            let mut g = lit.blend(storm(), m.overcast * 0.8);
            // The rad-blizzard turns everything sickly green.
            g.tint -= 0.016 * m.sick;
            g.saturation *= 1.0 - 0.1 * m.sick;
            g
        }
    };

    // How you are colours how you see.
    // Cold: the picture drains towards blue as body heat runs out.
    let cold = 1.0 - smoothstep(10.0, 55.0, m.body_heat);
    g.temperature -= 0.02 * cold;
    g.saturation *= 1.0 - 0.25 * cold;
    // Radiation: a green cast and a faint fringe.
    let rads = smoothstep(0.15, 0.7, m.rads);
    g.tint -= 0.01 * rads;
    g.aberration += 0.006 * rads;
    // Badly hurt: colour bleeds out, contrast climbs, the edges fringe.
    let dying = 1.0 - smoothstep(0.12, 0.4, m.health);
    g.saturation *= 1.0 - 0.6 * dying;
    g.midtones.contrast += 0.04 * dying;
    g.aberration += 0.012 * dying;
    // A hit: a jolt of fringing.
    g.aberration += 0.03 * m.hurt.clamp(0.0, 1.0);
    g.aberration = g.aberration.min(0.05);
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> Mood {
        Mood::default()
    }

    #[test]
    fn a_clear_day_is_crisp_and_unfringed() {
        let g = grade(&day());
        assert!(g.midtones.contrast > 1.0);
        assert!(g.saturation >= 1.0);
        assert_eq!(g.aberration, 0.0);
    }

    #[test]
    fn sunset_is_warmer_than_noon_and_night_is_bluer() {
        let noon = grade(&day());
        let dusk = grade(&Mood { warmth: 1.0, day: 0.7, ..day() });
        let midnight = grade(&Mood { day: 0.0, ..day() });
        assert!(dusk.temperature > noon.temperature + 0.01);
        assert!(midnight.temperature < noon.temperature - 0.01);
        assert!(midnight.saturation < noon.saturation);
    }

    #[test]
    fn storms_flatten_and_cool_the_picture() {
        let clear = grade(&day());
        let storm = grade(&Mood { overcast: 1.0, ..day() });
        assert!(storm.saturation < clear.saturation);
        assert!(storm.temperature < clear.temperature);
    }

    #[test]
    fn rad_blizzards_go_green() {
        let g = grade(&Mood { sick: 1.0, overcast: 1.0, ..day() });
        assert!(g.tint < -0.01);
    }

    #[test]
    fn rooms_have_their_own_looks() {
        let fish = grade(&Mood { interior: Some(Interior::FishHouse(0)), ..day() });
        let vault = grade(&Mood { interior: Some(Interior::VaultLobby), ..day() });
        let mart = grade(&Mood { interior: Some(Interior::Mart), ..day() });
        assert!(fish.temperature > 0.0 && vault.temperature < 0.0);
        assert!(mart.saturation < vault.saturation);
        // Indoors the time of day doesn't matter.
        assert_eq!(grade(&Mood { day: 0.0, interior: Some(Interior::Reactor), ..day() }), grade(&Mood { interior: Some(Interior::Reactor), ..day() }));
    }

    #[test]
    fn freezing_drains_colour_towards_blue() {
        let warm = grade(&day());
        let cold = grade(&Mood { body_heat: 5.0, ..day() });
        assert!(cold.temperature < warm.temperature - 0.015);
        assert!(cold.saturation < warm.saturation * 0.8);
    }

    #[test]
    fn dying_bleeds_colour_and_fringes() {
        let g = grade(&Mood { health: 0.05, ..day() });
        assert!(g.saturation < 0.5);
        assert!(g.aberration > 0.01);
        let hit = grade(&Mood { hurt: 1.0, ..day() });
        assert!(hit.aberration > 0.02 && hit.aberration <= 0.05);
    }

    #[test]
    fn lift_is_never_negative() {
        // A negative lift pushes dark pixels below zero, where they clip to black.
        let rooms = [None, Some(Interior::FishHouse(0)), Some(Interior::VaultLobby), Some(Interior::Mart), Some(Interior::Reactor)];
        for interior in rooms {
            for day in [0.0, 0.5, 1.0] {
                for health in [0.0, 0.5, 1.0] {
                    let g = grade(&Mood { interior, day, health, warmth: day, overcast: 1.0 - day, ..day_mood() });
                    for b in [g.shadows, g.midtones, g.highlights] {
                        assert!(b.lift >= 0.0 && b.gain > 0.0 && b.gamma > 0.0, "{interior:?} {b:?}");
                    }
                    // Contrast pivots on 0.5 in linear light, before tonemapping:
                    // any boost in the shadows clips dim rooms to black.
                    assert!(g.shadows.contrast <= 1.0, "{interior:?}");
                    assert!(g.midtones.contrast <= 1.1, "{interior:?}");
                }
            }
        }
    }

    fn day_mood() -> Mood {
        Mood::default()
    }

    #[test]
    fn approach_eases_and_lands() {
        let a = Grade::NEUTRAL;
        let b = grade(&Mood { warmth: 1.0, ..day() });
        let half = a.approach(b, 0.5);
        assert!((half.temperature - b.temperature * 0.5).abs() < 1e-6);
        assert_eq!(a.approach(b, 1.0), b);
        assert_eq!(a.approach(b, 5.0), b);
    }
}
