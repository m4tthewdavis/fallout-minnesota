//! The land beyond the map. The playable square is 400 m across; this is
//! what you see past its edge: the same snowfield rolling on into wooded
//! hills to the west, south and east, and to the north the shore of Lake
//! Mille Lacs, a plain of wind-scoured ice with a couple of wooded islands,
//! running out to the horizon.
//!
//! It is scenery only (nobody can walk there): a ring of terrain that meets
//! the map's own mesh vertex for vertex, and a few thousand flat "cards",
//! each a picture of a whole tree, standing where the woods are. Pure maths
//! (no Bevy), built from its own seed, so it never moves anything on the map.

use super::mathx::{lerp, smoothstep};
use super::meshgen::MeshData;
use super::rng::Rng;
use super::terrain::{self, HALF_SIZE, ICE_LEVEL, MESH_RES};

/// How far from the map's centre the backdrop reaches (as a half-width: it
/// is square, like the map). The rooms you enter are built off the map
/// beyond this, so it must stay short of them.
pub const REACH: f32 = 880.0;

/// Wooded islands out on the big lake: (x, z, radius).
pub const ISLANDS: [(f32, f32, f32); 2] = [(-210.0, -470.0, 60.0), (300.0, -640.0, 95.0)];

fn hash(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (z as u32).wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth noise in 0..=1 with features about `cell` metres across.
fn noise(x: f32, z: f32, cell: f32, seed: u32) -> f32 {
    let (fx, fz) = (x / cell, z / cell);
    let (x0, z0) = (fx.floor(), fz.floor());
    let (tx, tz) = (fx - x0, fz - z0);
    let (tx, tz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let (ix, iz) = (x0 as i32, z0 as i32);
    let a = lerp(hash(ix, iz, seed), hash(ix + 1, iz, seed), tx);
    let b = lerp(hash(ix, iz + 1, seed), hash(ix + 1, iz + 1, seed), tx);
    lerp(a, b, tz)
}

/// A few octaves of [`noise`], still in 0..=1.
fn fbm(x: f32, z: f32, cell: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut total, mut c) = (0.0, 1.0, 0.0, cell);
    for o in 0..octaves {
        sum += amp * noise(x + o as f32 * 37.0, z - o as f32 * 91.0, c, seed + o);
        total += amp;
        amp *= 0.5;
        c *= 0.5;
    }
    sum / total
}

/// Metres past the map's edge (0 on the map).
pub fn beyond(x: f32, z: f32) -> f32 {
    (x.abs().max(z.abs()) - HALF_SIZE).max(0.0)
}

/// How far out on the big lake a point is: 0 on land (and on the islands),
/// 1 on the open ice. The shore runs east-west north of the map, in bays
/// and points.
pub fn big_lake(x: f32, z: f32) -> f32 {
    let shore = -(HALF_SIZE + 55.0) - 90.0 * fbm(x, 0.0, 260.0, 2, 7);
    let mut lake = smoothstep(shore, shore - 40.0, z);
    for (ix, iz, r) in ISLANDS {
        let d = (x - ix).hypot(z - iz);
        lake *= smoothstep(r * 0.8, r * 1.15, d);
    }
    lake
}

/// Ground height anywhere: the map's own height on the map, and beyond it
/// the backdrop's hills, shore and lake ice. It joins the map without a step.
pub fn height(x: f32, z: f32) -> f32 {
    let inner = terrain::height(x, z);
    let d = beyond(x, z);
    if d <= 0.0 {
        return inner;
    }
    // The map's small rolls would repeat for ever: fade them into broader
    // country, with real hills further out and more of them to the south,
    // where the vault is dug into the hillside.
    let out = smoothstep(0.0, 260.0, d);
    let south = smoothstep(-100.0, 500.0, z);
    let rolling = 24.0 * (fbm(x, z, 330.0, 3, 1) - 0.40);
    let ridges = 75.0 * smoothstep(0.38, 0.85, fbm(x, z, 640.0, 2, 2)) * smoothstep(100.0, 620.0, d);
    let land = inner * (1.0 - 0.6 * out) + ((rolling + ridges) * (1.0 + 0.6 * south) + 2.0) * out;
    let mut h = lerp(land, ICE_LEVEL, big_lake(x, z));
    for (ix, iz, r) in ISLANDS {
        let dist = (x - ix).hypot(z - iz);
        if dist < r {
            h = h.max(ICE_LEVEL + 6.0 * smoothstep(r, r * 0.25, dist));
        }
    }
    h
}

/// Where the old highway would run on past the map: its z for a given x.
fn highway_z(x: f32) -> f32 {
    let (first, last) = (terrain::ROAD[0], terrain::ROAD[terrain::ROAD.len() - 1]);
    if x < 0.0 {
        first.1 + (x - first.0) * 0.02
    } else {
        last.1 + (x - last.0) * 0.08
    }
}

/// How thickly wooded the backdrop is at a point, 0 (open snow or ice) to 1
/// (deep woods). Nothing on the map itself, on the lake ice, or in the cut
/// the highway makes.
pub fn woods(x: f32, z: f32) -> f32 {
    let d = beyond(x, z);
    if d < 3.0 || big_lake(x, z) > 0.04 {
        return 0.0;
    }
    if x.abs() > HALF_SIZE && (z - highway_z(x)).abs() < 11.0 {
        return 0.0;
    }
    // Woods and open fields in broad patches; the islands are all trees.
    let mut w = smoothstep(0.38, 0.54, fbm(x, z, 230.0, 3, 5));
    for (ix, iz, r) in ISLANDS {
        if (x - ix).hypot(z - iz) < r * 0.8 {
            w = 1.0;
        }
    }
    w
}

/// Vertex colour for the backdrop's ground: the map's own snow tint, darker
/// and greener under the trees, and blue wind-polished ice out on the lake.
pub fn tint(x: f32, z: f32, h: f32) -> [f32; 4] {
    let base = terrain::terrain_tint(x, z, h);
    let mut c = [base[0], base[1], base[2]];
    let shade = woods(x, z) * smoothstep(0.0, 40.0, beyond(x, z));
    let floor = [0.46, 0.53, 0.58];
    for k in 0..3 {
        c[k] = lerp(c[k], floor[k], shade * 0.75);
    }
    let lake = big_lake(x, z);
    if lake > 0.0 {
        // Drifted snow in long streaks across bare ice.
        let streaks = smoothstep(0.45, 0.62, fbm(x * 0.35 + z * 0.2, z, 70.0, 3, 9));
        let ice = [lerp(0.86, 0.60, streaks), lerp(0.92, 0.74, streaks), lerp(1.0, 0.90, streaks)];
        for k in 0..3 {
            c[k] = lerp(c[k], ice[k], lake);
        }
    }
    [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), 1.0]
}

