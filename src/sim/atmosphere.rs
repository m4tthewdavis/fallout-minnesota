//! A single-scattering model of the sky (Rayleigh and Mie scattering through
//! a thin spherical atmosphere), used to paint the sky dome, colour the fog
//! and tint the sunlight. It's what makes the horizon pale, the zenith deep
//! blue, a low winter sun orange and the sky behind it a soft pink, and it
//! passes smoothly through dusk into night. Pure maths (no Bevy), so the
//! colours are unit-tested.

const EARTH_R: f32 = 6_360_000.0;
const ATMO_R: f32 = 6_420_000.0;
/// Where the observer stands above the ground.
const EYE_H: f32 = 10.0;
/// Rayleigh scattering per metre (red, green, blue): why the sky is blue.
const BETA_R: [f32; 3] = [7.0e-6, 16.5e-6, 41.0e-6];
/// Mie scattering per metre (haze, ice crystals): a pale halo round the sun.
const BETA_M: f32 = 21.0e-6;
const SCALE_R: f32 = 8_000.0;
const SCALE_M: f32 = 1_200.0;
const MIE_G: f32 = 0.76;
const SUN_POWER: f32 = 22.0;
const PRIMARY_STEPS: usize = 12;
const LIGHT_STEPS: usize = 5;

/// What the sky looks like right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    /// Unit vector from the ground to the sun.
    pub sun: [f32; 3],
    /// 1 = clear winter air, more = haze and drifting ice crystals.
    pub haze: f32,
    /// 0..=1 moonlight (the sky's night glow).
    pub moon: f32,
}

type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

/// Distance along a ray from `origin` to where it leaves the sphere of radius
/// `r` centred on the planet's centre (the ray starts inside it).
fn exit_distance(origin: V3, dir: V3, r: f32) -> f32 {
    let b = dot(origin, dir);
    let c = dot(origin, origin) - r * r;
    -b + (b * b - c).max(0.0).sqrt()
}

/// Does the ray hit the ground?
fn hits_ground(origin: V3, dir: V3) -> bool {
    let b = dot(origin, dir);
    let c = dot(origin, origin) - EARTH_R * EARTH_R;
    b < 0.0 && b * b - c > 0.0
}

fn rayleigh_phase(mu: f32) -> f32 {
    3.0 / (16.0 * std::f32::consts::PI) * (1.0 + mu * mu)
}

fn mie_phase(mu: f32) -> f32 {
    let g2 = MIE_G * MIE_G;
    3.0 / (8.0 * std::f32::consts::PI) * ((1.0 - g2) * (1.0 + mu * mu)) / ((2.0 + g2) * (1.0 + g2 - 2.0 * MIE_G * mu).powf(1.5))
}

