use super::*;

fn closed_and_outward(m: &MeshData, centre: V3) {
    assert!(m.is_valid());
    assert!(m.outwardness(centre) > 0.0, "triangles face inwards");
}

#[test]
fn ear_clipping_covers_a_concave_outline() {
    // An L shape: area 3.
    let l = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
    for poly in [l.to_vec(), l.iter().rev().cloned().collect()] {
        let tris = triangulate(&poly);
        assert_eq!(tris.len(), 4);
        let area: f32 = tris.iter().map(|t| cross2(poly[t[0]], poly[t[1]], poly[t[2]]) * 0.5).sum();
        assert!((area - 3.0).abs() < 1e-5, "area {area}");
        assert!(tris.iter().all(|t| cross2(poly[t[0]], poly[t[1]], poly[t[2]]) > 0.0));
    }
}

#[test]
fn extruded_slabs_are_closed_and_face_out() {
    let m = extrude_profile(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)], 0.5, 0.1, 0.0, 1.0);
    closed_and_outward(&m, [0.0, 0.5, 1.0]);
    let (lo, hi) = m.bounds();
    assert!((hi[0] - 0.5).abs() < 1e-5 && (lo[0] + 0.5).abs() < 1e-5);
    assert!((hi[1] - 1.0).abs() < 1e-5 && lo[1].abs() < 1e-5);
    // Every triangle faces away from the middle, not just on average.
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let mid = scale(add(add(a, b), c), 1.0 / 3.0);
        assert!(dot(n, sub(mid, [0.0, 0.5, 1.0])) > -1e-6);
    }
}

#[test]
fn the_sedan_body_has_wheel_arches_over_its_wheels() {
    let body = sedan_body();
    closed_and_outward(&body, [0.0, 0.75, 0.0]);
    let (lo, hi) = body.bounds();
    assert!((hi[0] - BODY_HALF_W).abs() < 1e-4);
    assert!(hi[2] - lo[2] > 4.8, "a full-size sedan");
    // No body directly above a wheel's top below the arch.
    for &wz in &WHEEL_Z {
        let over_wheel = sedan_outline().iter().filter(|q| (q.0 - wz).abs() < 0.1).map(|q| q.1).fold(f32::MAX, f32::min);
        assert!(over_wheel > WHEEL_R * 2.0 + 0.05, "arch clears the tyre: {over_wheel}");
    }
    assert!((body_top(1.3) - 0.99).abs() < 0.02);
}

#[test]
fn sedan_parts_are_valid_meshes() {
    let (vinyl, dark) = sedan_interior();
    let (head, tail) = sedan_lenses();
    let mut parts = vec![sedan_underbody(), sedan_fins(), sedan_roof(), sedan_pillars(), sedan_chrome(), sedan_trim(), vinyl, dark, head, tail, sedan_door(), door_opening(1.0)];
    for w in Window::ALL {
        parts.push(window_glass(w));
        parts.push(shards(&window_frame(w), 4));
    }
    for (i, p) in parts.iter().enumerate() {
        assert!(p.is_valid(), "part {i}");
    }
    // The roof sits on top of the pillars and the glass reaches it.
    let (_, roof_hi) = sedan_roof().bounds();
    assert!(roof_hi[1] > 1.5 && roof_hi[1] < 1.6);
    let (_, glass_hi) = window_glass(Window::Windshield).bounds();
    assert!(glass_hi[1] > 1.4);
    // An open door's hole faces outwards on its side.
    assert!(door_opening(-1.0).normals.iter().all(|n| n[0] < -0.9));
}

#[test]
fn tyres_are_round_and_a_flat_one_sags() {
    let t = tyre(WHEEL_R, 0.22, 0.0);
    closed_and_outward(&t, [0.0; 3]);
    let (lo, hi) = t.bounds();
    assert!(lo[1] < -WHEEL_R + 1e-3 && hi[1] > WHEEL_R - 1e-3);
    assert!(hi[0] <= 0.12);
    let flat = tyre(WHEEL_R, 0.22, 1.0);
    let (flo, fhi) = flat.bounds();
    assert!(flo[1] > lo[1] + 0.08, "flat bottom sits higher");
    assert!(fhi[0] > hi[0], "and bulges out");
    assert!(rim(RIM_R, 0.18).is_valid() && hubcap(0.2).is_valid());
}

