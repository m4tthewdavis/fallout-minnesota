//! The Glowmoose's behaviour: a huge, mostly placid animal that grazes until
//! you come close, then stares, paws the snow, bellows, and charges in a
//! straight line at where you *were*. Sidestep it. A charge that hits a tree
//! or a wall leaves it stunned; a charge that runs its course leaves it winded.

use super::rng::Rng;

pub const GRAZE_SPEED: f32 = 1.4;
pub const CHARGE_SPEED: f32 = 11.0;
/// Player's sprint speed is 8.5, so you can't outrun a charge, only dodge it.
pub const ALERT_RANGE: f32 = 30.0;
pub const CHARGE_RANGE: f32 = 17.0;
pub const WINDUP_SECS: f32 = 1.0;
pub const CHARGE_SECS: f32 = 2.4;
pub const WINDED_SECS: f32 = 1.6;
pub const STUNNED_SECS: f32 = 3.2;
/// How close a charging moose must be to hit you.
pub const HIT_RANGE: f32 = 2.0;
pub const HIT_DAMAGE: f32 = 35.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Graze,
    /// Staring at you, deciding.
    Alert,
    /// Pawing the ground and bellowing before the charge.
    Windup,
    Charge,
    /// Winded after a charge, or stunned after a crash.
    Recover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The windup began: bellow.
    Bellow,
    /// The charge began.
    Charged,
    /// Hit something solid at speed.
    Crashed,
    /// The charge ran out of steam.
    ChargeEnded,
    /// An idle grunt.
    Grunt,
}

#[derive(Clone, Debug)]
pub struct Moose {
    pub mode: Mode,
    pub timer: f32,
    /// Heading while charging (fixed when the windup ends).
    pub charge_dir: [f32; 2],
    /// Wander heading while grazing.
    wander_dir: [f32; 2],
    wander_timer: f32,
    pub stunned: bool,
    /// Has this charge already struck the player?
    pub hit_player: bool,
    grunt_timer: f32,
}

/// What the moose wants to do this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// Metres per second along `dir`.
    pub speed: f32,
    pub dir: [f32; 2],
    /// Direction it faces (it stares at you while winding up).
    pub face: [f32; 2],
    pub events: Vec<Event>,
}

fn norm(v: [f32; 2]) -> [f32; 2] {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if l < 1e-6 {
        [0.0, 0.0]
    } else {
        [v[0] / l, v[1] / l]
    }
}

impl Moose {
    pub fn new(rng: &mut Rng) -> Self {
        let a = rng.range(0.0, std::f32::consts::TAU);
        Moose {
            mode: Mode::Graze,
            timer: 0.0,
            charge_dir: [0.0, 1.0],
            wander_dir: [a.cos(), a.sin()],
            wander_timer: rng.range(2.0, 6.0),
            stunned: false,
            hit_player: false,
            grunt_timer: rng.range(6.0, 20.0),
        }
    }

