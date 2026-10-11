//! The basic shapes everything else is built from: boxes, lathes, lofts, blobs and discs.

use super::*;

/// A box centred on the origin with world-scaled UVs.
pub fn cuboid(size: V3, uv_scale: f32) -> MeshData {
    let h = scale(size, 0.5);
    // (normal, u axis, v axis) with u x v = normal.
    let faces: [(V3, V3, V3); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let mut m = MeshData::default();
    let half = |axis: V3| dot([axis[0].abs(), axis[1].abs(), axis[2].abs()], h);
    for (n, u, v) in faces {
        let (hn, hu, hv) = (half(n), half(u), half(v));
        let centre = scale(n, hn);
        let mut ids = [0u32; 4];
        for (k, (su, sv)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].into_iter().enumerate() {
            let p = add(add(centre, scale(u, su * hu)), scale(v, sv * hv));
            let uv = [dot(p, u) / uv_scale, -dot(p, v) / uv_scale];
            ids[k] = m.vertex(p, n, uv, WHITE);
        }
        m.quad(ids[0], ids[1], ids[2], ids[3]);
    }
    m
}

/// Surface of revolution around the Y axis. `profile` is (radius, y) from
/// bottom to top. Optional flat caps close the ends.
pub fn lathe(profile: &[(f32, f32)], segments: usize, uv_scale: f32, cap_bottom: bool, cap_top: bool) -> MeshData {
    let mut m = MeshData::default();
    let rings = profile.len();
    let mut arc = 0.0;
    let mut ring_start = Vec::with_capacity(rings);
    for i in 0..rings {
        if i > 0 {
            let (r0, y0) = profile[i - 1];
            let (r1, y1) = profile[i];
            arc += ((r1 - r0).powi(2) + (y1 - y0).powi(2)).sqrt();
        }
        // Profile tangent from neighbours -> outward normal (dy, -dr).
        let (ra, ya) = profile[i.saturating_sub(1)];
        let (rb, yb) = profile[(i + 1).min(rings - 1)];
        let (dr, dy) = (rb - ra, yb - ya);
        let pn = {
            let l = (dr * dr + dy * dy).sqrt().max(1e-6);
            (dy / l, -dr / l)
        };
        let (r, y) = profile[i];
        let max_r = profile.iter().fold(0.0f32, |a, p| a.max(p.0));
        ring_start.push(m.positions.len() as u32);
        for j in 0..=segments {
            let th = j as f32 / segments as f32 * TAU;
            let (c, s) = (th.cos(), th.sin());
            let p = [r * c, y, -r * s];
            let n = normalize([pn.0 * c, pn.1, -pn.0 * s]);
            let u = th * max_r.max(0.05) / uv_scale;
            m.vertex(p, n, [u, -arc / uv_scale], WHITE);
        }
    }
    for i in 0..rings - 1 {
        for j in 0..segments as u32 {
            let a = ring_start[i] + j;
            let b = a + 1;
            let c = ring_start[i + 1] + j + 1;
            let d = ring_start[i + 1] + j;
            m.quad(a, b, c, d);
        }
    }
    let cap = |m: &mut MeshData, (r, y): (f32, f32), up: bool| {
        if r <= 1e-4 {
            return;
        }
        let n = if up { [0.0, 1.0, 0.0] } else { [0.0, -1.0, 0.0] };
        let centre = m.vertex([0.0, y, 0.0], n, [0.0, 0.0], WHITE);
        let start = m.positions.len() as u32;
        for j in 0..=segments {
            let th = j as f32 / segments as f32 * TAU;
            let p = [r * th.cos(), y, -r * th.sin()];
            m.vertex(p, n, [p[0] / uv_scale, p[2] / uv_scale], WHITE);
        }
        for j in 0..segments as u32 {
            if up {
                m.tri(centre, start + j, start + j + 1);
            } else {
                m.tri(centre, start + j + 1, start + j);
            }
        }
    };
    if cap_bottom {
        cap(&mut m, profile[0], false);
    }
    if cap_top {
        cap(&mut m, profile[rings - 1], true);
    }
    m
}

/// One cross-section of a [`loft_z`] shape: a superellipse in the XY plane.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub z: f32,
    /// Centre height.
    pub y: f32,
    pub rx: f32,
    pub ry: f32,
    /// 2 = ellipse, higher = boxier.
    pub square: f32,
}

pub const fn sec(z: f32, y: f32, rx: f32, ry: f32, square: f32) -> Section {
    Section { z, y, rx, ry, square }
}

