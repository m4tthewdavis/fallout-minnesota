//! The snowmobile.

use super::*;

pub const SKI_X: f32 = 0.48;
pub const SKI_Z: f32 = 0.78;
pub const TRACK_HALF_W: f32 = 0.19;

/// Where a snowmobile touches the snow: both skis and the track's corners.
pub fn snowmobile_contacts() -> [(f32, f32); 6] {
    [(-SKI_X, SKI_Z), (SKI_X, SKI_Z), (-TRACK_HALF_W, -0.25), (TRACK_HALF_W, -0.25), (-TRACK_HALF_W, -1.42), (TRACK_HALF_W, -1.42)]
}

/// Painted body: the belly pan and nose, the cowl, the tunnel over the track
/// and the side panels. Vertex colours carry a dark lower stripe.
pub fn snowmobile_body() -> MeshData {
    use meshgen::sec;
    let mut hull = meshgen::loft_z(
        &[
            sec(-0.2, 0.48, 0.3, 0.18, 4.0),
            sec(0.0, 0.52, 0.44, 0.24, 4.0),
            sec(0.45, 0.56, 0.47, 0.28, 4.0),
            sec(0.85, 0.52, 0.44, 0.25, 4.0),
            sec(1.15, 0.46, 0.33, 0.18, 3.5),
            sec(1.32, 0.42, 0.16, 0.1, 3.0),
            sec(1.38, 0.41, 0.04, 0.04, 3.0),
        ],
        20,
        1.0,
    );
    let cowl = meshgen::loft_z(
        &[
            sec(0.02, 0.8, 0.38, 0.1, 3.0),
            sec(0.3, 0.84, 0.42, 0.14, 3.0),
            sec(0.7, 0.76, 0.4, 0.14, 3.0),
            sec(1.0, 0.64, 0.3, 0.1, 2.6),
            sec(1.18, 0.56, 0.16, 0.05, 2.4),
        ],
        18,
        1.0,
    );
    hull.append(&cowl);
    // Tunnel: a deck over the track with side plates, and running boards.
    hull.append(&meshgen::cuboid([0.54, 0.04, 1.6], 1.0).translated([0.0, 0.64, -0.82]));
    let plate = extrude_profile(&[(0.0, 0.3), (-1.42, 0.3), (-1.6, 0.44), (-1.6, 0.66), (0.0, 0.66)], 0.014, 0.006, 0.0, 1.0);
    hull.append(&with_mirror(plate.translated([0.26, 0.0, 0.0])));
    hull.colors.fill(WHITE);
    for (p, c) in hull.positions.iter().zip(hull.colors.iter_mut()) {
        if p[1] < 0.4 || (p[1] < 0.5 && p[2] > -0.1) {
            *c = [0.12, 0.12, 0.13, 1.0];
        }
    }
    hull
}

/// Black parts: seat, running boards, handlebar grips, snow flap.
pub fn snowmobile_black() -> MeshData {
    let mut m = extrude_profile(
        &[(0.06, 0.64), (-1.28, 0.64), (-1.34, 0.74), (-1.26, 0.83), (-0.2, 0.87), (0.02, 0.86), (0.1, 0.76)],
        0.23,
        0.07,
        0.0,
        0.8,
    );
    for s in [-1.0f32, 1.0] {
        m.append(&meshgen::cuboid([0.16, 0.025, 0.95], 1.0).translated([s * 0.36, 0.36, -0.48]));
        m.append(&tube(&[[s * 0.33, 1.07, -0.02], [s * 0.42, 1.09, -0.05]], 0.022, 8));
    }
    m.append(&meshgen::cuboid([0.44, 0.28, 0.012], 1.0).translated([0.0, 0.32, -1.64]));
    m
}