/// Light arriving from the direction `view` (unit; below the horizon is
/// treated as the horizon), as linear RGB before exposure.
pub fn sky_radiance(view: V3, c: &Conditions) -> V3 {
    let mut dir = norm(view);
    dir[1] = dir[1].max(0.0);
    let dir = norm(dir);
    let sun = norm(c.sun);
    let origin = [0.0, EARTH_R + EYE_H, 0.0];
    let len = exit_distance(origin, dir, ATMO_R);
    let step = len / PRIMARY_STEPS as f32;
    let mu = dot(dir, sun);
    let (pr, pm) = (rayleigh_phase(mu), mie_phase(mu));
    let beta_m = BETA_M * c.haze.max(0.0);
    let (mut sum_r, mut sum_m) = ([0.0f32; 3], [0.0f32; 3]);
    let (mut od_r, mut od_m) = (0.0f32, 0.0f32);
    for i in 0..PRIMARY_STEPS {
        let t = (i as f32 + 0.5) * step;
        let p = [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
        let h = (dot(p, p).sqrt() - EARTH_R).max(0.0);
        let (dr, dm) = ((-h / SCALE_R).exp() * step, (-h / SCALE_M).exp() * step);
        od_r += dr;
        od_m += dm;
        // Sunlight reaching this point, if the ground doesn't shadow it.
        if hits_ground(p, sun) {
            continue;
        }
        let l_len = exit_distance(p, sun, ATMO_R);
        let l_step = l_len / LIGHT_STEPS as f32;
        let (mut lod_r, mut lod_m) = (0.0f32, 0.0f32);
        for j in 0..LIGHT_STEPS {
            let lt = (j as f32 + 0.5) * l_step;
            let q = [p[0] + sun[0] * lt, p[1] + sun[1] * lt, p[2] + sun[2] * lt];
            let lh = (dot(q, q).sqrt() - EARTH_R).max(0.0);
            lod_r += (-lh / SCALE_R).exp() * l_step;
            lod_m += (-lh / SCALE_M).exp() * l_step;
        }
        for k in 0..3 {
            let tau = BETA_R[k] * (od_r + lod_r) + beta_m * 1.1 * (od_m + lod_m);
            let atten = (-tau).exp();
            sum_r[k] += atten * dr;
            sum_m[k] += atten * dm;
        }
    }
    let mut out = [0.0f32; 3];
    for k in 0..3 {
        out[k] = SUN_POWER * (sum_r[k] * BETA_R[k] * pr + sum_m[k] * beta_m * pm);
    }
    // A real sky is lit many times over, which whitens the horizon and keeps
    // it from going yellow-green; single scattering misses that. Fade a cool
    // white in as the sun climbs (a low sun keeps its fire).
    let alt = sun[1].clamp(-1.0, 1.0).asin().to_degrees();
    let whiten = ((alt - 3.0) / 11.0).clamp(0.0, 1.0);
    if whiten > 0.0 {
        let l = 0.2126 * out[0] + 0.7152 * out[1] + 0.0722 * out[2];
        let cool = [0.84, 0.97, 1.12];
        for k in 0..3 {
            let target = l * cool[k] + (out[k] - l) * 0.55;
            out[k] += (target - out[k]) * whiten * 0.8;
        }
    }
    // The night: a faint airglow, and moonlight's cold blue wash.
    let up = dir[1];
    let glow = 0.0035 + 0.05 * c.moon;
    out[0] += glow * 0.55 * (1.0 - 0.4 * up);
    out[1] += glow * 0.8 * (1.0 - 0.3 * up);
    out[2] += glow * 1.6;
    out
}

/// Squash radiance into 0..1 for the screen (a soft shoulder, so a bright sun
/// halo rolls off instead of clipping).
pub fn expose(c: V3, exposure: f32) -> V3 {
    [1.0 - (-c[0] * exposure).exp(), 1.0 - (-c[1] * exposure).exp(), 1.0 - (-c[2] * exposure).exp()]
}

/// The colour of direct sunlight at the ground: white-gold when high,
/// orange and then red as the sun sinks, gone once it is well below the horizon.
pub fn sun_transmittance(sun: V3, haze: f32) -> V3 {
    let alt = norm(sun)[1].clamp(-1.0, 1.0).asin().to_degrees();
    if alt < -4.0 {
        return [0.0; 3];
    }
    // Kasten-Young air mass.
    let air_mass = 1.0 / (alt.max(0.0).to_radians().sin() + 0.50572 * (alt.max(0.0) + 6.07995).powf(-1.6364));
    let mut t = [0.0f32; 3];
    for k in 0..3 {
        t[k] = (-(BETA_R[k] * SCALE_R + BETA_M * haze * 1.1 * SCALE_M) * air_mass).exp();
    }
    // Fade out through civil twilight, so the light doesn't snap off.
    let fade = ((alt + 4.0) / 4.0).clamp(0.0, 1.0);
    [t[0] * fade, t[1] * fade, t[2] * fade]
}

/// The sky's colour all round at a low angle: what distant land and the fog
/// fade into. Averaged over eight compass points.
pub fn horizon_color(c: &Conditions, exposure: f32) -> V3 {
    let mut sum = [0.0f32; 3];
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let e = 0.07f32;
        let col = expose(sky_radiance([a.cos() * (1.0 - e), e, a.sin() * (1.0 - e)], c), exposure);
        for k in 0..3 {
            sum[k] += col[k] / 8.0;
        }
    }
    sum
}

/// The sky's colour overhead, for ambient light.
pub fn zenith_color(c: &Conditions, exposure: f32) -> V3 {
    expose(sky_radiance([0.0, 1.0, 0.0], c), exposure)
}

// ---------------------------------------------------------------------------
// The density of the fog banks
// ---------------------------------------------------------------------------

