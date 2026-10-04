//! First-person weapon pose: where each weapon sits on screen and how its
//! moving parts sit, given what the player is doing. Pure maths so the
//! animation can be unit-tested; `gun.rs` applies it to the models.

use std::f32::consts::{PI, TAU};

use super::combat::WeaponKind;
use super::mathx::lerp;

/// Scale the weapon models are drawn at.
pub const SCALE: f32 = 0.8;

/// Hip-fire resting position, camera space (metres; -Z is forward).
pub fn hip(kind: WeaponKind) -> [f32; 3] {
    match kind {
        WeaponKind::PipeRifle => [0.19, -0.19, -0.43],
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
    /// The hands, in gun space.
    pub left: Hand,
    pub right: Hand,
    /// What the left hand is carrying.
    pub held: Held,
    /// Where the magazine is when a hand has it out of the gun (gun space).
    pub mag_pos: Option<[f32; 3]>,
}

/// A gloved hand: where its palm is and which way it faces, in gun space.
/// `x` runs across the knuckles (the thumb is on -x for the right hand and
/// +x for the mirrored left), `y` is the back of the hand; the fingers point
/// along -z before they curl.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    pub pos: [f32; 3],
    pub x: [f32; 3],
    pub y: [f32; 3],
    /// 0 = open, 1 = closed round a grip.
    pub grip: f32,
}

/// What the left hand is holding during a reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Nothing,
    Magazine,
    Shells,
    Rounds,
}

type V = [f32; 3];

