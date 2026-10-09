//! Simple 2D (top-down) collision: circles for trees, barrels and silos,
//! rectangles for walls, fish houses and the vault hillside, capsules for
//! long thin things (fallen logs).
//!
//! Moving things are circles that get pushed out of every overlapping shape,
//! so they slide along walls instead of stopping dead.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle {
        x: f32,
        z: f32,
        r: f32,
    },
    /// Axis-aligned rectangle from (x0, z0) to (x1, z1).
    Rect {
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
    },
    /// A segment from (x0, z0) to (x1, z1) thickened by radius `r`: a fallen
    /// log is one of these instead of a row of circles. A zero-length
    /// capsule is a circle. Counts as one "tree" in [`segment_cover`].
    Capsule {
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        r: f32,
    },
}

/// Closest point to (px, pz) on the segment (x0, z0)-(x1, z1).
fn closest_on_segment(px: f32, pz: f32, x0: f32, z0: f32, x1: f32, z1: f32) -> (f32, f32) {
    let (dx, dz) = (x1 - x0, z1 - z0);
    let len2 = dx * dx + dz * dz;
    if len2 < 1e-12 {
        return (x0, z0);
    }
    let t = (((px - x0) * dx + (pz - z0) * dz) / len2).clamp(0.0, 1.0);
    (x0 + dx * t, z0 + dz * t)
}

fn dist_to_segment(px: f32, pz: f32, x0: f32, z0: f32, x1: f32, z1: f32) -> f32 {
    let (cx, cz) = closest_on_segment(px, pz, x0, z0, x1, z1);
    (px - cx).hypot(pz - cz)
}

