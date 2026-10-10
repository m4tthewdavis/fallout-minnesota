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

/// Anything with a magnitude below this is converted through an `i32` exactly.
const INT_RANGE: f32 = 2_147_483_648.0;

/// `x.fract()` (`x` minus its whole part, toward zero), without the call.
///
/// The standard `fract`, `floor` and `round` compile to a software routine on
/// the baseline x86-64 target (no SSE4.1 `roundss`), some fifteen instructions
/// each, and synthesised audio calls them per sample. Below 2^31 the whole
/// part is exactly what a float-to-int conversion gives, which is one
/// instruction. Every input gives the same bits as the standard function.
#[inline]
pub fn fract(x: f32) -> f32 {
    if x.abs() < INT_RANGE {
        // (+ 0.0 turns the -0.0 of "-1.0 - -1.0" style results into the +0.0 the standard gives.)
        x - (x as i32) as f32 + 0.0
    } else {
        x.fract()
    }
}

/// `x.floor()`, without the call (see [`fract`]).
#[inline]
pub fn floor(x: f32) -> f32 {
    if x.abs() < INT_RANGE {
        if x == 0.0 {
            return x; // keeps the sign of a negative zero
        }
        let t = (x as i32) as f32;
        if t > x {
            t - 1.0
        } else {
            t
        }
    } else {
        x.floor()
    }
}

/// `x.round() as u8` (round half away from zero, saturating, NaN to 0).
#[inline]
pub fn round_u8(x: f32) -> u8 {
    if x.is_nan() || x <= 0.0 {
        return 0;
    }
    if x >= 255.5 {
        return 255;
    }
    let whole = x as u32;
    (whole + ((x - whole as f32) >= 0.5) as u32) as u8
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

    fn awkward() -> Vec<f32> {
        let mut v = vec![0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5, 0.49999997, 0.5000001, 254.5, 255.5, 255.49998, 255.50002, 256.0, 1e9, -1e9, 2147483520.0, 2147483648.0, -2147483648.0, 4294967296.0, 1e30, -1e30, f32::MAX, f32::MIN, f32::MIN_POSITIVE, -f32::MIN_POSITIVE, 1e-45, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -f32::NAN, 8388607.5, 8388608.0, 16777216.0, 0.1, 0.9999999];
        let mut r = crate::sim::rng::Rng::new(3);
        for _ in 0..400_000 {
            let bits = r.next_u64() as u32;
            v.push(f32::from_bits(bits));
            v.push(r.range(-300.0, 300.0));
            v.push(r.range(0.0, 1.0));
            v.push((r.f32() * 600.0).round() * 0.5);
        }
        v
    }

    #[test]
    fn the_cheap_fract_floor_and_round_give_the_same_bits_as_the_standard_ones() {
        for x in awkward() {
            let same = |a: f32, b: f32| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
            assert!(same(fract(x), x.fract()), "fract({x:e}) = {} vs {}", fract(x), x.fract());
            assert!(same(floor(x), x.floor()), "floor({x:e}) = {} vs {}", floor(x), x.floor());
            assert_eq!(round_u8(x), x.round() as u8, "round_u8({x:e})");
        }
    }

    /// Every one of the 4.3 billion floats (`cargo test -- --ignored every_float`).
    #[test]
    #[ignore]
    fn every_float_gives_the_same_bits_as_the_standard_fract_floor_and_round() {
        std::thread::scope(|s| {
            for t in 0..4u64 {
                s.spawn(move || {
                    for bits in (t << 30)..((t + 1) << 30) {
                        let x = f32::from_bits(bits as u32);
                        let same = |a: f32, b: f32| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
                        assert!(same(fract(x), x.fract()), "fract bits {bits:#x}");
                        assert!(same(floor(x), x.floor()), "floor bits {bits:#x}");
                        assert_eq!(round_u8(x), x.round() as u8, "round_u8 bits {bits:#x}");
                    }
                });
            }
        });
    }
}
