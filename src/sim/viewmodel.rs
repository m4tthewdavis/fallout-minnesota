//! First-person gun pose: where the pipe rifle sits on screen and how its
//! moving parts sit, given what the player is doing. Pure maths so the
//! animation can be unit-tested; `combat.rs` applies it to the model.

use std::f32::consts::PI;

/// Hip-fire resting position, camera space (metres; -Z is forward).
pub const HIP: [f32; 3] = [0.19, -0.2, -0.42];
/// Gun-space height of the sight line (rear peep and front post).
pub const SIGHT_Y: f32 = 0.075;
/// Scale the gun model is drawn at.
pub const SCALE: f32 = 0.8;
/// Aim-down-sights position: sights centred on the crosshair.
pub const ADS: [f32; 3] = [0.0, -SIGHT_Y * SCALE, -0.19];

/// Everything that drives the pose this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
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
    /// 0 = hip, 1 = fully aimed down the sights.
    pub ads: f32,
    /// 0 = clear, 1 = muzzle pressed against a wall or tree.
    pub blocked: f32,
    /// Smoothed mouse-look velocity (yaw, pitch), radians per second.
    pub look: [f32; 2],
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
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 -> 1 over [a, b].
fn ramp(t: f32, a: f32, b: f32) -> f32 {
    smooth((t - a) / (b - a))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn pose(i: &Inputs) -> Pose {
    let ads = smooth(i.ads);
    let hip_w = 1.0 - ads;
    let mut pos = [lerp(HIP[0], ADS[0], ads), lerp(HIP[1], ADS[1], ads), lerp(HIP[2], ADS[2], ads)];
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
    // Sprinting tilts the rifle across the chest.
    if i.sprinting && i.moving {
        let s = hip_w;
        pos[0] -= 0.04 * s;
        pos[1] -= 0.05 * s;
        rot[0] -= 0.25 * s;
        rot[1] += 0.55 * s;
        rot[2] += 0.2 * s;
    }

    // The gun lags behind fast mouse movement.
    let lag = 0.3 + 0.7 * hip_w;
    rot[1] += (i.look[0] * 0.012).clamp(-0.08, 0.08) * lag;
    rot[0] += (i.look[1] * 0.012).clamp(-0.08, 0.08) * lag;
    pos[0] -= (i.look[0] * 0.002).clamp(-0.015, 0.015) * lag;

    // Recoil: kick back and muzzle climb.
    let r = i.recoil.clamp(0.0, 1.0);
    pos[2] += r * lerp(0.07, 0.04, ads);
    rot[0] += r * lerp(0.13, 0.05, ads);

    // Muzzle against a wall: tip the gun down and pull it in.
    let b = smooth(i.blocked);
    pos[1] -= 0.08 * b;
    pos[2] += 0.06 * b;
    rot[0] -= 0.7 * b;

    let mut mag_drop = 0.0;
    let mut bolt = r;
    // Hammer falls with the shot and is re-cocked as the bolt cycles.
    let mut hammer = if r > 0.5 { (r - 0.5) * 2.0 } else { 0.0 };
    let trigger = if r > 0.6 { 1.0 } else { r / 0.6 };

    if let Some(p) = i.reload {
        let p = p.clamp(0.0, 1.0);
        // Bring the rifle down and roll it to show the action.
        let lowered = ramp(p, 0.0, 0.15) * (1.0 - ramp(p, 0.85, 1.0));
        pos[1] -= 0.06 * lowered;
        pos[0] -= 0.03 * lowered;
        rot[2] += 0.55 * lowered;
        rot[0] += 0.12 * lowered;
        if i.unjamming {
            // Rack the bolt hard, twice.
            bolt = ((p * 2.0 * PI * 2.0).sin() * 0.5 + 0.5) * lowered;
            rot[2] += (p * 40.0).sin() * 0.02 * lowered;
        } else {
            // Mag out (0.15-0.35), away (0.35-0.55), back in (0.55-0.75),
            // then work the bolt (0.75-0.9).
            mag_drop = 0.3 * ramp(p, 0.15, 0.35) * (1.0 - ramp(p, 0.55, 0.75));
            let rack = ramp(p, 0.75, 0.82) * (1.0 - ramp(p, 0.84, 0.9));
            bolt = bolt.max(rack);
            hammer = 0.0;
        }
    }

    Pose {
        pos,
        rot,
        mag_drop,
        bolt,
        hammer,
        trigger,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn hip_and_ads_positions() {
        let hip = pose(&Inputs::default());
        assert!(close(hip.pos[0], HIP[0], 0.005));
        let aimed = pose(&Inputs { ads: 1.0, ..Default::default() });
        // Sights on the crosshair: centred, with the sight line on the axis.
        assert!(aimed.pos[0].abs() < 0.003);
        assert!(close(aimed.pos[1] + SIGHT_Y * SCALE, 0.0, 0.003));
    }

    #[test]
    fn reload_drops_the_magazine_and_brings_it_back() {
        let at = |p: f32| {
            pose(&Inputs {
                reload: Some(p),
                ..Default::default()
            })
        };
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
    fn walls_lower_the_gun_and_sprinting_carries_it() {
        let rest = pose(&Inputs::default());
        let blocked = pose(&Inputs { blocked: 1.0, ..Default::default() });
        assert!(blocked.pos[1] < rest.pos[1] && blocked.rot[0] < rest.rot[0] - 0.5);
        let sprint = pose(&Inputs {
            moving: true,
            sprinting: true,
            ..Default::default()
        });
        assert!(sprint.rot[1] > 0.4, "rifle swings across the chest");
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
