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

/// A box centred on the origin with its edges and corners rounded off to
/// `radius` (machined or cast metal, worn wood), `steps` facets round each
/// rounded edge. UVs are planar per side, world-scaled like [`cuboid`]'s,
/// and the normals are exact, so the rounding shades smoothly.
pub fn rounded_box(size: V3, radius: f32, steps: usize, uv_scale: f32) -> MeshData {
    let h = scale(size, 0.5);
    let r = radius.max(0.0).min(h[0]).min(h[1]).min(h[2]);
    let inner = [h[0] - r, h[1] - r, h[2] - r];
    let steps = steps.max(1);
    // Where vertices go along one axis of half-width `hw`: the flat run, then
    // points spaced evenly in angle round the rounded edge at each end.
    let stations = |ri: f32| -> Vec<f32> {
        let mut out = Vec::new();
        for k in (0..=steps).rev() {
            out.push(-(ri + r * (k as f32 / steps as f32 * std::f32::consts::FRAC_PI_2).sin()));
        }
        for k in 0..=steps {
            out.push(ri + r * (k as f32 / steps as f32 * std::f32::consts::FRAC_PI_2).sin());
        }
        out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
        out
    };
    let axes = [stations(inner[0]), stations(inner[1]), stations(inner[2])];
    // (normal axis, sign, u axis, v axis).
    let faces: [(usize, f32, usize, usize); 6] = [(0, 1.0, 2, 1), (0, -1.0, 2, 1), (1, 1.0, 0, 2), (1, -1.0, 0, 2), (2, 1.0, 0, 1), (2, -1.0, 0, 1)];
    let mut m = MeshData::default();
    for (n, sign, ua, va) in faces {
        let base = m.positions.len() as u32;
        let (us, vs) = (&axes[ua], &axes[va]);
        for &v in vs {
            for &u in us {
                // A point on the flat box, pulled onto the rounded one.
                let mut p = [0.0f32; 3];
                p[n] = sign * h[n];
                p[ua] = u;
                p[va] = v;
                let c = [p[0].clamp(-inner[0], inner[0]), p[1].clamp(-inner[1], inner[1]), p[2].clamp(-inner[2], inner[2])];
                let d = sub(p, c);
                let len = length(d);
                let normal = if len > 1e-9 { scale(d, 1.0 / len) } else { let mut f = [0.0; 3]; f[n] = sign; f };
                let q = add(c, scale(normal, r));
                // Planar UVs on the side, as `cuboid` lays them.
                let uv = [q[ua] / uv_scale, -q[va] / uv_scale];
                m.vertex(q, normal, uv, WHITE);
            }
        }
        // Wind each side so it faces out: (u, v) must turn the same way as
        // the side's outward normal.
        let axis = |i: usize| {
            let mut e = [0.0f32; 3];
            e[i] = 1.0;
            e
        };
        let out = cross(axis(ua), axis(va))[n] * sign > 0.0;
        let row = us.len() as u32;
        for j in 0..vs.len() as u32 - 1 {
            for i in 0..row - 1 {
                let a = base + j * row + i;
                if out {
                    m.quad(a, a + 1, a + row + 1, a + row);
                } else {
                    m.quad(a, a + row, a + row + 1, a + 1);
                }
            }
        }
    }
    m
}

/// Rounds the corners off a closed outline (Chaikin's corner cutting,
/// `passes` times): each pass replaces every corner with two points a
/// quarter of the way along its edges.
pub fn smooth_outline(outline: &[[f32; 2]], passes: usize) -> Vec<[f32; 2]> {
    let mut pts = outline.to_vec();
    for _ in 0..passes {
        let n = pts.len();
        let mut next = Vec::with_capacity(n * 2);
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            next.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
            next.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
        }
        pts = next;
    }
    pts
}