fn vadd(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn vsub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vscale(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn vdot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vcross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn vnorm(a: V) -> V {
    let l = vdot(a, a).sqrt().max(1e-6);
    vscale(a, 1.0 / l)
}
fn vlerp(a: V, b: V, t: f32) -> V {
    vadd(a, vscale(vsub(b, a), t))
}

/// Where a closed fist's fingers wrap (the grip passes through here), in
/// hand space: just below and in front of the knuckles.
pub const FIST_CENTRE: [f32; 3] = [0.0, -0.024, -0.042];

impl Hand {
    /// A hand closed round a grip centred on `centre`, palm facing
    /// `palm`, fingers reaching towards `fingers` before they curl.
    pub fn gripping(centre: V, palm: V, fingers: V, grip: f32) -> Hand {
        let y = vnorm(vscale(palm, -1.0));
        let z0 = vscale(fingers, -1.0);
        let z = vnorm(vsub(z0, vscale(y, vdot(z0, y))));
        let x = vcross(y, z);
        let offset = vadd(vadd(vscale(x, FIST_CENTRE[0]), vscale(y, FIST_CENTRE[1])), vscale(z, FIST_CENTRE[2]));
        Hand { pos: vsub(centre, offset), x, y, grip }
    }

    pub fn z(&self) -> V {
        vcross(self.x, self.y)
    }

    /// Where the hand's grip centre is.
    pub fn centre(&self) -> V {
        let z = self.z();
        vadd(self.pos, vadd(vadd(vscale(self.x, FIST_CENTRE[0]), vscale(self.y, FIST_CENTRE[1])), vscale(z, FIST_CENTRE[2])))
    }

    /// Blend towards another hand pose.
    pub fn towards(&self, other: &Hand, t: f32) -> Hand {
        let t = t.clamp(0.0, 1.0);
        let y = vnorm(vlerp(self.y, other.y, t));
        let x0 = vlerp(self.x, other.x, t);
        let x = vnorm(vsub(x0, vscale(y, vdot(x0, y))));
        Hand { pos: vlerp(self.pos, other.pos, t), x, y, grip: self.grip + (other.grip - self.grip) * t }
    }

    fn moved(mut self, by: V) -> Hand {
        self.pos = vadd(self.pos, by);
        self
    }

    fn with_grip(mut self, grip: f32) -> Hand {
        self.grip = grip;
        self
    }
}

/// The rifle's magazine seat (top of the magazine), gun space.
pub const RIFLE_MAG_REST: [f32; 3] = [0.0, -0.0475, -0.075];
/// The rifle's bolt handle knob at rest, gun space.
pub const RIFLE_BOLT_KNOB: [f32; 3] = [0.084, 0.02, 0.0];
/// The shotgun's hinge, and the revolver cylinder's resting centre.
pub const SHOTGUN_HINGE: [f32; 3] = [0.0, 0.0, -0.025];
pub const REVOLVER_CYLINDER: [f32; 3] = [0.0, 0.004, -0.05];

/// Where each hand rests on each weapon: (left, right).
pub fn rest_hands(kind: WeaponKind) -> (Hand, Hand) {
    match kind {
        WeaponKind::PipeRifle => (
            // Under the handguard, palm up, fingers round its right side.
            Hand::gripping([0.0, -0.03, -0.37], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 1.0),
            // On the pistol grip, index finger by the trigger.
            Hand::gripping([0.0, -0.086, 0.088], [-1.0, 0.0, 0.0], [0.0, -0.343, -0.939], 1.0),
        ),
        WeaponKind::ScrapShotgun => (
            Hand::gripping([0.0, -0.014, -0.32], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 1.0),
            // Round the wrist of the stock.
            Hand::gripping([0.0, -0.02, 0.14], [-0.5, -0.85, 0.0], [-0.85, 0.5, 0.0], 1.0),
        ),
        WeaponKind::Revolver => (
            // Cupped round the right hand from the left.
            Hand::gripping([-0.03, -0.095, 0.075], [1.0, 0.0, 0.0], [0.0, -0.31, -0.95], 0.8),
            Hand::gripping([0.0, -0.078, 0.085], [-1.0, 0.0, 0.0], [0.0, -0.315, -0.949], 1.0),
        ),
        WeaponKind::IceAxe => (
            // Both hands on the haft, the left one lower.
            Hand::gripping([0.0, -0.085, 0.052], [1.0, 0.0, 0.0], [0.0, -0.523, -0.852], 1.0),
            Hand::gripping([0.0, -0.017, 0.0105], [-1.0, 0.0, 0.0], [0.0, -0.523, -0.852], 1.0),
        ),
    }
}

/// Off screen, low and to the left: where the left hand goes for a fresh
/// magazine or a handful of rounds.
fn pocket() -> Hand {
    Hand::gripping([-0.34, -0.42, 0.12], [0.0, 1.0, 0.0], [0.3, 0.0, -1.0], 1.0)
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
        left: rest_hands(kind).0,
        right: rest_hands(kind).1,
        held: Held::Nothing,
        mag_pos: None,
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

/// Bring the weapon in towards the middle of the screen and roll it so the
/// action faces you, then put it back. Returns how far into that pose it is.
/// How a weapon is held up for reloading: an offset from the hip and a turn
/// (pitch, yaw, roll) that shows its working side to the camera.
struct Present {
    offset: [f32; 3],
    turn: [f32; 3],
}

fn present_for(kind: WeaponKind) -> Present {
    match kind {
        // Muzzle swung right and up a touch: the left side and magazine well face you.
        WeaponKind::PipeRifle => Present { offset: [-0.11, 0.035, -0.02], turn: [0.16, -0.55, 0.3] },
        // Stock stays low under the arm; the barrels drop on their hinge so the
        // open breech looks up at you.
        WeaponKind::ScrapShotgun => Present { offset: [-0.08, -0.03, -0.06], turn: [0.14, 0.2, 0.3] },
        // Cylinder side towards you.
        WeaponKind::Revolver => Present { offset: [-0.09, 0.03, -0.03], turn: [0.1, -0.45, 0.3] },
        WeaponKind::IceAxe => Present { offset: [0.0; 3], turn: [0.0; 3] },
    }
}

fn present(out: &mut Pose, p: f32, kind: WeaponKind) -> f32 {
    let k = ramp(p, 0.0, 0.12) * (1.0 - ramp(p, 0.9, 1.0));
    let pr = present_for(kind);
    for a in 0..3 {
        out.pos[a] += pr.offset[a] * k;
        out.rot[a] += pr.turn[a] * k;
    }
    k
}

/// Step the left hand through a list of (time, pose) keys.
fn keyed(p: f32, keys: &[(f32, Hand)]) -> Hand {
    if p <= keys[0].0 {
        return keys[0].1;
    }
    for w in keys.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if p <= t1 {
            return a.towards(&b, smooth((p - t0) / (t1 - t0).max(1e-4)));
        }
    }
    keys[keys.len() - 1].1
}

fn rifle_reload(out: &mut Pose, p: f32) {
    present(out, p, WeaponKind::PipeRifle);
    let rest = out.left;
    // The hand cups the bottom of the magazine.
    let mag_hand = |drop: f32| Hand::gripping(vadd(RIFLE_MAG_REST, [0.0, -0.09 - drop, 0.004]), [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], 1.0);
    let bolt_hand = Hand::gripping(vadd(RIFLE_BOLT_KNOB, [0.0, 0.0, 0.0]), [-0.3, -0.3, 0.9], [0.0, -1.0, 0.0], 0.8);
    let away = 0.12;
    out.left = keyed(
        p,
        &[
            (0.0, rest),
            (0.1, mag_hand(0.0).with_grip(0.4)),
            (0.12, mag_hand(0.0)),
            (0.24, mag_hand(away)),
            (0.4, pocket()),
            (0.5, pocket()),
            (0.64, mag_hand(away)),
            (0.72, mag_hand(0.0)),
            (0.76, bolt_hand.with_grip(0.4)),
            (0.78, bolt_hand),
            (0.84, bolt_hand.moved([0.0, 0.0, 0.07])),
            (0.88, bolt_hand.moved([0.0, 0.0, 0.07]).with_grip(0.3)),
            (1.0, rest),
        ],
    );
    // The magazine travels with the hand from release until it's seated.
    if (0.12..0.72).contains(&p) {
        out.held = Held::Magazine;
        out.mag_pos = Some(vadd(out.left.centre(), [0.0, 0.09, -0.004]));
    }
    // A shove as it seats, then the bolt.
    out.pos[1] += 0.01 * bump(p, 0.68, 0.72, 0.76);
    out.bolt = out.bolt.max(ramp(p, 0.78, 0.84) * (1.0 - ramp(p, 0.84, 0.88)));
    out.hammer = 0.0;
}

fn rifle_unjam(out: &mut Pose, p: f32) {
    let k = present(out, p, WeaponKind::PipeRifle);
    let rest = out.left;
    let bolt_hand = Hand::gripping(RIFLE_BOLT_KNOB, [-0.3, -0.3, 0.9], [0.0, -1.0, 0.0], 0.8);
    // Rack the bolt hard, twice.
    let rack = ((p - 0.2) * 2.0 * PI * 2.0 / 0.8).sin() * 0.5 + 0.5;
    let racking = ramp(p, 0.15, 0.22) * (1.0 - ramp(p, 0.82, 0.9));
    out.bolt = rack * racking;
    out.left = keyed(p, &[(0.0, rest), (0.18, bolt_hand), (0.86, bolt_hand), (1.0, rest)]);
    if (0.18..0.86).contains(&p) {
        out.left = out.left.moved([0.0, 0.0, 0.07 * out.bolt]);
    }
    out.rot[2] += (p * 40.0).sin() * 0.02 * k;
}

/// Where the left hand holds the shotgun's forend, opened by `open`.
fn forend_hand(rest: Hand, open: f32) -> Hand {
    // The barrel assembly tips down about the hinge.
    let a = -0.62 * open;
    let (s, c) = a.sin_cos();
    let rot = |v: V| -> V { [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c] };
    let rel = vsub(rest.pos, SHOTGUN_HINGE);
    Hand { pos: vadd(SHOTGUN_HINGE, rot(rel)), x: rot(rest.x), y: rot(rest.y), grip: rest.grip }
}

fn shotgun_reload(out: &mut Pose, p: f32, unjam: bool) {
    let k = present(out, p, WeaponKind::ScrapShotgun);
    out.open = ramp(p, 0.14, 0.22) * (1.0 - ramp(p, 0.8, 0.86));
    let rest = out.left;
    if unjam {
        // A sharp rap on the breech.
        out.rot[2] += (p * 55.0).sin() * 0.03 * k;
        out.left = forend_hand(rest, out.open);
        return;
    }
    // Shells go in from above and behind the open chambers.
    let breech = Hand::gripping([0.0, 0.07, 0.03], [0.0, -0.6, -0.8], [0.0, -0.8, 0.6], 0.7);
    let on_forend = forend_hand(rest, out.open);
    out.left = if p < 0.34 || p > 0.76 {
        on_forend
    } else {
        keyed(
            p,
            &[
                (0.34, forend_hand(rest, 1.0)),
                (0.44, pocket()),
                (0.47, pocket()),
                (0.54, breech),
                (0.57, breech.moved([0.0, -0.02, 0.0])),
                (0.6, breech),
                (0.64, breech.moved([0.0, -0.02, 0.0])),
                (0.67, breech),
                (0.76, forend_hand(rest, 1.0)),
            ],
        )
    };
    if (0.4..0.64).contains(&p) {
        out.held = Held::Shells;
    }
    // Empties kicked out (0.3), two fresh shells pushed home (0.56, 0.64).
    out.chambers_loaded = !(0.3..0.56).contains(&p);
    out.pos[1] += 0.015 * bump(p, 0.26, 0.3, 0.36);
    for at in [0.56, 0.64] {
        out.pos[2] += 0.012 * bump(p, at - 0.03, at, at + 0.05);
    }
    // The snap shut.
    out.pos[1] += 0.012 * bump(p, 0.8, 0.84, 0.9);
}

fn revolver_reload(out: &mut Pose, p: f32, unjam: bool) {
    let k = present(out, p, WeaponKind::Revolver);
    out.open = ramp(p, 0.14, 0.22) * (1.0 - ramp(p, 0.72, 0.8));
    let rest = out.left;
    // The left hand's fingers push the cylinder out from the left.
    let at_cylinder = |open: f32| Hand::gripping(vadd(REVOLVER_CYLINDER, [-0.065 * open - 0.035, -0.012 * open, 0.0]), [1.0, 0.0, 0.0], [0.0, 0.3, -0.95], 0.6);
    if !unjam {
        // Muzzle up to dump the empties (0.3-0.45), rounds back in (0.64).
        out.rot[0] += 0.75 * bump(p, 0.28, 0.36, 0.46) * k;
        out.rot[2] += (p * 38.0).sin() * 0.03 * bump(p, 0.3, 0.38, 0.46);
        out.chambers_loaded = !(0.4..0.64).contains(&p);
        out.pos[2] += 0.012 * bump(p, 0.6, 0.64, 0.7);
        let loading = Hand::gripping(vadd(REVOLVER_CYLINDER, [-0.065, 0.04, 0.03]), [0.0, -0.7, -0.7], [0.0, -0.7, 0.7], 0.6);
        out.left = keyed(
            p,
            &[
                (0.0, rest),
                (0.12, at_cylinder(0.0)),
                (0.22, at_cylinder(1.0)),
                (0.46, at_cylinder(1.0)),
                (0.52, pocket()),
                (0.55, pocket()),
                (0.62, loading),
                (0.66, loading.moved([0.0, -0.015, 0.0])),
                (0.72, at_cylinder(1.0)),
                (0.8, at_cylinder(0.0)),
                (1.0, rest),
            ],
        );
        if (0.5..0.64).contains(&p) {
            out.held = Held::Rounds;
        }
    } else {
        out.left = keyed(p, &[(0.0, rest), (0.12, at_cylinder(0.0)), (0.22, at_cylinder(1.0)), (0.72, at_cylinder(1.0)), (0.8, at_cylinder(0.0)), (1.0, rest)]);
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

    fn dist(a: V, b: V) -> f32 {
        vdot(vsub(a, b), vsub(a, b)).sqrt()
    }

    #[test]
    fn hands_sit_on_the_weapon_at_rest() {
        for kind in WeaponKind::ALL {
            let p = pose(&with(kind));
            let (l, r) = rest_hands(kind);
            assert_eq!(p.left, l);
            assert_eq!(p.right, r);
            assert_eq!(p.held, Held::Nothing);
            // Each hand is a proper rotation.
            for h in [l, r] {
                assert!((vdot(h.x, h.x) - 1.0).abs() < 1e-4 && vdot(h.x, h.y).abs() < 1e-4, "{kind:?}");
            }
            // The grips are where we asked.
            assert!(dist(r.centre(), rest_hands(kind).1.centre()) < 1e-5);
        }
        let (l, _) = rest_hands(WeaponKind::PipeRifle);
        assert!(dist(l.centre(), [0.0, -0.03, -0.37]) < 1e-4, "left hand wraps the handguard");
    }

    #[test]
    fn the_rifle_magazine_leaves_in_the_hand_and_comes_back() {
        let at = |p: f32| pose(&Inputs { reload: Some(p), ..with(WeaponKind::PipeRifle) });
        // Pulled out and carried off screen.
        let out = at(0.2);
        assert_eq!(out.held, Held::Magazine);
        assert!(out.mag_pos.unwrap()[1] < RIFLE_MAG_REST[1] - 0.05);
        let gone = at(0.45);
        assert!(dist(gone.left.pos, rest_hands(WeaponKind::PipeRifle).0.pos) > 0.3, "hand off fetching a magazine");
        // Seated again and the bolt worked.
        let seated = at(0.74);
        assert!(seated.mag_pos.is_none());
        assert!(at(0.84).bolt > 0.9);
        // Back to normal at the end.
        let end = at(1.0);
        assert!(dist(end.left.pos, rest_hands(WeaponKind::PipeRifle).0.pos) < 1e-3);
    }

    #[test]
    fn shells_and_rounds_go_in_by_hand() {
        let sg = |p: f32| pose(&Inputs { reload: Some(p), ..with(WeaponKind::ScrapShotgun) });
        assert_eq!(sg(0.5).held, Held::Shells);
        assert!(!sg(0.45).chambers_loaded && sg(0.6).chambers_loaded);
        assert!(sg(0.2).open > 0.5);
        let rv = |p: f32| pose(&Inputs { reload: Some(p), ..with(WeaponKind::Revolver) });
        assert_eq!(rv(0.58).held, Held::Rounds);
        assert!(rv(0.3).open > 0.9);
    }

    #[test]
    fn reload_sounds_line_up_with_the_motion() {
        use crate::sim::sfx::cues;
        use crate::sim::synth::Sound;
        let at = |kind, p: f32| pose(&Inputs { reload: Some(p), ..with(kind) });
        for (t, s) in cues(WeaponKind::PipeRifle, false) {
            match s {
                Sound::ClunkOut => assert!(at(WeaponKind::PipeRifle, t + 0.03).mag_pos.is_some(), "mag released at {t}"),
                Sound::ClunkIn => assert!(at(WeaponKind::PipeRifle, t + 0.03).mag_pos.is_none(), "mag seated at {t}"),
                Sound::BoltRack => assert!(at(WeaponKind::PipeRifle, t + 0.03).bolt > 0.2, "bolt moving at {t}"),
                _ => {}
            }
        }
        for (t, s) in cues(WeaponKind::ScrapShotgun, false) {
            if s == Sound::BreakOpen {
                assert!(at(WeaponKind::ScrapShotgun, t + 0.02).open > 0.3);
            }
            if s == Sound::BreakClose {
                assert!(at(WeaponKind::ScrapShotgun, t).open < 0.7);
            }
        }
    }

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
        assert!(at(0.0).mag_pos.is_none());
        assert!(at(0.45).mag_pos.is_some(), "magazine is out mid-reload, in the hand");
        assert!(at(0.8).mag_pos.is_none(), "and seated again before the end");
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
        // The barrels drop on their hinge; the gun itself doesn't pitch down,
        // which would lift the stock into your face.
        assert!(at(0.4).rot[0] >= at(0.0).rot[0], "stock stays low while open");
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
