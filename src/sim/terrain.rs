//! The prototype map: a 400 m square of the Lake Mille Lacs shoreline around
//! Vault 143, with nuclear ice lakes, warming shelters and a glowing crater.
//!
//! The height function is shared by the renderer (terrain mesh) and gameplay
//! (player/wolf ground height), so they always agree.

use super::interiors::{self, Interior};
use super::mathx::{lerp, smoothstep};

pub const HALF_SIZE: f32 = 200.0;

/// Vault 143's door sits in the southern hillside; the player starts just north of it.
pub const VAULT_POS: (f32, f32) = (0.0, 165.0);
pub const PLAYER_SPAWN: (f32, f32) = (0.0, 152.0);

/// Frozen "nuclear ice" lakes: (x, z, radius).
pub const LAKES: [(f32, f32, f32); 3] = [(-60.0, 40.0, 28.0), (70.0, -30.0, 35.0), (10.0, -120.0, 22.0)];
pub const LAKE_LEVEL: f32 = -2.0;
/// The ice surface sits a little above the lake bed.
pub const ICE_LEVEL: f32 = LAKE_LEVEL + 0.15;
/// Share of a lake's radius covered by ice.
pub const ICE_FRACTION: f32 = 0.9;
pub const ICE_RADS_PER_SEC: f32 = 2.5;

/// Ice-fishing houses with fire barrels: (x, z).
pub const SHELTERS: [(f32, f32); 4] = [(30.0, 110.0), (-80.0, -40.0), (105.0, 60.0), (-20.0, -75.0)];
pub const SHELTER_RADIUS: f32 = 5.0;

/// Radiation hot spots: (x, z, radius, rads/sec at centre).
pub const RAD_SOURCES: [(f32, f32, f32, f32); 2] = [(-120.0, 120.0, 18.0, 15.0), (140.0, -140.0, 14.0, 10.0)];

/// Raider camps: (tent x, tent z, yaw in radians). The list is read by the
/// map, the prop placer and the raiders, and placement draws random numbers in
/// list order, so never reorder it.
pub const CAMPS: [(f32, f32, f32); 2] = [(-110.0, 15.0, 0.4), (160.0, 55.0, -0.9)];
/// A camp's fire pit sits this far (east, south) of its tent, before the
/// camp's yaw turns it.
pub const CAMP_FIRE_OFFSET: (f32, f32) = (3.6, 0.8);

/// Where a camp's fire is: the offset turned by the camp's yaw.
pub fn camp_fire_pit(camp: (f32, f32, f32)) -> (f32, f32) {
    let (cx, cz, yaw) = camp;
    let (dx, dz) = CAMP_FIRE_OFFSET;
    (cx + yaw.cos() * dx + yaw.sin() * dz, cz - yaw.sin() * dx + yaw.cos() * dz)
}

/// Pre-war landmarks trees and props should stay away from: (x, z, clear
/// radius). The Bullseye-Mart, then the Golden Atomic Mills.
pub const RUINS: [(f32, f32, f32); 2] = [(interiors::MART_POS.0, interiors::MART_POS.1, 20.0), (RAD_SOURCES[1].0, RAD_SOURCES[1].1, 18.0)];

/// Rusted cars along the highway: (x, z, yaw in radians). The list order is
/// the order the car models are dressed in, so never reorder it.
pub const CARS: [(f32, f32, f32); 6] = [
    (-150.0, 95.0, 1.62),
    (-128.0, 92.0, 1.4),
    (-60.0, 98.0, 4.6),
    (60.0, 140.0, 2.6),
    (150.0, 100.0, 1.75),
    (165.0, 20.0, 1.7),
];

/// The old US-169 highway: a polyline (x, z) running east-west across the map,
/// past the abandoned cars.
pub const ROAD: [(f32, f32); 9] = [
    (-205.0, 90.0),
    (-150.0, 93.0),
    (-100.0, 97.0),
    (-50.0, 99.0),
    (0.0, 97.0),
    (50.0, 93.0),
    (100.0, 96.0),
    (150.0, 100.0),
    (205.0, 104.0),
];
pub const ROAD_HALF_WIDTH: f32 = 3.5;

/// What the ground under your boots is made of (for footstep sounds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Snow,
    Ice,
    Road,
    Concrete,
    Wood,
}

/// Poured-concrete aprons: (centre x, centre z, half width, half depth).
/// One in front of the vault door, one under the Bullseye-Mart car park.
pub const CONCRETE_PADS: [(f32, f32, f32, f32); 2] = [(VAULT_POS.0, VAULT_POS.1 - 6.5, 9.0, 5.5), (interiors::MART_POS.0, interiors::MART_POS.1 + 2.0, 16.0, 16.0)];

/// Wooden landing around each shelter's fire barrel: (centre x, centre z, half width, half depth).
pub fn wood_decks() -> impl Iterator<Item = (f32, f32, f32, f32)> {
    SHELTERS.iter().map(|&(sx, sz)| (sx + 1.6, sz, 2.2, 1.9))
}