/// Shortest distance between segments `a`-`b` and `c`-`d` (0 if they cross).
fn segment_distance(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> f32 {
    let cross = |o: (f32, f32), p: (f32, f32), q: (f32, f32)| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let (d1, d2, d3, d4) = (cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    dist_to_segment(a.0, a.1, c.0, c.1, d.0, d.1)
        .min(dist_to_segment(b.0, b.1, c.0, c.1, d.0, d.1))
        .min(dist_to_segment(c.0, c.1, a.0, a.1, b.0, b.1))
        .min(dist_to_segment(d.0, d.1, a.0, a.1, b.0, b.1))
}

impl Shape {
    /// A log (or any rod) of radius `r` from (x0, z0) to (x1, z1).
    pub fn capsule(x0: f32, z0: f32, x1: f32, z1: f32, r: f32) -> Self {
        Shape::Capsule { x0, z0, x1, z1, r }
    }

    /// Rectangle from a centre point and full width (x) / depth (z).
    pub fn rect_centered(cx: f32, cz: f32, width: f32, depth: f32) -> Self {
        Shape::Rect {
            x0: cx - width / 2.0,
            z0: cz - depth / 2.0,
            x1: cx + width / 2.0,
            z1: cz + depth / 2.0,
        }
    }

    /// Push a circle at (x, z) with `radius` out of this shape.
    fn push(&self, x: f32, z: f32, radius: f32) -> (f32, f32) {
        match *self {
            Shape::Circle { x: cx, z: cz, r } => {
                let (dx, dz) = (x - cx, z - cz);
                let min = r + radius;
                let d2 = dx * dx + dz * dz;
                if d2 >= min * min {
                    return (x, z);
                }
                let d = d2.sqrt();
                if d < 1e-4 {
                    (cx + min, z)
                } else {
                    (cx + dx / d * min, cz + dz / d * min)
                }
            }
            Shape::Rect { x0, z0, x1, z1 } => {
                // Quick reject.
                if x < x0 - radius || x > x1 + radius || z < z0 - radius || z > z1 + radius {
                    return (x, z);
                }
                let (cx, cz) = (x.clamp(x0, x1), z.clamp(z0, z1));
                let (dx, dz) = (x - cx, z - cz);
                let d2 = dx * dx + dz * dz;
                if d2 > 1e-8 {
                    if d2 >= radius * radius {
                        return (x, z);
                    }
                    let d = d2.sqrt();
                    return (cx + dx / d * radius, cz + dz / d * radius);
                }
                // Centre is inside the rectangle: leave by the nearest side.
                let left = x - x0;
                let right = x1 - x;
                let back = z - z0;
                let front = z1 - z;
                let min = left.min(right).min(back).min(front);
                if min == left {
                    (x0 - radius, z)
                } else if min == right {
                    (x1 + radius, z)
                } else if min == back {
                    (x, z0 - radius)
                } else {
                    (x, z1 + radius)
                }
            }
            Shape::Capsule { x0, z0, x1, z1, r } => {
                let (cx, cz) = closest_on_segment(x, z, x0, z0, x1, z1);
                let (dx, dz) = (x - cx, z - cz);
                let min = r + radius;
                let d2 = dx * dx + dz * dz;
                if d2 >= min * min {
                    return (x, z);
                }
                let d = d2.sqrt();
                if d >= 1e-4 {
                    return (cx + dx / d * min, cz + dz / d * min);
                }
                // Dead on the axis: leave sideways, off the log's flank.
                let (ax, az) = (x1 - x0, z1 - z0);
                let len = ax.hypot(az);
                if len < 1e-4 {
                    (cx + min, z) // a zero-length capsule is a circle
                } else {
                    (cx - az / len * min, cz + ax / len * min)
                }
            }
        }
    }
}

/// Resolve a moving circle against all shapes. Two passes handle corners
/// where two shapes overlap.
pub fn push_out(mut x: f32, mut z: f32, radius: f32, shapes: &[Shape]) -> (f32, f32) {
    for _ in 0..2 {
        for s in shapes {
            let (nx, nz) = s.push(x, z, radius);
            x = nx;
            z = nz;
        }
    }
    (x, z)
}

/// Like [`push_out`] for a mover that goes from `from` to `to` in one step:
/// walks the step in short hops so a fast mover (a sprint, a lurch, a long
/// frame) can't jump clean over a thin log or wall. Returns the final spot.
#[allow(dead_code)]
pub fn move_and_push(from: (f32, f32), to: (f32, f32), radius: f32, shapes: &[Shape]) -> (f32, f32) {
    const HOP: f32 = 0.2;
    let (dx, dz) = (to.0 - from.0, to.1 - from.1);
    let dist = dx.hypot(dz);
    if !dist.is_finite() {
        return push_out(from.0, from.1, radius, shapes);
    }
    let hops = ((dist / HOP).ceil() as usize).clamp(1, 500);
    let (mut x, mut z) = from;
    for _ in 0..hops {
        // Step from where the last push left us, in the original direction.
        let (nx, nz) = (x + dx / hops as f32, z + dz / hops as f32);
        let (px, pz) = push_out(nx, nz, radius, shapes);
        x = px;
        z = pz;
    }
    (x, z)
}

/// True if a circle overlaps any shape.
pub fn blocked(x: f32, z: f32, radius: f32, shapes: &[Shape]) -> bool {
    shapes.iter().any(|s| s.push(x, z, radius) != (x, z))
}

impl Shape {
    fn contains(&self, x: f32, z: f32) -> bool {
        match *self {
            Shape::Circle { x: cx, z: cz, r } => (x - cx).hypot(z - cz) < r,
            Shape::Rect { x0, z0, x1, z1 } => x > x0 && x < x1 && z > z0 && z < z1,
            Shape::Capsule { x0, z0, x1, z1, r } => dist_to_segment(x, z, x0, z0, x1, z1) < r,
        }
    }

    /// Does the segment from `a` to `b` pass through this shape?
    fn crossed_by(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        match *self {
            Shape::Circle { x: cx, z: cz, r } => {
                let len2 = dx * dx + dz * dz;
                let t = if len2 < 1e-9 { 0.0 } else { (((cx - a.0) * dx + (cz - a.1) * dz) / len2).clamp(0.0, 1.0) };
                (a.0 + dx * t - cx).hypot(a.1 + dz * t - cz) < r
            }
            Shape::Rect { x0, z0, x1, z1 } => {
                // Slab method.
                let (mut t0, mut t1) = (0.0f32, 1.0f32);
                for (p, d, lo, hi) in [(a.0, dx, x0, x1), (a.1, dz, z0, z1)] {
                    if d.abs() < 1e-9 {
                        if p < lo || p > hi {
                            return false;
                        }
                    } else {
                        let (u, v) = ((lo - p) / d, (hi - p) / d);
                        t0 = t0.max(u.min(v));
                        t1 = t1.min(u.max(v));
                        if t0 > t1 {
                            return false;
                        }
                    }
                }
                true
            }
            Shape::Capsule { x0, z0, x1, z1, r } => segment_distance(a, b, (x0, z0), (x1, z1)) < r,
        }
    }
}

/// True if something solid stands between `a` and `b` (a line of sight is
/// cut). A shape that either end is inside of (someone leaning on a wall)
/// doesn't count, so neither can hide from the other behind what they touch.
pub fn segment_blocked(a: (f32, f32), b: (f32, f32), shapes: &[Shape]) -> bool {
    shapes.iter().any(|s| !s.contains(a.0, a.1) && !s.contains(b.0, b.1) && s.crossed_by(a, b))
}

/// How many trees (circles) and walls (rectangles) stand between `a` and
/// `b`: what a sound has to get through. Shapes either end is touching don't count.
pub fn segment_cover(a: (f32, f32), b: (f32, f32), shapes: &[Shape]) -> (usize, usize) {
    let (mut circles, mut rects) = (0, 0);
    for s in shapes {
        if s.contains(a.0, a.1) || s.contains(b.0, b.1) || !s.crossed_by(a, b) {
            continue;
        }
        match s {
            Shape::Circle { .. } | Shape::Capsule { .. } => circles += 1,
            Shape::Rect { .. } => rects += 1,
        }
    }
    (circles, rects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_counts_what_is_in_the_way() {
        let shapes = [
            Shape::Circle { x: 3.0, z: 0.0, r: 0.5 },
            Shape::Circle { x: 6.0, z: 0.1, r: 0.5 },
            Shape::Circle { x: 6.0, z: 5.0, r: 0.5 },
            Shape::rect_centered(8.0, 0.0, 1.0, 4.0),
        ];
        assert_eq!(segment_cover((0.0, 0.0), (12.0, 0.0), &shapes), (2, 1));
        assert_eq!(segment_cover((0.0, 0.0), (2.0, 0.0), &shapes), (0, 0));
        assert_eq!(segment_cover((0.0, 0.0), (12.0, 8.0), &shapes), (0, 0));
        assert_eq!(segment_cover((3.0, 0.2), (12.0, 0.0), &shapes).0, 1, "the tree it stands in doesn't count");
    }

    #[test]
    fn a_tree_or_a_wall_cuts_the_line_of_sight() {
        let tree = [Shape::Circle { x: 5.0, z: 0.0, r: 0.6 }];
        assert!(segment_blocked((0.0, 0.0), (10.0, 0.0), &tree));
        assert!(!segment_blocked((0.0, 0.0), (10.0, 3.0), &tree), "passes well to one side");
        assert!(!segment_blocked((0.0, 0.0), (4.0, 0.0), &tree), "stops short of it");
        let wall = [Shape::rect_centered(5.0, 0.0, 1.0, 6.0)];
        assert!(segment_blocked((0.0, 0.0), (10.0, 0.0), &wall));
        assert!(segment_blocked((0.0, -2.0), (10.0, 2.0), &wall), "diagonally through it");
        assert!(!segment_blocked((0.0, 4.0), (10.0, 4.0), &wall), "over the end of it");
        assert!(!segment_blocked((0.0, 0.0), (10.0, 0.0), &[]));
    }

    #[test]
    fn leaning_on_a_wall_does_not_hide_you_from_it() {
        let wall = [Shape::rect_centered(0.0, 0.0, 10.0, 2.0)];
        // Standing inside the (padded) collider, looking out.
        assert!(!segment_blocked((0.0, 0.5), (0.0, 20.0), &wall));
        let tree = [Shape::Circle { x: 0.0, z: 0.0, r: 1.0 }];
        assert!(!segment_blocked((0.5, 0.0), (10.0, 0.0), &tree));
    }

    #[test]
    fn clear_space_is_untouched() {
        let shapes = [
            Shape::Circle {
                x: 10.0,
                z: 0.0,
                r: 1.0,
            },
            Shape::rect_centered(-10.0, 0.0, 2.0, 2.0),
        ];
        assert_eq!(push_out(0.0, 0.0, 0.4, &shapes), (0.0, 0.0));
        assert!(!blocked(0.0, 0.0, 0.4, &shapes));
    }

    #[test]
    fn pushed_out_of_a_tree() {
        let shapes = [Shape::Circle { x: 0.0, z: 0.0, r: 1.0 }];
        let (x, z) = push_out(0.5, 0.0, 0.4, &shapes);
        assert!((x - 1.4).abs() < 1e-4 && z.abs() < 1e-4);
    }

    #[test]
    fn slides_along_a_wall() {
        let wall = [Shape::rect_centered(0.0, 0.0, 10.0, 1.0)];
        // Walking into the wall's front face keeps x, fixes z.
        let (x, z) = push_out(2.0, 0.7, 0.4, &wall);
        assert!((x - 2.0).abs() < 1e-4);
        assert!((z - 0.9).abs() < 1e-4);
    }

    #[test]
    fn escapes_from_inside_a_rect() {
        let rect = [Shape::rect_centered(0.0, 0.0, 4.0, 4.0)];
        let (x, z) = push_out(1.8, 0.0, 0.5, &rect);
        assert!(!blocked(x, z, 0.49, &rect), "({x}, {z})");
        assert!(x > 2.0);
    }

    #[test]
    fn corner_push_is_diagonal() {
        let rect = [Shape::rect_centered(0.0, 0.0, 2.0, 2.0)];
        let (x, z) = push_out(1.2, 1.2, 0.5, &rect);
        let d = ((x - 1.0).powi(2) + (z - 1.0).powi(2)).sqrt();
        assert!((d - 0.5).abs() < 1e-3);
    }

    // ---- capsules (fallen logs) ----

    /// A log lying along x from (-3, 0) to (3, 0), radius 0.3.
    fn log() -> [Shape; 1] {
        [Shape::capsule(-3.0, 0.0, 3.0, 0.0, 0.3)]
    }

    #[test]
    fn pushed_off_both_flanks_of_a_log() {
        let log = log();
        let (x, z) = push_out(1.0, 0.2, 0.4, &log);
        assert!((x - 1.0).abs() < 1e-4 && (z - 0.7).abs() < 1e-4, "({x}, {z})");
        let (x, z) = push_out(-1.5, -0.1, 0.4, &log);
        assert!((x + 1.5).abs() < 1e-4 && (z + 0.7).abs() < 1e-4, "({x}, {z})");
        assert!(!blocked(1.0, 0.71, 0.4, &log) && blocked(1.0, 0.69, 0.4, &log));
    }

    #[test]
    fn pushed_off_the_rounded_end_caps() {
        let log = log();
        // Along the axis past the right end: out by r + radius from the tip.
        let (x, z) = push_out(3.3, 0.0, 0.4, &log);
        assert!((x - 3.7).abs() < 1e-4 && z.abs() < 1e-4, "({x}, {z})");
        let (x, _) = push_out(-3.2, 0.0, 0.4, &log);
        assert!((x + 3.7).abs() < 1e-4);
        // Off the tip diagonally: pushed along the line from the tip.
        let (x, z) = push_out(3.3, 0.3, 0.4, &log);
        let d = ((x - 3.0).powi(2) + z.powi(2)).sqrt();
        assert!((d - 0.7).abs() < 1e-3 && x > 3.3 && z > 0.3, "({x}, {z})");
        // Just beyond the cap is free.
        assert!(!blocked(3.71, 0.0, 0.4, &log));
    }

    #[test]
    fn a_slanted_log_works_in_every_direction() {
        let log = [Shape::capsule(0.0, 0.0, 4.0, 4.0, 0.25)];
        let s = std::f32::consts::FRAC_1_SQRT_2;
        // Stand on the axis midpoint, nudged to one side: pushed out perpendicular by r + radius.
        let (x, z) = push_out(2.0 + 0.05, 2.0 - 0.05, 0.35, &log);
        assert!((x - (2.0 + 0.6 * s)).abs() < 1e-3 && (z - (2.0 - 0.6 * s)).abs() < 1e-3, "({x}, {z})");
        let (x, z) = push_out(2.0 - 0.05, 2.0 + 0.05, 0.35, &log);
        assert!((x - (2.0 - 0.6 * s)).abs() < 1e-3 && (z - (2.0 + 0.6 * s)).abs() < 1e-3);
    }

    #[test]
    fn dead_centre_on_the_log_still_escapes() {
        let log = log();
        let (x, z) = push_out(0.0, 0.0, 0.4, &log);
        assert!(x.is_finite() && z.is_finite());
        assert!(!blocked(x, z, 0.39, &log), "({x}, {z})");
        // Same from exactly on a cap tip.
        let (x, z) = push_out(3.0, 0.0, 0.4, &log);
        assert!(!blocked(x, z, 0.39, &log), "({x}, {z})");
    }

    #[test]
    fn a_zero_length_capsule_is_a_circle() {
        let cap = [Shape::capsule(2.0, -1.0, 2.0, -1.0, 0.8)];
        let circle = [Shape::Circle { x: 2.0, z: -1.0, r: 0.8 }];
        for (x, z) in [(2.3, -1.0), (2.0, -0.5), (1.0, -1.4), (2.0, -1.0), (5.0, 5.0)] {
            assert_eq!(push_out(x, z, 0.4, &cap), push_out(x, z, 0.4, &circle), "at ({x}, {z})");
            assert_eq!(blocked(x, z, 0.4, &cap), blocked(x, z, 0.4, &circle));
        }
        for (a, b) in [((0.0, -1.0), (6.0, -1.0)), ((0.0, 3.0), (6.0, 3.0)), ((2.5, -1.0), (9.0, 0.0))] {
            assert_eq!(segment_blocked(a, b, &cap), segment_blocked(a, b, &circle), "{a:?}->{b:?}");
            assert_eq!(segment_cover(a, b, &cap), segment_cover(a, b, &circle));
        }
    }

    #[test]
    fn a_fast_mover_cannot_tunnel_through_a_log() {
        let log = log();
        // A big single step straight across: plain push_out lets it jump clean over...
        let (_, z) = push_out(0.0, -3.0 + 6.0, 0.4, &log);
        assert!(z > 0.0, "(the single-step hole this guards against)");
        // ...the swept move stops it on the near side.
        for speed in [5.0f32, 12.0, 30.0, 90.0] {
            let (mut x, mut z) = (0.5, -4.0);
            for _ in 0..200 {
                let (nx, nz) = move_and_push((x, z), (x, z + speed / 60.0), 0.4, &log);
                x = nx;
                z = nz;
            }
            assert!(z < 0.0 && z <= -0.69, "speed {speed}: ended at z={z}");
            assert!((x - 0.5).abs() < 1e-3, "slides, doesn't shove sideways");
        }
        // One enormous step too, and through the thin end.
        let (_, z) = move_and_push((0.0, -4.0), (0.0, 4.0), 0.4, &log);
        assert!(z < 0.0, "z={z}");
        let (x, _) = move_and_push((-8.0, 0.0), (8.0, 0.0), 0.4, &log);
        assert!(x < -3.0, "x={x}");
    }

    #[test]
    fn swept_moves_are_plain_moves_in_open_ground_and_never_blow_up() {
        let log = log();
        assert_eq!(move_and_push((10.0, 10.0), (10.0, 10.0), 0.4, &log), (10.0, 10.0));
        let (x, z) = move_and_push((10.0, 10.0), (12.0, 13.0), 0.4, &log);
        assert!((x - 12.0).abs() < 1e-3 && (z - 13.0).abs() < 1e-3);
        // Walls and trees get the same protection.
        let wall = [Shape::rect_centered(0.0, 0.0, 0.2, 6.0)];
        let (x, _) = move_and_push((-5.0, 0.0), (5.0, 0.0), 0.3, &wall);
        assert!(x < 0.0);
        // Garbage in, no panic out.
        let r = move_and_push((0.0, 0.0), (f32::NAN, 1.0), 0.4, &log);
        let _ = r;
        let _ = move_and_push((0.0, 0.0), (f32::INFINITY, 1.0), 0.4, &log);
    }

    #[test]
    fn a_log_cuts_the_line_of_sight_once_and_only_where_it_lies() {
        let log = log();
        assert!(segment_blocked((0.0, -5.0), (0.0, 5.0), &log));
        assert!(segment_blocked((3.2, -2.0), (3.2, 2.0), &log), "through the cap");
        assert!(!segment_blocked((3.4, -2.0), (3.4, 2.0), &log), "past the end of it");
        assert!(!segment_blocked((-8.0, 1.0), (8.0, 1.0), &log), "alongside it");
        assert!(segment_blocked((-8.0, 0.2), (8.0, 0.2), &log), "grazing along its length");
        assert!(!segment_blocked((0.0, 2.0), (0.0, 5.0), &log), "stops short");
        // Parallel and just clear of the radius.
        assert!(!segment_blocked((-8.0, 0.31), (8.0, 0.31), &log));
    }

    #[test]
    fn leaning_on_a_log_does_not_hide_you_from_it() {
        let log = log();
        assert!(!segment_blocked((0.0, 0.2), (0.0, 20.0), &log));
        assert!(!segment_blocked((0.0, 20.0), (0.0, 0.2), &log));
        assert_eq!(segment_cover((0.0, 0.2), (0.0, 20.0), &log), (0, 0));
    }

    #[test]
    fn segment_cover_counts_a_log_once_not_per_circle() {
        let log = log();
        // A sound straight along a 6 m log crosses what used to be many circles.
        assert_eq!(segment_cover((-8.0, 0.0), (8.0, 0.0), &log), (1, 0));
        assert_eq!(segment_cover((0.0, -5.0), (0.0, 5.0), &log), (1, 0));
        let three_circles: Vec<Shape> = (0..3).map(|i| Shape::Circle { x: -2.0 + 2.0 * i as f32, z: 0.0, r: 0.3 }).collect();
        assert_eq!(segment_cover((-8.0, 0.0), (8.0, 0.0), &three_circles).0, 3, "the old way over-counted");
        // Mixed with a tree and a wall, each counts once.
        let mixed = [log[0], Shape::Circle { x: 6.0, z: 0.0, r: 0.5 }, Shape::rect_centered(8.0, 0.0, 1.0, 4.0)];
        assert_eq!(segment_cover((-8.0, 0.0), (12.0, 0.0), &mixed), (2, 1));
        assert_eq!(segment_cover((-8.0, 2.0), (12.0, 2.0), &log), (0, 0));
    }

    #[test]
    fn degenerate_capsules_stay_finite() {
        // Radius zero is a bare stick; negative radius is nothing at all.
        let stick = [Shape::capsule(-1.0, 0.0, 1.0, 0.0, 0.0)];
        assert!(!segment_blocked((0.0, -1.0), (0.0, 1.0), &stick));
        let (x, z) = push_out(0.0, 0.0, 0.3, &stick);
        assert!(!blocked(x, z, 0.29, &stick) && x.is_finite() && z.is_finite());
        let nothing = [Shape::capsule(-1.0, 0.0, 1.0, 0.0, -1.0)];
        assert!(!segment_blocked((0.0, -1.0), (0.0, 1.0), &nothing));
        // Reversed endpoints are the same log.
        let a = [Shape::capsule(3.0, 0.0, -3.0, 0.0, 0.3)];
        assert_eq!(push_out(1.0, 0.2, 0.4, &a), push_out(1.0, 0.2, 0.4, &log()));
        // Zero-length query segment inside and outside.
        assert!(!segment_blocked((5.0, 5.0), (5.0, 5.0), &log()));
    }

    #[test]
    fn push_out_never_leaves_a_mover_inside_a_crossing_of_logs() {
        // Two logs crossing in an X, like deadfall.
        let shapes = [Shape::capsule(-3.0, -3.0, 3.0, 3.0, 0.3), Shape::capsule(-3.0, 3.0, 3.0, -3.0, 0.3)];
        for (x, z) in [(0.0, 0.0), (0.1, 0.0), (0.0, 0.5), (1.0, 1.0), (-1.0, 1.1)] {
            let (px, pz) = push_out(x, z, 0.4, &shapes);
            assert!(!blocked(px, pz, 0.39, &shapes), "({x}, {z}) -> ({px}, {pz})");
        }
    }
}
