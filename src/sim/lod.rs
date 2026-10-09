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

/// Big things (boulders and rocks a metre or more tall) go when the trees do,
/// so a boulder never outlives the forest round it; on Far they always show.
pub fn large_cutoff(view: ViewDistance) -> f32 {
    view.tree_cull().unwrap_or(f32::INFINITY)
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

    #[test]
    fn big_rocks_outlast_small_props_and_always_show_on_far() {
        for v in ViewDistance::ALL {
            assert!(large_cutoff(v) >= prop_cutoff(v));
        }
        let far = large_cutoff(ViewDistance::Far);
        assert!(keep_visible(5000.0, far, true));
        assert!(keep_visible(5000.0, far, false));
    }

    // ---- audit ----

    #[test]
    fn large_cutoff_is_the_tree_cull_distance_and_infinite_only_on_far() {
        assert_eq!(large_cutoff(ViewDistance::Near), ViewDistance::Near.tree_cull().unwrap());
        assert_eq!(large_cutoff(ViewDistance::Medium), ViewDistance::Medium.tree_cull().unwrap());
        assert!(large_cutoff(ViewDistance::Far).is_infinite());
        assert!(large_cutoff(ViewDistance::Near) < large_cutoff(ViewDistance::Medium));
        assert!(large_cutoff(ViewDistance::Medium) < large_cutoff(ViewDistance::Far));
        for v in ViewDistance::ALL {
            assert!(large_cutoff(v) > 0.0 && !large_cutoff(v).is_nan());
        }
    }

    #[test]
    fn a_boulder_is_never_drawn_past_where_the_trees_stop() {
        for v in [ViewDistance::Near, ViewDistance::Medium] {
            let trees = v.tree_cull().unwrap();
            let c = large_cutoff(v);
            assert!(!keep_visible(trees * 1.2, c, true), "{v:?}: shown boulder hides soon after the trees");
            assert!(keep_visible(trees * 0.5, c, false));
        }
    }

    #[test]
    fn hysteresis_never_contradicts_itself() {
        // For any distance: if a hidden thing would show, a shown thing keeps showing.
        let mut d = 0.0f32;
        while d < 120.0 {
            if keep_visible(d, 50.0, false) {
                assert!(keep_visible(d, 50.0, true), "d={d}");
            }
            d += 0.25;
        }
        // And the band is a real band, not a point.
        assert!(keep_visible(52.0, 50.0, true) && !keep_visible(52.0, 50.0, false));
    }

    #[test]
    fn garbage_distances_and_cutoffs_hide_rather_than_panic() {
        for shown in [true, false] {
            assert!(!keep_visible(f32::NAN, 50.0, shown), "NaN distance hides");
            assert!(!keep_visible(10.0, f32::NAN, shown));
            assert!(!keep_visible(10.0, 0.0, shown), "zero cut-off shows nothing");
            assert!(keep_visible(-5.0, 50.0, shown), "behind the origin is still near");
            assert!(keep_visible(1e30, f32::INFINITY, shown));
        }
    }
}
