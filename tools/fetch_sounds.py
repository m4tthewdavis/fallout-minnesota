#!/usr/bin/env python3
"""Build the recorded sound effects in assets/sounds/ from CC0 / CC-BY recordings.

Run from the repository root:

    python3 tools/fetch_sounds.py            # download (cached), process, write, update CREDITS.md
    python3 tools/fetch_sounds.py --report   # also print a loudness / length table for every clip
    python3 tools/fetch_sounds.py --force    # rebuild every clip even if it is up to date
    python3 tools/fetch_sounds.py --music    # rebuild only the three music loops (+ credits)
    python3 tools/fetch_sounds.py --verify   # re-check each source's licence on its Freesound page (online)

What it does
------------
* Sources are Freesound "HQ preview" files (https://freesound.org, each page checked for CC0 or
  CC BY 3.0/4.0 when it was added; never NC) and the Kenney CC0 packs Impact Sounds, Interface
  Sounds and RPG Audio (https://kenney.nl/assets). They are cached under
  ~/stage/audio_cache/ (NOT /tmp, which gets wiped), so a second run works offline.
* Each clip in the CLIPS table is cut from its source (trim start/end in seconds), mixed to
  mono, pitch-shifted if asked (a speed change), resampled to 44.1 kHz, high-passed, given a
  tight head and a short fade-out, and matched in loudness to the synthesised clip it replaces
  (RMS over the active part, see TARGET_DB), with peaks held under -1 dBFS.
* Loops (wind, fire, siren, room hum) are cut to an exact length and the tail is cross-faded into
  the head, so they join seamlessly.
* Sounds the game muffles behind walls and trees (Sound::muffleable in src/sim/synth.rs) also get
  a `<stem>_<take>_muffled.ogg`, made with the same two one-pole 900 Hz low-passes as
  synth.rs `muffled()`, so the loader doesn't fall back to a bright copy through a wall.
* Files are named `<snake_case_sound>_<take>.ogg` (src/sim/recorded.rs). Sounds with no files stay
  synthesised (Geiger, Pip-Boy).
* The music (`music_calm_0`, `music_tense_0`, `music_danger_0`) comes from three CC0 OpenGameArt tracks listed in
  MUSIC. Each is cut to a loop of whole bars (the tail is cross-faded into the head at the best-matching phase),
  made mono when the recording is mono anyway (the music bus is not spatial), high-passed at 25 Hz, and levelled to
  the loudness of the synthesised music (TARGET_DB). Cached under ~/stage/audio_cache/music/.
* Finally the "Recorded sounds" section of assets/CREDITS.md is rewritten (between its own
  markers, below the `<!-- generated-above -->` line that tools/fetch_assets.py owns).

Needs python3 + numpy + soundfile (libsndfile with Vorbis). Nothing else.
"""

import hashlib
import io
import json
import os
import re
import sys
import urllib.request
import zipfile

import numpy as np
import soundfile as sf

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "assets", "sounds")
CREDITS = os.path.join(ROOT, "assets", "CREDITS.md")
CACHE = os.path.expanduser("~/stage/audio_cache")
UA = {"User-Agent": "Mozilla/5.0 (fallout-minnesota sound fetch)"}
SR = 44100
PEAK_MAX = 10 ** (-3.2 / 20)  # encoder ceiling; Vorbis overshoots by up to ~1.3 dB, so decoded peaks stay under -1 dBFS
CODE_VERSION = 18  # bump when the processing changes, to rebuild everything

# ---------------------------------------------------------------------------------------------
# Loudness targets: the active-part RMS (dBFS) of the synthesised clips each recording replaces,
# measured from the dump `FMN_DUMP_SYNTH=<folder>` writes (~/stage/synth). Measure = RMS of the
# 20 ms frames that are within 20 dB of the loudest frame (so silence and long quiet tails do not
# count). Recordings are matched to this per sound, then each take's `g` (dB) is added.
# ---------------------------------------------------------------------------------------------
TARGET_DB = {
    "bolt_clack": -20.2, "bolt_rack": -20.6, "break_close": -18.8, "break_open": -22.5, "caw": -20.4,
    "clunk_in": -17.0, "clunk_out": -18.6, "container_open": -21.2, "craft": -18.8, "creak": -17.3,
    "cylinder_spin": -19.6, "drip": -22.3, "dry_click": -16.2, "fire": -23.9, "geiger": -20.6, "growl": -20.5,
    "gust": -18.9, "hammer_cock": -23.1, "hoof": -13.9, "howl_far": -19.0, "howl_near": -12.4,
    "ice_crack": -13.2, "ice_creak": -17.6, "jam": -19.0, "melee_hit": -14.9, "moose_bellow": -8.6,
    "moose_grunt": -14.6, "music_calm": -22.0, "music_danger": -21.9, "music_tense": -22.1,
    "pickup_ammo": -20.4, "pickup_food": -17.6, "pickup_med": -18.9, "pickup_scrap": -19.3, "pip_hum": -19.5,
    "pip_off": -18.1, "pip_on": -18.2, "pip_scroll": -18.7, "pip_static": -23.9, "raider_grunt": -17.5,
    "raider_shout": -16.4, "revolver_shot": -13.4, "rifle_shot": -11.1, "room_hum": -15.9,
    "shell_tink": -23.6, "shot_tail_indoor": -18.0, "shot_tail_outdoor": -19.9, "shotgun_shot": -9.9,
    "siren": -15.0, "snarl": -21.3, "step_concrete": -23.0, "step_ice": -22.5, "step_road": -23.7,
    "step_snow": -21.9, "step_snow_run": -20.5, "step_snow_squeak": -24.4, "step_wood": -20.4, "swing": -17.7,
    "ui_tab": -19.1, "wind_breeze": -24.1, "wind_high": -21.4, "wind_howl": -19.4, "wind_low": -21.5,
    "wind_mid": -22.1, "yelp": -17.0,
}

# Sounds that get a muffled copy (mirror of Sound::muffleable in src/sim/synth.rs, in snake case).
MUFFLEABLE = {
    "rifle_shot", "shotgun_shot", "revolver_shot", "shot_tail_outdoor", "shot_tail_indoor", "bolt_clack",
    "hammer_cock", "snarl", "yelp", "growl", "howl_near", "howl_far", "moose_bellow", "moose_grunt", "caw",
    "raider_shout", "raider_grunt", "step_snow", "step_snow_run", "step_snow_squeak", "step_ice", "step_road",
    "step_concrete", "step_wood", "hoof",
}

# Loops: add_loop(stem, source, start, length s, cross-fade s, "noise" | "tonal", search s). Lengths are whole
# seconds that share no factors (23/29/37/41/47) so the layers never line up audibly.
LOOPS = set()  # loop stems, filled by add_loop below (each take carries its own length)

# ---------------------------------------------------------------------------------------------
# Sources. Freesound: id -> (user id, user name, title, licence). Licences were read from each
# sound's own page ("CC0" = Creative Commons 0 / public domain dedication).
# ---------------------------------------------------------------------------------------------
FS = {}
KENNEY = {
    "impact-sounds": "https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip",
    "interface-sounds": "https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip",
    "rpg-audio": "https://kenney.nl/media/pages/assets/rpg-audio/8e99002d76-1677590336/kenney_rpg-audio.zip",
}
KENNEY_TITLE = {"impact-sounds": "Impact Sounds", "interface-sounds": "Interface Sounds", "rpg-audio": "RPG Audio"}
LICENCE_URL = {
    "CC0": "https://creativecommons.org/publicdomain/zero/1.0/",
    "CC BY 3.0": "https://creativecommons.org/licenses/by/3.0/",
    "CC BY 4.0": "https://creativecommons.org/licenses/by/4.0/",
}

