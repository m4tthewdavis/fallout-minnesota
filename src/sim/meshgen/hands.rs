//! The first-person hands: a leather winter glove (in any grip from open to
//! a fist) and the jumpsuit sleeve over the forearm.
//!
//! The glove is one skin of overlapping parts in the same leather: a palm
//! that is wide and thin at the knuckles and narrows to the wrist, the two
//! pads at the heel of the hand, four tapering fingers with a swell at each
//! joint, a thumb growing out of its mound, a row of knuckles, the three
//! stitched lines down the back that every leather glove has, a strap round
//! the wrist and a snug cuff that runs up inside the sleeve. Everything is
//! sized in gun space, like the weapons, and scaled with them.

use super::*;

/// Tints over the scanned leather: the hand, and the darker cuff and strap.
const LEATHER: [f32; 4] = [0.60, 0.50, 0.44, 1.0];
const LEATHER_DARK: [f32; 4] = [0.36, 0.30, 0.27, 1.0];
const STITCH: [f32; 4] = [0.22, 0.18, 0.16, 1.0];
/// Leather's texture repeats over this many metres.
const LEATHER_UV: f32 = 0.09;

/// One cross-section of a swept limb: an ellipse centred on `c`, `ru` along
/// `u` and `rv` along `v`.
#[derive(Clone, Copy)]
struct Ring {
    c: V3,
    u: V3,
    v: V3,
    ru: f32,
    rv: f32,
}

/// A smooth skin over a row of elliptical rings (a finger, the thumb, the
/// sleeve). UVs run round the limb and along it in metres over `uv_scale`.
/// An end can be closed to a point at its last ring's centre (shrink the
/// rings towards it first for a rounded tip).
fn skin(rings: &[Ring], sides: usize, uv_scale: f32, close_start: bool, close_end: bool) -> MeshData {
    let mut m = MeshData::default();
    let mut along = 0.0;
    let girth = rings.iter().map(|r| PI * (r.ru + r.rv)).fold(0.0, f32::max);
    for (i, r) in rings.iter().enumerate() {
        if i > 0 {
            along += length(sub(r.c, rings[i - 1].c));
        }
        // The seam column is doubled so the texture can wrap.
        for j in 0..=sides {
            let th = j as f32 / sides as f32 * TAU;
            let out = add(scale(r.u, r.ru * th.cos()), scale(r.v, r.rv * th.sin()));
            m.vertex(add(r.c, out), [0.0, 1.0, 0.0], [j as f32 / sides as f32 * girth / uv_scale, along / uv_scale], WHITE);
        }
    }
    let row = sides as u32 + 1;
    for i in 0..rings.len() as u32 - 1 {
        for j in 0..sides as u32 {
            let a = i * row + j;
            m.quad(a, a + row, a + row + 1, a + 1);
        }
    }
    for (close, at, flip) in [(close_start, 0usize, true), (close_end, rings.len() - 1, false)] {
        if !close {
            continue;
        }
        let tip = m.vertex(rings[at].c, [0.0, 1.0, 0.0], [0.5 * girth / uv_scale, if at == 0 { 0.0 } else { along / uv_scale }], WHITE);
        for j in 0..sides as u32 {
            let (a, b) = (at as u32 * row + j, at as u32 * row + j + 1);
            if flip {
                m.tri(tip, b, a);
            } else {
                m.tri(tip, a, b);
            }
        }
    }
    // Face outwards whichever way the rings were wound.
    let mid = rings[rings.len() / 2];
    m.recompute_normals();
    let probe = (rings.len() / 2) * (sides + 1);
    if dot(m.normals[probe], sub(m.positions[probe], mid.c)) < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    // The doubled seam column shares one normal, so it doesn't show.
    for i in 0..rings.len() {
        let (a, b) = (i * (sides + 1), i * (sides + 1) + sides);
        let n = normalize(add(m.normals[a], m.normals[b]));
        m.normals[a] = n;
        m.normals[b] = n;
    }
    m
}

