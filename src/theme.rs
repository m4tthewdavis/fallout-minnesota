//! The look of every screen: HUD, Pip-Boy, pause menu, dialogue, loot and XP
//! panels. One cold winter palette, defined once. Ice blue is the ink
//! (text, bars, borders), cyan marks what's selected or live, frost white is
//! for emphasis and warnings, and the panels are deep sub-zero navy.

use bevy::prelude::*;

// ---- The five colours ----

/// Ice blue `#64B5F6`: the main ink.
pub const ICE: Color = Color::srgb(0.392, 0.710, 0.965);
/// Cyan `#00E5FF`: selection, active lamps, phosphor glow.
pub const CYAN: Color = Color::srgb(0.0, 0.898, 1.0);
/// Frost white `#E0F7FA`: emphasis, values, warnings.
pub const FROST: Color = Color::srgb(0.878, 0.969, 0.980);
/// Deep navy `#0A192F`: screens and the darkest panels.
pub const NAVY: Color = Color::srgb(0.039, 0.098, 0.184);

// ---- Roles ----

/// Text, bars, borders: ice blue.
pub const ACCENT: Color = ICE;
pub const ACCENT_DIM: Color = Color::srgba(0.392, 0.710, 0.965, 0.42);
pub const ACCENT_FAINT: Color = Color::srgba(0.392, 0.710, 0.965, 0.15);
pub const ACCENT_OFF: Color = Color::srgba(0.392, 0.710, 0.965, 0.28);
/// The row or page you're on: cyan, with a faint wash behind it.
pub const SELECTED: Color = CYAN;
pub const SELECTED_FILL: Color = Color::srgba(0.0, 0.898, 1.0, 0.16);
/// Something to notice (a warning, a jammed gun): frost white.
pub const WARN: Color = FROST;
/// Genuine danger keeps a red, so it can't be missed in all that blue.
pub const DANGER: Color = Color::srgb(1.0, 0.32, 0.32);

/// HUD plates sit over the world, so they're see-through.
pub const HUD_PANEL: Color = Color::srgba(0.039, 0.098, 0.184, 0.62);
/// Pause menu and dialogue boxes: lighter navy `#102A43`.
pub const MENU_PANEL: Color = Color::srgba(0.063, 0.165, 0.263, 0.96);
/// The Pip-Boy's screen.
pub const SCREEN_BG: Color = NAVY;
/// A map panel's dark ground.
pub const SCREEN_WELL: Color = Color::srgb(0.02, 0.05, 0.09);
/// An unlit lamp on the Pip-Boy case.
pub const LAMP_OFF: Color = Color::srgba(0.05, 0.1, 0.16, 0.85);
/// CRT static and the glow of its power-on beam.
pub const STATIC_TINT: Color = Color::srgb(0.7, 0.9, 1.0);
pub const BEAM: Color = FROST;
/// The Pip-Boy's key hints, under the screen.
pub const HINT: Color = Color::srgba(0.878, 0.969, 0.980, 0.75);

/// Paint a grey-scale value (0..=255) in the palette's ice ramp: black at
/// the bottom, ice blue in the middle, frost white at the top.
pub fn ice_ramp(l: f32) -> [u8; 3] {
    let t = (l / 255.0).clamp(0.0, 1.0);
    let ice = [0.392, 0.710, 0.965];
    let frost = [0.878, 0.969, 0.980];
    let c = if t < 0.7 {
        let k = t / 0.7;
        [ice[0] * k, ice[1] * k, ice[2] * k]
    } else {
        let k = (t - 0.7) / 0.3;
        [ice[0] + (frost[0] - ice[0]) * k, ice[1] + (frost[1] - ice[1]) * k, ice[2] + (frost[2] - ice[2]) * k]
    };
    [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Color) -> f32 {
        let l = c.to_linear();
        0.2126 * l.red + 0.7152 * l.green + 0.0722 * l.blue
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn the_palette_matches_the_brief() {
        let hex = |c: Color| {
            let s = c.to_srgba();
            format!("{:02X}{:02X}{:02X}", (s.red * 255.0).round() as u8, (s.green * 255.0).round() as u8, (s.blue * 255.0).round() as u8)
        };
        assert_eq!(hex(ICE), "64B5F6");
        assert_eq!(hex(CYAN), "00E5FF");
        assert_eq!(hex(FROST), "E0F7FA");
        assert_eq!(hex(NAVY), "0A192F");
        assert_eq!(hex(MENU_PANEL), "102A43");
    }

    #[test]
    fn text_is_readable_on_every_panel() {
        for panel in [NAVY, MENU_PANEL.with_alpha(1.0), SCREEN_WELL] {
            for ink in [ICE, CYAN, FROST] {
                assert!(contrast(ink, panel) >= 4.5, "{ink:?} on {panel:?}: {}", contrast(ink, panel));
            }
        }
        assert!(contrast(DANGER, NAVY) >= 4.5, "danger red must show on navy");
    }

    #[test]
    fn danger_stands_out_from_the_blue() {
        let d = DANGER.to_srgba();
        let i = ICE.to_srgba();
        assert!(d.red > d.blue + 0.4, "red");
        assert!(i.blue > i.red + 0.4, "blue");
    }

    #[test]
    fn the_ice_ramp_climbs_from_black_through_ice_to_frost() {
        assert_eq!(ice_ramp(0.0), [0, 0, 0]);
        let mid = ice_ramp(255.0 * 0.7);
        assert!(mid[2] > mid[1] && mid[1] > mid[0], "blue-weighted at ice: {mid:?}");
        let top = ice_ramp(255.0);
        assert!(top[0] > 200 && top[1] > 240 && top[2] > 240, "frost white at the top: {top:?}");
        let mut last = 0u32;
        for l in 0..=255 {
            let c = ice_ramp(l as f32);
            let sum = c[0] as u32 + c[1] as u32 + c[2] as u32;
            assert!(sum + 1 >= last, "brightness never falls ({l})");
            last = sum;
        }
    }
}
