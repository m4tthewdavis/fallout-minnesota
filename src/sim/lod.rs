//! Level of detail: what to stop drawing at a distance. Fine detail on
//! people (fur tufts, frost, faces) goes first; small props go at a distance
//! that follows the view-distance setting. A little hysteresis stops things
//! flickering in and out at the edge as you walk.

use super::settings::ViewDistance;

/// Fraction past the cut-off before a shown thing hides (and inside it
/// before a hidden thing shows).
const HYSTERESIS: f32 = 0.08;

/// Should something at `dist` with a cut-off of `cutoff` be drawn, given
/// whether it is drawn now?
pub fn keep_visible(dist: f32, cutoff: f32, shown: bool) -> bool {
    if shown {
        dist < cutoff * (1.0 + HYSTERESIS)
    } else {
        dist < cutoff * (1.0 - HYSTERESIS)
    }
}

/// Small props (cans, buckets, crates, tyres) stop being drawn beyond this.
pub fn prop_cutoff(view: ViewDistance) -> f32 {
    match view {
        ViewDistance::Near => 110.0,
        ViewDistance::Medium => 170.0,
        ViewDistance::Far => 240.0,
    }
}

/// Fine detail on people.
pub const PERSON_DETAIL: f32 = 32.0;
/// Whole crows (small, against the sky).
pub const CROW: f32 = 160.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_things_show_far_things_hide() {
        assert!(keep_visible(10.0, 50.0, false));
        assert!(!keep_visible(80.0, 50.0, true));
    }

    #[test]
    fn the_edge_does_not_flicker() {
        // Just past the cut-off: something already shown stays shown...
        assert!(keep_visible(52.0, 50.0, true));
        // ...and something hidden stays hidden until it's well inside.
        assert!(!keep_visible(48.0, 50.0, false));
        assert!(keep_visible(45.0, 50.0, false));
    }

    #[test]
    fn view_distance_moves_the_prop_cutoff() {
        assert!(prop_cutoff(ViewDistance::Near) < prop_cutoff(ViewDistance::Medium));
        assert!(prop_cutoff(ViewDistance::Medium) < prop_cutoff(ViewDistance::Far));
        // Never past the camera's far plane.
        for v in ViewDistance::ALL {
            assert!(prop_cutoff(v) < v.far_plane());
        }
        assert!(PERSON_DETAIL < prop_cutoff(ViewDistance::Near));
    }
}
