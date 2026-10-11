use super::*;

fn closed_and_outward(m: &MeshData, centre: V3) {
    assert!(m.is_valid());
    assert!(m.outwardness(centre) > 0.0, "triangles face inwards");
}

#[test]
fn cuboid_faces_out_and_uvs_scale_with_size() {
    let m = cuboid([4.0, 2.0, 1.0], 1.0);
    assert_eq!(m.triangle_count(), 12);
    closed_and_outward(&m, [0.0; 3]);
    // Every triangle must face out, not just on average.
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let mid = scale(add(add(a, b), c), 1.0 / 3.0);
        assert!(dot(n, mid) > 0.0);
        assert!(dot(normalize(n), m.normals[t[0] as usize]) > 0.99, "stored normal matches winding");
    }
    let (lo, hi) = m.bounds();
    assert_eq!(lo, [-2.0, -1.0, -0.5]);
    assert_eq!(hi, [2.0, 1.0, 0.5]);
    let umax = m.uvs.iter().map(|uv| uv[0].abs()).fold(0.0, f32::max);
    assert!((umax - 2.0).abs() < 1e-5, "a 4 m face spans 4 texture tiles");
}

#[test]
fn lathe_barrel_is_closed_and_outward() {
    let m = lathe(&[(0.4, 0.0), (0.45, 0.5), (0.4, 1.0)], 12, 1.0, true, true);
    closed_and_outward(&m, [0.0, 0.5, 0.0]);
    let (lo, hi) = m.bounds();
    assert!((hi[1] - 1.0).abs() < 1e-5 && lo[1].abs() < 1e-5);
    assert!(hi[0] <= 0.45 + 1e-5);
    // Analytic normals point away from the axis on the bulge.
    let i = 12 + 1; // second ring, first vertex at +X
    assert!(m.normals[i][0] > 0.9);
}

#[test]
fn loft_faces_out_with_caps() {
    let m = loft_z(&[sec(-1.0, 0.0, 0.5, 0.5, 2.0), sec(1.0, 0.0, 0.5, 0.5, 2.0)], 12, 1.0);
    closed_and_outward(&m, [0.0; 3]);
    // Side normals are horizontal-ish and outward.
    assert!(m.normals[0][0] > 0.6);
}

#[test]
fn blob_is_outward_and_bumpy() {
    let m = blob(2.0, 0.6, 0.15, 7, 1.0);
    closed_and_outward(&m, [0.0; 3]);
    let (lo, hi) = m.bounds();
    assert!(hi[1] < 2.0 * 0.6 * 1.2 && lo[1] > -2.0 * 0.6 * 1.2);
    let radii: Vec<f32> = m.positions.iter().map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt()).collect();
    let max = radii.iter().cloned().fold(0.0, f32::max);
    assert!(max > 2.0 * 1.02, "noise pushes some points out");
}

#[test]
fn flat_shapes_face_up() {
    let d = disc(3.0, 24, 2.0);
    assert!(d.is_valid());
    assert!(d.outwardness([0.0, -1.0, 0.0]) > 0.0);
    let g = ground_patch(0.0, 0.0, 5.0, 8, 0.05, None, &|x, z| (x * 0.3).sin() + z * 0.1);
    assert!(g.is_valid());
    assert!(g.normals.iter().all(|n| n[1] > 0.5));
    let road = ground_strip(&[(0.0, 0.0), (10.0, 0.0), (20.0, 5.0)], 6.0, 0.05, 6.0, &|_, _| 0.0);
    assert!(road.is_valid());
    assert!(road.normals.iter().all(|n| n[1] > 0.99));
    let reversed = ground_strip(&[(20.0, 5.0), (10.0, 0.0), (0.0, 0.0)], 6.0, 0.05, 6.0, &|_, _| 0.0);
    assert!(reversed.normals.iter().all(|n| n[1] > 0.99));
}

#[test]
fn drifts_rise_from_the_ground_with_a_steep_lee_face() {
    let flat = |_: f32, _: f32| 1.0;
    let m = drift_patch(0.0, 0.0, 6.0, 3.0, 0.8, [1.0, 0.0], 3, 4.0, &flat);
    assert!(m.is_valid());
    assert!(m.normals.iter().all(|n| n[1] > 0.0), "faces up");
    let (lo, hi) = m.bounds();
    assert!(hi[1] > 1.6 && hi[1] < 1.85, "peaks about 0.8 above the ground: {}", hi[1]);
    assert!(lo[1] < 1.0, "rim tucks under the ground");
    let at = |x: f32| m.positions.iter().filter(|p| p[2].abs() < 0.2).min_by(|a, b| (a[0] - x).abs().total_cmp(&(b[0] - x).abs())).unwrap()[1];
    // Downwind (+x) it falls away faster than upwind.
    assert!(at(1.5) < at(-1.5), "lee {} vs windward {}", at(1.5), at(-1.5));
}

