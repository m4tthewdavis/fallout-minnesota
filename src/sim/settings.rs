//! The player's settings: graphics quality, view distance, UI size and
//! volumes. Pure data with the numbers each choice means, plus JSON
//! load/save that tolerates missing, extra or garbled fields so an old or
//! hand-edited file never stops the game starting.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    High,
}

impl ShadowQuality {
    pub const ALL: [ShadowQuality; 4] = [ShadowQuality::Off, ShadowQuality::Low, ShadowQuality::Medium, ShadowQuality::High];

    pub fn label(self) -> &'static str {
        match self {
            ShadowQuality::Off => "Off",
            ShadowQuality::Low => "Low",
            ShadowQuality::Medium => "Medium",
            ShadowQuality::High => "High",
        }
    }

    /// Does the sun cast shadows at all?
    pub fn sun_shadows(self) -> bool {
        self != ShadowQuality::Off
    }

    /// How many shadow cascades the sun uses (more = sharper near the player).
    pub fn cascades(self) -> usize {
        match self {
            ShadowQuality::Off | ShadowQuality::Low => 1,
            ShadowQuality::Medium => 2,
            ShadowQuality::High => 3,
        }
    }

    /// How far from the camera (metres) shadows are drawn.
    pub fn shadow_distance(self) -> f32 {
        match self {
            ShadowQuality::Off => 0.0,
            ShadowQuality::Low => 40.0,
            ShadowQuality::Medium => 90.0,
            ShadowQuality::High => 140.0,
        }
    }

    /// Resolution of each shadow map.
    pub fn map_size(self) -> usize {
        match self {
            ShadowQuality::Off | ShadowQuality::Low => 1024,
            _ => 2048,
        }
    }

    /// Trees farther than this (metres) stop casting shadows.
    pub fn tree_shadow_distance(self) -> f32 {
        match self {
            ShadowQuality::Off => 0.0,
            ShadowQuality::Low => 30.0,
            ShadowQuality::Medium => 45.0,
            ShadowQuality::High => 60.0,
        }
    }

    /// Do the fire lights cast shadows too?
    pub fn point_light_shadows(self) -> bool {
        matches!(self, ShadowQuality::Medium | ShadowQuality::High)
    }

    /// One step up or down the list (`dir` of +1 / -1), stopping at the ends.
    pub fn step(self, dir: i32) -> ShadowQuality {
        let i = Self::ALL.iter().position(|q| *q == self).unwrap_or(3) as i32;
        Self::ALL[(i + dir).clamp(0, 3) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ViewDistance {
    Near,
    Medium,
    Far,
}

impl ViewDistance {
    pub const ALL: [ViewDistance; 3] = [ViewDistance::Near, ViewDistance::Medium, ViewDistance::Far];

    pub fn label(self) -> &'static str {
        match self {
            ViewDistance::Near => "Near",
            ViewDistance::Medium => "Medium",
            ViewDistance::Far => "Far",
        }
    }

    /// The camera's far clipping plane, metres.
    pub fn far_plane(self) -> f32 {
        match self {
            ViewDistance::Near => 200.0,
            ViewDistance::Medium => 350.0,
            ViewDistance::Far => 1000.0,
        }
    }

    /// Trees beyond this distance are not drawn at all (`None` = always drawn).
    pub fn tree_cull(self) -> Option<f32> {
        match self {
            ViewDistance::Near => Some(140.0),
            ViewDistance::Medium => Some(220.0),
            ViewDistance::Far => None,
        }
    }

    pub fn step(self, dir: i32) -> ViewDistance {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(2) as i32;
        Self::ALL[(i + dir).clamp(0, 2) as usize]
    }
}

pub const UI_SCALE_MIN: f32 = 0.6;
pub const UI_SCALE_MAX: f32 = 1.6;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub shadows: ShadowQuality,
    pub view: ViewDistance,
    /// Multiplier on the automatic UI size (1 = as designed for the window).
    pub ui_scale: f32,
    pub master: f32,
    pub sfx: f32,
    pub music: f32,
    pub ambience: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shadows: ShadowQuality::High,
            view: ViewDistance::Far,
            ui_scale: 1.0,
            master: 0.8,
            sfx: 1.0,
            music: 0.6,
            ambience: 0.8,
        }
    }
}

