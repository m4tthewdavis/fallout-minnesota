//! The Pip-Boy map: coordinates, landmarks, fog of war, and the map image
//! itself, drawn from the same terrain functions the game world uses so the
//! two always agree (lakes, the highway, hills as contour lines, radiation
//! zones, buildings). Nothing here depends on Bevy.

use super::terrain::{self, HALF_SIZE, ICE_FRACTION, LAKES, RAD_SOURCES, ROAD_HALF_WIDTH, SHELTERS, VAULT_POS};

/// World size in metres.
pub const WORLD: f32 = HALF_SIZE * 2.0;

/// Map position 0..1 (u right = east, v down = south) of a world point.
pub fn to_uv(x: f32, z: f32) -> (f32, f32) {
    ((x + HALF_SIZE) / WORLD, (z + HALF_SIZE) / WORLD)
}

/// The world point at map position (u, v).
pub fn from_uv(u: f32, v: f32) -> (f32, f32) {
    (u * WORLD - HALF_SIZE, v * WORLD - HALF_SIZE)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Vault,
    Shelter,
    Ruin,
    Hazard,
    Camp,
    Water,
}

#[derive(Clone, Copy, Debug)]
pub struct Landmark {
    pub name: &'static str,
    pub x: f32,
    pub z: f32,
    pub kind: Kind,
    /// You discover it by coming this close.
    pub radius: f32,
}

/// Every place worth naming. Index order is stable (discovery state is
/// stored by index).
pub fn landmarks() -> Vec<Landmark> {
    let mut v = vec![Landmark {
        name: "Vault 143",
        x: VAULT_POS.0,
        z: VAULT_POS.1 - 6.0,
        kind: Kind::Vault,
        radius: 60.0,
    }];
    let shelter_names = ["Lundgren's Fish House", "Sven's Shanty", "Olson's Bait & Tackle", "Ole's Ice Shack"];
    for (i, &(sx, sz)) in SHELTERS.iter().enumerate() {
        v.push(Landmark {
            name: shelter_names[i % shelter_names.len()],
            x: sx,
            z: sz,
            kind: Kind::Shelter,
            radius: 40.0,
        });
    }
    v.push(Landmark {
        name: "Bullseye-Mart (ruin)",
        x: -40.0,
        z: -110.0,
        kind: Kind::Ruin,
        radius: 45.0,
    });
    let (gx, gz, _, _) = RAD_SOURCES[1];
    v.push(Landmark {
        name: "Golden Atomic Mills",
        x: gx,
        z: gz + 9.0,
        kind: Kind::Ruin,
        radius: 45.0,
    });
    let (cx, cz, _, _) = RAD_SOURCES[0];
    v.push(Landmark {
        name: "Dud Warhead Crater",
        x: cx,
        z: cz,
        kind: Kind::Hazard,
        radius: 40.0,
    });
    for (i, &(x, z)) in [(-110.0f32, 15.0f32), (160.0, 55.0)].iter().enumerate() {
        v.push(Landmark {
            name: if i == 0 { "Abandoned Camp (west)" } else { "Abandoned Camp (east)" },
            x,
            z,
            kind: Kind::Camp,
            radius: 35.0,
        });
    }
    let lake_names = ["Mille Lacs West Bay", "The Big Freeze", "Black Ice Pond"];
    for (i, &(lx, lz, r)) in LAKES.iter().enumerate() {
        v.push(Landmark {
            name: lake_names[i % lake_names.len()],
            x: lx,
            z: lz,
            kind: Kind::Water,
            radius: r + 25.0,
        });
    }
    v
}

// ---------------------------------------------------------------------------
// Fog of war
// ---------------------------------------------------------------------------

/// Grid cells per side (each 4 m).
pub const FOG_CELLS: usize = 100;

/// Which parts of the map you have seen.
#[derive(Clone, Debug)]
pub struct Fog {
    seen: Vec<bool>,
    pub landmarks_found: Vec<bool>,
}

impl Default for Fog {
    fn default() -> Self {
        Fog::new()
    }
}

impl Fog {
    pub fn new() -> Self {
        Fog {
            seen: vec![false; FOG_CELLS * FOG_CELLS],
            landmarks_found: vec![false; landmarks().len()],
        }
    }

    fn cell(x: f32, z: f32) -> Option<(usize, usize)> {
        let (u, v) = to_uv(x, z);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        Some(((u * FOG_CELLS as f32) as usize, (v * FOG_CELLS as f32) as usize))
    }

