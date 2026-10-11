//! The rounded 1950s sedan.

use super::*;

pub const WHEEL_R: f32 = 0.36;
/// A bare steel rim, after the tyre has rotted away.
pub const RIM_R: f32 = 0.25;
pub const WHEEL_X: f32 = 0.8;
/// Front and rear axle positions.
pub const WHEEL_Z: [f32; 2] = [1.42, -1.38];
pub const BODY_HALF_W: f32 = 0.96;
pub const BELT_Y: f32 = 1.0;
const ARCH_R: f32 = 0.47;
const SILL_Y: f32 = 0.42;

/// Side view of the lower body with both wheel arches cut out.
pub fn sedan_outline() -> Vec<P2> {
    let mut p: Vec<P2> = vec![(-2.38, SILL_Y + 0.04), (-2.3, SILL_Y)];
    // Arches, rear then front, going over each wheel.
    for &wz in WHEEL_Z.iter().rev() {
        let a0 = ((SILL_Y - WHEEL_R) / ARCH_R).asin();
        let steps = 12;
        for k in 0..=steps {
            let t = PI - a0 - (PI - 2.0 * a0) * k as f32 / steps as f32;
            p.push((wz + ARCH_R * t.cos(), WHEEL_R + ARCH_R * t.sin()));
        }
    }
    p.extend_from_slice(&[
        (2.3, SILL_Y),
        (2.42, 0.5),
        (2.47, 0.62),
        (2.47, 0.76),
        (2.42, 0.88),
        (2.3, 0.94),
        (1.9, 0.97),
        (1.3, 0.99),
        (0.85, BELT_Y),
        (-1.3, BELT_Y),
        (-1.7, 0.995),
        (-2.15, 0.975),
        (-2.38, 0.93),
        (-2.46, 0.82),
        (-2.47, 0.62),
    ]);
    p
}

/// Height of the top of the body at `z` (hood and trunk lines), for trim.
pub fn body_top(z: f32) -> f32 {
    let top: Vec<P2> = sedan_outline().into_iter().filter(|q| q.1 > 0.9).collect();
    let mut best = (f32::MAX, BELT_Y);
    for w in top.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
        if z >= lo && z <= hi {
            let t = (z - a.0) / (b.0 - a.0);
            return a.1 + (b.1 - a.1) * t;
        }
        let d = (z - a.0).abs();
        if d < best.0 {
            best = (d, a.1);
        }
    }
    best.1
}

pub fn sedan_body() -> MeshData {
    extrude_profile(&sedan_outline(), BODY_HALF_W, 0.09, 0.0, 1.5)
}

/// Rear fins with a hint of 1950s flash.
pub fn sedan_fins() -> MeshData {
    let fin = extrude_profile(&[(-2.47, 0.9), (-1.55, 0.99), (-2.36, 1.15), (-2.48, 1.1)], 0.04, 0.015, 0.0, 1.5);
    with_mirror(fin.translated([0.89, 0.0, 0.0]))
}

/// Cabin roof panel.
pub fn sedan_roof() -> MeshData {
    extrude_profile(&[(-0.86, 1.455), (0.34, 1.455), (0.3, 1.5), (0.1, 1.535), (-0.5, 1.535), (-0.8, 1.5)], 0.765, 0.05, 0.0, 1.5)
}

/// Half-width of the cabin at height `y` (it leans in towards the roof).
fn cabin_x(y: f32) -> f32 {
    0.87 - (y - BELT_Y) * 0.25
}

/// The pillars holding up the roof: thin A and B pillars and a wide C pillar.
pub fn sedan_pillars() -> MeshData {
    let mut m = MeshData::default();
    let top = 1.47;
    let a = tube(&[[cabin_x(BELT_Y), BELT_Y, 0.86], [cabin_x(top), top, 0.31]], 0.04, 6);
    let b = tube(&[[cabin_x(BELT_Y), BELT_Y, -0.22], [cabin_x(top), top, -0.22]], 0.045, 6);
    // C pillar: a thin slab leaning in with the cabin.
    let lean = (cabin_x(BELT_Y) - cabin_x(top)).atan2(top - BELT_Y);
    let c = extrude_profile(&[(-1.29, BELT_Y), (-0.86, BELT_Y), (-0.6, top), (-0.82, top)], 0.03, 0.012, 0.0, 1.5)
        .translated([0.0, -BELT_Y, 0.0])
        .rotated_z(lean)
        .translated([cabin_x(BELT_Y) - 0.02, BELT_Y, 0.0]);
    for part in [a, b, c] {
        m.append(&with_mirror(part));
    }
    m
}

