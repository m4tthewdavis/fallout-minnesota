//! Procedural meshes: Northwoods pines, Frostfang wolves, pre-war cars, fish
//! houses, the Vault 143 gear door, rocks, snowdrifts, roads and power lines.
//!
//! Everything here is plain geometry (positions, normals, UVs, vertex colours
//! and triangle indices) with no Bevy types, so shapes can be unit-tested.
//! `crate::meshes` turns a [`MeshData`] into a Bevy mesh.
//!
//! Conventions: Y is up, triangles wind counter-clockwise seen from the front
//! (Bevy's default), and UVs are in world units divided by `uv_scale` so
//! tiling textures keep the same size on big and small objects.

use std::f32::consts::{PI, TAU};

use super::rng::Rng;

pub type V3 = [f32; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}
pub fn normalize(a: V3) -> V3 {
    let l = length(a);
    if l > 1e-8 {
        scale(a, 1.0 / l)
    } else {
        [0.0, 1.0, 0.0]
    }
}
fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    add(a, scale(sub(b, a), t))
}
fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Smooth pseudo-random noise in roughly -1..=1 (a few detuned sines).
fn wobble(seed: f32, a: f32, b: f32) -> f32 {
    ((a * 1.7 + seed).sin() * (b * 1.3 + seed * 1.9).cos()
        + 0.5 * (a * 3.1 - b * 2.3 + seed * 0.7).sin()
        + 0.25 * (a * 5.3 + b * 4.7 + seed * 2.3).cos())
        / 1.75
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<V3>,
    pub normals: Vec<V3>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn vertex(&mut self, p: V3, n: V3, uv: [f32; 2], c: [f32; 4]) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        self.uvs.push(uv);
        self.colors.push(c);
        (self.positions.len() - 1) as u32
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Two triangles; `a b c d` go counter-clockwise seen from the front.
    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn append(&mut self, other: &MeshData) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    pub fn translated(mut self, t: V3) -> Self {
        for p in &mut self.positions {
            *p = add(*p, t);
        }
        self
    }

    /// Rotate about the Y axis (radians, counter-clockwise seen from above).
    pub fn rotated_y(mut self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let rot = |v: V3| [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c];
        for p in &mut self.positions {
            *p = rot(*p);
        }
        for n in &mut self.normals {
            *n = rot(*n);
        }
        self
    }

    /// Rotate about the X axis (radians).
    pub fn rotated_x(mut self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let rot = |v: V3| [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c];
        for p in &mut self.positions {
            *p = rot(*p);
        }
        for n in &mut self.normals {
            *n = rot(*n);
        }
        self
    }

    /// Rotate about the Z axis (radians).
    pub fn rotated_z(mut self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let rot = |v: V3| [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]];
        for p in &mut self.positions {
            *p = rot(*p);
        }
        for n in &mut self.normals {
            *n = rot(*n);
        }
        self
    }

    /// Non-uniform scale; normals use the inverse scale so lighting stays right.
    pub fn scaled(mut self, s: V3) -> Self {
        for p in &mut self.positions {
            *p = [p[0] * s[0], p[1] * s[1], p[2] * s[2]];
        }
        for n in &mut self.normals {
            *n = normalize([n[0] / s[0], n[1] / s[1], n[2] / s[2]]);
        }
        self
    }

    pub fn tinted(mut self, c: [f32; 4]) -> Self {
        for col in &mut self.colors {
            *col = c;
        }
        self
    }

    /// Smooth, area-weighted vertex normals from the triangles.
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![[0.0f32; 3]; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = cross(
                sub(self.positions[b], self.positions[a]),
                sub(self.positions[c], self.positions[a]),
            );
            for i in [a, b, c] {
                acc[i] = add(acc[i], n);
            }
        }
        self.normals = acc.into_iter().map(normalize).collect();
    }

    /// Every index points at a vertex and every attribute has one entry per vertex.
    pub fn is_valid(&self) -> bool {
        let n = self.positions.len();
        n > 0
            && self.normals.len() == n
            && self.uvs.len() == n
            && self.colors.len() == n
            && self.indices.len() % 3 == 0
            && !self.indices.is_empty()
            && self.indices.iter().all(|&i| (i as usize) < n)
            && self.positions.iter().flatten().all(|v| v.is_finite())
            && self.normals.iter().all(|nv| (length(*nv) - 1.0).abs() < 1e-3)
    }

    /// Axis-aligned bounds: (min, max).
    pub fn bounds(&self) -> (V3, V3) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &self.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    /// Sum over triangles of (face normal · direction from `centre`). Positive
    /// means the triangles of a closed, roughly convex shape face outwards.
    pub fn outwardness(&self, centre: V3) -> f32 {
        self.indices
            .chunks_exact(3)
            .map(|t| {
                let (a, b, c) = (
                    self.positions[t[0] as usize],
                    self.positions[t[1] as usize],
                    self.positions[t[2] as usize],
                );
                let n = cross(sub(b, a), sub(c, a));
                let mid = scale(add(add(a, b), c), 1.0 / 3.0);
                dot(n, sub(mid, centre))
            })
            .sum()
    }
}

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