    pub fn is_seen(&self, x: f32, z: f32) -> bool {
        Self::cell(x, z).is_some_and(|(i, j)| self.seen[j * FOG_CELLS + i])
    }

    pub fn is_cell_seen(&self, i: usize, j: usize) -> bool {
        self.seen[j * FOG_CELLS + i]
    }

    /// Reveal everything within `r` metres of a point. Returns true if anything new was revealed.
    pub fn reveal(&mut self, x: f32, z: f32, r: f32) -> bool {
        let cell = WORLD / FOG_CELLS as f32;
        let span = (r / cell).ceil() as i32 + 1;
        let Some((ci, cj)) = Self::cell(x, z) else { return false };
        let mut changed = false;
        for dj in -span..=span {
            for di in -span..=span {
                let (i, j) = (ci as i32 + di, cj as i32 + dj);
                if i < 0 || j < 0 || i >= FOG_CELLS as i32 || j >= FOG_CELLS as i32 {
                    continue;
                }
                let (cx, cz) = from_uv((i as f32 + 0.5) / FOG_CELLS as f32, (j as f32 + 0.5) / FOG_CELLS as f32);
                if (cx - x).hypot(cz - z) <= r && !self.seen[j as usize * FOG_CELLS + i as usize] {
                    self.seen[j as usize * FOG_CELLS + i as usize] = true;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Mark landmarks you've come close to as found. Returns the indices newly found.
    pub fn discover(&mut self, x: f32, z: f32) -> Vec<usize> {
        let mut new = Vec::new();
        for (i, l) in landmarks().iter().enumerate() {
            if !self.landmarks_found[i] && (l.x - x).hypot(l.z - z) < l.radius {
                self.landmarks_found[i] = true;
                new.push(i);
            }
        }
        new
    }

    /// Share of the map explored, 0..1.
    pub fn fraction(&self) -> f32 {
        self.seen.iter().filter(|s| **s).count() as f32 / self.seen.len() as f32
    }

    /// An RGBA overlay (one pixel per cell): dark where unexplored, clear where seen.
    /// Stretched over the map with linear filtering it gives soft edges.
    pub fn overlay(&self) -> Vec<u8> {
        let n = FOG_CELLS as i32;
        let mut out = Vec::with_capacity(self.seen.len() * 4);
        for j in 0..n {
            for i in 0..n {
                // A 3x3 blur of the unexplored cells gives the edge of the fog some softness.
                let mut hidden = 0.0;
                let mut weight = 0.0;
                for dj in -1..=1 {
                    for di in -1..=1 {
                        let (x, y) = ((i + di).clamp(0, n - 1), (j + dj).clamp(0, n - 1));
                        let w = if di == 0 && dj == 0 { 4.0 } else if di == 0 || dj == 0 { 2.0 } else { 1.0 };
                        weight += w;
                        if !self.seen[(y * n + x) as usize] {
                            hidden += w;
                        }
                    }
                }
                out.extend_from_slice(&[0, 8, 3, (250.0 * hidden / weight) as u8]);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The map picture
// ---------------------------------------------------------------------------

type Rgb = [f32; 3];

const BG: Rgb = [0.02, 0.07, 0.03];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn hash(x: i32, z: i32) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393) ^ z.wrapping_mul(668_265_263)) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Draw the map as `size` x `size` RGBA8 pixels, north at the top. `trees` are
/// world positions to dot as forest.
pub fn render_base(size: usize, trees: &[(f32, f32)]) -> Vec<u8> {
    let mpp = WORLD / size as f32; // metres per pixel
    let mut img = vec![BG; size * size];
    let light = {
        let l = [-0.5f32, 0.8, -0.45];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    for py in 0..size {
        for px in 0..size {
            let (x, z) = from_uv((px as f32 + 0.5) / size as f32, (py as f32 + 0.5) / size as f32);
            let h = terrain::height(x, z);
            let n = terrain::normal(x, z);
            let shade = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).clamp(0.0, 1.0);
            // Snowy ground in dim Pip-Boy green, lit from the north-west.
            let mut c = mix([0.035, 0.13, 0.05], [0.1, 0.36, 0.14], shade * 0.9 + (h * 0.02).clamp(-0.1, 0.2));
            // Contour lines every 2 m, as wide as one pixel whatever the slope.
            let slope = ((n[0] * n[0] + n[2] * n[2]).sqrt() / n[1].max(0.1)).max(0.01);
            let step = 2.0;
            let f = (h / step - (h / step).round()).abs();
            if f * step < slope * mpp * 0.55 {
                c = mix(c, [0.2, 0.7, 0.25], 0.55);
            }
            // The nuclear ice: bright, cracked.
            if terrain::lake_at(x, z).is_some() {
                let crack = hash((x * 0.45) as i32, (z * 0.45) as i32) > 0.86;
                c = if crack { [0.35, 1.0, 0.6] } else { [0.12, 0.55, 0.38] };
            } else if let Some(&(lx, lz, r)) = LAKES.iter().find(|&&(lx, lz, r)| (x - lx).hypot(z - lz) < r * 1.12) {
                // Shore: a bright rim round the ice.
                let d = (x - lx).hypot(z - lz) / r;
                if d > ICE_FRACTION {
                    c = mix(c, [0.25, 0.8, 0.45], 0.7);
                }
            }
            // The old highway with a dashed centre line.
            let rd = terrain::road_distance(x, z);
            if rd < ROAD_HALF_WIDTH {
                c = [0.17, 0.3, 0.2];
                if rd < 0.35 && ((x / 5.0).floor() as i32) % 2 == 0 {
                    c = [0.7, 0.85, 0.5];
                }
            } else if rd < ROAD_HALF_WIDTH + mpp * 0.8 {
                c = mix(c, [0.3, 0.6, 0.35], 0.6);
            }
            // Radiation zones glow brighter towards the centre, with a rim.
            for &(cx, cz, r, _) in &RAD_SOURCES {
                let d = (x - cx).hypot(z - cz);
                if d < r {
                    c = mix(c, [0.5, 1.0, 0.2], 0.25 + 0.5 * (1.0 - d / r));
                    if d > r - mpp * 1.2 {
                        c = [0.9, 1.0, 0.3];
                    }
                }
            }
            img[py * size + px] = c;
        }
    }

    let plot = |img: &mut Vec<Rgb>, x: f32, z: f32, half: f32, color: Rgb| {
        let (u, v) = to_uv(x, z);
        let (cx, cy) = (u * size as f32, v * size as f32);
        let h = (half / mpp).max(0.6);
        let (x0, x1) = ((cx - h).floor().max(0.0) as usize, (cx + h).ceil().min(size as f32 - 1.0) as usize);
        let (y0, y1) = ((cy - h).floor().max(0.0) as usize, (cy + h).ceil().min(size as f32 - 1.0) as usize);
        for y in y0..=y1 {
            for xx in x0..=x1 {
                img[y * size + xx] = color;
            }
        }
    };
    // Forest: small dark dots.
    for &(x, z) in trees {
        plot(&mut img, x, z, 0.5, [0.02, 0.28, 0.1]);
    }
    // Buildings.
    let bright = [0.8, 1.0, 0.7];
    for &(sx, sz) in &SHELTERS {
        plot(&mut img, sx - 1.5, sz, 2.0, bright);
        plot(&mut img, sx + 1.5, sz, 0.8, [1.0, 0.7, 0.3]); // the fire barrel
    }
    // The Bullseye-Mart ruin: its remaining walls.
    for (cx, cz, hx, hz) in [(-40.0f32, -118.0f32, 12.0f32, 0.5f32), (-52.0, -110.0, 0.5, 8.0), (-28.0, -113.0, 0.5, 5.0), (-48.0, -102.0, 4.0, 0.5), (-30.5, -102.0, 2.5, 0.5)] {
        let mut x = cx - hx;
        while x <= cx + hx {
            let mut z = cz - hz;
            while z <= cz + hz {
                plot(&mut img, x, z, 0.3, bright);
                z += mpp * 0.7;
            }
            x += mpp * 0.7;
        }
    }
    // The silos.
    let (gx, gz, _, _) = RAD_SOURCES[1];
    for off in [-5.0f32, 0.0, 5.0] {
        let (sx, sz) = (gx + off, gz + 9.0);
        let mut a = 0.0f32;
        while a < std::f32::consts::TAU {
            plot(&mut img, sx + a.cos() * 2.3, sz + a.sin() * 2.3, 0.3, bright);
            a += 0.25;
        }
        plot(&mut img, sx, sz, 1.4, [0.35, 0.55, 0.3]);
    }
    // The vault: hillside and door.
    for k in 0..26 {
        plot(&mut img, VAULT_POS.0 - 13.0 + k as f32, VAULT_POS.1 + 4.0, 0.9, [0.3, 0.45, 0.3]);
    }
    plot(&mut img, VAULT_POS.0, VAULT_POS.1 - 1.0, 2.5, [1.0, 0.9, 0.3]);

    let mut out = Vec::with_capacity(size * size * 4);
    for c in &img {
        out.extend_from_slice(&[(c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8, 255]);
    }
    out
}

/// A small arrow pointing up (RGBA, `n` x `n`), for the player marker.
pub fn arrow_icon(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = ((x as f32 + 0.5) / n as f32 - 0.5, (y as f32 + 0.5) / n as f32);
            // Triangle tip at the top, notch at the bottom.
            let half_width = fy * 0.45 - 0.0;
            let notch = fy > 0.72 && fx.abs() < (fy - 0.72) * 0.9;
            if fy > 0.04 && fy < 0.95 && fx.abs() < half_width && !notch {
                let i = (y * n + x) * 4;
                out[i..i + 4].copy_from_slice(&[255, 255, 160, 255]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(img: &[u8], size: usize, x: f32, z: f32) -> [u8; 3] {
        let (u, v) = to_uv(x, z);
        let (px, py) = (((u * size as f32) as usize).min(size - 1), ((v * size as f32) as usize).min(size - 1));
        let i = (py * size + px) * 4;
        [img[i], img[i + 1], img[i + 2]]
    }

    fn luma(c: [u8; 3]) -> f32 {
        c[0] as f32 * 0.3 + c[1] as f32 * 0.6 + c[2] as f32 * 0.1
    }

    #[test]
    fn coordinates_round_trip_and_north_is_up() {
        for &(x, z) in &[(0.0, 0.0), (-199.0, 150.0), (120.5, -77.0)] {
            let (u, v) = to_uv(x, z);
            let (bx, bz) = from_uv(u, v);
            assert!((bx - x).abs() < 1e-3 && (bz - z).abs() < 1e-3);
        }
        assert_eq!(to_uv(0.0, 0.0), (0.5, 0.5));
        let (_, v_north) = to_uv(0.0, -100.0);
        let (_, v_south) = to_uv(0.0, 100.0);
        assert!(v_north < v_south, "north (-z) is towards the top");
        let (u_west, _) = to_uv(-100.0, 0.0);
        let (u_east, _) = to_uv(100.0, 0.0);
        assert!(u_west < u_east);
    }

    #[test]
    fn landmarks_cover_the_map_and_are_inside_it() {
        let l = landmarks();
        assert!(l.len() >= 13);
        assert!(l.iter().any(|m| m.name.contains("Vault")) && l.iter().any(|m| m.name.contains("Bullseye")) && l.iter().any(|m| m.name.contains("Golden Atomic")));
        for m in &l {
            assert!(m.x.abs() < HALF_SIZE && m.z.abs() < HALF_SIZE, "{} is off the map", m.name);
            assert!(m.radius > 20.0);
        }
        assert_eq!(l.iter().filter(|m| m.kind == Kind::Shelter).count(), SHELTERS.len());
        assert_eq!(l.iter().filter(|m| m.kind == Kind::Water).count(), LAKES.len());
        let names: std::collections::HashSet<_> = l.iter().map(|m| m.name).collect();
        assert_eq!(names.len(), l.len(), "names are unique");
    }

    #[test]
    fn fog_clears_as_you_explore() {
        let mut fog = Fog::new();
        assert_eq!(fog.fraction(), 0.0);
        assert!(!fog.is_seen(0.0, 150.0));
        assert!(fog.reveal(0.0, 150.0, 40.0));
        assert!(fog.is_seen(0.0, 150.0) && fog.is_seen(30.0, 150.0));
        assert!(!fog.is_seen(0.0, 100.0), "50 m away is still hidden");
        let f = fog.fraction();
        // A 40 m circle is about pi*40^2 / 400^2 of the map = 3.1%.
        assert!(f > 0.025 && f < 0.04, "{f}");
        assert!(!fog.reveal(0.0, 150.0, 40.0), "nothing new the second time");
        assert!(fog.reveal(10.0, 150.0, 40.0), "moving reveals a sliver more");
        // Off the map is harmless.
        assert!(!fog.reveal(1000.0, 0.0, 40.0));
        // Edge reveals don't panic or wrap.
        fog.reveal(-199.0, -199.0, 40.0);
        fog.reveal(199.0, 199.0, 40.0);
    }

    #[test]
    fn landmarks_are_found_by_walking_up_to_them() {
        let mut fog = Fog::new();
        let (vx, vz) = (VAULT_POS.0, VAULT_POS.1 - 6.0);
        assert!(fog.discover(0.0, 0.0).is_empty() || !landmarks().iter().any(|l| l.name.contains("Vault") && (l.x).hypot(l.z) > l.radius));
        let found = fog.discover(vx, vz + 5.0);
        assert!(found.iter().any(|&i| landmarks()[i].name == "Vault 143"));
        assert!(fog.discover(vx, vz).is_empty(), "each is announced once");
        let far = fog.discover(-40.0, -110.0);
        assert!(far.iter().any(|&i| landmarks()[i].name.contains("Bullseye")));
    }

    #[test]
    fn the_fog_overlay_is_dark_only_where_unexplored() {
        let mut fog = Fog::new();
        fog.reveal(0.0, 0.0, 30.0);
        let o = fog.overlay();
        assert_eq!(o.len(), FOG_CELLS * FOG_CELLS * 4);
        let cell = |i: usize, j: usize| o[(j * FOG_CELLS + i) * 4 + 3];
        assert_eq!(cell(50, 50), 0, "the centre is clear");
        assert!(cell(0, 0) > 240, "the corner is fogged");
        assert!(fog.is_cell_seen(50, 50));
        // The edge fades rather than stepping: some cells are partly clear.
        let partial = (0..FOG_CELLS).map(|i| cell(i, 50)).filter(|a| *a > 10 && *a < 235).count();
        assert!(partial >= 2, "soft edge ({partial} partial cells)");
    }

    #[test]
    fn the_map_picture_shows_the_real_terrain() {
        let size = 128;
        let img = render_base(size, &[(-100.0, -100.0), (50.0, 20.0)]);
        assert_eq!(img.len(), size * size * 4);
        assert!(img.chunks(4).all(|p| p[3] == 255));
        let (lx, lz, _) = LAKES[1];
        let ice = pixel(&img, size, lx, lz);
        let land = pixel(&img, size, 0.0, 130.0);
        assert!(luma(ice) > luma(land) + 25.0, "the nuclear ice stands out: {ice:?} vs {land:?}");
        // The highway is drawn where terrain says it is.
        let rz = terrain::road_z(-20.0);
        let road = pixel(&img, size, -20.0, rz + 0.6);
        let beside = pixel(&img, size, -20.0, rz + 12.0);
        assert_ne!(road, beside);
        // The vault door glows yellow.
        let door = pixel(&img, size, VAULT_POS.0, VAULT_POS.1 - 1.0);
        assert!(door[0] > 200 && door[1] > 180 && door[2] < 120, "{door:?}");
        // Radiation zones are tinted.
        let (cx, cz, _, _) = RAD_SOURCES[0];
        let hot = pixel(&img, size, cx, cz);
        assert!(hot[1] > 150, "{hot:?}");
        // Trees are dotted on.
        let tree = pixel(&img, size, -100.0, -100.0);
        assert!(tree[1] < 90 && tree[1] > 40, "{tree:?}");
    }

    #[test]
    fn contours_follow_the_hills() {
        // Contour lines make a noticeable share of land pixels brighter.
        let size = 96;
        let img = render_base(size, &[]);
        let bright = img.chunks(4).filter(|p| p[1] > 120).count();
        assert!(bright > size * size / 50, "contours drawn: {bright}");
    }

    #[test]
    fn the_player_arrow_points_up() {
        let n = 32;
        let a = arrow_icon(n);
        let alpha = |x: usize, y: usize| a[(y * n + x) * 4 + 3];
        assert!(alpha(16, 20) > 0 && alpha(16, 6) > 0, "along the centre line");
        assert_eq!(alpha(1, 28), 0);
        // Wider at the bottom than the top.
        let row = |y: usize| (0..n).filter(|&x| alpha(x, y) > 0).count();
        assert!(row(22) > row(6));
        assert!(row(2) <= 2, "pointed tip");
    }
}
