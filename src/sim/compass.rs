//! The HUD compass: bearings to places and the strip of ticks along the top.
//! Pure text and angles, so the strip can be unit-tested without a window.

use super::terrain::{SHELTERS, VAULT_POS};

/// Compass bearing from north (-Z), clockwise, in degrees 0..360.
pub fn bearing(dx: f32, dz: f32) -> f32 {
    dx.atan2(-dz).to_degrees().rem_euclid(360.0)
}

/// The heading shown on the strip for a player's yaw (radians, turning left is
/// positive): degrees clockwise from north, 0..360.
pub fn heading(yaw: f32) -> f32 {
    (-yaw).to_degrees().rem_euclid(360.0)
}

/// The two places the compass points at from `(x, z)`: the vault ('V') and the
/// nearest warming shelter ('H'), as `(bearing, letter)` markers.
pub fn markers(x: f32, z: f32) -> Vec<(f32, char)> {
    let mut markers = vec![(bearing(VAULT_POS.0 - x, VAULT_POS.1 - z), 'V')];
    if let Some(&(sx, sz)) = SHELTERS.iter().min_by(|a, b| {
        let da = (a.0 - x).hypot(a.1 - z);
        let db = (b.0 - x).hypot(b.1 - z);
        da.total_cmp(&db)
    }) {
        markers.push((bearing(sx - x, sz - z), 'H'));
    }
    markers
}

/// The line under the strip: the heading as three digits and the legend.
pub fn readout(heading: f32) -> String {
    let deg = (heading.round() as i32).rem_euclid(360);
    format!("{deg:03}   V vault   H shelter")
}

/// A strip of compass ticks centred on `heading`, 2 degrees per character,
/// with markers for points of interest.
pub fn compass_strip(heading: f32, markers: &[(f32, char)]) -> String {
    const HALF: i32 = 30;
    let mut out = String::with_capacity(64);
    for i in -HALF..=HALF {
        let a = (heading + i as f32 * 2.0).rem_euclid(360.0);
        let near = |target: f32| {
            let d = (a - target + 540.0).rem_euclid(360.0) - 180.0;
            d.abs() < 1.0
        };
        let mut c = if near(0.0) {
            'N'
        } else if near(90.0) {
            'E'
        } else if near(180.0) {
            'S'
        } else if near(270.0) {
            'W'
        } else if [45.0, 135.0, 225.0, 315.0].iter().any(|&t| near(t)) {
            '+'
        } else if (a.round() as i32) % 10 == 0 {
            '|'
        } else {
            '.'
        };
        for &(target, m) in markers {
            if near(target) {
                c = m;
            }
        }
        if i == 0 && !"NESWVH".contains(c) {
            c = '^';
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearings_follow_the_map() {
        assert!((bearing(0.0, -1.0) - 0.0).abs() < 1e-3, "north is -Z");
        assert!((bearing(1.0, 0.0) - 90.0).abs() < 1e-3, "east is +X");
        assert!((bearing(0.0, 1.0) - 180.0).abs() < 1e-3);
        assert!((bearing(-1.0, 0.0) - 270.0).abs() < 1e-3);
    }

    #[test]
    fn compass_centres_on_heading() {
        let strip = compass_strip(0.0, &[]);
        assert_eq!(strip.chars().count(), 61);
        assert_eq!(strip.chars().nth(30), Some('N'));
        assert_eq!(strip.chars().nth(30 + 45), None);
        let east = compass_strip(90.0, &[(100.0, 'V')]);
        assert_eq!(east.chars().nth(30), Some('E'));
        assert_eq!(east.chars().nth(35), Some('V'), "a marker 10 degrees right sits 5 chars right");
    }

    #[test]
    fn the_strip_always_has_a_centre_mark_and_never_breaks_on_garbage() {
        for h in [-720.0, -1.0, 0.0, 44.9, 359.99, 360.0, 1e9, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let strip = compass_strip(h, &markers(0.0, 150.0));
            assert_eq!(strip.chars().count(), 61, "heading {h}");
            assert!(strip.chars().nth(30).is_some_and(|c| "NESWVH^".contains(c)), "heading {h}: {strip}");
        }
        // A marker dead ahead shows as itself in the middle.
        assert_eq!(compass_strip(100.0, &[(100.0, 'V')]).chars().nth(30), Some('V'));
        // Wrapping round north: a marker at 359 seen from heading 1 sits one char left.
        assert_eq!(compass_strip(1.0, &[(359.0, 'H')]).chars().nth(29), Some('H'));
    }

    #[test]
    fn looking_left_turns_the_heading_towards_west() {
        assert_eq!(heading(0.0), 0.0);
        assert!((heading(std::f32::consts::FRAC_PI_2) - 270.0).abs() < 1e-3, "turned left, facing west");
        assert!((heading(-std::f32::consts::FRAC_PI_2) - 90.0).abs() < 1e-3, "turned right, facing east");
        assert!((0.0..360.0).contains(&heading(-1e-9)), "wraps into range");
        assert_eq!(readout(0.0), "000   V vault   H shelter");
        assert_eq!(readout(359.7), "000   V vault   H shelter", "rounds up round the dial");
        assert_eq!(readout(90.0), "090   V vault   H shelter");
    }

    #[test]
    fn the_markers_point_at_the_vault_and_the_nearest_shelter() {
        // From the spawn the vault is dead south and the nearest fish house is Lundgren's.
        let m = markers(0.0, 152.0);
        assert_eq!(m[0].1, 'V');
        assert!((m[0].0 - 180.0).abs() < 1e-3);
        assert_eq!(m[1].1, 'H');
        assert!((m[1].0 - bearing(SHELTERS[0].0, SHELTERS[0].1 - 152.0)).abs() < 1e-6);
        // Standing at another shelter, that one is the nearest.
        let (sx, sz) = SHELTERS[1];
        let m = markers(sx + 3.0, sz);
        assert!((m[1].0 - bearing(-3.0, 0.0)).abs() < 1e-3);
        // The old inline code, number for number.
        for (x, z) in [(0.0f32, 0.0f32), (-150.0, -150.0), (120.0, 40.0), (f32::NAN, 0.0)] {
            let mut old = vec![(bearing(VAULT_POS.0 - x, VAULT_POS.1 - z), 'V')];
            if let Some(&(sx, sz)) = SHELTERS.iter().min_by(|a, b| (a.0 - x).hypot(a.1 - z).total_cmp(&(b.0 - x).hypot(b.1 - z))) {
                old.push((bearing(sx - x, sz - z), 'H'));
            }
            let new = markers(x, z);
            assert_eq!(new.len(), old.len());
            for (n, o) in new.iter().zip(&old) {
                assert!(n.0.to_bits() == o.0.to_bits() && n.1 == o.1);
            }
        }
    }
}
