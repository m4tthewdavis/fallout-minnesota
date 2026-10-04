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

/// The kinds of weapon you can find.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    PipeRifle,
    ScrapShotgun,
    Revolver,
    IceAxe,
}

/// Which ammunition a firearm uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ammo {
    PipeRounds,
    Shells,
    RevolverRounds,
}

impl Ammo {
    pub fn name(self) -> &'static str {
        match self {
            Ammo::PipeRounds => "pipe rounds",
            Ammo::Shells => "shotgun shells",
            Ammo::RevolverRounds => "revolver rounds",
        }
    }
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 4] = [WeaponKind::PipeRifle, WeaponKind::ScrapShotgun, WeaponKind::Revolver, WeaponKind::IceAxe];

    /// Number-key slot, 0-based.
    pub fn slot(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            WeaponKind::PipeRifle => "Pipe Rifle",
            WeaponKind::ScrapShotgun => "Scrap Shotgun",
            WeaponKind::Revolver => "Frontier Revolver",
            WeaponKind::IceAxe => "Ice Axe",
        }
    }

    pub fn ammo(self) -> Option<Ammo> {
        match self {
            WeaponKind::PipeRifle => Some(Ammo::PipeRounds),
            WeaponKind::ScrapShotgun => Some(Ammo::Shells),
            WeaponKind::Revolver => Some(Ammo::RevolverRounds),
            WeaponKind::IceAxe => None,
        }
    }
}

/// Improvements crafted at a shelter workbench with scrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Upgrade {
    /// Greased, wrapped action: never jams in the cold.
    InsulatedAction,
    /// Longer magazine (rifle).
    ExtendedMag,
    /// Tighter pattern (shotgun).
    Choke,
    /// Hot-loaded rounds: +25% damage.
    HeavyLoads,
}