# ---------------------------------------------------------------------------------------------
# Music. stem -> source (OpenGameArt page, direct file URL, cache name, author, title, licence), the loop
# (start s, length s, cross-fade s, phase search s, "tonal"|"noise") and the Vorbis quality. The lengths are whole
# bars of each track (tense: 3.75 s bars; danger: 9.08 s = 4 bars), found by autocorrelating the onsets.
# ---------------------------------------------------------------------------------------------
OGA = "https://opengameart.org/sites/default/files/"
MUSIC = {
    "music_calm": dict(
        page="https://opengameart.org/content/the-world-fell-silent", url=OGA + "the_world_fell_silent_loop.flac",
        cache="the_world_fell_silent_loop.flac", author="Loukyo", title="The World Fell Silent (loop version)", lic="CC0",
        loop=(0.0, 148.0, 4.0, 0.0, "noise"), q=0.3,
        note="sparse post-nuclear ambient; slow pads, long swells"),
    "music_tense": dict(
        page="https://opengameart.org/content/a-lurking-evil-horror-ambience", url=OGA + "evil_0.ogg",
        cache="evil_0.ogg", author="Tsorthan Grove", title="A lurking evil (horror ambience)", lic="CC0",
        loop=(0.0, 67.5, 2.0, 0.15, "tonal"), q=0.3,
        note="heartbeat pulses and sparse piano"),
    "music_danger": dict(
        page="https://opengameart.org/content/danger-escape", url=OGA + "Danger%20Escape_0.ogg",
        cache="danger_escape_0.ogg", author="Fupi", title="Danger Escape", lic="CC0",
        loop=(0.0, 72.64, 3.0, 0.15, "tonal"), q=0.3,
        note="bassy electronic pulse, driving but not bombastic"),
}
MUSIC_NAMES = {f"{k}_0.ogg" for k in MUSIC}

CLIPS = []  # (stem, take, source, start, end, opts)


def add(stem, src, *spans, **opts):
    """Add one take per (start, end) span of `src` (a Freesound id or "kn:<pack>/<file>")."""
    for span in spans:
        if isinstance(span, (int, float)):  # a bare start time: runs to `dur` (or the default length)
            span = (span, span + opts.get("dur", 0.5))
        take = sum(1 for c in CLIPS if c[0] == stem)
        CLIPS.append((stem, take, src, span[0], span[1], {k: v for k, v in opts.items() if k != "dur"}))


def add_loop(stem, src, start, length, xfade, kind="noise", search=0.0, **opts):
    """A seamless loop of `length` seconds cut from `src` at `start` (tonal loops may stretch by up to
    +-`search` seconds to join at the best-matching phase). Several takes of one loop stem are allowed."""
    LOOPS.add(stem)
    add(stem, src, (start, start + length + xfade + 2 * search), loop=(length, xfade, kind, search), **opts)


# @@TABLE-BEGIN@@
FS[34708] = (282197, 'Jon285', '44_black_powder.wav', 'CC0')
FS[59988] = (71257, 'qubodup', 'SWOSH-01 44.1kHz', 'CC0')
FS[60013] = (71257, 'qubodup', 'Whoosh', 'CC0')
FS[72831] = (995351, 'audible-edge', 'Tornado siren in Streamwood IL.wav', 'CC0')
FS[75162] = (1088850, 'nigelcoop', 'crow.wav', 'CC0')
FS[107795] = (367313, 'j1987', 'gunfiddle.wav', 'CC0')
FS[108793] = (1553758, 'CeebFrack', 'shell load.ogg', 'CC0')
FS[109485] = (1213320, 'BudJillett', 'Wind-Gusts-late-autumn.wav', 'CC0')
FS[122183] = (71257, 'qubodup', 'Dog Growling Snarling Grumbling', 'CC0')
FS[156506] = (2827474, 'primeval_polypod', 'low_grunt1.wav', 'CC BY 3.0')
FS[159710] = (2886479, 'AnthonyChan0', 'Mossberg 500A - 1 shot and pump', 'CC0')
FS[160478] = (1038806, 'unfa', "Dog's Yelping 7", 'CC0')
FS[163280] = (2183018, 'TobiasKosmos', 'DogYelp.wav', 'CC BY 4.0')
FS[177958] = (985466, 'Sclolex', 'Water Dripping in Cave.wav', 'CC0')
FS[181563] = (1857065, 'kingsrow', 'Fire Crackling 01.wav', 'CC0')
FS[182806] = (1038806, 'unfa', 'IR-02 (gunshot in a chapel MIXED)', 'CC0')
FS[186921] = (545448, 'ReadeOnly', 'Angry Ram', 'CC0')
FS[197361] = (1661766, 'felix.blume', 'Buffalos in the tall-grass prairie in Oklahoma, growling, grunting, sniffing and eating some food', 'CC0')
FS[204204] = (3310361, 'Danwardvs', '22 Bolt.wav', 'CC0')
FS[207993] = (1038806, 'unfa', 'Rusty Metal Creaking', 'CC0')
FS[210102] = (3278936, 'GrayJoy', 'Brass bullet shell casing drop onto concrete, multiple takes', 'CC0')
FS[210531] = (985466, 'Sclolex', 'distantshot.wav', 'CC0')
FS[216570] = (71257, 'qubodup', 'Snow Footsteps Running', 'CC BY 3.0')
FS[217186] = (2429597, 'Bosk1', 'Wind at door howling 4.wav', 'CC BY 4.0')
FS[233044] = (181941, 'klangfabrik', 'UTS-15 Shotgun with pump action 4takes.wav', 'CC0')
FS[250191] = (4592890, 'Hyperionn', 'Elk 7.wav', 'CC BY 4.0')
FS[253087] = (4415905, 'YleArkisto', 'Sudet ulvovat / Wolves howling, small pack, frost snapping', 'CC BY 4.0')
FS[256603] = (2276808, 'Kodack', 'Male Grunts', 'CC0')
FS[263489] = (2663250, 'lwdickens', 'elk.1.wav', 'CC0')
FS[263491] = (2663250, 'lwdickens', 'footsteps crunchy ice.wav', 'CC0')
FS[338674] = (4067257, 'newagesoup', 'wolf-growl.wav', 'CC BY 4.0')
FS[346905] = (2963485, 'reishugo', 'Revolver calibre 38 - dois disparos', 'CC0')
FS[362777] = (2940947, 'taure', 'Walking_Wood.mp3', 'CC0')
FS[370345] = (2558531, 'Zott820', 'Mosin Nagant Bolt Action Cycle', 'CC0')
FS[380156] = (2940947, 'taure', 'Howl_Echo.wav', 'CC0')
FS[384717] = (5937039, 'morganpurkis', 'Distant Gunshot 1.wav', 'CC0')
FS[386698] = (5798760, 'tommy_mooney', 'ammo box opening.wav', 'CC0')
FS[389590] = (6512973, 'Jofae', 'Swing Woosh', 'CC0')
FS[399066] = (586391, 'fastson', 'Tikka M65_1.wav', 'CC BY 3.0')
FS[399116] = (586391, 'fastson', 'DryFire_01.wav', 'CC BY 3.0')
FS[399266] = (5937039, 'morganpurkis', 'Simple Gunshot Reverb Test.wav', 'CC0')
FS[399268] = (5937039, 'morganpurkis', 'Simple Gunshot Reverb Test.wav', 'CC0')
FS[402790] = (7111288, 'acidsnowflake', 'Cocking a revolver', 'CC0')
FS[411567] = (6142149, 'LilMati', 'Centerfire Rifle Gun Shot 01.wav', 'CC0')
FS[416939] = (3719168, 'OBXJohn', 'Footsteps on crunchy ice on sidewalk - MP3', 'CC0')
FS[417345] = (5121236, 'InspectorJ', 'Gunshot, Distant, A.wav', 'CC BY 4.0')
FS[422513] = (3302499, 'Nightflame', 'Swinging staff whoosh (strong) 04.wav', 'CC0')
FS[450853] = (612689, 'kyles', 'gun lee enfield 303 rifle clean shot.wav', 'CC0')
FS[450869] = (612689, 'kyles', 'snowmobile footsteps boots hard crunchy snow walk away to snowmobile, start and pull away far.wav', 'CC0')
FS[452102] = (612689, 'kyles', 'footsteps rubber boots walk run packed snow squeaky on and offmic.flac', 'CC0')
FS[452180] = (612689, 'kyles', 'bark yelp dog small int.flac', 'CC0')
FS[453168] = (6253486, 'florianreichelt', 'footsteps in fresh snow', 'CC0')
FS[453445] = (612689, 'kyles', 'hum electric transformer in large room2.flac', 'CC0')
FS[454021] = (612689, 'kyles', 'footsteps boots squeaky snow medium speed pass 3 times.flac', 'CC0')
FS[456428] = (1505134, 'Frigus_XIII', 'footsteps on surfaced road in boots', 'CC0')
FS[459964] = (6253486, 'florianreichelt', 'Footsteps on concrete', 'CC0')
FS[465299] = (5993580, 'SoftDistortionFX', 'Concrete Footsteps', 'CC0')
FS[467183] = (6918403, 'Sophia_C', 'Pistol Dry Fire (Bersa BP9CC 9x19)', 'CC BY 4.0')
FS[472401] = (9934646, 'JoseAgudelo', '21_Lobo_gruñendo.wav', 'CC0')
FS[483536] = (6142149, 'LilMati', 'Centerfire Rifle Gun Shot 02.wav', 'CC0')
FS[484036] = (6142149, 'LilMati', 'Rifle Gun Shot 02.wav', 'CC0')
FS[500646] = (339183, 'betchkal', 'Cooper Creek 20160313_014852 solitary wolf howl very clear.wav', 'CC0')
FS[501560] = (8644110, 'shelbyshark', 'Single Action Revolver Cylinder Spinning.wav', 'CC0')
FS[506665] = (4921277, 'Rudmer_Rotteveel', 'Wood Creak Single V10', 'CC0')
FS[532191] = (9735871, 'mcmikai', 'Fire in the stove', 'CC0')
FS[539683] = (5923045, 'Anthousai', 'footsteps - ice 09.wav', 'CC0')
FS[541164] = (9662992, 'Resaural', 'Distant Gunshot', 'CC0')
FS[543685] = (9250976, 'Nox_Sound', 'Footsteps_Wood_Walk_Mono.wav', 'CC0')
FS[610998] = (1038806, 'unfa', 'Medium Male Pain Grunts', 'CC0')
FS[613849] = (9250976, 'Nox_Sound', 'Footsteps_Mountain_Boots_Snow_Walk_Mono.wav', 'CC0')
FS[620928] = (6493174, 'Metrolynn', '22 bolt action rifle cycle', 'CC0')
FS[632075] = (13497056, 'WannyManny', 'Ninja Vocalizations, Several Types.wav', 'CC0')
FS[637556] = (612689, 'kyles', 'footsteps shoes walk road asphalt hard.flac', 'CC0')
FS[647593] = (13618669, 'oneshotofficial', 'Single Action Army - classic revolver cock', 'CC BY 4.0')
FS[670307] = (11519060, 'bruno.auzet', 'silent windy pine forest.wav', 'CC0')
FS[674568] = (7157894, 'ser%C3%B8ut%C5%8Dnin--depriv%C9%99d', '9mm Handgun Being Dry Fired', 'CC0')
FS[683186] = (3692246, 'Shark_Anthony', '357 Magnum Revolver Gunshot', 'CC0')
FS[710084] = (1661766, 'felix.blume', 'Rifle gunshot, one shot', 'CC0')
FS[725402] = (7157894, 'ser%C3%B8ut%C5%8Dnin--depriv%C9%99d', 'A rifle being dry fired once', 'CC0')
FS[737896] = (13973196, 'Vrymaa', 'Ice - Lake fractures', 'CC0')
FS[741366] = (16104671, 'Mish7913', 'Hooded Crow: Cawing', 'CC0')
FS[773873] = (15468302, 'MrGungus', 'shotgun shoot', 'CC0')
FS[775017] = (13973196, 'Vrymaa', 'Metal lid - Open & close', 'CC0')
FS[779877] = (1661766, 'felix.blume', 'Wood fire crackling, near flames', 'CC0')
FS[795313] = (2825355, 'Guy_Personface', 'Revolver Cylinder Spin - Remington 1875', 'CC0')
FS[835200] = (8956746, 'C-V', 'Revolver Gunshots', 'CC0')
FS[844289] = (16968183, 'randbsoundbites', 'Opening & putting down lid back down onto a ornate metal container', 'CC0')
FS[868416] = (16968183, 'randbsoundbites', 'Opening & closing a small metal container 2', 'CC0')

