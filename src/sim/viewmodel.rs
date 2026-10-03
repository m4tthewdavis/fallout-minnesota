//! First-person weapon pose: where each weapon sits on screen and how its
//! moving parts sit, given what the player is doing. Pure maths so the
//! animation can be unit-tested; `gun.rs` applies it to the models.

use std::f32::consts::{PI, TAU};

use super::combat::WeaponKind;

/// Scale the weapon models are drawn at.
pub const SCALE: f32 = 0.8;

/// Hip-fire resting position, camera space (metres; -Z is forward).
pub fn hip(kind: WeaponKind) -> [f32; 3] {
    match kind {
        WeaponKind::PipeRifle => [0.19, -0.2, -0.42],
        WeaponKind::ScrapShotgun => [0.2, -0.21, -0.4],
        WeaponKind::Revolver => [0.16, -0.17, -0.34],
        WeaponKind::IceAxe => [0.2, -0.2, -0.42],
    }
}

/// Gun-space height of the sight line (rear sight over the front sight).
pub fn sight_y(kind: WeaponKind) -> f32 {
    match kind {
        WeaponKind::PipeRifle => 0.075,
        WeaponKind::ScrapShotgun => 0.062,
        WeaponKind::Revolver => 0.066,
        WeaponKind::IceAxe => 0.0,
    }
}

/// Aim-down-sights position: sights centred on the crosshair, with the rear
/// sight about 12 cm from the eye.
pub fn ads_position(kind: WeaponKind) -> [f32; 3] {
    let z = match kind {
        WeaponKind::PipeRifle => -0.19,
        WeaponKind::ScrapShotgun => -0.36,
        WeaponKind::Revolver => -0.252,
        WeaponKind::IceAxe => hip(kind)[2],
    };
    [0.0, -sight_y(kind) * SCALE, z]
}

pub fn can_aim(kind: WeaponKind) -> bool {
    kind != WeaponKind::IceAxe
}

/// How hard each weapon kicks.
fn recoil_scale(kind: WeaponKind) -> f32 {
    match kind {
        WeaponKind::PipeRifle => 1.0,
        WeaponKind::ScrapShotgun => 1.9,
        WeaponKind::Revolver => 1.5,
        WeaponKind::IceAxe => 0.0,
    }
}

/// Everything that drives the pose this frame.
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub kind: WeaponKind,
    /// Seconds since start (for idle breathing).
    pub time: f32,
    /// Walk-cycle phase in radians (advances with distance walked).
    pub stride: f32,
    pub moving: bool,
    pub sprinting: bool,
    /// 1 right after a shot, decaying to 0.
    pub recoil: f32,
    /// Reload or unjam progress 0..1, if one is under way.
    pub reload: Option<f32>,
    /// True when the busy action is clearing a jam rather than reloading.
    pub unjamming: bool,
    /// Melee swing progress 0..1 while the axe is mid-swing.
    pub swing: Option<f32>,
    /// 1 right after switching weapons, falling to 0 (the draw animation).
    pub draw: f32,
    /// 0 = hip, 1 = fully aimed down the sights.
    pub ads: f32,
    /// 0 = clear, 1 = muzzle pressed against a wall or tree.
    pub blocked: f32,
    /// Smoothed mouse-look velocity (yaw, pitch), radians per second.
    pub look: [f32; 2],
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            kind: WeaponKind::PipeRifle,
            time: 0.0,
            stride: 0.0,
            moving: false,
            sprinting: false,
            recoil: 0.0,
            reload: None,
            unjamming: false,
            swing: None,
            draw: 0.0,
            ads: 0.0,
            blocked: 0.0,
            look: [0.0; 2],
        }
    }
}