/// Which pane of glass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Windshield,
    Rear,
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

impl Window {
    pub const ALL: [Window; 6] = [Window::Windshield, Window::Rear, Window::FrontLeft, Window::FrontRight, Window::RearLeft, Window::RearRight];
}

/// The outline of a window opening, in 3D, going round its frame.
pub fn window_frame(w: Window) -> Vec<V3> {
    let (lo, hi) = (BELT_Y + 0.02, 1.445);
    let side = |s: f32, pts: &[P2]| -> Vec<V3> { pts.iter().map(|&(z, y)| [s * (cabin_x(y) - 0.012), y, z]).collect() };
    match w {
        Window::FrontLeft => side(-1.0, &[(0.82, lo), (-0.18, lo), (-0.18, hi), (0.32, hi)]),
        Window::FrontRight => side(1.0, &[(0.82, lo), (0.32, hi), (-0.18, hi), (-0.18, lo)]),
        Window::RearLeft => side(-1.0, &[(-0.26, lo), (-0.84, lo), (-0.62, hi), (-0.26, hi)]),
        Window::RearRight => side(1.0, &[(-0.26, lo), (-0.26, hi), (-0.62, hi), (-0.84, lo)]),
        // Wraparound glass: bowed forwards in the middle.
        Window::Windshield => {
            let mut pts = Vec::new();
            for k in 0..=8 {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(lo) - 0.04), lo, 0.85 + 0.05 * (1.0 - f * f)]);
            }
            for k in (0..=8).rev() {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(hi) - 0.04), hi, 0.32 + 0.03 * (1.0 - f * f)]);
            }
            pts
        }
        Window::Rear => {
            let mut pts = Vec::new();
            for k in (0..=8).rev() {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(lo) - 0.04), lo, -1.27 - 0.05 * (1.0 - f * f)]);
            }
            for k in 0..=8 {
                let f = k as f32 / 8.0 * 2.0 - 1.0;
                pts.push([f * (cabin_x(hi) - 0.04), hi, -0.8 - 0.03 * (1.0 - f * f)]);
            }
            pts
        }
    }
}