fn hash3(x: u32, y: u32, z: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ z.wrapping_mul(0xC2B2_AE3D) ^ seed.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise on a lattice of `n` cells that wraps in x and z (so the
/// texture tiles and can be scrolled with the wind) and not in y.
fn lattice_noise(u: f32, v: f32, w: f32, n: u32, ny: u32, seed: u32) -> f32 {
    let (x, y, z) = (u * n as f32, v * ny as f32, w * n as f32);
    let (x0, y0, z0) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (x - x0, y - y0, z - z0);
    let sm = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy, fz) = (sm(fx), sm(fy), sm(fz));
    let at = |dx: u32, dy: u32, dz: u32| hash3((x0 as u32 + dx) % n, (y0 as u32 + dy).min(ny), (z0 as u32 + dz) % n, seed);
    let mut out = 0.0;
    for dx in 0..2 {
        for dy in 0..2 {
            for dz in 0..2 {
                let wgt = (if dx == 1 { fx } else { 1.0 - fx }) * (if dy == 1 { fy } else { 1.0 - fy }) * (if dz == 1 { fz } else { 1.0 - fz });
                out += wgt * at(dx, dy, dz);
            }
        }
    }
    out
}

/// A 3D grid of fog density (0..=255) for the blizzard banks: clumpy drifting
/// banks that thicken towards the ground (height fog). `w` x `h` x `d`, x
/// fastest, then y, then z. Tiles in x and z.
pub fn fog_density_field(w: usize, h: usize, d: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * d);
    for zi in 0..d {
        for yi in 0..h {
            for xi in 0..w {
                let (u, v, wv) = ((xi as f32 + 0.5) / w as f32, (yi as f32 + 0.5) / h as f32, (zi as f32 + 0.5) / d as f32);
                // Three octaves of clumps.
                let n = 0.55 * lattice_noise(u, v, wv, 3, 3, 1) + 0.3 * lattice_noise(u, v, wv, 6, 5, 2) + 0.15 * lattice_noise(u, v, wv, 12, 9, 3);
                // Thicker near the ground (v = 0), thin up high.
                let height = (1.0 - v).powf(1.6);
                let banks = ((n - 0.28) * 2.2).clamp(0.0, 1.0);
                let density = (0.18 + 0.82 * banks) * (0.12 + 0.88 * height);
                out.push((density.clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun_at(alt_deg: f32, az_deg: f32) -> V3 {
        let (a, z) = (alt_deg.to_radians(), az_deg.to_radians());
        [a.cos() * z.cos(), a.sin(), a.cos() * z.sin()]
    }

    fn conds(alt: f32) -> Conditions {
        Conditions { sun: sun_at(alt, 90.0), haze: 1.0, moon: 0.0 }
    }

    const EXPOSURE: f32 = 1.4;

    fn lum(c: V3) -> f32 {
        0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    }

    #[test]
    fn a_daytime_sky_is_blue_overhead() {
        let z = zenith_color(&conds(24.0), EXPOSURE);
        assert!(z[2] > z[1] && z[1] > z[0], "blue-weighted: {z:?}");
        assert!(z[2] > 0.3, "and bright: {z:?}");
    }

    #[test]
    fn the_horizon_is_paler_and_brighter_than_the_zenith() {
        let c = conds(24.0);
        let h = horizon_color(&c, EXPOSURE);
        let z = zenith_color(&c, EXPOSURE);
        assert!(lum(h) > lum(z), "horizon {h:?} vs zenith {z:?}");
        // Paler: less saturated.
        let sat = |c: V3| (c[2] - c[0]) / c[2].max(1e-3);
        assert!(sat(h) < sat(z), "paler at the horizon");
    }

    #[test]
    fn a_low_sun_turns_the_sky_round_it_orange() {
        let c = conds(4.0);
        // Looking at the sun's side of the horizon vs the opposite side.
        // (Compared as raw radiance: a bright sky saturates once exposed.)
        let toward = sky_radiance([0.0, 0.08, 1.0], &Conditions { sun: sun_at(4.0, 90.0), ..c });
        let away = sky_radiance([0.0, 0.08, -1.0], &Conditions { sun: sun_at(4.0, 90.0), ..c });
        assert!(toward[0] > toward[2] * 0.9, "warm toward the sun: {toward:?}");
        assert!(toward[0] / toward[2] > away[0] / away[2], "warmer than the far side: {toward:?} vs {away:?}");
        assert!(lum(toward) > lum(away));
    }

    #[test]
    fn sunlight_reddens_as_the_sun_sinks() {
        let high = sun_transmittance(sun_at(24.0, 90.0), 1.0);
        let low = sun_transmittance(sun_at(3.0, 90.0), 1.0);
        let ratio = |t: V3| t[0] / t[2].max(1e-6);
        assert!(ratio(low) > ratio(high) * 3.0, "much redder low: {low:?} vs {high:?}");
        assert!(high[0] > 0.5 && high[2] > 0.2, "still bright at noon: {high:?}");
        assert!(low[0] > low[1] && low[1] > low[2]);
        assert_eq!(sun_transmittance(sun_at(-10.0, 90.0), 1.0), [0.0; 3], "no sun at night");
    }

    #[test]
    fn the_light_fades_through_twilight_rather_than_snapping_off() {
        let mut last = f32::MAX;
        for alt in [10.0f32, 5.0, 2.0, 0.0, -2.0, -3.5, -4.5] {
            let t = lum(sun_transmittance(sun_at(alt, 90.0), 1.0));
            assert!(t <= last + 1e-6, "dimmer as it sinks ({alt}): {t} vs {last}");
            last = t;
        }
    }

    #[test]
    fn night_is_dark_but_not_black_and_the_moon_brightens_it() {
        let dark = Conditions { sun: sun_at(-35.0, 90.0), haze: 1.0, moon: 0.0 };
        let moonlit = Conditions { moon: 1.0, ..dark };
        let (a, b) = (horizon_color(&dark, EXPOSURE), horizon_color(&moonlit, EXPOSURE));
        assert!(lum(a) > 0.0 && lum(a) < 0.05, "night horizon {a:?}");
        assert!(lum(b) > lum(a));
        assert!(b[2] > b[0], "moonlight is blue: {b:?}");
        let day = horizon_color(&conds(24.0), EXPOSURE);
        assert!(lum(day) > lum(b) * 8.0);
    }

    #[test]
    fn twilight_glows_after_the_sun_has_set() {
        // Just below the horizon the high atmosphere is still lit: brighter than deep night.
        let dusk = horizon_color(&Conditions { sun: sun_at(-3.0, 90.0), haze: 1.0, moon: 0.0 }, EXPOSURE);
        let night = horizon_color(&Conditions { sun: sun_at(-35.0, 90.0), haze: 1.0, moon: 0.0 }, EXPOSURE);
        assert!(lum(dusk) > lum(night) * 2.0, "{dusk:?} vs {night:?}");
    }

    #[test]
    fn haze_puts_a_bright_halo_round_the_sun() {
        let near_sun = [0.0, 0.3, 1.0];
        let clear = Conditions { sun: sun_at(17.0, 90.0), haze: 1.0, moon: 0.0 };
        let hazy = Conditions { haze: 3.0, ..clear };
        assert!(lum(sky_radiance(near_sun, &hazy)) > lum(sky_radiance(near_sun, &clear)) * 1.2);
    }

    #[test]
    fn the_fog_field_is_patchy_thicker_low_down_and_tiles() {
        let (w, h, d) = (32, 16, 32);
        let f = fog_density_field(w, h, d);
        assert_eq!(f.len(), w * h * d);
        let at = |x: usize, y: usize, z: usize| f[(z * h + y) * w + x] as f32;
        let layer_mean = |y: usize| (0..d).flat_map(|z| (0..w).map(move |x| (x, z))).map(|(x, z)| at(x, y, z)).sum::<f32>() / (w * d) as f32;
        assert!(layer_mean(0) > layer_mean(h - 1) * 2.0, "ground fog: {} vs {}", layer_mean(0), layer_mean(h - 1));
        let low: Vec<f32> = (0..d).flat_map(|z| (0..w).map(move |x| (x, z))).map(|(x, z)| at(x, 2, z)).collect();
        let mean = low.iter().sum::<f32>() / low.len() as f32;
        let var = low.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / low.len() as f32;
        assert!(var.sqrt() > 12.0, "patchy, not flat: sd {}", var.sqrt());
        // Wraps in x: the seam between the last and first column is no rougher than any other step.
        let step = |x0: usize, x1: usize| (0..d).map(|z| (at(x0, 3, z) - at(x1, 3, z)).abs()).sum::<f32>() / d as f32;
        let typical = (0..w - 1).map(|x| step(x, x + 1)).sum::<f32>() / (w - 1) as f32;
        assert!(step(w - 1, 0) < typical * 2.5 + 4.0, "seam {} vs typical {typical}", step(w - 1, 0));
    }

    #[test]
    fn it_never_produces_nonsense() {
        for alt in [-40.0f32, -5.0, 0.0, 10.0, 24.0, 80.0] {
            for dir in [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.3, -0.1, -0.9], [0.0, 0.0, 0.0]] {
                let r = sky_radiance(dir, &conds(alt));
                assert!(r.iter().all(|v| v.is_finite() && *v >= 0.0), "{alt} {dir:?}: {r:?}");
                let e = expose(r, EXPOSURE);
                assert!(e.iter().all(|v| (0.0..=1.0).contains(v)));
            }
        }
    }
}
