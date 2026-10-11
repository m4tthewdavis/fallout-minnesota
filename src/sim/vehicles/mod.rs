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

mod settle;
mod sedan;
mod snowmobile;

pub use self::settle::*;
pub use self::sedan::*;
pub use self::snowmobile::*;

#[cfg(test)]
mod tests;

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
    for t in out.indices.as_chunks_mut::<3>().0 {
        t.swap(1, 2);
    }
    out
}

fn with_mirror(m: MeshData) -> MeshData {
    let mut both = m.clone();
    both.append(&mirror_x(&m));
    both
}
