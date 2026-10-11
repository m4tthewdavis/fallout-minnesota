//! Setting a vehicle down on uneven ground so every wheel or ski touches the snow.

/// Where a vehicle's root goes so it rests on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rest {
    pub y: f32,
    /// Nose-down rotation about the local X axis (radians; apply after yaw).
    pub pitch: f32,
    /// Rotation about the local Z axis (radians; positive lifts the +X side).
    pub roll: f32,
}

/// Fit a vehicle with yaw `yaw` at (`x`, `z`) to the ground. `contacts` are
/// (local x, local z, lift): where the wheels, skis or track touch, and how far
/// above the vehicle's base each contact sits (a flat tyre's contact is higher
/// than a full one). The vehicle tilts with the slope, and is then lowered so
/// no contact floats, plus `sink` to bury it a little in the snow.
pub fn settle(x: f32, z: f32, yaw: f32, contacts: &[(f32, f32, f32)], sink: f32, height: &dyn Fn(f32, f32) -> f32) -> Rest {
    let (s, c) = yaw.sin_cos();
    let samples: Vec<(f32, f32, f32)> = contacts
        .iter()
        .map(|&(lx, lz, lift)| {
            // Bevy's rotation_y(yaw): local +X -> (cos, -sin), local +Z -> (sin, cos).
            let (wx, wz) = (x + lx * c + lz * s, z - lx * s + lz * c);
            (lx, lz, height(wx, wz) - lift)
        })
        .collect();
    // Least squares plane h = a + b*lx + d*lz.
    let k = samples.len() as f32;
    let (mut sx, mut sz, mut sxx, mut szz, mut sxz, mut sh, mut sxh, mut szh) = (0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for &(lx, lz, h) in &samples {
        sx += lx;
        sz += lz;
        sxx += lx * lx;
        szz += lz * lz;
        sxz += lx * lz;
        sh += h;
        sxh += lx * h;
        szh += lz * h;
    }
    let mat = [[k, sx, sz], [sx, sxx, sxz], [sz, sxz, szz]];
    let rhs = [sh, sxh, szh];
    let (a, b, d) = match solve3(mat, rhs) {
        Some(v) => (v[0], v[1], v[2]),
        None => (sh / k.max(1.0), 0.0, 0.0),
    };
    // Lower the plane until nothing floats.
    let float = samples.iter().map(|&(lx, lz, h)| (a + b * lx + d * lz) - h).fold(0.0f32, f32::max);
    Rest { y: a - float - sink, pitch: -d.atan(), roll: b.atan() }
}

fn solve3(m: [[f32; 3]; 3], r: [f32; 3]) -> Option<[f32; 3]> {
    let det = |m: [[f32; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(m);
    if d.abs() < 1e-6 {
        return None;
    }
    let mut out = [0.0; 3];
    for (col, o) in out.iter_mut().enumerate() {
        let mut mm = m;
        for row in 0..3 {
            mm[row][col] = r[row];
        }
        *o = det(mm) / d;
    }
    Some(out)
}