/// Rings that round a limb off beyond `end`, along `t`: a dome `r` long.
fn dome(end: Ring, t: V3, r: f32) -> Vec<Ring> {
    [0.38f32, 0.72, 1.02, 1.3, 1.5]
        .into_iter()
        .map(|a| Ring { c: add(end.c, scale(t, r * a.sin())), ru: end.ru * a.cos(), rv: end.rv * a.cos(), ..end })
        .collect()
}

/// An ellipsoid (the pads of the palm, the knuckles).
fn pad(centre: V3, radii: V3) -> MeshData {
    blob(1.0, 1.0, 0.0, 1, 1.0).scaled(radii).translated(centre)
}

/// Where each finger's knuckle is across the hand (index to little finger;
/// the thumb is on -x), and how long each finger is from that knuckle.
const FINGER_X: [f32; 4] = [-0.03, -0.01, 0.01, 0.028];
const FINGER_LEN: [f32; 4] = [0.085, 0.092, 0.086, 0.07];
/// The grip a fist closes round (see `viewmodel::FIST_CENTRE`) and the
/// radius the middle of a finger wraps it at.
const GRIP_CENTRE: V3 = [0.0, -0.024, -0.042];
const GRIP_RADIUS: f32 = 0.025;

/// The palm's cross-sections from the fingers back to the wrist: (z, centre
/// height, half-width, half-thickness, squareness). Thin where a grip lies
/// across the roots of the fingers, deep at the heel of the hand.
const PALM: [Section; 9] = [
    sec(-0.066, 0.0000, 0.0372, 0.0064, 2.4),
    sec(-0.060, 0.0012, 0.0402, 0.0104, 2.5),
    sec(-0.051, 0.0028, 0.0425, 0.0132, 2.7),
    sec(-0.038, 0.0036, 0.0445, 0.0140, 2.7),
    sec(-0.022, 0.0005, 0.0450, 0.0182, 2.6),
    sec(0.000, -0.0025, 0.0430, 0.0200, 2.5),
    sec(0.025, -0.0040, 0.0372, 0.0200, 2.4),
    sec(0.045, -0.0040, 0.0318, 0.0190, 2.3),
    sec(0.060, -0.0040, 0.0298, 0.0184, 2.3),
];

/// Height of the back of the hand at (x, z), from [`PALM`].
fn back_of_hand(x: f32, z: f32) -> f32 {
    let i = PALM.windows(2).position(|w| z <= w[1].z).unwrap_or(PALM.len() - 2);
    let (a, b) = (PALM[i], PALM[i + 1]);
    let t = ((z - a.z) / (b.z - a.z)).clamp(0.0, 1.0);
    let mix = |p: f32, q: f32| p + (q - p) * t;
    let (y, rx, ry, sq) = (mix(a.y, b.y), mix(a.rx, b.rx), mix(a.ry, b.ry), mix(a.square, b.square));
    y + ry * (1.0 - (x / rx).abs().min(1.0).powf(sq)).max(0.0).powf(1.0 / sq)
}

/// The middle of finger `k`, `s` metres from its knuckle, at grip `g`: lying
/// nearly straight (a relaxed hand curls a little) or wrapped round the grip.
fn finger_point(k: usize, s: f32, g: f32) -> V3 {
    let (x, len) = (FINGER_X[k], FINGER_LEN[k]);
    let knuckle = [x, 0.0, -0.05 + (k as f32 - 1.5).abs() * 0.003];
    let f = s / len;
    // Open fingers fan apart a little and droop towards their tips.
    let splay = (k as f32 - 1.4) * 0.0075 * f;
    let straight = [x + splay, knuckle[1] - s * 0.12 - f * f * 0.014, knuckle[2] - s];
    let phi0 = (knuckle[1] - GRIP_CENTRE[1]).atan2(-(knuckle[2] - GRIP_CENTRE[2]));
    let phi = phi0 - s / GRIP_RADIUS;
    let curled = [x, GRIP_CENTRE[1] + GRIP_RADIUS * phi.sin(), GRIP_CENTRE[2] - GRIP_RADIUS * phi.cos()];
    lerp3(straight, curled, g)
}

