//! What every enemy has in common: a [`Body`] (health, a hit sphere, a
//! flinch timer) so shooting and melee work on any creature, and a death
//! animation that tips the corpse onto its side before it sinks away.

use bevy::prelude::*;

/// Which creature this is (decides drops and messages).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Species {
    Wolf { alpha: bool },
    Moose,
}

impl Species {
    pub fn name(self) -> &'static str {
        match self {
            Species::Wolf { alpha: true } => "Frostfang alpha",
            Species::Wolf { alpha: false } => "Frostfang",
            Species::Moose => "Glowmoose",
        }
    }
}

/// Screenshot mode: this creature stands still and never acts.
#[derive(Component)]
pub struct Frozen;

/// Anything the player can shoot or hit.
#[derive(Component, Clone, Debug)]
pub struct Body {
    pub health: f32,
    pub max_health: f32,
    /// Height of the hit sphere's centre above the feet, before scaling.
    pub center_y: f32,
    /// Radius of the hit sphere, before scaling.
    pub radius: f32,
    pub species: Species,
    /// Counts down from 1 after a hit; drives the flinch animation.
    pub flinch: f32,
    /// Flat direction the last hit travelled, for the flinch.
    pub hit_dir: Vec3,
}

impl Body {
    pub fn new(species: Species, health: f32, center_y: f32, radius: f32) -> Self {
        Body {
            health,
            max_health: health,
            center_y,
            radius,
            species,
            flinch: 0.0,
            hit_dir: Vec3::ZERO,
        }
    }

    /// Centre of the hit sphere in the world.
    pub fn center(&self, tf: &Transform) -> Vec3 {
        tf.translation + Vec3::Y * self.center_y * tf.scale.y
    }

    pub fn hit_radius(&self, tf: &Transform) -> f32 {
        self.radius * tf.scale.x
    }

    /// Apply damage from a hit travelling along `dir`. Returns true if it killed.
    pub fn hurt(&mut self, damage: f32, dir: Vec3) -> bool {
        self.health -= damage;
        self.flinch = 1.0;
        self.hit_dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        self.health <= 0.0
    }

    pub fn health_fraction(&self) -> f32 {
        (self.health / self.max_health).clamp(0.0, 1.0)
    }
}

/// A corpse: rolls onto its side, lies there, then sinks into the snow.
#[derive(Component)]
pub struct Dying {
    pub t: f32,
    /// Where it fell and which way it was facing.
    base: Vec3,
    yaw: Quat,
    /// Which side it falls to (+1 or -1).
    side: f32,
    /// Hit-sphere centre height and thickness, for resting on the ground.
    center_y: f32,
    half_thickness: f32,
    scale: f32,
}

const FALL_SECS: f32 = 0.6;
const LIE_SECS: f32 = 10.0;
const SINK_SECS: f32 = 4.0;

impl Dying {
    pub fn new(tf: &Transform, body: &Body, side: f32) -> Self {
        Dying {
            t: 0.0,
            base: tf.translation,
            yaw: tf.rotation,
            side: if side >= 0.0 { 1.0 } else { -1.0 },
            center_y: body.center_y,
            half_thickness: body.radius * 0.3,
            scale: tf.scale.x,
        }
    }

    /// Roll angle after `t` seconds: ease-out with a small bounce at the end.
    pub fn roll_at(t: f32) -> f32 {
        let x = (t / FALL_SECS).clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - x).powi(3);
        let bounce = if x >= 1.0 {
            let b = ((t - FALL_SECS) / 0.25).clamp(0.0, 1.0);
            0.07 * (1.0 - b) * (b * std::f32::consts::PI * 2.0).sin().abs()
        } else {
            0.0
        };
        (ease + bounce) * std::f32::consts::FRAC_PI_2
    }

    /// Seconds until the corpse has fully gone.
    pub const fn total_secs() -> f32 {
        FALL_SECS + LIE_SECS + SINK_SECS
    }
}

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (animate_dying, fade_flinch));
    }
}

fn fade_flinch(time: Res<Time>, mut bodies: Query<&mut Body>) {
    for mut b in &mut bodies {
        if b.flinch > 0.0 {
            b.flinch = (b.flinch - time.delta_secs() * 3.5).max(0.0);
        }
    }
}

