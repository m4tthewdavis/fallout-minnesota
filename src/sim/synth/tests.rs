use super::*;

fn peak(s: &[f32]) -> f32 {
    s.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
}

#[test]
fn wav_header_is_valid() {
    let wav = encode_wav(&[0.0, 0.5, -0.5]);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(wav.len(), 44 + 6);
    assert_eq!(u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]), 36 + 6);
}

#[test]
fn the_list_covers_every_sound_once() {
    for (i, s) in Sound::ALL.iter().enumerate() {
        assert_eq!(*s as usize, i, "{s:?} is out of order in Sound::ALL");
    }
    assert_eq!(Sound::UiTab as usize + 1, Sound::ALL.len());
}

#[test]
fn every_variant_is_audible_finite_and_never_clips() {
    for &sound in Sound::ALL {
        for v in 0..sound.variants() {
            let s = sound.samples(v);
            assert!(s.len() > 500, "{sound:?}/{v} too short");
            assert!(s.iter().all(|x| x.is_finite()), "{sound:?}/{v} has NaN");
            let p = peak(&s);
            assert!(p > 0.02, "{sound:?}/{v} is silent (peak {p})");
            assert!(p <= 1.0 + 1e-3, "{sound:?}/{v} clips (peak {p})");
        }
    }
}

#[test]
fn muffled_sounds_lose_their_highs_but_not_their_weight() {
    // Energy above ~2 kHz, as a fraction of the whole (crudely: the high-passed remainder).
    fn brightness(s: &[f32]) -> f32 {
        let mut lp = LowPass::new(2000.0);
        let hi: f32 = s.iter().map(|&x| (x - lp.run(x)).powi(2)).sum();
        let all: f32 = s.iter().map(|x| x * x).sum::<f32>().max(1e-9);
        hi / all
    }
    for sound in [Sound::RifleShot, Sound::ShotgunShot, Sound::Snarl, Sound::RaiderShout, Sound::StepSnow] {
        let dry = sound.samples(0);
        let wet = sound.samples_muffled(0);
        assert_eq!(dry.len(), wet.len());
        assert!(brightness(&wet) < brightness(&dry) * 0.5, "{sound:?}: {} vs {}", brightness(&wet), brightness(&dry));
        let (pd, pw) = (peak(&dry), peak(&wet));
        assert!(pw > 0.2 * pd && pw <= 1.0 + 1e-3, "{sound:?} {pd} {pw}");
    }
    assert!(Sound::RifleShot.muffleable() && !Sound::UiTab.muffleable() && !Sound::WindLow.muffleable());
}

#[test]
fn indoor_tails_are_short_and_outdoor_tails_roll_on() {
    let indoor = Sound::ShotTailIndoor.samples(0);
    let outdoor = Sound::ShotTailOutdoor.samples(0);
    assert!(outdoor.len() > indoor.len() * 2, "{} vs {}", outdoor.len(), indoor.len());
    // When has it fallen below 10% of its peak and stayed there?
    let settle = |s: &[f32]| {
        let p = peak(s) * 0.1;
        s.iter().rposition(|v| v.abs() > p).unwrap_or(0) as f32 / SR
    };
    assert!(settle(&indoor) < 0.5, "indoor settles in {}", settle(&indoor));
    assert!(settle(&outdoor) > 0.8, "outdoor rolls on for {}", settle(&outdoor));
}

#[test]
fn guns_have_real_bass_under_the_crack() {
    // The energy below 120 Hz (a one-pole low-pass at 120 Hz) is a solid share of the shot.
    for sound in [Sound::RifleShot, Sound::ShotgunShot, Sound::RevolverShot] {
        let s = sound.samples(0);
        let mut lp = LowPass::new(120.0);
        let low: f32 = s.iter().map(|&x| lp.run(x).powi(2)).sum();
        let all: f32 = s.iter().map(|x| x * x).sum();
        assert!(low / all > 0.04, "{sound:?} bass share {}", low / all);
    }
    // The shotgun is the deepest.
    let low = |sound: Sound| {
        let s = sound.samples(0);
        let mut lp = LowPass::new(120.0);
        s.iter().map(|&x| lp.run(x).powi(2)).sum::<f32>() / s.iter().map(|x| x * x).sum::<f32>()
    };
    assert!(low(Sound::ShotgunShot) > low(Sound::RevolverShot));
}

#[test]
fn variants_really_differ() {
    for &sound in Sound::ALL {
        if sound.variants() < 2 {
            continue;
        }
        let a = sound.samples(0);
        let b = sound.samples(1);
        let same = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6);
        assert!(!same, "{sound:?} variants 0 and 1 are identical");
    }
}

#[test]
fn frequent_sounds_have_enough_variants() {
    for s in [Sound::StepSnow, Sound::StepIce, Sound::StepRoad, Sound::StepConcrete, Sound::StepWood] {
        assert!(s.variants() >= 6, "{s:?}");
    }
    for s in [Sound::RifleShot, Sound::Snarl, Sound::Yelp, Sound::HowlNear, Sound::HowlFar, Sound::Geiger] {
        assert!(s.variants() >= 3, "{s:?}");
    }
}

