//! Minnesota's winter flora, built from code: the conifers (white pine, red
//! pine, balsam fir, white spruce), the bare hardwoods (paper birch, quaking
//! aspen), tamarack, and the undergrowth (red osier dogwood, staghorn sumac,
//! juniper, prairie grass, cattails and reeds), plus where each grows.
//!
//! Trees are a tapered trunk, branch stems, and "cards": small textured
//! quads carrying a spray of needles or twigs, the usual way real-time trees
//! get their detail. Card vertex colours tint the near-white textures per
//! species, and snow cards rest on top of the branches. Card normals point
//! out from the trunk so the crown is lit like one soft mass, not as flat
//! sheets.

use std::f32::consts::{PI, TAU};

use super::meshgen::{add, cross, lathe, normalize, scale, sub, MeshData, V3, WHITE};
use super::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Species {
    WhitePine,
    RedPine,
    BalsamFir,
    WhiteSpruce,
    PaperBirch,
    QuakingAspen,
    Tamarack,
}

impl Species {
    pub const ALL: [Species; 7] = [
        Species::WhitePine,
        Species::RedPine,
        Species::BalsamFir,
        Species::WhiteSpruce,
        Species::PaperBirch,
        Species::QuakingAspen,
        Species::Tamarack,
    ];

    /// Which card texture its foliage uses.
    pub fn card(self) -> Card {
        match self {
            Species::WhitePine | Species::RedPine => Card::Pine,
            Species::BalsamFir | Species::WhiteSpruce => Card::Fir,
            Species::PaperBirch | Species::QuakingAspen => Card::Twigs,
            Species::Tamarack => Card::Tamarack,
        }
    }

    /// Which bark it wears.
    pub fn bark(self) -> Bark {
        match self {
            Species::RedPine => Bark::RedPine,
            Species::PaperBirch => Bark::Birch,
            Species::QuakingAspen => Bark::Aspen,
            _ => Bark::Pine,
        }
    }

    /// Typical height range in metres for the trees we place.
    pub fn heights(self) -> (f32, f32) {
        match self {
            Species::WhitePine => (13.0, 19.0),
            Species::RedPine => (12.0, 17.0),
            Species::BalsamFir => (7.0, 11.0),
            Species::WhiteSpruce => (9.0, 14.0),
            Species::PaperBirch => (9.0, 14.0),
            Species::QuakingAspen => (10.0, 15.0),
            Species::Tamarack => (8.0, 12.0),
        }
    }

