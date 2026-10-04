//! Abandoned vehicles built from code: a rounded 1950s sedan (body with
//! wheel arches, roof and pillars, glass, chrome, interior, tyres) and a
//! snowmobile (hull, cowl, seat, tunnel, a lugged track on its wheels, skis
//! on proper suspension), plus [`settle`], which sets a vehicle down on
//! uneven ground so every wheel or ski touches the snow.
//!
//! Same conventions as [`super::meshgen`]: Y up, front is +Z, ground at y = 0
//! under the wheels, counter-clockwise front faces.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::meshgen::{self, add, cross, dot, lathe, normalize, scale, sub, tube, MeshData, V3, WHITE};
use super::rng::Rng;

// ---------------------------------------------------------------------------
// 2D helpers
// ---------------------------------------------------------------------------

type P2 = (f32, f32);

fn signed_area(poly: &[P2]) -> f32 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f32>()
        * 0.5
}

fn cross2(a: P2, b: P2, c: P2) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn in_triangle(p: P2, a: P2, b: P2, c: P2) -> bool {
    // Points on an edge count as inside (a diagonal through another corner
    // is not a valid ear), but the triangle's own corners do not.
    let same = |q: P2| (q.0 - p.0).abs() < 1e-7 && (q.1 - p.1).abs() < 1e-7;
    if same(a) || same(b) || same(c) {
        return false;
    }
    let e = -1e-7;
    cross2(a, b, p) >= e && cross2(b, c, p) >= e && cross2(c, a, p) >= e
}

/// Ear-clipping triangulation of a simple polygon (either winding). The
/// triangles come back counter-clockwise.
pub fn triangulate(poly: &[P2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::with_capacity(n.saturating_sub(2));
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&k| {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            cross2(a, b, c) > 1e-9 && !idx.iter().any(|&j| j != ia && j != ib && j != ic && in_triangle(poly[j], a, b, c))
        });
        // A degenerate outline: clip the flattest corner anyway so we finish.
        let k = ear.unwrap_or(0);
        out.push([idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]]);
        idx.remove(k);
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

fn norm2(v: P2) -> P2 {
    let l = (v.0 * v.0 + v.1 * v.1).sqrt().max(1e-9);
    (v.0 / l, v.1 / l)
}

/// Add a quad, flipping it if needed so it faces `want`.
fn quad_facing(m: &mut MeshData, a: u32, b: u32, c: u32, d: u32, want: V3) {
    let (pa, pb, pc) = (m.positions[a as usize], m.positions[b as usize], m.positions[c as usize]);
    let mut n = cross(sub(pb, pa), sub(pc, pa));
    if dot(n, n) < 1e-14 {
        let pd = m.positions[d as usize];
        n = cross(sub(pc, pa), sub(pd, pa));
    }
    if dot(n, want) >= 0.0 {
        m.quad(a, b, c, d);
    } else {
        m.quad(a, d, c, b);
    }
}

