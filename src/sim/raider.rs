//! Frozen Raiders: people who've been out in the Long Winter too long. They
//! hold a camp, and fight like people with the same trouble you have: they
//! can only shoot what they can see (trees and walls block the line, a
//! blizzard or the dark shortens it), their guns jam in the cold, they miss
//! more at range and when you're running, and when badly hurt they fall back
//! to the fire. Pure rules (no Bevy), driven by `raiders.rs`.

use super::combat::jam_chance;
use super::rng::Rng;

/// What a raider carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gun {
    Rifle,
    Shotgun,
    Revolver,
}

impl Gun {
    pub fn name(self) -> &'static str {
        match self {
            Gun::Rifle => "hunting rifle",
            Gun::Shotgun => "scrap shotgun",
            Gun::Revolver => "revolver",
        }
    }
    /// Damage of one pellet or bullet that connects at point-blank range.
    pub fn damage(self) -> f32 {
        match self {
            Gun::Rifle => 13.0,
            Gun::Shotgun => 5.0,
            Gun::Revolver => 8.0,
        }
    }
    pub fn pellets(self) -> u32 {
        if self == Gun::Shotgun {
            6
        } else {
            1
        }
    }
    /// Beyond this it won't shoot.
    pub fn range(self) -> f32 {
        match self {
            Gun::Rifle => 75.0,
            Gun::Shotgun => 22.0,
            Gun::Revolver => 42.0,
        }
    }
    pub fn interval(self) -> f32 {
        match self {
            Gun::Rifle => 1.8,
            Gun::Shotgun => 2.2,
            Gun::Revolver => 1.2,
        }
    }
    pub fn mag(self) -> u32 {
        match self {
            Gun::Rifle => 5,
            Gun::Shotgun => 2,
            Gun::Revolver => 6,
        }
    }
    pub fn reload_secs(self) -> f32 {
        match self {
            Gun::Rifle => 3.0,
            Gun::Shotgun => 2.6,
            Gun::Revolver => 3.4,
        }
    }
    /// The range band it likes to fight at.
    pub fn preferred(self) -> (f32, f32) {
        match self {
            Gun::Rifle => (28.0, 50.0),
            Gun::Shotgun => (5.0, 12.0),
            Gun::Revolver => (12.0, 24.0),
        }
    }
    /// Rounds you get from the body.
    pub fn loot_rounds(self) -> u32 {
        match self {
            Gun::Rifle => 10,
            Gun::Shotgun => 4,
            Gun::Revolver => 8,
        }
    }
}

pub const WALK_SPEED: f32 = 1.5;
pub const STRAFE_SPEED: f32 = 2.3;
pub const ADVANCE_SPEED: f32 = 3.2;
pub const RETREAT_SPEED: f32 = 4.2;
pub const SEARCH_SPEED: f32 = 2.8;
/// Seconds a raider keeps looking after it last saw or heard you.
pub const ALERT_SECS: f32 = 14.0;
/// Seconds between spotting you and the first shot (it has to aim).
pub const AIM_SECS: f32 = 0.7;
/// Seconds it takes to clear a jam.
pub const UNJAM_SECS: f32 = 2.2;
/// Below this health fraction it falls back to its fire.
pub const RETREAT_HEALTH: f32 = 0.3;
/// Gunshots within this range are heard (less in a blizzard).
pub const HEARING: f32 = 90.0;

/// How far a raider can pick you out: far in daylight, little in the dark,
/// almost nothing in a rad-blizzard.
pub fn sight_range(night: bool, blizzard: bool) -> f32 {
    match (blizzard, night) {
        (true, _) => 18.0,
        (false, true) => 35.0,
        (false, false) => 65.0,
    }
}