# =============================================================================================
# Footsteps. One take per footfall, cut at the onset of each step.
# =============================================================================================
# Snow, walking: Nox_Sound boots on snow, florianreichelt fresh snow, kyles hard crunchy snow, Kenney.
add("step_snow", 613849, (1.028, 1.244), (4.748, 5.072), (8.364, 8.688), (10.572, 10.896))
add("step_snow", 453168, (0.196, 0.412), (8.48, 8.7), (17.188, 17.512))
add("step_snow", 450869, (0.172, 0.408), (5.816, 6.056))
add("step_snow", "kn:impact-sounds/footstep_snow_000.ogg", (0, None))
add("step_snow", "kn:impact-sounds/footstep_snow_002.ogg", (0, None))
# Snow, running: qubodup sprinting in snow.
add("step_snow_run", 216570, (0.46, 0.764), (0.84, 1.06), (3.076, 3.38), (3.416, 3.656), (4.752, 5.036), (5.264, 5.5))
# Snow, squeaking (dry snow in deep cold): kyles squeaky snow, picked for the high-frequency squeal.
add("step_snow_squeak", 454021, (1.044, 1.368), (3.164, 3.488), (4.0, 4.272), (15.992, 16.232), (36.128, 36.372))
add("step_snow_squeak", 452102, (4.716, 5.04), (11.64, 11.936), (17.088, 17.412))
# Ice: crunchy ice and an icy sidewalk.
add("step_ice", 263491, (30.256, 30.528), (77.472, 77.796), (123.868, 124.132))
add("step_ice", 416939, (10.416, 10.664), (27.776, 28.1), (43.588, 43.82))
add("step_ice", 539683, (1.148, 1.412), (5.992, 6.22))
# Road: hard asphalt and boots on a surfaced road.
add("step_road", 637556, (0.712, 1.036), (5.172, 5.4), (5.916, 6.228))
add("step_road", 456428, (5.132, 5.456), (20.84, 21.076), (38.496, 38.82))
# Concrete.
add("step_concrete", 459964, (0.304, 0.528), (8.752, 9.076), (19.48, 19.804))
add("step_concrete", 465299, (0.428, 0.704), (27.384, 27.708))
add("step_concrete", "kn:impact-sounds/footstep_concrete_000.ogg", (0, None))
add("step_concrete", "kn:impact-sounds/footstep_concrete_003.ogg", (0, None))
# Wood floors.
add("step_wood", 543685, (1.0, 1.324), (12.096, 12.42), (22.112, 22.436))
add("step_wood", 362777, (0.892, 1.104), (11.784, 12.0), (16.06, 16.348))
add("step_wood", "kn:impact-sounds/footstep_wood_000.ogg", (0, None))
add("step_wood", "kn:impact-sounds/footstep_wood_002.ogg", (0, None))

# =============================================================================================
# Guns. Shots keep their natural decay (the game adds an outdoor / indoor tail after them).
# =============================================================================================
# Pipe rifle: Tikka M65 at 100 m (fastson), Lee-Enfield .303 (kyles), LilMati centerfire rifles, a field rifle shot.
add("rifle_shot", 399066, (0.9, 2.5))
add("rifle_shot", 450853, (0.0, 1.6))
add("rifle_shot", 483536, (0.0, 1.6))
add("rifle_shot", 484036, (0.0, 1.5))
add("rifle_shot", 411567, (0.0, 1.7))
add("rifle_shot", 710084, (0.8, 2.4))
# Scrap shotgun: Mossberg 500A in woodland, UTS-15 slug shots, a phone-mic 12 gauge.
add("shotgun_shot", 159710, (0.05, 1.55))
add("shotgun_shot", 233044, (0.0, 1.7), (2.05, 3.75))
add("shotgun_shot", 773873, (0.0, 1.2))
# Frontier revolver: .357 Magnum, .38 Special at a range, a black-powder percussion revolver, .38 police revolver.
add("revolver_shot", 683186, (0.02, 0.9))
add("revolver_shot", 835200, (0.2, 1.3))
add("revolver_shot", 34708, (4.10, 4.58), fo=0.12)
add("revolver_shot", 346905, (0.48, 1.5))
add("revolver_shot", 835200, (2.04, 3.1), (4.0, 5.1))
# The echo after a shot: it plays 0.07 s (outdoors) / 0.025 s (indoors) after the bang, so these start with the
# crack softened away (slow fade-in) and let the rolling answer through.
add("shot_tail_outdoor", 417345, (0.14, 2.7), at=-60, fi=0.04)
add("shot_tail_outdoor", 210531, (0.0, 2.6), at=-60, fi=0.04)
add("shot_tail_outdoor", 541164, (0.02, 2.45), at=-60, fi=0.04)
add("shot_tail_outdoor", 384717, (0.0, 2.4), at=-60, fi=0.04)
add("shot_tail_indoor", 182806, (0.0, 0.95), at=-60, fi=0.015)
add("shot_tail_indoor", 399268, (0.0, 0.67), at=-60, fi=0.015)
add("shot_tail_indoor", 399266, (0.0, 0.67), at=-60, fi=0.015)