#[test]
fn loops_are_long_and_wind_layers_do_not_line_up() {
    let secs = |s: Sound| s.samples(0).len() as f32 / SAMPLE_RATE as f32;
    assert!(secs(Sound::WindLow) > 20.0 && secs(Sound::WindMid) > 26.0 && secs(Sound::WindHigh) > 34.0);
    // Co-prime lengths (23, 29, 37 s) mean the combined wind repeats only
    // after 23 * 29 * 37 seconds (about 6.9 hours).
    let l: Vec<u32> = [Sound::WindLow, Sound::WindMid, Sound::WindHigh].iter().map(|s| secs(*s).round() as u32).collect();
    assert_eq!(l, vec![23, 29, 37]);
    for fire in 0..3 {
        assert!(Sound::Fire.samples(fire).len() as f32 / SAMPLE_RATE as f32 > 5.5);
    }
    for m in [Sound::MusicCalm, Sound::MusicTense, Sound::MusicDanger] {
        assert!(secs(m) >= 45.0, "{m:?}");
    }
}

#[test]
fn loops_are_seamless() {
    for s in [Sound::WindLow, Sound::WindMid, Sound::WindHigh, Sound::MusicCalm, Sound::MusicTense, Sound::MusicDanger, Sound::Fire] {
        for v in 0..s.variants() {
            let w = s.samples(v);
            // The seam must look like any other step between samples.
            let jump = (w[0] - w[w.len() - 1]).abs();
            let biggest_step = w.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0f32, f32::max);
            assert!(jump <= biggest_step + 1e-6, "{s:?}/{v} jumps by {jump} at the seam (normal steps reach {biggest_step})");
        }
    }
}

#[test]
fn mix_levels_are_sane() {
    // Music and ambience sit well under the effects.
    let bed = rms(&Sound::MusicCalm.samples(0)) * Sound::MusicCalm.profile().volume;
    let shot = rms(&Sound::RifleShot.samples(0)[..4000]) * Sound::RifleShot.profile().volume;
    assert!(bed < shot * 0.6, "music {bed} vs shot {shot}");
    // The far howl is quieter than the near one.
    assert!(peak(&Sound::HowlFar.samples(0)) * Sound::HowlFar.profile().volume < peak(&Sound::HowlNear.samples(0)) * Sound::HowlNear.profile().volume);
    // The Geiger tick is quiet and very short.
    let g = Sound::Geiger.samples(0);
    assert!(g.len() < len(0.05) && Sound::Geiger.profile().volume <= 0.35);
}

#[test]
fn profiles_make_sense() {
    for &s in Sound::ALL {
        let p = s.profile();
        assert!((0.0..=1.0).contains(&p.volume), "{s:?}");
        assert!(p.pitch < 0.2 && p.vary < 0.5, "{s:?}");
        assert_eq!(p.group.profile().group, p.group, "{s:?}'s group must be its own group");
        if s.is_loop() {
            assert_eq!(p.pitch, 0.0, "{s:?} loops must not wobble in pitch");
        }
    }
    // Howls share one cooldown so a pack can't stack them.
    assert_eq!(Sound::HowlFar.profile().group, Sound::HowlNear);
}

/// FNV-1a over the bytes of every plain clip, each followed by its muffled
/// twin where it has one, per sound.
fn clip_hash(sound: Sound) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for v in 0..sound.variants() {
        eat(&sound.wav(v));
        if sound.muffleable() {
            eat(&sound.wav_muffled(v));
        }
    }
    h
}