/// Chance one shot or pellet hits you at `dist`: falls with range, worse when
/// you're running, in a blizzard and in numbing cold.
pub fn hit_chance(dist: f32, range: f32, sprinting: bool, moving: bool, blizzard: bool, temp_f: f32) -> f32 {
    let mut p = 0.68 - 0.58 * (dist / range.max(1.0)).clamp(0.0, 1.0);
    if sprinting {
        p *= 0.55;
    } else if moving {
        p *= 0.8;
    }
    if blizzard {
        p *= 0.7;
    }
    if temp_f < -20.0 {
        p *= 0.9;
    }
    p.clamp(0.03, 0.9)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// At camp, round the fire.
    Idle,
    /// Heading for where it last saw or heard you.
    Search,
    /// Has you in its sights.
    Fight,
    /// Hurt: falling back to the fire (and shooting back from there).
    Retreat,
}

/// What a raider can tell about the world this frame.
#[derive(Clone, Copy, Debug)]
pub struct Senses {
    pub pos: [f32; 2],
    pub player: [f32; 2],
    /// Nothing solid between its eyes and yours.
    pub los: bool,
    pub night: bool,
    pub blizzard: bool,
    pub temp_f: f32,
    pub health_frac: f32,
    /// Where a gunshot was just fired, if one was heard.
    pub heard: Option<[f32; 2]>,
    pub player_moving: bool,
    pub player_sprinting: bool,
}

/// One shot, and what it did to you.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Volley {
    pub gun: Gun,
    pub pellets: u32,
    pub hits: u32,
    /// Total damage landed.
    pub damage: f32,
    /// The cold seized the action as it fired.
    pub jammed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Act {
    /// Unit direction to walk (zero to stand), and how fast.
    pub dir: [f32; 2],
    pub speed: f32,
    /// Direction to face.
    pub face: [f32; 2],
    pub volley: Option<Volley>,
    /// It started reloading this frame.
    pub reloading: bool,
    /// It's aiming at you (raise the gun).
    pub aiming: bool,
}

