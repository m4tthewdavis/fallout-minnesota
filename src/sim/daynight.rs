//! Day/night cycle. A full day lasts 12 real minutes. Nights are darker,
//! colder, and the Frostfangs howl more.

use super::mathx::smoothstep;

/// Real seconds per in-game 24 hours.
pub const DAY_LENGTH_SECS: f32 = 720.0;
pub const START_HOUR: f32 = 7.5;
/// How much colder the deepest night is than midday, in degrees F.
pub const NIGHT_CHILL_F: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// Unit vector pointing from the ground towards the sun.
    pub sun_pos: [f32; 3],
    /// 0..=1 strength of direct sunlight.
    pub sun: f32,
    /// 0..=1 strength of moonlight.
    pub moon: f32,
    /// 0..=1 overall brightness of the sky (never fully black).
    pub daylight: f32,
    /// 0..=1 how orange the light is (sunrise / sunset).
    pub warmth: f32,
}

#[derive(Clone, Debug)]
pub struct Clock {
    /// 0.0..24.0
    pub hours: f32,
    pub day: u32,
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}


impl Clock {
    pub fn new() -> Self {
        Clock {
            hours: START_HOUR,
            day: 1,
        }
    }

    /// Advance by `dt` real seconds. Returns true when a new day begins.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.hours += dt * 24.0 / DAY_LENGTH_SECS;
        if self.hours >= 24.0 {
            self.hours -= 24.0;
            self.day += 1;
            return true;
        }
        false
    }

    /// Sun height: +1 at noon, -1 at midnight, 0 at 6:00 and 18:00.
    pub fn sun_elevation(&self) -> f32 {
        let angle = (self.hours - 6.0) / 12.0 * std::f32::consts::PI;
        angle.sin()
    }

    pub fn sky(&self) -> Sky {
        let angle = (self.hours - 6.0) / 12.0 * std::f32::consts::PI;
        let elev = angle.sin();
        // The sun rises in the east (+x) and arcs over the southern sky (+z).
        let raw = [angle.cos(), elev.max(-0.2), 0.35];
        let len = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
        let sun_pos = [raw[0] / len, raw[1] / len, raw[2] / len];

        let sun = smoothstep(-0.05, 0.25, elev);
        let moon = 1.0 - smoothstep(-0.15, 0.05, elev);
        let daylight = 0.12 + 0.88 * smoothstep(-0.2, 0.3, elev);
        let warmth = if elev > -0.1 {
            1.0 - smoothstep(0.0, 0.35, elev)
        } else {
            0.0
        };
        Sky {
            sun_pos,
            sun,
            moon,
            daylight,
            warmth,
        }
    }

    pub fn is_night(&self) -> bool {
        self.sun_elevation() < -0.05
    }

    /// Degrees F to add to the weather's air temperature.
    pub fn temp_offset_f(&self) -> f32 {
        -NIGHT_CHILL_F * (1.0 - smoothstep(-0.3, 0.6, self.sun_elevation()))
    }

    /// "07:30"
    pub fn label(&self) -> String {
        let h = self.hours.floor() as u32 % 24;
        let m = ((self.hours.fract()) * 60.0).floor() as u32;
        format!("{h:02}:{m:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: f32) -> Clock {
        Clock { hours, day: 1 }
    }

    #[test]
    fn noon_is_bright_and_midnight_is_dark() {
        let noon = at(12.0).sky();
        let midnight = at(0.0).sky();
        assert!(noon.sun > 0.99 && noon.daylight > 0.99);
        assert!(midnight.sun < 0.01 && midnight.moon > 0.99);
        assert!(midnight.daylight >= 0.12 && midnight.daylight < 0.2);
    }

    #[test]
    fn sunsets_are_warm() {
        assert!(at(18.2).sky().warmth > 0.5);
        assert!(at(12.0).sky().warmth < 0.01);
    }

    #[test]
    fn nights_are_colder() {
        assert!(at(0.0).temp_offset_f() < -11.0);
        assert!(at(12.0).temp_offset_f().abs() < 0.5);
        assert!(at(0.0).is_night() && !at(12.0).is_night());
    }

    #[test]
    fn clock_wraps_into_a_new_day() {
        let mut c = at(23.9);
        let new_day = c.advance(DAY_LENGTH_SECS / 24.0 * 0.2);
        assert!(new_day);
        assert_eq!(c.day, 2);
        assert!(c.hours < 0.2);
    }

    #[test]
    fn labels() {
        assert_eq!(at(7.5).label(), "07:30");
        assert_eq!(at(23.99).label(), "23:59");
    }

    #[test]
    fn sun_direction_is_unit() {
        for h in 0..24 {
            let s = at(h as f32).sky().sun_pos;
            let len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4);
        }
    }
}
