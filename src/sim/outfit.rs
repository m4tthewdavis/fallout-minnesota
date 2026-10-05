//! Winter clothing for the people in the game, built from simple shapes: a
//! flared parka with a quilted belt, fur-trimmed hoods and cuffs, knit hats,
//! mittens, boots, packs, scarves, ammo bandoliers and patches of frost. Every
//! piece is modelled with the person standing at the origin, feet on y = 0,
//! about 1.8 m tall and facing +Z, so the Bevy side only has to place them.

use super::meshgen::{blob, cuboid, lathe, MeshData};
use std::f32::consts::TAU;

/// The parka: a flared skirt narrowing to the shoulders and collar, an
/// elliptical (front-to-back flatter) body.
pub fn parka_body() -> MeshData {
    let profile = [(0.0, 0.66), (0.31, 0.66), (0.3, 0.72), (0.26, 0.95), (0.24, 1.08), (0.275, 1.28), (0.255, 1.42), (0.17, 1.49), (0.15, 1.56), (0.0, 1.56)];
    lathe(&profile, 20, 0.8, false, false).scaled([1.0, 1.0, 0.78])
}

/// A leather belt cinching the parka at the waist.
pub fn belt() -> MeshData {
    lathe(&[(0.262, 1.02), (0.262, 1.1)], 20, 0.4, true, true).scaled([1.0, 1.0, 0.8])
}

/// Quilting: horizontal ridges round the body.
pub fn quilting() -> MeshData {
    let mut m = MeshData::default();
    for k in 0..4 {
        let y = 0.8 + k as f32 * 0.12 - if k == 3 { 0.0 } else { 0.0 };
        // The parka's radius at height y, roughly (see `parka_body`).
        let r = 0.3 - (y - 0.72) * 0.17;
        m.append(&lathe(&[(r + 0.004, y - 0.008), (r + 0.012, y), (r + 0.004, y + 0.008)], 20, 0.3, false, false).scaled([1.0, 1.0, 0.78]));
    }
    m
}

/// A ring of tufts: the fur trim of a hood, cuff or hem. `axis_z` makes the
/// ring face +Z (round a face); otherwise it lies flat (round a waist).
pub fn fur_ring(radius: f32, tuft: f32, count: usize, seed: u64, axis_z: bool) -> MeshData {
    let mut m = MeshData::default();
    for i in 0..count {
        let a = i as f32 / count as f32 * TAU;
        let jitter = 1.0 + 0.25 * (((seed as f32) * 1.7 + i as f32 * 2.3).sin());
        let t = blob(tuft * jitter, 0.75, 0.35, seed + i as u64, 0.3);
        let at = if axis_z { [a.cos() * radius, a.sin() * radius, 0.0] } else { [a.cos() * radius, 0.0, a.sin() * radius] };
        m.append(&t.translated(at));
    }
    m
}

/// The hood: a soft shell behind and over the head, open at the front.
pub fn hood_shell() -> MeshData {
    // A dome that hugs the back of the head, sliced open at the face.
    let mut m = lathe(&[(0.0, 0.0), (0.17, 0.03), (0.2, 0.14), (0.17, 0.26), (0.09, 0.33), (0.0, 0.35)], 14, 0.4, false, false);
    // Keep only the back half (z <= 0.04 after turning the dome to look along +Z).
    let keep: Vec<bool> = m.positions.iter().map(|p| p[2] <= 0.045).collect();
    let mut out = MeshData::default();
    let mut remap = vec![u32::MAX; m.positions.len()];
    for (i, k) in keep.iter().enumerate() {
        if *k {
            remap[i] = out.vertex(m.positions[i], m.normals[i], m.uvs[i], m.colors[i]);
        }
    }
    for t in m.indices.chunks_exact(3) {
        if t.iter().all(|&i| keep[i as usize]) {
            out.tri(remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]);
        }
    }
    m = out;
    m.translated([0.0, 1.5, -0.02])
}

