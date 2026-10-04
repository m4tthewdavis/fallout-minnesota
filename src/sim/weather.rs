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

/// Direction the prevailing wind blows towards (x, z): snow streams this way,
/// ripples form across it and drifts pile up downwind of things.
pub const WIND_DIR: [f32; 2] = [0.928, 0.371];

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
                fog_density: 0.0075,
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

    #[test]
    fn nothing_changes_until_the_timer_runs_out() {
        let mut w = Weather::new();
        let mut rng = Rng::new(1);
        assert_eq!((w.phase, w.timer), (Phase::Calm, 75.0), "the player gets a head start");
        assert_eq!(w.update(74.0, &mut rng), None);
        assert_eq!(w.phase, Phase::Calm);
        assert!((w.timer - 1.0).abs() < 1e-4);
        assert_eq!(w.update(1.5, &mut rng), Some(Phase::Warning));
    }

    #[test]
    fn the_siren_always_gives_the_same_warning_time() {
        let mut w = Weather { phase: Phase::Calm, timer: 0.5, blizzards: 0 };
        let mut rng = Rng::new(3);
        assert_eq!(w.update(1.0, &mut rng), Some(Phase::Warning));
        assert_eq!(w.timer, Weather::WARNING_SECS);
        assert_eq!(w.blizzards, 0, "a warning is not yet a blizzard");
    }

    #[test]
    fn a_huge_step_moves_only_one_phase() {
        let mut w = Weather { phase: Phase::Calm, timer: 1.0, blizzards: 0 };
        let mut rng = Rng::new(3);
        assert_eq!(w.update(10_000.0, &mut rng), Some(Phase::Warning));
        assert_eq!(w.phase, Phase::Warning, "no skipping the siren");
    }

    #[test]
    fn blizzards_are_counted_when_they_begin_and_last_a_set_time() {
        for seed in 0..20 {
            let mut rng = Rng::new(seed);
            let mut w = Weather { phase: Phase::Warning, timer: 0.1, blizzards: 4 };
            assert_eq!(w.update(0.2, &mut rng), Some(Phase::Blizzard));
            assert_eq!(w.blizzards, 5);
            assert!((40.0..60.0).contains(&w.timer), "blizzard lasts {}", w.timer);
            w.timer = 0.0;
            assert_eq!(w.update(0.1, &mut rng), Some(Phase::Calm));
            assert_eq!(w.blizzards, 5, "ending one doesn't count another");
            assert!((70.0..120.0).contains(&w.timer), "calm lasts {}", w.timer);
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_weather() {
        let run = |seed| {
            let (mut w, mut rng) = (Weather::new(), Rng::new(seed));
            (0..20_000).filter_map(|_| w.update(0.1, &mut rng).map(|p| (p, (w.timer * 10.0) as i32))).collect::<Vec<_>>()
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
    }

    #[test]
    fn conditions_worsen_with_each_phase() {
        let c = [Phase::Calm, Phase::Warning, Phase::Blizzard].map(Weather::conditions_for);
        for pair in c.windows(2) {
            assert!(pair[1].air_temp_f < pair[0].air_temp_f, "colder");
            assert!(pair[1].wind_chill_f > pair[0].wind_chill_f, "more wind chill");
            assert!(pair[1].fog_density > pair[0].fog_density, "foggier");
            assert!(pair[1].wind > pair[0].wind, "windier");
            assert!(pair[1].snow > pair[0].snow, "snowier");
            assert!(pair[1].light < pair[0].light, "darker");
        }
        assert_eq!((c[0].rads_per_sec, c[1].rads_per_sec), (0.0, 0.0), "radiation only in the blizzard");
        assert!(c[2].rads_per_sec > 0.0);
    }

    #[test]
    fn wolves_hunt_only_in_blizzards_and_conditions_follow_the_phase() {
        for phase in [Phase::Calm, Phase::Warning, Phase::Blizzard] {
            let w = Weather { phase, timer: 10.0, blizzards: 0 };
            assert_eq!(w.wolves_hunting(), phase == Phase::Blizzard);
            assert_eq!(w.conditions(), Weather::conditions_for(phase));
        }
    }
}