    /// Advance one frame. `dist` and `to_player` (a flat vector, not
    /// necessarily unit length) describe where you are; `hurt` means it was
    /// just shot or struck; `crashed` means its last charge step hit a solid.
    pub fn update(&mut self, dt: f32, dist: f32, to_player: [f32; 2], hurt: bool, crashed: bool, rng: &mut Rng) -> Step {
        let toward = norm(to_player);
        let mut events = Vec::new();
        self.timer += dt;

        // Being hurt makes it furious at once (unless it is already charging or reeling).
        if hurt && matches!(self.mode, Mode::Graze | Mode::Alert) {
            self.start_windup(&mut events);
        }

        let (speed, dir, face) = match self.mode {
            Mode::Graze => {
                self.wander_timer -= dt;
                if self.wander_timer <= 0.0 {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    self.wander_dir = [a.cos(), a.sin()];
                    self.wander_timer = rng.range(4.0, 9.0);
                }
                self.grunt_timer -= dt;
                if self.grunt_timer <= 0.0 {
                    self.grunt_timer = rng.range(12.0, 30.0);
                    events.push(Event::Grunt);
                }
                if dist < ALERT_RANGE {
                    self.mode = Mode::Alert;
                    self.timer = 0.0;
                }
                (GRAZE_SPEED * 0.6, self.wander_dir, self.wander_dir)
            }
            Mode::Alert => {
                if dist > ALERT_RANGE * 1.4 {
                    self.mode = Mode::Graze;
                    self.timer = 0.0;
                } else if dist < CHARGE_RANGE && self.timer > 1.0 {
                    self.start_windup(&mut events);
                }
                (0.0, toward, toward)
            }
            Mode::Windup => {
                if self.timer >= WINDUP_SECS {
                    // Lock the heading: it charges where you were, not where you go.
                    self.charge_dir = toward;
                    self.mode = Mode::Charge;
                    self.timer = 0.0;
                    self.hit_player = false;
                    events.push(Event::Charged);
                }
                (0.0, toward, toward)
            }
            Mode::Charge => {
                if crashed {
                    self.mode = Mode::Recover;
                    self.stunned = true;
                    self.timer = 0.0;
                    events.push(Event::Crashed);
                    (0.0, self.charge_dir, self.charge_dir)
                } else if self.timer >= CHARGE_SECS {
                    self.mode = Mode::Recover;
                    self.stunned = false;
                    self.timer = 0.0;
                    events.push(Event::ChargeEnded);
                    (0.0, self.charge_dir, self.charge_dir)
                } else {
                    (CHARGE_SPEED, self.charge_dir, self.charge_dir)
                }
            }
            Mode::Recover => {
                let need = if self.stunned { STUNNED_SECS } else { WINDED_SECS };
                if self.timer >= need {
                    self.stunned = false;
                    self.mode = if dist < ALERT_RANGE { Mode::Alert } else { Mode::Graze };
                    // After a charge it is angry: straight back to the stare.
                    self.timer = if self.mode == Mode::Alert { 0.6 } else { 0.0 };
                }
                (0.0, toward, toward)
            }
        };
        Step { speed, dir, face, events }
    }

    fn start_windup(&mut self, events: &mut Vec<Event>) {
        self.mode = Mode::Windup;
        self.timer = 0.0;
        events.push(Event::Bellow);
    }

    /// True if a charge strike at this distance hits (once per charge).
    pub fn strikes(&mut self, dist: f32) -> bool {
        if self.mode == Mode::Charge && !self.hit_player && dist < HIT_RANGE {
            self.hit_player = true;
            true
        } else {
            false
        }
    }

