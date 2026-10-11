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
use super::mathx::{dot3, smoothstep};

mod hands;
mod moose;
mod primitives;
mod scenery;
mod wolf;

pub use self::hands::*;
pub use self::moose::*;
pub use self::primitives::*;
pub use self::scenery::*;
pub use self::wolf::*;

#[cfg(test)]
mod tests;

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
    dot3(a, b)
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

    #[cfg(test)]
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
        self.colors.fill(c);
        self
    }

    /// Smooth, area-weighted vertex normals from the triangles.
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![[0.0f32; 3]; self.positions.len()];
        for t in self.indices.as_chunks::<3>().0 {
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
    #[cfg(test)]
    pub fn is_valid(&self) -> bool {
        let n = self.positions.len();
        n > 0
            && self.normals.len() == n
            && self.uvs.len() == n
            && self.colors.len() == n
            && self.indices.len().is_multiple_of(3)
            && !self.indices.is_empty()
            && self.indices.iter().all(|&i| (i as usize) < n)
            && self.positions.iter().flatten().all(|v| v.is_finite())
            && self.normals.iter().all(|nv| (length(*nv) - 1.0).abs() < 1e-3)
    }

    /// Axis-aligned bounds: (min, max).
    #[cfg(test)]
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
            .as_chunks::<3>().0.iter()
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

const SNOW: [f32; 4] = [0.92, 0.95, 1.0, 1.0];