/// The resulting pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Gun position in camera space.
    pub pos: [f32; 3],
    /// Gun rotation as (pitch, yaw, roll) radians.
    pub rot: [f32; 3],
    /// Magazine drop below its seat (metres, gun space; 0 = seated).
    pub mag_drop: f32,
    /// How far back the bolt is pulled, 0..1.
    pub bolt: f32,
    /// Hammer angle: 0 = cocked, 1 = fallen.
    pub hammer: f32,
    /// Trigger pull 0..1.
    pub trigger: f32,
    /// Shotgun barrels broken open / revolver cylinder swung out, 0..1.
    pub open: f32,
    /// Revolver cylinder rotation, radians.
    pub spin: f32,
    /// Rounds are sitting in the chambers (hidden while a reload has them out).
    pub chambers_loaded: bool,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 -> 1 over [a, b].
fn ramp(t: f32, a: f32, b: f32) -> f32 {
    smooth((t - a) / (b - a))
}

/// A bump that rises over [a, b] and falls over [b, c].
fn bump(t: f32, a: f32, b: f32, c: f32) -> f32 {
    ramp(t, a, b) * (1.0 - ramp(t, b, c))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn pose(i: &Inputs) -> Pose {
    let kind = i.kind;
    let ads = if can_aim(kind) { smooth(i.ads) } else { 0.0 };
    let hip_w = 1.0 - ads;
    let (hp, ap) = (hip(kind), ads_position(kind));
    let mut pos = [lerp(hp[0], ap[0], ads), lerp(hp[1], ap[1], ads), lerp(hp[2], ap[2], ads)];
    let mut rot = [0.0f32; 3];

    // Idle breathing, much smaller when aiming.
    let breathe = 0.25 + 0.75 * hip_w;
    pos[1] += (i.time * 1.6).sin() * 0.003 * breathe;
    pos[0] += (i.time * 0.8).sin() * 0.002 * breathe;

    // Walking / sprinting bob: a figure-eight following the stride.
    if i.moving {
        let amp = if i.sprinting { 1.0 } else { 0.4 } * (0.3 + 0.7 * hip_w);
        pos[0] += i.stride.sin() * 0.012 * amp;
        pos[1] -= (i.stride * 2.0).sin().abs() * 0.014 * amp;
        rot[2] += i.stride.sin() * 0.03 * amp;
    }
    // Sprinting carries the weapon across the chest.
    if i.sprinting && i.moving {
        let s = hip_w;
        pos[0] -= 0.04 * s;
        pos[1] -= 0.05 * s;
        rot[0] -= 0.25 * s;
        rot[1] += 0.55 * s;
        rot[2] += 0.2 * s;
    }

    // The weapon lags behind fast mouse movement.
    let lag = 0.3 + 0.7 * hip_w;
    rot[1] += (i.look[0] * 0.012).clamp(-0.08, 0.08) * lag;
    rot[0] += (i.look[1] * 0.012).clamp(-0.08, 0.08) * lag;
    pos[0] -= (i.look[0] * 0.002).clamp(-0.015, 0.015) * lag;

    // Recoil: kick back and muzzle climb.
    let r = i.recoil.clamp(0.0, 1.0);
    let k = recoil_scale(kind);
    pos[2] += r * k * lerp(0.07, 0.04, ads);
    rot[0] += r * k * lerp(0.13, 0.05, ads);

    // Muzzle against a wall: tip the weapon down and pull it in.
    let b = smooth(i.blocked);
    pos[1] -= 0.08 * b;
    pos[2] += 0.06 * b;
    rot[0] -= 0.7 * b;

    // Drawing a weapon: it comes up from below.
    let d = smooth(i.draw);
    pos[1] -= 0.28 * d;
    pos[0] += 0.05 * d;
    rot[0] += 0.7 * d;
    rot[2] += 0.3 * d;

    let mut out = Pose {
        pos,
        rot,
        mag_drop: 0.0,
        bolt: r,
        // The hammer falls with the shot and is re-cocked as the action cycles.
        hammer: if r > 0.5 { (r - 0.5) * 2.0 } else { 0.0 },
        trigger: if r > 0.6 { 1.0 } else { r / 0.6 },
        open: 0.0,
        spin: 0.0,
        chambers_loaded: true,
    };

    if kind == WeaponKind::IceAxe {
        axe_swing(&mut out, i.swing);
        return out;
    }
    if let Some(p) = i.reload {
        let p = p.clamp(0.0, 1.0);
        match (kind, i.unjamming) {
            (WeaponKind::PipeRifle, false) => rifle_reload(&mut out, p),
            (WeaponKind::PipeRifle, true) => rifle_unjam(&mut out, p),
            (WeaponKind::ScrapShotgun, unjam) => shotgun_reload(&mut out, p, unjam),
            (WeaponKind::Revolver, unjam) => revolver_reload(&mut out, p, unjam),
            (WeaponKind::IceAxe, _) => {}
        }
    }
    out
}

/// Lower the weapon and roll it so the action is visible, then bring it back.
fn lower(out: &mut Pose, p: f32, roll: f32) -> f32 {
    let lowered = ramp(p, 0.0, 0.15) * (1.0 - ramp(p, 0.88, 1.0));
    out.pos[1] -= 0.06 * lowered;
    out.pos[0] -= 0.03 * lowered;
    out.rot[2] += roll * lowered;
    out.rot[0] += 0.12 * lowered;
    lowered
}

fn rifle_reload(out: &mut Pose, p: f32) {
    lower(out, p, 0.55);
    // Mag out (0.15-0.35), away (0.35-0.55), back in (0.55-0.75), then the bolt (0.75-0.9).
    out.mag_drop = 0.3 * ramp(p, 0.15, 0.35) * (1.0 - ramp(p, 0.55, 0.75));
    out.bolt = out.bolt.max(ramp(p, 0.75, 0.82) * (1.0 - ramp(p, 0.84, 0.9)));
    out.hammer = 0.0;
}

fn rifle_unjam(out: &mut Pose, p: f32) {
    let lowered = lower(out, p, 0.55);
    // Rack the bolt hard, twice.
    out.bolt = ((p * 2.0 * PI * 2.0).sin() * 0.5 + 0.5) * lowered;
    out.rot[2] += (p * 40.0).sin() * 0.02 * lowered;
}

fn shotgun_reload(out: &mut Pose, p: f32, unjam: bool) {
    let lowered = lower(out, p, 0.35);
    // Barrels down so the breech is open to the player, held up where we can see it.
    out.rot[0] -= 0.3 * lowered;
    out.pos[1] += 0.1 * lowered;
    out.pos[2] -= 0.12 * lowered;
    out.open = ramp(p, 0.14, 0.24) * (1.0 - ramp(p, 0.78, 0.86));
    if unjam {
        // A sharp rap on the breech.
        out.rot[2] += (p * 55.0).sin() * 0.03 * lowered;
        return;
    }
    // Shells out (0.36), then two in (0.58, 0.66): the gun twitches with each.
    out.chambers_loaded = !(0.36..0.58).contains(&p);
    for at in [0.36, 0.58, 0.66] {
        out.pos[2] += 0.012 * bump(p, at - 0.03, at, at + 0.05);
    }
}

fn revolver_reload(out: &mut Pose, p: f32, unjam: bool) {
    let lowered = lower(out, p, 0.3);
    out.open = ramp(p, 0.14, 0.22) * (1.0 - ramp(p, 0.72, 0.8));
    if !unjam {
        // Muzzle up to dump the empties (0.3-0.45), rounds back in (0.64).
        out.rot[0] += 0.75 * bump(p, 0.28, 0.36, 0.46) * lowered;
        out.rot[2] += (p * 38.0).sin() * 0.03 * bump(p, 0.3, 0.38, 0.46);
        out.chambers_loaded = !(0.4..0.64).contains(&p);
        out.pos[2] += 0.012 * bump(p, 0.6, 0.64, 0.7);
    }
    // Spin the cylinder as it closes.
    out.spin = ramp(p, 0.78, 0.95) * TAU * 1.5;
}

/// The axe swings overhead and chops down across the screen.
fn axe_swing(out: &mut Pose, swing: Option<f32>) {
    let Some(p) = swing else { return };
    let p = p.clamp(0.0, 1.0);
    let windup = bump(p, 0.0, 0.22, 0.34);
    let strike = ramp(p, 0.3, 0.5) * (1.0 - ramp(p, 0.62, 1.0));
    out.pos[1] += 0.09 * windup - 0.14 * strike;
    out.pos[2] += 0.1 * windup - 0.16 * strike;
    out.pos[0] += -0.06 * windup - 0.14 * strike;
    out.rot[0] += -0.9 * windup + 1.5 * strike;
    out.rot[1] += 0.3 * windup - 0.6 * strike;
    out.rot[2] += 0.5 * windup - 0.9 * strike;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    fn with(kind: WeaponKind) -> Inputs {
        Inputs { kind, ..Default::default() }
    }

    #[test]
    fn hip_and_ads_positions() {
        for kind in [WeaponKind::PipeRifle, WeaponKind::ScrapShotgun, WeaponKind::Revolver] {
            let hip_pose = pose(&with(kind));
            assert!(close(hip_pose.pos[0], hip(kind)[0], 0.005), "{kind:?}");
            let aimed = pose(&Inputs { ads: 1.0, ..with(kind) });
            // Sights on the crosshair: centred, with the sight line on the axis.
            assert!(aimed.pos[0].abs() < 0.003, "{kind:?}");
            assert!(close(aimed.pos[1] + sight_y(kind) * SCALE, 0.0, 0.003), "{kind:?}");
        }
    }

    #[test]
    fn the_nearest_part_is_a_sensible_distance_from_the_eye() {
        // The part of each weapon nearest the eye when aiming, as a gun-space z
        // (the rifle's peep sight, the shotgun's receiver, the revolver's rear
        // sight; see gun.rs). Too close and it fills the screen.
        for (kind, near_z) in [(WeaponKind::PipeRifle, 0.09), (WeaponKind::ScrapShotgun, 0.095), (WeaponKind::Revolver, 0.065)] {
            let eye = ads_position(kind)[2] + near_z * SCALE;
            assert!(eye < -0.09 && eye > -0.3, "{kind:?}: nearest part {eye} m from the eye");
        }
    }

    #[test]
    fn the_axe_cannot_aim() {
        assert!(!can_aim(WeaponKind::IceAxe));
        let a = pose(&Inputs { ads: 1.0, ..with(WeaponKind::IceAxe) });
        assert!(close(a.pos[0], hip(WeaponKind::IceAxe)[0], 0.01));
    }

    #[test]
    fn rifle_reload_drops_the_magazine_and_brings_it_back() {
        let at = |p: f32| pose(&Inputs { reload: Some(p), ..Default::default() });
        assert_eq!(at(0.0).mag_drop, 0.0);
        assert!(at(0.45).mag_drop > 0.25, "magazine is out mid-reload");
        assert_eq!(at(0.8).mag_drop, 0.0, "and seated again before the end");
        assert!(at(0.83).bolt > 0.9, "bolt is worked after the new mag goes in");
        let end = at(1.0);
        let rest = pose(&Inputs::default());
        for k in 0..3 {
            assert!(close(end.pos[k], rest.pos[k], 1e-4), "back at rest when done");
        }
    }

    #[test]
    fn unjam_racks_the_bolt_without_dropping_the_mag() {
        let mut max_bolt = 0.0f32;
        for k in 0..=20 {
            let p = pose(&Inputs {
                reload: Some(k as f32 / 20.0),
                unjamming: true,
                ..Default::default()
            });
            assert_eq!(p.mag_drop, 0.0);
            max_bolt = max_bolt.max(p.bolt);
        }
        assert!(max_bolt > 0.8);
    }

    #[test]
    fn shotgun_breaks_open_unloads_reloads_and_closes() {
        let at = |p: f32| pose(&Inputs { reload: Some(p), ..with(WeaponKind::ScrapShotgun) });
        assert_eq!(at(0.0).open, 0.0);
        assert!(at(0.4).open > 0.95, "wide open while unloading");
        assert!(!at(0.45).chambers_loaded, "shells are out");
        assert!(at(0.7).chambers_loaded, "new shells are in");
        assert!(at(0.7).open > 0.9, "still open until they are in");
        assert!(close(at(0.95).open, 0.0, 0.01), "snapped shut");
        assert!(at(0.4).rot[0] < at(0.0).rot[0] - 0.1, "barrels tip down while open");
    }

    #[test]
    fn revolver_swings_the_cylinder_out_dumps_loads_and_spins_shut() {
        let at = |p: f32| pose(&Inputs { reload: Some(p), ..with(WeaponKind::Revolver) });
        assert!(at(0.3).open > 0.95);
        assert!(!at(0.5).chambers_loaded && at(0.7).chambers_loaded);
        assert!(at(0.36).rot[0] > at(0.1).rot[0] + 0.4, "muzzle up to dump the empties");
        assert!(at(0.9).spin > 2.0, "spun as it closes");
        assert!(close(at(1.0).open, 0.0, 0.01));
        assert_eq!(at(0.0).spin, 0.0);
    }

    #[test]
    fn recoil_kicks_back_and_up_and_drops_the_hammer() {
        let shot = pose(&Inputs { recoil: 1.0, ..Default::default() });
        let rest = pose(&Inputs::default());
        assert!(shot.pos[2] > rest.pos[2]);
        assert!(shot.rot[0] > rest.rot[0]);
        assert_eq!(shot.hammer, 1.0);
        assert_eq!(shot.trigger, 1.0);
        assert_eq!(rest.hammer, 0.0);
    }

    #[test]
    fn heavier_weapons_kick_harder() {
        let kick = |kind| {
            let a = pose(&Inputs { recoil: 1.0, ..with(kind) });
            let b = pose(&with(kind));
            a.rot[0] - b.rot[0]
        };
        assert!(kick(WeaponKind::ScrapShotgun) > kick(WeaponKind::Revolver));
        assert!(kick(WeaponKind::Revolver) > kick(WeaponKind::PipeRifle));
    }

    #[test]
    fn walls_lower_the_weapon_and_sprinting_carries_it() {
        let rest = pose(&Inputs::default());
        let blocked = pose(&Inputs { blocked: 1.0, ..Default::default() });
        assert!(blocked.pos[1] < rest.pos[1] && blocked.rot[0] < rest.rot[0] - 0.5);
        let sprint = pose(&Inputs {
            moving: true,
            sprinting: true,
            ..Default::default()
        });
        assert!(sprint.rot[1] > 0.4, "weapon swings across the chest");
    }

    #[test]
    fn drawing_a_weapon_raises_it_from_below() {
        let rest = pose(&with(WeaponKind::Revolver));
        let drawing = pose(&Inputs { draw: 1.0, ..with(WeaponKind::Revolver) });
        assert!(drawing.pos[1] < rest.pos[1] - 0.2);
        let done = pose(&Inputs { draw: 0.0, ..with(WeaponKind::Revolver) });
        assert_eq!(done, rest);
    }

    #[test]
    fn the_axe_winds_up_then_chops_down_across_the_screen() {
        let at = |p: f32| pose(&Inputs { swing: Some(p), ..with(WeaponKind::IceAxe) });
        let rest = pose(&with(WeaponKind::IceAxe));
        assert!(at(0.22).rot[0] < rest.rot[0] - 0.5, "head goes back and up");
        assert!(at(0.22).pos[1] > rest.pos[1]);
        assert!(at(0.5).rot[0] > rest.rot[0] + 1.0, "then forward and down");
        assert!(at(0.5).pos[0] < rest.pos[0] - 0.1, "across the body");
        assert!(close(at(1.0).rot[0], rest.rot[0], 0.01), "recovers");
        assert!(close(at(1.0).pos[1], rest.pos[1], 0.01));
    }

    #[test]
    fn mouse_sway_is_bounded() {
        let p = pose(&Inputs {
            look: [1000.0, -1000.0],
            ..Default::default()
        });
        assert!(p.rot[1].abs() <= 0.08 + 1e-6 && p.rot[0].abs() <= 0.08 + 1e-6);
    }
}