/// Round to the nearest tenth, so repeated nudges never drift (0.1 + 0.2 != 0.3).
fn snap(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

impl Settings {
    /// Pull every value into its legal range, replacing nonsense (NaN, huge
    /// numbers) with the default.
    pub fn sanitized(mut self) -> Settings {
        let d = Settings::default();
        let fix = |v: f32, lo: f32, hi: f32, def: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        self.ui_scale = snap(fix(self.ui_scale, UI_SCALE_MIN, UI_SCALE_MAX, d.ui_scale));
        self.master = snap(fix(self.master, 0.0, 1.0, d.master));
        self.sfx = snap(fix(self.sfx, 0.0, 1.0, d.sfx));
        self.music = snap(fix(self.music, 0.0, 1.0, d.music));
        self.ambience = snap(fix(self.ambience, 0.0, 1.0, d.ambience));
        self
    }

    /// Nudge the UI scale by `dir` tenths, within its range.
    pub fn nudge_ui_scale(&mut self, dir: i32) {
        self.ui_scale = snap((self.ui_scale + dir as f32 * 0.1).clamp(UI_SCALE_MIN, UI_SCALE_MAX));
    }

    pub fn to_json(self) -> String {
        serde_json::to_string_pretty(&self).unwrap_or_default()
    }

    /// Read settings from JSON. Missing fields keep their defaults; text that
    /// isn't valid JSON gives `None`.
    pub fn from_json(text: &str) -> Option<Settings> {
        serde_json::from_str::<Settings>(text).ok().map(Settings::sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_shadow_quality_means_more_of_everything() {
        let q = ShadowQuality::ALL;
        for pair in q.windows(2) {
            assert!(pair[1].shadow_distance() >= pair[0].shadow_distance());
            assert!(pair[1].cascades() >= pair[0].cascades());
            assert!(pair[1].tree_shadow_distance() >= pair[0].tree_shadow_distance());
            assert!(pair[1].map_size() >= pair[0].map_size());
        }
        assert!(!ShadowQuality::Off.sun_shadows() && ShadowQuality::Low.sun_shadows());
        assert_eq!(ShadowQuality::Off.tree_shadow_distance(), 0.0);
        assert!(!ShadowQuality::Low.point_light_shadows() && ShadowQuality::Medium.point_light_shadows());
        // The top setting is what the game shipped with before this menu existed.
        assert_eq!((ShadowQuality::High.cascades(), ShadowQuality::High.shadow_distance()), (3, 140.0));
        assert_eq!(ShadowQuality::High.tree_shadow_distance(), 60.0);
    }

    #[test]
    fn view_distance_trades_range_for_speed() {
        assert!(ViewDistance::Near.far_plane() < ViewDistance::Medium.far_plane());
        assert!(ViewDistance::Medium.far_plane() < ViewDistance::Far.far_plane());
        assert_eq!(ViewDistance::Far.tree_cull(), None, "far draws every tree");
        let (n, m) = (ViewDistance::Near.tree_cull().unwrap(), ViewDistance::Medium.tree_cull().unwrap());
        assert!(n < m);
        assert!(n > 100.0, "even Near keeps the nearby woods");
    }

    #[test]
    fn stepping_stops_at_the_ends() {
        assert_eq!(ShadowQuality::Off.step(-1), ShadowQuality::Off);
        assert_eq!(ShadowQuality::Off.step(1), ShadowQuality::Low);
        assert_eq!(ShadowQuality::High.step(1), ShadowQuality::High);
        assert_eq!(ShadowQuality::High.step(-2), ShadowQuality::Low);
        assert_eq!(ViewDistance::Near.step(-1), ViewDistance::Near);
        assert_eq!(ViewDistance::Far.step(1), ViewDistance::Far);
        assert_eq!(ViewDistance::Far.step(-1), ViewDistance::Medium);
    }

    #[test]
    fn ui_scale_nudges_in_clean_tenths_within_range() {
        let mut s = Settings::default();
        for _ in 0..3 {
            s.nudge_ui_scale(1);
        }
        assert_eq!(s.ui_scale, 1.3, "no float drift after repeated nudges");
        for _ in 0..20 {
            s.nudge_ui_scale(1);
        }
        assert_eq!(s.ui_scale, UI_SCALE_MAX);
        for _ in 0..30 {
            s.nudge_ui_scale(-1);
        }
        assert_eq!(s.ui_scale, UI_SCALE_MIN);
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let s = Settings { shadows: ShadowQuality::Low, view: ViewDistance::Medium, ui_scale: 1.2, master: 0.5, sfx: 0.3, music: 0.0, ambience: 1.0 };
        assert_eq!(Settings::from_json(&s.to_json()), Some(s));
    }

    #[test]
    fn missing_fields_keep_defaults_and_unknown_ones_are_ignored() {
        let s = Settings::from_json(r#"{ "shadows": "Medium", "something_new": 7 }"#).unwrap();
        assert_eq!(s.shadows, ShadowQuality::Medium);
        assert_eq!(s.view, Settings::default().view);
        assert_eq!(s.master, Settings::default().master);
        assert_eq!(Settings::from_json("{}"), Some(Settings::default()));
    }

    #[test]
    fn garbage_never_panics_and_out_of_range_values_are_pulled_in() {
        assert_eq!(Settings::from_json("not json at all"), None);
        assert_eq!(Settings::from_json(""), None);
        assert_eq!(Settings::from_json(r#"{"shadows": "Ultra"}"#), None, "unknown quality name");
        let s = Settings::from_json(r#"{ "master": 7.5, "ui_scale": -3.0, "music": 0.34 }"#).unwrap();
        assert_eq!((s.master, s.ui_scale, s.music), (1.0, UI_SCALE_MIN, 0.3));
        let nan = Settings { master: f32::NAN, ui_scale: f32::INFINITY, ..Settings::default() }.sanitized();
        assert_eq!(nan.master, Settings::default().master);
        assert!(nan.ui_scale.is_finite() && nan.ui_scale <= UI_SCALE_MAX);
    }
}
