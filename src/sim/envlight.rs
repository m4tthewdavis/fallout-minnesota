//! Image-based lighting rules: which baked sky to light the world with, how
//! strongly, what tint, and how much of the old flat `AmbientLight` to keep.
//!
//! The maps come from `tools/bake_ibl.py` and are normalised so that the
//! sphere-average of the diffuse map is 1.0. That makes `intensity` read
//! exactly like the old uniform ambient brightness (Bevy cd/m^2), so the fill
//! level of the game does not jump when IBL is switched on: the same energy
//! now arrives from the right directions (bright sky above, darker ground and
//! trees below) and snow, ice, metal and guns pick up reflections.
//!
//! The sun is NOT in the maps (the baker clamps it out): the game's
//! `DirectionalLight` supplies it, so nothing is lit twice.

use super::daynight::Sky;
use super::interiors::Interior;
use super::mathx::{lerp, smoothstep};

/// Which pair of baked cubemaps is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvMap {
    /// `snow_field`: flat, soft, low-contrast overcast light.
    Overcast,
    /// `rural_winter_roadside`: clearer winter sky with a brighter dome.
    Clear,
}

impl EnvMap {
    /// Asset path (under `assets/`) of the diffuse irradiance cubemap.
    pub fn diffuse_path(self) -> &'static str {
        match self {
            EnvMap::Overcast => "environment/snow_field_diffuse.ktx2",
            EnvMap::Clear => "environment/rural_winter_roadside_diffuse.ktx2",
        }
    }

    /// Asset path (under `assets/`) of the GGX-prefiltered specular cubemap.
    pub fn specular_path(self) -> &'static str {
        match self {
            EnvMap::Overcast => "environment/snow_field_specular.ktx2",
            EnvMap::Clear => "environment/rural_winter_roadside_specular.ktx2",
        }
    }

    /// Azimuth (radians, 0 = world -Z, +PI/2 = +X) of the sun in the baked
    /// photo, as printed by `bake_ibl.py`. Used to line up its bright patch of
    /// sky with the game's sun.
    pub fn baked_sun_azimuth(self) -> f32 {
        match self {
            EnvMap::Overcast => 0.677, // 38.8 deg
            EnvMap::Clear => 0.494,    // 28.3 deg
        }
    }
}

/// The least of the old uniform `AmbientLight` that stays outdoors once IBL
/// fills in. Not zero: the floor keeps shadowed snow from going to pure black
/// if a map is still loading.
pub const OUTDOOR_AMBIENT_KEEP: f32 = 0.18;
/// The most of the outdoor fill the photographed sky may supply, on a clear
/// day. The rest stays with the flat ambient light, which carries the
/// atmosphere model's colour: the photo maps can't be tinted, so a sky that
/// did all the filling would light dusk, moonlight and a rad storm with the
/// same white noon.
pub const SKY_SHARE_DAY: f32 = 0.55;
/// Indoors the room keeps its own full ambient.
pub const INDOOR_AMBIENT_KEEP: f32 = 1.0;
/// Seconds to fade the old map out (and the new one in) when swapping.
pub const FADE_SECS: f32 = 1.2;

/// Overcast amount above which we switch to the grey map, and below which we
/// switch back (a gap, so drifting weather does not flicker between maps).
const TO_OVERCAST: f32 = 0.6;
const TO_CLEAR: f32 = 0.4;

/// Weather as the lighting sees it (same numbers `apply_atmosphere` computes).
#[derive(Clone, Copy, Debug)]
pub struct Weather {
    /// 0..=1 storm cloud cover: `((fog - 0.01) / 0.03).clamp(0, 1)`.
    pub overcast: f32,
    /// 0..=1 rad-storm green whiteout (`VisualWeather::sick`).
    pub sick: f32,
    /// 0..=1 how much light survives the weather (`VisualWeather::light`).
    pub light: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Weather { overcast: 0.0, sick: 0.0, light: 1.0 }
    }
}