/// Half-widths of the backdrop's square rings, from the map's edge out to
/// [`REACH`]: close together at first and widening with distance.
pub fn ring_sizes() -> Vec<f32> {
    let mut sizes = vec![HALF_SIZE];
    let mut step = 2.0 * HALF_SIZE / MESH_RES as f32;
    loop {
        let last = sizes[sizes.len() - 1];
        let next = last + step;
        if next >= REACH - step * 0.5 {
            sizes.push(REACH);
            return sizes;
        }
        sizes.push(next);
        step *= 1.3;
    }
}

/// A point on the square ring of half-width `s`: `i` counts round it,
/// `MESH_RES` steps to a side. On the innermost ring these are exactly the
/// edge vertices of the map's terrain mesh (same arithmetic, same floats).
fn ring_point(s: f32, i: usize) -> (f32, f32) {
    let n = MESH_RES;
    let (side, k) = ((i / n) % 4, i % n);
    let along = |j: usize| -s + 2.0 * s * (j as f32 / n as f32);
    match side {
        0 => (along(k), -s),
        1 => (s, along(k)),
        2 => (along(n - k), s),
        _ => (-s, along(n - k)),
    }
}

/// The backdrop's ground: square rings of quads from the map's edge to
/// [`REACH`], with heights from [`height`], smooth normals, UVs in the map's
/// own scale (so the snow texture carries straight on) and [`tint`] colours.
pub fn skirt_mesh() -> MeshData {
    let sizes = ring_sizes();
    let per_ring = MESH_RES * 4;
    let mut m = MeshData::default();
    for (r, &s) in sizes.iter().enumerate() {
        // Sample the slope over about the spacing of the rings out here.
        let e = if r == 0 { 0.5 } else { ((s - sizes[r - 1]) * 0.5).max(0.5) };
        for i in 0..per_ring {
            let (x, z) = ring_point(s, i);
            let h = height(x, z);
            let nx = height(x - e, z) - height(x + e, z);
            let nz = height(x, z - e) - height(x, z + e);
            let len = (nx * nx + 4.0 * e * e + nz * nz).sqrt();
            m.vertex([x, h, z], [nx / len, 2.0 * e / len, nz / len], [x / 4.0, z / 4.0], tint(x, z, h));
        }
    }
    let per = per_ring as u32;
    for r in 0..sizes.len() as u32 - 1 {
        for i in 0..per {
            let (a, b) = (r * per + i, r * per + (i + 1) % per);
            let (c, d) = (b + per, a + per);
            // Wound so the top faces up (the ring runs clockwise seen from above).
            m.tri(a, b, c);
            m.tri(a, c, d);
        }
    }
    m
}