#[test]
fn ground_patch_hugs_the_height_field() {
    let h = |x: f32, z: f32| x * 0.5 + z;
    let g = ground_patch(10.0, -4.0, 3.0, 4, 0.1, Some(2.0), &h);
    for p in &g.positions {
        assert!((p[1] - (h(p[0], p[2]) + 0.1)).abs() < 1e-4);
    }
}

#[test]
fn cables_sag_in_the_middle() {
    let pts = catenary([0.0, 8.0, 0.0], [30.0, 8.0, 0.0], 1.5, 10);
    assert_eq!(pts.len(), 11);
    assert_eq!(pts[0], [0.0, 8.0, 0.0]);
    assert!((pts[5][1] - 6.5).abs() < 1e-4);
    let wire = tube(&pts, 0.02, 4);
    assert!(wire.is_valid());
    assert_eq!(wire.triangle_count(), 10 * 4 * 2);
}

#[test]
fn gear_has_teeth_and_faces_out() {
    let m = gear(4.0, 0.5, 10, 0.9, 2.0);
    closed_and_outward(&m, [0.0; 3]);
    let (lo, hi) = m.bounds();
    assert!(hi[0] > 4.3 && lo[0] < -4.0);
    assert!((hi[2] - 0.45).abs() < 1e-5);
}

#[test]
fn shed_walls_have_gables_and_no_floor() {
    let m = gable_walls(3.0, 2.4, 3.0, 1.0, 1.0);
    assert!(m.is_valid());
    let (_, hi) = m.bounds();
    assert!((hi[1] - 3.4).abs() < 1e-5);
    assert!(m.outwardness([0.0, 1.5, 0.0]) > 0.0);
    let floor = m.indices.chunks_exact(3).any(|t| t.iter().all(|&i| m.positions[i as usize][1] == 0.0 && m.normals[i as usize][1] < -0.5));
    assert!(!floor);
}

#[test]
fn tent_has_a_roof_that_faces_up_and_out() {
    let m = tent(2.4, 1.1, 3.0, 0.9, 0.15, 1.5);
    assert!(m.is_valid());
    let (lo, hi) = m.bounds();
    assert!((hi[1] - 2.0).abs() < 1e-5, "ridge at wall height + gable");
    assert!(hi[0] > 1.2 && lo[0] < -1.2, "eaves overhang the walls");
    assert!(hi[2] > 1.5, "roof runs past the gable ends");
    // Every roof triangle (above the walls, sloping) winds the way its normal points, upwards.
    let roof: Vec<_> = m.indices.chunks_exact(3).filter(|t| t.iter().all(|&i| m.normals[i as usize][1] > 0.3 && m.normals[i as usize][1] < 0.99)).collect();
    assert_eq!(roof.len(), 4);
    for t in roof {
        let p = |k: usize| m.positions[t[k] as usize];
        let f = cross(sub(p(1), p(0)), sub(p(2), p(0)));
        assert!(f[1] > 0.0, "roof faces up");
    }
}

#[test]
fn wolf_parts_line_up() {
    let body = wolf_body_variant(0);
    let head = wolf_head_variant(0);
    let leg = wolf_leg();
    let tail = wolf_tail();
    for m in [&body, &head, &leg, &tail] {
        assert!(m.is_valid());
    }
    closed_and_outward(&body, [0.0, 0.85, 0.0]);
    closed_and_outward(&leg, [0.0, -0.3, 0.0]);
    // Head starts where the neck ends, and legs reach the ground from the
    // 0.6 m hip pivot used by the animation.
    let (_, body_hi) = body.bounds();
    let (head_lo, head_hi) = head.bounds();
    assert!(head_lo[2] < body_hi[2]);
    assert!(head_hi[2] > 1.25);
    let (leg_lo, _) = leg.bounds();
    assert!((leg_lo[1] + 0.62).abs() < 1e-4);
    // Pale belly, darker back.
    let top = body.colors[body.normals.iter().position(|n| n[1] > 0.95).unwrap()];
    let bottom = body.colors[body.normals.iter().position(|n| n[1] < -0.9).unwrap()];
    assert!(top[0] < bottom[0]);
}

#[test]
fn gloves_open_and_close_round_the_grip() {
    let open = glove(0.0);
    let fist = glove(1.0);
    assert!(open.is_valid() && fist.is_valid());
    assert_eq!(open.positions.len(), fist.positions.len());
    // Open fingers reach well forward; a fist's fingertips wrap back under the palm.
    let reach = |m: &MeshData| m.positions.iter().map(|p| -p[2]).fold(f32::MIN, f32::max);
    assert!(reach(&open) > 0.13 && reach(&fist) < 0.085);
    // Nothing of the fist sits inside the grip itself.
    let inside = fist.positions.iter().filter(|p| ((p[1] + 0.024).powi(2) + (p[2] + 0.042).powi(2)).sqrt() < 0.012 && p[0].abs() < 0.035).count();
    assert_eq!(inside, 0);
    let s = sleeve();
    assert!(s.is_valid());
    let (lo, hi) = s.bounds();
    assert!(lo[2].abs() < 1e-4 && (hi[2] - 1.0).abs() < 1e-4);
}

