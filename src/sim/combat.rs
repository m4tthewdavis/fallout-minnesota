//! The pipe rifle: magazine, reloads, cold-weather jams, and hit detection.
//!
//! Design doc: "Below -20F, ballistic weapons can jam."

use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireResult {
    Fired,
    /// The shot went off but the gun jammed afterwards.
    FiredAndJammed,
    Empty,
    Jammed,
    /// Still cycling or reloading.
    Busy,
}

#[derive(Clone, Debug)]
pub struct Weapon {
    pub name: &'static str,
    pub mag: u32,
    pub mag_size: u32,
    pub damage: f32,
    pub range: f32,
    pub fire_interval: f32,
    pub reload_time: f32,
    pub unjam_time: f32,
    pub jammed: bool,
    /// Seconds until the gun can fire again.
    pub cooldown: f32,
    /// Seconds left on a reload / unjam, 0 when idle.
    pub busy: f32,
    /// Rounds moving into the magazine when the reload finishes.
    pending: u32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self::pipe_rifle()
    }
}

impl Weapon {
    pub fn pipe_rifle() -> Self {
        Weapon {
            name: "Pipe Rifle",
            mag: 8,
            mag_size: 8,
            damage: 25.0,
            range: 80.0,
            fire_interval: 0.35,
            reload_time: 1.6,
            unjam_time: 1.0,
            jammed: false,
            cooldown: 0.0,
            busy: 0.0,
            pending: 0,
        }
    }

    pub fn is_reloading(&self) -> bool {
        self.busy > 0.0
    }

    /// Advance timers. Returns true on the frame a reload or unjam completes.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if self.busy > 0.0 {
            self.busy -= dt;
            if self.busy <= 0.0 {
                self.busy = 0.0;
                if self.jammed {
                    self.jammed = false;
                } else {
                    self.mag += self.pending;
                    self.pending = 0;
                }
                return true;
            }
        }
        false
    }

    pub fn try_fire(&mut self, air_temp_f: f32, rng: &mut Rng) -> FireResult {
        if self.busy > 0.0 || self.cooldown > 0.0 {
            return FireResult::Busy;
        }
        if self.jammed {
            return FireResult::Jammed;
        }
        if self.mag == 0 {
            return FireResult::Empty;
        }
        self.mag -= 1;
        self.cooldown = self.fire_interval;
        if rng.chance(jam_chance(air_temp_f)) {
            self.jammed = true;
            FireResult::FiredAndJammed
        } else {
            FireResult::Fired
        }
    }

    /// Press R: clears a jam if jammed, otherwise reloads from `reserve`.
    /// Returns false if there is nothing to do.
    pub fn start_reload(&mut self, reserve: &mut u32) -> bool {
        if self.busy > 0.0 {
            return false;
        }
        if self.jammed {
            self.busy = self.unjam_time;
            return true;
        }
        let need = self.mag_size - self.mag;
        let take = need.min(*reserve);
        if take == 0 {
            return false;
        }
        *reserve -= take;
        self.pending = take;
        self.busy = self.reload_time;
        true
    }
}

/// Chance a shot jams at this air temperature (0 above -20F, up to 18%).
pub fn jam_chance(air_temp_f: f32) -> f32 {
    if air_temp_f >= -20.0 {
        0.0
    } else {
        (0.03 + (-20.0 - air_temp_f) / 60.0 * 0.15).min(0.18)
    }
}

/// Distance along a ray to the first hit on a sphere, if any.
/// `dir` must be normalised.
pub fn ray_sphere(origin: [f32; 3], dir: [f32; 3], center: [f32; 3], radius: f32) -> Option<f32> {
    let oc = [origin[0] - center[0], origin[1] - center[1], origin[2] - center[2]];
    let b = oc[0] * dir[0] + oc[1] * dir[1] + oc[2] * dir[2];
    let c = oc[0] * oc[0] + oc[1] * oc[1] + oc[2] * oc[2] - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t0 = -b - sq;
    let t1 = -b + sq;
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_until_empty_then_reload() {
        let mut w = Weapon::pipe_rifle();
        let mut rng = Rng::new(1);
        let mut reserve = 10;
        for _ in 0..8 {
            assert_eq!(w.try_fire(20.0, &mut rng), FireResult::Fired);
            w.tick(1.0);
        }
        assert_eq!(w.try_fire(20.0, &mut rng), FireResult::Empty);
        assert!(w.start_reload(&mut reserve));
        assert_eq!(w.try_fire(20.0, &mut rng), FireResult::Busy);
        assert!(w.tick(2.0));
        assert_eq!(w.mag, 8);
        assert_eq!(reserve, 2);
    }

    #[test]
    fn no_jams_when_warm_some_when_frigid() {
        assert_eq!(jam_chance(0.0), 0.0);
        assert_eq!(jam_chance(-20.0), 0.0);
        assert!(jam_chance(-35.0) > 0.0);
        assert!(jam_chance(-200.0) <= 0.18);

        let mut w = Weapon::pipe_rifle();
        w.mag_size = 10_000;
        w.mag = 10_000;
        let mut rng = Rng::new(99);
        let mut jams = 0;
        for _ in 0..2000 {
            w.cooldown = 0.0;
            if w.try_fire(-60.0, &mut rng) == FireResult::FiredAndJammed {
                jams += 1;
                w.jammed = false;
            }
        }
        assert!(jams > 50 && jams < 600, "jams = {jams}");
    }

    #[test]
    fn reload_clears_jam_first() {
        let mut w = Weapon::pipe_rifle();
        w.jammed = true;
        let mut reserve = 5;
        assert!(w.start_reload(&mut reserve));
        assert_eq!(reserve, 5, "unjamming uses no ammo");
        w.tick(2.0);
        assert!(!w.jammed);
    }

    #[test]
    fn ray_hits_and_misses() {
        let hit = ray_sphere([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, -10.0], 1.0);
        assert!((hit.unwrap() - 9.0).abs() < 1e-4);
        let miss = ray_sphere([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [5.0, 0.0, -10.0], 1.0);
        assert!(miss.is_none());
        let behind = ray_sphere([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 0.0, 10.0], 1.0);
        assert!(behind.is_none());
    }
}