/// A knit beanie with a turned-up cuff and a pompom.
pub fn beanie() -> MeshData {
    let mut m = lathe(&[(0.132, 1.68), (0.138, 1.74), (0.12, 1.83), (0.07, 1.88), (0.0, 1.9)], 16, 0.3, false, true);
    // The cuff.
    m.append(&lathe(&[(0.14, 1.675), (0.15, 1.69), (0.15, 1.745), (0.14, 1.76)], 16, 0.3, false, false));
    m.append(&blob(0.04, 0.9, 0.2, 5, 0.2).translated([0.0, 1.93, 0.0]));
    m
}

/// A trapper hat: a beanie dome with flaps hanging over the ears.
pub fn earflap_hat() -> MeshData {
    let mut m = lathe(&[(0.15, 1.7), (0.15, 1.76), (0.125, 1.85), (0.07, 1.89), (0.0, 1.9)], 16, 0.3, false, true);
    for side in [-1.0f32, 1.0] {
        m.append(&blob(0.07, 1.0, 0.15, 9 + (side > 0.0) as u64, 0.2).scaled([0.45, 1.5, 1.0]).translated([side * 0.15, 1.62, 0.0]));
    }
    // A fur band round the front of the brim.
    m.append(&fur_ring(0.148, 0.035, 12, 31, false).translated([0.0, 1.71, 0.0]));
    m
}

/// A balaclava: a snug tube over the head with a slot for the eyes.
pub fn balaclava() -> MeshData {
    lathe(&[(0.128, 1.54), (0.137, 1.62), (0.14, 1.72), (0.12, 1.82), (0.0, 1.87)], 16, 0.3, false, true)
}

/// A scarf wound twice round the neck with a tail hanging at the front.
pub fn scarf() -> MeshData {
    let mut m = lathe(&[(0.15, 1.46), (0.165, 1.48), (0.165, 1.54), (0.15, 1.56)], 14, 0.3, true, true);
    m.append(&lathe(&[(0.158, 1.5), (0.172, 1.52), (0.158, 1.54)], 14, 0.3, true, true));
    m.append(&cuboid([0.07, 0.34, 0.025], 0.2).translated([0.09, 1.34, 0.17]));
    m
}

/// A thick mitten with a thumb.
pub fn mitten() -> MeshData {
    let mut m = blob(0.058, 0.95, 0.1, 3, 0.2).scaled([0.8, 1.0, 0.95]);
    m.append(&blob(0.027, 1.0, 0.1, 4, 0.2).translated([0.035, 0.0, 0.03]));
    m.translated([0.0, 0.0, 0.0])
}

/// A fat winter boot, toe towards +Z.
pub fn boot() -> MeshData {
    let mut m = cuboid([0.14, 0.2, 0.27], 0.3).translated([0.0, 0.1, 0.05]);
    m.append(&cuboid([0.15, 0.04, 0.3], 0.3).translated([0.0, 0.02, 0.05]));
    // A rolled fur cuff round the top.
    m.append(&fur_ring(0.085, 0.032, 8, 17, false).translated([0.0, 0.21, -0.02]));
    m
}

/// A pack on the back: a squared canvas bag with a bedroll strapped on top.
pub fn backpack() -> MeshData {
    let mut m = cuboid([0.3, 0.42, 0.14], 0.3).translated([0.0, 1.2, -0.25]);
    m.append(&lathe(&[(0.07, -0.17), (0.07, 0.17)], 10, 0.2, true, true).rotated_z(std::f32::consts::FRAC_PI_2).translated([0.0, 1.47, -0.25]));
    m
}

/// A bandolier of cartridge pouches across the chest.
pub fn bandolier() -> MeshData {
    let mut m = cuboid([0.04, 0.02, 0.52], 0.2).rotated_y(0.0);
    // The strap runs diagonally; build it along +Z-ish then tilt.
    m = m.rotated_z(0.9).translated([0.0, 1.22, 0.2]);
    for k in 0..6 {
        let t = k as f32 / 5.0 - 0.5;
        m.append(&cuboid([0.04, 0.07, 0.035], 0.2).rotated_z(0.9).translated([t * 0.36, 1.22 - t * 0.43 + 0.0, 0.2 + 0.03]));
    }
    m
}

