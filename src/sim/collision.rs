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

// ---------------------------------------------------------------------------
// A spatial index for the same queries
// ---------------------------------------------------------------------------

use super::terrain::HALF_SIZE;

/// Slack (metres) added around every box in the index. Float rounding in the
/// shape tests is a few hundred-thousandths of a metre even a kilometre from
/// the origin; a centimetre of slack means the index can never leave out a
/// shape the plain scan would have found.
const SLACK: f32 = 0.01;

/// Width of the coarse cells used for shapes outside the main grid (the
/// interiors are built 1000 m off the map, 60 m apart).
const FAR_CELL: f32 = 64.0;

impl Shape {
    /// Box (x0, z0, x1, z1) outside of which this shape cannot move a circle
    /// of any radius up to `reach` (and cannot be crossed by a segment).
    /// `None` for a shape with a non-finite number in it.
    fn bounds(&self) -> Option<[f32; 4]> {
        let b = match *self {
            Shape::Circle { x, z, r } => [x - r.abs(), z - r.abs(), x + r.abs(), z + r.abs()],
            Shape::Rect { x0, z0, x1, z1 } => [x0.min(x1), z0.min(z1), x0.max(x1), z0.max(z1)],
            Shape::Capsule { x0, z0, x1, z1, r } => [x0.min(x1) - r.abs(), z0.min(z1) - r.abs(), x0.max(x1) + r.abs(), z0.max(z1) + r.abs()],
        };
        b.iter().all(|v| v.is_finite()).then_some(b)
    }
}

/// A set of shapes with the same answers as the free functions above
/// ([`push_out`], [`blocked`], [`segment_blocked`], [`segment_cover`],
/// [`move_and_push`]), but without testing every shape every time. The
/// shapes are kept in a uniform grid of 8 m cells, and a query only looks at
/// the cells it touches: a few shapes instead of a thousand. Results are
/// identical to scanning the whole list in order (the tests compare them bit
/// for bit), including for shapes far off the map and for garbage input,
/// which falls back to the full scan.
///
/// It stands in for a `Vec<Shape>`: [`push`](Self::push) appends and it
/// derefs to the shapes in insertion order, so code that still takes a
/// `&[Shape]` keeps working (with the full scan).
#[derive(Clone, Debug)]
pub struct SpatialGrid {
    shapes: Vec<Shape>,
    /// Lower corner and side length of the square grid, and its cells per side.
    min: f32,
    cell: f32,
    side: usize,
    /// Indices into `shapes`, per cell, in insertion order.
    cells: Vec<Vec<u32>>,
    /// Shapes not (entirely) inside the grid, by coarse cell.
    far: Vec<((i32, i32), Vec<u32>)>,
    /// Shapes that can't be placed (not finite, or enormous): tested every time.
    always: Vec<u32>,
}

impl Default for SpatialGrid {
    /// A grid over the whole map with a little margin.
    fn default() -> Self {
        SpatialGrid::new(HALF_SIZE + 8.0, 8.0)
    }
}

impl From<Vec<Shape>> for SpatialGrid {
    fn from(shapes: Vec<Shape>) -> Self {
        let mut grid = SpatialGrid::default();
        for s in shapes {
            grid.push(s);
        }
        grid
    }
}

impl std::ops::Deref for SpatialGrid {
    type Target = [Shape];
    fn deref(&self) -> &[Shape] {
        &self.shapes
    }
}

/// The few shapes a query has to look at: indices, sorted and without
/// repeats. Lives on the stack unless a query touches more than 64.
struct Candidates {
    len: usize,
    stack: [u32; 64],
    heap: Vec<u32>,
}

impl Candidates {
    fn new() -> Self {
        Candidates { len: 0, stack: [0; 64], heap: Vec::new() }
    }

    fn add(&mut self, i: u32) {
        if self.heap.is_empty() && self.len < self.stack.len() {
            self.stack[self.len] = i;
            self.len += 1;
        } else {
            if self.heap.is_empty() {
                self.heap.extend_from_slice(&self.stack[..self.len]);
            }
            self.heap.push(i);
        }
    }