fn animate_dying(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Transform, &mut Dying)>) {
    for (entity, mut tf, mut d) in &mut q {
        d.t += time.delta_secs();
        if d.t >= Dying::total_secs() {
            commands.entity(entity).despawn();
            continue;
        }
        let roll = Dying::roll_at(d.t) * d.side;
        let rot = d.yaw * Quat::from_rotation_z(roll);
        // Keep the body resting on the ground rather than swinging about its feet.
        let centre_offset = Vec3::Y * d.center_y * d.scale;
        let lying = Vec3::Y * d.half_thickness * d.scale;
        let sink = ((d.t - FALL_SECS - LIE_SECS) / SINK_SECS).clamp(0.0, 1.0);
        let fall = roll.abs() / std::f32::consts::FRAC_PI_2;
        let rest = centre_offset + (lying - centre_offset) * fall;
        tf.rotation = rot;
        tf.translation = d.base + rest - rot * centre_offset - Vec3::Y * sink * 1.2 * d.scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Body {
        Body::new(Species::Wolf { alpha: false }, 60.0, 0.75, 0.8)
    }

    #[test]
    fn hits_hurt_and_eventually_kill() {
        let mut b = body();
        assert!(!b.hurt(25.0, Vec3::new(1.0, -0.2, 0.0)));
        assert_eq!(b.flinch, 1.0);
        assert_eq!(b.hit_dir, Vec3::X, "flinch direction is flat");
        assert!((b.health_fraction() - 35.0 / 60.0).abs() < 1e-5);
        assert!(!b.hurt(25.0, Vec3::X));
        assert!(b.hurt(25.0, Vec3::X), "third hit kills");
        assert_eq!(b.health_fraction(), 0.0);
    }

    #[test]
    fn hit_sphere_scales_with_the_creature() {
        let b = body();
        let tf = Transform::from_xyz(10.0, 2.0, 5.0).with_scale(Vec3::splat(1.3));
        assert!((b.center(&tf).y - (2.0 + 0.75 * 1.3)).abs() < 1e-5);
        assert!((b.hit_radius(&tf) - 0.8 * 1.3).abs() < 1e-5);
    }

    #[test]
    fn falling_rolls_to_a_quarter_turn_and_stays() {
        assert_eq!(Dying::roll_at(0.0), 0.0);
        let mid = Dying::roll_at(FALL_SECS * 0.5);
        assert!(mid > 0.5 && mid < std::f32::consts::FRAC_PI_2);
        let quarter = std::f32::consts::FRAC_PI_2;
        assert!((Dying::roll_at(FALL_SECS + 0.5) - quarter).abs() < 1e-5);
        assert!((Dying::roll_at(5.0) - quarter).abs() < 1e-5);
        // Monotonic while falling.
        let mut prev = 0.0;
        for k in 0..=20 {
            let r = Dying::roll_at(FALL_SECS * k as f32 / 20.0);
            assert!(r >= prev - 1e-6);
            prev = r;
        }
    }

    #[test]
    fn a_fallen_body_lies_on_the_ground_next_to_where_it_died() {
        let tf = Transform::from_xyz(3.0, 1.0, 4.0).with_scale(Vec3::splat(1.0));
        let b = body();
        let d = Dying::new(&tf, &b, 1.0);
        // Same maths as animate_dying at the end of the fall.
        let roll = Dying::roll_at(FALL_SECS + 1.0) * d.side;
        let rot = d.yaw * Quat::from_rotation_z(roll);
        let centre_offset = Vec3::Y * d.center_y * d.scale;
        let lying = Vec3::Y * d.half_thickness * d.scale;
        let pos = d.base + lying - rot * centre_offset;
        // The body's centre (pos + rot * centre_offset) is just above the ground at the death spot.
        let centre = pos + rot * centre_offset;
        assert!((centre.x - 3.0).abs() < 1e-4 && (centre.z - 4.0).abs() < 1e-4);
        assert!((centre.y - (1.0 + d.half_thickness)).abs() < 1e-4);
    }
}
