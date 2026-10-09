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
    /// Every take that plays in variant slot `slot` of `slots`: take `t` goes
    /// in slot `t % slots`, so extra takes share slots and none go unused.
    /// With fewer takes than slots, a slot borrows a take round-robin. With no
    /// muffled recordings a muffled slot is empty, so the synthesised muffled
    /// clip plays (a clear recording would sound wrong through a wall).
    pub fn for_slot(&self, slot: usize, slots: usize, muffled: bool) -> Vec<T> {
        let list = if muffled { &self.muffled } else { &self.plain };
        if list.is_empty() || slots == 0 {
            return Vec::new();
        }
        let mine: Vec<T> = list.iter().enumerate().filter(|(t, _)| t % slots == slot).map(|(_, x)| x.clone()).collect();
        if mine.is_empty() {
            vec![list[slot % list.len()].clone()]
        } else {
            mine
        }
    }
}

/// Can the game's decoder play these bytes? Only Ogg Vorbis and RIFF WAV are
/// accepted: Bevy panics on the first play of anything it can't decode (an
/// Opus or MP3 file renamed `.ogg`, a broken download), so those fall back to
/// the synthesised clip instead.
pub fn playable(bytes: &[u8]) -> bool {
    let vorbis = bytes.starts_with(b"OggS") && bytes.len() > 64 && bytes[..64].windows(7).any(|w| w == b"\x01vorbis");
    let wav = bytes.len() > 44 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE";
    vorbis || wav
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
    fn takes_borrow_round_robin_and_muffled_needs_its_own() {
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
        assert_eq!(caw.for_slot(0, 4, false), vec!["c0"]);
        assert_eq!(caw.for_slot(3, 4, false), vec!["c1"]);
        assert!(caw.for_slot(2, 4, true).is_empty());
        let howl = &got.iter().find(|(s, _)| *s == Sound::HowlFar).unwrap().1;
        assert_eq!(howl.for_slot(1, 3, true), vec!["h0m"]);
    }

    #[test]
    fn only_vorbis_and_wav_are_playable() {
        let mut ogg = b"OggS".to_vec();
        ogg.extend_from_slice(&[0; 24]);
        ogg.extend_from_slice(b"\x01vorbis");
        ogg.resize(200, 0);
        assert!(playable(&ogg));
        let mut opus = b"OggS".to_vec();
        opus.extend_from_slice(&[0; 24]);
        opus.extend_from_slice(b"OpusHead");
        opus.resize(200, 0);
        assert!(!playable(&opus));
        assert!(playable(&Sound::Caw.wav(0)));
        assert!(!playable(b"ID3\x03 an mp3"));
        assert!(!playable(b""));
        assert!(!playable(b"OggS"));
    }

    #[test]
    fn every_take_lands_in_some_slot() {
        let t = Takes { plain: (0..11).collect::<Vec<_>>(), muffled: vec![] };
        let mut all: Vec<i32> = (0..6).flat_map(|s| t.for_slot(s, 6, false)).collect();
        all.sort();
        assert_eq!(all, (0..11).collect::<Vec<_>>());
        assert_eq!(t.for_slot(0, 6, false), vec![0, 6]);
        // Fewer takes than slots: every slot still gets one.
        let few = Takes { plain: vec!["a", "b"], muffled: vec![] };
        assert_eq!(few.for_slot(3, 4, false), vec!["b"]);
        assert!(few.for_slot(2, 4, true).is_empty());
        assert!(Takes::<i32>::default().for_slot(0, 4, false).is_empty());
    }

    // ---- audit ----

    #[test]
    fn odd_file_names_never_confuse_the_parser() {
        for bad in ["", ".ogg", "_0.ogg", "caw_.ogg", "caw_-1.ogg", "caw_1.5.ogg", "caw_muffled.ogg", "caw_1_muffled_muffled.ogg", "Caw_0.ogg", "caw_0.OGG", "caw_0.ogg.bak", "caw_0", "dir/caw_0.ogg"] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        // Leading zeros are the same take; long names resolve to the longest sound.
        assert_eq!(parse("caw_007.ogg"), Some((Sound::Caw, 7, false)));
        assert_eq!(parse("step_snow_run_0.wav"), Some((Sound::StepSnowRun, 0, false)));
        assert_eq!(parse("step_snow_0.wav"), Some((Sound::StepSnow, 0, false)));
    }

    #[test]
    fn gather_is_independent_of_listing_order_and_keeps_duplicates() {
        let a = gather(vec![("caw_2.ogg".to_string(), 2), ("caw_0_muffled.ogg".to_string(), 10), ("caw_0.ogg".to_string(), 0), ("caw_1.ogg".to_string(), 1)]);
        let b = gather(vec![("caw_0.ogg".to_string(), 0), ("caw_1.ogg".to_string(), 1), ("caw_2.ogg".to_string(), 2), ("caw_0_muffled.ogg".to_string(), 10)]);
        assert_eq!(a, b);
        assert_eq!(a[0].1.plain, vec![0, 1, 2]);
        assert_eq!(a[0].1.muffled, vec![10]);
        // A .wav and an .ogg of the same take both survive; neither is dropped silently.
        let dup = gather(vec![("caw_0.ogg".to_string(), 'a'), ("caw_0.wav".to_string(), 'b')]);
        assert_eq!(dup[0].1.plain.len(), 2);
        assert!(gather(Vec::<(String, u8)>::new()).is_empty());
        assert!(gather(vec![("notes.txt".to_string(), 1)]).is_empty());
    }

    #[test]
    fn gather_output_follows_the_sound_list_order() {
        let got = gather(vec![("howl_far_0.ogg".to_string(), 1), ("caw_0.ogg".to_string(), 2), ("step_snow_0.ogg".to_string(), 3)]);
        let pos = |s: Sound| Sound::ALL.iter().position(|x| *x == s).unwrap();
        let order: Vec<usize> = got.iter().map(|(s, _)| pos(*s)).collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted);
    }

    #[test]
    fn for_slot_degenerate_arguments() {
        let t = Takes { plain: vec![1, 2, 3], muffled: vec![9] };
        assert!(t.for_slot(0, 0, false).is_empty(), "zero slots");
        // A slot number past the end still gets something to play (no panic, no modulo by zero).
        assert_eq!(t.for_slot(7, 4, false), vec![2]);
        assert_eq!(t.for_slot(100, 1, false), vec![2]);
        // One slot gets every take; plain and muffled lists never mix.
        assert_eq!(t.for_slot(0, 1, false), vec![1, 2, 3]);
        assert_eq!(t.for_slot(0, 1, true), vec![9]);
        assert!(Takes::<i32> { plain: vec![1], muffled: vec![] }.for_slot(0, 2, true).is_empty());
    }

    #[test]
    fn playable_rejects_lookalikes_and_truncated_files() {
        // RIFF but not WAVE (an AVI, say), and a WAV header cut short.
        let mut avi = b"RIFF\0\0\0\0AVI ".to_vec();
        avi.resize(100, 0);
        assert!(!playable(&avi));
        let mut wav = b"RIFF\0\0\0\0WAVE".to_vec();
        wav.resize(44, 0);
        assert!(!playable(&wav), "header only, no audio");
        wav.resize(45, 0);
        assert!(playable(&wav));
        // Vorbis marker must be in the first page; "OggS" with FLAC or nothing is out.
        let mut late = b"OggS".to_vec();
        late.resize(100, 0);
        late.extend_from_slice(b"\x01vorbis");
        assert!(!playable(&late));
        let mut flac = b"OggS".to_vec();
        flac.extend_from_slice(&[0; 24]);
        flac.extend_from_slice(b"\x7fFLAC");
        flac.resize(200, 0);
        assert!(!playable(&flac));
        assert!(!playable(&[0u8; 500]));
        assert!(!playable(b"RIFF"));
    }
}
