//! Rad-crows: glowing black crows that wheel over their roost until something
//! loud goes off. A gunshot scatters the flock; if it was close to them, they
//! regroup over the shooter and come down one at a time to peck, climb away
//! and come back until they lose interest. Pure rules (no Bevy), driven by
//! `crows.rs`.

use super::rng::Rng;

/// Orbit radius and altitude band while circling.
pub const CIRCLE_RADIUS: f32 = 15.0;
pub const CIRCLE_ALT: (f32, f32) = (11.0, 16.0);
pub const CIRCLE_SPEED: f32 = 6.5;
pub const SCATTER_SPEED: f32 = 13.0;
pub const DIVE_SPEED: f32 = 15.0;
pub const CLIMB_SPEED: f32 = 8.5;
/// A crow this close to your head gets a peck in.
pub const PECK_RANGE: f32 = 1.5;
pub const PECK_DAMAGE: f32 = 5.0;
/// Gunshots within this many metres of a crow startle it.
pub const HEAR_RANGE: f32 = 90.0;
/// Shots within this range of a crow make it angry (it hunts the shooter).
pub const ANGER_RANGE: f32 = 65.0;
/// How long a shot keeps the flock diving at you.
pub const ANGER_SECS: f32 = 28.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    /// Wheeling over the roost.
    Circle,
    /// Fleeing a shot, up and away.
    Scatter,
    /// Over the player, waiting its turn.
    Hover,
    Dive,
    /// Pulling out of a dive.
    Climb,
}

#[derive(Clone, Debug)]
pub struct Crow {
    pub mode: Mode,
    timer: f32,
    anger: f32,
    /// Centre of the roost it circles.
    pub home: [f32; 2],
    alt: f32,
    /// +1 or -1: which way it wheels.
    spin: f32,
    from: [f32; 2],
    dive_secs: f32,
}

