//! The Long Winter's weather cycle: calm -> siren warning -> rad-blizzard -> calm.
//!
//! Design doc: "Rad-blizzards (the 'Alberta Clippers') roll in from the
//! northwest with a siren warning. They deal steady radiation and cold damage
//! and cut visibility."

use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Calm,
    Warning,
    Blizzard,
}

/// What the weather is doing right now, in gameplay terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    pub air_temp_f: f32,
    pub wind_chill_f: f32,
    pub rads_per_sec: f32,
    /// Exponential fog density for the renderer.
    pub fog_density: f32,
    /// 0..=1 share of snow particles that are active.
    pub snow: f32,
    /// Sideways wind speed applied to falling snow, m/s.
    pub wind: f32,
    /// 0..=1 multiplier for sunlight.
    pub light: f32,
}

#[derive(Clone, Debug)]
pub struct Weather {
    pub phase: Phase,
    /// Seconds left in the current phase.
    pub timer: f32,
    /// How many blizzards have hit so far.
    pub blizzards: u32,
}

impl Default for Weather {
    fn default() -> Self {
        Self::new()
    }
}

impl Weather {
    pub const WARNING_SECS: f32 = 20.0;

    pub fn new() -> Self {
        // Give the player a little time to leave the vault first.
        Weather {
            phase: Phase::Calm,
            timer: 75.0,
            blizzards: 0,
        }
    }

    /// Advance the cycle. Returns the new phase when it changes.
    pub fn update(&mut self, dt: f32, rng: &mut Rng) -> Option<Phase> {
        self.timer -= dt;
        if self.timer > 0.0 {
            return None;
        }
        let (next, secs) = match self.phase {
            Phase::Calm => (Phase::Warning, Self::WARNING_SECS),
            Phase::Warning => {
                self.blizzards += 1;
                (Phase::Blizzard, rng.range(40.0, 60.0))
            }
            Phase::Blizzard => (Phase::Calm, rng.range(70.0, 120.0)),
        };
        self.phase = next;
        self.timer = secs;
        Some(next)
    }

    pub fn conditions(&self) -> Conditions {
        Self::conditions_for(self.phase)
    }

    pub fn conditions_for(phase: Phase) -> Conditions {
        match phase {
            Phase::Calm => Conditions {
                air_temp_f: -8.0,
                wind_chill_f: 10.0,
                rads_per_sec: 0.0,
                fog_density: 0.012,
                snow: 0.25,
                wind: 1.0,
                light: 1.0,
            },
            Phase::Warning => Conditions {
                air_temp_f: -15.0,
                wind_chill_f: 25.0,
                rads_per_sec: 0.0,
                fog_density: 0.025,
                snow: 0.5,
                wind: 4.0,
                light: 0.7,
            },
            Phase::Blizzard => Conditions {
                air_temp_f: -35.0,
                wind_chill_f: 40.0,
                rads_per_sec: 6.0,
                fog_density: 0.08,
                snow: 1.0,
                wind: 14.0,
                light: 0.35,
            },
        }
    }

    /// Frostfang packs only hunt in earnest during rad-blizzards.
    pub fn wolves_hunting(&self) -> bool {
        self.phase == Phase::Blizzard
    }

    pub fn label(&self) -> &'static str {
        match self.phase {
            Phase::Calm => "Calm",
            Phase::Warning => "SIREN: blizzard inbound",
            Phase::Blizzard => "RAD-BLIZZARD",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_all_phases() {
        let mut w = Weather::new();
        let mut rng = Rng::new(7);
        let mut seen = vec![];
        for _ in 0..10_000 {
            if let Some(p) = w.update(0.1, &mut rng) {
                seen.push(p);
            }
        }
        assert!(seen.len() >= 6);
        assert_eq!(&seen[..3], &[Phase::Warning, Phase::Blizzard, Phase::Calm]);
        assert!(w.blizzards >= 2);
    }

    #[test]
    fn blizzard_is_harshest() {
        let calm = Weather::conditions_for(Phase::Calm);
        let storm = Weather::conditions_for(Phase::Blizzard);
        assert!(storm.air_temp_f < calm.air_temp_f);
        assert!(storm.rads_per_sec > calm.rads_per_sec);
        assert!(storm.fog_density > calm.fog_density);
    }
}