#[derive(Clone, Debug)]
pub struct Raider {
    pub gun: Gun,
    pub mode: Mode,
    pub mag: u32,
    cooldown: f32,
    busy: f32,
    jammed: bool,
    alert: f32,
    last_known: Option<[f32; 2]>,
    pub home: [f32; 2],
    aim: f32,
    strafe: f32,
    strafe_timer: f32,
    wander: [f32; 2],
    wander_timer: f32,
    wait: f32,
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn len(a: [f32; 2]) -> f32 {
    a[0].hypot(a[1])
}

fn unit(a: [f32; 2]) -> [f32; 2] {
    let l = len(a);
    if l < 1e-6 {
        [0.0, 0.0]
    } else {
        [a[0] / l, a[1] / l]
    }
}

impl Raider {
    pub fn new(gun: Gun, home: [f32; 2]) -> Raider {
        Raider {
            gun,
            mode: Mode::Idle,
            mag: gun.mag(),
            cooldown: 0.0,
            busy: 0.0,
            jammed: false,
            alert: 0.0,
            last_known: None,
            home,
            aim: 0.0,
            strafe: 1.0,
            strafe_timer: 0.0,
            wander: home,
            wander_timer: 0.0,
            wait: 0.0,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy > 0.0
    }

    /// Someone nearby spotted you: look where they say.
    pub fn warn(&mut self, at: [f32; 2]) {
        self.alert = self.alert.max(ALERT_SECS);
        self.last_known = Some(at);
    }

    pub fn think(&mut self, dt: f32, s: &Senses, rng: &mut Rng) -> Act {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if self.busy > 0.0 {
            self.busy -= dt;
            if self.busy <= 0.0 {
                self.busy = 0.0;
                if self.jammed {
                    self.jammed = false;
                } else {
                    self.mag = self.gun.mag();
                }
            }
        }

        let to_player = sub(s.player, s.pos);
        let dist = len(to_player);
        let sees = s.los && dist <= sight_range(s.night, s.blizzard);
        let hearing = if s.blizzard { HEARING * 0.6 } else { HEARING };
        if sees {
            self.alert = ALERT_SECS;
            self.last_known = Some(s.player);
            self.aim += dt;
        } else {
            self.aim = (self.aim - dt * 2.0).max(0.0);
            self.alert = (self.alert - dt).max(0.0);
        }
        if let Some(at) = s.heard {
            if len(sub(at, s.pos)) <= hearing && !sees {
                self.alert = self.alert.max(ALERT_SECS);
                self.last_known = Some(at);
            }
        }

        self.mode = if s.health_frac < RETREAT_HEALTH && dist > 5.0 {
            Mode::Retreat
        } else if sees {
            Mode::Fight
        } else if self.alert > 0.0 && self.last_known.is_some() {
            Mode::Search
        } else {
            Mode::Idle
        };

        let (pref_min, pref_max) = self.gun.preferred();
        let mut dir = [0.0, 0.0];
        let mut speed = 0.0;
        let mut face = unit(to_player);
        match self.mode {
            Mode::Fight => {
                self.strafe_timer -= dt;
                if self.strafe_timer <= 0.0 {
                    self.strafe_timer = rng.range(1.2, 3.0);
                    self.strafe = if rng.chance(0.5) { 1.0 } else { -1.0 };
                }
                let radial = unit(to_player);
                let tangent = [-radial[1] * self.strafe, radial[0] * self.strafe];
                if dist > pref_max {
                    dir = unit([radial[0] * 0.9 + tangent[0] * 0.3, radial[1] * 0.9 + tangent[1] * 0.3]);
                    speed = ADVANCE_SPEED;
                } else if dist < pref_min {
                    dir = unit([-radial[0] * 0.8 + tangent[0] * 0.4, -radial[1] * 0.8 + tangent[1] * 0.4]);
                    speed = STRAFE_SPEED;
                } else if !self.is_busy() {
                    dir = tangent;
                    speed = STRAFE_SPEED * 0.6;
                }
            }
            Mode::Search => {
                let target = self.last_known.unwrap_or(self.home);
                let to = sub(target, s.pos);
                if len(to) > 2.0 {
                    dir = unit(to);
                    speed = SEARCH_SPEED;
                    face = dir;
                } else {
                    // Arrived and found nothing: stand and listen.
                    self.wait += dt;
                    if self.wait > 4.0 {
                        self.alert = 0.0;
                        self.last_known = None;
                        self.wait = 0.0;
                    }
                }
            }
            Mode::Retreat => {
                let to = sub(self.home, s.pos);
                if len(to) > 2.0 {
                    dir = unit(to);
                    speed = RETREAT_SPEED;
                }
            }
            Mode::Idle => {
                self.wait = 0.0;
                // Stand round the fire; in a blizzard, huddle.
                if !s.blizzard {
                    self.wander_timer -= dt;
                    if self.wander_timer <= 0.0 || len(sub(self.wander, s.pos)) < 0.8 {
                        self.wander_timer = rng.range(4.0, 9.0);
                        let a = rng.range(0.0, std::f32::consts::TAU);
                        let r = rng.range(1.0, 3.2);
                        self.wander = [self.home[0] + a.cos() * r, self.home[1] + a.sin() * r];
                    }
                    let to = sub(self.wander, s.pos);
                    if len(to) > 0.8 {
                        dir = unit(to);
                        speed = WALK_SPEED;
                    }
                }
                face = if speed > 0.0 { dir } else { unit(sub(self.home, s.pos)) };
            }
        }
        if self.mode == Mode::Retreat && sees {
            face = unit(to_player);
        }

        // Shooting: needs to see you, be in range, be aimed and be loaded.
        let mut volley = None;
        let mut reloading = false;
        let can_shoot = sees && dist <= self.gun.range() && self.aim >= AIM_SECS && !self.is_busy() && self.cooldown <= 0.0;
        if self.mag == 0 && !self.is_busy() && !self.jammed {
            self.busy = self.gun.reload_secs();
            reloading = true;
        } else if can_shoot && self.mag > 0 {
            self.mag -= 1;
            self.cooldown = self.gun.interval() * rng.range(0.9, 1.3);
            let jammed = rng.chance(jam_chance(s.temp_f));
            if jammed {
                self.jammed = true;
                self.busy = UNJAM_SECS;
            }
            let p = hit_chance(dist, self.gun.range(), s.player_sprinting, s.player_moving, s.blizzard, s.temp_f);
            let hits = (0..self.gun.pellets()).filter(|_| rng.chance(p)).count() as u32;
            // Damage falls off with range, more for shot than for a bullet.
            let falloff = 1.0 - if self.gun == Gun::Shotgun { 0.6 } else { 0.25 } * (dist / self.gun.range()).clamp(0.0, 1.0);
            volley = Some(Volley { gun: self.gun, pellets: self.gun.pellets(), hits, damage: hits as f32 * self.gun.damage() * falloff, jammed });
        }
        Act { dir, speed, face, volley, reloading, aiming: sees && self.mode != Mode::Idle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn senses(dist: f32) -> Senses {
        Senses {
            pos: [0.0, 0.0],
            player: [dist, 0.0],
            los: true,
            night: false,
            blizzard: false,
            temp_f: 0.0,
            health_frac: 1.0,
            heard: None,
            player_moving: false,
            player_sprinting: false,
        }
    }

    /// Run `secs` of game time, collecting every volley.
    fn run(r: &mut Raider, s: &Senses, secs: f32, rng: &mut Rng) -> Vec<Volley> {
        let mut v = Vec::new();
        for _ in 0..(secs * 30.0) as usize {
            if let Some(shot) = r.think(1.0 / 30.0, s, rng).volley {
                v.push(shot);
            }
        }
        v
    }

    #[test]
    fn an_idle_raider_stays_round_its_fire() {
        let mut rng = Rng::new(1);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(500.0);
        s.los = false;
        for _ in 0..600 {
            let a = r.think(0.1, &s, &mut rng);
            s.pos[0] += a.dir[0] * a.speed * 0.1;
            s.pos[1] += a.dir[1] * a.speed * 0.1;
            assert!(len(s.pos) < 5.0, "wandered to {:?}", s.pos);
        }
        assert_eq!(r.mode, Mode::Idle);
    }

    #[test]
    fn in_a_blizzard_they_huddle_and_do_not_wander() {
        let mut rng = Rng::new(2);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(500.0);
        s.los = false;
        s.blizzard = true;
        for _ in 0..200 {
            assert_eq!(r.think(0.1, &s, &mut rng).speed, 0.0);
        }
    }

    #[test]
    fn it_cannot_shoot_what_it_cannot_see() {
        let mut rng = Rng::new(3);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(30.0);
        s.los = false;
        assert!(run(&mut r, &s, 20.0, &mut rng).is_empty(), "a tree between you");
        s.los = true;
        assert!(!run(&mut r, &s, 5.0, &mut rng).is_empty(), "and then it has you");
    }

    #[test]
    fn a_blizzard_and_the_dark_shorten_its_sight() {
        let (day, night, storm) = (sight_range(false, false), sight_range(true, false), sight_range(false, true));
        assert!(day > night, "night is shorter than day");
        assert!(night > storm, "and a blizzard is shorter than night");
        assert_eq!(sight_range(true, true), storm, "a blizzard at night is no worse than a blizzard");
        let mut rng = Rng::new(4);
        let mut calm = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut storm = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(40.0);
        assert!(!run(&mut calm, &s, 6.0, &mut rng).is_empty(), "40 m in daylight is in range");
        s.blizzard = true;
        assert!(run(&mut storm, &s, 20.0, &mut rng).is_empty(), "40 m in a blizzard is out of sight");
        assert_ne!(storm.mode, Mode::Fight);
    }

    #[test]
    fn it_takes_a_moment_to_aim_and_then_is_paced_by_its_gun() {
        let mut rng = Rng::new(5);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let s = senses(35.0);
        assert!(run(&mut r, &s, AIM_SECS - 0.2, &mut rng).is_empty(), "no shot before it's aimed");
        let shots = run(&mut r, &s, 6.0, &mut rng);
        assert!(!shots.is_empty());
        assert!(shots.len() as f32 <= 6.0 / (Gun::Rifle.interval() * 0.9) + 1.0, "{} shots", shots.len());
    }

    #[test]
    fn an_empty_gun_is_reloaded_and_nothing_is_fired_meanwhile() {
        let mut rng = Rng::new(6);
        let mut r = Raider::new(Gun::Shotgun, [0.0, 0.0]);
        let s = senses(10.0);
        // Two shells, then a reload of 2.6 s.
        let mut times = Vec::new();
        let mut reloads = 0;
        for i in 0..(15.0 * 30.0) as usize {
            let a = r.think(1.0 / 30.0, &s, &mut rng);
            if a.volley.is_some() {
                times.push(i as f32 / 30.0);
            }
            if a.reloading {
                reloads += 1;
            }
        }
        assert!(reloads >= 2, "it reloaded ({reloads})");
        assert!(times.len() >= 4);
        // After a pair of shots there's a gap at least as long as the reload.
        let gap = times[2] - times[1];
        assert!(gap >= Gun::Shotgun.reload_secs() - 0.1, "gap {gap}");
    }

    #[test]
    fn cold_jams_its_gun_and_warmth_does_not() {
        let mut rng = Rng::new(7);
        let mut cold = senses(30.0);
        cold.temp_f = -35.0;
        let mut jams = 0;
        for _ in 0..40 {
            let mut r = Raider::new(Gun::Revolver, [0.0, 0.0]);
            jams += run(&mut r, &cold, 12.0, &mut rng).iter().filter(|v| v.jammed).count();
        }
        assert!(jams > 5, "{jams} jams in the cold");
        let warm = senses(30.0);
        for _ in 0..20 {
            let mut r = Raider::new(Gun::Revolver, [0.0, 0.0]);
            assert!(run(&mut r, &warm, 12.0, &mut rng).iter().all(|v| !v.jammed), "no jams above -20F");
        }
    }

    #[test]
    fn a_jammed_raider_stops_shooting_until_it_clears_it() {
        let mut rng = Rng::new(8);
        let mut r = Raider::new(Gun::Revolver, [0.0, 0.0]);
        let mut s = senses(25.0);
        s.temp_f = -60.0;
        let mut last_jam_at = None;
        for i in 0..(60.0 * 30.0) as usize {
            let t = i as f32 / 30.0;
            if let Some(v) = r.think(1.0 / 30.0, &s, &mut rng).volley {
                if let Some(j) = last_jam_at.take() {
                    assert!(t - j >= UNJAM_SECS - 0.1, "fired {:.2}s after a jam", t - j);
                }
                if v.jammed {
                    last_jam_at = Some(t);
                }
            }
        }
    }

    #[test]
    fn accuracy_falls_with_range_and_when_you_run() {
        let r = Gun::Rifle.range();
        let near = hit_chance(5.0, r, false, false, false, 0.0);
        let far = hit_chance(60.0, r, false, false, false, 0.0);
        assert!(near > far && far > 0.0);
        assert!(hit_chance(30.0, r, true, true, false, 0.0) < hit_chance(30.0, r, false, true, false, 0.0));
        assert!(hit_chance(30.0, r, false, true, false, 0.0) < hit_chance(30.0, r, false, false, false, 0.0));
        assert!(hit_chance(30.0, r, false, false, true, 0.0) < hit_chance(30.0, r, false, false, false, 0.0));
        for d in [0.0, 10.0, 80.0, 500.0] {
            let p = hit_chance(d, r, true, true, true, -50.0);
            assert!((0.03..=0.9).contains(&p));
        }
    }

    #[test]
    fn a_shotgun_is_deadly_close_and_harmless_far() {
        let mut rng = Rng::new(9);
        let total = |dist: f32, rng: &mut Rng| -> f32 {
            let mut sum = 0.0;
            for _ in 0..60 {
                let mut r = Raider::new(Gun::Shotgun, [0.0, 0.0]);
                sum += run(&mut r, &senses(dist), 6.0, rng).iter().map(|v| v.damage).sum::<f32>();
            }
            sum
        };
        let close = total(7.0, &mut rng);
        let mid = total(20.0, &mut rng);
        let far = total(40.0, &mut rng);
        assert!(close > mid * 1.5, "{close} vs {mid}");
        assert_eq!(far, 0.0, "out of range it holds fire (and walks closer)");
    }

    #[test]
    fn it_closes_in_to_its_range_and_backs_off_if_you_get_too_near() {
        let mut rng = Rng::new(10);
        let mut r = Raider::new(Gun::Shotgun, [0.0, 0.0]);
        let s = senses(40.0);
        let a = r.think(0.1, &s, &mut rng);
        assert!(a.dir[0] > 0.5 && a.speed > 0.0, "advances on a far target");
        let mut near = senses(1.5);
        near.player = [1.5, 0.0];
        let _ = r.think(0.1, &near, &mut rng);
        let a = r.think(0.1, &near, &mut rng);
        assert!(a.dir[0] < 0.0, "gives ground when you're on top of it");
    }

    #[test]
    fn a_badly_hurt_raider_falls_back_to_the_fire_and_shoots_from_there() {
        let mut rng = Rng::new(11);
        let mut r = Raider::new(Gun::Revolver, [-20.0, 0.0]);
        let mut s = senses(20.0);
        s.health_frac = 0.2;
        let a = r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Retreat);
        assert!(a.dir[0] < -0.9 && a.speed >= RETREAT_SPEED - 0.01, "runs home, away from you");
        assert!(a.face[0] > 0.9, "but keeps facing you");
        assert!(!run(&mut r, &s, 4.0, &mut rng).is_empty(), "and fires back while it sees you");
    }

    #[test]
    fn cornered_it_fights_instead() {
        let mut rng = Rng::new(12);
        let mut r = Raider::new(Gun::Shotgun, [-20.0, 0.0]);
        let mut s = senses(3.0);
        s.health_frac = 0.1;
        r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Fight, "no running past someone at arm's length");
    }

    #[test]
    fn a_heard_shot_sends_it_to_look_and_it_gives_up_after_a_while() {
        let mut rng = Rng::new(13);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(300.0);
        s.los = false;
        s.heard = Some([40.0, 0.0]);
        let a = r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Search);
        assert!(a.dir[0] > 0.9, "heads for the sound");
        s.heard = None;
        s.pos = [40.0, 0.0];
        for _ in 0..400 {
            r.think(0.1, &s, &mut rng);
        }
        assert_eq!(r.mode, Mode::Idle, "nothing there: back to the fire");
    }