/// A picture of a whole tree standing in the backdrop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub x: f32,
    pub z: f32,
    /// Height of the ground it stands on.
    pub y: f32,
    /// How tall it is drawn, metres.
    pub height: f32,
    /// Which tree on the sheet: 0 spruce, 1 fir, 2 pine, 3 birch.
    pub kind: u8,
    /// Which belt of the backdrop it is in: 0 nearest the map.
    pub band: u8,
    /// Mirror the picture, so neighbours of a kind don't match.
    pub flip: bool,
    /// Brightness of this tree against its neighbours (about 1).
    pub shade: f32,
}

/// The first few metres past the map's edge are close enough to see a flat
/// picture for what it is: real trees stand there instead (see [`fringe`]),
/// and the cards begin behind them.
pub const FRINGE: f32 = 26.0;

/// Trees just past the map's edge, in the woods the cards carry on: where
/// (x, z), which of the four sheet kinds grows there (0 spruce, 1 fir, 2
/// pine, 3 birch) and how grown it is (its share of a full-size tree).
pub fn fringe(seed: u64) -> Vec<(f32, f32, u8, f32)> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    let spacing = 11.5;
    let outer = HALF_SIZE + FRINGE + 4.0;
    let n = (2.0 * outer / spacing).ceil() as i32;
    for gz in 0..n {
        for gx in 0..n {
            let (jx, jz, roll, pick, size) = (rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.range(0.6, 1.0));
            let x = -outer + (gx as f32 + jx) * spacing;
            let z = -outer + (gz as f32 + jz) * spacing;
            let d = beyond(x, z);
            if !(2.5..FRINGE + 4.0).contains(&d) {
                continue;
            }
            let w = woods(x, z);
            if roll > w * w * 0.8 {
                continue;
            }
            let kind = if pick < 0.14 {
                3
            } else if pick < 0.26 {
                2
            } else if pick < 0.6 {
                1
            } else {
                0
            };
            out.push((x, z, kind, size));
        }
    }
    out
}