    /// Sort and drop repeats; the result is in the order the shapes were added to the grid.
    fn sorted(&mut self) -> &[u32] {
        if !self.heap.is_empty() {
            self.heap.sort_unstable();
            self.heap.dedup();
            return &self.heap;
        }
        let s = &mut self.stack[..self.len];
        s.sort_unstable();
        let mut n = 0;
        for k in 0..s.len() {
            if k == 0 || s[k] != s[k - 1] {
                s[n] = s[k];
                n += 1;
            }
        }
        self.len = n;
        &self.stack[..n]
    }
}

impl SpatialGrid {
    /// A grid covering `-half..half` on both axes with square cells `cell` metres wide.
    /// (Shapes outside it are still handled, just less quickly.)
    pub fn new(half: f32, cell: f32) -> Self {
        let cell = if cell.is_finite() && cell > 0.5 { cell } else { 8.0 };
        let half = if half.is_finite() && half > cell { half.min(2000.0) } else { 208.0 };
        let side = (2.0 * half / cell).ceil() as usize;
        SpatialGrid { shapes: Vec::new(), min: -half, cell, side, cells: vec![Vec::new(); side * side], far: Vec::new(), always: Vec::new() }
    }

    /// Add a shape (like `Vec::push`); it goes last in the order.
    pub fn push(&mut self, shape: Shape) {
        let id = self.shapes.len() as u32;
        self.shapes.push(shape);
        let Some([x0, z0, x1, z1]) = shape.bounds() else {
            self.always.push(id);
            return;
        };
        let (x0, z0, x1, z1) = (x0 - SLACK, z0 - SLACK, x1 + SLACK, z1 + SLACK);
        let max = self.min + self.side as f32 * self.cell;
        let inside = x0 >= self.min && z0 >= self.min && x1 <= max && z1 <= max;
        // The part in the grid.
        if x1 >= self.min && z1 >= self.min && x0 <= max && z0 <= max {
            let (ix0, ix1, iz0, iz1) = self.cell_range(x0, z0, x1, z1);
            for iz in iz0..=iz1 {
                for ix in ix0..=ix1 {
                    self.cells[iz * self.side + ix].push(id);
                }
            }
        }
        // The part outside it, by coarse cell.
        if !inside {
            let (fx0, fx1, fz0, fz1) = ((x0 / FAR_CELL).floor(), (x1 / FAR_CELL).floor(), (z0 / FAR_CELL).floor(), (z1 / FAR_CELL).floor());
            if (fx1 - fx0 + 1.0) * (fz1 - fz0 + 1.0) > 4096.0 {
                self.always.push(id);
                return;
            }
            for kz in fz0 as i32..=fz1 as i32 {
                for kx in fx0 as i32..=fx1 as i32 {
                    match self.far.iter_mut().find(|(k, _)| *k == (kx, kz)) {
                        Some((_, list)) => list.push(id),
                        None => self.far.push(((kx, kz), vec![id])),
                    }
                }
            }
        }
    }