fn dist(x: f32, z: f32, cx: f32, cz: f32) -> f32 {
    ((x - cx).powi(2) + (z - cz).powi(2)).sqrt()
}

/// Rolling snow-covered hills before lakes and flat spots are carved in.
pub fn base_height(x: f32, z: f32) -> f32 {
    3.0 * (x * 0.021).sin() * (z * 0.017).cos()
        + 1.6 * (x * 0.053 + 1.3).sin()
        + 1.2 * (z * 0.047 + 0.7).cos()
        + 0.4 * ((x + z) * 0.11).sin()
}

/// How high the player's eyes (and the camera) are above their feet.
pub const EYE_HEIGHT: f32 = 1.7;

/// Where a camera at `cam` has its feet (for footprints).
pub fn feet(cam: [f32; 3]) -> [f32; 3] {
    [cam[0], cam[1] - EYE_HEIGHT, cam[2]]
}

/// Where a shot from `origin` along `dir` (a unit vector) that missed every
/// creature strikes the ground, if that is within `range`. Walks the ray in
/// 0.75 m steps from 0.5 m out, so a very thin rise can be skipped over.
pub fn ground_hit(origin: [f32; 3], dir: [f32; 3], range: f32) -> Option<[f32; 3]> {
    let mut t = 0.5;
    while t < range {
        let p = [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
        if p[1] <= walk_height(p[0], p[2]) {
            return Some(p);
        }
        t += 0.75;
    }
    None
}

/// Snow tint for the terrain: bluer in hollows, trampled near shelters, slushy
/// along the highway and sickly green around the radiation craters. `h` is the
/// ground height at `(x, z)`; the result is an RGBA vertex colour.
pub fn terrain_tint(x: f32, z: f32, h: f32) -> [f32; 4] {
    let mut c = [0.97f32, 0.98, 1.0];
    let hollow = ((-h - 0.5) / 3.0).clamp(0.0, 1.0);
    c = [c[0] - 0.08 * hollow, c[1] - 0.05 * hollow, c[2]];
    let ripple = 0.03 * ((x * 0.31).sin() * (z * 0.27).cos() + (x * 0.07 + z * 0.05).sin());
    c = [c[0] + ripple, c[1] + ripple, c[2] + ripple * 0.5];
    for &(sx, sz) in &SHELTERS {
        let t = (1.0 - (x - sx).hypot(z - sz) / 9.0).clamp(0.0, 1.0);
        c = [c[0] - 0.18 * t, c[1] - 0.2 * t, c[2] - 0.22 * t];
    }
    let road = (1.0 - (road_distance(x, z) - ROAD_HALF_WIDTH) / 4.0).clamp(0.0, 1.0);
    c = [c[0] - 0.15 * road, c[1] - 0.15 * road, c[2] - 0.13 * road];
    for &(cx, cz, r, _) in &RAD_SOURCES {
        let t = (1.0 - (x - cx).hypot(z - cz) / (r * 1.6)).clamp(0.0, 1.0);
        c = [c[0] - 0.35 * t, c[1] - 0.12 * t, c[2] - 0.4 * t];
    }
    [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), 1.0]
}

/// Flat pads for the vault entrance and the shelters: (x, z, radius).
const PADS: [(f32, f32, f32); 5] = [
    (VAULT_POS.0, VAULT_POS.1 - 8.0, 16.0),
    (SHELTERS[0].0, SHELTERS[0].1, 9.0),
    (SHELTERS[1].0, SHELTERS[1].1, 9.0),
    (SHELTERS[2].0, SHELTERS[2].1, 9.0),
    (SHELTERS[3].0, SHELTERS[3].1, 9.0),
];

/// The ground height each pad is flattened to (the field at its centre).
/// Worked out once: it never changes, and `height` is the hottest function
/// in the game (the terrain mesh, the map, every footprint and particle).
fn pad_floors() -> &'static [f32; 5] {
    static FLOORS: std::sync::OnceLock<[f32; 5]> = std::sync::OnceLock::new();
    FLOORS.get_or_init(|| PADS.map(|(px, pz, _)| base_height(px, pz)))
}

/// Is a point whose squared distance from a centre is `d2` certainly at
/// least `edge` away? Errs on the side of "no" near the boundary (and for
/// NaN), so the exact calculation is always used where the answer matters.
fn certainly_beyond(d2: f32, edge: f32) -> bool {
    d2 > edge * edge * 1.0001
}