/// A flat part cut from plate and dressed: the closed `outline` (z, y)
/// extruded across x to `half_width` either side, with its edges eased by a
/// `bevel` (the faces are inset by it). The outline must be star-shaped
/// about its centroid (true of any convex shape and most gun parts). The
/// sides are smooth round the outline, so a smoothed outline (see
/// [`smooth_outline`]) gives a part with no hard corners.
pub fn extrude_x(outline: &[[f32; 2]], half_width: f32, bevel: f32, uv_scale: f32) -> MeshData {
    let n = outline.len();
    let c = outline.iter().fold([0.0f32; 2], |a, p| [a[0] + p[0] / n as f32, a[1] + p[1] / n as f32]);
    let bevel = bevel.clamp(0.0, half_width * 0.9);
    // The face outline, pulled in towards the centre by the bevel.
    let inset: Vec<[f32; 2]> = outline
        .iter()
        .map(|p| {
            let (dz, dy) = (p[0] - c[0], p[1] - c[1]);
            let d = (dz * dz + dy * dy).sqrt().max(1e-6);
            let k = ((d - bevel) / d).max(0.1);
            [c[0] + dz * k, c[1] + dy * k]
        })
        .collect();
    let wall_x = half_width - bevel;
    let mut m = MeshData::default();
    // A ring of quads between two loops of the outline.
    let strip = |m: &mut MeshData, a: &[[f32; 2]], ax: f32, b: &[[f32; 2]], bx: f32| {
        let mut piece = MeshData::default();
        let mut along = 0.0;
        for i in 0..=n {
            let (p, q) = (a[i % n], b[i % n]);
            if i > 0 {
                let prev = a[i - 1];
                along += ((p[0] - prev[0]).powi(2) + (p[1] - prev[1]).powi(2)).sqrt();
            }
            piece.vertex([ax, p[1], p[0]], [0.0, 1.0, 0.0], [along / uv_scale, ax / uv_scale], WHITE);
            piece.vertex([bx, q[1], q[0]], [0.0, 1.0, 0.0], [along / uv_scale, bx / uv_scale], WHITE);
        }
        for i in 0..n as u32 {
            let (p0, q0, p1, q1) = (i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 3);
            piece.quad(p0, p1, q1, q0);
        }
        // Face away from the part's middle.
        if piece.outwardness([0.0, c[1], c[0]]) < 0.0 {
            for t in piece.indices.as_chunks_mut::<3>().0 {
                t.swap(1, 2);
            }
        }
        piece.recompute_normals();
        // The loop's first and last columns are the same place: one normal.
        for (x, y) in [(0usize, n * 2), (1, n * 2 + 1)] {
            let nn = normalize(add(piece.normals[x], piece.normals[y]));
            piece.normals[x] = nn;
            piece.normals[y] = nn;
        }
        m.append(&piece);
    };
    // The sides, then the bevels down to each face.
    strip(&mut m, outline, -wall_x, outline, wall_x);
    for sign in [-1.0f32, 1.0] {
        strip(&mut m, outline, sign * wall_x, &inset, sign * half_width);
    }
    // The two faces: fans from the centre.
    for sign in [-1.0f32, 1.0] {
        let normal = [sign, 0.0, 0.0];
        let base = m.positions.len() as u32;
        m.vertex([sign * half_width, c[1], c[0]], normal, [c[0] / uv_scale, -c[1] / uv_scale], WHITE);
        for p in &inset {
            m.vertex([sign * half_width, p[1], p[0]], normal, [p[0] / uv_scale, -p[1] / uv_scale], WHITE);
        }
        for i in 0..n as u32 {
            let (a, b) = (base + 1 + i, base + 1 + (i + 1) % n as u32);
            // Wound to face along `normal`.
            let pa = m.positions[a as usize];
            let pb = m.positions[b as usize];
            let pc = m.positions[base as usize];
            let face = cross(sub(pa, pc), sub(pb, pc));
            if face[0] * sign >= 0.0 {
                m.tri(base, a, b);
            } else {
                m.tri(base, b, a);
            }
        }
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