/// One finger: rooted inside the palm, tapering, a little swollen over each
/// joint on the back, rounded at the tip.
fn finger(k: usize, g: f32) -> MeshData {
    let len = FINGER_LEN[k];
    // The little finger is slimmer than the rest.
    let slim = if k == 3 { 0.9 } else { 1.0 };
    let tip_r = 0.0078 * slim;
    let body = len - tip_r;
    let steps = 11;
    let side = [1.0, 0.0, 0.0];
    let mut rings = Vec::new();
    let mut last_t = [0.0, 0.0, -1.0];
    for i in 0..=steps {
        let s = body * i as f32 / steps as f32;
        let f = s / len;
        let e = 0.002;
        let t = normalize(sub(finger_point(k, s + e, g), finger_point(k, (s - e).max(0.0), g)));
        let back = normalize(cross(side, t));
        // The two joints of the finger show as swells on its back, with the
        // finger a little waisted between them.
        let joint: f32 = [0.44f32, 0.74].iter().map(|j| (-((f - j) / 0.055).powi(2)).exp()).sum();
        let waist: f32 = [0.22f32, 0.6].iter().map(|j| (-((f - j) / 0.09).powi(2)).exp()).sum();
        let taper = 1.0 - 0.24 * f;
        let c = add(finger_point(k, s, g), scale(back, 0.0007 * joint));
        rings.push(Ring { c, u: side, v: back, ru: (0.0099 * taper + 0.0004 * joint - 0.0004 * waist) * slim, rv: (0.0096 * taper + 0.0011 * joint - 0.0003 * waist) * slim });
        last_t = t;
    }
    // Start a little way back inside the palm, so no root shows.
    let root = Ring { c: add(rings[0].c, [0.0, -0.0015, 0.014]), ..rings[0] };
    rings.insert(0, root);
    let end = rings[rings.len() - 1];
    rings.extend(dome(end, last_t, tip_r));
    let sides = 12;
    let mut m = skin(&rings, sides, LEATHER_UV, true, true).tinted(LEATHER);
    // The seams down both sides of a glove's finger: a darker line each.
    for i in 0..rings.len() {
        for j in [0, sides / 2, sides] {
            m.colors[i * (sides + 1) + j] = lerp4(LEATHER, STITCH, 0.55);
        }
    }
    m
}

/// The thumb: out of its mound at the heel of the hand, along the side of
/// the palm when open, across the curled fingers in a fist.
fn thumb(g: f32) -> MeshData {
    let keys = [
        [-0.024, -0.012, 0.034],
        [-0.04, -0.01, 0.015],
        [-0.055, -0.018, -0.02],
        // (In a fist it goes round the end of the grip, not through it.)
        lerp3([-0.06, -0.02, -0.055], [-0.041, -0.046, -0.054], g),
        lerp3([-0.062, -0.02, -0.08], [-0.012, -0.052, -0.058], g),
    ];
    // A smooth curve through the keys (Catmull-Rom).
    let at = |t: f32| -> V3 {
        let n = keys.len() - 1;
        let seg = (t * n as f32).floor().min(n as f32 - 1.0) as usize;
        let u = t * n as f32 - seg as f32;
        let p = |i: isize| keys[(seg as isize + i).clamp(0, n as isize) as usize];
        let (p0, p1, p2, p3) = (p(-1), p(0), p(1), p(2));
        let mut out = [0.0; 3];
        for a in 0..3 {
            out[a] = 0.5 * ((2.0 * p1[a]) + (-p0[a] + p2[a]) * u + (2.0 * p0[a] - 5.0 * p1[a] + 4.0 * p2[a] - p3[a]) * u * u + (-p0[a] + 3.0 * p1[a] - 3.0 * p2[a] + p3[a]) * u * u * u);
        }
        out
    };
    let tip_r = 0.0094;
    let steps = 12;
    // Stop a tip's length short of the last key, so the dome ends on it.
    let total = length(sub(keys[4], keys[3])).max(1e-4);
    let stop = 1.0 - 0.25 * (tip_r / total).min(0.9);
    let mut rings = Vec::new();
    let mut u = [0.0, 1.0, 0.0];
    let mut last_t = [0.0, 0.0, -1.0];
    for i in 0..=steps {
        let t01 = stop * i as f32 / steps as f32;
        let t = normalize(sub(at((t01 + 0.01).min(1.0)), at((t01 - 0.01).max(0.0))));
        // Carry the frame along the curve without twisting it.
        u = normalize(sub(u, scale(t, dot(u, t))));
        let v = cross(t, u);
        let f = i as f32 / steps as f32;
        // Thick at the mound, a waist, the knuckle, then the tapering end.
        let knuckle = (-((f - 0.62) / 0.09).powi(2)).exp();
        let r = 0.0135 - 0.0038 * f + 0.0008 * knuckle;
        rings.push(Ring { c: at(t01), u, v, ru: r, rv: r * 0.94 });
        last_t = t;
    }
    let end = rings[rings.len() - 1];
    rings.extend(dome(end, last_t, tip_r));
    skin(&rings, 10, LEATHER_UV, true, true).tinted(LEATHER)
}

