//! Small maths helpers shared by the sim modules.

/// 0 -> 1 as `x` goes from `e0` to `e1`, easing at both ends (works with
/// the edges reversed, giving 1 -> 0).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_eases_between_its_edges_in_either_order() {
        assert_eq!(smoothstep(0.0, 2.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 2.0, 5.0), 1.0);
        assert!((smoothstep(0.0, 2.0, 1.0) - 0.5).abs() < 1e-6);
        assert!(smoothstep(0.0, 2.0, 0.5) < 0.25, "slow start");
        assert_eq!(smoothstep(10.0, 4.0, 12.0), 0.0);
        assert_eq!(smoothstep(10.0, 4.0, 2.0), 1.0);
    }

    #[test]
    fn lerp_hits_both_ends() {
        assert_eq!(lerp(2.0, 6.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 6.0, 1.0), 6.0);
        assert_eq!(lerp(2.0, 6.0, 0.25), 3.0);
    }
}