impl Upgrade {
    pub const ALL: [Upgrade; 4] = [Upgrade::InsulatedAction, Upgrade::ExtendedMag, Upgrade::Choke, Upgrade::HeavyLoads];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::InsulatedAction => "Insulated action",
            Upgrade::ExtendedMag => "Extended magazine",
            Upgrade::Choke => "Choke",
            Upgrade::HeavyLoads => "Heavy loads",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Upgrade::InsulatedAction => "never jams in the cold",
            Upgrade::ExtendedMag => "+50% magazine",
            Upgrade::Choke => "tighter spread",
            Upgrade::HeavyLoads => "+25% damage",
        }
    }

    pub fn scrap_cost(self) -> u32 {
        match self {
            Upgrade::InsulatedAction => 6,
            Upgrade::ExtendedMag => 8,
            Upgrade::Choke => 5,
            Upgrade::HeavyLoads => 8,
        }
    }

    pub fn applies_to(self, kind: WeaponKind) -> bool {
        match self {
            Upgrade::InsulatedAction | Upgrade::HeavyLoads => kind.ammo().is_some(),
            Upgrade::ExtendedMag => kind == WeaponKind::PipeRifle,
            Upgrade::Choke => kind == WeaponKind::ScrapShotgun,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Weapon {
    pub kind: WeaponKind,
    pub name: &'static str,
    pub mag: u32,
    pub mag_size: u32,
    /// Damage per pellet (or per swing for melee).
    pub damage: f32,
    pub pellets: u32,
    /// Half-angle of the pellet cone, radians.
    pub spread: f32,
    /// Fraction of damage lost at maximum range (shotgun pellets).
    pub falloff: f32,
    pub range: f32,
    pub fire_interval: f32,
    pub reload_time: f32,
    pub unjam_time: f32,
    /// Scales the cold-weather jam chance (revolvers jam less, insulated never).
    pub jam_mult: f32,
    pub melee: bool,
    pub jammed: bool,
    /// Seconds until the weapon can fire again.
    pub cooldown: f32,
    /// Seconds left on a reload / unjam, 0 when idle.
    pub busy: f32,
    /// Rounds moving into the magazine when the reload finishes.
    pending: u32,
    pub upgrades: [bool; 4],
}

impl Default for Weapon {
    fn default() -> Self {
        Self::pipe_rifle()
    }
}

impl Weapon {
    pub fn new(kind: WeaponKind) -> Self {
        let base = Weapon {
            kind,
            name: kind.name(),
            mag: 8,
            mag_size: 8,
            damage: 25.0,
            pellets: 1,
            spread: 0.0,
            falloff: 0.0,
            range: 80.0,
            fire_interval: 0.35,
            reload_time: 2.2,
            unjam_time: 1.0,
            jam_mult: 1.0,
            melee: false,
            jammed: false,
            cooldown: 0.0,
            busy: 0.0,
            pending: 0,
            upgrades: [false; 4],
        };
        match kind {
            WeaponKind::PipeRifle => base,
            WeaponKind::ScrapShotgun => Weapon {
                mag: 2,
                mag_size: 2,
                damage: 9.0,
                pellets: 7,
                spread: 0.07,
                falloff: 0.7,
                range: 40.0,
                fire_interval: 0.8,
                reload_time: 2.4,
                jam_mult: 0.5,
                ..base
            },
            WeaponKind::Revolver => Weapon {
                mag: 6,
                mag_size: 6,
                damage: 42.0,
                range: 70.0,
                fire_interval: 0.5,
                reload_time: 2.6,
                jam_mult: 0.25,
                ..base
            },
            WeaponKind::IceAxe => Weapon {
                mag: 0,
                mag_size: 0,
                damage: 36.0,
                range: 2.6,
                fire_interval: 0.75,
                reload_time: 0.0,
                jam_mult: 0.0,
                melee: true,
                ..base
            },
        }
    }

    pub fn pipe_rifle() -> Self {
        Self::new(WeaponKind::PipeRifle)
    }

    pub fn has_upgrade(&self, up: Upgrade) -> bool {
        self.upgrades[up as usize]
    }

    /// Fit an upgrade. Fails if it doesn't suit this weapon or is already fitted.
    pub fn apply_upgrade(&mut self, up: Upgrade) -> Result<(), &'static str> {
        if !up.applies_to(self.kind) {
            return Err("That doesn't fit this weapon.");
        }
        if self.has_upgrade(up) {
            return Err("Already fitted.");
        }
        match up {
            Upgrade::InsulatedAction => self.jam_mult = 0.0,
            Upgrade::ExtendedMag => self.mag_size = self.mag_size * 3 / 2,
            Upgrade::Choke => self.spread *= 0.55,
            Upgrade::HeavyLoads => self.damage *= 1.25,
        }
        self.upgrades[up as usize] = true;
        Ok(())
    }

    /// The next upgrade this weapon can still take, cheapest first.
    pub fn next_upgrade(&self) -> Option<Upgrade> {
        let mut options: Vec<Upgrade> = Upgrade::ALL
            .into_iter()
            .filter(|u| u.applies_to(self.kind) && !self.has_upgrade(*u))
            .collect();
        options.sort_by_key(|u| u.scrap_cost());
        options.first().copied()
    }

    /// Damage of one pellet or swing that connects at `dist` metres.
    pub fn damage_at(&self, dist: f32) -> f32 {
        let t = (dist / self.range.max(0.1)).clamp(0.0, 1.0);
        self.damage * (1.0 - self.falloff * t)
    }

    pub fn is_reloading(&self) -> bool {
        self.busy > 0.0
    }

    /// Progress 0..1 through the current reload or unjam, if one is running.
    pub fn reload_progress(&self) -> Option<f32> {
        if self.busy <= 0.0 {
            return None;
        }
        let total = if self.jammed { self.unjam_time } else { self.reload_time };
        Some((1.0 - self.busy / total).clamp(0.0, 1.0))
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
        if self.melee {
            self.cooldown = self.fire_interval;
            return FireResult::Fired;
        }
        if self.jammed {
            return FireResult::Jammed;
        }
        if self.mag == 0 {
            return FireResult::Empty;
        }
        self.mag -= 1;
        self.cooldown = self.fire_interval;
        if rng.chance(jam_chance(air_temp_f) * self.jam_mult) {
            self.jammed = true;
            FireResult::FiredAndJammed
        } else {
            FireResult::Fired
        }
    }

    /// Press R: clears a jam if jammed, otherwise reloads from `reserve`.
    /// Returns false if there is nothing to do.
    pub fn start_reload(&mut self, reserve: &mut u32) -> bool {
        if self.melee || self.busy > 0.0 {
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

/// Every weapon you carry, which ones you own, and which is in your hands.
#[derive(Clone, Debug)]
pub struct Arsenal {
    pub weapons: Vec<Weapon>,
    pub owned: [bool; 4],
    pub current: usize,
    /// Seconds left of the draw animation after switching.
    pub draw: f32,
}

/// How long drawing a weapon takes.
pub const DRAW_SECS: f32 = 0.4;

impl Default for Arsenal {
    fn default() -> Self {
        Self::starting()
    }
}

impl Arsenal {
    /// You leave the vault with only the pipe rifle.
    pub fn starting() -> Self {
        Arsenal {
            weapons: WeaponKind::ALL.iter().map(|k| Weapon::new(*k)).collect(),
            owned: [true, false, false, false],
            current: 0,
            draw: 0.0,
        }
    }

    pub fn current(&self) -> &Weapon {
        &self.weapons[self.current]
    }

    pub fn current_mut(&mut self) -> &mut Weapon {
        &mut self.weapons[self.current]
    }

    pub fn get(&self, kind: WeaponKind) -> &Weapon {
        &self.weapons[kind.slot()]
    }

    /// Take a weapon you've found. Returns false if you already had it.
    pub fn unlock(&mut self, kind: WeaponKind) -> bool {
        let had = self.owned[kind.slot()];
        self.owned[kind.slot()] = true;
        !had
    }

    /// Switch to a slot. Fails if it's not owned, already in hand, or the
    /// current weapon is mid-reload.
    pub fn select(&mut self, slot: usize) -> bool {
        if slot >= self.weapons.len() || !self.owned[slot] || slot == self.current || self.current().is_reloading() {
            return false;
        }
        self.current = slot;
        self.draw = DRAW_SECS;
        let w = self.current_mut();
        w.cooldown = w.cooldown.max(DRAW_SECS);
        true
    }

    /// Next owned weapon in `dir` (+1 or -1), wrapping.
    pub fn cycle(&mut self, dir: i32) -> bool {
        let n = self.weapons.len() as i32;
        for step in 1..n {
            let slot = (self.current as i32 + dir.signum() * step).rem_euclid(n) as usize;
            if self.owned[slot] {
                return self.select(slot);
            }
        }
        false
    }

    pub fn tick(&mut self, dt: f32) {
        self.draw = (self.draw - dt).max(0.0);
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
        assert!(!w.tick(1.0), "still reloading");
        assert!(w.tick(1.5));
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

    #[test]
    fn reload_progress_runs_zero_to_one() {
        let mut w = Weapon::pipe_rifle();
        assert_eq!(w.reload_progress(), None);
        w.mag = 0;
        let mut reserve = 20;
        assert!(w.start_reload(&mut reserve));
        assert_eq!(w.reload_progress(), Some(0.0));
        w.tick(w.reload_time / 2.0);
        assert!((w.reload_progress().unwrap() - 0.5).abs() < 1e-4);
        w.tick(w.reload_time);
        assert_eq!(w.reload_progress(), None);
        // Unjamming uses the (shorter) unjam time.
        w.jammed = true;
        assert!(w.start_reload(&mut reserve));
        w.tick(w.unjam_time / 2.0);
        assert!((w.reload_progress().unwrap() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn each_weapon_has_its_own_character() {
        let rifle = Weapon::new(WeaponKind::PipeRifle);
        let shotgun = Weapon::new(WeaponKind::ScrapShotgun);
        let revolver = Weapon::new(WeaponKind::Revolver);
        let axe = Weapon::new(WeaponKind::IceAxe);
        assert!(shotgun.pellets > 1 && shotgun.mag_size == 2);
        assert!(shotgun.damage * shotgun.pellets as f32 > rifle.damage, "point blank the shotgun out-hits the rifle");
        assert!(revolver.damage > rifle.damage && revolver.mag_size == 6);
        assert!(revolver.jam_mult < rifle.jam_mult && shotgun.jam_mult < rifle.jam_mult);
        assert!(axe.melee && axe.range < 4.0 && axe.mag_size == 0);
        assert!(shotgun.range < rifle.range);
        for k in WeaponKind::ALL {
            assert_eq!(Weapon::new(k).kind, k);
            assert_eq!(WeaponKind::ALL[k.slot()], k);
        }
    }

    #[test]
    fn shotgun_pellets_lose_power_with_distance() {
        let w = Weapon::new(WeaponKind::ScrapShotgun);
        assert!((w.damage_at(0.0) - w.damage).abs() < 1e-5);
        assert!(w.damage_at(w.range) < w.damage * 0.4);
        assert!(w.damage_at(w.range * 2.0) >= w.damage * (1.0 - w.falloff) - 1e-5, "no further loss beyond range");
        let rifle = Weapon::new(WeaponKind::PipeRifle);
        assert_eq!(rifle.damage_at(70.0), rifle.damage);
    }

    #[test]
    fn melee_never_jams_or_needs_ammo() {
        let mut axe = Weapon::new(WeaponKind::IceAxe);
        let mut rng = Rng::new(3);
        for _ in 0..100 {
            axe.cooldown = 0.0;
            assert_eq!(axe.try_fire(-80.0, &mut rng), FireResult::Fired);
        }
        let mut reserve = 10;
        assert!(!axe.start_reload(&mut reserve));
        assert_eq!(axe.try_fire(0.0, &mut rng), FireResult::Busy, "swing has a recovery time");
    }

    #[test]
    fn revolvers_jam_far_less_in_the_cold() {
        let count = |kind| {
            let mut w = Weapon::new(kind);
            w.mag_size = 100_000;
            w.mag = 100_000;
            let mut rng = Rng::new(5);
            let mut jams = 0;
            for _ in 0..4000 {
                w.cooldown = 0.0;
                if w.try_fire(-60.0, &mut rng) == FireResult::FiredAndJammed {
                    jams += 1;
                    w.jammed = false;
                }
            }
            jams
        };
        let (rifle, revolver) = (count(WeaponKind::PipeRifle), count(WeaponKind::Revolver));
        assert!(revolver * 2 < rifle, "revolver {revolver} vs rifle {rifle}");
    }

    #[test]
    fn upgrades_change_the_weapon_once() {
        let mut w = Weapon::new(WeaponKind::PipeRifle);
        assert!(w.apply_upgrade(Upgrade::ExtendedMag).is_ok());
        assert_eq!(w.mag_size, 12);
        assert!(w.apply_upgrade(Upgrade::ExtendedMag).is_err(), "only once");
        assert!(w.apply_upgrade(Upgrade::Choke).is_err(), "choke is for shotguns");
        w.apply_upgrade(Upgrade::InsulatedAction).unwrap();
        let mut rng = Rng::new(9);
        w.mag = 100_000;
        w.mag_size = 100_000;
        for _ in 0..500 {
            w.cooldown = 0.0;
            assert_ne!(w.try_fire(-90.0, &mut rng), FireResult::FiredAndJammed, "insulated action never jams");
        }
        w.apply_upgrade(Upgrade::HeavyLoads).unwrap();
        assert!((w.damage - 31.25).abs() < 1e-4);
        assert_eq!(w.next_upgrade(), None, "all rifle upgrades fitted");
        let mut sg = Weapon::new(WeaponKind::ScrapShotgun);
        let spread = sg.spread;
        sg.apply_upgrade(Upgrade::Choke).unwrap();
        assert!(sg.spread < spread * 0.6);
        assert!(Weapon::new(WeaponKind::IceAxe).apply_upgrade(Upgrade::HeavyLoads).is_err());
        // Cheapest first.
        assert_eq!(Weapon::new(WeaponKind::PipeRifle).next_upgrade(), Some(Upgrade::InsulatedAction));
    }

    #[test]
    fn arsenal_starts_with_only_the_rifle() {
        let mut a = Arsenal::starting();
        assert_eq!(a.current().kind, WeaponKind::PipeRifle);
        assert!(!a.select(1), "can't switch to a weapon you don't own");
        assert!(!a.cycle(1));
        assert!(a.unlock(WeaponKind::Revolver));
        assert!(!a.unlock(WeaponKind::Revolver), "already owned");
        assert!(a.select(WeaponKind::Revolver.slot()));
        assert_eq!(a.current().kind, WeaponKind::Revolver);
        assert!(a.draw > 0.0 && a.current().cooldown >= DRAW_SECS - 1e-6, "can't fire while drawing");
        a.tick(1.0);
        assert_eq!(a.draw, 0.0);
    }

    #[test]
    fn cycling_skips_unowned_slots_and_wraps() {
        let mut a = Arsenal::starting();
        a.unlock(WeaponKind::IceAxe);
        a.unlock(WeaponKind::ScrapShotgun);
        assert!(a.cycle(1));
        assert_eq!(a.current().kind, WeaponKind::ScrapShotgun);
        assert!(a.cycle(1));
        assert_eq!(a.current().kind, WeaponKind::IceAxe, "skips the revolver");
        assert!(a.cycle(1));
        assert_eq!(a.current().kind, WeaponKind::PipeRifle, "wraps");
        assert!(a.cycle(-1));
        assert_eq!(a.current().kind, WeaponKind::IceAxe);
    }

    #[test]
    fn cannot_switch_mid_reload() {
        let mut a = Arsenal::starting();
        a.unlock(WeaponKind::Revolver);
        a.current_mut().mag = 0;
        let mut reserve = 10;
        assert!(a.current_mut().start_reload(&mut reserve));
        assert!(!a.select(WeaponKind::Revolver.slot()));
        a.current_mut().tick(5.0);
        assert!(a.select(WeaponKind::Revolver.slot()));
    }

    #[test]
    fn reload_takes_only_what_the_reserve_has() {
        let mut w = Weapon::new(WeaponKind::Revolver);
        w.mag = 1;
        let mut reserve = 2;
        assert!(w.start_reload(&mut reserve));
        w.tick(10.0);
        assert_eq!((w.mag, reserve), (3, 0));
        let mut none = 0;
        assert!(!w.start_reload(&mut none));
    }
}