#[test]
fn settling_on_flat_ground_leaves_the_vehicle_level() {
    let contacts: Vec<_> = sedan_wheels().iter().map(|&(x, z)| (x, z, 0.0)).collect();
    let r = settle(10.0, 5.0, 0.7, &contacts, 0.0, &|_, _| 2.0);
    assert!((r.y - 2.0).abs() < 1e-4 && r.pitch.abs() < 1e-5 && r.roll.abs() < 1e-5);
}

#[test]
fn settling_follows_a_slope_and_no_wheel_floats() {
    // Ground rising towards +X in the world; vehicle faces +X (yaw 90 degrees),
    // so its nose should point up.
    let slope = |x: f32, _z: f32| 0.2 * x;
    let contacts: Vec<_> = sedan_wheels().iter().map(|&(x, z)| (x, z, 0.0)).collect();
    let yaw = FRAC_PI_2;
    let r = settle(0.0, 0.0, yaw, &contacts, 0.0, &slope);
    assert!(r.pitch < -0.15 && r.pitch > -0.25, "nose up: {}", r.pitch);
    assert!(r.roll.abs() < 1e-4);
    // Bumpy ground: every wheel ends up touching or slightly buried.
    let bumpy = |x: f32, z: f32| (x * 0.9).sin() * 0.3 + (z * 1.3).cos() * 0.2;
    for yaw in [0.0f32, 0.8, 2.5] {
        let r = settle(3.0, -2.0, yaw, &contacts, 0.05, &bumpy);
        let rot_y = |lx: f32, lz: f32| -> f32 {
            // Height of a contact point after pitch and roll (small angles).
            r.y + lx * r.roll.tan() - lz * r.pitch.tan()
        };
        let (s, c) = yaw.sin_cos();
        for &(lx, lz, _) in &contacts {
            let (wx, wz) = (3.0 + lx * c + lz * s, -2.0 - lx * s + lz * c);
            assert!(rot_y(lx, lz) <= bumpy(wx, wz) + 1e-3, "wheel floats at yaw {yaw}");
        }
    }
    // A flat tyre's corner drops.
    let flat: Vec<_> = sedan_wheels().iter().enumerate().map(|(i, &(x, z))| (x, z, if i == 0 { 0.1 } else { 0.0 })).collect();
    let r = settle(0.0, 0.0, 0.0, &flat, 0.0, &|_, _| 0.0);
    assert!(r.roll > 0.0, "left-front flat tips the car to the left: {}", r.roll);
}

#[test]
fn snowmobile_parts_hang_together() {
    let body = snowmobile_body();
    let black = snowmobile_black();
    let steel = snowmobile_steel();
    let track = snowmobile_track();
    let (head, tail) = snowmobile_lenses();
    for p in [&body, &black, &steel, &track, &snowmobile_windshield(), &head, &tail] {
        assert!(p.is_valid());
    }
    // Skis and track touch y = 0; nothing pokes below it.
    let (slo, shi) = steel.bounds();
    assert!(slo[1].abs() < 0.01, "skis on the snow: {}", slo[1]);
    assert!(shi[0] > SKI_X && slo[0] < -SKI_X);
    let (tlo, thi) = track.bounds();
    assert!(tlo[1] > -0.06 && tlo[1] < 0.0, "lugs bite into the snow: {}", tlo[1]);
    assert!(thi[1] < 0.64, "track fits under the tunnel deck");
    // Seat is above the track and the body joins the tunnel.
    let (blo, _) = black.bounds();
    assert!(blo[1] < 0.4);
    let (_, bhi) = body.bounds();
    assert!(bhi[1] > 0.9 && bhi[1] < 1.0);
}