# =============================================================================================
# Weapon handling
# =============================================================================================
add("dry_click", 725402, (0.0, 0.12))
add("dry_click", 674568, (0.02, 0.12))
add("dry_click", 467183, (0.0, 0.1))
add("dry_click", 399116, (0.28, 0.4))
add("jam", 107795, (0.36, 0.72), sq=1)
add("jam", "kn:impact-sounds/impactMetal_light_003.ogg", (0, 0.35))
add("shell_tink", 210102, (2.0, 2.42), (3.83, 4.25), (5.18, 5.6), (9.71, 10.13))
add("clunk_out", "kn:impact-sounds/impactMetal_medium_000.ogg", (0, 0.3))
add("clunk_out", "kn:impact-sounds/impactMetal_medium_003.ogg", (0, 0.3))
add("clunk_in", 108793, (0.07, 0.4), sq=1)
add("clunk_in", "kn:impact-sounds/impactMetal_light_001.ogg", (0, 0.3))
add("bolt_rack", 370345, (0.74, 1.2), (3.59, 4.05), (5.75, 6.2))
add("bolt_rack", 204204, (0.24, 0.6))
add("bolt_clack", 620928, (0.1, 0.55))
add("bolt_clack", 370345, (0.37, 0.8), (7.89, 8.5))
add("break_open", "kn:rpg-audio/metalLatch.ogg", (0, 0.26))
add("break_open", "kn:impact-sounds/impactMetal_light_002.ogg", (0, 0.35))
add("break_close", "kn:rpg-audio/metalClick.ogg", (0, 0.4), sq=1)
add("break_close", "kn:impact-sounds/impactPlate_light_001.ogg", (0, 0.4))
add("hammer_cock", 402790, (0.7, 1.05))
add("hammer_cock", 647593, (0.0, 0.4))
add("cylinder_spin", 501560, (0.7, 1.35))
add("cylinder_spin", 795313, (0.85, 1.4))
add("swing", 389590, (0.0, 0.3), fo=0.1)
add("swing", 59988, (0.0, 0.3), fo=0.1)
add("swing", 422513, (0.0, 0.3), fo=0.1)
add("swing", 60013, (0.0, 0.4), fo=0.12)
add("melee_hit", "kn:impact-sounds/impactMining_000.ogg", (0, 0.45))
add("melee_hit", "kn:impact-sounds/impactMining_001.ogg", (0, 0.45))
add("melee_hit", "kn:impact-sounds/impactMining_003.ogg", (0, 0.45))
add("melee_hit", "kn:impact-sounds/impactPunch_heavy_001.ogg", (0, 0.45))

# =============================================================================================
# Creatures
# =============================================================================================
# Wolves. Near howls: single wolves with a slight echo (taure) cut to the first ~4.5 s; far howls: betchkal's
# reverberant solitary wolf (Wrangell-St. Elias), rolled off above 4.5 kHz for distance.
add("howl_near", 380156, (2.37, 6.9), (8.02, 12.5), (14.12, 18.6), fi=0.05, fo=0.9)
add("howl_near", 253087, (4.0, 8.5), fi=0.05, fo=0.9)
add("howl_far", 500646, (1.84, 8.9), (11.49, 18.5), (29.32, 36.3), (40.57, 47.5), fi=0.15, fo=1.6, lp=4500)
# Snarl: short bursts of a growling dog, pitched down a little to wolf size.
add("snarl", 122183, (4.43, 4.98), (5.40, 5.95), (6.60, 7.15), (7.62, 8.17), p=-2, fo=0.12)
# Growl: a wolf growling (newagesoup), a dog growling, and "Lobo gruñendo" (wolf growling).
add("growl", 338674, (0.04, 1.5), fi=0.04, fo=0.3)
add("growl", 122183, (12.82, 14.15), p=-2, fi=0.04, fo=0.3)
add("growl", 472401, (1.0, 2.4), (4.0, 5.4), fi=0.04, fo=0.3)
# Yelp: quick dog yelps pitched down.
add("yelp", 160478, (2.63, 2.98), (5.97, 6.3), p=-2, fo=0.1)
add("yelp", 163280, (0.09, 0.42), p=-2, fo=0.1)
add("yelp", 452180, (0.78, 1.12), p=-2, fo=0.1)
# The mutated moose: red-deer / elk bugles pitched well down for the bellow; grunts from the elk's rut grunts
# and buffalo grunts.
add("moose_bellow", 263489, (2.4, 4.7), p=-6, fi=0.05, fo=0.5)
add("moose_bellow", 250191, (0.0, 2.9), p=-6, fi=0.05, fo=0.5)
add("moose_bellow", 186921, (3.15, 5.4), p=-3, fi=0.05, fo=0.5)
add("moose_grunt", 263489, (5.7, 6.1), (6.6, 7.0), p=-4, fo=0.1)
add("moose_grunt", 197361, (0.19, 0.55), (7.57, 7.95), p=-3, fo=0.1)
add("moose_grunt", 156506, (0.23, 0.57), p=-2, fo=0.1)
# Hooves on frozen ground: dull thuds, softened.
add("hoof", "kn:impact-sounds/impactSoft_heavy_000.ogg", (0, 0.25), lp=2500, fo=0.08)
add("hoof", "kn:impact-sounds/impactSoft_heavy_001.ogg", (0, 0.25), lp=2500, fo=0.08)
add("hoof", "kn:impact-sounds/impactSoft_heavy_002.ogg", (0, 0.25), lp=2500, fo=0.08)
add("hoof", "kn:impact-sounds/impactPlank_medium_001.ogg", (0, 0.25), lp=2200, fo=0.08)
# Rad-crows: hooded crow caws (Mish7913) and a crow flying off (nigelcoop), a touch lower.
add("caw", 741366, (0.12, 0.45), (1.15, 1.5), (2.29, 2.68), (3.52, 3.9), p=-2, fo=0.12)
add("caw", 75162, (0.08, 0.92), (1.16, 1.8), p=-2, fo=0.15)
# Raiders: non-verbal only. Pain / effort grunts and martial-arts shouts.
add("raider_grunt", 610998, (1.10, 1.45), (4.87, 5.22), (7.13, 7.48), (10.36, 10.7), fo=0.1)
add("raider_grunt", 256603, (0.28, 0.6), (3.65, 3.95), fo=0.1)
add("raider_shout", 632075, (0.24, 0.95), (14.8, 15.5), fo=0.15)
add("raider_shout", 610998, (4.87, 5.57), (12.67, 13.37), fo=0.15)

# =============================================================================================
# World, items and crafting
# =============================================================================================
add("ice_crack", 737896, (39.85, 41.2), (22.85, 24.1), fi=0.005, fo=0.4)
add("ice_creak", 737896, (34.45, 35.5), (29.6, 30.7), (7.1, 8.0), fi=0.03, fo=0.3)
add("container_open", 844289, (0.12, 0.75))
add("container_open", 868416, (0.15, 0.62))
add("container_open", 775017, (0.8, 1.5))
add("container_open", 386698, (0.4, 0.95))
add("creak", "kn:rpg-audio/creak1.ogg", (0, None), fi=0.01, fo=0.15)
add("creak", "kn:rpg-audio/creak2.ogg", (0, None), fi=0.01, fo=0.15)
add("creak", 506665, (0.05, 0.9), fi=0.01, fo=0.15)
add("creak", 207993, (0.19, 1.5), fi=0.02, fo=0.2)
add("drip", 177958, (25.14, 25.95), fo=0.25)
add("drip", 177958, (34.86, 35.7), fo=0.25, sq=1)
add("drip", 177958, (41.92, 42.75), (70.55, 71.4), fo=0.25)
add("craft", [("kn:impact-sounds/impactWood_light_000.ogg", 0, 0.3, 0.0), ("kn:impact-sounds/impactMetal_light_002.ogg", 0, 0.3, 0.3), ("kn:impact-sounds/impactWood_medium_001.ogg", 0, 0.35, 0.6)], (0, None))
add("craft", [("kn:rpg-audio/knifeSlice2.ogg", 0, 0.5, 0.0), ("kn:impact-sounds/impactMetal_medium_002.ogg", 0, 0.3, 0.45), ("kn:rpg-audio/metalClick.ogg", 0, 0.25, 0.7)], (0, None))
add("pickup_ammo", "kn:rpg-audio/metalClick.ogg", (0, 0.35))
add("pickup_ammo", "kn:rpg-audio/metalLatch.ogg", (0, 0.26))
add("pickup_ammo", "kn:impact-sounds/impactMetal_light_000.ogg", (0, 0.35))
add("pickup_med", "kn:impact-sounds/impactGlass_light_000.ogg", (0, 0.3))
add("pickup_med", "kn:impact-sounds/impactGlass_light_001.ogg", (0, 0.3))
add("pickup_med", "kn:rpg-audio/handleSmallLeather.ogg", (0, 0.34), sq=1)
add("pickup_food", "kn:impact-sounds/impactTin_medium_000.ogg", (0, 0.16))
add("pickup_food", "kn:impact-sounds/impactTin_medium_001.ogg", (0, 0.16))
add("pickup_food", "kn:rpg-audio/cloth2.ogg", (0, 0.3))
add("pickup_scrap", "kn:impact-sounds/impactMetal_medium_001.ogg", (0, 0.4))
add("pickup_scrap", "kn:rpg-audio/handleCoins.ogg", (0, 0.4), sq=1)
add("pickup_scrap", "kn:impact-sounds/impactMetal_heavy_000.ogg", (0, 0.4))