    /// Trunk radius at the base for a tree `height` tall.
    pub fn trunk_radius(self, height: f32) -> f32 {
        let k = match self {
            Species::WhitePine | Species::RedPine => 0.024,
            Species::PaperBirch | Species::QuakingAspen => 0.016,
            _ => 0.02,
        };
        (height * k).max(0.08)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Card {
    Pine,
    Fir,
    Twigs,
    Tamarack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bark {
    Pine,
    RedPine,
    Birch,
    Aspen,
}

/// A finished tree: bark (trunk and branches), foliage cards and snow cards.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub bark: MeshData,
    pub foliage: MeshData,
    pub snow: MeshData,
}

fn rgba(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

fn jitter(rng: &mut Rng, c: [f32; 3], amount: f32) -> [f32; 4] {
    let k = 1.0 + rng.range(-amount, amount);
    rgba([(c[0] * k).min(1.0), (c[1] * k).min(1.0), (c[2] * k).min(1.0)])
}

/// A tube that narrows from `r0` to `r1` along `path`, with UVs in metres
/// (so bark textures keep their size): u around, v along.
pub fn tapered_tube(path: &[V3], r0: f32, r1: f32, sides: usize, uv_scale: f32) -> MeshData {
    let mut m = MeshData::default();
    let mut along = 0.0;
    for (i, &p) in path.iter().enumerate() {
        if i > 0 {
            along += super::meshgen::length(sub(p, path[i - 1]));
        }
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(path.len() - 1)];
        let t = normalize(sub(b, a));
        let helper = if t[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = normalize(cross(t, helper));
        let v = cross(u, t);
        let f = i as f32 / (path.len() - 1).max(1) as f32;
        let r = r0 + (r1 - r0) * f;
        for j in 0..=sides {
            let th = j as f32 / sides as f32 * TAU;
            let n = add(scale(u, th.cos()), scale(v, th.sin()));
            m.vertex(add(p, scale(n, r)), n, [th * r0.max(0.05) / uv_scale, -along / uv_scale], WHITE);
        }
    }
    let row = sides as u32 + 1;
    for i in 0..path.len() as u32 - 1 {
        for j in 0..sides as u32 {
            let a = i * row + j;
            m.quad(a, a + row, a + row + 1, a + 1);
        }
    }
    m
}

/// A card from `base` along `dir` (unit), `width` across `side` (unit).
/// Texture v runs from 1 at the base to 0 at the tip.
fn card(m: &mut MeshData, base: V3, dir: V3, side: V3, length: f32, width: f32, color: [f32; 4]) {
    let tip = add(base, scale(dir, length));
    let hw = scale(side, width * 0.5);
    let n = normalize(cross(side, dir));
    let s = m.positions.len() as u32;
    m.vertex(sub(base, hw), n, [0.0, 1.0], color);
    m.vertex(add(base, hw), n, [1.0, 1.0], color);
    m.vertex(add(tip, hw), n, [1.0, 0.0], color);
    m.vertex(sub(tip, hw), n, [0.0, 0.0], color);
    m.quad(s, s + 1, s + 2, s + 3);
}

/// A horizontal unit vector square to `dir` (or X if `dir` is vertical).
fn flat_side(dir: V3) -> V3 {
    let s = cross([0.0, 1.0, 0.0], dir);
    if super::meshgen::length(s) < 1e-4 {
        [1.0, 0.0, 0.0]
    } else {
        normalize(s)
    }
}

/// Rotate `v` about unit axis `k` by `a` (Rodrigues).
fn rotate(v: V3, k: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    add(add(scale(v, c), scale(cross(k, v), s)), scale(k, super::meshgen::dot(k, v) * (1.0 - c)))
}

/// Light the crown as one rounded mass: normals point out from the trunk
/// and upwards.
fn soften_normals(m: &mut MeshData, height: f32) {
    for (p, n) in m.positions.iter().zip(m.normals.iter_mut()) {
        let r = (p[0] * p[0] + p[2] * p[2]).sqrt().max(1e-3);
        let up = 0.55 + 0.4 * (p[1] / height).clamp(0.0, 1.0);
        *n = normalize([p[0] / r, up, p[2] / r]);
    }
}

/// How a conifer's crown is shaped.
struct ConiferShape {
    /// Branches start this far up the trunk (share of height).
    crown_start: f32,
    /// Crown radius (share of height) at height share `t` within the crown.
    radius: fn(f32) -> f32,
    whorl_spacing: f32,
    branches: usize,
    /// Branch pitch: positive points upwards.
    pitch: f32,
    /// How far tips sag (radians over the branch length).
    droop: f32,
    /// Chance a branch is missing (irregular old pines).
    gaps: f32,
    /// Cards only on the outer part of each branch (pine tufts).
    tufts: bool,
    needles: [f32; 3],
    card_len: f32,
}

fn conifer_shape(s: Species) -> ConiferShape {
    match s {
        Species::BalsamFir => ConiferShape {
            crown_start: 0.07,
            radius: |t| 0.2 * (1.0 - t).powf(1.05) + 0.02,
            whorl_spacing: 0.36,
            branches: 6,
            pitch: 0.12,
            droop: 0.15,
            gaps: 0.05,
            tufts: false,
            needles: [0.1, 0.2, 0.12],
            card_len: 0.7,
        },
        Species::WhiteSpruce => ConiferShape {
            crown_start: 0.05,
            radius: |t| 0.24 * (1.0 - t).powf(0.95) + 0.02,
            whorl_spacing: 0.4,
            branches: 7,
            pitch: -0.25,
            droop: -0.35,
            gaps: 0.05,
            tufts: false,
            needles: [0.12, 0.2, 0.17],
            card_len: 0.75,
        },
        Species::WhitePine => ConiferShape {
            crown_start: 0.32,
            radius: |t| 0.22 * (1.0 - t).powf(0.55) * (0.85 + 0.15 * (t * 9.0).sin()),
            whorl_spacing: 0.9,
            branches: 5,
            pitch: 0.05,
            droop: 0.12,
            gaps: 0.3,
            tufts: true,
            needles: [0.15, 0.25, 0.2],
            card_len: 0.7,
        },
        Species::RedPine => ConiferShape {
            crown_start: 0.52,
            radius: |t| 0.16 * (t * PI).sin().powf(0.6) + 0.03,
            whorl_spacing: 0.75,
            branches: 5,
            pitch: 0.35,
            droop: 0.2,
            gaps: 0.2,
            tufts: true,
            needles: [0.17, 0.27, 0.12],
            card_len: 0.65,
        },
        // Tamarack: a narrow cone of bare twigs and the last golden tufts.
        _ => ConiferShape {
            crown_start: 0.12,
            radius: |t| 0.18 * (1.0 - t).powf(0.9) + 0.02,
            whorl_spacing: 0.45,
            branches: 5,
            pitch: 0.05,
            droop: 0.25,
            gaps: 0.15,
            tufts: false,
            needles: [0.62, 0.46, 0.24],
            card_len: 0.85,
        },
    }
}

/// A conifer (or tamarack) `height` tall. `snow` 0..1 sets how laden it is.
fn conifer(species: Species, height: f32, snow: f32, seed: u64) -> Tree {
    let mut rng = Rng::new(seed);
    let shape = conifer_shape(species);
    let mut tree = Tree::default();
    let r0 = species.trunk_radius(height);
    // Trunk with a slight lean and wobble.
    let lean = [rng.range(-0.02, 0.02), 0.0, rng.range(-0.02, 0.02)];
    let trunk_path: Vec<V3> = (0..=6)
        .map(|k| {
            let t = k as f32 / 6.0;
            let y = -0.3 + (height * 0.98 + 0.3) * t;
            [lean[0] * y + rng.range(-0.03, 0.03) * t, y, lean[2] * y + rng.range(-0.03, 0.03) * t]
        })
        .collect();
    tree.bark.append(&tapered_tube(&trunk_path, r0, r0 * 0.12, 8, 1.2));
    let axis = |y: f32| -> V3 { [lean[0] * y, y, lean[2] * y] };

    let crown0 = height * shape.crown_start;
    let mut y = crown0;
    let mut whorl = 0;
    while y < height * 0.96 {
        let t = (y - crown0) / (height - crown0).max(0.1);
        let radius = (shape.radius)(t) * height;
        let twist = rng.range(0.0, TAU) + whorl as f32 * 0.7;
        for b in 0..shape.branches {
            if rng.chance(shape.gaps) {
                continue;
            }
            let yaw = twist + b as f32 / shape.branches as f32 * TAU + rng.range(-0.25, 0.25);
            let len = radius * rng.range(0.75, 1.15);
            if len < 0.12 {
                continue;
            }
            let pitch = shape.pitch + rng.range(-0.1, 0.1);
            let out = [yaw.cos(), 0.0, -yaw.sin()];
            // The branch: a gentle arc from the trunk, sagging (or curling up) at the tip.
            let start = axis(y);
            let mut pts = vec![start];
            let mut dir = normalize([out[0] * pitch.cos(), pitch.sin(), out[2] * pitch.cos()]);
            let steps = 4;
            for k in 1..=steps {
                let f = k as f32 / steps as f32;
                dir = normalize(add(dir, [0.0, -shape.droop * 0.6 / steps as f32 * (1.0 + f), 0.0]));
                let p = add(*pts.last().unwrap(), scale(dir, len / steps as f32));
                pts.push(p);
            }
            let br = (r0 * 0.35 * (1.0 - t * 0.7)).max(0.012);
            // Mostly hidden by needles, so a light three-sided stem will do.
            tree.bark.append(&tapered_tube(&[pts[0], pts[2], pts[4]], br, br * 0.3, 3, 0.8));
            // Needle cards along the branch (only near the tip for pines).
            // Big trees carry bigger sprays; pines carry many along the outer branch.
            let card_len = shape.card_len * (height / 10.0).max(1.0);
            let from = if shape.tufts { 0.3 } else { 0.1 };
            let cards = if shape.tufts {
                ((len * (1.0 - from) / (card_len * 0.4)).ceil() as usize).clamp(3, 9)
            } else {
                ((len / (card_len * 0.55)).ceil() as usize).clamp(2, 4)
            };
            for c in 0..cards {
                let f = from + (1.0 - from) * c as f32 / cards as f32;
                let seg = ((f * steps as f32) as usize).min(steps - 1);
                let local = f * steps as f32 - seg as f32;
                let base = add(pts[seg], scale(sub(pts[seg + 1], pts[seg]), local));
                let bdir = normalize(sub(pts[seg + 1], pts[seg]));
                let clen = (card_len * rng.range(0.8, 1.2)).min(len * (1.0 - f) * 1.1 + 0.25);
                let cdir = normalize(add(bdir, [rng.range(-0.15, 0.15), rng.range(-0.05, 0.15), rng.range(-0.15, 0.15)]));
                let side = flat_side(cdir);
                let color = jitter(&mut rng, shape.needles, 0.18);
                let w = clen * if shape.tufts { 0.55 } else { 0.8 };
                // Two crossed cards: one flat, one tipped up.
                card(&mut tree.foliage, base, cdir, side, clen, w, color);
                card(&mut tree.foliage, base, cdir, rotate(side, cdir, 1.1), clen, w * 0.8, color);
                if !shape.tufts {
                    // Fir and spruce sprays are full: a third card tipped the other way.
                    card(&mut tree.foliage, base, cdir, rotate(side, cdir, -1.1), clen, w * 0.8, color);
                }
                if shape.tufts {
                    // Pine tufts splay upwards like a brush.
                    let up = normalize(add(cdir, [0.0, 0.8, 0.0]));
                    card(&mut tree.foliage, base, up, flat_side(up), clen * 0.7, w, color);
                }
                // Snow resting on top of the spray.
                if rng.chance(snow * 0.85) {
                    let lift = [0.0, 0.06, 0.0];
                    let sl = clen * rng.range(0.55, 0.85);
                    card(&mut tree.snow, add(base, lift), normalize([cdir[0], cdir[1] * 0.3, cdir[2]]), side, sl, w * 0.9, WHITE);
                }
            }
        }
        whorl += 1;
        y += shape.whorl_spacing * rng.range(0.8, 1.2) * if species == Species::WhitePine { 1.0 + t } else { 1.0 };
    }
    // The leader at the very top.
    let top = axis(height * 0.95);
    let color = jitter(&mut rng, shape.needles, 0.1);
    for k in 0..3 {
        let side = rotate([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], k as f32 * PI / 3.0);
        card(&mut tree.foliage, add(top, [0.0, -0.4, 0.0]), [0.0, 1.0, 0.0], side, shape.card_len * 1.2, shape.card_len * 0.45, color);
    }
    soften_normals(&mut tree.foliage, height);
    for n in tree.snow.normals.iter_mut() {
        *n = normalize(add(*n, [0.0, 2.0, 0.0]));
    }
    tree
}

/// Paper birch or quaking aspen: a bare trunk (birch often grows as a clump
/// of two or three), forking branches, and twig cards at the ends.
fn hardwood(species: Species, height: f32, snow: f32, seed: u64) -> Tree {
    let mut rng = Rng::new(seed);
    let mut tree = Tree::default();
    let stems = if species == Species::PaperBirch { 1 + (rng.f32() * 2.6) as usize } else { 1 };
    let twig_color = if species == Species::PaperBirch { [0.38, 0.22, 0.17] } else { [0.5, 0.5, 0.46] };
    for s in 0..stems {
        let h = height * if s == 0 { 1.0 } else { rng.range(0.7, 0.92) };
        let r0 = species.trunk_radius(h) * if s == 0 { 1.0 } else { 0.8 };
        let lean_a = rng.range(0.0, TAU);
        let lean = if stems > 1 { rng.range(0.05, 0.14) } else { rng.range(0.0, 0.04) };
        let off = if stems > 1 { [lean_a.cos() * 0.18, 0.0, -lean_a.sin() * 0.18] } else { [0.0; 3] };
        let lean_v = [lean_a.cos() * lean, 0.0, -lean_a.sin() * lean];
        let path: Vec<V3> = (0..=7)
            .map(|k| {
                let y = -0.3 + (h + 0.3) * k as f32 / 7.0;
                let wob = [rng.range(-0.06, 0.06), 0.0, rng.range(-0.06, 0.06)];
                add(add(off, scale(lean_v, y)), add([0.0, y, 0.0], scale(wob, (k > 0) as u8 as f32)))
            })
            .collect();
        tree.bark.append(&tapered_tube(&path, r0, r0 * 0.15, 8, 1.0));
        // Branches from the upper 60% of the stem, forking once.
        let n_branches = 7 + (rng.f32() * 5.0) as usize;
        for b in 0..n_branches {
            let f = rng.range(0.38, 0.93);
            let seg = ((f * 7.0) as usize).min(6);
            let local = f * 7.0 - seg as f32;
            let start = add(path[seg], scale(sub(path[seg + 1], path[seg]), local));
            let yaw = b as f32 * 2.4 + rng.range(-0.4, 0.4);
            let out = [yaw.cos(), 0.0, -yaw.sin()];
            let pitch = rng.range(0.6, 1.1) - f * 0.2;
            let len = h * rng.range(0.16, 0.26) * (1.15 - f * 0.6);
            let dir = normalize([out[0] * pitch.cos(), pitch.sin(), out[2] * pitch.cos()]);
            let mid = add(start, add(scale(dir, len * 0.5), [0.0, len * 0.05, 0.0]));
            let end = add(mid, scale(normalize(add(dir, [0.0, 0.25, 0.0])), len * 0.5));
            let br = (r0 * 0.3 * (1.1 - f)).max(0.015);
            tree.bark.append(&tapered_tube(&[start, mid, end], br, br * 0.35, 5, 0.6));
            // A fork, and twig cards at both ends.
            let fork_dir = normalize(add(rotate(dir, [0.0, 1.0, 0.0], rng.range(-0.9, 0.9)), [0.0, 0.3, 0.0]));
            let fork_end = add(mid, scale(fork_dir, len * 0.45));
            tree.bark.append(&tapered_tube(&[mid, fork_end], br * 0.6, br * 0.25, 4, 0.6));
            for (p, d) in [(end, normalize(add(dir, [0.0, 0.4, 0.0]))), (fork_end, fork_dir), (mid, normalize(add(dir, [0.0, 0.8, 0.0])))] {
                let clen = h * rng.range(0.1, 0.15);
                let color = jitter(&mut rng, twig_color, 0.15);
                let base = sub(p, scale(d, clen * 0.25));
                card(&mut tree.foliage, base, d, flat_side(d), clen, clen * 0.6, color);
                card(&mut tree.foliage, base, d, rotate(flat_side(d), d, 1.2), clen, clen * 0.6, color);
            }
            if rng.chance(snow * 0.5) {
                let sl = len * 0.4;
                card(&mut tree.snow, add(start, add(scale(dir, len * 0.15), [0.0, br + 0.02, 0.0])), normalize([dir[0], 0.05, dir[2]]), flat_side(dir), sl, (br * 6.0).max(0.12), WHITE);
            }
        }
        // Crown twigs at the top of the stem.
        let top = path[7];
        for k in 0..4 {
            let d = normalize(add([0.0, 1.0, 0.0], scale([(k as f32 * 1.6).cos(), 0.0, (k as f32 * 1.6).sin()], 0.6)));
            let clen = h * 0.13;
            let color = jitter(&mut rng, twig_color, 0.15);
            card(&mut tree.foliage, sub(top, scale(d, clen * 0.4)), d, flat_side(d), clen, clen * 0.6, color);
        }
    }
    soften_normals(&mut tree.foliage, height);
    for n in tree.snow.normals.iter_mut() {
        *n = normalize(add(*n, [0.0, 2.0, 0.0]));
    }
    tree
}

/// A tree of `species`, about `height` tall, base at the origin.
pub fn tree(species: Species, height: f32, snow: f32, seed: u64) -> Tree {
    match species {
        Species::PaperBirch | Species::QuakingAspen => hardwood(species, height, snow, seed),
        _ => conifer(species, height, snow, seed),
    }
}

// ---------------------------------------------------------------------------
// Undergrowth
// ---------------------------------------------------------------------------

/// Red osier dogwood: a clump of slender, bright red stems standing out of
/// the snow, some forking near the top. Colours are in the vertex colours.
pub fn dogwood(seed: u64) -> MeshData {
    let mut rng = Rng::new(seed);
    let mut m = MeshData::default();
    for _ in 0..(14 + (rng.f32() * 10.0) as usize) {
        let a = rng.range(0.0, TAU);
        let base = [a.cos() * rng.range(0.0, 0.35), -0.1, -a.sin() * rng.range(0.0, 0.35)];
        let h = rng.range(0.8, 1.9);
        let lean = [a.cos() * rng.range(0.05, 0.3), 0.0, -a.sin() * rng.range(0.05, 0.3)];
        let mid = add(base, add([0.0, h * 0.5, 0.0], scale(lean, 0.4)));
        let top = add(base, add([0.0, h, 0.0], lean));
        let red = [rng.range(0.55, 0.75), rng.range(0.06, 0.12), rng.range(0.05, 0.09), 1.0];
        let mut stem = tapered_tube(&[base, mid, top], 0.014, 0.005, 4, 1.0);
        if rng.chance(0.5) {
            let fork = add(mid, add(scale(lean, 0.8), [rng.range(-0.2, 0.2), h * 0.4, rng.range(-0.2, 0.2)]));
            stem.append(&tapered_tube(&[mid, fork], 0.009, 0.004, 4, 1.0));
        }
        m.append(&stem.tinted(red));
    }
    m
}

/// Staghorn sumac: a few thick, forking stems with dark red, fuzzy seed
/// cones at the tips. Returns (stems, cones).
pub fn sumac(seed: u64) -> (MeshData, MeshData) {
    let mut rng = Rng::new(seed);
    let mut stems = MeshData::default();
    let mut cones = MeshData::default();
    let cone = lathe(&[(0.0, 0.0), (0.045, 0.03), (0.05, 0.1), (0.035, 0.18), (0.0, 0.24)], 8, 0.3, false, false);
    for _ in 0..(3 + (rng.f32() * 4.0) as usize) {
        let a = rng.range(0.0, TAU);
        let base = [a.cos() * rng.range(0.0, 0.6), -0.1, -a.sin() * rng.range(0.0, 0.6)];
        let h = rng.range(1.0, 2.2);
        let fork = add(base, [a.cos() * 0.15, h * 0.6, -a.sin() * 0.15]);
        stems.append(&tapered_tube(&[base, fork], 0.035, 0.025, 6, 0.5).tinted([0.36, 0.3, 0.26, 1.0]));
        for k in 0..(2 + (rng.f32() * 2.0) as usize) {
            let b = a + (k as f32 - 0.8) * 0.9;
            let tip = add(fork, [b.cos() * rng.range(0.25, 0.5), h * rng.range(0.3, 0.45), -b.sin() * rng.range(0.25, 0.5)]);
            stems.append(&tapered_tube(&[fork, tip], 0.022, 0.014, 5, 0.5).tinted([0.36, 0.3, 0.26, 1.0]));
            let red = [rng.range(0.42, 0.55), rng.range(0.06, 0.1), rng.range(0.06, 0.09), 1.0];
            cones.append(&cone.clone().translated(tip).tinted(red));
        }
    }
    (stems, cones)
}

/// Common juniper: a low, spreading mound of prickly blue-green sprays with
/// snow sitting on it. Returns (foliage cards, snow cards).
pub fn juniper(seed: u64) -> (MeshData, MeshData) {
    let mut rng = Rng::new(seed);
    let mut foliage = MeshData::default();
    let mut snow = MeshData::default();
    for k in 0..34 {
        let yaw = k as f32 * 2.4 + rng.range(-0.3, 0.3);
        let pitch = rng.range(0.3, 1.0);
        let dir = normalize([yaw.cos() * pitch.cos(), pitch.sin(), -yaw.sin() * pitch.cos()]);
        let base = [rng.range(-0.25, 0.25), rng.range(0.0, 0.2), rng.range(-0.25, 0.25)];
        let len = rng.range(0.6, 1.1);
        let color = jitter(&mut rng, [0.11, 0.2, 0.19], 0.2);
        card(&mut foliage, base, dir, flat_side(dir), len, len * 0.5, color);
        card(&mut foliage, base, dir, rotate(flat_side(dir), dir, 1.2), len, len * 0.45, color);
        if rng.chance(0.6) {
            card(&mut snow, add(base, [0.0, 0.08, 0.0]), normalize([dir[0], dir[1] * 0.3, dir[2]]), flat_side(dir), len * 0.7, len * 0.45, WHITE);
        }
    }
    soften_normals(&mut foliage, 0.8);
    (foliage, snow)
}

/// A tuft of prairie grass (big bluestem in coppery winter colour, or pale
/// little bluestem): three crossed grass cards.
pub fn grass_tuft(seed: u64) -> MeshData {
    let mut rng = Rng::new(seed);
    let mut m = MeshData::default();
    let h = rng.range(0.5, 1.2);
    let color = if rng.chance(0.6) { jitter(&mut rng, [0.62, 0.42, 0.26], 0.15) } else { jitter(&mut rng, [0.72, 0.62, 0.45], 0.15) };
    let tilt = [rng.range(-0.15, 0.15), 1.0, rng.range(-0.15, 0.15)];
    let up = normalize(tilt);
    for k in 0..3 {
        let side = rotate([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], k as f32 * PI / 3.0 + rng.range(0.0, 0.5));
        card(&mut m, [0.0, -0.08, 0.0], up, side, h, h * 1.1, color);
    }
    for n in m.normals.iter_mut() {
        *n = [0.0, 1.0, 0.0];
    }
    m
}

/// Cattails at the water's edge: stems, strap leaves and brown seed heads
/// (some burst into fluff). Returns (stems and leaves, heads).
pub fn cattails(seed: u64) -> (MeshData, MeshData) {
    let mut rng = Rng::new(seed);
    let mut stalks = MeshData::default();
    let mut heads = MeshData::default();
    let head = lathe(&[(0.0, 0.0), (0.028, 0.015), (0.03, 0.2), (0.026, 0.23), (0.0, 0.25)], 8, 0.3, false, false);
    let fluff = lathe(&[(0.0, 0.0), (0.05, 0.03), (0.06, 0.15), (0.04, 0.22), (0.0, 0.25)], 8, 0.3, false, false);
    let tan = [0.62, 0.52, 0.36, 1.0];
    for _ in 0..(8 + (rng.f32() * 10.0) as usize) {
        let base = [rng.range(-0.6, 0.6), -0.1, rng.range(-0.6, 0.6)];
        let h = rng.range(1.1, 1.9);
        let top = add(base, [rng.range(-0.12, 0.12), h, rng.range(-0.12, 0.12)]);
        stalks.append(&tapered_tube(&[base, top], 0.008, 0.005, 4, 1.0).tinted(tan));
        if rng.chance(0.75) {
            let at = sub(top, [0.0, 0.4, 0.0]);
            let burst = rng.chance(0.3);
            let brown = if burst { [0.8, 0.76, 0.66, 1.0] } else { [0.32, 0.2, 0.12, 1.0] };
            heads.append(&(if burst { fluff.clone() } else { head.clone() }).translated(at).tinted(brown));
        }
        // A dry strap leaf, bent over part way up.
        let yaw = rng.range(0.0, TAU);
        let out = [yaw.cos(), 0.0, -yaw.sin()];
        let bend = add(base, add([0.0, h * 0.55, 0.0], scale(out, 0.08)));
        let tip = add(bend, add(scale(out, h * 0.35), [0.0, -h * 0.15, 0.0]));
        let side = flat_side(out);
        let leaf_w = 0.03;
        for (a, b) in [(base, bend), (bend, tip)] {
            let d = sub(b, a);
            let len = super::meshgen::length(d);
            card(&mut stalks, a, normalize(d), side, len, leaf_w, [0.68, 0.58, 0.4, 1.0]);
        }
    }
    (stalks, heads)
}

/// Reeds (phragmites): tall thin stems with feathery plume cards on top.
/// Returns (stems, plume cards).
pub fn reeds(seed: u64) -> (MeshData, MeshData) {
    let mut rng = Rng::new(seed);
    let mut stems = MeshData::default();
    let mut plumes = MeshData::default();
    for _ in 0..(10 + (rng.f32() * 10.0) as usize) {
        let base = [rng.range(-0.7, 0.7), -0.1, rng.range(-0.7, 0.7)];
        let h = rng.range(1.6, 2.6);
        let top = add(base, [rng.range(-0.2, 0.2), h, rng.range(-0.2, 0.2)]);
        let color = jitter(&mut rng, [0.7, 0.62, 0.45], 0.1);
        stems.append(&tapered_tube(&[base, top], 0.007, 0.004, 4, 1.0).tinted(color));
        let d = normalize(sub(top, base));
        let side = rotate([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], rng.range(0.0, PI));
        card(&mut plumes, sub(top, scale(d, 0.15)), d, side, 0.5, 0.14, color);
    }
    (stems, plumes)
}

// ---------------------------------------------------------------------------
// Where things grow
// ---------------------------------------------------------------------------

fn hash(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (z as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

fn smooth_noise(x: f32, z: f32) -> f32 {
    let (ix, iz) = (x.floor(), z.floor());
    let (fx, fz) = (x - ix, z - iz);
    let (ux, uz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (i, k) = (ix as i32, iz as i32);
    let a = hash(i, k) + (hash(i + 1, k) - hash(i, k)) * ux;
    let b = hash(i, k + 1) + (hash(i + 1, k + 1) - hash(i, k + 1)) * ux;
    a + (b - a) * uz
}

/// 0 in conifer forest, 1 in birch and aspen woods, varying over ~60 m.
pub fn hardwood_share(x: f32, z: f32) -> f32 {
    let n = smooth_noise(x / 60.0 + 3.1, z / 60.0 + 7.7) * 0.7 + smooth_noise(x / 23.0, z / 23.0) * 0.3;
    ((n - 0.42) * 3.0).clamp(0.0, 1.0)
}

/// Pick a species for a spot. `roll` is a uniform 0..1 draw, `near_water`
/// is how close the nearest lake shore is (0 at the shore, 1 far away).
pub fn pick_species(x: f32, z: f32, near_water: f32, roll: f32) -> Species {
    // Tamarack and spruce like the wet ground by lakes.
    if near_water < 0.3 {
        return match roll {
            r if r < 0.45 => Species::Tamarack,
            r if r < 0.8 => Species::WhiteSpruce,
            _ => Species::BalsamFir,
        };
    }
    if roll < hardwood_share(x, z) * 0.92 {
        let r = (roll / (hardwood_share(x, z) * 0.92).max(1e-3)).clamp(0.0, 1.0);
        return if r < 0.5 { Species::PaperBirch } else { Species::QuakingAspen };
    }
    let r = (roll * 7.31).fract();
    match r {
        r if r < 0.28 => Species::BalsamFir,
        r if r < 0.5 => Species::WhiteSpruce,
        r if r < 0.78 => Species::WhitePine,
        r if r < 0.95 => Species::RedPine,
        _ => Species::Tamarack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_species_builds_a_sensible_tree() {
        for (i, s) in Species::ALL.into_iter().enumerate() {
            let (lo, hi) = s.heights();
            let h = (lo + hi) * 0.5;
            let t = tree(s, h, 1.0, 10 + i as u64);
            assert!(t.bark.is_valid() && t.foliage.is_valid(), "{s:?}");
            let (blo, bhi) = t.bark.bounds();
            assert!(blo[1] < 0.0, "{s:?}: trunk goes into the ground");
            assert!(bhi[1] > h * 0.85 && bhi[1] < h * 1.15, "{s:?}: about {h} m tall, got {}", bhi[1]);
            let (flo, fhi) = t.foliage.bounds();
            let spread = (fhi[0] - flo[0]).max(fhi[2] - flo[2]);
            assert!(spread > h * 0.15 && spread < h * 0.9, "{s:?}: crown {spread} m wide");
            assert!(t.foliage.triangle_count() < 6000, "{s:?}: {} triangles", t.foliage.triangle_count());
            if s != Species::PaperBirch && s != Species::QuakingAspen {
                assert!(t.snow.is_valid(), "{s:?} carries snow");
            }
        }
    }

    #[test]
    fn crowns_have_their_species_shape() {
        let width_at = |t: &Tree, y0: f32, y1: f32| {
            t.foliage
                .positions
                .iter()
                .filter(|p| p[1] > y0 && p[1] < y1)
                .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
                .fold(0.0, f32::max)
        };
        // Balsam fir: a spire, branches right down near the ground.
        let fir = tree(Species::BalsamFir, 10.0, 1.0, 1);
        assert!(width_at(&fir, 0.5, 2.0) > width_at(&fir, 7.0, 9.0) * 2.0);
        // Red pine: a bare trunk below a high crown.
        let red = tree(Species::RedPine, 15.0, 1.0, 2);
        assert_eq!(width_at(&red, 0.5, 6.0), 0.0, "no needles low on a red pine");
        assert!(width_at(&red, 9.0, 14.0) > 1.5);
    }

    #[test]
    fn snow_cards_sit_on_top_and_face_up() {
        let t = tree(Species::WhiteSpruce, 11.0, 1.0, 5);
        assert!(t.snow.normals.iter().all(|n| n[1] > 0.5));
        assert!(t.snow.triangle_count() > 20);
        let bare = tree(Species::WhiteSpruce, 11.0, 0.0, 5);
        assert!(bare.snow.positions.is_empty());
    }

    #[test]
    fn undergrowth_is_valid_and_low() {
        let d = dogwood(3);
        assert!(d.is_valid());
        assert!(d.colors.iter().all(|c| c[0] > c[1] * 3.0), "dogwood stems are red");
        let (st, co) = sumac(4);
        assert!(st.is_valid() && co.is_valid());
        let (jf, js) = juniper(5);
        assert!(jf.is_valid() && js.is_valid());
        assert!(jf.bounds().1[1] < 1.3, "juniper stays low");
        assert!(grass_tuft(6).is_valid());
        let (cs, ch) = cattails(7);
        assert!(cs.is_valid() && ch.is_valid());
        let (rs, rp) = reeds(8);
        assert!(rs.is_valid() && rp.is_valid());
    }

    #[test]
    fn woods_change_with_place_and_water() {
        let mut hard = 0;
        let mut soft = 0;
        for i in 0..400 {
            let (x, z) = ((i % 20) as f32 * 19.0 - 190.0, (i / 20) as f32 * 19.0 - 190.0);
            let s = pick_species(x, z, 1.0, (i as f32 * 0.618).fract());
            if matches!(s, Species::PaperBirch | Species::QuakingAspen) {
                hard += 1;
            } else {
                soft += 1;
            }
        }
        assert!(hard > 40 && soft > 150, "a mixed forest: {hard} hardwoods, {soft} conifers");
        for k in 0..20 {
            let s = pick_species(0.0, 0.0, 0.1, k as f32 / 20.0);
            assert!(matches!(s, Species::Tamarack | Species::WhiteSpruce | Species::BalsamFir));
        }
    }
}