/// A grid that hugs the ground (`height` gives the surface), lifted by `lift`.
/// Used for road surfaces and crater scorch decals. UVs span 0..1 when
/// `uv_scale` is `None`, otherwise they are world-scaled.
pub fn ground_patch(
    cx: f32,
    cz: f32,
    half: f32,
    res: usize,
    lift: f32,
    uv_scale: Option<f32>,
    height: &dyn Fn(f32, f32) -> f32,
) -> MeshData {
    let mut m = MeshData::default();
    for i in 0..=res {
        for j in 0..=res {
            let (fx, fz) = (i as f32 / res as f32, j as f32 / res as f32);
            let x = cx - half + 2.0 * half * fx;
            let z = cz - half + 2.0 * half * fz;
            let uv = match uv_scale {
                Some(s) => [x / s, z / s],
                None => [fx, fz],
            };
            m.vertex([x, height(x, z) + lift, z], [0.0, 1.0, 0.0], uv, WHITE);
        }
    }
    let row = res as u32 + 1;
    for i in 0..res as u32 {
        for j in 0..res as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m.recompute_normals();
    m
}

/// A wind-blown snowdrift that grows out of the ground: a terrain-following
/// grid, `length` along the wind and `width` across, rising to `peak`. The
/// windward side is a long gentle ramp and the lee side a short steep face,
/// like real drifts. The rim dips just under the ground so it blends with the
/// terrain without a seam. `wind` is the direction the wind blows towards.
/// Positions are in world space; UVs are world xz / `uv_scale` like the terrain.
#[allow(clippy::too_many_arguments)]
pub fn drift_patch(
    cx: f32,
    cz: f32,
    length: f32,
    width: f32,
    peak: f32,
    wind: [f32; 2],
    seed: u64,
    uv_scale: f32,
    height: &dyn Fn(f32, f32) -> f32,
) -> MeshData {
    let res = 18usize;
    let half = length.max(width);
    let (wx, wz) = {
        let l = (wind[0] * wind[0] + wind[1] * wind[1]).sqrt().max(1e-6);
        (wind[0] / l, wind[1] / l)
    };
    let fs = (seed % 997) as f32 * 0.61;
    let bump = |x: f32, z: f32| -> f32 {
        let (dx, dz) = (x - cx, z - cz);
        // u along the wind, v across it.
        let u = dx * wx + dz * wz;
        let v = -dx * wz + dz * wx;
        let ul = if u > 0.0 { length * 0.35 } else { length };
        let edge = 1.0 + 0.18 * wobble(fs, x * 0.7, z * 0.7);
        let d = ((u / ul).powi(2) + (v / (width * 0.5)).powi(2)).sqrt() / edge;
        if d >= 1.0 {
            return -0.04;
        }
        let t = 0.5 + 0.5 * (d * PI).cos();
        peak * t * t.sqrt() - 0.04 * (1.0 - t)
    };
    let mut m = MeshData::default();
    for i in 0..=res {
        for j in 0..=res {
            let x = cx - half + 2.0 * half * i as f32 / res as f32;
            let z = cz - half + 2.0 * half * j as f32 / res as f32;
            m.vertex([x, height(x, z) + bump(x, z), z], [0.0, 1.0, 0.0], [x / uv_scale, z / uv_scale], WHITE);
        }
    }
    let row = res as u32 + 1;
    for i in 0..res as u32 {
        for j in 0..res as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    m.recompute_normals();
    m
}

/// A ribbon along a path in the XZ plane that hugs the ground (roads).
/// V runs along the road, U across it (0..1).
pub fn ground_strip(path: &[(f32, f32)], width: f32, lift: f32, uv_len: f32, height: &dyn Fn(f32, f32) -> f32) -> MeshData {
    let mut m = MeshData::default();
    let mut along = 0.0;
    for (i, &(x, z)) in path.iter().enumerate() {
        let (ax, az) = path[i.saturating_sub(1)];
        let (bx, bz) = path[(i + 1).min(path.len() - 1)];
        let dir = normalize([bx - ax, 0.0, bz - az]);
        let side = [dir[2], 0.0, -dir[0]];
        if i > 0 {
            let (px, pz) = path[i - 1];
            along += ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
        }
        for (k, s) in [-0.5f32, 0.5].into_iter().enumerate() {
            let px = x + side[0] * s * width;
            let pz = z + side[2] * s * width;
            m.vertex([px, height(px, pz) + lift, pz], [0.0, 1.0, 0.0], [k as f32, along / uv_len], WHITE);
        }
    }
    for i in 0..path.len() as u32 - 1 {
        let (a, b, c, d) = (i * 2, i * 2 + 1, i * 2 + 3, i * 2 + 2);
        // Pick the winding that faces up.
        let n = cross(sub(m.positions[b as usize], m.positions[a as usize]), sub(m.positions[c as usize], m.positions[a as usize]));
        if n[1] >= 0.0 {
            m.quad(a, b, c, d);
        } else {
            m.quad(a, d, c, b);
        }
    }
    m.recompute_normals();
    m
}

/// Points along a hanging cable between `a` and `b` that sags by `sag` metres.
pub fn catenary(a: V3, b: V3, sag: f32, points: usize) -> Vec<V3> {
    (0..=points)
        .map(|i| {
            let t = i as f32 / points as f32;
            let mut p = lerp3(a, b, t);
            p[1] -= sag * 4.0 * t * (1.0 - t);
            p
        })
        .collect()
}

/// A thin tube following a polyline (wires, pipes).
pub fn tube(path: &[V3], radius: f32, sides: usize) -> MeshData {
    let mut m = MeshData::default();
    for (i, &p) in path.iter().enumerate() {
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(path.len() - 1)];
        let t = normalize(sub(b, a));
        let helper = if t[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = normalize(cross(t, helper));
        let v = cross(u, t);
        for j in 0..sides {
            let th = j as f32 / sides as f32 * TAU;
            let n = add(scale(u, th.cos()), scale(v, th.sin()));
            m.vertex(add(p, scale(n, radius)), n, [j as f32 / sides as f32, i as f32], WHITE);
        }
    }
    let s = sides as u32;
    for i in 0..path.len() as u32 - 1 {
        for j in 0..s {
            let a = i * s + j;
            let b = i * s + (j + 1) % s;
            m.quad(a, a + s, b + s, b);
        }
    }
    m
}

/// Vault-door cog: a disc of `radius` with `teeth` around the rim, extruded
/// `thickness` along Z (front face +Z). Planar UVs for a metal texture.
pub fn gear(radius: f32, tooth: f32, teeth: usize, thickness: f32, uv_scale: f32) -> MeshData {
    // Outline: each tooth is a trapezoid (base -> top -> top -> base).
    let mut outline: Vec<(f32, f32)> = Vec::new();
    for k in 0..teeth {
        let a0 = k as f32 / teeth as f32 * TAU;
        let step = TAU / teeth as f32;
        for (frac, r) in [(0.0, radius), (0.18, radius + tooth), (0.48, radius + tooth), (0.66, radius)] {
            let a = a0 + frac * step;
            outline.push((r * a.cos(), r * a.sin()));
        }
    }
    let mut m = MeshData::default();
    let hz = thickness * 0.5;
    for (z, n) in [(hz, [0.0, 0.0, 1.0]), (-hz, [0.0, 0.0, -1.0])] {
        let centre = m.vertex([0.0, 0.0, z], n, [0.5, 0.5], WHITE);
        let start = m.positions.len() as u32;
        for &(x, y) in &outline {
            m.vertex([x, y, z], n, [x / uv_scale, -y / uv_scale], WHITE);
        }
        let len = outline.len() as u32;
        for j in 0..len {
            let (a, b) = (start + j, start + (j + 1) % len);
            if z > 0.0 {
                m.tri(centre, a, b);
            } else {
                m.tri(centre, b, a);
            }
        }
    }
    // Rim walls, one flat quad per outline edge.
    let len = outline.len();
    let mut along = 0.0;
    for j in 0..len {
        let (x0, y0) = outline[j];
        let (x1, y1) = outline[(j + 1) % len];
        let n = normalize([y1 - y0, -(x1 - x0), 0.0]);
        let edge = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let (u0, u1) = (along / uv_scale, (along + edge) / uv_scale);
        along += edge;
        let a = m.vertex([x0, y0, -hz], n, [u0, 0.0], WHITE);
        let b = m.vertex([x1, y1, -hz], n, [u1, 0.0], WHITE);
        let c = m.vertex([x1, y1, hz], n, [u1, thickness / uv_scale], WHITE);
        let d = m.vertex([x0, y0, hz], n, [u0, thickness / uv_scale], WHITE);
        m.quad(a, b, c, d);
    }
    m
}

/// Gabled shed walls (fish houses): a box `w` x `h` x `d` with triangular
/// gables of height `gable` on the front (+Z) and back. No floor.
pub fn gable_walls(w: f32, h: f32, d: f32, gable: f32, uv_scale: f32) -> MeshData {
    let mut m = cuboid([w, h, d], uv_scale).translated([0.0, h * 0.5, 0.0]);
    // Drop the bottom face (vertices 12..16 are the cuboid's -Y face).
    let bottom: Vec<u32> = (12..16).collect();
    let mut kept = Vec::with_capacity(m.indices.len());
    for t in m.indices.chunks_exact(3) {
        if !t.iter().all(|i| bottom.contains(i)) {
            kept.extend_from_slice(t);
        }
    }
    m.indices = kept;
    for (z, n) in [(d * 0.5, [0.0, 0.0, 1.0]), (-d * 0.5, [0.0, 0.0, -1.0])] {
        let l = m.vertex([-w * 0.5, h, z], n, [-w * 0.5 / uv_scale, -h / uv_scale], WHITE);
        let r = m.vertex([w * 0.5, h, z], n, [w * 0.5 / uv_scale, -h / uv_scale], WHITE);
        let t = m.vertex([0.0, h + gable, z], n, [0.0, -(h + gable) / uv_scale], WHITE);
        if z > 0.0 {
            m.tri(l, r, t);
        } else {
            m.tri(r, l, t);
        }
    }
    m
}

/// Pine colours.
const NEEDLES_DARK: [f32; 4] = [0.07, 0.16, 0.11, 1.0];
const NEEDLES_LIGHT: [f32; 4] = [0.13, 0.27, 0.17, 1.0];
const SNOW: [f32; 4] = [0.92, 0.95, 1.0, 1.0];

/// Snow-laden Northwoods pine foliage (no trunk), base at y = 0, `height` tall.
/// Tiers of drooping, jagged branch skirts; vertex colours carry the needle
/// greens and the snow sitting on top of each tier.
pub fn pine(height: f32, tiers: usize, snow: f32, seed: u64) -> MeshData {
    let mut rng = Rng::new(seed);
    let mut m = MeshData::default();
    let spikes = 9;
    let segs = spikes * 2;
    let bottom = height * 0.18;
    let span = height - bottom;
    for k in 0..tiers {
        let f = k as f32 / tiers as f32;
        let top = bottom + span * (f + 1.6 / tiers as f32).min(1.0);
        let base = bottom + span * f;
        let radius = height * 0.36 * (1.0 - f * 0.82) * rng.range(0.9, 1.1);
        let droop = radius * 0.28;
        let twist = rng.range(0.0, TAU);
        let green = lerp4(NEEDLES_DARK, NEEDLES_LIGHT, rng.f32());
        // Rings from the trunk outwards: (radius fraction, height).
        let rings = [(0.08, top), (0.55, base + (top - base) * 0.35), (1.0, base - droop)];
        let start = m.positions.len() as u32;
        for (ri, &(rf, y)) in rings.iter().enumerate() {
            for j in 0..segs {
                let th = twist + j as f32 / segs as f32 * TAU;
                let tip = if j % 2 == 0 { 1.0 } else { 0.72 };
                let jag = if ri == 0 { 1.0 } else { tip * rng.range(0.9, 1.08) };
                let r = radius * rf * jag;
                let yy = y + if ri == 2 && j % 2 == 0 { -droop * 0.3 } else { 0.0 };
                m.vertex([r * th.cos(), yy, -r * th.sin()], [0.0, 1.0, 0.0], [0.0, 0.0], green);
            }
        }
        let s = segs as u32;
        for ri in 0..2u32 {
            for j in 0..s {
                let a = start + ri * s + j;
                let b = start + ri * s + (j + 1) % s;
                let c = start + (ri + 1) * s + (j + 1) % s;
                let d = start + (ri + 1) * s + j;
                // Outward/upward facing: inner ring -> outer ring.
                m.quad(a, d, c, b);
            }
        }
        // Underside back to the trunk so the tier isn't hollow from below.
        let under = m.vertex([0.0, base + (top - base) * 0.15, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0], NEEDLES_DARK);
        for j in 0..s {
            let a = start + 2 * s + j;
            let b = start + 2 * s + (j + 1) % s;
            m.tri(under, b, a);
        }
    }
    // Tip.
    let tip_base = m.positions.len() as u32;
    let tip_y = height;
    let ring_y = height * 0.9;
    let rr = height * 0.035;
    for j in 0..6 {
        let th = j as f32 / 6.0 * TAU;
        m.vertex([rr * th.cos(), ring_y, -rr * th.sin()], [0.0, 1.0, 0.0], [0.0, 0.0], NEEDLES_LIGHT);
    }
    let apex = m.vertex([0.0, tip_y, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0], NEEDLES_LIGHT);
    for j in 0..6 {
        m.tri(tip_base + j, tip_base + (j + 1) % 6, apex);
    }
    m.recompute_normals();
    // Snow settles on upward-facing needles, more on the outer branches.
    for i in 0..m.positions.len() {
        let n = m.normals[i];
        let p = m.positions[i];
        let outer = ((p[0] * p[0] + p[2] * p[2]).sqrt() / (height * 0.2)).min(1.0);
        let patch = 0.75 + 0.25 * wobble(seed as f32, p[0] * 3.0, p[2] * 3.0 + p[1]);
        let cover = smoothstep(0.35, 0.8, n[1]) * snow * patch * (0.55 + 0.45 * outer);
        m.colors[i] = lerp4(m.colors[i], SNOW, cover.clamp(0.0, 1.0));
    }
    m
}

/// A bare, dead tree (snag): a few crooked branches. Returned as tubes in one
/// mesh, base at the origin.
pub fn dead_branches(height: f32, seed: u64) -> MeshData {
    let mut rng = Rng::new(seed);
    let mut m = MeshData::default();
    let count = 5 + (rng.f32() * 4.0) as usize;
    for _ in 0..count {
        let y = rng.range(height * 0.35, height * 0.9);
        let ang = rng.range(0.0, TAU);
        let len = rng.range(0.6, 1.6) * (1.2 - y / height);
        let dir = [ang.cos(), rng.range(0.3, 0.9), -ang.sin()];
        let start = [0.0, y, 0.0];
        let mid = add(start, scale(dir, len * 0.5));
        let end = add(add(start, scale(dir, len)), [rng.range(-0.2, 0.2), rng.range(-0.1, 0.2), rng.range(-0.2, 0.2)]);
        m.append(&tube(&[start, mid, end], 0.05, 5));
    }
    m
}

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

pub fn wolf_body() -> MeshData {
    wolf_body_variant(0)
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

pub fn wolf_head() -> MeshData {
    wolf_head_variant(0)
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

/// Lower body of a rounded 1950s sedan, along Z (front +Z), wheels' ground at y = 0.
pub fn car_body(uv_scale: f32) -> MeshData {
    let q = 5.0;
    loft_z(
        &[
            sec(-2.32, 0.72, 0.8, 0.2, q),
            sec(-2.25, 0.74, 0.92, 0.3, q),
            sec(-1.8, 0.76, 0.96, 0.34, q),
            sec(-1.0, 0.74, 0.97, 0.33, q),
            sec(0.6, 0.74, 0.97, 0.33, q),
            sec(1.5, 0.74, 0.96, 0.32, q),
            sec(2.1, 0.7, 0.93, 0.28, q),
            sec(2.3, 0.66, 0.82, 0.2, q),
        ],
        20,
        uv_scale,
    )
}

/// Rounded cabin (greenhouse) of the sedan, sits on top of [`car_body`].
pub fn car_cabin(uv_scale: f32) -> MeshData {
    let q = 3.5;
    loft_z(
        &[
            sec(-1.15, 1.1, 0.8, 0.04, q),
            sec(-0.95, 1.22, 0.8, 0.18, q),
            sec(-0.6, 1.3, 0.78, 0.27, q),
            sec(0.35, 1.3, 0.77, 0.27, q),
            sec(0.7, 1.2, 0.79, 0.17, q),
            sec(0.85, 1.1, 0.8, 0.04, q),
        ],
        16,
        uv_scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed_and_outward(m: &MeshData, centre: V3) {
        assert!(m.is_valid());
        assert!(m.outwardness(centre) > 0.0, "triangles face inwards");
    }

    #[test]
    fn cuboid_faces_out_and_uvs_scale_with_size() {
        let m = cuboid([4.0, 2.0, 1.0], 1.0);
        assert_eq!(m.triangle_count(), 12);
        closed_and_outward(&m, [0.0; 3]);
        // Every triangle must face out, not just on average.
        for t in m.indices.chunks_exact(3) {
            let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let mid = scale(add(add(a, b), c), 1.0 / 3.0);
            assert!(dot(n, mid) > 0.0);
            assert!(dot(normalize(n), m.normals[t[0] as usize]) > 0.99, "stored normal matches winding");
        }
        let (lo, hi) = m.bounds();
        assert_eq!(lo, [-2.0, -1.0, -0.5]);
        assert_eq!(hi, [2.0, 1.0, 0.5]);
        let umax = m.uvs.iter().map(|uv| uv[0].abs()).fold(0.0, f32::max);
        assert!((umax - 2.0).abs() < 1e-5, "a 4 m face spans 4 texture tiles");
    }

    #[test]
    fn lathe_barrel_is_closed_and_outward() {
        let m = lathe(&[(0.4, 0.0), (0.45, 0.5), (0.4, 1.0)], 12, 1.0, true, true);
        closed_and_outward(&m, [0.0, 0.5, 0.0]);
        let (lo, hi) = m.bounds();
        assert!((hi[1] - 1.0).abs() < 1e-5 && lo[1].abs() < 1e-5);
        assert!(hi[0] <= 0.45 + 1e-5);
        // Analytic normals point away from the axis on the bulge.
        let i = 12 + 1; // second ring, first vertex at +X
        assert!(m.normals[i][0] > 0.9);
    }

    #[test]
    fn loft_faces_out_with_caps() {
        let m = loft_z(&[sec(-1.0, 0.0, 0.5, 0.5, 2.0), sec(1.0, 0.0, 0.5, 0.5, 2.0)], 12, 1.0);
        closed_and_outward(&m, [0.0; 3]);
        // Side normals are horizontal-ish and outward.
        assert!(m.normals[0][0] > 0.6);
    }

    #[test]
    fn blob_is_outward_and_bumpy() {
        let m = blob(2.0, 0.6, 0.15, 7, 1.0);
        closed_and_outward(&m, [0.0; 3]);
        let (lo, hi) = m.bounds();
        assert!(hi[1] < 2.0 * 0.6 * 1.2 && lo[1] > -2.0 * 0.6 * 1.2);
        let radii: Vec<f32> = m.positions.iter().map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt()).collect();
        let max = radii.iter().cloned().fold(0.0, f32::max);
        assert!(max > 2.0 * 1.02, "noise pushes some points out");
    }

    #[test]
    fn flat_shapes_face_up() {
        let d = disc(3.0, 24, 2.0);
        assert!(d.is_valid());
        assert!(d.outwardness([0.0, -1.0, 0.0]) > 0.0);
        let g = ground_patch(0.0, 0.0, 5.0, 8, 0.05, None, &|x, z| (x * 0.3).sin() + z * 0.1);
        assert!(g.is_valid());
        assert!(g.normals.iter().all(|n| n[1] > 0.5));
        let road = ground_strip(&[(0.0, 0.0), (10.0, 0.0), (20.0, 5.0)], 6.0, 0.05, 6.0, &|_, _| 0.0);
        assert!(road.is_valid());
        assert!(road.normals.iter().all(|n| n[1] > 0.99));
        let reversed = ground_strip(&[(20.0, 5.0), (10.0, 0.0), (0.0, 0.0)], 6.0, 0.05, 6.0, &|_, _| 0.0);
        assert!(reversed.normals.iter().all(|n| n[1] > 0.99));
    }

    #[test]
    fn drifts_rise_from_the_ground_with_a_steep_lee_face() {
        let flat = |_: f32, _: f32| 1.0;
        let m = drift_patch(0.0, 0.0, 6.0, 3.0, 0.8, [1.0, 0.0], 3, 4.0, &flat);
        assert!(m.is_valid());
        assert!(m.normals.iter().all(|n| n[1] > 0.0), "faces up");
        let (lo, hi) = m.bounds();
        assert!(hi[1] > 1.6 && hi[1] < 1.85, "peaks about 0.8 above the ground: {}", hi[1]);
        assert!(lo[1] < 1.0, "rim tucks under the ground");
        let at = |x: f32| m.positions.iter().filter(|p| p[2].abs() < 0.2).min_by(|a, b| (a[0] - x).abs().total_cmp(&(b[0] - x).abs())).unwrap()[1];
        // Downwind (+x) it falls away faster than upwind.
        assert!(at(1.5) < at(-1.5), "lee {} vs windward {}", at(1.5), at(-1.5));
    }

    #[test]
    fn ground_patch_hugs_the_height_field() {
        let h = |x: f32, z: f32| x * 0.5 + z;
        let g = ground_patch(10.0, -4.0, 3.0, 4, 0.1, Some(2.0), &h);
        for p in &g.positions {
            assert!((p[1] - (h(p[0], p[2]) + 0.1)).abs() < 1e-4);
        }
    }

    #[test]
    fn cables_sag_in_the_middle() {
        let pts = catenary([0.0, 8.0, 0.0], [30.0, 8.0, 0.0], 1.5, 10);
        assert_eq!(pts.len(), 11);
        assert_eq!(pts[0], [0.0, 8.0, 0.0]);
        assert!((pts[5][1] - 6.5).abs() < 1e-4);
        let wire = tube(&pts, 0.02, 4);
        assert!(wire.is_valid());
        assert_eq!(wire.triangle_count(), 10 * 4 * 2);
    }

    #[test]
    fn gear_has_teeth_and_faces_out() {
        let m = gear(4.0, 0.5, 10, 0.9, 2.0);
        closed_and_outward(&m, [0.0; 3]);
        let (lo, hi) = m.bounds();
        assert!(hi[0] > 4.3 && lo[0] < -4.0);
        assert!((hi[2] - 0.45).abs() < 1e-5);
    }

    #[test]
    fn shed_walls_have_gables_and_no_floor() {
        let m = gable_walls(3.0, 2.4, 3.0, 1.0, 1.0);
        assert!(m.is_valid());
        let (_, hi) = m.bounds();
        assert!((hi[1] - 3.4).abs() < 1e-5);
        assert!(m.outwardness([0.0, 1.5, 0.0]) > 0.0);
        let floor = m.indices.chunks_exact(3).any(|t| t.iter().all(|&i| m.positions[i as usize][1] == 0.0 && m.normals[i as usize][1] < -0.5));
        assert!(!floor);
    }

    #[test]
    fn pines_are_snowy_on_top_and_tall_enough() {
        let m = pine(7.0, 6, 1.0, 3);
        assert!(m.is_valid());
        let (lo, hi) = m.bounds();
        assert!((hi[1] - 7.0).abs() < 1e-4);
        assert!(lo[1] > 0.0, "foliage starts above the ground");
        // Upward-facing vertices are whiter than downward ones.
        let avg = |up: bool| {
            let v: Vec<f32> = m
                .normals
                .iter()
                .zip(&m.colors)
                .filter(|(n, _)| (n[1] > 0.7) == up && n[1].abs() > 0.7)
                .map(|(_, c)| c[0] + c[1] + c[2])
                .collect();
            v.iter().sum::<f32>() / v.len().max(1) as f32
        };
        assert!(avg(true) > avg(false) + 0.5);
        let bare = pine(7.0, 6, 0.0, 3);
        assert!(bare.colors.iter().all(|c| c[0] < 0.3));
    }

    #[test]
    fn wolf_parts_line_up() {
        let body = wolf_body();
        let head = wolf_head();
        let leg = wolf_leg();
        let tail = wolf_tail();
        for m in [&body, &head, &leg, &tail] {
            assert!(m.is_valid());
        }
        closed_and_outward(&body, [0.0, 0.85, 0.0]);
        closed_and_outward(&leg, [0.0, -0.3, 0.0]);
        // Head starts where the neck ends, and legs reach the ground from the
        // 0.6 m hip pivot used by the animation.
        let (_, body_hi) = body.bounds();
        let (head_lo, head_hi) = head.bounds();
        assert!(head_lo[2] < body_hi[2]);
        assert!(head_hi[2] > 1.25);
        let (leg_lo, _) = leg.bounds();
        assert!((leg_lo[1] + 0.62).abs() < 1e-4);
        // Pale belly, darker back.
        let top = body.colors[body.normals.iter().position(|n| n[1] > 0.95).unwrap()];
        let bottom = body.colors[body.normals.iter().position(|n| n[1] < -0.9).unwrap()];
        assert!(top[0] < bottom[0]);
    }

    #[test]
    fn car_cabin_sits_on_the_body() {
        let body = car_body(1.0);
        let cabin = car_cabin(1.0);
        closed_and_outward(&body, [0.0, 0.74, 0.0]);
        closed_and_outward(&cabin, [0.0, 1.3, 0.0]);
        let (blo, bhi) = body.bounds();
        let (clo, chi) = cabin.bounds();
        assert!(blo[1] > 0.3, "body clears the wheels' ground");
        assert!(clo[1] < bhi[1] && chi[1] > bhi[1]);
        assert!(bhi[2] - blo[2] > 4.5);
    }

    #[test]
    fn transforms_keep_normals_unit() {
        let m = cuboid([1.0, 1.0, 1.0], 1.0)
            .scaled([2.0, 0.5, 1.0])
            .rotated_y(0.7)
            .rotated_x(0.3)
            .rotated_z(-0.2)
            .translated([5.0, 0.0, 0.0]);
        assert!(m.is_valid());
        assert!(m.outwardness([5.0, 0.0, 0.0]) > 0.0);
        let mut both = m.clone();
        both.append(&disc(1.0, 8, 1.0));
        assert!(both.is_valid());
        assert_eq!(both.triangle_count(), 12 + 8);
    }

    #[test]
    fn dead_branches_are_valid() {
        let m = dead_branches(6.0, 9);
        assert!(m.is_valid());
        let (lo, hi) = m.bounds();
        assert!(lo[1] > 1.0 && hi[1] < 8.0);
    }

    #[test]
    fn wolf_variants_look_different_but_share_a_shape() {
        let base = wolf_body_variant(0);
        for v in 0..WOLF_VARIANTS {
            let b = wolf_body_variant(v);
            let h = wolf_head_variant(v);
            assert!(b.is_valid() && h.is_valid(), "variant {v}");
            assert_eq!(b.positions, base.positions, "same body shape");
            if v > 0 {
                assert_ne!(b.colors, base.colors, "variant {v} has its own markings");
            }
        }
        // Ears differ: variant 2 stands taller than variant 1's torn ear.
        let top = |m: &MeshData| m.bounds().1[1];
        assert!(top(&wolf_head_variant(2)) > top(&wolf_head_variant(1)) + 0.02);
        // Variant numbers wrap rather than panic.
        assert_eq!(wolf_head_variant(WOLF_VARIANTS).colors, wolf_head_variant(0).colors);
    }

    #[test]
    fn wolf_patterns_keep_colours_in_range() {
        for v in 0..WOLF_VARIANTS {
            for m in [wolf_body_variant(v), wolf_head_variant(v)] {
                assert!(m.colors.iter().all(|c| c.iter().all(|x| (0.0..=1.001).contains(x))), "variant {v}");
            }
        }
    }

    #[test]
    fn the_moose_is_big_and_hangs_together() {
        let body = moose_body();
        let head = moose_head();
        let leg = moose_leg();
        for m in [&body, &head, &leg] {
            assert!(m.is_valid());
        }
        closed_and_outward(&body, [0.0, 1.55, 0.0]);
        let (blo, bhi) = body.bounds();
        assert!(bhi[2] - blo[2] > 2.3, "over 2 m long");
        assert!(bhi[1] > 2.0 && blo[1] > 0.8, "belly clears the snow, back is well above it");
        // The head starts inside the body's front and reaches well past it.
        let (hlo, hhi) = head.bounds();
        assert!(hlo[2] < bhi[2] && hhi[2] > bhi[2] + 1.2);
        // Legs reach from the body down to the snow: hip pivot at ~1.1.
        let (llo, lhi) = leg.bounds();
        assert!((llo[1] + 1.1).abs() < 1e-4 && lhi[1] > 0.0);
    }

    #[test]
    fn antlers_are_wide_mirror_images_with_tips() {
        let (l, lt) = moose_antler(-1.0);
        let (r, rt) = moose_antler(1.0);
        assert!(l.is_valid() && r.is_valid());
        assert_eq!(lt.len(), 6);
        assert_eq!(lt.len(), rt.len());
        for (a, b) in lt.iter().zip(&rt) {
            assert!((a[0] + b[0]).abs() < 1e-5 && a[1] == b[1] && a[2] == b[2], "mirror images");
        }
        let (_, rhi) = r.bounds();
        assert!(rhi[0] > 1.2, "palms spread wide: {}", rhi[0]);
        // Tips stand above the head and nowhere near the ground.
        assert!(rt.iter().all(|t| t[1] > 2.1));
    }
}