/// Steel: skis, suspension arms, shocks with springs, spindles, handlebars,
/// the grab bar and the track's wheels and rails.
pub fn snowmobile_steel() -> MeshData {
    let mut m = MeshData::default();
    // Skis with an upturned tip, a carbide keel and a loop handle.
    let ski = extrude_profile(
        &[(0.25, 0.0), (1.2, 0.0), (1.36, 0.04), (1.46, 0.12), (1.51, 0.24), (1.48, 0.29), (1.43, 0.19), (1.33, 0.09), (1.2, 0.045), (0.3, 0.045), (0.22, 0.035)],
        0.075,
        0.012,
        0.0,
        1.0,
    );
    let loop_handle = tube(&[[0.0, 0.05, 1.18], [0.0, 0.2, 1.3], [0.0, 0.27, 1.45]], 0.012, 5);
    let saddle = meshgen::cuboid([0.06, 0.08, 0.2], 1.0).translated([0.0, 0.08, SKI_Z]);
    let spindle = tube(&[[0.0, 0.1, SKI_Z], [0.0, 0.46, SKI_Z]], 0.03, 8);
    let mut corner = ski;
    corner.append(&loop_handle);
    corner.append(&saddle);
    corner.append(&spindle);
    let sx = SKI_X;
    let mut corner = corner.translated([sx, 0.0, 0.0]);
    // Upper and lower A-arms from the belly pan out to the spindle.
    for y in [0.3f32, 0.42] {
        corner.append(&tube(&[[0.24, y + 0.02, SKI_Z - 0.22], [sx - 0.02, y, SKI_Z]], 0.016, 6));
        corner.append(&tube(&[[0.24, y + 0.02, SKI_Z + 0.2], [sx - 0.02, y, SKI_Z]], 0.016, 6));
    }
    // Shock absorber with a coil spring round it.
    let (bot, top) = ([sx - 0.03, 0.44, SKI_Z], [0.26, 0.74, SKI_Z - 0.05]);
    corner.append(&tube(&[bot, top], 0.018, 6));
    let axis = sub(top, bot);
    let len = meshgen::length(axis);
    let dir = normalize(axis);
    let helper = if dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let u = normalize(cross(dir, helper));
    let v = cross(u, dir);
    let coil: Vec<V3> = (0..=64)
        .map(|k| {
            let t = k as f32 / 64.0;
            let a = t * 7.0 * TAU;
            add(add(bot, scale(dir, 0.04 + t * (len - 0.1))), add(scale(u, 0.045 * a.cos()), scale(v, 0.045 * a.sin())))
        })
        .collect();
    corner.append(&tube(&coil, 0.007, 4));
    m.append(&with_mirror(corner));
    // Handlebar on its riser, with brake and throttle levers.
    m.append(&tube(&[[0.0, 0.86, 0.16], [0.0, 1.0, 0.06]], 0.025, 6));
    m.append(&tube(&[[-0.34, 1.07, -0.02], [-0.18, 1.03, 0.03], [0.18, 1.03, 0.03], [0.34, 1.07, -0.02]], 0.016, 6));
    m.append(&tube(&[[-0.24, 1.05, 0.02], [-0.32, 1.03, 0.1]], 0.008, 4));
    m.append(&tube(&[[0.24, 1.05, 0.02], [0.3, 1.02, 0.08]], 0.008, 4));
    // Grab bar round the back.
    m.append(&tube(&[[-0.26, 0.62, -1.45], [-0.27, 0.66, -1.68], [0.27, 0.66, -1.68], [0.26, 0.62, -1.45]], 0.02, 6));
    // Track wheels: big rear idlers, front drive wheels and small bogies.
    for x in [-0.12f32, 0.12] {
        for (z, y, r) in [(-1.42f32, 0.17f32, 0.15f32), (0.04, 0.3, 0.1), (-0.45, 0.075, 0.065), (-0.8, 0.075, 0.065), (-1.12, 0.075, 0.065)] {
            let wheel = lathe(&[(0.0, -0.03), (r, -0.03), (r, 0.03), (0.0, 0.03)], 14, 1.0, false, false).rotated_z(-FRAC_PI_2);
            m.append(&wheel.translated([x, y, z]));
        }
        m.append(&meshgen::cuboid([0.03, 0.035, 1.3], 1.0).translated([x, 0.06, -0.8]));
    }
    m
}