/// Pins every synthesised clip (all 272) to the bytes the original
/// synthesiser made, so speed-ups to the maths (caching a coefficient,
/// remembering a pitch) provably change no sample. If you change a
/// sound on purpose, run `cargo test print_synth_hashes -- --ignored
/// --nocapture` and paste the new line in.
const GOLDEN: &[(&str, u64)] = &[
    ("StepSnow", 0x1dcfcd76d1be4b64),
    ("StepIce", 0x6952f4a0fcf7dbd3),
    ("StepRoad", 0xa4ba32175198aa72),
    ("StepConcrete", 0xb347d2b7900a1ced),
    ("StepWood", 0x7104fc426d10b255),
    ("RifleShot", 0x0f78c12f8eecfe23),
    ("ShotgunShot", 0xea1d5b04427b85d6),
    ("RevolverShot", 0x531bb1696322ad56),
    ("DryClick", 0x953b66896042526e),
    ("Jam", 0xf15d3dec8f3385cf),
    ("ShellTink", 0xc8030f6377cf06c3),
    ("ClunkOut", 0xb0badbcb30e2b07a),
    ("ClunkIn", 0xc67c98659a8c6d01),
    ("BoltRack", 0x7bae1f3ff7dc6756),
    ("BreakOpen", 0x2f95f73cccf9d7f8),
    ("BreakClose", 0x2f675c57ee3002bb),
    ("CylinderSpin", 0xdf0b661a0965e970),
    ("Swing", 0x8b139b5466436190),
    ("MeleeHit", 0x2da9eb80bc5774c8),
    ("HowlNear", 0xe88c270f3e20e870),
    ("HowlFar", 0x26fe94faa70c0fe9),
    ("Snarl", 0x40229e92dedea33d),
    ("Yelp", 0x72b8208e4dcb0809),
    ("Growl", 0x88b661069fc90724),
    ("MooseBellow", 0xfc36b58284309a77),
    ("MooseGrunt", 0x8c756079221d354b),
    ("Hoof", 0x45850a83c7368a2c),
    ("Caw", 0x13eacc9248d4a36c),
    ("ShotTailOutdoor", 0xff13160291fbb039),
    ("ShotTailIndoor", 0xaed323960e73dabc),
    ("BoltClack", 0xea42a0cc4842d135),
    ("HammerCock", 0x2f31e985680af7d5),
    ("WindBreeze", 0x4f24a12c0e2ddac6),
    ("WindHowl", 0x2d37c20bb14082cb),
    ("RoomHum", 0x344fbc74e97c4604),
    ("Drip", 0xce3be65aaa3238e6),
    ("Creak", 0x909b9fede500e78f),
    ("RaiderShout", 0x56dd681d4c0a2f0e),
    ("RaiderGrunt", 0x0405c7d6c795d92a),
    ("StepSnowRun", 0x221d9600e56ce3be),
    ("StepSnowSqueak", 0xfaa809d144a7b61c),
    ("IceCrack", 0x33bb15cf07f3dc9c),
    ("IceCreak", 0x8400c3c80b630c42),
    ("Geiger", 0xceab9bcc320514cb),
    ("PickupAmmo", 0x7628532719302dbc),
    ("PickupMed", 0xd81cb8fac175aacb),
    ("PickupFood", 0xa5087052275a3ce2),
    ("PickupScrap", 0xecfdaee1cdffbe7f),
    ("Craft", 0x980a620baace5435),
    ("ContainerOpen", 0x93051381f702bc32),
    ("WindLow", 0x7aa98345b2cd0bbb),
    ("WindMid", 0xa25c90456551b897),
    ("WindHigh", 0x6fcf461848d15c37),
    ("Gust", 0x6e53f70eb2954e8d),
    ("Fire", 0x88ce9cfe7c420ed9),
    ("Siren", 0x8a98eb7b86fa6a89),
    ("MusicCalm", 0x89895bc00c84315d),
    ("MusicTense", 0x47c4a54de794aba0),
    ("MusicDanger", 0x27516ade117416c3),
    ("PipOn", 0xcfe709d5f4cf533d),
    ("PipOff", 0x2526a0d23a728eea),
    ("PipStatic", 0x3aaa921efaa6119a),
    ("PipScroll", 0xc21425da0ccff6f5),
    ("PipHum", 0x14d1b8664cee7a1a),
    ("UiTab", 0x70775b77961e7ef1),
];

#[test]
fn every_clip_is_byte_for_byte_what_the_original_synthesiser_made() {
    assert_eq!(GOLDEN.len(), Sound::ALL.len(), "a sound was added or removed: update GOLDEN");
    // The fingerprints were recorded on Linux x86-64. Synthesis uses sin, exp
    // and powf, which another platform's maths library may round differently
    // in the last bit, so the exact check holds only there; elsewhere every
    // clip is still checked to be a real, finite sound.
    let exact = cfg!(all(target_os = "linux", target_arch = "x86_64"));
    for (&sound, &(name, want)) in Sound::ALL.iter().zip(GOLDEN) {
        assert_eq!(format!("{sound:?}"), name, "GOLDEN is in Sound::ALL order");
        if exact {
            assert_eq!(clip_hash(sound), want, "{name} changed");
        } else {
            let samples = sound.samples(0);
            assert!(!samples.is_empty() && samples.iter().all(|s| s.is_finite() && s.abs() <= 1.5), "{name} is not a sound");
        }
    }
}

#[test]
#[ignore]
fn print_synth_hashes() {
    for &s in Sound::ALL {
        println!("        (\"{s:?}\", {:#018x}),", clip_hash(s));
    }
}

#[test]
fn a_clip_and_its_muffled_twin_from_one_run_are_the_same_bytes_as_two_runs() {
    for &s in Sound::ALL.iter().filter(|s| !s.is_loop()) {
        for v in 0..s.variants() {
            let (plain, muffled) = s.wav_pair(v);
            assert!(plain == s.wav(v), "{s:?} v{v} plain");
            match muffled {
                Some(m) => {
                    assert!(s.muffleable(), "{s:?} has no muffled twin to make");
                    assert!(m == s.wav_muffled(v), "{s:?} v{v} muffled");
                }
                None => assert!(!s.muffleable(), "{s:?} should have had one"),
            }
        }
    }
}
