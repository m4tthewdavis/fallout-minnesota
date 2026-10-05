//! Simple 2D (top-down) collision: circles for trees, barrels and silos,
//! rectangles for walls, fish houses and the vault hillside.
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
}

impl Shape {
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

/// True if a circle overlaps any shape.
pub fn blocked(x: f32, z: f32, radius: f32, shapes: &[Shape]) -> bool {
    shapes.iter().any(|s| s.push(x, z, radius) != (x, z))
}

impl Shape {
    fn contains(&self, x: f32, z: f32) -> bool {
        match *self {
            Shape::Circle { x: cx, z: cz, r } => (x - cx).hypot(z - cz) < r,
            Shape::Rect { x0, z0, x1, z1 } => x > x0 && x < x1 && z > z0 && z < z1,
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
        }
    }
}

/// True if something solid stands between `a` and `b` (a line of sight is
/// cut). A shape that either end is inside of (someone leaning on a wall)
/// doesn't count, so neither can hide from the other behind what they touch.
pub fn segment_blocked(a: (f32, f32), b: (f32, f32), shapes: &[Shape]) -> bool {
    shapes.iter().any(|s| !s.contains(a.0, a.1) && !s.contains(b.0, b.1) && s.crossed_by(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