/// What a crow does this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Velocity, m/s.
    pub vel: [f32; 3],
    /// A peck lands this frame.
    pub peck: bool,
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-6 {
        [0.0; 3]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

fn scale3(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dist_xz(a: [f32; 3], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[1])
}

impl Crow {
    pub fn new(home: [f32; 2], rng: &mut Rng) -> Crow {
        Crow {
            mode: Mode::Circle,
            timer: 0.0,
            anger: 0.0,
            home,
            alt: rng.range(CIRCLE_ALT.0, CIRCLE_ALT.1),
            spin: if rng.chance(0.5) { 1.0 } else { -1.0 },
            from: home,
            dive_secs: 0.0,
        }
    }

    /// Seconds of anger left (it hunts you while this is above zero).
    pub fn anger(&self) -> f32 {
        self.anger
    }

    /// A gunshot went off at `shot` while this crow was at `pos`.
    pub fn hear_shot(&mut self, pos: [f32; 3], shot: [f32; 3], rng: &mut Rng) {
        let d = dist_xz(pos, [shot[0], shot[2]]);
        if d > HEAR_RANGE {
            return;
        }
        if d < ANGER_RANGE {
            self.anger = ANGER_SECS;
        }
        if matches!(self.mode, Mode::Circle | Mode::Hover) {
            self.mode = Mode::Scatter;
            self.timer = rng.range(2.0, 3.5);
            self.from = [shot[0], shot[2]];
        }
    }

    /// Fly for `dt` seconds. `player` is where the player's head is.
    pub fn update(&mut self, dt: f32, pos: [f32; 3], player: [f32; 3], rng: &mut Rng) -> Step {
        self.anger = (self.anger - dt).max(0.0);
        self.timer -= dt;
        let mut peck = false;
        let vel = match self.mode {
            Mode::Circle => {
                // Chase a point a little ahead of me on the circle.
                let (dx, dz) = (pos[0] - self.home[0], pos[2] - self.home[1]);
                let angle = dz.atan2(dx) + 0.6 * self.spin;
                let target = [self.home[0] + angle.cos() * CIRCLE_RADIUS, self.alt, self.home[1] + angle.sin() * CIRCLE_RADIUS];
                scale3(norm3([target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]]), CIRCLE_SPEED)
            }
            Mode::Scatter => {
                let away = [pos[0] - self.from[0], 0.0, pos[2] - self.from[1]];
                let dir = norm3([away[0], 0.0, away[2]]);
                let dir = if dir == [0.0; 3] { [1.0, 0.0, 0.0] } else { dir };
                if self.timer <= 0.0 {
                    if self.anger > 0.0 {
                        self.mode = Mode::Hover;
                        self.timer = rng.range(0.6, 5.0);
                    } else {
                        // Settle into a new roost where the flock ended up.
                        self.mode = Mode::Circle;
                        self.home = [pos[0], pos[2]];
                    }
                }
                scale3(norm3([dir[0], 0.45, dir[2]]), SCATTER_SPEED)
            }
            Mode::Hover => {
                if self.anger <= 0.0 {
                    self.mode = Mode::Circle;
                    self.home = [pos[0], pos[2]];
                } else if self.timer <= 0.0 {
                    self.mode = Mode::Dive;
                    self.dive_secs = 0.0;
                }
                // Wheel round the player's position.
                let (dx, dz) = (pos[0] - player[0], pos[2] - player[2]);
                let angle = dz.atan2(dx) + 0.7 * self.spin;
                let target = [player[0] + angle.cos() * 9.0, player[1] + 7.0, player[2] + angle.sin() * 9.0];
                scale3(norm3([target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]]), CIRCLE_SPEED * 1.2)
            }
            Mode::Dive => {
                self.dive_secs += dt;
                let to = [player[0] - pos[0], player[1] - pos[1], player[2] - pos[2]];
                let dist = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
                if dist < PECK_RANGE {
                    peck = true;
                    self.mode = Mode::Climb;
                    self.timer = rng.range(1.6, 2.6);
                } else if self.dive_secs > 4.5 {
                    // Missed and overshot: pull up and go round again.
                    self.mode = Mode::Climb;
                    self.timer = 1.5;
                }
                scale3(norm3(to), DIVE_SPEED)
            }
            Mode::Climb => {
                if self.timer <= 0.0 {
                    if self.anger > 0.0 {
                        self.mode = Mode::Hover;
                        self.timer = rng.range(0.4, 2.5);
                    } else {
                        self.mode = Mode::Circle;
                        self.home = [pos[0], pos[2]];
                    }
                }
                let away = norm3([pos[0] - player[0], 0.0, pos[2] - player[2]]);
                let away = if away == [0.0; 3] { [1.0, 0.0, 0.0] } else { away };
                scale3(norm3([away[0] * 0.6, 1.0, away[2] * 0.6]), CLIMB_SPEED)
            }
        };
        Step { vel, peck }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: [f32; 3] = [0.0, 1.7, 0.0];

    fn fly(c: &mut Crow, pos: &mut [f32; 3], player: [f32; 3], secs: f32, rng: &mut Rng) -> u32 {
        let mut pecks = 0;
        for _ in 0..(secs * 30.0) as usize {
            let s = c.update(1.0 / 30.0, *pos, player, rng);
            for (p, v) in pos.iter_mut().zip(s.vel) {
                *p += v / 30.0;
            }
            // Never through the ground.
            pos[1] = pos[1].max(0.3);
            if s.peck {
                pecks += 1;
            }
        }
        pecks
    }

    #[test]
    fn a_calm_flock_wheels_over_its_roost_and_stays_up() {
        let mut rng = Rng::new(1);
        let mut c = Crow::new([100.0, 100.0], &mut rng);
        let mut pos = [100.0 + CIRCLE_RADIUS, 13.0, 100.0];
        let mut lowest = f32::MAX;
        for _ in 0..40 {
            fly(&mut c, &mut pos, PLAYER, 1.0, &mut rng);
            let r = (pos[0] - 100.0).hypot(pos[2] - 100.0);
            assert!((CIRCLE_RADIUS - 6.0..CIRCLE_RADIUS + 6.0).contains(&r), "radius {r}");
            lowest = lowest.min(pos[1]);
        }
        assert!(lowest > 8.0, "it flew at {lowest} m");
        assert_eq!(c.mode, Mode::Circle);
    }

    #[test]
    fn a_far_shot_goes_unheard() {
        let mut rng = Rng::new(2);
        let mut c = Crow::new([0.0, 0.0], &mut rng);
        c.hear_shot([0.0, 12.0, 0.0], [HEAR_RANGE + 5.0, 1.0, 0.0], &mut rng);
        assert_eq!(c.mode, Mode::Circle);
        assert_eq!(c.anger(), 0.0);
    }

    #[test]
    fn a_distant_shot_scatters_the_flock_but_does_not_anger_it() {
        let mut rng = Rng::new(3);
        let mut c = Crow::new([0.0, 0.0], &mut rng);
        let mut pos = [CIRCLE_RADIUS, 13.0, 0.0];
        // Heard (within HEAR_RANGE of the crow) but beyond ANGER_RANGE.
        let shot_x = CIRCLE_RADIUS + ANGER_RANGE + 12.0;
        c.hear_shot(pos, [shot_x, 1.0, 0.0], &mut rng);
        assert_eq!(c.mode, Mode::Scatter);
        assert_eq!(c.anger(), 0.0);
        let start = (pos[0] - shot_x).abs();
        fly(&mut c, &mut pos, [shot_x, 1.7, 0.0], 2.0, &mut rng);
        assert!((pos[0] - shot_x).abs() > start, "it flew away from the shot");
        fly(&mut c, &mut pos, [shot_x, 1.7, 0.0], 3.0, &mut rng);
        assert_eq!(c.mode, Mode::Circle, "and settled to wheel again");
        assert!(c.home[0] != 0.0, "over a new roost");
    }

    #[test]
    fn a_close_shot_sends_the_crows_at_the_shooter() {
        let mut rng = Rng::new(4);
        let mut c = Crow::new([30.0, 0.0], &mut rng);
        let mut pos = [30.0 + CIRCLE_RADIUS, 13.0, 0.0];
        c.hear_shot(pos, [0.0, 1.5, 0.0], &mut rng);
        assert_eq!(c.mode, Mode::Scatter);
        assert!(c.anger() > 0.0);
        let pecks = fly(&mut c, &mut pos, PLAYER, 20.0, &mut rng);
        assert!(pecks >= 1, "it came down and pecked ({pecks})");
    }

    #[test]
    fn a_dive_closes_on_the_player_and_pecks_only_in_range() {
        let mut rng = Rng::new(5);
        let mut c = Crow::new([0.0, 0.0], &mut rng);
        c.mode = Mode::Dive;
        c.anger = 10.0;
        let mut pos = [12.0, 9.0, 0.0];
        let mut last = f32::MAX;
        for _ in 0..200 {
            let s = c.update(1.0 / 30.0, pos, PLAYER, &mut rng);
            for (p, v) in pos.iter_mut().zip(s.vel) {
                *p += v / 30.0;
            }
            let d = ((pos[0] - PLAYER[0]).powi(2) + (pos[1] - PLAYER[1]).powi(2) + (pos[2] - PLAYER[2]).powi(2)).sqrt();
            if s.peck {
                assert!(d < PECK_RANGE + 0.7, "pecked from {d} m");
                return;
            }
            assert!(d <= last + 0.01, "it dives straight in");
            last = d;
        }
        panic!("never reached the player");
    }

    #[test]
    fn crows_come_one_at_a_time_not_all_at_once() {
        // Anger several crows together: their dive times differ.
        let mut rng = Rng::new(6);
        let mut dive_at = Vec::new();
        for i in 0..6 {
            let mut c = Crow::new([0.0, 0.0], &mut rng);
            let mut pos = [CIRCLE_RADIUS, 13.0, i as f32];
            c.hear_shot(pos, [3.0, 1.5, 0.0], &mut rng);
            let mut t = 0.0;
            while c.mode != Mode::Dive && t < 30.0 {
                let s = c.update(1.0 / 30.0, pos, PLAYER, &mut rng);
                for (p, v) in pos.iter_mut().zip(s.vel) {
                    *p += v / 30.0;
                }
                t += 1.0 / 30.0;
            }
            dive_at.push(t);
        }
        let (min, max) = dive_at.iter().fold((f32::MAX, 0.0f32), |(a, b), t| (a.min(*t), b.max(*t)));
        assert!(max - min > 1.0, "dives were staggered: {dive_at:?}");
    }

    #[test]
    fn they_lose_interest_when_the_anger_runs_out() {
        let mut rng = Rng::new(7);
        let mut c = Crow::new([0.0, 0.0], &mut rng);
        let mut pos = [CIRCLE_RADIUS, 13.0, 0.0];
        c.hear_shot(pos, [2.0, 1.5, 0.0], &mut rng);
        // Keep the player far away so there's nothing to peck, and run out the clock.
        fly(&mut c, &mut pos, [400.0, 1.7, 400.0], ANGER_SECS + 25.0, &mut rng);
        assert_eq!(c.anger(), 0.0);
        assert_eq!(c.mode, Mode::Circle);
    }

    #[test]
    fn a_new_shot_while_diving_renews_the_anger_without_cancelling_the_dive() {
        let mut rng = Rng::new(8);
        let mut c = Crow::new([0.0, 0.0], &mut rng);
        c.mode = Mode::Dive;
        c.anger = 3.0;
        c.hear_shot([5.0, 5.0, 0.0], [0.0, 1.5, 0.0], &mut rng);
        assert_eq!(c.mode, Mode::Dive);
        assert_eq!(c.anger(), ANGER_SECS);
    }
}
