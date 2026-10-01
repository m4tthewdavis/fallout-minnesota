//! Frostfang pack behaviour.
//!
//! Design doc: "Hunt only during rad-blizzards in packs of 4-8. Nearly
//! invisible in whiteouts."

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WolfMode {
    Wander,
    /// Shadowing the player from a distance during a blizzard.
    Stalk,
    Chase,
    Flee,
}

pub const CALM_AGGRO_RANGE: f32 = 12.0;
pub const HUNT_AGGRO_RANGE: f32 = 45.0;
pub const STALK_RANGE: f32 = 75.0;
pub const BITE_RANGE: f32 = 1.9;

pub const WANDER_SPEED: f32 = 2.5;
pub const STALK_SPEED: f32 = 4.5;
pub const CHASE_SPEED: f32 = 8.5;
pub const FLEE_SPEED: f32 = 8.0;

pub fn decide(dist_to_player: f32, hunting: bool, health_frac: f32) -> WolfMode {
    // Wounded wolves bolt - unless the blizzard has made them bold.
    if health_frac < 0.25 && !hunting {
        return WolfMode::Flee;
    }
    let aggro = if hunting { HUNT_AGGRO_RANGE } else { CALM_AGGRO_RANGE };
    if dist_to_player < aggro {
        WolfMode::Chase
    } else if hunting && dist_to_player < STALK_RANGE {
        WolfMode::Stalk
    } else {
        WolfMode::Wander
    }
}

pub fn speed(mode: WolfMode) -> f32 {
    match mode {
        WolfMode::Wander => WANDER_SPEED,
        WolfMode::Stalk => STALK_SPEED,
        WolfMode::Chase => CHASE_SPEED,
        WolfMode::Flee => FLEE_SPEED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blizzards_make_wolves_dangerous() {
        assert_eq!(decide(30.0, false, 1.0), WolfMode::Wander);
        assert_eq!(decide(30.0, true, 1.0), WolfMode::Chase);
        assert_eq!(decide(60.0, true, 1.0), WolfMode::Stalk);
        assert_eq!(decide(5.0, false, 1.0), WolfMode::Chase);
    }

    #[test]
    fn wounded_wolves_flee_in_calm_only() {
        assert_eq!(decide(5.0, false, 0.1), WolfMode::Flee);
        assert_eq!(decide(5.0, true, 0.1), WolfMode::Chase);
    }
}
