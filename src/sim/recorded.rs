//! Recorded sounds: clips in `assets/sounds/` named after the sound they
//! replace (`step_snow_0.ogg`, `step_snow_1.ogg`, `howl_far_0_muffled.ogg`)
//! take the place of the synthesised ones. Any sound without a recording,
//! or whose files are missing, stays synthesised.

use crate::sim::synth::Sound;

/// The file name stem for a sound: its name in snake case (`StepSnowRun`
/// is `step_snow_run`).
pub fn stem(sound: Sound) -> String {
    let name = format!("{sound:?}");
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// What a file in the sounds folder is: the sound, which take, and whether
/// it's the muffled (heard through a wall) copy. `None` for anything else.
pub fn parse(file_name: &str) -> Option<(Sound, usize, bool)> {
    let base = file_name.strip_suffix(".ogg").or_else(|| file_name.strip_suffix(".wav"))?;
    let (base, muffled) = match base.strip_suffix("_muffled") {
        Some(b) => (b, true),
        None => (base, false),
    };
    let (name, take) = base.rsplit_once('_')?;
    let take: usize = take.parse().ok()?;
    let sound = Sound::ALL.iter().copied().find(|s| stem(*s) == name)?;
    Some((sound, take, muffled))
}

/// The recordings found for one sound: plain and muffled takes, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Takes<T> {
    pub plain: Vec<T>,
    pub muffled: Vec<T>,
}

impl<T: Clone> Takes<T> {
    /// The take that stands in for synthesised variant `variant` (the takes
    /// are cycled when there are fewer of them than variants). A muffled copy
    /// falls back to the plain take when there's no muffled recording.
    pub fn pick(&self, variant: usize, muffled: bool) -> Option<T> {
        let list = if muffled && !self.muffled.is_empty() { &self.muffled } else { &self.plain };
        if list.is_empty() {
            return None;
        }
        Some(list[variant % list.len()].clone())
    }
}

/// Sort a folder listing into takes per sound. Takes are ordered by number;
/// gaps are allowed.
pub fn gather<T: Clone>(files: impl IntoIterator<Item = (String, T)>) -> Vec<(Sound, Takes<T>)> {
    let mut found: Vec<(Sound, usize, bool, T)> = files.into_iter().filter_map(|(name, t)| parse(&name).map(|(s, n, m)| (s, n, m, t))).collect();
    found.sort_by_key(|(s, n, m, _)| (Sound::ALL.iter().position(|x| x == s), *m, *n));
    let mut out: Vec<(Sound, Takes<T>)> = Vec::new();
    for (sound, _, muffled, t) in found {
        if out.last().is_none_or(|(s, _)| *s != sound) {
            out.push((sound, Takes { plain: Vec::new(), muffled: Vec::new() }));
        }
        let takes = &mut out.last_mut().expect("just pushed").1;
        if muffled {
            takes.muffled.push(t);
        } else {
            takes.plain.push(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_are_snake_case() {
        assert_eq!(stem(Sound::StepSnow), "step_snow");
        assert_eq!(stem(Sound::StepSnowRun), "step_snow_run");
        assert_eq!(stem(Sound::Caw), "caw");
        assert_eq!(stem(Sound::UiTab), "ui_tab");
    }

    #[test]
    fn every_stem_is_unique_and_parses_back() {
        for &s in Sound::ALL {
            assert_eq!(parse(&format!("{}_3.ogg", stem(s))), Some((s, 3, false)), "{s:?}");
            assert_eq!(parse(&format!("{}_0_muffled.wav", stem(s))), Some((s, 0, true)), "{s:?}");
        }
    }

    #[test]
    fn other_files_are_ignored() {
        assert_eq!(parse("README.md"), None);
        assert_eq!(parse("step_snow.ogg"), None);
        assert_eq!(parse("step_snow_x.ogg"), None);
        assert_eq!(parse("not_a_sound_1.ogg"), None);
        assert_eq!(parse("step_snow_1.mp3"), None);
    }

    #[test]
    fn takes_cycle_and_muffled_falls_back() {
        let got = gather(vec![
            ("caw_1.ogg".to_string(), "c1"),
            ("caw_0.ogg".to_string(), "c0"),
            ("howl_far_0.ogg".to_string(), "h0"),
            ("howl_far_0_muffled.ogg".to_string(), "h0m"),
            ("readme.txt".to_string(), "x"),
        ]);
        assert_eq!(got.len(), 2);
        let caw = &got.iter().find(|(s, _)| *s == Sound::Caw).unwrap().1;
        assert_eq!(caw.plain, vec!["c0", "c1"]);
        assert_eq!(caw.pick(0, false), Some("c0"));
        assert_eq!(caw.pick(3, false), Some("c1"));
        assert_eq!(caw.pick(2, true), Some("c0"));
        let howl = &got.iter().find(|(s, _)| *s == Sound::HowlFar).unwrap().1;
        assert_eq!(howl.pick(5, true), Some("h0m"));
        assert_eq!(Takes::<&str>::default().pick(0, false), None);
    }
}