# =============================================================================================
# Ambience loops (mono) and gusts
# =============================================================================================
# Wind layers cut from bruno.auzet's "silent windy pine forest", split by band so each plays its role in the mix.
add_loop("wind_low", 670307, 214.5, 23, 2.0, hp=20, lp=380, order=3)
add_loop("wind_mid", 670307, 115.0, 29, 2.0, hp=300, lp=3500)
add_loop("wind_high", 670307, 250.0, 37, 2.0, hp=2500, order=3)
add_loop("wind_breeze", 670307, 64.5, 41, 2.0, hp=20, lp=700, order=3)
# The blizzard howl: wind through a door gap (Bosk1), kept to the tonal band.
add_loop("wind_howl", 217186, 30.0, 47, 2.0, hp=180, lp=1800)
# Gusts: one-shot swells from BudJillett's late-autumn gusts.
add("gust", 109485, (55.3, 60.5), (67.0, 72.2), (80.8, 86.0), (98.3, 103.5), fi=1.2, fo=1.8, hp=60)
add_loop("fire", 532191, 13.0, 9, 1.0, hp=60, sq=1)
add_loop("fire", 779877, 99.0, 11, 1.0, hp=60)
add_loop("fire", 181563, 6.5, 7, 0.8, hp=60)
add_loop("room_hum", 453445, 8.5, 8, 1.0, "tonal", search=1.0, hp=30, lp=600, order=3)
add_loop("siren", 72831, 5.0, 9, 1.5, "tonal", search=1.5, hp=100)
# @@TABLE-END@@


# ---------------------------------------------------------------------------------------------
# Download
# ---------------------------------------------------------------------------------------------
def http_get(url):
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def cached(path, url):
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        print("  download", url)
        data = http_get(url)
        with open(path, "wb") as f:
            f.write(data)
    return path


def fs_preview(sid):
    uid = FS[sid][0]
    url = f"https://cdn.freesound.org/previews/{sid // 1000}/{sid}_{uid}-hq.ogg"
    return cached(os.path.join(CACHE, "freesound", f"{sid}.ogg"), url)


_kenney_zips = {}


def kenney_file(spec):
    pack, name = spec[3:].split("/", 1)
    if pack not in _kenney_zips:
        path = cached(os.path.join(CACHE, "kenney", f"kenney_{pack}.zip"), KENNEY[pack])
        _kenney_zips[pack] = zipfile.ZipFile(path)
    z = _kenney_zips[pack]
    member = next(n for n in z.namelist() if n.endswith("/" + name) or n == name)
    return z.read(member)


_decoded = {}


def decode(src):
    """The whole source as (mono float64, sample rate)."""
    if src not in _decoded:
        if isinstance(src, str) and src.startswith("kn:"):
            x, sr = sf.read(io.BytesIO(kenney_file(src)), dtype="float64")
        else:
            x, sr = sf.read(fs_preview(src), dtype="float64")
        if x.ndim > 1:
            x = x.mean(axis=1)
        _decoded[src] = (x, sr)
    return _decoded[src]


def verify_licences():
    ok = True
    for sid, (uid, user, title, lic) in sorted(FS.items()):
        page = http_get(f"https://freesound.org/people/{user}/sounds/{sid}/").decode("utf8", "replace")
        m = re.search(r"creativecommons\.org/(licenses|publicdomain)/([a-z\-]+)/([\d.]+)", page)
        found = {"zero": "CC0", "by": f"CC BY {m.group(3)}" if m else "?"}.get(m.group(2) if m else "", f"{m.group(2)} {m.group(3)}" if m else "?")
        flag = "ok " if found == lic else "BAD"
        ok &= found == lic
        print(flag, sid, user, title[:50], lic, "page says", found)
    return ok


# ---------------------------------------------------------------------------------------------
# DSP (numpy only)
# ---------------------------------------------------------------------------------------------
def reflect_pad(x, n):
    n = min(n, len(x) - 1)
    if n <= 0:
        return x, 0
    head = 2 * x[0] - x[n:0:-1]
    tail = 2 * x[-1] - x[-2:-n - 2:-1]
    return np.concatenate([head, x, tail]), n


def fft_resample(x, n_out):
    """Resample to exactly n_out samples (band-limited, via the spectrum)."""
    n = len(x)
    if n_out == n:
        return x.copy()
    X = np.fft.rfft(x)
    m = n_out // 2 + 1
    Y = np.zeros(m, dtype=complex)
    k = min(len(X), m)
    Y[:k] = X[:k]
    if n_out < n:  # taper the top 8% of the new band to avoid ringing
        t = max(4, int(0.08 * m))
        Y[m - t:] *= 0.5 * (1 + np.cos(np.linspace(0, np.pi, t)))
    return np.fft.irfft(Y, n_out) * (n_out / n)


def convert(x, sr, semitones=0.0):
    """To 44.1 kHz, with an optional pitch change (speed change: semitones > 0 is higher and shorter)."""
    ratio = (SR / sr) / (2 ** (semitones / 12.0))
    if abs(ratio - 1) < 1e-9:
        return x
    pad = int(0.03 * sr)
    xp, p = reflect_pad(x, pad)
    n_out = int(round(len(xp) * ratio))
    y = fft_resample(xp, n_out)
    lo = int(round(p * ratio))
    return y[lo:lo + int(round(len(x) * ratio))]


def spectral_filter(x, sr, hp=None, lp=None, order=2):
    """Zero-phase Butterworth-shaped high-pass / low-pass applied in the frequency domain."""
    n = len(x)
    pad = min(n - 1, int(0.1 * sr))
    xp, p = reflect_pad(x, pad)
    X = np.fft.rfft(xp)
    f = np.fft.rfftfreq(len(xp), 1.0 / sr)
    H = np.ones_like(f)
    if hp:
        H *= 1.0 / (1.0 + (hp / np.maximum(f, 1e-6)) ** (2 * order))
    if lp:
        H *= 1.0 / (1.0 + (f / lp) ** (2 * order))
    return np.fft.irfft(X * H, len(xp))[p:p + n]


def frame_rms(x, sr, win=0.02):
    w = max(1, int(sr * win))
    n = len(x) // w
    if n < 1:
        return np.array([np.sqrt(np.mean(x ** 2) + 1e-20)])
    return np.sqrt((x[:n * w].reshape(n, w) ** 2).mean(axis=1))


def active_rms_db(x, sr=SR):
    """RMS (dBFS) over the 20 ms frames within 20 dB of the loudest frame."""
    fr = frame_rms(x, sr)
    m = fr.max()
    if m < 1e-9:
        return -120.0
    a = fr[fr > 0.1 * m]
    return 20 * np.log10(np.sqrt((a ** 2).mean()) + 1e-12)


def one_pole_lp(x, cutoff, sr=SR):
    a = 1.0 - np.exp(-2 * np.pi * cutoff / sr)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a * (x[i] - acc)
        y[i] = acc
    return y


