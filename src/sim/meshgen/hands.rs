//! The first-person hands: the leather glove and the sleeve.

use super::*;

/// Leather glove colour.
const GLOVE: [f32; 4] = [0.07, 0.045, 0.028, 1.0];
const GLOVE_DARK: [f32; 4] = [0.035, 0.024, 0.016, 1.0];

/// A gloved right hand (mirror it on X for the left), gun-model sized. The
/// palm is centred on the origin, the back of the hand faces +Y, fingers
/// point along -Z and curl down round an axis along X as `grip` goes from 0
/// (open) to 1 (a fist round a ~3 cm grip centred near (0, -0.024, -0.042)).
/// The thumb is on the -X side and the cuff runs back along +Z.
pub fn glove(grip: f32) -> MeshData {
    let g = grip.clamp(0.0, 1.0);
    let mut m = blob(0.05, 0.4, 0.06, 811, 0.1).scaled([0.95, 1.0, 1.0]).tinted(GLOVE);
    let centre = [0.0, -0.024, -0.042];
    let r = 0.025;
    for (k, x) in [-0.03f32, -0.01, 0.01, 0.028].into_iter().enumerate() {
        let len = [0.085, 0.092, 0.086, 0.07][k];
        let knuckle = [x, 0.0, -0.05 + (k as f32 - 1.5).abs() * 0.003];
        let phi0 = (knuckle[1] - centre[1]).atan2(-(knuckle[2] - centre[2]));
        let pts: Vec<V3> = (0..=6)
            .map(|i| {
                let s = len * i as f32 / 6.0;
                let straight = [knuckle[0], knuckle[1] - s * 0.15, knuckle[2] - s];
                let phi = phi0 - s / r;
                let curled = [x, centre[1] + r * phi.sin(), centre[2] - r * phi.cos()];
                lerp3(straight, curled, g)
            })
            .collect();
        let mut finger = tube(&pts, 0.0092, 7).tinted(GLOVE);
        // Round the fingertip off.
        finger.append(&blob(0.0092, 1.0, 0.0, 3, 0.1).translated(*pts.last().unwrap()).tinted(GLOVE));
        m.append(&finger);
    }
    // Thumb: from the heel of the palm, curling across the fingers.
    let thumb = [
        [-0.04, -0.01, 0.015],
        [-0.055, -0.018, -0.02],
        lerp3([-0.06, -0.02, -0.055], [-0.03, -0.042, -0.052], g),
        lerp3([-0.062, -0.02, -0.08], [-0.008, -0.05, -0.058], g),
    ];
    m.append(&tube(&thumb, 0.0105, 7).tinted(GLOVE));
    m.append(&blob(0.0105, 1.0, 0.0, 5, 0.1).translated(thumb[3]).tinted(GLOVE));
    // Knuckle seam and a flared cuff.
    m.append(&tube(&[[-0.04, 0.012, -0.044], [0.04, 0.012, -0.044]], 0.006, 5).tinted(GLOVE_DARK));
    m.append(&lathe(&[(0.034, 0.0), (0.038, 0.05), (0.041, 0.075)], 12, 0.1, false, false).rotated_x(PI * 0.5).translated([0.0, 0.0, 0.035]).scaled([1.0, 0.75, 1.0]).tinted(GLOVE_DARK));
    m
}

/// A Vault 143 jumpsuit sleeve over the forearm: wrist at the origin, the
/// elbow at z = 1 (scaled to the arm's length when drawn), a gold cuff band.
pub fn sleeve() -> MeshData {
    // Rings either side of each band edge keep the colours crisp.
    let rings = [(0.036, 0.0), (0.037, 0.045), (0.037, 0.05), (0.038, 0.13), (0.038, 0.135), (0.04, 0.25), (0.048, 0.6), (0.055, 1.0)];
    let mut m = lathe(&rings, 12, 0.2, false, false).rotated_x(PI * 0.5);
    for i in 0..m.positions.len() {
        let z = m.positions[i][2];
        m.colors[i] = if (0.048..0.132).contains(&z) { [0.6, 0.42, 0.04, 1.0] } else { [0.03, 0.06, 0.2, 1.0] };
    }
    m
}