/// The rubber track: a belt loop with raised lugs, around the wheels.
pub fn snowmobile_track() -> MeshData {
    let loop_pts: [P2; 15] = [
        (-1.42, 0.0),
        (-0.2, 0.0),
        (-0.02, 0.05),
        (0.1, 0.16),
        (0.16, 0.28),
        (0.14, 0.37),
        (0.05, 0.42),
        (-0.6, 0.41),
        (-1.3, 0.35),
        (-1.42, 0.335),
        (-1.53, 0.3),
        (-1.585, 0.17),
        (-1.55, 0.06),
        (-1.49, 0.015),
        (-1.42, 0.0),
    ];
    // Resample the loop evenly.
    let mut pts: Vec<P2> = Vec::new();
    for w in loop_pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let steps = (len / 0.06).ceil().max(1.0) as usize;
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            pts.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    let n = pts.len();
    let ccw = signed_area(&pts) > 0.0;
    let hw = TRACK_HALF_W;
    let thick = 0.025;
    let mut m = MeshData::default();
    // Outer and inner surfaces as two strips; edges close them.
    let outward = |i: usize| -> P2 {
        let a = pts[(i + n - 1) % n];
        let b = pts[(i + 1) % n];
        let d = norm2((b.0 - a.0, b.1 - a.1));
        // Right of travel is outside for a counter-clockwise loop.
        if ccw { (d.1, -d.0) } else { (-d.1, d.0) }
    };
    let mut rows: Vec<[u32; 4]> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let k = i % n;
        let (z, y) = pts[k];
        let o = outward(k);
        let outer = (z + o.0 * thick * 0.5, y + o.1 * thick * 0.5);
        let inner = (z - o.0 * thick * 0.5, y - o.1 * thick * 0.5);
        let on: V3 = [0.0, o.1, o.0];
        let inn: V3 = [0.0, -o.1, -o.0];
        let v = i as f32 * 0.06 / 0.5;
        rows.push([
            m.vertex([-hw, outer.1, outer.0], on, [0.0, v], WHITE),
            m.vertex([hw, outer.1, outer.0], on, [1.0, v], WHITE),
            m.vertex([hw, inner.1, inner.0], inn, [1.0, v], WHITE),
            m.vertex([-hw, inner.1, inner.0], inn, [0.0, v], WHITE),
        ]);
    }
    for i in 0..n {
        let (a, b) = (rows[i], rows[i + 1]);
        let o = outward(i);
        let on: V3 = [0.0, o.1, o.0];
        quad_facing(&mut m, a[0], a[1], b[1], b[0], on);
        quad_facing(&mut m, a[3], a[2], b[2], b[3], scale(on, -1.0));
        quad_facing(&mut m, a[1], a[2], b[2], b[1], [1.0, 0.0, 0.0]);
        quad_facing(&mut m, a[0], a[3], b[3], b[0], [-1.0, 0.0, 0.0]);
    }
    // Lugs: raised bars across the belt every other sample.
    for i in (0..n).step_by(2) {
        let (z, y) = pts[i];
        let o = outward(i);
        let angle = o.1.atan2(o.0);
        let lug = meshgen::cuboid([hw * 1.9, 0.03, 0.022], 1.0)
            // Stand the lug up along the outward direction (its local +Y).
            .rotated_x(FRAC_PI_2 - angle)
            .translated([0.0, y + o.1 * (thick * 0.5 + 0.012), z + o.0 * (thick * 0.5 + 0.012)]);
        m.append(&lug);
    }
    m
}

/// Windshield: a curved, swept-back sheet above the cowl.
pub fn snowmobile_windshield() -> MeshData {
    let mut m = MeshData::default();
    let cols = 8;
    for row in 0..=1 {
        let (y, z, hw) = if row == 0 { (0.88, 0.24, 0.36) } else { (1.24, 0.02, 0.28) };
        for k in 0..=cols {
            let f = k as f32 / cols as f32 * 2.0 - 1.0;
            m.vertex([f * hw, y - 0.04 * f * f, z - 0.08 * f * f], [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
        }
    }
    let r = cols as u32 + 1;
    for k in 0..cols as u32 {
        m.quad(k, k + 1, r + k + 1, r + k);
    }
    m.recompute_normals();
    m
}

/// Headlight lens and tail lamp positions: (centre, facing +Z?).
pub fn snowmobile_lenses() -> (MeshData, MeshData) {
    let head = lathe(&[(0.08, 0.0), (0.07, 0.02), (0.0, 0.03)], 12, 1.0, false, false).rotated_x(1.2).translated([0.0, 0.72, 0.95]);
    let tail = meshgen::cuboid([0.2, 0.05, 0.02], 1.0).translated([0.0, 0.6, -1.61]);
    (head, tail)
}