    #[test]
    fn a_shot_too_far_off_is_not_heard() {
        let mut rng = Rng::new(14);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        let mut s = senses(500.0);
        s.los = false;
        s.heard = Some([HEARING + 20.0, 0.0]);
        r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Idle);
        s.blizzard = true;
        s.heard = Some([HEARING * 0.8, 0.0]);
        r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Idle, "the storm muffles it");
    }

    #[test]
    fn a_warning_from_a_friend_sends_it_looking() {
        let mut rng = Rng::new(15);
        let mut r = Raider::new(Gun::Rifle, [0.0, 0.0]);
        r.warn([30.0, 10.0]);
        let mut s = senses(300.0);
        s.los = false;
        r.think(0.1, &s, &mut rng);
        assert_eq!(r.mode, Mode::Search);
    }

    #[test]
    fn it_is_deterministic_for_a_seed() {
        let go = |seed| {
            let mut rng = Rng::new(seed);
            let mut r = Raider::new(Gun::Revolver, [0.0, 0.0]);
            run(&mut r, &senses(25.0), 10.0, &mut rng)
        };
        assert_eq!(go(99), go(99));
    }

    #[test]
    fn every_gun_has_sensible_numbers() {
        for g in [Gun::Rifle, Gun::Shotgun, Gun::Revolver] {
            let (lo, hi) = g.preferred();
            assert!(lo < hi && hi < g.range(), "{}", g.name());
            assert!(g.damage() > 0.0 && g.mag() > 0 && g.interval() > 0.0 && g.reload_secs() > g.interval());
            assert!(g.loot_rounds() > 0 && g.pellets() >= 1);
        }
    }
}