/// A slab whose side view is `outline` ((z, y) points, either winding),
/// `2 * half_width` wide across X, with rounded edges `bevel` wide. `taper`
/// narrows the top: the width shrinks by that fraction from the lowest point
/// of the outline to the highest (a car cabin's tumblehome).
pub fn extrude_profile(outline: &[P2], half_width: f32, bevel: f32, taper: f32, uv_scale: f32) -> MeshData {
    let mut p: Vec<P2> = outline.to_vec();
    if signed_area(&p) < 0.0 {
        p.reverse();
    }
    let n = p.len();
    // Inward normal of edge i (p[i] -> p[i+1]) for a counter-clockwise outline.
    let inward: Vec<P2> = (0..n)
        .map(|i| {
            let d = norm2((p[(i + 1) % n].0 - p[i].0, p[(i + 1) % n].1 - p[i].1));
            (-d.1, d.0)
        })
        .collect();
    let vert_in: Vec<P2> = (0..n).map(|i| norm2((inward[(i + n - 1) % n].0 + inward[i].0, inward[(i + n - 1) % n].1 + inward[i].1))).collect();
    let inset: Vec<P2> = (0..n)
        .map(|i| {
            let m = vert_in[i];
            let len = bevel / (m.0 * inward[i].0 + m.1 * inward[i].1).max(0.5);
            (p[i].0 + m.0 * len, p[i].1 + m.1 * len)
        })
        .collect();
    let (vmin, vmax) = p.iter().fold((f32::MAX, f32::MIN), |(lo, hi), q| (lo.min(q.1), hi.max(q.1)));
    let width_at = |v: f32| 1.0 - taper * ((v - vmin) / (vmax - vmin).max(1e-6));
    let pt = |q: P2, x: f32| -> V3 { [x * width_at(q.1), q.1, q.0] };
    let mut m = MeshData::default();
    let hw = half_width;
    let inner = hw - bevel;

    // Flat sides.
    for side in [1.0f32, -1.0] {
        let normal = [side, 0.0, 0.0];
        let start = m.positions.len() as u32;
        for &q in &inset {
            m.vertex(pt(q, side * hw), normal, [side * q.0 / uv_scale, -q.1 / uv_scale], WHITE);
        }
        for t in triangulate(&inset) {
            // Counter-clockwise in (z, y) faces -X once mapped to (x, y, z).
            let (a, b, c) = (start + t[0] as u32, start + t[1] as u32, start + t[2] as u32);
            if side > 0.0 {
                m.tri(a, c, b);
            } else {
                m.tri(a, b, c);
            }
        }
    }

    // Edges all the way round: bevel, band, bevel. Gentle corners share a
    // smoothed normal; sharp ones (more than ~40 degrees) keep a crease.
    let outward = |i: usize| -> P2 { (-inward[i].0, -inward[i].1) };
    let corner_normal = |i: usize, edge: usize| -> P2 {
        let prev = (i + n - 1) % n;
        let (a, b) = (outward(prev), outward(i));
        if a.0 * b.0 + a.1 * b.1 > 0.76 {
            norm2((a.0 + b.0, a.1 + b.1))
        } else {
            outward(edge)
        }
    };
    let mut along = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let seg = ((p[j].0 - p[i].0).powi(2) + (p[j].1 - p[i].1).powi(2)).sqrt();
        let (u0, u1) = (along / uv_scale, (along + seg) / uv_scale);
        along += seg;
        let (ni, nj) = (corner_normal(i, i), corner_normal(j, i));
        let out3 = |o: P2| -> V3 { [0.0, o.1, o.0] };
        let face = out3(outward(i));
        // Rows across the edge: (point, x, normal).
        let rows: [(P2, P2, f32, V3, V3); 4] = [
            (inset[i], inset[j], hw, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            (p[i], p[j], inner, out3(ni), out3(nj)),
            (p[i], p[j], -inner, out3(ni), out3(nj)),
            (inset[i], inset[j], -hw, [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ];
        let mut ids = [[0u32; 2]; 4];
        for (r, &(qa, qb, x, na, nb)) in rows.iter().enumerate() {
            ids[r][0] = m.vertex(pt(qa, x), na, [u0, x / uv_scale], WHITE);
            ids[r][1] = m.vertex(pt(qb, x), nb, [u1, x / uv_scale], WHITE);
        }
        for r in 0..3 {
            let want = match r {
                0 => add(face, [0.7, 0.0, 0.0]),
                2 => add(face, [-0.7, 0.0, 0.0]),
                _ => face,
            };
            quad_facing(&mut m, ids[r][0], ids[r][1], ids[r + 1][1], ids[r + 1][0], want);
        }
    }
    m
}

/// Flat polygon through 3D points (window glass), normals from the winding.
fn pane(points: &[V3]) -> MeshData {
    let mut m = MeshData::default();
    for (k, &p) in points.iter().enumerate() {
        m.vertex(p, [0.0, 1.0, 0.0], [k as f32 * 0.3, 0.0], WHITE);
    }
    for k in 1..points.len() as u32 - 1 {
        m.tri(0, k, k + 1);
    }
    m.recompute_normals();
    m
}

/// Jagged glass left in a frame after the window was smashed: teeth of glass
/// pointing in from each edge of the frame.
pub fn shards(frame: &[V3], seed: u64) -> MeshData {
    let mut rng = Rng::new(seed);
    let centre = scale(frame.iter().fold([0.0; 3], |a, &b| add(a, b)), 1.0 / frame.len() as f32);
    let mut m = MeshData::default();
    for i in 0..frame.len() {
        let (a, b) = (frame[i], frame[(i + 1) % frame.len()]);
        let teeth = 2 + (rng.f32() * 3.0) as usize;
        for t in 0..teeth {
            let (f0, f1) = (t as f32 / teeth as f32, (t as f32 + 1.0) / teeth as f32);
            let p0 = add(a, scale(sub(b, a), f0));
            let p1 = add(a, scale(sub(b, a), f1));
            let mid = scale(add(p0, p1), 0.5);
            let depth = rng.range(0.08, 0.35);
            let tip = add(mid, scale(sub(centre, mid), depth));
            let s = m.positions.len() as u32;
            for p in [p0, p1, tip] {
                m.vertex(p, [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
            }
            m.tri(s, s + 1, s + 2);
        }
    }
    m.recompute_normals();
    m
}

/// A box rotated so its long (Z) axis runs from `a` to `b`.
fn beam(a: V3, b: V3, w: f32, h: f32) -> MeshData {
    let d = sub(b, a);
    let len = meshgen::length(d);
    let yaw = d[0].atan2(d[2]);
    let pitch = -(d[1] / len.max(1e-6)).asin();
    meshgen::cuboid([w, h, len], 1.0).rotated_x(pitch).rotated_y(yaw).translated(scale(add(a, b), 0.5))
}

fn circle_path(centre: V3, radius: f32, axis_tilt: f32, points: usize) -> Vec<V3> {
    // A circle in the XY plane tilted back about X by `axis_tilt`.
    (0..=points)
        .map(|k| {
            let a = k as f32 / points as f32 * TAU;
            let local = [radius * a.cos(), radius * a.sin(), 0.0];
            let (s, c) = axis_tilt.sin_cos();
            add(centre, [local[0], local[1] * c, local[1] * s])
        })
        .collect()
}

fn mirror_x(m: &MeshData) -> MeshData {
    let mut out = m.clone();
    for p in &mut out.positions {
        p[0] = -p[0];
    }
    for n in &mut out.normals {
        n[0] = -n[0];
    }
    for t in out.indices.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    out
}

fn with_mirror(m: MeshData) -> MeshData {
    let mut both = m.clone();
    both.append(&mirror_x(&m));
    both
}

// ---------------------------------------------------------------------------
// Setting a vehicle down on the ground
// ---------------------------------------------------------------------------

/// Where a vehicle's root goes so it rests on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rest {
    pub y: f32,
    /// Nose-down rotation about the local X axis (radians; apply after yaw).
    pub pitch: f32,
    /// Rotation about the local Z axis (radians; positive lifts the +X side).
    pub roll: f32,
}

/// Fit a vehicle with yaw `yaw` at (`x`, `z`) to the ground. `contacts` are
/// (local x, local z, lift): where the wheels, skis or track touch, and how far
/// above the vehicle's base each contact sits (a flat tyre's contact is higher
/// than a full one). The vehicle tilts with the slope, and is then lowered so
/// no contact floats, plus `sink` to bury it a little in the snow.
pub fn settle(x: f32, z: f32, yaw: f32, contacts: &[(f32, f32, f32)], sink: f32, height: &dyn Fn(f32, f32) -> f32) -> Rest {
    let (s, c) = yaw.sin_cos();
    let samples: Vec<(f32, f32, f32)> = contacts
        .iter()
        .map(|&(lx, lz, lift)| {
            // Bevy's rotation_y(yaw): local +X -> (cos, -sin), local +Z -> (sin, cos).
            let (wx, wz) = (x + lx * c + lz * s, z - lx * s + lz * c);
            (lx, lz, height(wx, wz) - lift)
        })
        .collect();
    // Least squares plane h = a + b*lx + d*lz.
    let k = samples.len() as f32;
    let (mut sx, mut sz, mut sxx, mut szz, mut sxz, mut sh, mut sxh, mut szh) = (0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for &(lx, lz, h) in &samples {
        sx += lx;
        sz += lz;
        sxx += lx * lx;
        szz += lz * lz;
        sxz += lx * lz;
        sh += h;
        sxh += lx * h;
        szh += lz * h;
    }
    let mat = [[k, sx, sz], [sx, sxx, sxz], [sz, sxz, szz]];
    let rhs = [sh, sxh, szh];
    let (a, b, d) = match solve3(mat, rhs) {
        Some(v) => (v[0], v[1], v[2]),
        None => (sh / k.max(1.0), 0.0, 0.0),
    };
    // Lower the plane until nothing floats.
    let float = samples.iter().map(|&(lx, lz, h)| (a + b * lx + d * lz) - h).fold(0.0f32, f32::max);
    Rest { y: a - float - sink, pitch: -d.atan(), roll: b.atan() }
}

fn solve3(m: [[f32; 3]; 3], r: [f32; 3]) -> Option<[f32; 3]> {
    let det = |m: [[f32; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(m);
    if d.abs() < 1e-6 {
        return None;
    }
    let mut out = [0.0; 3];
    for (col, o) in out.iter_mut().enumerate() {
        let mut mm = m;
        for row in 0..3 {
            mm[row][col] = r[row];
        }
        *o = det(mm) / d;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The sedan
// ---------------------------------------------------------------------------

pub const WHEEL_R: f32 = 0.36;
/// A bare steel rim, after the tyre has rotted away.
pub const RIM_R: f32 = 0.25;
pub const WHEEL_X: f32 = 0.8;
/// Front and rear axle positions.
pub const WHEEL_Z: [f32; 2] = [1.42, -1.38];
pub const BODY_HALF_W: f32 = 0.96;
pub const BELT_Y: f32 = 1.0;
const ARCH_R: f32 = 0.47;
const SILL_Y: f32 = 0.42;

/// Side view of the lower body with both wheel arches cut out.
pub fn sedan_outline() -> Vec<P2> {
    let mut p: Vec<P2> = vec![(-2.38, SILL_Y + 0.04), (-2.3, SILL_Y)];
    // Arches, rear then front, going over each wheel.
    for &wz in WHEEL_Z.iter().rev() {
        let a0 = ((SILL_Y - WHEEL_R) / ARCH_R).asin();
        let steps = 12;
        for k in 0..=steps {
            let t = PI - a0 - (PI - 2.0 * a0) * k as f32 / steps as f32;
            p.push((wz + ARCH_R * t.cos(), WHEEL_R + ARCH_R * t.sin()));
        }
    }
    p.extend_from_slice(&[
        (2.3, SILL_Y),
        (2.42, 0.5),
        (2.47, 0.62),
        (2.47, 0.76),
        (2.42, 0.88),
        (2.3, 0.94),
        (1.9, 0.97),
        (1.3, 0.99),
        (0.85, BELT_Y),
        (-1.3, BELT_Y),
        (-1.7, 0.995),
        (-2.15, 0.975),
        (-2.38, 0.93),
        (-2.46, 0.82),
        (-2.47, 0.62),
    ]);
    p
}

/// Height of the top of the body at `z` (hood and trunk lines), for trim.
pub fn body_top(z: f32) -> f32 {
    let top: Vec<P2> = sedan_outline().into_iter().filter(|q| q.1 > 0.9).collect();
    let mut best = (f32::MAX, BELT_Y);
    for w in top.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
        if z >= lo && z <= hi {
            let t = (z - a.0) / (b.0 - a.0);
            return a.1 + (b.1 - a.1) * t;
        }
        let d = (z - a.0).abs();
        if d < best.0 {
            best = (d, a.1);
        }
    }
    best.1
}

pub fn sedan_body() -> MeshData {
    extrude_profile(&sedan_outline(), BODY_HALF_W, 0.09, 0.0, 1.5)
}

/// Rear fins with a hint of 1950s flash.
pub fn sedan_fins() -> MeshData {
    let fin = extrude_profile(&[(-2.47, 0.9), (-1.55, 0.99), (-2.36, 1.15), (-2.48, 1.1)], 0.04, 0.015, 0.0, 1.5);
    with_mirror(fin.translated([0.89, 0.0, 0.0]))
}

/// Cabin roof panel.
pub fn sedan_roof() -> MeshData {
    extrude_profile(&[(-0.86, 1.455), (0.34, 1.455), (0.3, 1.5), (0.1, 1.535), (-0.5, 1.535), (-0.8, 1.5)], 0.765, 0.05, 0.0, 1.5)
}

/// Half-width of the cabin at height `y` (it leans in towards the roof).
fn cabin_x(y: f32) -> f32 {
    0.87 - (y - BELT_Y) * 0.25
}

/// The pillars holding up the roof: thin A and B pillars and a wide C pillar.
pub fn sedan_pillars() -> MeshData {
    let mut m = MeshData::default();
    let top = 1.47;
    let a = tube(&[[cabin_x(BELT_Y), BELT_Y, 0.86], [cabin_x(top), top, 0.31]], 0.04, 6);
    let b = tube(&[[cabin_x(BELT_Y), BELT_Y, -0.22], [cabin_x(top), top, -0.22]], 0.045, 6);
    // C pillar: a thin slab leaning in with the cabin.
    let lean = (cabin_x(BELT_Y) - cabin_x(top)).atan2(top - BELT_Y);
    let c = extrude_profile(&[(-1.29, BELT_Y), (-0.86, BELT_Y), (-0.6, top), (-0.82, top)], 0.03, 0.012, 0.0, 1.5)
        .translated([0.0, -BELT_Y, 0.0])
        .rotated_z(lean)
        .translated([cabin_x(BELT_Y) - 0.02, BELT_Y, 0.0]);
    for part in [a, b, c] {
        m.append(&with_mirror(part));
    }
    m
}

/// Which pane of glass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Windshield,
    Rear,
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

impl Window {
    pub const ALL: [Window; 6] = [Window::Windshield, Window::Rear, Window::FrontLeft, Window::FrontRight, Window::RearLeft, Window::RearRight];
}

/// The outline of a window opening, in 3D, going round its frame.
pub fn window_frame(w: Window) -> Vec<V3> {
    let (lo, hi) = (BELT_Y + 0.02, 1.445);
    let side = |s: f32, pts: &[P2]| -> Vec<V3> { pts.iter().map(|&(z, y)| [s * (cabin_x(y) - 0.012), y, z]).collect() };
    match w {
        Window::FrontLeft => side(-1.0, &[(0.82, lo), (-0.18, lo), (-0.18, hi), (0.32, hi)]),
        Window::FrontRight => side(1.0, &[(0.82, lo), (0.32, hi), (-0.18, hi), (-0.18, lo)]),
        Window::RearLeft => side(-1.0, &[(-0.26, lo), (-0.84, lo), (-0.62, hi), (-0.26, hi)]),
        Window::RearRight => side(1.0, &[(-0.26, lo), (-0.26, hi), (-0.62, hi), (-0.84, lo)]),
        // Wraparound glass: bowed forwards in the middle.
        Window::Windshield => {
            let mut pts = Vec::new();
            for k in 0..=8 {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(lo) - 0.04), lo, 0.85 + 0.05 * (1.0 - f * f)]);
            }
            for k in (0..=8).rev() {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(hi) - 0.04), hi, 0.32 + 0.03 * (1.0 - f * f)]);
            }
            pts
        }
        Window::Rear => {
            let mut pts = Vec::new();
            for k in (0..=8).rev() {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(lo) - 0.04), lo, -1.27 - 0.05 * (1.0 - f * f)]);
            }
            for k in 0..=8 {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(hi) - 0.04), hi, -0.8 - 0.03 * (1.0 - f * f)]);
            }
            pts
        }
    }
}

