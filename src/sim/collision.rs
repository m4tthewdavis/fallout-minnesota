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

#[cfg(test)]
mod tests {
    use super::*;

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
