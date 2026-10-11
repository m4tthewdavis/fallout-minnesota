//! The Frostfang wolf: body, head, legs and tail in four looks.

use super::*;

/// How many distinct Frostfang looks there are.
pub const WOLF_VARIANTS: u32 = 4;

/// Frostfang wolf body (torso + neck), facing +Z, feet at y = 0. Variant 0 is
/// plain; 1 is scarred; 2 has a dark saddle and mask; 3 is mangy and patchy.
pub fn wolf_body_variant(variant: u32) -> MeshData {
    let s = 2.3;
    let mut m = loft_z(
        &[
            sec(-0.74, 0.86, 0.06, 0.07, s),
            sec(-0.66, 0.85, 0.17, 0.2, s),
            sec(-0.5, 0.83, 0.23, 0.26, s),
            sec(-0.3, 0.8, 0.24, 0.26, s),
            sec(-0.05, 0.78, 0.21, 0.24, s),
            sec(0.2, 0.8, 0.24, 0.3, s),
            sec(0.42, 0.86, 0.26, 0.32, s),
            sec(0.6, 0.95, 0.2, 0.24, s),
            sec(0.74, 1.04, 0.14, 0.16, s),
            sec(0.8, 1.07, 0.08, 0.09, s),
        ],
        16,
        1.0,
    );
    shade_fur(&mut m);
    wolf_pattern(&mut m, variant);
    m
}

/// Wolf head with muzzle and ears, facing +Z. The variant changes the ears
/// (1: one ear torn short, 2: tall and alert, 3: ragged and tilted) and the
/// markings.
pub fn wolf_head_variant(variant: u32) -> MeshData {
    let s = 2.2;
    let mut m = loft_z(
        &[
            sec(0.66, 1.07, 0.08, 0.09, s),
            sec(0.74, 1.08, 0.15, 0.15, s),
            sec(0.86, 1.08, 0.16, 0.15, s),
            sec(0.97, 1.05, 0.13, 0.12, s),
            sec(1.06, 1.0, 0.085, 0.085, s),
            sec(1.18, 0.98, 0.065, 0.065, s),
            sec(1.28, 0.97, 0.045, 0.05, s),
            sec(1.31, 0.97, 0.02, 0.025, s),
        ],
        14,
        1.0,
    );
    // (height scale, outward lean) for the left and right ear.
    let ears: [(f32, f32); 2] = match variant % WOLF_VARIANTS {
        1 => [(0.45, 0.25), (1.0, 0.25)],
        2 => [(1.35, 0.1), (1.35, 0.1)],
        3 => [(0.9, 0.7), (0.75, -0.2)],
        _ => [(1.0, 0.25), (1.0, 0.25)],
    };
    for (x, (h, lean)) in [-0.085f32, 0.085].into_iter().zip(ears) {
        let ear = lathe(&[(0.055, 0.0), (0.035, 0.07 * h), (0.0, 0.14 * h)], 6, 1.0, false, false)
            .scaled([1.0, 1.0, 0.55])
            .rotated_z(-x.signum() * lean)
            .translated([x, 1.18, 0.8]);
        m.append(&ear);
    }
    shade_fur(&mut m);
    wolf_pattern(&mut m, variant);
    m
}

/// Tapered leg hanging from its pivot (y = 0) down to the paw (y = -0.62).
pub fn wolf_leg() -> MeshData {
    let mut m = lathe(
        &[(0.06, -0.62), (0.055, -0.6), (0.04, -0.52), (0.045, -0.4), (0.07, -0.22), (0.095, -0.05), (0.07, 0.03)],
        8,
        1.0,
        true,
        true,
    );
    shade_fur(&mut m);
    m
}

/// Bushy tail from its pivot back along -Z, drooping.
pub fn wolf_tail() -> MeshData {
    let s = 2.0;
    let mut m = loft_z(
        &[
            sec(-0.62, -0.26, 0.015, 0.015, s),
            sec(-0.55, -0.22, 0.07, 0.07, s),
            sec(-0.4, -0.14, 0.1, 0.1, s),
            sec(-0.2, -0.05, 0.085, 0.085, s),
            sec(0.0, 0.0, 0.05, 0.05, s),
        ],
        8,
        1.0,
    );
    shade_fur(&mut m);
    m
}

/// Countershading: darker saddle along the back, pale belly.
fn shade_fur(m: &mut MeshData) {
    for i in 0..m.positions.len() {
        let ny = m.normals[i][1];
        let saddle = smoothstep(0.4, 0.95, ny);
        let belly = smoothstep(-0.3, -0.9, ny);
        let g = 0.9 - 0.35 * saddle + 0.08 * belly;
        m.colors[i] = [g * 0.95, g * 0.98, g, 1.0];
    }
}

/// Markings on top of the basic countershading.
fn wolf_pattern(m: &mut MeshData, variant: u32) {
    let v = variant % WOLF_VARIANTS;
    if v == 0 {
        return;
    }
    for i in 0..m.positions.len() {
        let p = m.positions[i];
        let n = m.normals[i];
        let c = &mut m.colors[i];
        match v {
            1 => {
                // Pale scar streaks across the flank and a raw patch on the shoulder.
                if (p[2] * 9.0 + p[1] * 6.0).sin() > 0.94 && p[0].abs() > 0.1 {
                    *c = lerp4(*c, [1.0, 0.82, 0.8, 1.0], 0.85);
                }
                if p[0].abs() > 0.15 && (p[2] - 0.3).abs() < 0.1 && (p[1] - 0.85).abs() < 0.1 {
                    *c = lerp4(*c, [0.85, 0.55, 0.55, 1.0], 0.7);
                }
            }
            2 => {
                // A dark saddle, darker legs-and-mask.
                let saddle = smoothstep(0.3, 0.9, n[1]);
                let f = 1.0 - 0.38 * saddle;
                *c = [c[0] * f, c[1] * f, c[2] * f, 1.0];
                if p[2] > 0.95 && p[1] < 1.07 {
                    *c = [c[0] * 0.55, c[1] * 0.55, c[2] * 0.58, 1.0];
                }
            }
            _ => {
                // Mange: pale, pinkish bald patches and a duller coat.
                let patch = wobble(7.0, p[0] * 6.0 + p[2] * 4.0, p[1] * 7.0);
                if patch < -0.25 {
                    *c = lerp4(*c, [0.95, 0.78, 0.74, 1.0], 0.75);
                } else {
                    *c = [c[0] * 0.82, c[1] * 0.82, c[2] * 0.84, 1.0];
                }
            }
        }
    }
}