/// The backdrop's belts: (from, to) in metres past the map's edge and how far
/// apart the trees stand. Further out they are fewer and drawn bigger.
pub const BANDS: [(f32, f32, f32); 3] = [(FRINGE, 120.0, 6.2), (120.0, 340.0, 11.0), (340.0, REACH - HALF_SIZE - 12.0, 23.0)];

/// Kinds of tree on the sheet (`assets/textures/generated/tree_cards.png`).
pub const CARD_KINDS: u8 = 4;

/// Where every tree card stands, for a seed. The same seed always gives the
/// same woods.
pub fn tree_cards(seed: u64) -> Vec<Card> {
    let mut rng = Rng::new(seed);
    let mut cards = Vec::new();
    for (band, &(from, to, spacing)) in BANDS.iter().enumerate() {
        let outer = HALF_SIZE + to;
        let n = (2.0 * outer / spacing).ceil() as i32;
        for gz in 0..n {
            for gx in 0..n {
                // One try per cell of a jittered grid: even cover, no rows.
                let (jx, jz, roll, pick, size, flip, shade) = (rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.f32(), rng.chance(0.5), rng.range(0.70, 0.92));
                let x = -outer + (gx as f32 + jx) * spacing;
                let z = -outer + (gz as f32 + jz) * spacing;
                let d = beyond(x, z);
                if d < from || d >= to {
                    continue;
                }
                let w = woods(x, z);
                // Thin at the edges of the woods, close-grown inside.
                if roll > w * w {
                    continue;
                }
                // Birch and fir like the low ground and the shore; pines stand on the rises.
                let y = height(x, z);
                let low = 1.0 - smoothstep(0.0, 14.0, y - ICE_LEVEL);
                let birch = 0.08 + 0.22 * low;
                let pine = 0.10 + 0.12 * (1.0 - low);
                let fir = 0.28;
                let kind = if pick < birch {
                    3
                } else if pick < birch + pine {
                    2
                } else if pick < birch + pine + fir {
                    1
                } else {
                    0
                };
                let (lo, hi) = match kind {
                    0 => (11.0, 17.0),
                    1 => (8.0, 13.0),
                    2 => (15.0, 22.0),
                    _ => (10.0, 15.0),
                };
                // Far belts stand for more trees than they draw: make them count.
                let grow = 1.0 + band as f32 * 0.3;
                cards.push(Card { x, z, y, height: (lo + (hi - lo) * size) * grow, kind, band: band as u8, flip, shade });
            }
        }
    }
    cards
}

/// How many pie slices the cards are split into (so the ones behind you
/// needn't be drawn).
pub const SECTORS: usize = 12;

/// Which slice of the compass a point is in, 0..[`SECTORS`].
pub fn sector(x: f32, z: f32) -> usize {
    let a = z.atan2(x) + std::f32::consts::PI;
    ((a / std::f32::consts::TAU * SECTORS as f32) as usize).min(SECTORS - 1)
}