/// A gloved right hand (mirror it on X for the left), gun-model sized. The
/// palm is centred on the origin, the back of the hand faces +Y, fingers
/// point along -Z and curl down round an axis along X as `grip` goes from 0
/// (open) to 1 (a fist round a ~3 cm grip centred near (0, -0.024, -0.042)).
/// The thumb is on the -X side and the cuff runs back along +Z. Every grip
/// has the same vertices in the same order.
pub fn glove(grip: f32) -> MeshData {
    let g = grip.clamp(0.0, 1.0);
    let mut m = loft_z(&PALM, 28, LEATHER_UV).tinted(LEATHER);
    // The heel of the hand: the thumb's mound and the pad under the little finger.
    // (Broad and shallow: they swell the palm rather than sit on it.)
    m.append(&pad([-0.024, -0.0105, 0.016], [0.0195, 0.0145, 0.034]).rotated_y(0.3).tinted(LEATHER));
    m.append(&pad([0.0255, -0.0105, 0.02], [0.0165, 0.013, 0.031]).tinted(LEATHER));
    for k in 0..4 {
        m.append(&finger(k, g));
        // The knuckle stands up as the fist closes.
        let z = -0.049 + (k as f32 - 1.5).abs() * 0.003;
        m.append(&pad([FINGER_X[k], 0.0088 + 0.003 * g, z + 0.004 - 0.002 * g], [0.0102, 0.0066, 0.0205]).tinted(LEATHER));
    }
    m.append(&thumb(g));
    // Three stitched lines down the back of the hand, fanning from the wrist
    // towards the gaps between the fingers.
    for x in [-0.02f32, 0.0, 0.019] {
        let line: Vec<V3> = (0..=8)
            .map(|i| {
                let f = i as f32 / 8.0;
                let (px, pz) = (x * (0.62 + 0.38 * f), 0.018 - 0.056 * f);
                [px, back_of_hand(px, pz) + 0.0004, pz]
            })
            .collect();
        m.append(&tube(&line, 0.0008, 5).tinted(lerp4(LEATHER, STITCH, 0.7)));
    }
    // The wrist: gathered in under a strap with its buckle, then a snug
    // cuff that runs up inside the sleeve and is rounded off there (so the
    // hand can turn in the sleeve's mouth like a ball in a socket).
    let wrist = [
        sec(0.056, -0.004, 0.0300, 0.0186, 2.3),
        sec(0.066, -0.004, 0.0288, 0.0178, 2.2),
        sec(0.076, -0.004, 0.0286, 0.0178, 2.2),
        sec(0.082, -0.004, 0.0262, 0.0162, 2.1),
        sec(0.087, -0.004, 0.0212, 0.0130, 2.0),
        sec(GLOVE_END, -0.004, 0.0120, 0.0074, 2.0),
    ];
    m.append(&loft_z(&wrist, 28, LEATHER_UV).tinted(LEATHER_DARK));
    let strap = [sec(0.058, -0.004, 0.0316, 0.0202, 2.3), sec(0.071, -0.004, 0.0316, 0.0202, 2.3)];
    m.append(&loft_z(&strap, 28, LEATHER_UV).tinted(STITCH));
    m.append(&cuboid([0.013, 0.0035, 0.011], LEATHER_UV).translated([0.012, 0.0165, 0.0645]).tinted([0.5, 0.5, 0.52, 1.0]));
    m
}