fn spow(v: f32, e: f32) -> f32 {
    v.signum() * v.abs().powf(e)
}

/// Smooth body swept along +Z through superellipse sections (wolf bodies,
/// car bodies). Ends are closed with caps.
pub fn loft_z(sections: &[Section], segments: usize, uv_scale: f32) -> MeshData {
    let mut m = MeshData::default();
    let ring = |s: &Section, j: usize| -> V3 {
        let phi = j as f32 / segments as f32 * TAU;
        let e = 2.0 / s.square;
        [s.rx * spow(phi.cos(), e), s.y + s.ry * spow(phi.sin(), e), s.z]
    };
    for s in sections {
        let per = PI * (s.rx + s.ry);
        for j in 0..segments {
            let p = ring(s, j);
            let u = j as f32 / segments as f32 * per / uv_scale;
            m.vertex(p, [0.0, 1.0, 0.0], [u, -s.z / uv_scale], WHITE);
        }
    }
    let seg = segments as u32;
    for i in 0..sections.len() as u32 - 1 {
        for j in 0..seg {
            let a = i * seg + j;
            let b = i * seg + (j + 1) % seg;
            let c = (i + 1) * seg + (j + 1) % seg;
            let d = (i + 1) * seg + j;
            m.quad(a, b, c, d);
        }
    }
    m.recompute_normals();
    for (s, front) in [(sections[0], false), (sections[sections.len() - 1], true)] {
        if s.rx < 1e-4 || s.ry < 1e-4 {
            continue;
        }
        let n = if front { [0.0, 0.0, 1.0] } else { [0.0, 0.0, -1.0] };
        let centre = m.vertex([0.0, s.y, s.z], n, [0.0, 0.0], WHITE);
        let start = m.positions.len() as u32;
        for j in 0..segments {
            let p = ring(&s, j);
            m.vertex(p, n, [p[0] / uv_scale, -p[1] / uv_scale], WHITE);
        }
        for j in 0..seg {
            let (a, b) = (start + j, start + (j + 1) % seg);
            if front {
                m.tri(centre, a, b);
            } else {
                m.tri(centre, b, a);
            }
        }
    }
    m
}

/// A bumpy, squashed sphere: boulders, snowdrifts, snow caps.
pub fn blob(radius: f32, squash: f32, bumpiness: f32, seed: u64, uv_scale: f32) -> MeshData {
    let stacks = 10;
    let slices = 16;
    let fs = (seed % 1000) as f32 * 0.37;
    let mut m = MeshData::default();
    for i in 0..=stacks {
        let v = i as f32 / stacks as f32;
        let lat = PI * (v - 0.5);
        for j in 0..=slices {
            let u = j as f32 / slices as f32;
            let lon = u * TAU;
            // Wrap the noise so the seam matches.
            let noise = wobble(fs, lon.sin() * 2.0 + lat, lon.cos() * 2.0 - lat * 1.3);
            let r = radius * (1.0 + bumpiness * noise);
            let p = [r * lat.cos() * lon.cos(), r * squash * lat.sin(), -r * lat.cos() * lon.sin()];
            m.vertex(p, [0.0, 1.0, 0.0], [u * TAU * radius / uv_scale, v * PI * radius / uv_scale], WHITE);
        }
    }
    let row = slices as u32 + 1;
    for i in 0..stacks as u32 {
        for j in 0..slices as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m.recompute_normals();
    // The seam column is duplicated; average its normals so it doesn't show.
    for i in 0..=stacks {
        let (a, b) = (i * (slices + 1), i * (slices + 1) + slices);
        let n = normalize(add(m.normals[a], m.normals[b]));
        m.normals[a] = n;
        m.normals[b] = n;
    }
    m
}

/// Flat disc facing +Y (lake ice).
pub fn disc(radius: f32, segments: usize, uv_scale: f32) -> MeshData {
    let mut m = MeshData::default();
    let up = [0.0, 1.0, 0.0];
    let centre = m.vertex([0.0; 3], up, [0.0, 0.0], WHITE);
    for j in 0..=segments {
        let th = j as f32 / segments as f32 * TAU;
        let p = [radius * th.cos(), 0.0, -radius * th.sin()];
        m.vertex(p, up, [p[0] / uv_scale, p[2] / uv_scale], WHITE);
    }
    for j in 0..segments as u32 {
        m.tri(centre, 1 + j, 2 + j);
    }
    m
}