#[test]
fn transforms_keep_normals_unit() {
    let m = cuboid([1.0, 1.0, 1.0], 1.0)
        .scaled([2.0, 0.5, 1.0])
        .rotated_y(0.7)
        .rotated_x(0.3)
        .rotated_z(-0.2)
        .translated([5.0, 0.0, 0.0]);
    assert!(m.is_valid());
    assert!(m.outwardness([5.0, 0.0, 0.0]) > 0.0);
    let mut both = m.clone();
    both.append(&disc(1.0, 8, 1.0));
    assert!(both.is_valid());
    assert_eq!(both.triangle_count(), 12 + 8);
}

#[test]
fn dead_branches_are_valid() {
    let m = dead_branches(6.0, 9);
    assert!(m.is_valid());
    let (lo, hi) = m.bounds();
    assert!(lo[1] > 1.0 && hi[1] < 8.0);
}

#[test]
fn wolf_variants_look_different_but_share_a_shape() {
    let base = wolf_body_variant(0);
    for v in 0..WOLF_VARIANTS {
        let b = wolf_body_variant(v);
        let h = wolf_head_variant(v);
        assert!(b.is_valid() && h.is_valid(), "variant {v}");
        assert_eq!(b.positions, base.positions, "same body shape");
        if v > 0 {
            assert_ne!(b.colors, base.colors, "variant {v} has its own markings");
        }
    }
    // Ears differ: variant 2 stands taller than variant 1's torn ear.
    let top = |m: &MeshData| m.bounds().1[1];
    assert!(top(&wolf_head_variant(2)) > top(&wolf_head_variant(1)) + 0.02);
    // Variant numbers wrap rather than panic.
    assert_eq!(wolf_head_variant(WOLF_VARIANTS).colors, wolf_head_variant(0).colors);
}

#[test]
fn wolf_patterns_keep_colours_in_range() {
    for v in 0..WOLF_VARIANTS {
        for m in [wolf_body_variant(v), wolf_head_variant(v)] {
            assert!(m.colors.iter().all(|c| c.iter().all(|x| (0.0..=1.001).contains(x))), "variant {v}");
        }
    }
}

#[test]
fn the_moose_is_big_and_hangs_together() {
    let body = moose_body();
    let head = moose_head();
    let leg = moose_leg();
    for m in [&body, &head, &leg] {
        assert!(m.is_valid());
    }
    closed_and_outward(&body, [0.0, 1.55, 0.0]);
    let (blo, bhi) = body.bounds();
    assert!(bhi[2] - blo[2] > 2.3, "over 2 m long");
    assert!(bhi[1] > 2.0 && blo[1] > 0.8, "belly clears the snow, back is well above it");
    // The head starts inside the body's front and reaches well past it.
    let (hlo, hhi) = head.bounds();
    assert!(hlo[2] < bhi[2] && hhi[2] > bhi[2] + 1.2);
    // Legs reach from the body down to the snow: hip pivot at ~1.1.
    let (llo, lhi) = leg.bounds();
    assert!((llo[1] + 1.1).abs() < 1e-4 && lhi[1] > 0.0);
}

#[test]
fn antlers_are_wide_mirror_images_with_tips() {
    let (l, lt) = moose_antler(-1.0);
    let (r, rt) = moose_antler(1.0);
    assert!(l.is_valid() && r.is_valid());
    assert_eq!(lt.len(), 6);
    assert_eq!(lt.len(), rt.len());
    for (a, b) in lt.iter().zip(&rt) {
        assert!((a[0] + b[0]).abs() < 1e-5 && a[1] == b[1] && a[2] == b[2], "mirror images");
    }
    let (_, rhi) = r.bounds();
    assert!(rhi[0] > 1.2, "palms spread wide: {}", rhi[0]);
    // Tips stand above the head and nowhere near the ground.
    assert!(rt.iter().all(|t| t[1] > 2.1));
}

#[test]
fn tent_without_overhang_stops_at_the_walls_and_still_faces_up() {
    let m = tent(2.0, 1.0, 3.0, 0.8, 0.0, 1.0);
    assert!(m.is_valid());
    let (lo, hi) = m.bounds();
    assert!((hi[0] - 1.0).abs() < 1e-5 && (lo[0] + 1.0).abs() < 1e-5, "no eaves");
    assert!((hi[2] - 1.5).abs() < 1e-5 && (lo[2] + 1.5).abs() < 1e-5, "no front/back overhang");
    assert!((hi[1] - 1.8).abs() < 1e-5);
    assert!(lo[1] >= -1e-5, "base stays at the origin");
    // More canvas never makes the tent smaller than the bare walls.
    let (_, big) = tent(2.0, 1.0, 3.0, 0.8, 0.4, 1.0).bounds();
    assert!(big[0] > hi[0] && big[2] > hi[2]);
    // A tent with no gable rise is a flat-topped box and must still be a valid mesh.
    assert!(tent(2.0, 1.0, 3.0, 0.0, 0.1, 1.0).is_valid());
}