/// An intact pane filling a window opening.
pub fn window_glass(w: Window) -> MeshData {
    let frame = window_frame(w);
    match w {
        // Curved panes: strip between the bottom and top rows.
        Window::Windshield | Window::Rear => {
            let half = frame.len() / 2;
            let mut m = MeshData::default();
            for &p in &frame[..half] {
                m.vertex(p, [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
            }
            for k in 0..half {
                m.vertex(frame[frame.len() - 1 - k], [0.0, 1.0, 0.0], [0.0, 0.0], WHITE);
            }
            for k in 0..half as u32 - 1 {
                let h = half as u32;
                m.quad(k, k + 1, h + k + 1, h + k);
            }
            m.recompute_normals();
            m
        }
        _ => pane(&frame),
    }
}

/// Bench seats, dashboard, steering wheel and floor.
pub fn sedan_interior() -> (MeshData, MeshData) {
    let mut vinyl = MeshData::default();
    let bench = [(0.28, 0.58), (0.3, 0.78), (0.22, 0.82), (-0.26, 0.8), (-0.3, 1.24), (-0.36, 1.28), (-0.44, 1.26), (-0.42, 0.66), (-0.34, 0.58)];
    let front = extrude_profile(&bench, 0.74, 0.06, 0.0, 0.8).translated([0.0, 0.0, 0.0]);
    let rear = extrude_profile(&bench, 0.76, 0.06, 0.0, 0.8).scaled([1.0, 0.92, 1.0]).translated([0.0, 0.03, -0.68]);
    vinyl.append(&front);
    vinyl.append(&rear);
    vinyl.append(&meshgen::cuboid([1.7, 0.05, 2.3], 1.0).translated([0.0, 0.5, -0.25]));
    let mut dark = extrude_profile(&[(0.9, 0.7), (0.9, 1.0), (0.8, 1.09), (0.58, 1.07), (0.54, 0.94), (0.62, 0.7)], 0.84, 0.03, 0.0, 0.8);
    // Steering wheel on its column, driver's side (left).
    let hub = [-0.38, 1.1, 0.42];
    let tilt = 1.0;
    dark.append(&tube(&circle_path(hub, 0.19, tilt, 20), 0.017, 6));
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + FRAC_PI_2;
        let rim = add(hub, [0.19 * a.cos(), 0.19 * a.sin() * tilt.cos(), 0.19 * a.sin() * tilt.sin()]);
        dark.append(&tube(&[hub, rim], 0.01, 4));
    }
    dark.append(&tube(&[hub, [-0.38, 0.92, 0.72]], 0.025, 6));
    (vinyl, dark)
}

/// Bright work: bumpers with bullet guards, grille, headlight bezels, side
/// trim, door handles, hood ornament, mirror, wipers and an aerial.
pub fn sedan_chrome() -> MeshData {
    let mut m = MeshData::default();
    for (dir, zb) in [(1.0f32, 2.5f32), (-1.0, -2.5)] {
        let y = 0.5;
        let path = [
            [-0.95, y, zb - dir * 0.22],
            [-0.88, y, zb - dir * 0.07],
            [-0.6, y, zb],
            [0.6, y, zb],
            [0.88, y, zb - dir * 0.07],
            [0.95, y, zb - dir * 0.22],
        ];
        m.append(&tube(&path, 0.075, 8));
        // Bullet guards.
        for x in [-0.42f32, 0.42] {
            let bullet = lathe(&[(0.085, 0.0), (0.08, 0.06), (0.06, 0.12), (0.03, 0.16), (0.0, 0.175)], 10, 1.0, false, false)
                .rotated_x(dir * FRAC_PI_2)
                .translated([x, y + 0.08, zb]);
            m.append(&bullet);
        }
    }
    // Grille: a frame with bars and teeth.
    let (gz, gy0, gy1, gx) = (2.465, 0.6, 0.8, 0.52);
    m.append(&tube(&[[-gx, gy0, gz], [gx, gy0, gz], [gx, gy1, gz - 0.02], [-gx, gy1, gz - 0.02], [-gx, gy0, gz]], 0.025, 6));
    for k in 1..4 {
        let y = gy0 + (gy1 - gy0) * k as f32 / 4.0;
        m.append(&tube(&[[-gx, y, gz - 0.01], [gx, y, gz - 0.01]], 0.012, 5));
    }
    for k in 0..9 {
        let x = -gx + 0.1 + (2.0 * gx - 0.2) * k as f32 / 8.0;
        m.append(&lathe(&[(0.026, 0.0), (0.022, 0.06), (0.0, 0.08)], 6, 1.0, false, false).rotated_x(FRAC_PI_2).translated([x, gy0 + 0.06, gz]));
    }
    // Headlight bezels.
    for x in [-0.7f32, 0.7] {
        let ring = lathe(&[(0.1, 0.0), (0.14, 0.01), (0.15, 0.035), (0.13, 0.055), (0.11, 0.05)], 16, 1.0, false, false);
        m.append(&ring.rotated_x(FRAC_PI_2).translated([x, 0.78, 2.43]));
    }
    // Side trim along the flanks and door handles.
    for s in [-1.0f32, 1.0] {
        m.append(&tube(&[[s * 0.966, 0.86, 2.2], [s * 0.966, 0.86, 0.9], [s * 0.966, 0.8, -0.6], [s * 0.966, 0.8, -2.3]], 0.012, 5));
        for z in [0.02f32, -0.8] {
            m.append(&meshgen::cuboid([0.03, 0.035, 0.15], 1.0).translated([s * 0.975, 0.93, z]));
        }
    }
    // Hood ornament: a little rocket.
    m.append(&lathe(&[(0.03, 0.0), (0.035, 0.08), (0.02, 0.2), (0.0, 0.26)], 8, 1.0, true, false).rotated_x(FRAC_PI_2 * 0.8).translated([0.0, body_top(2.25) - 0.01, 2.18]));
    // Driver's mirror.
    m.append(&tube(&[[-0.93, 1.01, 0.72], [-1.02, 1.09, 0.7]], 0.012, 5));
    m.append(&lathe(&[(0.06, 0.0), (0.065, 0.02), (0.0, 0.035)], 10, 1.0, true, false).rotated_x(-FRAC_PI_2).translated([-1.03, 1.11, 0.69]));
    // Wipers lying on the cowl, and a whip aerial on the front fender.
    m.append(&tube(&[[-0.65, 1.035, 0.88], [-0.08, 1.06, 0.86]], 0.01, 4));
    m.append(&tube(&[[0.1, 1.035, 0.88], [0.62, 1.06, 0.86]], 0.01, 4));
    m.append(&tube(&[[0.85, 0.99, 1.6], [0.83, 1.9, 1.5]], 0.006, 4));
    m
}

/// Dark seams and fittings: door shut lines, hood and trunk lines, the
/// exhaust pipe and the radiator behind the grille.
pub fn sedan_trim() -> MeshData {
    let mut m = MeshData::default();
    let x = BODY_HALF_W + 0.002;
    for s in [-1.0f32, 1.0] {
        for (z, y0) in [(0.86f32, SILL_Y + 0.05), (-0.22, SILL_Y + 0.02), (-0.93, SILL_Y + 0.05)] {
            m.append(&beam([s * x, y0, z], [s * x, BELT_Y - 0.03, z], 0.006, 0.014));
        }
        m.append(&beam([s * x, SILL_Y + 0.05, 0.86], [s * x, SILL_Y + 0.05, -0.93], 0.006, 0.014));
        // Hood and trunk shut lines run along the top.
        for (z0, z1) in [(0.95f32, 2.3f32), (-1.45, -2.3)] {
            let pts: Vec<V3> = (0..=6).map(|k| {
                let z = z0 + (z1 - z0) * k as f32 / 6.0;
                [s * 0.7, body_top(z) + 0.004, z]
            }).collect();
            m.append(&tube(&pts, 0.008, 4));
        }
    }
    m.append(&beam([-0.7, body_top(0.95) + 0.004, 0.95], [0.7, body_top(0.95) + 0.004, 0.95], 0.014, 0.006));
    m.append(&beam([-0.7, body_top(-1.45) + 0.004, -1.45], [0.7, body_top(-1.45) + 0.004, -1.45], 0.014, 0.006));
    m.append(&tube(&[[-0.5, 0.36, -1.9], [-0.5, 0.35, -2.3], [-0.5, 0.36, -2.52]], 0.035, 8));
    m.append(&meshgen::cuboid([1.0, 0.22, 0.04], 1.0).translated([0.0, 0.7, 2.42]));
    m
}

/// Under the car: floor pan, frame rails, axles and differential, drive
/// shaft, leaf springs, engine sump and fuel tank (seen on the wreck that
/// ended up on its roof, and in the gap under the others).
pub fn sedan_underbody() -> MeshData {
    let mut m = meshgen::cuboid([1.74, 0.03, 4.3], 1.0).translated([0.0, SILL_Y + 0.005, -0.05]);
    for x in [-0.5f32, 0.5] {
        m.append(&meshgen::cuboid([0.08, 0.12, 4.3], 1.0).translated([x, SILL_Y - 0.06, -0.05]));
        m.append(&beam([x * 1.2, SILL_Y - 0.13, -2.0], [x * 1.2, SILL_Y - 0.13, -0.75], 0.06, 0.03));
    }
    for &wz in &WHEEL_Z {
        m.append(&tube(&[[-WHEEL_X, WHEEL_R, wz], [WHEEL_X, WHEEL_R, wz]], 0.045, 8));
    }
    let diff = lathe(&[(0.0, -0.12), (0.1, -0.1), (0.13, 0.0), (0.1, 0.1), (0.0, 0.12)], 10, 1.0, false, false).rotated_x(FRAC_PI_2);
    m.append(&diff.translated([0.0, WHEEL_R, WHEEL_Z[1]]));
    m.append(&tube(&[[0.0, WHEEL_R + 0.02, WHEEL_Z[1] + 0.12], [0.0, SILL_Y - 0.04, 0.7]], 0.035, 8));
    m.append(&meshgen::cuboid([0.5, 0.18, 0.7], 1.0).translated([0.0, SILL_Y - 0.08, 1.55]));
    m.append(&meshgen::cuboid([0.9, 0.16, 0.45], 1.0).translated([0.0, SILL_Y - 0.07, -2.0]));
    m
}

/// Headlight lenses and the red tail lamps in the fins.
pub fn sedan_lenses() -> (MeshData, MeshData) {
    let mut head = MeshData::default();
    for x in [-0.7f32, 0.7] {
        head.append(&lathe(&[(0.105, 0.0), (0.09, 0.025), (0.05, 0.042), (0.0, 0.048)], 14, 1.0, false, false).rotated_x(FRAC_PI_2).translated([x, 0.78, 2.44]));
    }
    let mut tail = MeshData::default();
    for x in [-0.89f32, 0.89] {
        tail.append(&lathe(&[(0.05, 0.0), (0.045, 0.06), (0.0, 0.08)], 10, 1.0, false, false).rotated_x(-FRAC_PI_2).translated([x, 1.04, -2.45]));
    }
    (head, tail)
}

/// A front door panel hinged at its front edge (the origin), for wrecks with a
/// door hanging open. Same shape as the hole it leaves.
pub fn sedan_door() -> MeshData {
    extrude_profile(&[(0.0, SILL_Y + 0.05), (-1.08, SILL_Y + 0.02), (-1.08, BELT_Y), (0.0, BELT_Y)], 0.03, 0.015, 0.0, 1.5)
}

/// The dark hole an open door leaves, a flat panel just outside the body.
pub fn door_opening(side: f32) -> MeshData {
    let x = side * (BODY_HALF_W + 0.004);
    let pts = [[x, SILL_Y + 0.06, 0.85], [x, SILL_Y + 0.03, -0.21], [x, BELT_Y - 0.02, -0.21], [x, BELT_Y - 0.02, 0.85]];
    let mut m = pane(&pts);
    if dot(m.normals[0], [side, 0.0, 0.0]) < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    m
}

/// A tyre around the X axis, centred on the origin: rounded sidewalls and a
/// blocky tread. `flat` (0..1) squashes the bottom like a flat.
pub fn tyre(radius: f32, width: f32, flat: f32) -> MeshData {
    let hw = width * 0.5;
    // Cross-section (radius, x) from the inner bead round the tread to the outer.
    let k = radius / WHEEL_R;
    let section: [(f32, f32); 9] = [
        (0.24 * k, -hw * 0.9),
        (0.31 * k, -hw),
        (0.345 * k, -hw * 0.92),
        (radius, -hw * 0.62),
        (radius, 0.0),
        (radius, hw * 0.62),
        (0.345 * k, hw * 0.92),
        (0.31 * k, hw),
        (0.24 * k, hw * 0.9),
    ];
    let segs = 48;
    let mut m = MeshData::default();
    let floor = -radius * (1.0 - 0.28 * flat);
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * TAU;
        let block = (i / 2) % 2 == 0;
        for (r_i, &(r, x)) in section.iter().enumerate() {
            let tread = (3..=5).contains(&r_i);
            let rr = if tread && block { r + 0.012 * k } else { r };
            let mut p = [x, rr * a.cos(), rr * a.sin()];
            if p[1] < floor {
                let squash = floor - p[1];
                p[1] = floor;
                p[0] *= 1.0 + squash * 2.5;
            }
            m.vertex(p, [1.0, 0.0, 0.0], [r_i as f32 / 8.0, i as f32 / segs as f32 * 6.0], WHITE);
        }
    }
    let rows = section.len() as u32;
    for i in 0..segs as u32 {
        for r in 0..rows - 1 {
            let a = i * rows + r;
            let b = a + rows;
            m.quad(a, a + 1, b + 1, b);
        }
    }
    m.recompute_normals();
    // Make sure the tread faces out.
    if m.outwardness([0.0; 3]) < 0.0 {
        for t in m.indices.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        m.recompute_normals();
    }
    m
}