/// What the camera's `EnvironmentMapLight` and the `AmbientLight` should be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvLight {
    pub map: EnvMap,
    /// `EnvironmentMapLight::intensity` (cd/m^2). 0 means "off".
    pub intensity: f32,
    /// Linear RGB multiplier, brightest channel = 1. Apply to the ambient
    /// colour (the environment light has no tint of its own, so bake it into
    /// `intensity` per channel only if you need it; see `tinted_ambient`).
    pub tint: [f32; 3],
    /// Multiply the existing `AmbientLight::brightness` by this.
    pub ambient_keep: f32,
}

fn clean(x: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

/// The fill brightness the game used before IBL (flat ambient, cd/m^2).
/// Kept here so IBL replaces it one for one.
pub fn legacy_fill(sky: &Sky, weather: &Weather) -> f32 {
    let daylight = clean(sky.daylight, 0.12);
    let light = clean(weather.light, 1.0);
    let day = ((daylight - 0.12) / 0.88).clamp(0.0, 1.0);
    ((170.0 + 230.0 * light) * daylight).max(60.0) * (0.75 + 0.5 * day)
}

/// Pick a map with hysteresis. `current` is what is showing now.
pub fn choose_map(current: EnvMap, sky: &Sky, weather: &Weather) -> EnvMap {
    let overcast = clean(weather.overcast, 0.0).max(clean(weather.sick, 0.0));
    // At night there is no clear dome to reflect: use the flat map.
    let dark = clean(sky.sun, 0.0) < 0.2;
    match current {
        EnvMap::Clear if overcast > TO_OVERCAST || dark => EnvMap::Overcast,
        EnvMap::Overcast if overcast < TO_CLEAR && clean(sky.sun, 0.0) > 0.3 => EnvMap::Clear,
        other => other,
    }
}

/// Everything the renderer needs this frame.
///
/// `interior` is `Some(room)` while the player is inside: the sky does not
/// reach a vault or fish house, so the map is only a very dim neutral
/// (a faint sheen on metal, never a window-lit glow) and the room keeps its
/// own ambient.
pub fn evaluate(current: EnvMap, sky: &Sky, weather: &Weather, interior: Option<Interior>) -> EnvLight {
    if let Some(room) = interior {
        let (_, brightness) = room.ambient();
        return EnvLight {
            // Keep whichever map is up: a door never starts a swap.
            map: current,
            intensity: 0.05 * brightness.max(0.0),
            tint: [1.0, 1.0, 1.0],
            ambient_keep: INDOOR_AMBIENT_KEEP,
        };
    }
    let map = choose_map(current, sky, weather);
    let sick = clean(weather.sick, 0.0);
    let overcast = clean(weather.overcast, 0.0);
    let warmth = clean(sky.warmth, 0.0);
    let daylight = clean(sky.daylight, 0.12);
    let moon = clean(sky.moon, 0.0) * (1.0 - clean(sky.sun, 0.0));

    let gain = match map {
        EnvMap::Overcast => 1.1, // the grey map has less contrast: lift a touch
        EnvMap::Clear => 1.0,
    };
    let share = sky_share(sky, weather);
    let intensity = legacy_fill(sky, weather) * gain * share;

    // Tint: warm at sunrise/sunset (clear skies only), cold blue under the
    // moon, sickly green in a rad storm.
    let warm = [1.0, 0.86, 0.68];
    let night = [0.72, 0.82, 1.0];
    let green = [0.62, 1.0, 0.60];
    let mut tint = [1.0f32; 3];
    let w = warmth * (1.0 - overcast) * 0.55;
    let n = (1.0 - smoothstep(0.12, 0.5, daylight)) * (0.5 + 0.5 * moon);
    for i in 0..3 {
        tint[i] = lerp(tint[i], warm[i], w);
        tint[i] = lerp(tint[i], night[i], n);
        tint[i] = lerp(tint[i], green[i], sick * 0.7);
    }
    let m = tint.iter().cloned().fold(0.0, f32::max).max(1e-4);
    for t in &mut tint {
        *t /= m;
    }
    EnvLight { map, intensity, tint, ambient_keep: (1.0 - share).max(OUTDOOR_AMBIENT_KEEP) }
}

/// How much of the outdoor fill comes from the sky map (the rest is the flat,
/// atmosphere-coloured ambient). Most in clear daylight; less at golden hour
/// and under the moon, whose colour only the ambient light can carry, and in
/// a rad storm, whose green it must keep.
pub fn sky_share(sky: &Sky, weather: &Weather) -> f32 {
    let daylight = clean(sky.daylight, 0.12);
    let warmth = clean(sky.warmth, 0.0) * (1.0 - clean(weather.overcast, 0.0));
    let sick = clean(weather.sick, 0.0);
    let day = smoothstep(0.12, 0.5, daylight);
    let share = SKY_SHARE_DAY * lerp(0.45, 1.0, day) * (1.0 - 0.4 * warmth) * (1.0 - 0.45 * sick);
    share.clamp(0.0, 1.0 - OUTDOOR_AMBIENT_KEEP)
}

/// Y rotation (radians) for `EnvironmentMapLight::rotation`
/// (`Quat::from_rotation_y`) so the baked sun patch sits under the game's
/// sun. Bevy rotates the map content, turning azimuth `a` into `a - yaw`.
/// Result is in (-PI, PI].
pub fn map_yaw(map: EnvMap, sun_pos: [f32; 3]) -> f32 {
    let (x, z) = (sun_pos[0], sun_pos[2]);
    if !(x.is_finite() && z.is_finite()) || (x == 0.0 && z == 0.0) {
        return 0.0;
    }
    let game = x.atan2(-z);
    let mut yaw = map.baked_sun_azimuth() - game;
    let tau = std::f32::consts::TAU;
    yaw = (yaw + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    if yaw <= -std::f32::consts::PI {
        yaw += tau;
    }
    yaw
}

/// Fades the active map out, swaps it, and fades the new one in, so a change
/// of weather never pops the lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvFade {
    /// The map currently loaded on the camera.
    pub shown: EnvMap,
    /// 0..=1 multiplier for the intensity.
    pub level: f32,
}

impl EnvFade {
    pub fn new(map: EnvMap) -> Self {
        EnvFade { shown: map, level: 1.0 }
    }

    /// Advance by `dt` seconds towards `want`. Returns the multiplier to apply
    /// to `EnvLight::intensity`; read `shown` for the map to put on the camera.
    pub fn step(&mut self, dt: f32, want: EnvMap) -> f32 {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let rate = dt / (FADE_SECS * 0.5);
        if self.shown != want {
            self.level -= rate;
            if self.level <= 0.0 {
                self.shown = want;
                self.level = 0.0;
            }
        } else {
            self.level = (self.level + rate).min(1.0);
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::daynight::Clock;

    fn sky_at(h: f32) -> Sky {
        Clock { hours: h, day: 1 }.sky()
    }

    fn storm() -> Weather {
        Weather { overcast: 1.0, sick: 0.0, light: 0.5 }
    }

    #[test]
    fn clear_noon_uses_the_clear_map_and_a_storm_uses_the_grey_one() {
        let noon = sky_at(12.0);
        assert_eq!(choose_map(EnvMap::Clear, &noon, &Weather::default()), EnvMap::Clear);
        assert_eq!(choose_map(EnvMap::Clear, &noon, &storm()), EnvMap::Overcast);
        assert_eq!(choose_map(EnvMap::Overcast, &noon, &Weather::default()), EnvMap::Clear);
    }

    #[test]
    fn rad_storm_counts_as_overcast_even_if_the_fog_is_thin() {
        let w = Weather { overcast: 0.0, sick: 1.0, light: 1.0 };
        assert_eq!(choose_map(EnvMap::Clear, &sky_at(12.0), &w), EnvMap::Overcast);
    }

    #[test]
    fn night_uses_the_flat_map() {
        assert_eq!(choose_map(EnvMap::Clear, &sky_at(0.0), &Weather::default()), EnvMap::Overcast);
        assert_eq!(choose_map(EnvMap::Overcast, &sky_at(0.0), &Weather::default()), EnvMap::Overcast);
    }

    #[test]
    fn map_choice_does_not_flicker_inside_the_hysteresis_band() {
        let noon = sky_at(12.0);
        let mid = Weather { overcast: 0.5, sick: 0.0, light: 1.0 };
        assert_eq!(choose_map(EnvMap::Clear, &noon, &mid), EnvMap::Clear);
        assert_eq!(choose_map(EnvMap::Overcast, &noon, &mid), EnvMap::Overcast);
        // Exactly on the edges nothing changes either.
        let hi = Weather { overcast: TO_OVERCAST, ..mid };
        let lo = Weather { overcast: TO_CLEAR, ..mid };
        assert_eq!(choose_map(EnvMap::Clear, &noon, &hi), EnvMap::Clear);
        assert_eq!(choose_map(EnvMap::Overcast, &noon, &lo), EnvMap::Overcast);
    }

    #[test]
    fn ibl_replaces_the_old_fill_instead_of_adding_to_it() {
        let sky = sky_at(12.0);
        let e = evaluate(EnvMap::Clear, &sky, &Weather::default(), None);
        let old = legacy_fill(&sky, &Weather::default());
        assert!(e.intensity > old * 0.3 && e.intensity < old, "the sky does a good part of the filling");
        assert!(e.ambient_keep < 0.7 && e.ambient_keep >= OUTDOOR_AMBIENT_KEEP, "old ambient dialled down, not removed");
        // Total outdoor fill (IBL + what is left of the ambient) must stay
        // close to the old look, never double it.
        let total = e.intensity + old * e.ambient_keep;
        assert!(total < old * 1.4 && total > old * 0.9, "total {total} vs old {old}");
    }

    #[test]
    fn night_is_dark_but_not_black_and_cold_blue() {
        let night = evaluate(EnvMap::Overcast, &sky_at(0.0), &Weather::default(), None);
        let day = evaluate(EnvMap::Clear, &sky_at(12.0), &Weather::default(), None);
        let old = legacy_fill(&sky_at(0.0), &Weather::default());
        assert!(night.intensity + old * night.ambient_keep > 20.0, "never pitch black");
        assert!(night.intensity < day.intensity * 0.5);
        assert!(night.tint[2] > night.tint[0], "moonlight is blue");
    }

    #[test]
    fn sunrise_is_warm_only_when_the_sky_is_clear() {
        let dawn = sky_at(6.3);
        assert!(dawn.warmth > 0.3);
        let clear = evaluate(EnvMap::Clear, &dawn, &Weather::default(), None);
        let cloudy = evaluate(EnvMap::Clear, &dawn, &storm(), None);
        assert!(clear.tint[0] > clear.tint[2], "warm light at dawn");
        assert!(cloudy.tint[0] - cloudy.tint[2] < clear.tint[0] - clear.tint[2], "clouds mute the orange");
    }

    #[test]
    fn coloured_light_leans_on_the_tinted_ambient() {
        // The photo sky can't be tinted, so at dusk, at night and in a rad
        // storm more of the fill stays with the atmosphere-coloured ambient.
        let w = Weather::default();
        let noon = sky_share(&sky_at(12.0), &w);
        assert!(sky_share(&sky_at(0.0), &w) < noon * 0.6);
        let dusk = sky_at(17.6);
        assert!(dusk.warmth > 0.3);
        assert!(sky_share(&dusk, &w) < noon);
        let sick = Weather { overcast: 1.0, sick: 1.0, light: 0.5 };
        assert!(sky_share(&sky_at(12.0), &sick) < noon * 0.6);
        for h in 0..24 {
            let s = sky_share(&sky_at(h as f32), &w);
            assert!(s > 0.0 && s <= 1.0 - OUTDOOR_AMBIENT_KEEP);
        }
    }

    #[test]
    fn rad_storm_tints_the_fill_green() {
        let w = Weather { overcast: 1.0, sick: 1.0, light: 0.5 };
        let e = evaluate(EnvMap::Overcast, &sky_at(12.0), &w, None);
        assert!(e.tint[1] > e.tint[0] && e.tint[1] > e.tint[2]);
    }

    #[test]
    fn tint_is_normalised_so_it_never_adds_light() {
        for h in [0.0, 5.0, 6.3, 9.0, 12.0, 17.8, 20.0] {
            for (o, s) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let w = Weather { overcast: o, sick: s, light: 0.6 };
                let t = evaluate(EnvMap::Clear, &sky_at(h), &w, None).tint;
                let m = t.iter().cloned().fold(0.0, f32::max);
                assert!((m - 1.0).abs() < 1e-5, "max channel {m} at {h}h");
                assert!(t.iter().all(|c| *c > 0.0));
            }
        }
    }

    #[test]
    fn indoors_gets_no_sky_and_keeps_the_rooms_own_light() {
        let rooms = [Interior::FishHouse(0), Interior::VaultLobby, Interior::Mart, Interior::Reactor];
        for room in rooms {
            let e = evaluate(EnvMap::Clear, &sky_at(12.0), &Weather::default(), Some(room));
            let (_, room_light) = room.ambient();
            assert!(e.intensity < room_light * 0.1, "{room:?}: only a faint sheen");
            assert_eq!(e.ambient_keep, 1.0);
            assert_eq!(e.tint, [1.0, 1.0, 1.0], "neutral");
            // Blazing noon outside makes no difference inside.
            let night = evaluate(EnvMap::Overcast, &sky_at(0.0), &storm(), Some(room));
            assert_eq!(e.intensity, night.intensity);
            // And going in or out never starts a map swap.
            assert_eq!(e.map, EnvMap::Clear);
            assert_eq!(night.map, EnvMap::Overcast);
        }
    }

    #[test]
    fn garbage_input_still_gives_sane_light() {
        let mut sky = sky_at(12.0);
        sky.daylight = f32::NAN;
        sky.warmth = f32::INFINITY;
        sky.sun = f32::NAN;
        sky.moon = -5.0;
        let w = Weather { overcast: f32::NAN, sick: f32::NEG_INFINITY, light: f32::NAN };
        let e = evaluate(EnvMap::Clear, &sky, &w, None);
        assert!(e.intensity.is_finite() && e.intensity > 0.0 && e.intensity < 2000.0);
        assert!(e.tint.iter().all(|c| c.is_finite() && *c > 0.0 && *c <= 1.0));
        // Out-of-range weather clamps rather than amplifying.
        let big = Weather { overcast: 50.0, sick: 9.0, light: 7.0 };
        let b = evaluate(EnvMap::Clear, &sky_at(12.0), &big, None);
        assert!(b.intensity <= 1.1 * (170.0 + 230.0) * 1.25 + 1.0);
    }

    #[test]
    fn intensity_is_continuous_through_the_day() {
        let mut prev = evaluate(EnvMap::Clear, &sky_at(0.0), &Weather::default(), None).intensity;
        let mut h = 0.0;
        while h < 24.0 {
            h += 0.05;
            let now = evaluate(EnvMap::Clear, &sky_at(h % 24.0), &Weather::default(), None).intensity;
            assert!((now - prev).abs() < 25.0, "jump at {h}h: {prev} -> {now}");
            prev = now;
        }
    }

    #[test]
    fn yaw_lines_the_baked_sun_up_with_the_game_sun() {
        // Whatever direction the game's sun is in, rotating the map by the
        // yaw moves the baked azimuth onto it.
        for map in [EnvMap::Overcast, EnvMap::Clear] {
            for h in [7.0f32, 9.0, 12.0, 15.0, 17.0] {
                let s = sky_at(h).sun_pos;
                let yaw = map_yaw(map, s);
                assert!(yaw > -std::f32::consts::PI - 1e-4 && yaw <= std::f32::consts::PI + 1e-4);
                let game = s[0].atan2(-s[2]);
                let moved = map.baked_sun_azimuth() - yaw; // Bevy: a -> a - yaw
                let d = (moved - game + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                assert!(d.abs() < 1e-4, "{map:?} at {h}h off by {d}");
            }
        }
        assert_eq!(map_yaw(EnvMap::Clear, [f32::NAN, 1.0, 0.0]), 0.0);
        assert_eq!(map_yaw(EnvMap::Clear, [0.0, 1.0, 0.0]), 0.0, "sun straight up has no azimuth");
    }

    #[test]
    fn asset_paths_point_at_the_baked_files() {
        for map in [EnvMap::Overcast, EnvMap::Clear] {
            assert!(map.diffuse_path().ends_with("_diffuse.ktx2"));
            assert!(map.specular_path().ends_with("_specular.ktx2"));
            assert!(map.diffuse_path().starts_with("environment/"));
        }
        assert_ne!(EnvMap::Clear.specular_path(), EnvMap::Overcast.specular_path());
    }

    #[test]
    fn swapping_maps_fades_out_swaps_at_zero_and_fades_back_in() {
        let mut f = EnvFade::new(EnvMap::Clear);
        assert_eq!(f.step(0.0, EnvMap::Clear), 1.0);
        let mut min = 1.0f32;
        let mut swapped_at_zero = false;
        for _ in 0..200 {
            let before = f.shown;
            let lvl = f.step(0.016, EnvMap::Overcast);
            min = min.min(lvl);
            if before != f.shown {
                swapped_at_zero = lvl == 0.0;
            }
        }
        assert!(swapped_at_zero, "the map only changes while it is invisible");
        assert_eq!(min, 0.0);
        assert_eq!(f.shown, EnvMap::Overcast);
        assert_eq!(f.level, 1.0);
    }

    #[test]
    fn fade_ignores_bad_time_steps_and_can_be_reversed_midway() {
        let mut f = EnvFade::new(EnvMap::Clear);
        assert_eq!(f.step(f32::NAN, EnvMap::Overcast), 1.0);
        assert_eq!(f.step(-3.0, EnvMap::Overcast), 1.0);
        f.step(0.3, EnvMap::Overcast);
        assert!(f.level < 1.0 && f.shown == EnvMap::Clear);
        // Weather flips back before the swap: no swap happens, it just recovers.
        f.step(0.3, EnvMap::Clear);
        assert_eq!(f.shown, EnvMap::Clear);
        let big = f.step(1000.0, EnvMap::Clear);
        assert_eq!(big, 1.0);
        // A huge dt (e.g. after sleeping) completes the swap but never overshoots.
        let lvl = f.step(1000.0, EnvMap::Overcast);
        assert_eq!((f.shown, lvl), (EnvMap::Overcast, 0.0));
    }

    #[test]
    fn every_room_in_the_game_is_dim_and_neutral_for_the_sky() {
        for room in crate::sim::interiors::ALL {
            let e = evaluate(EnvMap::Clear, &sky_at(12.0), &Weather::default(), Some(room));
            assert!(e.intensity >= 0.0 && e.intensity < room.ambient().1 * 0.1, "{room:?}");
            assert_eq!(e.ambient_keep, INDOOR_AMBIENT_KEEP);
        }
    }

    #[test]
    fn evaluate_is_deterministic_and_the_chosen_map_matches_choose_map() {
        for h in [0.0, 6.5, 12.0, 18.0] {
            for o in [0.0, 0.5, 1.0] {
                let w = Weather { overcast: o, ..Weather::default() };
                let a = evaluate(EnvMap::Overcast, &sky_at(h), &w, None);
                assert_eq!(a, evaluate(EnvMap::Overcast, &sky_at(h), &w, None));
                assert_eq!(a.map, choose_map(EnvMap::Overcast, &sky_at(h), &w));
            }
        }
    }
}