    /// 0..1 how far into its windup the moose is (for the pawing animation).
    pub fn windup_progress(&self) -> f32 {
        if self.mode == Mode::Windup {
            (self.timer / WINDUP_SECS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moose() -> (Moose, Rng) {
        let mut rng = Rng::new(5);
        (Moose::new(&mut rng), rng)
    }

    fn run(m: &mut Moose, rng: &mut Rng, secs: f32, dist: f32, to: [f32; 2]) -> Vec<Event> {
        let mut events = Vec::new();
        let mut t = 0.0;
        while t < secs {
            events.extend(m.update(0.05, dist, to, false, false, rng).events);
            t += 0.05;
        }
        events
    }

    #[test]
    fn it_grazes_quietly_until_you_come_close() {
        let (mut m, mut rng) = moose();
        let step = m.update(0.05, 80.0, [80.0, 0.0], false, false, &mut rng);
        assert_eq!(m.mode, Mode::Graze);
        assert!(step.speed > 0.0 && step.speed < 1.5, "a slow amble");
        run(&mut m, &mut rng, 5.0, 25.0, [25.0, 0.0]);
        assert_eq!(m.mode, Mode::Alert, "stares when you get within {ALERT_RANGE}");
        run(&mut m, &mut rng, 5.0, 60.0, [60.0, 0.0]);
        assert_eq!(m.mode, Mode::Graze, "loses interest when you go away");
    }

    #[test]
    fn the_stare_gives_you_a_chance_to_leave() {
        let (mut m, mut rng) = moose();
        run(&mut m, &mut rng, 0.5, 25.0, [25.0, 0.0]);
        assert_eq!(m.mode, Mode::Alert);
        // Standing 25 m away it never charges (it's outside CHARGE_RANGE).
        let events = run(&mut m, &mut rng, 10.0, 25.0, [25.0, 0.0]);
        assert!(!events.contains(&Event::Bellow));
        assert_eq!(m.mode, Mode::Alert);
    }

    #[test]
    fn getting_close_makes_it_wind_up_bellow_and_charge() {
        let (mut m, mut rng) = moose();
        run(&mut m, &mut rng, 0.2, 14.0, [14.0, 0.0]);
        let mut all = run(&mut m, &mut rng, 1.5, 14.0, [14.0, 0.0]);
        assert!(all.contains(&Event::Bellow), "bellows at the start of the windup");
        all.extend(run(&mut m, &mut rng, 0.8, 14.0, [14.0, 0.0]));
        assert!(all.contains(&Event::Charged));
        assert_eq!(m.mode, Mode::Charge);
    }

    #[test]
    fn it_charges_where_you_were_so_sidestepping_works() {
        let (mut m, mut rng) = moose();
        // Winds up with you due east...
        let mut t = 0.0;
        m.update(0.05, 14.0, [14.0, 0.0], false, false, &mut rng);
        m.mode = Mode::Windup;
        m.timer = 0.0;
        while m.mode == Mode::Windup && t < 3.0 {
            m.update(0.05, 14.0, [14.0, 0.0], false, false, &mut rng);
            t += 0.05;
        }
        assert_eq!(m.mode, Mode::Charge);
        // ...then you leap north. It keeps running east.
        let step = m.update(0.05, 14.0, [0.0, 14.0], false, false, &mut rng);
        assert_eq!(step.speed, CHARGE_SPEED);
        assert!((step.dir[0] - 1.0).abs() < 1e-4 && step.dir[1].abs() < 1e-4, "{:?}", step.dir);
        assert!(CHARGE_SPEED > 8.5, "you cannot outrun it");
    }

    #[test]
    fn a_charge_runs_out_and_leaves_it_winded() {
        let (mut m, mut rng) = moose();
        m.mode = Mode::Charge;
        m.timer = 0.0;
        let mut events = Vec::new();
        let mut travelled = 0.0;
        for _ in 0..100 {
            let s = m.update(0.05, 25.0, [25.0, 0.0], false, false, &mut rng);
            travelled += s.speed * 0.05;
            events.extend(s.events);
        }
        assert!(events.contains(&Event::ChargeEnded) && !events.contains(&Event::Crashed));
        assert!((travelled - CHARGE_SPEED * CHARGE_SECS).abs() < 1.0, "ran {travelled} m");
        assert_eq!(m.mode, Mode::Alert, "winded, then wary again (you are 25 m away)");
    }

    #[test]
    fn crashing_into_a_tree_stuns_it_for_longer() {
        let (mut m, mut rng) = moose();
        m.mode = Mode::Charge;
        let s = m.update(0.05, 10.0, [10.0, 0.0], false, true, &mut rng);
        assert!(s.events.contains(&Event::Crashed));
        assert_eq!((m.mode, m.stunned), (Mode::Recover, true));
        // Stunned: helpless for STUNNED_SECS, longer than being winded.
        assert!(STUNNED_SECS > WINDED_SECS * 1.5);
        let mut t = 0.0;
        while m.mode == Mode::Recover && t < 10.0 {
            let s = m.update(0.05, 10.0, [10.0, 0.0], false, false, &mut rng);
            assert_eq!(s.speed, 0.0);
            t += 0.05;
        }
        assert!((t - STUNNED_SECS).abs() < 0.2, "stunned for {t}");
    }

    #[test]
    fn being_shot_makes_it_furious_at_once() {
        let (mut m, mut rng) = moose();
        let s = m.update(0.05, 40.0, [40.0, 0.0], true, false, &mut rng);
        assert_eq!(m.mode, Mode::Windup);
        assert!(s.events.contains(&Event::Bellow));
        // But a charging moose isn't interrupted by more hits.
        m.mode = Mode::Charge;
        m.update(0.05, 40.0, [40.0, 0.0], true, false, &mut rng);
        assert_eq!(m.mode, Mode::Charge);
    }

    #[test]
    fn a_charge_strikes_once() {
        let (mut m, _) = moose();
        assert!(!m.strikes(1.0), "only while charging");
        m.mode = Mode::Charge;
        assert!(!m.strikes(5.0), "out of reach");
        assert!(m.strikes(1.5));
        assert!(!m.strikes(1.0), "only once per charge");
    }

    #[test]
    fn windup_progress_drives_the_pawing() {
        let (mut m, mut rng) = moose();
        assert_eq!(m.windup_progress(), 0.0);
        m.mode = Mode::Windup;
        m.timer = 0.0;
        m.update(0.5, 14.0, [14.0, 0.0], false, false, &mut rng);
        assert!((m.windup_progress() - 0.5).abs() < 0.01);
    }
}