/// An intact pane filling a window opening.
pub fn window_glass(w: Window) -> MeshData {
    let frame = window_frame(w);
    match w {
        // Curved panes: strip between the bottom and top rows.
        Window::Windshield | Window::Rear => {
            let half = frame.len() / 2;
            let mut m = MeshData::default();
            for k in 0..half {
                m.vertex(frame[k], [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
            }
            for k in 0..half {
                m.vertex(frame[frame.len() - 1 - k], [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
            }
            for k in 0..half as u32 - 1 {
                let h = half as u32;
                m.quad(k, k + 1, h + k + 1, h + k);
            }
            m.recompute_normals();
            m
        }
        _ => pane(&frame),
    }
}

/// Bench seats, dashboard, steering wheel and floor.
pub fn sedan_interior() -> (MeshData, MeshData) {
    let mut vinyl = MeshData::default();
    let bench = [(0.28, 0.58), (0.3, 0.78), (0.22, 0.82), (-0.26, 0.8), (-0.3, 1.24), (-0.36, 1.28), (-0.44, 1.26), (-0.42, 0.66), (-0.34, 0.58)];
    let front = extrude_profile(&bench, 0.74, 0.06, 0.0, 0.8).translated([0.0, 0.0, 0.0]);
    let rear = extrude_profile(&bench, 0.76, 0.06, 0.0, 0.8).scaled([1.0, 0.92, 1.0]).translated([0.0, 0.03, -0.68]);
    vinyl.append(&front);
    vinyl.append(&rear);
    vinyl.append(&meshgen::cuboid([1.7, 0.05, 2.3], 1.0).translated([0.0, 0.5, -0.25]));
    let mut dark = extrude_profile(&[(0.9, 0.7), (0.9, 1.0), (0.8, 1.09), (0.58, 1.07), (0.54, 0.94), (0.62, 0.7)], 0.84, 0.03, 0.0, 0.8);
    // Steering wheel on its column, driver's side (left).
    let hub = [-0.38, 1.1, 0.42];
    let tilt = 1.0;
    dark.append(&tube(&circle_path(hub, 0.19, tilt, 20), 0.017, 6));
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + FRAC_PI_2;
        let rim = add(hub, [0.19 * a.cos(), 0.19 * a.sin() * tilt.cos(), 0.19 * a.sin() * tilt.sin()]);
        dark.append(&tube(&[hub, rim], 0.01, 4));
    }
    dark.append(&tube(&[hub, [-0.38, 0.92, 0.72]], 0.025, 6));
    (vinyl, dark)
}

/// Bright work: bumpers with bullet guards, grille, headlight bezels, side
/// trim, door handles, hood ornament, mirror, wipers and an aerial.
pub fn sedan_chrome() -> MeshData {
    let mut m = MeshData::default();
    for (dir, zb) in [(1.0f32, 2.5f32), (-1.0, -2.5)] {
        let y = 0.5;
        let path = [
            [-0.95, y, zb - dir * 0.22],
            [-0.88, y, zb - dir * 0.07],
            [-0.6, y, zb],
            [0.6, y, zb],
            [0.88, y, zb - dir * 0.07],
            [0.95, y, zb - dir * 0.22],
        ];
        m.append(&tube(&path, 0.075, 8));
        // Bullet guards.
        for x in [-0.42f32, 0.42] {
            let bullet = lathe(&[(0.085, 0.0), (0.08, 0.06), (0.06, 0.12), (0.03, 0.16), (0.0, 0.175)], 10, 1.0, false, false)
                .rotated_x(dir * FRAC_PI_2)
                .translated([x, y + 0.08, zb]);
            m.append(&bullet);
        }
    }
    // Grille: a frame with bars and teeth.
    let (gz, gy0, gy1, gx) = (2.465, 0.6, 0.8, 0.52);
    m.append(&tube(&[[-gx, gy0, gz], [gx, gy0, gz], [gx, gy1, gz - 0.02], [-gx, gy1, gz - 0.02], [-gx, gy0, gz]], 0.025, 6));
    for k in 1..4 {
        let y = gy0 + (gy1 - gy0) * k as f32 / 4.0;
        m.append(&tube(&[[-gx, y, gz - 0.01], [gx, y, gz - 0.01]], 0.012, 5));
    }
    for k in 0..9 {
        let x = -gx + 0.1 + (2.0 * gx - 0.2) * k as f32 / 8.0;
        m.append(&lathe(&[(0.026, 0.0), (0.022, 0.06), (0.0, 0.08)], 6, 1.0, false, false).rotated_x(FRAC_PI_2).translated([x, gy0 + 0.06, gz]));
    }
    // Headlight bezels.
    for x in [-0.7f32, 0.7] {
        let ring = lathe(&[(0.1, 0.0), (0.14, 0.01), (0.15, 0.035), (0.13, 0.055), (0.11, 0.05)], 16, 1.0, false, false);
        m.append(&ring.rotated_x(FRAC_PI_2).translated([x, 0.78, 2.43]));
    }
    // Side trim along the flanks and door handles.
    for s in [-1.0f32, 1.0] {
        m.append(&tube(&[[s * 0.966, 0.86, 2.2], [s * 0.966, 0.86, 0.9], [s * 0.966, 0.8, -0.6], [s * 0.966, 0.8, -2.3]], 0.012, 5));
        for z in [0.02f32, -0.8] {
            m.append(&meshgen::cuboid([0.03, 0.035, 0.15], 1.0).translated([s * 0.975, 0.93, z]));
        }
    }
    // Hood ornament: a little rocket.
    m.append(&lathe(&[(0.03, 0.0), (0.035, 0.08), (0.02, 0.2), (0.0, 0.26)], 8, 1.0, true, false).rotated_x(FRAC_PI_2 * 0.8).translated([0.0, body_top(2.25) - 0.01, 2.18]));
    // Driver's mirror.
    m.append(&tube(&[[-0.93, 1.01, 0.72], [-1.02, 1.09, 0.7]], 0.012, 5));
    m.append(&lathe(&[(0.06, 0.0), (0.065, 0.02), (0.0, 0.035)], 10, 1.0, true, false).rotated_x(-FRAC_PI_2).translated([-1.03, 1.11, 0.69]));
    // Wipers lying on the cowl, and a whip aerial on the front fender.
    m.append(&tube(&[[-0.65, 1.035, 0.88], [-0.08, 1.06, 0.86]], 0.01, 4));
    m.append(&tube(&[[0.1, 1.035, 0.88], [0.62, 1.06, 0.86]], 0.01, 4));
    m.append(&tube(&[[0.85, 0.99, 1.6], [0.83, 1.9, 1.5]], 0.006, 4));
    m
}

/// Dark seams and fittings: door shut lines, hood and trunk lines, the
/// exhaust pipe and the radiator behind the grille.
pub fn sedan_trim() -> MeshData {
    let mut m = MeshData::default();
    let x = BODY_HALF_W + 0.002;
    for s in [-1.0f32, 1.0] {
        for (z, y0) in [(0.86f32, SILL_Y + 0.05), (-0.22, SILL_Y + 0.02), (-0.93, SILL_Y + 0.05)] {
            m.append(&beam([s * x, y0, z], [s * x, BELT_Y - 0.03, z], 0.006, 0.014));
        }
        m.append(&beam([s * x, SILL_Y + 0.05, 0.86], [s * x, SILL_Y + 0.05, -0.93], 0.006, 0.014));
        // Hood and trunk shut lines run along the top.
        for (z0, z1) in [(0.95f32, 2.3f32), (-1.45, -2.3)] {
            let pts: Vec<V3> = (0..=6).map(|k| {
                let z = z0 + (z1 - z0) * k as f32 / 6.0;
                [s * 0.7, body_top(z) + 0.004, z]
            }).collect();
            m.append(&tube(&pts, 0.008, 4));
        }
    }
    m.append(&beam([-0.7, body_top(0.95) + 0.004, 0.95], [0.7, body_top(0.95) + 0.004, 0.95], 0.014, 0.006));
    m.append(&beam([-0.7, body_top(-1.45) + 0.004, -1.45], [0.7, body_top(-1.45) + 0.004, -1.45], 0.014, 0.006));
    m.append(&tube(&[[-0.5, 0.36, -1.9], [-0.5, 0.35, -2.3], [-0.5, 0.36, -2.52]], 0.035, 8));
    m.append(&meshgen::cuboid([1.0, 0.22, 0.04], 1.0).translated([0.0, 0.7, 2.42]));
    m
}

/// Under the car: floor pan, frame rails, axles and differential, drive
/// shaft, leaf springs, engine sump and fuel tank (seen on the wreck that
/// ended up on its roof, and in the gap under the others).
pub fn sedan_underbody() -> MeshData {
    let mut m = meshgen::cuboid([1.74, 0.03, 4.3], 1.0).translated([0.0, SILL_Y + 0.005, -0.05]);
    for x in [-0.5f32, 0.5] {
        m.append(&meshgen::cuboid([0.08, 0.12, 4.3], 1.0).translated([x, SILL_Y - 0.06, -0.05]));
        m.append(&beam([x * 1.2, SILL_Y - 0.13, -2.0], [x * 1.2, SILL_Y - 0.13, -0.75], 0.06, 0.03));
    }
    for &wz in &WHEEL_Z {
        m.append(&tube(&[[-WHEEL_X, WHEEL_R, wz], [WHEEL_X, WHEEL_R, wz]], 0.045, 8));
    }
    let diff = lathe(&[(0.0, -0.12), (0.1, -0.1), (0.13, 0.0), (0.1, 0.1), (0.0, 0.12)], 10, 1.0, false, false).rotated_x(FRAC_PI_2);
    m.append(&diff.translated([0.0, WHEEL_R, WHEEL_Z[1]]));
    m.append(&tube(&[[0.0, WHEEL_R + 0.02, WHEEL_Z[1] + 0.12], [0.0, SILL_Y - 0.04, 0.7]], 0.035, 8));
    m.append(&meshgen::cuboid([0.5, 0.18, 0.7], 1.0).translated([0.0, SILL_Y - 0.08, 1.55]));
    m.append(&meshgen::cuboid([0.9, 0.16, 0.45], 1.0).translated([0.0, SILL_Y - 0.07, -2.0]));
    m
}

/// Headlight lenses and the red tail lamps in the fins.
pub fn sedan_lenses() -> (MeshData, MeshData) {
    let mut head = MeshData::default();
    for x in [-0.7f32, 0.7] {
        head.append(&lathe(&[(0.105, 0.0), (0.09, 0.025), (0.05, 0.042), (0.0, 0.048)], 14, 1.0, false, false).rotated_x(FRAC_PI_2).translated([x, 0.78, 2.44]));
    }
    let mut tail = MeshData::default();
    for x in [-0.89f32, 0.89] {
        tail.append(&lathe(&[(0.05, 0.0), (0.045, 0.06), (0.0, 0.08)], 10, 1.0, false, false).rotated_x(-FRAC_PI_2).translated([x, 1.04, -2.45]));
    }
    (head, tail)
}

/// A front door panel hinged at its front edge (the origin), for wrecks with a
/// door hanging open. Same shape as the hole it leaves.
pub fn sedan_door() -> MeshData {
    extrude_profile(&[(0.0, SILL_Y + 0.05), (-1.08, SILL_Y + 0.02), (-1.08, BELT_Y), (0.0, BELT_Y)], 0.03, 0.015, 0.0, 1.5)
}

/// The dark hole an open door leaves, a flat panel just outside the body.
pub fn door_opening(side: f32) -> MeshData {
    let x = side * (BODY_HALF_W + 0.004);
    let pts = [[x, SILL_Y + 0.06, 0.85], [x, SILL_Y + 0.03, -0.21], [x, BELT_Y - 0.02, -0.21], [x, BELT_Y - 0.02, 0.85]];
    let mut m = pane(&pts);
    if dot(m.normals[0], [side, 0.0, 0.0]) < 0.0 {
        for t in m.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    m
}

/// A tyre around the X axis, centred on the origin: rounded sidewalls and a
/// blocky tread. `flat` (0..1) squashes the bottom like a flat.
pub fn tyre(radius: f32, width: f32, flat: f32) -> MeshData {
    let hw = width * 0.5;
    // Cross-section (radius, x) from the inner bead round the tread to the outer.
    let k = radius / WHEEL_R;
    let section: [(f32, f32); 9] = [
        (0.24 * k, -hw * 0.9),
        (0.31 * k, -hw),
        (0.345 * k, -hw * 0.92),
        (radius, -hw * 0.62),
        (radius, 0.0),
        (radius, hw * 0.62),
        (0.345 * k, hw * 0.92),
        (0.31 * k, hw),
        (0.24 * k, hw * 0.9),
    ];
    let segs = 48;
    let mut m = MeshData::default();
    let floor = -radius * (1.0 - 0.28 * flat);
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * TAU;
        let block = (i / 2) % 2 == 0;
        for (r_i, &(r, x)) in section.iter().enumerate() {
            let tread = (3..=5).contains(&r_i);
            let rr = if tread && block { r + 0.012 * k } else { r };
            let mut p = [x, rr * a.cos(), rr * a.sin()];
            if p[1] < floor {
                let squash = floor - p[1];
                p[1] = floor;
                p[0] *= 1.0 + squash * 2.5;
            }
            m.vertex(p, [1.0, 0.0, 0.0], [r_i as f32 / 8.0, i as f32 / segs as f32 * 6.0], WHITE);
        }
    }
    let rows = section.len() as u32;
    for i in 0..segs as u32 {
        for r in 0..rows - 1 {
            let a = i * rows + r;
            let b = a + rows;
            m.quad(a, a + 1, b + 1, b);
        }
    }
    m.recompute_normals();
    // Make sure the tread faces out.
    if m.outwardness([0.0; 3]) < 0.0 {
        for t in m.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    m
}

/// A pressed-steel wheel rim around the X axis, outer face towards +X.
pub fn rim(radius: f32, width: f32) -> MeshData {
    let hw = width * 0.5;
    let mut barrel = lathe(&[(radius, -hw), (radius * 0.92, -hw * 0.8), (radius * 0.92, hw * 0.8), (radius, hw)], 20, 1.0, false, false);
    // The lathe faces outwards; the barrel is seen from outside too.
    barrel = barrel.rotated_z(-FRAC_PI_2);
    let disc = lathe(&[(radius * 0.92, hw * 0.3), (radius * 0.6, hw * 0.45), (radius * 0.35, hw * 0.75), (radius * 0.2, hw * 0.75), (0.0, hw * 0.8)], 20, 1.0, false, false).rotated_z(-FRAC_PI_2);
    let mut m = barrel;
    m.append(&disc);
    // Lug nuts.
    for k in 0..5 {
        let a = k as f32 / 5.0 * TAU;
        let nut = lathe(&[(0.016, 0.0), (0.016, 0.02), (0.0, 0.025)], 6, 1.0, false, false).rotated_z(-FRAC_PI_2);
        m.append(&nut.translated([hw * 0.75, radius * 0.27 * a.cos(), radius * 0.27 * a.sin()]));
    }
    m
}

/// A domed chrome hubcap, outer face towards +X.
pub fn hubcap(radius: f32) -> MeshData {
    lathe(&[(radius, 0.0), (radius * 0.97, 0.015), (radius * 0.75, 0.035), (radius * 0.3, 0.06), (radius * 0.1, 0.065), (0.0, 0.066)], 20, 1.0, false, false).rotated_z(-FRAC_PI_2)
}

/// Where a sedan's wheels touch the ground (local x, z).
pub fn sedan_wheels() -> [(f32, f32); 4] {
    [(-WHEEL_X, WHEEL_Z[0]), (WHEEL_X, WHEEL_Z[0]), (-WHEEL_X, WHEEL_Z[1]), (WHEEL_X, WHEEL_Z[1])]
}

// ---------------------------------------------------------------------------
// The snowmobile
// ---------------------------------------------------------------------------

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
    for c in &mut hull.colors {
        *c = WHITE;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn closed_and_outward(m: &MeshData, centre: V3) {
        assert!(m.is_valid());
        assert!(m.outwardness(centre) > 0.0, "triangles face inwards");
    }

    #[test]
    fn ear_clipping_covers_a_concave_outline() {
        // An L shape: area 3.
        let l = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        for poly in [l.to_vec(), l.iter().rev().cloned().collect()] {
            let tris = triangulate(&poly);
            assert_eq!(tris.len(), 4);
            let area: f32 = tris.iter().map(|t| cross2(poly[t[0]], poly[t[1]], poly[t[2]]) * 0.5).sum();
            assert!((area - 3.0).abs() < 1e-5, "area {area}");
            assert!(tris.iter().all(|t| cross2(poly[t[0]], poly[t[1]], poly[t[2]]) > 0.0));
        }
    }

    #[test]
    fn extruded_slabs_are_closed_and_face_out() {
        let m = extrude_profile(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)], 0.5, 0.1, 0.0, 1.0);
        closed_and_outward(&m, [0.0, 0.5, 1.0]);
        let (lo, hi) = m.bounds();
        assert!((hi[0] - 0.5).abs() < 1e-5 && (lo[0] + 0.5).abs() < 1e-5);
        assert!((hi[1] - 1.0).abs() < 1e-5 && lo[1].abs() < 1e-5);
        // Every triangle faces away from the middle, not just on average.
        for t in m.indices.chunks_exact(3) {
            let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let mid = scale(add(add(a, b), c), 1.0 / 3.0);
            assert!(dot(n, sub(mid, [0.0, 0.5, 1.0])) > -1e-6);
        }
    }

    #[test]
    fn the_sedan_body_has_wheel_arches_over_its_wheels() {
        let body = sedan_body();
        closed_and_outward(&body, [0.0, 0.75, 0.0]);
        let (lo, hi) = body.bounds();
        assert!((hi[0] - BODY_HALF_W).abs() < 1e-4);
        assert!(hi[2] - lo[2] > 4.8, "a full-size sedan");
        // No body directly above a wheel's top below the arch.
        for &wz in &WHEEL_Z {
            let over_wheel = sedan_outline().iter().filter(|q| (q.0 - wz).abs() < 0.1).map(|q| q.1).fold(f32::MAX, f32::min);
            assert!(over_wheel > WHEEL_R * 2.0 + 0.05, "arch clears the tyre: {over_wheel}");
        }
        assert!((body_top(1.3) - 0.99).abs() < 0.02);
    }

    #[test]
    fn sedan_parts_are_valid_meshes() {
        let (vinyl, dark) = sedan_interior();
        let (head, tail) = sedan_lenses();
        let mut parts = vec![sedan_underbody(), sedan_fins(), sedan_roof(), sedan_pillars(), sedan_chrome(), sedan_trim(), vinyl, dark, head, tail, sedan_door(), door_opening(1.0)];
        for w in Window::ALL {
            parts.push(window_glass(w));
            parts.push(shards(&window_frame(w), 4));
        }
        for (i, p) in parts.iter().enumerate() {
            assert!(p.is_valid(), "part {i}");
        }
        // The roof sits on top of the pillars and the glass reaches it.
        let (_, roof_hi) = sedan_roof().bounds();
        assert!(roof_hi[1] > 1.5 && roof_hi[1] < 1.6);
        let (_, glass_hi) = window_glass(Window::Windshield).bounds();
        assert!(glass_hi[1] > 1.4);
        // An open door's hole faces outwards on its side.
        assert!(door_opening(-1.0).normals.iter().all(|n| n[0] < -0.9));
    }

    #[test]
    fn tyres_are_round_and_a_flat_one_sags() {
        let t = tyre(WHEEL_R, 0.22, 0.0);
        closed_and_outward(&t, [0.0; 3]);
        let (lo, hi) = t.bounds();
        assert!(lo[1] < -WHEEL_R + 1e-3 && hi[1] > WHEEL_R - 1e-3);
        assert!(hi[0] <= 0.12);
        let flat = tyre(WHEEL_R, 0.22, 1.0);
        let (flo, fhi) = flat.bounds();
        assert!(flo[1] > lo[1] + 0.08, "flat bottom sits higher");
        assert!(fhi[0] > hi[0], "and bulges out");
        assert!(rim(RIM_R, 0.18).is_valid() && hubcap(0.2).is_valid());
    }

    #[test]
    fn settling_on_flat_ground_leaves_the_vehicle_level() {
        let contacts: Vec<_> = sedan_wheels().iter().map(|&(x, z)| (x, z, 0.0)).collect();
        let r = settle(10.0, 5.0, 0.7, &contacts, 0.0, &|_, _| 2.0);
        assert!((r.y - 2.0).abs() < 1e-4 && r.pitch.abs() < 1e-5 && r.roll.abs() < 1e-5);
    }

    #[test]
    fn settling_follows_a_slope_and_no_wheel_floats() {
        // Ground rising towards +X in the world; vehicle faces +X (yaw 90 degrees),
        // so its nose should point up.
        let slope = |x: f32, _z: f32| 0.2 * x;
        let contacts: Vec<_> = sedan_wheels().iter().map(|&(x, z)| (x, z, 0.0)).collect();
        let yaw = FRAC_PI_2;
        let r = settle(0.0, 0.0, yaw, &contacts, 0.0, &slope);
        assert!(r.pitch < -0.15 && r.pitch > -0.25, "nose up: {}", r.pitch);
        assert!(r.roll.abs() < 1e-4);
        // Bumpy ground: every wheel ends up touching or slightly buried.
        let bumpy = |x: f32, z: f32| (x * 0.9).sin() * 0.3 + (z * 1.3).cos() * 0.2;
        for yaw in [0.0f32, 0.8, 2.5] {
            let r = settle(3.0, -2.0, yaw, &contacts, 0.05, &bumpy);
            let rot_y = |lx: f32, lz: f32| -> f32 {
                // Height of a contact point after pitch and roll (small angles).
                r.y + lx * r.roll.tan() - lz * r.pitch.tan()
            };
            let (s, c) = yaw.sin_cos();
            for &(lx, lz, _) in &contacts {
                let (wx, wz) = (3.0 + lx * c + lz * s, -2.0 - lx * s + lz * c);
                assert!(rot_y(lx, lz) <= bumpy(wx, wz) + 1e-3, "wheel floats at yaw {yaw}");
            }
        }
        // A flat tyre's corner drops.
        let flat: Vec<_> = sedan_wheels().iter().enumerate().map(|(i, &(x, z))| (x, z, if i == 0 { 0.1 } else { 0.0 })).collect();
        let r = settle(0.0, 0.0, 0.0, &flat, 0.0, &|_, _| 0.0);
        assert!(r.roll > 0.0, "left-front flat tips the car to the left: {}", r.roll);
    }

    #[test]
    fn snowmobile_parts_hang_together() {
        let body = snowmobile_body();
        let black = snowmobile_black();
        let steel = snowmobile_steel();
        let track = snowmobile_track();
        let (head, tail) = snowmobile_lenses();
        for p in [&body, &black, &steel, &track, &snowmobile_windshield(), &head, &tail] {
            assert!(p.is_valid());
        }
        // Skis and track touch y = 0; nothing pokes below it.
        let (slo, shi) = steel.bounds();
        assert!(slo[1].abs() < 0.01, "skis on the snow: {}", slo[1]);
        assert!(shi[0] > SKI_X && slo[0] < -SKI_X);
        let (tlo, thi) = track.bounds();
        assert!(tlo[1] > -0.06 && tlo[1] < 0.0, "lugs bite into the snow: {}", tlo[1]);
        assert!(thi[1] < 0.64, "track fits under the tunnel deck");
        // Seat is above the track and the body joins the tunnel.
        let (blo, _) = black.bounds();
        assert!(blo[1] < 0.4);
        let (_, bhi) = body.bounds();
        assert!(bhi[1] > 0.9 && bhi[1] < 1.0);
    }
}