    /// The shapes, in the order they were added.
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    /// Cell index range covering a box (clamped to the grid).
    fn cell_range(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> (usize, usize, usize, usize) {
        let to = |v: f32| (((v - self.min) / self.cell).floor().max(0.0) as usize).min(self.side - 1);
        (to(x0), to(x1), to(z0), to(z1))
    }

    /// Every shape that could touch the box (and some that don't).
    fn near_box(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> Candidates {
        let (x0, z0, x1, z1) = (x0 - SLACK, z0 - SLACK, x1 + SLACK, z1 + SLACK);
        let mut c = Candidates::new();
        for &i in &self.always {
            c.add(i);
        }
        let max = self.min + self.side as f32 * self.cell;
        if x1 >= self.min && z1 >= self.min && x0 <= max && z0 <= max {
            let (ix0, ix1, iz0, iz1) = self.cell_range(x0, z0, x1, z1);
            for iz in iz0..=iz1 {
                for ix in ix0..=ix1 {
                    for &i in &self.cells[iz * self.side + ix] {
                        c.add(i);
                    }
                }
            }
        }
        self.add_far(&mut c, x0, z0, x1, z1);
        c
    }

    /// Add the shapes filed by coarse cell that a box (already padded) overlaps,
    /// if it isn't wholly inside the main grid.
    fn add_far(&self, c: &mut Candidates, x0: f32, z0: f32, x1: f32, z1: f32) {
        let max = self.min + self.side as f32 * self.cell;
        if self.far.is_empty() || (x0 >= self.min && z0 >= self.min && x1 <= max && z1 <= max) {
            return;
        }
        for &((kx, kz), ref list) in &self.far {
            let (cx0, cz0) = (kx as f32 * FAR_CELL, kz as f32 * FAR_CELL);
            if x1 >= cx0 && x0 <= cx0 + FAR_CELL && z1 >= cz0 && z0 <= cz0 + FAR_CELL {
                for &i in list {
                    c.add(i);
                }
            }
        }
    }

    /// Every shape that could touch the segment `a`-`b` (and some that don't):
    /// the shapes in the cells the segment passes through, not the whole box
    /// around it, which for a long diagonal is mostly empty air.
    fn near_segment(&self, a: (f32, f32), b: (f32, f32)) -> Candidates {
        let mut c = Candidates::new();
        for &i in &self.always {
            c.add(i);
        }
        let (bx0, bz0, bx1, bz1) = (a.0.min(b.0) - SLACK, a.1.min(b.1) - SLACK, a.0.max(b.0) + SLACK, a.1.max(b.1) + SLACK);
        let max = self.min + self.side as f32 * self.cell;
        if bx1 >= self.min && bz1 >= self.min && bx0 <= max && bz0 <= max {
            // Walk along the longer axis, taking the cells the line occupies in each slab
            // of the other one (so the line's slope is at most 1).
            let wide = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
            let ((au, av), (bu, bv)) = if wide { (a, b) } else { ((a.1, a.0), (b.1, b.0)) };
            let (lo_u, hi_u) = (au.min(bu), au.max(bu));
            let slope = if bu != au { (bv - av) / (bu - au) } else { 0.0 };
            let to = |v: f32| (((v - self.min) / self.cell).floor().max(0.0) as usize).min(self.side - 1);
            for iu in to(lo_u - SLACK)..=to(hi_u + SLACK) {
                let (cu0, cu1) = (self.min + iu as f32 * self.cell - SLACK, self.min + (iu + 1) as f32 * self.cell + SLACK);
                let (ul, ur) = (lo_u.max(cu0), hi_u.min(cu1));
                if ul > ur {
                    continue;
                }
                let (vl, vr) = (av + (ul - au) * slope, av + (ur - au) * slope);
                let (iv0, iv1) = (to(vl.min(vr) - 2.0 * SLACK), to(vl.max(vr) + 2.0 * SLACK));
                for iv in iv0..=iv1 {
                    let idx = if wide { iv * self.side + iu } else { iu * self.side + iv };
                    for &i in &self.cells[idx] {
                        c.add(i);
                    }
                }
            }
        }
        self.add_far(&mut c, bx0, bz0, bx1, bz1);
        c
    }

    /// The same as [`push_out`] over all the shapes, in order.
    ///
    /// A shape only moves the circle if it is within `radius` of the shape's
    /// box, so first only those shapes are run, in order, twice over. Being
    /// pushed can carry the circle into the reach of a shape that wasn't
    /// near at the start, so the box the circle travelled through is checked
    /// afterwards, and the run is repeated with any extra shapes until
    /// nothing new turns up. At that point every shape left out is out of
    /// reach of every spot the circle visited, so the answer is exact.
    pub fn push_out(&self, x: f32, z: f32, radius: f32) -> (f32, f32) {
        if !(x.is_finite() && z.is_finite() && radius.is_finite()) {
            return push_out(x, z, radius, &self.shapes);
        }
        let reach = radius.abs();
        let mut seen = [x, z, x, z];
        let mut cand = self.near_box(x - reach, z - reach, x + reach, z + reach);
        loop {
            let ids = cand.sorted();
            let (mut px, mut pz) = (x, z);
            let mut vis = [x, z, x, z];
            for _ in 0..2 {
                for &i in ids {
                    let (nx, nz) = self.shapes[i as usize].push(px, pz, radius);
                    px = nx;
                    pz = nz;
                    vis = [vis[0].min(px), vis[1].min(pz), vis[2].max(px), vis[3].max(pz)];
                }
            }
            if !(px.is_finite() && pz.is_finite()) {
                return push_out(x, z, radius, &self.shapes);
            }
            // Did it go anywhere new? Then look again at everything near its whole track.
            let grown = vis[0] < seen[0] || vis[1] < seen[1] || vis[2] > seen[2] || vis[3] > seen[3];
            if !grown {
                return (px, pz);
            }
            seen = [seen[0].min(vis[0]), seen[1].min(vis[1]), seen[2].max(vis[2]), seen[3].max(vis[3])];
            let mut wider = self.near_box(seen[0] - reach, seen[1] - reach, seen[2] + reach, seen[3] + reach);
            if wider.sorted() == ids {
                return (px, pz);
            }
            cand = wider;
        }
    }

    /// The same as [`move_and_push`].
    #[allow(dead_code)]
    pub fn move_and_push(&self, from: (f32, f32), to: (f32, f32), radius: f32) -> (f32, f32) {
        const HOP: f32 = 0.2;
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let dist = dx.hypot(dz);
        if !dist.is_finite() {
            return self.push_out(from.0, from.1, radius);
        }
        let hops = ((dist / HOP).ceil() as usize).clamp(1, 500);
        let (mut x, mut z) = from;
        for _ in 0..hops {
            let (nx, nz) = (x + dx / hops as f32, z + dz / hops as f32);
            let (px, pz) = self.push_out(nx, nz, radius);
            x = px;
            z = pz;
        }
        (x, z)
    }

    /// The same as [`blocked`].
    pub fn blocked(&self, x: f32, z: f32, radius: f32) -> bool {
        if !(x.is_finite() && z.is_finite() && radius.is_finite()) {
            return blocked(x, z, radius, &self.shapes);
        }
        let reach = radius.abs();
        let mut cand = self.near_box(x - reach, z - reach, x + reach, z + reach);
        cand.sorted().iter().any(|&i| self.shapes[i as usize].push(x, z, radius) != (x, z))
    }

    /// The same as [`segment_blocked`].
    pub fn segment_blocked(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
            return segment_blocked(a, b, &self.shapes);
        }
        let mut cand = self.near_segment(a, b);
        cand.sorted().iter().any(|&i| {
            let s = &self.shapes[i as usize];
            !s.contains(a.0, a.1) && !s.contains(b.0, b.1) && s.crossed_by(a, b)
        })
    }

    /// The same as [`segment_cover`].
    pub fn segment_cover(&self, a: (f32, f32), b: (f32, f32)) -> (usize, usize) {
        if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
            return segment_cover(a, b, &self.shapes);
        }
        let (mut circles, mut rects) = (0, 0);
        let mut cand = self.near_segment(a, b);
        for &i in cand.sorted() {
            let s = &self.shapes[i as usize];
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
            assert!(z <= -0.69, "speed {speed}: ended at z={z}");
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

    // ---- The spatial index gives exactly the answers of the full scan ----------

    use crate::sim::rng::Rng;

    /// A world like the game's: lots of trees, some walls, logs, a few
    /// big buildings, and (optionally) rooms a kilometre off the map.
    fn world(seed: u64, n: usize, with_interiors: bool) -> Vec<Shape> {
        let mut r = Rng::new(seed);
        let mut v = Vec::new();
        for k in 0..n {
            let (x, z) = (r.range(-200.0, 200.0), r.range(-200.0, 200.0));
            v.push(match k % 25 {
                0..=18 => Shape::Circle { x, z, r: r.range(0.15, 1.6) },
                19..=21 => Shape::rect_centered(x, z, r.range(0.5, 9.0), r.range(0.5, 9.0)),
                22 => Shape::rect_centered(x, z, r.range(10.0, 40.0), r.range(0.3, 3.0)),
                _ => Shape::capsule(x, z, x + r.range(-4.0, 4.0), z + r.range(-4.0, 4.0), r.range(0.15, 0.5)),
            });
        }
        // Awkward ones: on the grid's edge, straddling it, enormous, a point, on a cell boundary.
        v.push(Shape::Circle { x: 207.9, z: 0.0, r: 1.0 });
        v.push(Shape::rect_centered(-208.0, 50.0, 6.0, 6.0));
        v.push(Shape::rect_centered(0.0, 0.0, 600.0, 0.5));
        v.push(Shape::capsule(5.0, 5.0, 5.0, 5.0, 0.7));
        v.push(Shape::Circle { x: 8.0, z: 8.0, r: 0.5 });
        v.push(Shape::Circle { x: -8.0, z: -16.0, r: 0.5 });
        v.push(Shape::Circle { x: 40.0, z: 40.0, r: -0.8 });
        if with_interiors {
            for room in 0..4 {
                let (ox, oz) = (1000.0 + 60.0 * room as f32, 1000.0);
                v.push(Shape::rect_centered(ox, oz - 6.0, 14.0, 0.4));
                v.push(Shape::rect_centered(ox, oz + 6.0, 14.0, 0.4));
                v.push(Shape::rect_centered(ox - 7.0, oz, 0.4, 12.0));
                v.push(Shape::rect_centered(ox + 7.0, oz, 0.4, 12.0));
                for _ in 0..12 {
                    v.push(Shape::Circle { x: ox + r.range(-5.0, 5.0), z: oz + r.range(-5.0, 5.0), r: r.range(0.2, 0.6) });
                }
            }
        }
        v
    }

    fn grid_of(shapes: &[Shape]) -> SpatialGrid {
        let mut g = SpatialGrid::default();
        for &s in shapes {
            g.push(s);
        }
        g
    }

    /// Query spots: spread over the map and past its edge, near every shape,
    /// and in the interiors.
    fn spots(shapes: &[Shape], seed: u64, n: usize) -> Vec<(f32, f32)> {
        let mut r = Rng::new(seed);
        let mut v: Vec<(f32, f32)> = (0..n).map(|_| (r.range(-225.0, 225.0), r.range(-225.0, 225.0))).collect();
        for s in shapes.iter().step_by(3) {
            let (cx, cz) = match *s {
                Shape::Circle { x, z, .. } => (x, z),
                Shape::Rect { x0, z0, x1, z1 } => ((x0 + x1) / 2.0, (z0 + z1) / 2.0),
                Shape::Capsule { x0, z0, x1, z1, .. } => ((x0 + x1) / 2.0, (z0 + z1) / 2.0),
            };
            v.push((cx + r.range(-2.5, 2.5), cz + r.range(-2.5, 2.5)));
        }
        v
    }

    #[test]
    fn grid_push_out_is_bit_for_bit_the_full_scan() {
        for (seed, n, interiors) in [(1u64, 300usize, false), (2, 1000, true), (3, 1500, true), (4, 40, false)] {
            let shapes = world(seed, n, interiors);
            let grid = grid_of(&shapes);
            for (x, z) in spots(&shapes, seed + 50, 4000) {
                for radius in [0.0f32, 0.12, 0.45, 0.8, 2.0, 6.0] {
                    let (a, b) = (grid.push_out(x, z, radius), push_out(x, z, radius, &shapes));
                    assert!(a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits(), "n={n} push_out({x}, {z}, {radius}): {a:?} vs {b:?}");
                }
            }
        }
    }

    #[test]
    fn grid_push_out_follows_a_chain_of_overlapping_shapes_out() {
        // A dense thicket where being pushed out of one tree lands you in the next:
        // the case that makes a naive "only the shapes near the start" shortcut wrong.
        let mut r = Rng::new(77);
        let mut shapes: Vec<Shape> = (0..250).map(|_| Shape::Circle { x: r.range(-6.0, 6.0), z: r.range(-6.0, 6.0), r: r.range(0.3, 1.0) }).collect();
        shapes.extend((0..30).map(|_| Shape::capsule(r.range(-6.0, 6.0), r.range(-6.0, 6.0), r.range(-6.0, 6.0), r.range(-6.0, 6.0), 0.3)));
        shapes.push(Shape::rect_centered(0.0, 8.0, 12.0, 1.0));
        let grid = grid_of(&shapes);
        let mut moved = 0;
        for k in 0..20_000 {
            let (x, z) = (r.range(-9.0, 9.0), r.range(-9.0, 12.0));
            let radius = [0.2f32, 0.45, 1.0][k % 3];
            let (a, b) = (grid.push_out(x, z, radius), push_out(x, z, radius, &shapes));
            assert!(a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits(), "({x}, {z}, {radius}): {a:?} vs {b:?}");
            moved += ((a.0 - x).hypot(a.1 - z) > 1.5) as usize;
        }
        assert!(moved > 500, "the thicket really did push things a long way ({moved})");
    }

    #[test]
    fn grid_blocked_and_swept_moves_match_the_full_scan() {
        for (seed, n) in [(11u64, 400usize), (12, 1200)] {
            let shapes = world(seed, n, true);
            let grid = grid_of(&shapes);
            for (x, z) in spots(&shapes, seed, 6000) {
                for radius in [0.0f32, 0.12, 0.6, 1.2, 2.0] {
                    assert_eq!(grid.blocked(x, z, radius), blocked(x, z, radius, &shapes), "blocked({x}, {z}, {radius})");
                }
            }
            let mut r = Rng::new(seed);
            for _ in 0..400 {
                let from = (r.range(-200.0, 200.0), r.range(-200.0, 200.0));
                let to = (from.0 + r.range(-6.0, 6.0), from.1 + r.range(-6.0, 6.0));
                let (a, b) = (grid.move_and_push(from, to, 0.45), move_and_push(from, to, 0.45, &shapes));
                assert!(a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits(), "{from:?}->{to:?}");
            }
        }
    }

    #[test]
    fn grid_sight_lines_and_cover_match_the_full_scan() {
        for (seed, n) in [(21u64, 500usize), (22, 1500)] {
            let shapes = world(seed, n, true);
            let grid = grid_of(&shapes);
            let mut r = Rng::new(seed);
            let mut hits = (0, 0);
            for k in 0..8000 {
                let a = (r.range(-210.0, 210.0), r.range(-210.0, 210.0));
                let len = [2.0f32, 15.0, 40.0, 120.0, 500.0][k % 5];
                let ang = r.range(0.0, std::f32::consts::TAU);
                let b = (a.0 + ang.cos() * len, a.1 + ang.sin() * len);
                let (blocked_new, cover_new) = (grid.segment_blocked(a, b), grid.segment_cover(a, b));
                assert_eq!(blocked_new, segment_blocked(a, b, &shapes), "segment_blocked {a:?}->{b:?}");
                assert_eq!(cover_new, segment_cover(a, b, &shapes), "segment_cover {a:?}->{b:?}");
                hits.0 += blocked_new as usize;
                hits.1 += (cover_new != (0, 0)) as usize;
            }
            assert!(hits.0 > 500 && hits.1 > 500, "the test sees both blocked and open lines: {hits:?}");
            // Inside the rooms a kilometre away, and from a room to the map.
            for _ in 0..500 {
                let a = (1000.0 + r.range(-8.0, 190.0), 1000.0 + r.range(-7.0, 7.0));
                let b = (a.0 + r.range(-15.0, 15.0), a.1 + r.range(-15.0, 15.0));
                assert_eq!(grid.segment_blocked(a, b), segment_blocked(a, b, &shapes));
                assert_eq!(grid.segment_cover(a, b), segment_cover(a, b, &shapes));
                assert_eq!(grid.segment_blocked(a, (0.0, 0.0)), segment_blocked(a, (0.0, 0.0), &shapes));
            }
        }
    }

    #[test]
    fn the_grid_is_correct_after_every_single_addition() {
        // Shapes are added one by one as the world is built, with queries in between.
        let shapes = world(31, 300, true);
        let mut grid = SpatialGrid::default();
        let mut r = Rng::new(31);
        for (i, &s) in shapes.iter().enumerate() {
            grid.push(s);
            assert_eq!(grid.len(), i + 1);
            let (x, z) = (r.range(-200.0, 200.0), r.range(-200.0, 200.0));
            assert_eq!(grid.blocked(x, z, 1.0), blocked(x, z, 1.0, &shapes[..=i]));
            assert_eq!(grid.push_out(x, z, 0.6), push_out(x, z, 0.6, &shapes[..=i]));
            // A query right on the shape just added.
            if let Shape::Circle { x, z, .. } = s {
                assert!(grid.blocked(x, z, 0.1));
            }
        }
        assert_eq!(grid.shapes(), &shapes[..]);
        assert_eq!(&grid[..], &shapes[..], "it reads like the Vec it replaces");
    }

    #[test]
    fn the_grid_copes_with_garbage_like_the_full_scan_does() {
        let mut shapes = world(41, 200, false);
        shapes.push(Shape::Circle { x: f32::NAN, z: 0.0, r: 1.0 });
        shapes.push(Shape::Rect { x0: f32::NEG_INFINITY, z0: 0.0, x1: 3.0, z1: 1.0 });
        shapes.push(Shape::capsule(0.0, 0.0, 1.0, f32::INFINITY, 0.3));
        shapes.push(Shape::Circle { x: 3.0, z: 3.0, r: f32::NAN });
        let grid = grid_of(&shapes);
        let nan = f32::NAN;
        let same = |a: (f32, f32), b: (f32, f32)| a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits();
        for (x, z, r) in [(nan, 0.0, 0.4), (0.0, nan, 0.4), (1.0, 1.0, nan), (f32::INFINITY, 0.0, 0.4), (1e30, -1e30, 0.4), (0.0, 0.0, f32::INFINITY), (0.0, 0.0, -0.5), (3.0, 3.0, 0.4), (-208.0, 50.0, 0.4)] {
            assert!(same(grid.push_out(x, z, r), push_out(x, z, r, &shapes)), "push_out({x}, {z}, {r})");
            assert_eq!(grid.blocked(x, z, r), blocked(x, z, r, &shapes), "blocked({x}, {z}, {r})");
        }
        for (a, b) in [((nan, 0.0), (1.0, 1.0)), ((0.0, 0.0), (f32::INFINITY, 2.0)), ((1e30, 1e30), (-1e30, 5.0)), ((5.0, 5.0), (5.0, 5.0))] {
            assert_eq!(grid.segment_blocked(a, b), segment_blocked(a, b, &shapes), "{a:?}->{b:?}");
            assert_eq!(grid.segment_cover(a, b), segment_cover(a, b, &shapes), "{a:?}->{b:?}");
        }
        // A grid with silly settings still works.
        let odd: SpatialGrid = SpatialGrid::new(f32::NAN, -3.0);
        assert!(odd.is_empty() && !odd.blocked(0.0, 0.0, 1.0) && odd.segment_cover((0.0, 0.0), (5.0, 5.0)) == (0, 0));
        let from_vec = SpatialGrid::from(shapes.clone());
        assert_eq!(from_vec.len(), shapes.len());
    }

    #[test]
    fn an_empty_grid_blocks_nothing_and_moves_nothing() {
        let g = SpatialGrid::default();
        assert_eq!(g.push_out(3.0, 4.0, 0.5), (3.0, 4.0));
        assert!(!g.blocked(3.0, 4.0, 0.5) && !g.segment_blocked((0.0, 0.0), (9.0, 9.0)));
        assert_eq!(g.move_and_push((0.0, 0.0), (5.0, 5.0), 0.4), move_and_push((0.0, 0.0), (5.0, 5.0), 0.4, &[]));
    }
}