/// Final ground height (the lake bed under the ice on lakes).
pub fn height(x: f32, z: f32) -> f32 {
    let mut h = base_height(x, z);

    // Flatten pads for the vault entrance and the shelters. A pad has no
    // effect at all (t is exactly 0) beyond its radius, which is nearly
    // everywhere: skip the square root and the blend there.
    let floors = pad_floors();
    for (&(px, pz, r), &floor) in PADS.iter().zip(floors) {
        let (dx, dz) = (x - px, z - pz);
        if certainly_beyond(dx * dx + dz * dz, r) {
            continue;
        }
        let t = smoothstep(r, r * 0.6, dist(x, z, px, pz));
        h = lerp(h, floor, t);
    }

    // Carve the lakes (same trick: nothing happens beyond 1.15 radii).
    for (lx, lz, r) in LAKES {
        let (dx, dz) = (x - lx, z - lz);
        if certainly_beyond(dx * dx + dz * dz, r * 1.15) {
            continue;
        }
        let t = smoothstep(r * 1.15, r * 0.85, dist(x, z, lx, lz));
        h = lerp(h, LAKE_LEVEL - 0.5, t);
    }
    h
}

/// The original, slow `height`, kept as the reference the fast one is
/// tested against (bit for bit).
#[cfg(test)]
fn reference_height(x: f32, z: f32) -> f32 {
    let mut h = base_height(x, z);
    let mut pads: Vec<(f32, f32, f32)> = vec![(VAULT_POS.0, VAULT_POS.1 - 8.0, 16.0)];
    pads.extend(SHELTERS.iter().map(|&(sx, sz)| (sx, sz, 9.0)));
    for (px, pz, r) in pads {
        let t = smoothstep(r, r * 0.6, dist(x, z, px, pz));
        h = lerp(h, base_height(px, pz), t);
    }
    for (lx, lz, r) in LAKES {
        let t = smoothstep(r * 1.15, r * 0.85, dist(x, z, lx, lz));
        h = lerp(h, LAKE_LEVEL - 0.5, t);
    }
    h
}

/// Index of the lake whose ice the point is on, if any.
pub fn lake_at(x: f32, z: f32) -> Option<usize> {
    LAKES
        .iter()
        .position(|&(lx, lz, r)| dist(x, z, lx, lz) < r * ICE_FRACTION)
}

/// Height you actually stand on: ice if you are over a lake, else the ground.
pub fn walk_height(x: f32, z: f32) -> f32 {
    // Indoors the floor is flat, wherever the rooms sit.
    if interiors::zone_at(x, z).is_some() {
        return interiors::FLOOR_Y;
    }
    let h = height(x, z);
    if lake_at(x, z).is_some() {
        h.max(ICE_LEVEL)
    } else {
        h
    }
}

/// Grid cells per side of the rendered terrain mesh.
pub const MESH_RES: usize = 200;

/// Height of the rendered terrain: the triangulated grid the player actually
/// sees, which cuts corners off the smooth [`height`] field between grid
/// points. Props set down with this touch the visible snow exactly.
pub fn mesh_height(x: f32, z: f32) -> f32 {
    let cell = 2.0 * HALF_SIZE / MESH_RES as f32;
    let fx = ((x + HALF_SIZE) / cell).clamp(0.0, MESH_RES as f32 - 1e-3);
    let fz = ((z + HALF_SIZE) / cell).clamp(0.0, MESH_RES as f32 - 1e-3);
    let (i, j) = (fx.floor(), fz.floor());
    let (tx, tz) = (fx - i, fz - j);
    let at = |di: f32, dj: f32| height(-HALF_SIZE + (i + di) * cell, -HALF_SIZE + (j + dj) * cell);
    let (h00, h11) = (at(0.0, 0.0), at(1.0, 1.0));
    // Same diagonal as meshgen::ground_patch: (i, j) to (i + 1, j + 1).
    if tz >= tx {
        let h01 = at(0.0, 1.0);
        h00 + tz * (h01 - h00) + tx * (h11 - h01)
    } else {
        let h10 = at(1.0, 0.0);
        h00 + tx * (h10 - h00) + tz * (h11 - h10)
    }
}

/// Approximate surface normal from finite differences.
pub fn normal(x: f32, z: f32) -> [f32; 3] {
    let e = 0.5;
    let nx = height(x - e, z) - height(x + e, z);
    let nz = height(x, z - e) - height(x, z + e);
    let ny = 2.0 * e;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    [nx / len, ny / len, nz / len]
}

/// Background radiation from ice and hot spots (weather radiation is separate).
pub fn ambient_rads(x: f32, z: f32) -> f32 {
    let mut rads = if lake_at(x, z).is_some() { ICE_RADS_PER_SEC } else { 0.0 };
    for (cx, cz, r, strength) in RAD_SOURCES {
        let d = dist(x, z, cx, cz);
        if d < r {
            rads += strength * (1.0 - d / r);
        }
    }
    rads
}

/// Index of the shelter the point is inside / next to, if any (inside a fish
/// house counts too). Shelters have a workbench and a stove.
pub fn shelter_at(x: f32, z: f32) -> Option<usize> {
    if let Some(Interior::FishHouse(i)) = interiors::zone_at(x, z) {
        return Some(i as usize);
    }
    SHELTERS
        .iter()
        .position(|&(sx, sz)| dist(x, z, sx, sz) < SHELTER_RADIUS)
}