/// One mesh for a set of cards: an upright quad each, square on to the map's
/// centre (plus a second at right angles when `crossed`, for the nearest
/// trees, which you can walk round a little). Normals lean up and towards
/// the map, so a card is lit like the side of the tree you can see. Each
/// card is half as wide as it is tall, like its cell on the sheet.
pub fn cards_mesh(cards: &[Card], crossed: bool) -> MeshData {
    let mut m = MeshData::default();
    for c in cards {
        let len = c.x.hypot(c.z).max(1e-3);
        // Towards the map's centre, and across that.
        let (tx, tz) = (-c.x / len, -c.z / len);
        let (sx, sz) = (-tz, tx);
        let normal = {
            let n = [tx * 0.6, 0.8, tz * 0.6];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            [n[0] / l, n[1] / l, n[2] / l]
        };
        let half = c.height * 0.25;
        let (u0, u1) = {
            let (a, b) = (c.kind as f32 / CARD_KINDS as f32, (c.kind + 1) as f32 / CARD_KINDS as f32);
            if c.flip {
                (b, a)
            } else {
                (a, b)
            }
        };
        let color = [c.shade, c.shade, c.shade, 1.0];
        // Sunk a little, so no card floats where the ground falls away.
        let (y0, y1) = (c.y - 0.5, c.y - 0.5 + c.height);
        let quads: &[(f32, f32)] = if crossed { &[(sx, sz), (tx, tz)] } else { &[(sx, sz)] };
        for &(ax, az) in quads {
            let a = m.vertex([c.x - ax * half, y0, c.z - az * half], normal, [u0, 1.0], color);
            let b = m.vertex([c.x + ax * half, y0, c.z + az * half], normal, [u1, 1.0], color);
            let d = m.vertex([c.x + ax * half, y1, c.z + az * half], normal, [u1, 0.0], color);
            let e = m.vertex([c.x - ax * half, y1, c.z - az * half], normal, [u0, 0.0], color);
            m.quad(a, b, d, e);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backdrop_meets_the_map_without_a_step() {
        // On the map it is the map.
        for (x, z) in [(0.0, 0.0), (150.0, -120.0), (-199.0, 199.0), (200.0, 37.0)] {
            assert_eq!(height(x, z), terrain::height(x, z));
        }
        // Just past the edge it has barely moved, all the way round.
        for i in 0..400 {
            let t = i as f32 / 400.0 * 2.0 - 1.0;
            for (x, z, ox, oz) in [(HALF_SIZE, t * HALF_SIZE, 1.0, 0.0), (-HALF_SIZE, t * HALF_SIZE, -1.0, 0.0), (t * HALF_SIZE, HALF_SIZE, 0.0, 1.0), (t * HALF_SIZE, -HALF_SIZE, 0.0, -1.0)] {
                let step = (height(x + ox * 0.5, z + oz * 0.5) - height(x, z)).abs();
                assert!(step < 0.35, "a step of {step} m at ({x}, {z})");
            }
        }
    }

    #[test]
    fn the_first_ring_is_the_edge_of_the_terrain_mesh() {
        let sizes = ring_sizes();
        assert_eq!(sizes[0], HALF_SIZE);
        assert_eq!(*sizes.last().unwrap(), REACH);
        assert!(sizes.windows(2).all(|w| w[1] > w[0] + 1.0), "rings step outwards: {sizes:?}");
        let m = skirt_mesh();
        let cell = 2.0 * HALF_SIZE / MESH_RES as f32;
        for i in 0..MESH_RES * 4 {
            let p = m.positions[i];
            // On the map's border, on its 2 m grid, at the terrain's own height.
            assert_eq!(p[0].abs().max(p[2].abs()), HALF_SIZE);
            let on_grid = |v: f32| ((v + HALF_SIZE) / cell - ((v + HALF_SIZE) / cell).round()).abs() < 1e-4;
            assert!(on_grid(p[0]) && on_grid(p[2]), "{p:?}");
            assert_eq!(p[1], terrain::height(p[0], p[2]));
            assert!((p[1] - terrain::mesh_height(p[0], p[2])).abs() < 2e-3, "the mesh the player sees agrees at its own vertices");
        }
        // Every border vertex of the terrain grid is there, once.
        let mut seen = std::collections::HashSet::new();
        for i in 0..MESH_RES * 4 {
            let p = m.positions[i];
            assert!(seen.insert((p[0].to_bits(), p[2].to_bits())), "twice: {p:?}");
        }
        assert_eq!(seen.len(), MESH_RES * 4);
    }

    #[test]
    fn the_skirt_is_a_sound_mesh_facing_up() {
        let m = skirt_mesh();
        assert!(m.is_valid());
        let rings = ring_sizes().len();
        assert_eq!(m.positions.len(), rings * MESH_RES * 4);
        assert_eq!(m.triangle_count(), (rings - 1) * MESH_RES * 4 * 2);
        assert!(m.triangle_count() < 60_000, "{} triangles", m.triangle_count());
        for n in &m.normals {
            assert!(n[1] > 0.3, "normal {n:?}");
        }
        // Each triangle's own facing is up too (so it isn't culled from above).
        for t in m.indices.chunks(3) {
            let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let ny = u[2] * v[0] - u[0] * v[2];
            assert!(ny > 0.0, "a triangle faces down: {a:?} {b:?} {c:?}");
        }
        let (lo, hi) = m.bounds();
        assert!(lo[0] == -REACH && hi[0] == REACH && lo[2] == -REACH && hi[2] == REACH);
        assert!(hi[1] > 25.0, "there are hills: {}", hi[1]);
    }

    #[test]
    fn the_big_lake_lies_flat_to_the_north_with_islands_in_it() {
        // Open ice, dead level.
        for (x, z) in [(0.0, -600.0), (-500.0, -800.0), (700.0, -500.0), (120.0, -870.0)] {
            assert!(big_lake(x, z) > 0.99, "({x}, {z})");
            assert!((height(x, z) - ICE_LEVEL).abs() < 1e-3);
            assert_eq!(woods(x, z), 0.0);
        }
        // Not to the south, east or west, and never on the map.
        for (x, z) in [(0.0, 600.0), (700.0, 100.0), (-700.0, 300.0), (0.0, -190.0), (150.0, -199.0)] {
            assert_eq!(big_lake(x, z), 0.0, "({x}, {z})");
        }
        for (ix, iz, _) in ISLANDS {
            assert_eq!(big_lake(ix, iz), 0.0);
            assert!(height(ix, iz) > ICE_LEVEL + 3.0, "an island stands out of the ice");
            assert_eq!(woods(ix, iz), 1.0);
        }
        // Nothing reaches the rooms built off the map.
        let room = crate::sim::interiors::Interior::FishHouse(0).origin();
        assert!(room.0 - 30.0 > REACH && room.1 - 30.0 > REACH);
    }

    #[test]
    fn the_woods_are_dense_patchy_and_keep_off_the_map_the_ice_and_the_road() {
        let cards = tree_cards(7);
        assert_eq!(cards, tree_cards(7), "the same seed, the same woods");
        assert_ne!(cards.len(), tree_cards(8).len());
        assert!((5_000..16_000).contains(&cards.len()), "{} cards", cards.len());
        let mut per_band = [0usize; 3];
        let mut kinds = [0usize; 4];
        for c in &cards {
            let d = beyond(c.x, c.z);
            let (from, to, _) = BANDS[c.band as usize];
            assert!(d >= from && d < to, "{c:?}");
            assert!(c.x.abs().max(c.z.abs()) < REACH);
            assert!(big_lake(c.x, c.z) <= 0.04, "a tree on the ice: {c:?}");
            assert!(woods(c.x, c.z) > 0.0);
            assert_eq!(c.y, height(c.x, c.z));
            assert!((8.0..40.0).contains(&c.height));
            assert!(c.kind < CARD_KINDS);
            per_band[c.band as usize] += 1;
            kinds[c.kind as usize] += 1;
        }
        assert!(per_band.iter().all(|n| *n > 800), "{per_band:?}");
        assert!(kinds.iter().all(|n| *n > 200), "every kind grows somewhere: {kinds:?}");
        assert!(kinds[0] > kinds[3], "mostly spruce: {kinds:?}");
        // Open fields as well as woods: neither everywhere.
        let mut wooded = 0;
        for i in 0..2000 {
            let (x, z) = ((i as f32 * 0.618).fract() * 1200.0 - 600.0, 300.0 + (i as f32 * 0.414).fract() * 500.0);
            wooded += (woods(x, z) > 0.5) as u32;
        }
        assert!((500..1700).contains(&wooded), "{wooded} of 2000 points wooded");
    }

    #[test]
    fn real_trees_stand_between_the_map_and_the_cards() {
        let f = fringe(3);
        assert_eq!(f, fringe(3));
        assert!((120..520).contains(&f.len()), "{} fringe trees", f.len());
        for &(x, z, kind, size) in &f {
            let d = beyond(x, z);
            assert!(d >= 2.5 && d < FRINGE + 4.0, "({x}, {z}) is {d} m out");
            assert!(woods(x, z) > 0.0 && kind < CARD_KINDS && (0.6..=1.0).contains(&size));
        }
        // No card stands nearer the map than the fringe.
        assert!(tree_cards(7).iter().all(|c| beyond(c.x, c.z) >= FRINGE));
    }

    #[test]
    fn cards_stand_upright_on_the_ground_facing_the_map() {
        let cards = tree_cards(7);
        let near: Vec<Card> = cards.iter().filter(|c| c.band == 0).take(50).copied().collect();
        let single = cards_mesh(&near, false);
        let crossed = cards_mesh(&near, true);
        assert!(single.is_valid() && crossed.is_valid());
        assert_eq!(single.triangle_count(), near.len() * 2);
        assert_eq!(crossed.triangle_count(), near.len() * 4);
        for (i, c) in near.iter().enumerate() {
            let q = &single.positions[i * 4..i * 4 + 4];
            // Bottom edge just under the snow, top edge the tree's height above it.
            assert!((q[0][1] - (c.y - 0.5)).abs() < 1e-4 && (q[2][1] - q[0][1] - c.height).abs() < 1e-3);
            // Half as wide as tall.
            let w = (q[1][0] - q[0][0]).hypot(q[1][2] - q[0][2]);
            assert!((w - c.height * 0.5).abs() < 1e-3);
            // Square on to the centre of the map.
            let across = [(q[1][0] - q[0][0]) / w, (q[1][2] - q[0][2]) / w];
            let to_centre = [-c.x / c.x.hypot(c.z), -c.z / c.x.hypot(c.z)];
            assert!((across[0] * to_centre[0] + across[1] * to_centre[1]).abs() < 1e-3);
            let n = single.normals[i * 4];
            assert!(n[1] > 0.5 && n[0] * to_centre[0] + n[2] * to_centre[1] > 0.3);
            // The picture is one cell of the sheet, the right way up.
            let (u0, u1) = (single.uvs[i * 4][0], single.uvs[i * 4 + 1][0]);
            assert!(((u1 - u0).abs() - 0.25).abs() < 1e-5);
            assert!(single.uvs[i * 4][1] == 1.0 && single.uvs[i * 4 + 2][1] == 0.0);
        }
        // Every card falls in exactly one compass slice.
        let mut counts = [0usize; SECTORS];
        for c in &cards {
            counts[sector(c.x, c.z)] += 1;
        }
        assert_eq!(counts.iter().sum::<usize>(), cards.len());
    }

    #[test]
    fn the_ground_is_tinted_for_woods_and_ice() {
        let open_ice = tint(0.0, -700.0, ICE_LEVEL);
        assert!(open_ice[2] > open_ice[0], "ice is blue: {open_ice:?}");
        // Find deep woods and an open field well out, and compare.
        let (mut wood, mut field) = (None, None);
        for i in 0..4000 {
            let (x, z) = ((i as f32 * 0.618).fract() * 1400.0 - 700.0, 320.0 + (i as f32 * 0.414).fract() * 400.0);
            let w = woods(x, z);
            if w > 0.99 && wood.is_none() {
                wood = Some(tint(x, z, height(x, z)));
            }
            if w == 0.0 && field.is_none() && (z - highway_z(x)).abs() > 30.0 {
                field = Some(tint(x, z, height(x, z)));
            }
        }
        let (wood, field) = (wood.expect("some deep woods"), field.expect("an open field"));
        assert!(wood[0] < field[0] - 0.2, "the forest floor is in shade: {wood:?} vs {field:?}");
    }
}
