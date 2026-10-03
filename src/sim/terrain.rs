//! The prototype map: a 400 m square of the Lake Mille Lacs shoreline around
//! Vault 143, with nuclear ice lakes, warming shelters and a glowing crater.
//!
//! The height function is shared by the renderer (terrain mesh) and gameplay
//! (player/wolf ground height), so they always agree.

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
pub const CONCRETE_PADS: [(f32, f32, f32, f32); 2] = [(VAULT_POS.0, VAULT_POS.1 - 6.5, 9.0, 5.5), (-40.0, -108.0, 16.0, 16.0)];

/// Wooden landing around each shelter's fire barrel: (centre x, centre z, half width, half depth).
pub fn wood_decks() -> impl Iterator<Item = (f32, f32, f32, f32)> {
    SHELTERS.iter().map(|&(sx, sz)| (sx + 1.6, sz, 2.2, 1.9))
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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

/// Final ground height (the lake bed under the ice on lakes).
pub fn height(x: f32, z: f32) -> f32 {
    let mut h = base_height(x, z);

    // Flatten pads for the vault entrance and the shelters.
    let mut pads: Vec<(f32, f32, f32)> = vec![(VAULT_POS.0, VAULT_POS.1 - 8.0, 16.0)];
    pads.extend(SHELTERS.iter().map(|&(sx, sz)| (sx, sz, 9.0)));
    for (px, pz, r) in pads {
        let t = smoothstep(r, r * 0.6, dist(x, z, px, pz));
        h = lerp(h, base_height(px, pz), t);
    }

    // Carve the lakes.
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
    let h = height(x, z);
    if lake_at(x, z).is_some() {
        h.max(ICE_LEVEL)
    } else {
        h
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

/// Index of the shelter the point is inside / next to, if any.
pub fn shelter_at(x: f32, z: f32) -> Option<usize> {
    SHELTERS
        .iter()
        .position(|&(sx, sz)| dist(x, z, sx, sz) < SHELTER_RADIUS)
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
    use super::*;

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
}