/// How much the weather reaches a spot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cover {
    /// Out in the wind and the radiation.
    Open,
    /// Out of the wind and the fallout, but no warmer than the air.
    Sheltered,
    /// Sheltered, and heated (a fish house, the vault lobby).
    Warm,
}

impl Cover {
    pub fn sheltered(self) -> bool {
        self != Cover::Open
    }
    pub fn warm(self) -> bool {
        self == Cover::Warm
    }
}

/// What kind of cover there is at a point: inside a room, near a shelter's
/// fire, or none.
pub fn cover_at(x: f32, z: f32) -> Cover {
    if let Some(room) = interiors::zone_at(x, z) {
        return if room.warm() { Cover::Warm } else { Cover::Sheltered };
    }
    if shelter_at(x, z).is_some() {
        Cover::Warm
    } else {
        Cover::Open
    }
}

/// Where to put a player who falls through the ice: just past the shore,
/// on the side of the lake they were already on.
pub fn shore_point(lake: usize, x: f32, z: f32) -> (f32, f32) {
    let (lx, lz, r) = LAKES[lake];
    let (mut dx, mut dz) = (x - lx, z - lz);
    let len = (dx * dx + dz * dz).sqrt();
    if len < 0.01 {
        dx = 0.0;
        dz = 1.0;
    } else {
        dx /= len;
        dz /= len;
    }
    let out = r * 1.2;
    (lx + dx * out, lz + dz * out)
}