def muffled(x):
    """synth.rs `muffled()`: two one-pole 900 Hz low-passes, keeping the peak (gain capped at 2.5)."""
    peak = np.abs(x).max()
    y = one_pole_lp(one_pole_lp(x, 900.0), 900.0)
    after = np.abs(y).max()
    if after > 1e-6:
        y = y * min(peak / after, 2.5)
    return y


def soft_limit(x, ceiling=PEAK_MAX, knee=0.55):  # (unused now; kept for reference)
    """Leave everything under `knee` alone and squeeze what is above it so the peak stays <= ceiling."""
    y = x.copy()
    a = np.abs(y)
    over = a > knee
    room = ceiling - knee
    y[over] = np.sign(y[over]) * (knee + room * np.tanh((a[over] - knee) / room))
    return y


def fade_out(x, secs):
    n = min(len(x), max(2, int(secs * SR)))
    x[-n:] *= 0.5 * (1 + np.cos(np.linspace(0, np.pi, n)))
    return x


def fade_in(x, secs):
    n = min(len(x), max(2, int(secs * SR)))
    x[:n] *= 0.5 * (1 - np.cos(np.linspace(0, np.pi, n)))
    return x


def tighten_head(x, thresh_db):
    """Drop the lead-in before the sound starts: everything before the first sample within `thresh_db` of the
    peak (and well above the noise), keeping 3 ms. Never cuts more than 0.12 s."""
    if thresh_db <= -90:
        return x
    peak = np.abs(x).max()
    level = max(peak * 10 ** (thresh_db / 20), 6.0 * np.median(np.abs(x)))
    idx = np.nonzero(np.abs(x) > level)[0]
    if len(idx) == 0:
        return x
    return x[min(max(0, idx[0] - int(0.003 * SR)), int(0.12 * SR)):]


def make_loop(seg, length_s, xfade_s, kind, search_s=0.0):
    """Fold the tail of `seg` over its head so the loop joins seamlessly.

    Equal-power cross-fade for noise, linear for tones. With `search_s` the loop length may move by that far
    either way to where the tail best matches the head's waveform (tones: siren, mains hum)."""
    L = int(round(length_s * SR))
    C = int(round(xfade_s * SR))
    S = int(round(search_s * SR))
    seg = np.concatenate([seg, np.zeros(max(0, L + C + 2 * S - len(seg)))])
    if S > 0:
        head = seg[:C]
        win = seg[L - S:L + S + C]
        # normalised cross-correlation of the head against every candidate tail start
        n = len(win) + len(head)
        corr = np.fft.irfft(np.fft.rfft(win, n) * np.conj(np.fft.rfft(head, n)), n)[:2 * S + 1]
        e = np.sqrt(np.convolve(win ** 2, np.ones(C), "valid")[:2 * S + 1] * (head ** 2).sum() + 1e-12)
        L = L - S + int(np.argmax(corr / e))
    seg = seg[:L + C]
    out = seg[:L].copy()
    t = np.linspace(0, 1, C, endpoint=False)
    if kind == "tonal":
        fin, fout = t, 1 - t
    else:
        fin, fout = np.sin(t * np.pi / 2), np.cos(t * np.pi / 2)
    out[:C] = seg[:C] * fin + seg[L:L + C] * fout
    return out


def lp_test_seam(x):
    """Largest jump across the loop point relative to typical sample-to-sample movement."""
    d = np.abs(np.diff(x))
    jump = abs(x[0] - x[-1])
    return jump / (np.percentile(d, 99) + 1e-9)


# ---------------------------------------------------------------------------------------------
# Building clips
# ---------------------------------------------------------------------------------------------
def src_name(src):
    if isinstance(src, list):
        return json.dumps([(src_name(l[0]),) + tuple(l[1:]) for l in src])
    return src if isinstance(src, str) else f"fs{src}"


def prep(src, start, end, opts, is_loop):
    """One source span: cut, mean removed, to 44.1 kHz (with the pitch change), filtered."""
    x, sr = decode(src)
    a = int(max(0, start) * sr)
    b = len(x) if end is None else min(len(x), int(end * sr))
    seg = x[a:b]
    if len(seg) < int(0.01 * sr):
        raise ValueError(f"empty span {start}-{end} of {src_name(src)} ({len(x) / sr:.2f}s long)")
    seg = seg - seg.mean()
    seg = convert(seg, sr, opts.get("p", 0.0))
    hp = opts.get("hp", 25 if is_loop else 45)
    return spectral_filter(seg, SR, hp=hp, lp=opts.get("lp"), order=opts.get("order", 2))


def render(stem, src, start, end, opts):
    """The processed clip as float64 mono at 44.1 kHz (before levelling).

    `src` may also be a list of layers (source, start, end, offset_s[, gain_db]) mixed into one clip."""
    is_loop = stem in LOOPS
    if isinstance(src, list):
        parts = []
        for layer in src:
            lsrc, ls, le, off = layer[:4]
            g = layer[4] if len(layer) > 4 else 0.0
            parts.append((int(off * SR), prep(lsrc, ls, le, opts, False) * 10 ** (g / 20)))
        seg = np.zeros(max(o + len(p) for o, p in parts))
        for o, p in parts:
            seg[o:o + len(p)] += p
    else:
        seg = prep(src, start, end, opts, is_loop)
    if is_loop:
        length, xf, kind, search = opts["loop"]
        return make_loop(seg, length, xf, kind, search)
    seg = tighten_head(seg, opts.get("at", -34.0))
    seg = fade_in(seg, opts["fi"] if opts.get("fi") is not None else 0.002)
    dur = len(seg) / SR
    seg = fade_out(seg, opts.get("fo", min(0.15, 0.3 * dur)))
    return seg


def lookahead_limiter(x, ceiling, look_ms=1.5, release_ms=70.0):
    """Peak limiter: the gain dips ahead of each peak (so no clipping) and recovers over `release_ms`."""
    n = len(x)
    la = max(1, int(SR * look_ms / 1000))
    need = np.minimum(1.0, ceiling / np.maximum(np.abs(x), 1e-9))
    pad = np.concatenate([np.ones(la), need, np.ones(la)])
    g = pad[la:la + n].copy()
    for k in range(1, la + 1):  # sliding minimum over [i - la, i + la]
        g = np.minimum(g, np.minimum(pad[la + k:la + k + n], pad[la - k:la - k + n]))
    g = np.convolve(np.concatenate([np.full(la, g[0]), g, np.full(la, g[-1])]), np.ones(la) / la, "same")[la:la + n]
    g = np.minimum(g, need)  # never above what a peak allows
    rel = 1.0 - np.exp(-1.0 / (SR * release_ms / 1000))
    out = np.empty(n)
    cur = 1.0
    for i in range(n):  # fast down (g already ramps ahead of peaks), slow up
        cur = g[i] if g[i] < cur else cur + (1.0 - cur) * rel
        out[i] = cur if cur < g[i] else g[i]
    return x * np.minimum(out, g)


MAX_LIMIT_DB = 12.0  # up to this much gain reduction on peaks; beyond that the clip is simply left quieter
SHOT_STEMS = {"rifle_shot", "shotgun_shot", "revolver_shot"}
LIMIT_FOR = {"rifle_shot": 14.0, "shotgun_shot": 14.0, "revolver_shot": 14.0}  # games squash gunshots hard


def limit_db(stem):
    return LIMIT_FOR.get(stem, MAX_LIMIT_DB)


def compress(x, thresh_db=-18.0, ratio=4.0, attack_ms=4.0, release_ms=140.0):
    """Fast-ish compressor (the first few ms of the crack get through) that lifts the body and tail of a shot.
    Threshold is relative to the clip's peak; makeup keeps the crack's level."""
    peak = np.abs(x).max()
    thr = peak * 10 ** (thresh_db / 20)
    a = 1 - np.exp(-1.0 / (SR * attack_ms / 1000))
    r = 1 - np.exp(-1.0 / (SR * release_ms / 1000))
    env = 0.0
    g = np.empty(len(x))
    ab = np.abs(x)
    for i in range(len(x)):
        v = ab[i]
        env += (a if v > env else r) * (v - env)
        g[i] = 1.0 if env <= thr else (thr / env) ** (1 - 1 / ratio)
    return x * g