/// A pressed-steel wheel rim around the X axis, outer face towards +X.
pub fn rim(radius: f32, width: f32) -> MeshData {
    let hw = width * 0.5;
    let mut barrel = lathe(&[(radius, -hw), (radius * 0.92, -hw * 0.8), (radius * 0.92, hw * 0.8), (radius, hw)], 20, 1.0, false, false);
    // The lathe faces outwards; the barrel is seen from outside too.
    barrel = barrel.rotated_z(-FRAC_PI_2);
    let disc = lathe(&[(radius * 0.92, hw * 0.3), (radius * 0.6, hw * 0.45), (radius * 0.35, hw * 0.75), (radius * 0.2, hw * 0.75), (0.0, hw * 0.8)], 20, 1.0, false, false).rotated_z(-FRAC_PI_2);
    let mut m = barrel;
    m.append(&disc);
    // Lug nuts.
    for k in 0..5 {
        let a = k as f32 / 5.0 * TAU;
        let nut = lathe(&[(0.016, 0.0), (0.016, 0.02), (0.0, 0.025)], 6, 1.0, false, false).rotated_z(-FRAC_PI_2);
        m.append(&nut.translated([hw * 0.75, radius * 0.27 * a.cos(), radius * 0.27 * a.sin()]));
    }
    m
}

/// A domed chrome hubcap, outer face towards +X.
pub fn hubcap(radius: f32) -> MeshData {
    lathe(&[(radius, 0.0), (radius * 0.97, 0.015), (radius * 0.75, 0.035), (radius * 0.3, 0.06), (radius * 0.1, 0.065), (0.0, 0.066)], 20, 1.0, false, false).rotated_z(-FRAC_PI_2)
}

/// Where a sedan's wheels touch the ground (local x, z).
pub fn sedan_wheels() -> [(f32, f32); 4] {
    [(-WHEEL_X, WHEEL_Z[0]), (WHEEL_X, WHEEL_Z[0]), (-WHEEL_X, WHEEL_Z[1]), (WHEEL_X, WHEEL_Z[1])]
}
