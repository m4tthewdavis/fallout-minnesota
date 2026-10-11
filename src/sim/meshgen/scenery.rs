//! Ground, drifts, roads, wires, gears, huts, tents and dead trees.

use super::*;

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
    for t in m.indices.as_chunks::<3>().0 {
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

/// A ridge tent: [`gable_walls`] with two sloping canvas roof panels that
/// run past the walls by `overhang` (eaves and front/back), so the roof is
/// part of the tent instead of a lid. Base at the origin, ridge along z.
pub fn tent(w: f32, h: f32, d: f32, gable: f32, overhang: f32, uv_scale: f32) -> MeshData {
    let mut m = gable_walls(w, h, d, gable, uv_scale);
    let half = w * 0.5;
    let top = h + gable;
    // The slope carries on past the wall top to the eave.
    let ex = half + overhang;
    let ey = h - overhang * gable / half;
    let zf = d * 0.5 + overhang;
    let slope = ((ex * ex) + (top - ey) * (top - ey)).sqrt();
    for side in [-1.0f32, 1.0] {
        let n = normalize([side * gable, half, 0.0]);
        let v = |m: &mut MeshData, x: f32, y: f32, z: f32, along: f32| m.vertex([x, y, z], n, [z / uv_scale, -along / uv_scale], WHITE);
        let a = v(&mut m, side * ex, ey, zf, 0.0);
        let b = v(&mut m, side * ex, ey, -zf, 0.0);
        let c = v(&mut m, 0.0, top, -zf, slope);
        let e = v(&mut m, 0.0, top, zf, slope);
        // Wind the quad so it faces the way its normal does.
        let p = |i: u32| m.positions[i as usize];
        let facing = cross(sub(p(b), p(a)), sub(p(c), p(a)));
        if facing[0] * n[0] + facing[1] * n[1] > 0.0 {
            m.quad(a, b, c, e);
        } else {
            m.quad(a, e, c, b);
        }
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