def limit_peaks(stem, y, gain):
    """Hold the peak under -1 dBFS: a look-ahead limiter for up to MAX_LIMIT_DB, then give up loudness instead."""
    peak = np.abs(y).max()
    shave = 20 * np.log10(peak / PEAK_MAX) if peak > PEAK_MAX else 0.0
    if shave > 0:
        if shave > limit_db(stem):
            y = y * 10 ** (-(shave - limit_db(stem)) / 20)
            gain -= shave - limit_db(stem)
        if stem in LOOPS:  # limit three laps and keep the middle one so the seam stays smooth
            n = len(y)
            y = lookahead_limiter(np.concatenate([y, y, y]), PEAK_MAX)[n:2 * n]
            y = np.clip(y, -PEAK_MAX, PEAK_MAX)
        else:
            y = lookahead_limiter(y, PEAK_MAX)
            y = np.clip(y, -PEAK_MAX, PEAK_MAX)
    return y, gain, shave


def squash_to_target(stem, x, target):
    """Gunshots: compress, then drive into a tanh soft clip and a limiter until the active-part RMS is `target`.
    The crack stays (the compressor attacks in 4 ms, the clip only rounds the top), the body and tail come up."""
    x = compress(x)
    lo, hi = -10.0, 60.0
    for _ in range(22):
        g = (lo + hi) / 2
        y = lookahead_limiter(PEAK_MAX * np.tanh(x * 10 ** (g / 20) / PEAK_MAX) / np.tanh(1.0), PEAK_MAX)
        if active_rms_db(y) < target:
            lo = g
        else:
            hi = g
    return y, g, 20 * np.log10(np.abs(x * 10 ** (g / 20)).max() / PEAK_MAX)


GAIN_SPREAD_DB = 4.0  # takes from one recording are never boosted more than this above their median


def group_gains(stem, members):
    """Gain (dB) for each take of `stem`.

    The takes cut from one recording are levelled together: their median is matched to the synth clip's
    loudness, and no take is turned up more than GAIN_SPREAD_DB above that median (a take that needs more is a weak one). (Levelling every take on its
    own would turn the quiet ones up by 20-30 dB along with their noise floor.)
    """
    target = TARGET_DB.get(stem)
    by_src = {}
    for i, (src, rms, opts) in enumerate(members):
        by_src.setdefault(src_name(src), []).append(i)
    gains = [0.0] * len(members)
    for idx in by_src.values():
        need = [((members[i][2].get("tgt", target) - members[i][1]) if members[i][2].get("tgt", target) is not None else 0.0) for i in idx]
        med = float(np.median(need))
        for i, n in zip(idx, need):
            gains[i] = min(n, med + GAIN_SPREAD_DB) + members[i][2].get("g", 0.0)
    return gains


def quality(stem):
    return 0.5 if stem in LOOPS else 0.45