/// Distance from a point to the centre line of the highway.
pub fn road_distance(x: f32, z: f32) -> f32 {
    // Compare squared distances and take one square root at the end (the
    // square root is monotonic, so the nearest segment is the same and the
    // answer is bit for bit what taking eight roots and a minimum gave).
    let mut best = f32::MAX;
    for w in ROAD.windows(2) {
        let ((ax, az), (bx, bz)) = (w[0], w[1]);
        let (dx, dz) = (bx - ax, bz - az);
        let t = (((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
        let (ex, ez) = (x - (ax + dx * t), z - (az + dz * t));
        best = best.min(ex * ex + ez * ez);
    }
    // Nothing finite to measure (NaN input): the old answer, "very far".
    if best == f32::MAX {
        f32::MAX
    } else {
        best.sqrt()
    }
}

/// The original `road_distance`, kept as the reference for the fast one.
#[cfg(test)]
fn reference_road_distance(x: f32, z: f32) -> f32 {
    ROAD.windows(2)
        .map(|w| {
            let ((ax, az), (bx, bz)) = (w[0], w[1]);
            let (dx, dz) = (bx - ax, bz - az);
            let t = (((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
            dist(x, z, ax + dx * t, az + dz * t)
        })
        .fold(f32::MAX, f32::min)
}

/// Z coordinate of the highway centre line at `x` (for placing things along it).
pub fn road_z(x: f32) -> f32 {
    for w in ROAD.windows(2) {
        let ((ax, az), (bx, bz)) = (w[0], w[1]);
        if x >= ax && x <= bx {
            return az + (bz - az) * (x - ax) / (bx - ax);
        }
    }
    if x < ROAD[0].0 {
        ROAD[0].1
    } else {
        ROAD[ROAD.len() - 1].1
    }
}

/// The surface at a point, for footsteps. Ice beats everything (it is a lake),
/// then decks, concrete, the road, and finally snow.
pub fn surface_at(x: f32, z: f32) -> Surface {
    if let Some(room) = interiors::zone_at(x, z) {
        return room.surface();
    }
    if lake_at(x, z).is_some() {
        return Surface::Ice;
    }
    let inside = |(cx, cz, hx, hz): (f32, f32, f32, f32)| (x - cx).abs() < hx && (z - cz).abs() < hz;
    if wood_decks().any(inside) {
        return Surface::Wood;
    }
    if CONCRETE_PADS.iter().copied().any(inside) {
        return Surface::Concrete;
    }
    if road_distance(x, z) < ROAD_HALF_WIDTH {
        return Surface::Road;
    }
    Surface::Snow
}

/// Distance from a point to Vault 143's door.
pub fn dist_to_vault(x: f32, z: f32) -> f32 {
    dist(x, z, VAULT_POS.0, VAULT_POS.1)
}

/// True if a tree or prop can go here without blocking key locations.
pub fn is_open_ground(x: f32, z: f32) -> bool {
    if x.abs() > HALF_SIZE - 4.0 || z.abs() > HALF_SIZE - 4.0 {
        return false;
    }
    if LAKES.iter().any(|&(lx, lz, r)| dist(x, z, lx, lz) < r * 1.2 + 2.0) {
        return false;
    }
    if SHELTERS.iter().any(|&(sx, sz)| dist(x, z, sx, sz) < 10.0) {
        return false;
    }
    if RAD_SOURCES.iter().any(|&(cx, cz, r, _)| dist(x, z, cx, cz) < r * 0.6) {
        return false;
    }
    if road_distance(x, z) < ROAD_HALF_WIDTH + 2.5 {
        return false;
    }
    dist(x, z, VAULT_POS.0, VAULT_POS.1) > 22.0
}

#[cfg(test)]
mod tests {
    #[test]
    fn mesh_height_matches_the_grid_and_stays_close_to_the_field() {
        let cell = 2.0 * HALF_SIZE / MESH_RES as f32;
        for (i, j) in [(10, 20), (100, 100), (57, 133)] {
            let (x, z) = (-HALF_SIZE + i as f32 * cell, -HALF_SIZE + j as f32 * cell);
            assert!((mesh_height(x, z) - height(x, z)).abs() < 1e-3);
        }
        let mut worst = 0.0f32;
        for k in 0..400 {
            let (x, z) = ((k as f32 * 7.31) % 380.0 - 190.0, (k as f32 * 3.77) % 380.0 - 190.0);
            worst = worst.max((mesh_height(x, z) - height(x, z)).abs());
        }
        assert!(worst < 0.6, "the grid follows the field: {worst}");
    }

    use super::*;

    /// Sample points for the equivalence tests: a fine scatter over (and past
    /// the edge of) the map, plus rings hugging every pad and lake boundary,
    /// where the fast paths hand over to the exact calculation.
    fn probe_points() -> Vec<(f32, f32)> {
        let mut r = crate::sim::rng::Rng::new(0xFA57);
        let mut pts: Vec<(f32, f32)> = (0..60_000).map(|_| (r.range(-230.0, 230.0), r.range(-230.0, 230.0))).collect();
        let mut centres: Vec<(f32, f32, f32)> = PADS.to_vec();
        centres.extend(LAKES.iter().flat_map(|&(x, z, rad)| [(x, z, rad * 1.15), (x, z, rad * 0.85), (x, z, rad * ICE_FRACTION), (x, z, rad * 1.2 + 2.0)]));
        centres.extend(ROAD.iter().map(|&(x, z)| (x, z, ROAD_HALF_WIDTH + 2.5)));
        for (cx, cz, rad) in centres {
            for k in 0..400 {
                let a = k as f32 / 400.0 * std::f32::consts::TAU;
                for scale in [0.999, 0.99999, 1.0, 1.00001, 1.0001, 1.001, 1.01] {
                    pts.push((cx + a.cos() * rad * scale, cz + a.sin() * rad * scale));
                }
            }
        }
        pts.extend([(0.0, 0.0), (-0.0, 0.0), (f32::MAX, 0.0), (-1e30, 1e30), (1e20, -3.0)]);
        pts
    }

    #[test]
    fn fast_height_is_bit_for_bit_the_old_height() {
        let pts = probe_points();
        assert!(pts.len() > 100_000);
        for (x, z) in pts {
            let (fast, old) = (height(x, z), reference_height(x, z));
            assert!(fast.to_bits() == old.to_bits() || (fast.is_nan() && old.is_nan()), "height({x}, {z}): {fast} vs {old}");
        }
    }

    #[test]
    fn height_of_garbage_is_still_garbage_not_a_panic() {
        for (x, z) in [(f32::NAN, 0.0), (0.0, f32::NAN), (f32::INFINITY, 0.0), (0.0, f32::NEG_INFINITY)] {
            assert_eq!(height(x, z).is_nan(), reference_height(x, z).is_nan());
        }
    }

    #[test]
    fn fast_road_distance_is_bit_for_bit_the_old_one() {
        for (x, z) in probe_points() {
            let (fast, old) = (road_distance(x, z), reference_road_distance(x, z));
            assert!(fast.to_bits() == old.to_bits(), "road_distance({x}, {z}): {fast} vs {old}");
        }
        for (x, z) in [(f32::NAN, 0.0), (0.0, f32::NAN), (f32::INFINITY, 0.0), (f32::NAN, f32::NAN)] {
            assert_eq!(road_distance(x, z).to_bits(), reference_road_distance(x, z).to_bits(), "({x}, {z})");
        }
    }

    #[test]
    fn walk_and_mesh_height_and_normals_are_unchanged_by_the_speed_up() {
        // They are built on `height`, so exact equality with a copy built on
        // the reference one proves nothing else moved.
        let ref_mesh = |x: f32, z: f32| {
            let cell = 2.0 * HALF_SIZE / MESH_RES as f32;
            let fx = ((x + HALF_SIZE) / cell).clamp(0.0, MESH_RES as f32 - 1e-3);
            let fz = ((z + HALF_SIZE) / cell).clamp(0.0, MESH_RES as f32 - 1e-3);
            let (i, j) = (fx.floor(), fz.floor());
            let (tx, tz) = (fx - i, fz - j);
            let at = |di: f32, dj: f32| reference_height(-HALF_SIZE + (i + di) * cell, -HALF_SIZE + (j + dj) * cell);
            let (h00, h11) = (at(0.0, 0.0), at(1.0, 1.0));
            if tz >= tx {
                let h01 = at(0.0, 1.0);
                h00 + tz * (h01 - h00) + tx * (h11 - h01)
            } else {
                let h10 = at(1.0, 0.0);
                h00 + tx * (h10 - h00) + tz * (h11 - h10)
            }
        };
        let ref_normal = |x: f32, z: f32| {
            let e = 0.5;
            let nx = reference_height(x - e, z) - reference_height(x + e, z);
            let nz = reference_height(x, z - e) - reference_height(x, z + e);
            let ny = 2.0 * e;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            [nx / len, ny / len, nz / len]
        };
        for (x, z) in probe_points().into_iter().step_by(7) {
            assert_eq!(mesh_height(x, z).to_bits(), ref_mesh(x, z).to_bits(), "mesh_height({x}, {z})");
            assert_eq!(normal(x, z).map(f32::to_bits), ref_normal(x, z).map(f32::to_bits), "normal({x}, {z})");
        }
    }

    #[test]
    fn the_flat_pads_are_still_flat_and_the_lakes_still_sunk() {
        for &(px, pz, _) in &PADS {
            assert!((height(px, pz) - base_height(px, pz)).abs() < 1e-5, "pad centre keeps the natural height");
            assert!((height(px + 2.0, pz) - height(px, pz)).abs() < 0.2, "level near the middle");
        }
        for &(lx, lz, _) in &LAKES {
            assert!((height(lx, lz) - (LAKE_LEVEL - 0.5)).abs() < 1e-5);
        }
    }

    #[test]
    fn lakes_have_ice_and_radiation() {
        for (i, &(lx, lz, _)) in LAKES.iter().enumerate() {
            assert_eq!(lake_at(lx, lz), Some(i));
            assert!((walk_height(lx, lz) - ICE_LEVEL).abs() < 1e-4);
            assert!(ambient_rads(lx, lz) >= ICE_RADS_PER_SEC);
        }
    }

    #[test]
    fn spawn_is_safe_and_dry() {
        let (x, z) = PLAYER_SPAWN;
        assert!(lake_at(x, z).is_none());
        assert_eq!(ambient_rads(x, z), 0.0);
        assert!(!is_open_ground(x, z), "no trees on the spawn point");
    }

    #[test]
    fn shelters_are_reachable_and_not_on_ice() {
        for (i, &(sx, sz)) in SHELTERS.iter().enumerate() {
            assert_eq!(shelter_at(sx, sz), Some(i));
            assert!(lake_at(sx, sz).is_none());
        }
    }

    #[test]
    fn shore_point_is_off_the_ice() {
        let (lx, lz, _) = LAKES[1];
        let (x, z) = shore_point(1, lx + 3.0, lz);
        assert!(lake_at(x, z).is_none());
        assert!(x > lx);
    }

    #[test]
    fn normals_are_unit_length() {
        for i in 0..50 {
            let n = normal(i as f32 * 7.3 - 150.0, i as f32 * -5.1 + 90.0);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4);
            assert!(n[1] > 0.0);
        }
    }

    #[test]
    fn highway_runs_past_the_cars_and_keeps_clear() {
        for &(x, z) in &ROAD {
            assert!(road_distance(x, z) < 1e-4);
        }
        assert!((road_z(25.0) - 95.0).abs() < 1e-4);
        assert!((road_distance(25.0, 105.0) - 10.0).abs() < 0.2);
        assert!(!is_open_ground(25.0, road_z(25.0)), "no trees on the road");
        // The road stays off the ice, the shelters and the vault.
        for &(lx, lz, r) in &LAKES {
            assert!(road_distance(lx, lz) > r * 1.2);
        }
        for &(sx, sz) in &SHELTERS {
            assert!(road_distance(sx, sz) > SHELTER_RADIUS + ROAD_HALF_WIDTH);
        }
        assert!(road_distance(VAULT_POS.0, VAULT_POS.1) > 40.0);
    }

    #[test]
    fn surfaces_follow_the_map() {
        let (lx, lz, _) = LAKES[0];
        assert_eq!(surface_at(lx, lz), Surface::Ice);
        assert_eq!(surface_at(25.0, road_z(25.0)), Surface::Road);
        assert_eq!(surface_at(25.0, road_z(25.0) + ROAD_HALF_WIDTH + 1.0), Surface::Snow);
        let (sx, sz) = SHELTERS[0];
        assert_eq!(surface_at(sx + 1.6, sz), Surface::Wood);
        assert_eq!(surface_at(VAULT_POS.0, VAULT_POS.1 - 6.0), Surface::Concrete);
        assert_eq!(surface_at(-40.0, -108.0), Surface::Concrete);
        assert_eq!(surface_at(PLAYER_SPAWN.0 + 30.0, PLAYER_SPAWN.1 - 40.0), Surface::Snow);
        // Spawn is on the vault apron or snow, never ice or road.
        assert!(matches!(surface_at(PLAYER_SPAWN.0, PLAYER_SPAWN.1), Surface::Snow | Surface::Concrete));
    }

    #[test]
    fn pads_do_not_overlap_ice_or_road() {
        for &(cx, cz, hx, hz) in &CONCRETE_PADS {
            for &(lx, lz, r) in &LAKES {
                assert!(dist(cx, cz, lx, lz) > r + hx.max(hz), "pad at ({cx},{cz}) touches a lake");
            }
            assert!(road_distance(cx, cz) > hx.max(hz) + ROAD_HALF_WIDTH - 1.0);
        }
    }

    #[test]
    fn hot_spots_fall_off() {
        let (cx, cz, r, s) = RAD_SOURCES[0];
        assert!((ambient_rads(cx, cz) - s).abs() < 1e-3);
        assert_eq!(ambient_rads(cx + r + 1.0, cz), 0.0);
    }

    #[test]
    fn indoors_the_floor_is_flat_the_ground_sounds_right_and_the_weather_stays_out() {
        for room in interiors::ALL {
            let (x, z, _) = room.entry();
            assert_eq!(walk_height(x, z), interiors::FLOOR_Y);
            assert_eq!(surface_at(x, z), room.surface());
            assert!(cover_at(x, z).sheltered(), "{room:?}");
            assert_eq!(cover_at(x, z).warm(), room.warm(), "{room:?}");
            assert_eq!(shelter_at(x, z).is_some(), matches!(room, Interior::FishHouse(_)), "only fish houses count as shelters");
        }
        // Outdoors nothing changed: open ground, a fire's warmth by a shelter.
        assert_eq!(cover_at(0.0, 0.0), Cover::Open);
        assert_eq!(cover_at(SHELTERS[0].0, SHELTERS[0].1), Cover::Warm);
        assert_eq!(shelter_at(SHELTERS[2].0, SHELTERS[2].1), Some(2));
        assert!((walk_height(10.0, 20.0) - height(10.0, 20.0)).abs() < 1e-6);
    }

    /// World layout is data other code walks in order (placement draws random
    /// numbers per entry), so every number is pinned as it was when the lists
    /// lived in three different files.
    #[test]
    fn world_layout_lists_keep_their_numbers_and_order() {
        assert_eq!(CAMPS, [(-110.0, 15.0, 0.4), (160.0, 55.0, -0.9)]);
        assert_eq!(
            CARS,
            [(-150.0, 95.0, 1.62), (-128.0, 92.0, 1.4), (-60.0, 98.0, 4.6), (60.0, 140.0, 2.6), (150.0, 100.0, 1.75), (165.0, 20.0, 1.7)]
        );
        assert_eq!(RUINS, [(-40.0, -110.0, 20.0), (140.0, -140.0, 18.0)]);
        assert_eq!(CONCRETE_PADS, [(0.0, 158.5, 9.0, 5.5), (-40.0, -108.0, 16.0, 16.0)]);
        assert_eq!(interiors::MART_POS, (-40.0, -110.0));
        assert_eq!(CAMP_FIRE_OFFSET, (3.6, 0.8));
        // The ruin on the plain is the radiation source's spot.
        assert_eq!((RUINS[1].0, RUINS[1].1), (RAD_SOURCES[1].0, RAD_SOURCES[1].1));
    }

    #[test]
    fn a_camps_fire_is_beside_its_tent_and_turns_with_it() {
        // The raiders' old private formula, as it was.
        fn old_fire_pit(camp: (f32, f32, f32)) -> (f32, f32) {
            let (cx, cz, yaw) = camp;
            let (dx, dz) = (3.6f32, 0.8f32);
            (cx + yaw.cos() * dx + yaw.sin() * dz, cz - yaw.sin() * dx + yaw.cos() * dz)
        }
        for camp in CAMPS {
            let (fx, fz) = camp_fire_pit(camp);
            assert_eq!((fx, fz), old_fire_pit(camp));
            let d = (fx - camp.0).hypot(fz - camp.1);
            assert!((d - 3.6f32.hypot(0.8)).abs() < 1e-4, "the fire is the same distance from the tent however it is turned");
        }
        // Facing straight ahead (yaw 0) the fire is just east and a little south.
        let (x, z) = camp_fire_pit((10.0, 20.0, 0.0));
        assert!((x - 13.6).abs() < 1e-5 && (z - 20.8).abs() < 1e-5);
        for camp in CAMPS {
            assert!(is_open_ground(camp.0, camp.1), "camps are on open ground");
        }
    }

    /// Cars and ruins keep trees out, so they must be on the map.
    #[test]
    fn cars_and_ruins_are_on_the_map() {
        for (x, z, _) in CARS {
            assert!(x.abs() < HALF_SIZE && z.abs() < HALF_SIZE, "({x}, {z})");
        }
        for (x, z, r) in RUINS {
            assert!(x.abs() + r < HALF_SIZE && z.abs() + r < HALF_SIZE);
        }
    }

    /// The renderer's old tint function, copied as it was, to prove the move
    /// changed no vertex colour of the ground.
    fn old_tint(x: f32, z: f32, h: f32) -> [f32; 4] {
        let mut c = [0.97f32, 0.98, 1.0];
        let hollow = ((-h - 0.5) / 3.0).clamp(0.0, 1.0);
        c = [c[0] - 0.08 * hollow, c[1] - 0.05 * hollow, c[2]];
        let ripple = 0.03 * ((x * 0.31).sin() * (z * 0.27).cos() + (x * 0.07 + z * 0.05).sin());
        c = [c[0] + ripple, c[1] + ripple, c[2] + ripple * 0.5];
        for &(sx, sz) in &SHELTERS {
            let t = (1.0 - (x - sx).hypot(z - sz) / 9.0).clamp(0.0, 1.0);
            c = [c[0] - 0.18 * t, c[1] - 0.2 * t, c[2] - 0.22 * t];
        }
        let road = (1.0 - (road_distance(x, z) - ROAD_HALF_WIDTH) / 4.0).clamp(0.0, 1.0);
        c = [c[0] - 0.15 * road, c[1] - 0.15 * road, c[2] - 0.13 * road];
        for &(cx, cz, r, _) in &RAD_SOURCES {
            let t = (1.0 - (x - cx).hypot(z - cz) / (r * 1.6)).clamp(0.0, 1.0);
            c = [c[0] - 0.35 * t, c[1] - 0.12 * t, c[2] - 0.4 * t];
        }
        [c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), 1.0]
    }

    #[test]
    fn the_ground_tint_is_bit_for_bit_what_it_was_and_reads_as_snow_slush_and_sickness() {
        for i in 0..(MESH_RES + 1) {
            for j in (0..(MESH_RES + 1)).step_by(7) {
                let x = -HALF_SIZE + i as f32 * 2.0 * HALF_SIZE / MESH_RES as f32;
                let z = -HALF_SIZE + j as f32 * 2.0 * HALF_SIZE / MESH_RES as f32;
                let h = height(x, z);
                let (new, old) = (terrain_tint(x, z, h), old_tint(x, z, h));
                for k in 0..4 {
                    assert_eq!(new[k].to_bits(), old[k].to_bits(), "({x}, {z}) channel {k}");
                }
            }
        }
        let snow = terrain_tint(0.0, 0.0, 1.0);
        assert!(snow[0] > 0.85 && snow[1] > 0.85 && snow[2] > 0.9, "open snow is near white: {snow:?}");
        let road = terrain_tint(0.0, 97.0, 0.0);
        assert!(road[0] < snow[0] - 0.05, "the highway is slushy");
        let (rx, rz, _, _) = RAD_SOURCES[0];
        let crater = terrain_tint(rx, rz, -1.0);
        assert!(crater[1] > crater[0] + 0.15 && crater[1] > crater[2] + 0.15, "the crater is sickly green: {crater:?}");
        assert!(terrain_tint(0.0, 0.0, -50.0)[3] == 1.0, "always opaque, however deep");
        // Garbage in never escapes the 0..1 colour range as a bad number.
        for v in terrain_tint(f32::NAN, 3.0, f32::INFINITY) {
            assert!(v.is_nan() || (0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn a_shot_into_the_snow_leaves_a_mark_where_it_lands() {
        let ground = walk_height(10.0, 10.0);
        // Straight down from 3 m up: lands under it, on the ground.
        let p = ground_hit([10.0, ground + 3.0, 10.0], [0.0, -1.0, 0.0], 20.0).expect("hits the snow");
        assert!((p[0], p[2]) == (10.0, 10.0));
        assert!(p[1] <= ground && p[1] > ground - 0.8, "within one step below the surface: {p:?}");
        // Out of range, or climbing away: no mark.
        assert_eq!(ground_hit([10.0, ground + 3.0, 10.0], [0.0, -1.0, 0.0], 2.0), None);
        assert_eq!(ground_hit([10.0, ground + 3.0, 10.0], [0.0, 1.0, 0.0], 100.0), None);
        // No range, no mark; a bad number never marks anything.
        assert_eq!(ground_hit([10.0, ground + 3.0, 10.0], [0.0, -1.0, 0.0], 0.0), None);
        assert_eq!(ground_hit([f32::NAN; 3], [0.0, -1.0, 0.0], 50.0), None);
        // A ray that starts underground hits at once (0.5 m out).
        let p = ground_hit([10.0, ground - 5.0, 10.0], [0.0, 0.0, 1.0], 10.0).unwrap();
        assert!((p[2] - 10.5).abs() < 1e-6);
    }

    #[test]
    fn footprints_go_under_the_camera_at_eye_height() {
        assert_eq!(feet([3.0, 1.7, -4.0]), [3.0, 0.0, -4.0]);
        let f = feet([0.0, 10.0, 0.0]);
        assert!((10.0 - f[1] - EYE_HEIGHT).abs() < 1e-6);
        assert_eq!(EYE_HEIGHT, 1.7, "the player's eye height is part of the feel of the game");
    }
}
