//! The Glowmoose.

use super::*;

// ---------------------------------------------------------------------------
// The Glowmoose
// ---------------------------------------------------------------------------

const MOOSE_FUR_TOP: [f32; 4] = [0.2, 0.15, 0.11, 1.0];
const MOOSE_FUR_BELLY: [f32; 4] = [0.36, 0.3, 0.24, 1.0];

/// Dark top, paler belly, snow dusted along the back.
fn shade_moose(m: &mut MeshData) {
    for i in 0..m.positions.len() {
        let (n, p) = (m.normals[i], m.positions[i]);
        let belly = smoothstep(0.2, -0.8, n[1]);
        let mut c = lerp4(MOOSE_FUR_TOP, MOOSE_FUR_BELLY, belly);
        // Pale "stockings" on the lower legs.
        if p[1] < 0.8 {
            c = lerp4(c, [0.5, 0.46, 0.4, 1.0], smoothstep(0.8, 0.3, p[1]) * 0.7);
        }
        let dust = smoothstep(0.65, 0.95, n[1]) * smoothstep(1.4, 1.8, p[1]) * (0.5 + 0.5 * wobble(3.0, p[0] * 5.0, p[2] * 4.0));
        c = lerp4(c, SNOW, dust.clamp(0.0, 0.8));
        m.colors[i] = c;
    }
}

/// Barrel body with a shoulder hump, facing +Z, feet at y = 0 (shoulder ~1.7 m).
pub fn moose_body() -> MeshData {
    let s = 2.4;
    let mut m = loft_z(
        &[
            sec(-1.2, 1.5, 0.28, 0.36, s),
            sec(-0.98, 1.52, 0.44, 0.52, s),
            sec(-0.4, 1.5, 0.5, 0.58, s),
            sec(0.2, 1.56, 0.52, 0.64, s),
            sec(0.7, 1.68, 0.47, 0.67, s),
            sec(1.05, 1.72, 0.36, 0.52, s),
            sec(1.2, 1.74, 0.24, 0.34, s),
        ],
        18,
        1.0,
    );
    shade_moose(&mut m);
    m
}

/// Neck, long head, bulbous nose and the hanging "bell" under the chin.
pub fn moose_head() -> MeshData {
    let s = 2.3;
    let mut m = loft_z(
        &[
            sec(1.0, 1.72, 0.3, 0.4, s),
            sec(1.35, 1.86, 0.21, 0.3, s),
            sec(1.65, 1.82, 0.18, 0.25, s),
            sec(2.0, 1.72, 0.19, 0.25, s),
            sec(2.4, 1.56, 0.13, 0.18, s),
            sec(2.62, 1.49, 0.16, 0.2, s),
            sec(2.72, 1.48, 0.06, 0.09, s),
        ],
        14,
        1.0,
    );
    m.append(&blob(0.12, 1.6, 0.1, 17, 1.0).scaled([0.7, 1.0, 0.7]).translated([0.0, 1.4, 1.95]));
    for x in [-0.13f32, 0.13] {
        m.append(&lathe(&[(0.05, 0.0), (0.03, 0.09), (0.0, 0.2)], 6, 1.0, false, false).scaled([1.0, 1.0, 0.5]).rotated_z(-x.signum() * 0.9).translated([x, 1.93, 1.8]));
    }
    shade_moose(&mut m);
    m
}

/// A long hanging leg from the hip pivot (y = 0) down to the hoof (y = -1.1).
pub fn moose_leg() -> MeshData {
    let mut m = lathe(
        &[(0.06, -1.1), (0.05, -1.0), (0.045, -0.9), (0.055, -0.5), (0.1, -0.2), (0.15, 0.0), (0.12, 0.08)],
        10,
        1.0,
        true,
        true,
    );
    // Dark split hooves.
    for i in 0..m.positions.len() {
        if m.positions[i][1] < -1.0 {
            m.colors[i] = [0.05, 0.04, 0.04, 1.0];
        } else {
            m.colors[i] = lerp4(MOOSE_FUR_TOP, [0.5, 0.46, 0.4, 1.0], smoothstep(-0.2, -0.9, m.positions[i][1]) * 0.8);
        }
    }
    m
}

/// One palmate antler for `side` (-1 left, +1 right): a beam, a broad palm
/// and five tines. Returns the mesh and the points where the tines end, which
/// get glowing tips.
pub fn moose_antler(side: f32) -> (MeshData, Vec<V3>) {
    let mut m = MeshData::default();
    let beam = [[side * 0.1, 1.95, 1.85], [side * 0.5, 2.12, 1.82], [side * 0.85, 2.28, 1.86]];
    m.append(&tube(&beam, 0.045, 7));
    // The palm: a flattened, slightly cupped plate.
    let palm = blob(1.0, 0.05, 0.1, 29, 1.0).scaled([0.5, 0.05, 0.42]).rotated_z(side * 0.25).translated([side * 1.1, 2.38, 1.88]);
    m.append(&palm);
    let mut tips = Vec::new();
    for k in 0..5 {
        let f = k as f32 / 4.0;
        let start = [side * (0.85 + 0.55 * f), 2.35 + 0.02 * k as f32, 1.5 + 0.38 * (1.0 - (2.0 * f - 1.0).abs()) * 0.0 + 0.19 * k as f32];
        let end = [start[0] + side * (0.12 + 0.1 * f), start[1] + 0.35 + 0.12 * (1.0 - f), start[2] + 0.28 + 0.05 * k as f32];
        m.append(&tube(&[start, end], 0.03, 6));
        tips.push(end);
    }
    // A brow tine pointing forward from the base.
    let brow_end = [side * 0.35, 2.25, 2.25];
    m.append(&tube(&[[side * 0.3, 2.08, 1.9], brow_end], 0.035, 6));
    tips.push(brow_end);
    for i in 0..m.colors.len() {
        m.colors[i] = [0.82, 0.76, 0.64, 1.0];
    }
    (m, tips)
}