def write_ogg(path, x, stem):
    """Encode, decode again and turn the clip down until the decoded peak is under -1 dBFS (Vorbis overshoots)."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    x = x.astype(np.float32)
    for _ in range(6):
        sf.write(path, x, SR, format="OGG", subtype="VORBIS", compression_level=quality(stem))
        peak = float(np.abs(sf.read(path, dtype="float32")[0]).max())
        if peak <= 0.8912:
            break
        x = x * np.float32(0.8912 / peak * 0.985)


# ---------------------------------------------------------------------------------------------
# Music
# ---------------------------------------------------------------------------------------------
def music_source(stem):
    m = MUSIC[stem]
    return cached(os.path.join(CACHE, "music", m["cache"]), m["url"])


def write_blocks(path, y, q):
    """Vorbis in one-second blocks: libsndfile segfaults on one big write of a long clip here."""
    y = np.ascontiguousarray(y, dtype=np.float32)
    with sf.SoundFile(path, "w", SR, 1 if y.ndim == 1 else y.shape[1], format="OGG", subtype="VORBIS", compression_level=q) as f:
        for i in range(0, len(y), SR):
            f.write(y[i:i + SR])


def build_music(report=False, force=False):
    manifest_path = os.path.join(CACHE, "built.json")
    manifest = json.load(open(manifest_path)) if os.path.exists(manifest_path) else {}
    rows = []
    for stem, m in MUSIC.items():
        name = f"{stem}_0.ogg"
        key = hashlib.sha1(json.dumps([CODE_VERSION, stem, m, TARGET_DB[stem]], sort_keys=True).encode()).hexdigest()[:16]
        if not force and not report and manifest.get(name) == key and os.path.exists(os.path.join(OUT, name)):
            continue
        x, sr = sf.read(music_source(stem), dtype="float64")
        if x.ndim == 1:
            x = x[:, None]
        mono = x.shape[1] == 1 or abs(np.corrcoef(x[:, 0], x[:, 1])[0, 1]) > 0.99
        chans = [x.mean(axis=1)] if mono else [x[:, 0], x[:, 1]]
        start, length, xf, search, kind = m["loop"]
        out = []
        for c in chans:
            seg = c[int(start * sr):int((start + length + xf + 2 * search) * sr) + 1]
            seg = seg - seg.mean()
            seg = spectral_filter(convert(seg, sr), SR, hp=25, order=2)
            out.append(make_loop(seg, length, xf, kind, search if c is chans[0] else 0.0) if c is chans[0] else None)
        if len(chans) == 2:  # same loop length for both channels: re-cut the second at the first one's length
            n = len(out[0])
            seg = chans[1][int(start * sr):int((start + length + xf + 2 * search) * sr) + 1]
            seg = spectral_filter(convert(seg - seg.mean(), sr), SR, hp=25, order=2)
            C = int(round(xf * SR))
            seg = np.concatenate([seg, np.zeros(max(0, n + C - len(seg)))])
            t = np.linspace(0, 1, C, endpoint=False)
            fin, fout = (t, 1 - t) if kind == "tonal" else (np.sin(t * np.pi / 2), np.cos(t * np.pi / 2))
            r = seg[:n].copy()
            r[:C] = seg[:C] * fin + seg[n:n + C] * fout
            out[1] = r
        y = np.stack(out, axis=1)
        level = active_rms_db(y.T.reshape(-1))
        y = y * 10 ** ((TARGET_DB[stem] - level) / 20)
        peak = float(np.abs(y).max())
        shave = 20 * np.log10(peak / PEAK_MAX) if peak > PEAK_MAX else 0.0
        if shave > 0:  # gentle: linked look-ahead limiter on three laps, keeping the middle one (seam stays smooth)
            n = len(y)
            env = np.abs(y).max(axis=1)
            g = lookahead_limiter(np.concatenate([env] * 3), PEAK_MAX)
            ratio = g / np.maximum(np.concatenate([env] * 3), 1e-9)
            y = y * ratio[n:2 * n, None]
        y = np.clip(y, -PEAK_MAX, PEAK_MAX)
        path = os.path.join(OUT, name)
        write_blocks(path, y[:, 0] if mono else y, m["q"])
        dec = sf.read(path, dtype="float32")[0]
        if float(np.abs(dec).max()) > 0.8912:  # Vorbis overshoot: turn down and re-encode
            y = y * (0.8912 / float(np.abs(dec).max()) * 0.985)
            write_blocks(path, y[:, 0] if mono else y, m["q"])
        manifest[name] = key
        dec = sf.read(path, dtype="float64")[0]
        d = dec if dec.ndim == 1 else dec
        seam = float(np.abs(d[0] - d[-1]).max()) / (np.percentile(np.abs(np.diff(d, axis=0)), 99) + 1e-9)
        rows.append([name, len(dec) / SR, float(np.abs(dec).max()), active_rms_db((dec if dec.ndim == 1 else dec.T.reshape(-1))), TARGET_DB[stem], shave, seam, "mono" if mono else "stereo", os.path.getsize(path)])
    os.makedirs(CACHE, exist_ok=True)
    json.dump(manifest, open(manifest_path, "w"), indent=0)
    return rows


def build(report=False, force=False):
    os.makedirs(OUT, exist_ok=True)
    manifest_path = os.path.join(CACHE, "built.json")
    manifest = json.load(open(manifest_path)) if os.path.exists(manifest_path) else {}
    wanted = set()
    rows = []
    stems = []
    for c in CLIPS:
        if c[0] not in stems:
            stems.append(c[0])
    for stem in stems:
        group = [c for c in CLIPS if c[0] == stem]
        group_spec = json.dumps([(c[1], src_name(c[2]), c[3], c[4], c[5]) for c in group], sort_keys=True)
        names_of = []
        for _, take, src, start, end, opts in group:
            names = [f"{stem}_{take}.ogg"]
            if stem in MUFFLEABLE and opts.get("muffle", True):
                names.append(f"{stem}_{take}_muffled.ogg")
            names_of.append(names)
            wanted.update(names)
        key = hashlib.sha1(json.dumps([CODE_VERSION, group_spec, TARGET_DB.get(stem), GAIN_SPREAD_DB, MAX_LIMIT_DB, LIMIT_FOR]).encode()).hexdigest()[:16]
        done = all(os.path.exists(os.path.join(OUT, n)) and manifest.get(n) == key for names in names_of for n in names)
        if done and not force and not report:
            continue
        rendered = [render(stem, src, start, end, opts) for _, take, src, start, end, opts in group]
        members = [(c[2], active_rms_db(x), c[5]) for c, x in zip(group, rendered)]
        gains = group_gains(stem, members)
        for c, x, g, names in zip(group, rendered, gains, names_of):
            if stem in SHOT_STEMS or c[5].get("sq") or stem == "moose_bellow":
                y, g2, shave = squash_to_target(stem, x, TARGET_DB[stem] + c[5].get("g", 0.0))
                for n in names:
                    manifest[n] = key
                write_ogg(os.path.join(OUT, names[0]), y, stem)
                if len(names) > 1:
                    write_ogg(os.path.join(OUT, names[1]), muffled(y), stem)
                rows.append([names[0], len(y) / SR, np.abs(y).max(), active_rms_db(y), TARGET_DB.get(stem), g2, shave, c[2]])
                continue
            y, g2, shave = limit_peaks(stem, x * 10 ** (g / 20), g)
            goal = c[5].get("tgt", TARGET_DB.get(stem))
            for _ in range(3):  # a limiter eats loudness: ask for what it took, as far as the limit allows
                if goal is None or shave <= 0.3 or goal + c[5].get("g", 0.0) - active_rms_db(y) < 0.7 or shave >= limit_db(stem) - 0.2:
                    break
                g += min(goal + c[5].get("g", 0.0) - active_rms_db(y), limit_db(stem) - shave)
                y, g2, shave = limit_peaks(stem, x * 10 ** (g / 20), g)
            write_ogg(os.path.join(OUT, names[0]), y, stem)
            if len(names) > 1:
                write_ogg(os.path.join(OUT, names[1]), muffled(y), stem)
            for n in names:
                manifest[n] = key
            rows.append([names[0], len(y) / SR, np.abs(y).max(), active_rms_db(y), TARGET_DB.get(stem), g2, shave, c[2]])
    # remove clips that are no longer in the table
    for n in sorted(os.listdir(OUT)):
        if n.endswith(".ogg") and n not in wanted and n not in MUSIC_NAMES:
            os.remove(os.path.join(OUT, n))
            manifest.pop(n, None)
            print("  removed", n)
    os.makedirs(CACHE, exist_ok=True)
    json.dump(manifest, open(manifest_path, "w"), indent=0)
    return rows


def print_report(rows):
    print(f"{'file':34s} {'sec':>6s} {'peak':>6s} {'rms':>7s} {'target':>7s} {'gain':>6s} {'limit':>6s}  source")
    for name, dur, peak, rms, target, gain, shave, src in rows:
        t = f"{target:7.1f}" if target is not None else "      -"
        print(f"{name:34s} {dur:6.2f} {20 * np.log10(peak + 1e-12):6.1f} {rms:7.1f} {t} {gain:6.1f} {shave:6.1f}  {src_name(src)}")


# ---------------------------------------------------------------------------------------------
# CREDITS.md
# ---------------------------------------------------------------------------------------------
BEGIN = "<!-- recorded-sounds-begin -->"
END = "<!-- recorded-sounds-end -->"


def source_line(src):
    if isinstance(src, str):
        pack, name = src[3:].split("/", 1)
        return (f"Kenney, {KENNEY_TITLE[pack]}", f"<https://kenney.nl/assets/{pack}>", "CC0", f"`{name}`")
    uid, user, title, lic = FS[src]
    page = f"https://freesound.org/people/{user}/sounds/{src}/"
    return (user, f"<{page}>", lic, f"\"{title}\"")


def credits_section():
    by_src = {}
    for stem, take, src, start, end, opts in CLIPS:
        for one in ([l[0] for l in src] if isinstance(src, list) else [src]):
            by_src.setdefault(one, []).append(f"{stem}_{take}")
    lines = [
        BEGIN,
        "## Recorded sounds",
        "",
        "Built by `tools/fetch_sounds.py` into `sounds/` (Ogg Vorbis). Every clip is trimmed, mixed to mono, resampled to",
        "44.1 kHz, faded, level-matched and (for the muffled copies) low-passed: modified from the originals. Sources are the",
        "CC0 Kenney packs (Impact Sounds and RPG Audio; <https://kenney.nl/assets>) and Freesound sounds under CC0 or",
        "CC BY (attribution below, as the licence requires). The three music loops are cut from CC0 OpenGameArt tracks (listed",
        "under CC0, loop lengths and levels are ours). Sounds with no file here (the Geiger counter and the Pip-Boy",
        "interface sounds) are still synthesised from code.",
        "",
    ]
    for lic in ("CC BY 4.0", "CC BY 3.0", "CC0"):
        rows = []
        for src, used in by_src.items():
            who, page, l, title = source_line(src)
            if l != lic:
                continue
            stems = sorted({re.sub(r"_\d+$", "", u) for u in used})
            rows.append((who.lower(), who, title, page, ", ".join(f"`{s}`" for s in stems)))
        for stem, m in MUSIC.items():
            if m["lic"] == lic:
                rows.append((m["author"].lower(), m["author"], f"\"{m['title']}\" (cut into a loop)", f"<{m['page']}>", f"`{stem}`"))
        if not rows:
            continue
        rows.sort()
        lines += [f"### {lic} (<{LICENCE_URL[lic]}>)", "", "| Title | Author | Source | Used for |", "| --- | --- | --- | --- |"]
        merged = {}  # one row per page (Kenney has one page per pack, many files)
        for _, who, title, page, used in rows:
            if page in merged:
                old = merged[page][3]
                used = ", ".join(sorted(set(old.split(", ")) | set(used.split(", "))))
                merged[page] = (who, title, page, used)
            else:
                merged[page] = (who, title, page, used)
        for who, title, page, used in merged.values():
            if who.startswith("Kenney"):
                title = "Sound pack"
            lines.append(f"| {title} | {who} | {page} | {used} |")
        lines.append("")
    lines.append(END)
    return "\n".join(lines) + "\n"


def write_credits():
    text = open(CREDITS, encoding="utf-8").read() if os.path.exists(CREDITS) else ""
    marker = "<!-- generated-above -->"
    if marker not in text:
        text = text.rstrip("\n") + "\n\n" + marker + "\n"
    section = credits_section()
    if BEGIN in text and END in text:
        a = text.index(BEGIN)
        b = text.index(END) + len(END)
        text = text[:a] + section.rstrip("\n") + text[b:]
    else:
        text = text.rstrip("\n") + "\n\n" + section
    open(CREDITS, "w", encoding="utf-8").write(text)
    print("wrote", os.path.relpath(CREDITS, ROOT))


def main():
    args = set(sys.argv[1:])
    if "--verify" in args:
        return 0 if verify_licences() else 1
    if "--music" in args:  # music only (the effects are left untouched)
        for r in build_music(report=True, force=True):
            print(f"{r[0]:22s} {r[1]:6.2f}s peak {20 * np.log10(r[2]):5.1f} rms {r[3]:6.1f} (target {r[4]:.1f}) limiter {r[5]:.1f} dB seam {r[6]:.1f} {r[7]} {r[8] / 1e6:.2f} MB")
        write_credits()
        return 0
    rows = build(report="--report" in args, force="--force" in args)
    if "--report" in args:
        print_report(rows)
    music_rows = build_music(report="--report" in args, force="--force" in args)
    if "--report" in args:
        for r in music_rows:
            print(f"{r[0]:22s} {r[1]:6.2f}s peak {20 * np.log10(r[2]):5.1f} rms {r[3]:6.1f} (target {r[4]:.1f}) limiter {r[5]:.1f} dB seam {r[6]:.1f} {r[7]} {r[8] / 1e6:.2f} MB")
    write_credits()
    total = sum(os.path.getsize(os.path.join(OUT, n)) for n in os.listdir(OUT) if n.endswith(".ogg"))
    print(f"{len([n for n in os.listdir(OUT) if n.endswith('.ogg')])} files, {total / 1e6:.2f} MB in assets/sounds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