/// A flat crust of frost for shoulders, hoods and packs.
pub fn frost_patch(size: f32, seed: u64) -> MeshData {
    blob(size, 0.35, 0.45, seed, 0.3).scaled([1.0, 0.7, 1.0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<(&'static str, MeshData)> {
        vec![
            ("parka", parka_body()),
            ("belt", belt()),
            ("quilting", quilting()),
            ("hood fur", fur_ring(0.14, 0.04, 12, 1, true)),
            ("hem fur", fur_ring(0.3, 0.04, 16, 2, false)),
            ("hood", hood_shell()),
            ("beanie", beanie()),
            ("earflap hat", earflap_hat()),
            ("balaclava", balaclava()),
            ("scarf", scarf()),
            ("mitten", mitten()),
            ("boot", boot()),
            ("backpack", backpack()),
            ("bandolier", bandolier()),
            ("frost", frost_patch(0.1, 4)),
        ]
    }

    #[test]
    fn every_piece_is_a_valid_mesh() {
        for (name, m) in all() {
            assert!(m.is_valid(), "{name}");
            assert!(m.triangle_count() > 8, "{name} has too few triangles");
        }
    }

    #[test]
    fn the_clothes_fit_a_person_of_about_1_8_metres() {
        // (The fur rings and the frost patch are laid out round the origin and placed by the caller.)
        for (name, m) in all().into_iter().filter(|(n, _)| !n.contains("fur") && *n != "frost" && *n != "mitten") {
            let (lo, hi) = m.bounds();
            assert!(hi[1] < 2.05 && lo[1] > -0.05, "{name}: {lo:?} {hi:?}");
            assert!(hi[0] - lo[0] < 0.8 && hi[2] - lo[2] < 0.8, "{name} is wider than a person: {lo:?} {hi:?}");
        }
    }

    #[test]
    fn the_parka_flares_at_the_hem_and_covers_hips_to_collar() {
        let (lo, hi) = parka_body().bounds();
        assert!(lo[1] < 0.7 && hi[1] > 1.5);
        let wide = |y: f32| parka_body().positions.iter().filter(|p| (p[1] - y).abs() < 0.04).map(|p| p[0].abs()).fold(0.0f32, f32::max);
        assert!(wide(0.7) > wide(1.0), "flared skirt");
        assert!(wide(1.3) > wide(1.5), "narrows to the collar");
        // Flatter front to back than side to side.
        let depth = parka_body().positions.iter().map(|p| p[2].abs()).fold(0.0f32, f32::max);
        assert!(depth < hi[0]);
    }

    #[test]
    fn the_hood_is_open_at_the_front() {
        let m = hood_shell();
        assert!(m.positions.iter().all(|p| p[2] <= 0.05), "nothing in front of the face");
        let (lo, hi) = m.bounds();
        assert!(hi[1] - lo[1] > 0.25, "tall enough to cover the head");
    }

    #[test]
    fn the_head_gear_sits_on_the_head_not_the_shoulders() {
        for (name, m) in [("beanie", beanie()), ("earflap hat", earflap_hat()), ("balaclava", balaclava())] {
            let (lo, hi) = m.bounds();
            assert!(lo[1] > 1.45 && hi[1] > 1.85, "{name}: {lo:?} {hi:?}");
        }
    }

    #[test]
    fn a_fur_ring_is_round_and_the_right_size() {
        let r = fur_ring(0.3, 0.04, 16, 2, false);
        let (lo, hi) = r.bounds();
        assert!((hi[0] - lo[0] - 0.68).abs() < 0.12 && (hi[2] - lo[2] - 0.68).abs() < 0.12, "{lo:?} {hi:?}");
        assert!(hi[1] - lo[1] < 0.15, "flat");
        let face = fur_ring(0.14, 0.04, 12, 1, true);
        let (flo, fhi) = face.bounds();
        assert!(fhi[2] - flo[2] < 0.15 && fhi[1] - flo[1] > 0.2, "faces +Z");
    }
}
