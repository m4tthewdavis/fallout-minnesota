//! The sky's colours: what paints the sky dome, colours the fog and tints
//! the sunlight. The dome is an art-directed palette keyed on how high the
//! sun is (deep clean blue by day over a pale horizon, gold round a low sun,
//! orange and rose at sunset with a violet belt opposite, a long ember dusk,
//! and a moonlit night), bent round the sun's compass direction and thickened
//! by haze. Sunlight's own colour comes from how much air it has crossed.
//! Pure maths (no Bevy), so the colours are unit-tested.

use super::mathx::{dot3 as dot, smoothstep};

/// Rayleigh scattering per metre (red, green, blue): why low sunlight is red.
const BETA_R: [f32; 3] = [7.0e-6, 16.5e-6, 41.0e-6];
/// Mie scattering per metre (haze, ice crystals).
const BETA_M: f32 = 21.0e-6;
const SCALE_R: f32 = 8_000.0;
const SCALE_M: f32 = 1_200.0;

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

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l < 1e-9 {
        [0.0, 1.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

fn mix3(a: V3, b: V3, t: f32) -> V3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The sky at one height of the sun: straight up, at the horizon away from
/// the sun, at the horizon under the sun, and the glare round the sun itself.
struct Key {
    /// Sun altitude in degrees this look belongs to.
    alt: f32,
    zenith: V3,
    away: V3,
    toward: V3,
    glare: V3,
}

/// The day's looks, from deep night up to full day (linear RGB, as shown).
const KEYS: [Key; 7] = [
    // Night: near black, a trace of blue.
    Key { alt: -14.0, zenith: [0.006, 0.010, 0.028], away: [0.013, 0.020, 0.044], toward: [0.013, 0.020, 0.044], glare: [0.0, 0.0, 0.0] },
    // Late dusk: the last ember on the horizon under a navy sky.
    Key { alt: -7.0, zenith: [0.014, 0.028, 0.090], away: [0.035, 0.050, 0.120], toward: [0.300, 0.120, 0.080], glare: [0.10, 0.03, 0.01] },
    // Dusk: a band of fire where the sun went down, violet opposite.
    Key { alt: -3.0, zenith: [0.035, 0.080, 0.230], away: [0.170, 0.160, 0.300], toward: [0.920, 0.380, 0.140], glare: [0.50, 0.18, 0.05] },
    // Sunset: orange under the sun, the rose "belt of Venus" on the far side.
    Key { alt: 1.0, zenith: [0.075, 0.160, 0.400], away: [0.500, 0.400, 0.500], toward: [1.000, 0.580, 0.220], glare: [0.90, 0.45, 0.14] },
    // Golden hour.
    Key { alt: 6.0, zenith: [0.088, 0.200, 0.450], away: [0.560, 0.620, 0.740], toward: [0.980, 0.800, 0.540], glare: [0.80, 0.60, 0.34] },
    // A low winter day.
    Key { alt: 14.0, zenith: [0.090, 0.215, 0.490], away: [0.590, 0.700, 0.840], toward: [0.860, 0.850, 0.820], glare: [0.60, 0.56, 0.46] },
    // Full day: deep clean blue over a pale, luminous horizon.
    Key { alt: 30.0, zenith: [0.085, 0.210, 0.500], away: [0.600, 0.715, 0.860], toward: [0.800, 0.840, 0.880], glare: [0.50, 0.50, 0.46] },
];

/// The look for a sun `alt` degrees up, blended between its neighbours.
fn key_at(alt: f32) -> Key {
    let first = &KEYS[0];
    if alt <= first.alt {
        return Key { alt, zenith: first.zenith, away: first.away, toward: first.toward, glare: first.glare };
    }
    for pair in KEYS.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if alt <= b.alt {
            let t = smoothstep(a.alt, b.alt, alt);
            return Key { alt, zenith: mix3(a.zenith, b.zenith, t), away: mix3(a.away, b.away, t), toward: mix3(a.toward, b.toward, t), glare: mix3(a.glare, b.glare, t) };
        }
    }
    let last = &KEYS[KEYS.len() - 1];
    Key { alt, zenith: last.zenith, away: last.away, toward: last.toward, glare: last.glare }
}

/// The colour of the sky in the direction `view` (unit; below the horizon is
/// treated as the horizon), as linear RGB in 0..=1 ready for the screen.
pub fn sky_color(view: V3, c: &Conditions) -> V3 {
    let mut dir = norm(view);
    dir[1] = dir[1].max(0.0);
    let dir = norm(dir);
    let sun = norm(c.sun);
    let alt = sun[1].clamp(-1.0, 1.0).asin().to_degrees();
    let key = key_at(alt);
    let haze = c.haze.max(0.0);

    // How much this direction faces the sun's side of the compass (0..1).
    let flat = |v: V3| {
        let l = (v[0] * v[0] + v[2] * v[2]).sqrt();
        if l < 1e-6 {
            [0.0, 0.0]
        } else {
            [v[0] / l, v[2] / l]
        }
    };
    let (d2, s2) = (flat(dir), flat(sun));
    let facing = 0.5 + 0.5 * (d2[0] * s2[0] + d2[1] * s2[1]);
    let sunward = facing * facing;

    // The horizon's colour reaches up the sky in a band: narrow on a clear
    // day, wider round a low sun and in haze.
    let low_sun = 1.0 - smoothstep(0.0, 14.0, alt);
    let reach = (5.2 - 2.2 * low_sun * sunward) / (1.0 + 0.35 * (haze - 1.0).max(0.0));
    let band = (-dir[1] * reach).exp();
    let horizon = mix3(key.away, key.toward, sunward);
    let mut out = mix3(key.zenith, horizon, band);

    // At sunset the Earth's shadow lies along the far horizon: a cool grey-blue
    // strip under the rose belt.
    let dusk = smoothstep(-8.0, -1.0, alt) * (1.0 - smoothstep(2.0, 7.0, alt));
    let shadow = dusk * (1.0 - sunward) * (-dir[1] * 22.0).exp() * 0.6;
    out = mix3(out, [key.zenith[0] * 1.6, key.zenith[1] * 1.5, key.zenith[2] * 1.2], shadow);

    // Glare round the sun: light scattered forward by haze and ice crystals.
    let mu = dot(dir, sun).max(0.0);
    let glare = (0.30 * mu.powf(10.0) + 0.45 * mu.powf(90.0)) * (0.7 + 0.3 * haze.min(4.0));
    for k in 0..3 {
        out[k] += key.glare[k] * glare;
    }

    // The night: a faint airglow, and moonlight's cold blue wash.
    let night = 1.0 - smoothstep(-10.0, -2.0, alt);
    let wash = (0.002 + 0.030 * c.moon.clamp(0.0, 1.0)) * night;
    out[0] += wash * 0.55 * (1.0 - 0.4 * dir[1]);
    out[1] += wash * 0.80 * (1.0 - 0.3 * dir[1]);
    out[2] += wash * 1.60;
    [out[0].clamp(0.0, 1.0), out[1].clamp(0.0, 1.0), out[2].clamp(0.0, 1.0)]
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
pub fn horizon_color(c: &Conditions) -> V3 {
    let mut sum = [0.0f32; 3];
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let e = 0.07f32;
        let col = sky_color([a.cos() * (1.0 - e), e, a.sin() * (1.0 - e)], c);
        for k in 0..3 {
            sum[k] += col[k] / 8.0;
        }
    }
    sum
}

/// The sky's colour overhead, for ambient light.
pub fn zenith_color(c: &Conditions) -> V3 {
    sky_color([0.0, 1.0, 0.0], c)
}

// ---------------------------------------------------------------------------
// Haze
// ---------------------------------------------------------------------------

/// The exponential fog density the picture is drawn with, for the weather's
/// own `fog_density`. Calm winter air is clear: far woods and hills stand
/// in layers, each paler than the last, out to a kilometre and more. A storm
/// closes in to the same whiteout the weather asks for.
pub fn air_density(weather_fog: f32) -> f32 {
    (weather_fog - 0.0053).max(0.0015)
}

/// How much of a thing `distance` metres off still shows through air of `density`.
#[cfg(test)]
fn seen_through(density: f32, distance: f32) -> f32 {
    (-density * distance).exp()
}

// ---------------------------------------------------------------------------
// Cloud
// ---------------------------------------------------------------------------

/// How much of the sky the cumulus deck covers (0..=1) `hours` into the game:
/// fair-weather cloud that builds and thins through the days, and closes
/// right over as the sky goes `overcast` (0..=1) ahead of a storm.
pub fn cloud_cover(hours: f32, overcast: f32) -> f32 {
    let fair = 0.40 + 0.10 * (hours * 0.37).sin() + 0.06 * (hours * 0.11 + 1.0).sin();
    fair + (1.0 - fair) * overcast.clamp(0.0, 1.0)
}

/// How much high cirrus is streaked across the sky (0..=1), `hours` into the
/// game. It comes and goes on its own slower rhythm.
pub fn cirrus_amount(hours: f32) -> f32 {
    (0.55 + 0.35 * (hours * 0.19 + 2.0).sin()).clamp(0.0, 1.0)
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

    #[test]
    fn calm_air_is_clear_and_a_blizzard_is_a_whiteout() {
        use crate::sim::weather::{Phase, Weather};
        let calm = air_density(Weather::conditions_for(Phase::Calm).fog_density);
        let warning = air_density(Weather::conditions_for(Phase::Warning).fog_density);
        let storm = air_density(Weather::conditions_for(Phase::Blizzard).fog_density);
        assert!(calm < warning && warning < storm);
        // On a calm day the far side of the map is plain to see and hills a
        // kilometre off still show; in a blizzard a tree 40 m away is all but gone.
        assert!(seen_through(calm, 400.0) > 0.35);
        assert!(seen_through(calm, 1000.0) > 0.08);
        assert!(seen_through(storm, 40.0) < 0.08);
        // Never perfectly clear, whatever the weather says.
        assert!(air_density(0.0) > 0.0 && air_density(-1.0) > 0.0);
    }

    #[test]
    fn cloud_is_broken_in_fair_weather_and_closes_over_in_a_storm() {
        for i in 0..2000 {
            let hours = i as f32 * 0.37;
            let fair = cloud_cover(hours, 0.0);
            assert!((0.2..=0.6).contains(&fair), "fair-weather cover {fair} at {hours} h");
            assert!((cloud_cover(hours, 1.0) - 1.0).abs() < 1e-6);
            assert!(cloud_cover(hours, 0.5) > fair);
            assert!((0.0..=1.0).contains(&cirrus_amount(hours)));
        }
        // Out-of-range overcast is clamped, not extrapolated.
        assert_eq!(cloud_cover(3.0, 7.0), cloud_cover(3.0, 1.0));
        assert_eq!(cloud_cover(3.0, -7.0), cloud_cover(3.0, 0.0));
    }

    fn sun_at(alt_deg: f32, az_deg: f32) -> V3 {
        let (a, z) = (alt_deg.to_radians(), az_deg.to_radians());
        [a.cos() * z.cos(), a.sin(), a.cos() * z.sin()]
    }

    fn conds(alt: f32) -> Conditions {
        Conditions { sun: sun_at(alt, 90.0), haze: 1.0, moon: 0.0 }
    }

    fn lum(c: V3) -> f32 {
        0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    }

    #[test]
    fn a_daytime_sky_is_blue_overhead() {
        let z = zenith_color(&conds(24.0));
        assert!(z[2] > z[1] && z[1] > z[0], "blue-weighted: {z:?}");
        assert!(z[2] > 0.3, "and bright: {z:?}");
    }

    #[test]
    fn the_horizon_is_paler_and_brighter_than_the_zenith() {
        let c = conds(24.0);
        let h = horizon_color(&c);
        let z = zenith_color(&c);
        assert!(lum(h) > lum(z), "horizon {h:?} vs zenith {z:?}");
        // Paler: less saturated.
        let sat = |c: V3| (c[2] - c[0]) / c[2].max(1e-3);
        assert!(sat(h) < sat(z), "paler at the horizon");
        // And still blue, not the green-grey of a muddy day.
        assert!(h[2] > h[1] && h[1] > h[0], "a clean pale blue: {h:?}");
    }

    #[test]
    fn the_sky_darkens_smoothly_from_horizon_to_zenith() {
        for alt in [-5.0f32, 2.0, 10.0, 24.0] {
            let c = conds(alt);
            let mut last = f32::MAX;
            // (From a few degrees up: at sunset the Earth's shadow darkens the
            // far horizon itself, under the rose belt.)
            for e in [0.12f32, 0.2, 0.3, 0.5, 0.75, 1.0] {
                // Looking away from the sun, so its glare doesn't come into it.
                let l = lum(sky_color([0.0, e, -(1.0 - e * e).max(0.0).sqrt()], &c));
                assert!(l <= last + 1e-4, "sun {alt}: brighter higher up at {e}");
                last = l;
            }
        }
    }

    #[test]
    fn a_low_sun_turns_the_sky_round_it_orange() {
        let c = conds(4.0);
        // Looking at the sun's side of the horizon vs the opposite side.
        let toward = sky_color([0.0, 0.08, 1.0], &c);
        let away = sky_color([0.0, 0.08, -1.0], &c);
        assert!(toward[0] > toward[2] * 1.3, "warm toward the sun: {toward:?}");
        assert!(toward[0] / toward[2] > away[0] / away[2], "warmer than the far side: {toward:?} vs {away:?}");
        assert!(lum(toward) > lum(away));
        // The far side at sunset is rose, not grey: red and blue both over green.
        let belt = sky_color([0.0, 0.08, -1.0], &conds(0.5));
        assert!(belt[0] > belt[1] && belt[2] > belt[1], "a rose belt opposite the sunset: {belt:?}");
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
        let (a, b) = (horizon_color(&dark), horizon_color(&moonlit));
        assert!(lum(a) > 0.0 && lum(a) < 0.05, "night horizon {a:?}");
        assert!(lum(b) > lum(a));
        assert!(b[2] > b[0], "moonlight is blue: {b:?}");
        let day = horizon_color(&conds(24.0));
        assert!(lum(day) > lum(b) * 8.0);
    }

    #[test]
    fn twilight_glows_after_the_sun_has_set() {
        // Just below the horizon the high atmosphere is still lit: brighter than deep night.
        let dusk = horizon_color(&Conditions { sun: sun_at(-3.0, 90.0), haze: 1.0, moon: 0.0 });
        let night = horizon_color(&Conditions { sun: sun_at(-35.0, 90.0), haze: 1.0, moon: 0.0 });
        assert!(lum(dusk) > lum(night) * 2.0, "{dusk:?} vs {night:?}");
        // And the glow sits where the sun went down.
        let c = Conditions { sun: sun_at(-3.0, 90.0), haze: 1.0, moon: 0.0 };
        let (toward, away) = (sky_color([0.0, 0.05, 1.0], &c), sky_color([0.0, 0.05, -1.0], &c));
        assert!(lum(toward) > lum(away) * 1.5 && toward[0] > toward[2]);
    }

    #[test]
    fn haze_puts_a_bright_halo_round_the_sun() {
        let near_sun = [0.0, 0.3, 1.0];
        let clear = Conditions { sun: sun_at(17.0, 90.0), haze: 1.0, moon: 0.0 };
        let hazy = Conditions { haze: 3.0, ..clear };
        assert!(lum(sky_color(near_sun, &hazy)) > lum(sky_color(near_sun, &clear)) * 1.1);
    }

    #[test]
    fn the_day_passes_without_a_jump_in_colour() {
        // Step the sun through the whole range: no two neighbouring looks differ much.
        let mut last: Option<[V3; 3]> = None;
        let mut alt = -20.0f32;
        while alt <= 35.0 {
            let c = conds(alt);
            let now = [zenith_color(&c), sky_color([0.0, 0.05, 1.0], &c), sky_color([0.0, 0.05, -1.0], &c)];
            if let Some(prev) = last {
                for (a, b) in prev.iter().zip(&now) {
                    for k in 0..3 {
                        assert!((a[k] - b[k]).abs() < 0.03, "a jump at sun {alt}: {a:?} -> {b:?}");
                    }
                }
            }
            last = Some(now);
            alt += 0.1;
        }
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
            for haze in [0.0f32, 1.0, 4.0, 50.0] {
                for dir in [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.3, -0.1, -0.9], [0.0, 0.0, 0.0]] {
                    let r = sky_color(dir, &Conditions { haze, ..conds(alt) });
                    assert!(r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)), "{alt} {dir:?}: {r:?}");
                }
            }
        }
    }
}