/// How far back the glove's cuff reaches (it ends inside the sleeve).
pub const GLOVE_END: f32 = 0.091;

/// Where the sleeve's mouth sits on the glove (hand space): just behind the
/// wrist strap.
pub const SLEEVE_MOUTH: V3 = [0.0, -0.004, 0.068];

/// How long the sleeve is in gun space: the forearm and on up the arm, far
/// enough that its end is always off the screen.
pub const SLEEVE_LEN: f32 = 1.0;

/// A Vault 143 jumpsuit sleeve over the forearm: its mouth at the origin,
/// big enough to take the glove's wrist, the elbow at z = 1 (scaled to the
/// arm's length when drawn). A plain dark cuff, the cloth gathered into it
/// in a roll, then an arm's shape (filling out over the muscle) with the
/// cloth in folds along it and deeper ones at the elbow, the vault's gold
/// band sewn round, and a stitched seam down the outside. The mouth is
/// closed a little way in, dark, as a sleeve is inside.
pub fn sleeve() -> MeshData {
    // Tints over a tan woven cloth (drawn 1.8 times as bright: see gun.rs).
    let blue = [0.064, 0.178, 1.0, 1.0];
    let gold = [1.0, 0.95, 0.3, 1.0];
    let seam = [0.03, 0.09, 0.55, 1.0];
    let cuff = [0.035, 0.085, 0.42, 1.0];
    let inside = [0.004, 0.006, 0.02, 1.0];
    /// How long the cuff is.
    const CUFF: f32 = 0.03;
    let band = (0.078, 0.112);
    // Stations along the arm (gun-space metres): close together where the
    // cloth folds, and doubled at the band's edges to keep its colour crisp.
    let mut stations: Vec<f32> = vec![0.0, 0.004, 0.009, 0.015, 0.022];
    let mut s = 0.022;
    while s < SLEEVE_LEN {
        s += if s < 0.34 { 0.005 } else { 0.03 };
        stations.push(s.min(SLEEVE_LEN));
    }
    for edge in [band.0, band.1, CUFF] {
        stations.push(edge - 0.0004);
        stations.push(edge + 0.0004);
    }
    stations.sort_by(|a, b| a.total_cmp(b));
    stations.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    let sides = 36;
    // Where the cloth's surface is, `s` along the arm and `th` round it.
    let surface = |s: f32, th: f32| -> V3 {
        // The arm inside: (half-width, half-depth), starting at a cuff
        // that takes the glove's wrist.
        let grow = |a: f32, b: f32, from: f32, to: f32| a + (b - a) * smoothstep(from, to, s);
        let rx = grow(grow(0.0346, 0.0378, 0.026, 0.05), 0.050, 0.10, 0.30) + 0.008 * smoothstep(0.3, 0.62, s);
        let ry = grow(grow(0.0256, 0.0290, 0.026, 0.05), 0.046, 0.10, 0.30) + 0.008 * smoothstep(0.3, 0.62, s);
        // The cuff's edge is turned in a little.
        let round = 1.0 - 0.05 * (1.0 - (s / 0.004).min(1.0));
        let in_band = s > band.0 && s < band.1;
        // Folds: deep where the cloth is pushed up above the glove, slight
        // along the forearm, deeper again towards the bend of the elbow.
        let depth = (0.0065 * (-(s - 0.03).max(0.0) / 0.07).exp() + 0.0026 + 0.005 * smoothstep(0.2, 0.34, s)) * smoothstep(CUFF + 0.008, CUFF + 0.03, s);
        let wave = |n: f32, turns: f32, shift: f32| (TAU * s / n + turns * th.sin() + 0.7 * (2.0 * th + shift).sin() + shift).sin();
        // Broad rounded ridges with sharp creases between them.
        let fold = 0.5 - (0.62 * wave(0.047, 1.6, 0.4) + 0.38 * wave(0.027, -2.3, 2.3)).abs().powf(0.65);
        // Above the cuff the cloth is gathered into it in a soft roll.
        let roll = 0.0042 * (-((s - 0.043) / 0.011).powi(2)).exp() * (1.0 + 0.25 * (3.0 * th + 0.5).sin());
        // The band is a strip sewn on: it lies flat and stands a little proud.
        let lift = if in_band { 0.0011 + depth * 0.2 * fold } else { depth * fold } + roll;
        // Cloth hangs: a little fuller underneath.
        let sag = 1.0 + 0.07 * (-th.sin()).max(0.0);
        [(rx + lift) * round * th.cos(), (ry + lift) * round * th.sin() * sag - 0.002, s / SLEEVE_LEN]
    };
    let mut m = MeshData::default();
    for &s in &stations {
        let in_band = s > band.0 && s < band.1;
        for j in 0..=sides {
            let th = j as f32 / sides as f32 * TAU;
            let color = if in_band {
                gold
            } else if (s - band.0).abs() < 0.003 || (s - band.1).abs() < 0.003 || (s - CUFF).abs() < 0.0025 {
                seam
            } else if s < CUFF {
                cuff
            } else {
                blue
            };
            m.vertex(surface(s, th), [0.0, 1.0, 0.0], [j as f32 / sides as f32 * 4.0, s / 0.06], color);
        }
    }
    let row = sides as u32 + 1;
    for i in 0..stations.len() as u32 - 1 {
        for j in 0..sides as u32 {
            let a = i * row + j;
            m.quad(a, a + 1, a + row + 1, a + row);
        }
    }
    // Close the mouth a little way in: a dark hollow round the glove's wrist.
    let lip: Vec<u32> = (0..=sides as u32).map(|j| m.vertex(surface(0.0, j as f32 / sides as f32 * TAU), [0.0, 0.0, -1.0], [0.0, 0.0], cuff)).collect();
    let tip = m.vertex([0.0, -0.002, 0.016 / SLEEVE_LEN], [0.0, 0.0, -1.0], [2.0, 0.0], inside);
    for j in 0..sides {
        m.tri(tip, lip[j + 1], lip[j]);
    }
    m.recompute_normals();
    if m.normals[(stations.len() / 2) * (sides + 1)][0] < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    for i in 0..stations.len() {
        let (a, b) = (i * (sides + 1), i * (sides + 1) + sides);
        let n = normalize(add(m.normals[a], m.normals[b]));
        m.normals[a] = n;
        m.normals[b] = n;
    }
    // The hollow of the mouth is dark right to its edge.
    for &v in &lip {
        m.colors[v as usize] = lerp4(cuff, inside, 0.55);
    }
    // The sleeve's seam: two rows of stitching down the outside of the arm,
    // riding over the folds.
    for th in [1.05f32, 1.17] {
        let line: Vec<V3> = stations.iter().filter(|s| **s > CUFF + 0.004).map(|&s| {
            let p = surface(s, th);
            // Lifted a hair off the cloth, squarely away from the arm's middle.
            let out = normalize([p[0], p[1] + 0.002, 0.0]);
            [p[0] + out[0] * 0.0005, p[1] + out[1] * 0.0005, p[2]]
        }).collect();
        let mut stitch = tube(&line, 0.0007, 4).tinted(seam);
        // (`tube` rounds in metres; the sleeve is drawn stretched along z,
        // which flattens nothing across the arm.)
        for uv in stitch.uvs.iter_mut() {
            *uv = [0.0, 0.0];
        }
        m.append(&stitch);
    }
    m
}
