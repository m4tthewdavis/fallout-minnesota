# Fallout: Minnesota — Prototype (Milestone 4)

A first-person cold-survival prototype made with **Rust + Bevy 0.16**. It is based on the *Fallout: Minnesota* design doc.

You are the Thawborn, just out of **Vault 143** on the shore of Lake Mille Lacs. The Long Winter is still on. Stay warm, avoid the glow, and survive the Frostfang packs that hunt in rad-blizzards.

## Running it (Windows)

1. Install Rust from <https://rustup.rs> (use the default MSVC toolchain). If it asks for the Visual Studio C++ Build Tools, install them too.
2. Open a terminal in this folder and run:

   ```
   cargo run
   ```

   The first build compiles the Bevy engine, which takes a few minutes. Later builds take seconds.
3. To run the game-rule unit tests: `cargo test`.

### Running the downloaded build

1. Download `fallout-minnesota-windows` from the GitHub Actions run.
2. **Unzip the whole thing first** (right-click → *Extract All…*). Don't double-click the `.exe` inside the zip: Windows then copies only the `.exe` to a temp folder, without its `assets`.
3. Run `FalloutMinnesota.exe` from the extracted folder. The `assets` folder must stay next to it.

The game looks for `assets` next to the `.exe`, one or two folders above it (so `target\release\fallout_minnesota.exe` finds the project's folder), in the current folder, and in the project folder when started with `cargo run`. If it still can't find its files it says so on screen and falls back to plain colours instead of drawing an invisible world.

If the build reports an error, copy the error text back to Claude and it will fix it.

## Controls

| Key | Action |
| --- | --- |
| WASD / Mouse | Move / look |
| Shift | Sprint (keeps you warmer, but cracks nuclear ice) |
| Space | Jump |
| Left mouse | Fire (hold for the rifle; click for the shotgun and revolver) / swing the ice axe |
| Right mouse | Aim down the sights |
| 1 2 3 4 / mouse wheel | Pipe rifle / scrap shotgun / revolver / ice axe (once found) |
| R | Reload, or clear a cold-weather jam. When dead, respawn |
| E | Open a container (crates, footlockers, tackle boxes) |
| B | At a shelter workbench: fit the next upgrade to your weapon (costs scrap) |
| Tab or M | Pip-Boy: map, stats, inventory (pauses the game) |
| H / X / F | Stimpak / RadAway / Vault 143 Hotdish |
| C | Craft a Frostfang coat (3 pelts, at a fish-house shelter) |
| Esc | Free the mouse (click to recapture) |
| G | Geiger counter on / off |
| F9 | Mute / unmute all sound |
| F10 / F11 | Master volume down / up |
| F5 / F6 | Effects volume down / up |
| F7 / F8 | Music volume down / up |

## What's in Milestone 1

- **Body Heat**: drains with air temperature and wind chill. Shelters, fire barrels, hotdish, sprinting and insulation warm you up. At 0 Heat you get frostbite and start losing health.
- **Radiation**: comes from the glowing snow during blizzards, the nuclear-ice lakes, a crater with a dud warhead, and the leaking Golden Atomic Mills silos. Rads lower your max HP, as in mainline Fallout.
- **Rad-blizzard cycle**: calm, then a 20-second siren warning, then the blizzard, which brings green whiteout fog, heavy snow, -35°F air, radiation and wolf hunts.
- **Nuclear ice**: lakes are fast shortcuts, but sprinting on them for about 2 seconds cracks the ice. You fall in and lose Heat and gain rads.
- **Pipe rifle**: hitscan with tracers, recoil and reloads. Below -20°F each shot has a chance to jam.
- **Frostfang wolves**: translucent fur and glowing eyes. In calm weather they wander. In blizzards they stalk and chase. Each blizzard brings a new pack. Killing them drops pelts, which craft the Frostfang coat (+0.35 insulation).
- **World**: the Vault 143 door, four fish-house shelters, pines, the Bullseye-Mart ruin, rusted cars, and loot (Stimpaks, RadAway, ammo, hotdish).

## What's new in Milestone 2

- **Collision**: you and the wolves now slide around trees, walls, fish houses, cars, silos and the vault hillside instead of walking through them.
- **Sound**: every sound is synthesised in code, so there are no audio files. You'll hear calm wind, a howling blizzard gale, the air-raid siren before a storm, footsteps crunching in snow, gunshots, jams, reloads, wolf howls, snarls and yelps, cracking ice, and Pip-Boy pickup blips.
- **Day/night cycle**: a full day lasts 12 real minutes. The sun moves and casts moving shadows, sunrise and sunset glow orange, and nights are dark blue with moonlight. Nights are up to 12°F colder, and distant howls carry across the ice. The HUD shows the day and clock.
- **Animated wolves**: Frostfangs trot with swinging legs (diagonal pairs, like a real trot). Their stride speeds up when they chase, and their tails wag harder on the hunt.

## What's new in Milestone 4

- **Sound, rebuilt.** Before changing anything I measured every sound. The worst offenders were the Geiger counter (it clicked 15 times a second through a whole blizzard), footsteps (three near-identical clips in a fixed 1-2-3 order), wolf howls (every wolf on its own timer, so packs stacked), and wind and fire (short, obvious loops). Now: every frequent sound has 3-6 variants and is never repeated back to back, with random pitch and volume; footsteps differ on snow, ice, road, concrete and wooden decks; wind is three loops of 23, 29 and 37 seconds plus random gusts, muffled in shelters; fires crackle from their own barrels; wolves, fires and brass are positioned in 3D (they pan and fade); howls share one 18-second cooldown and quick repeats get quieter; the Geiger counter is gentle (about 2 clicks a second in a blizzard) and G turns it off; music has calm, tense and danger moods that crossfade and leaves long silences. Volume keys are in the controls table. Still all synthesised in code.
- **Three new weapons, found not given.** The scrap shotgun (7 pellets, breaks open to reload) is in a military footlocker inside the Bullseye-Mart ruin; the frontier revolver (heavy, rarely jams in the cold) is in an armoury crate at the Golden Atomic Mills; the ice axe (melee) is in a tackle box at a fish house. Each has its own model, aim point, recoil, reload animation and sounds. Spent revolver and shotgun brass is dumped when you reload.
- **Scrap and upgrades.** Containers and creatures hold scrap. At a workbench beside any fish house, press B to fit an insulated action (never jams), extended magazine, choke or heavy loads. Fitted upgrades show on the models.
- **More props.** Shelter workbenches and stashes, supply caches, two abandoned camps with tents and cold fire pits, snowmobiles, sleds, shopping carts, flickering Frost Cola vending machines, mailboxes, street lamps, chain-link fences, ice-fishing sets on the lakes, more road signs.
- **Enemies with variety and life.** Frostfangs come in four fur patterns (plain, scarred, dark-masked, mangy), three coats and different ears and proportions; they gallop when they chase, lunge as they bite, flinch when hit, breathe glowing vapour and fall over when killed. New: the Glowmoose, a van-sized mutated moose with glowing antlers that paws the snow, bellows and charges at where you *were*. You can't outrun it, but you can dodge it, or lead it into a tree to stun it.
- **The Pip-Boy.** Tab or M pauses the game and opens a map drawn from the real terrain (contours, nuclear ice, highway, radiation zones, buildings, forest), fogged until you explore. Your position and heading, named places once found, zoom and pan, plus Stats and Inventory tabs.

## What's new in Milestone 3

The prototype now looks, sounds and feels like a game instead of a box test.

- **Real 3D art**: low-poly CC0 models from [Poly Haven](https://polyhaven.com) (barrel stoves, oil drums, tyres, crates, jerrycans, ammo boxes, medical kits, rocks, stumps, a tarp-covered car) and tiled PBR textures (snow, bark, rust, corrugated iron, planks, concrete, rock, diamond plate, snowy asphalt). Every texture gets mipmaps so it doesn't shimmer in the distance.
- **Procedural models**: snow-laden Northwoods pines and dead snags, Frostfang wolves with real bodies, heads, ears and bushy tails, rusted 1950s sedans, gabled fish houses, the Vault 143 gear door, Golden Atomic Mills silos, boulders, snowdrifts and the pipe rifle are all generated in code (`src/sim/meshgen.rs`, unit-tested).
- **A fuller map**: the old US-169 highway with plough banks, power lines with sagging wires, road signs, the Bullseye-Mart ruin with a collapsed roof and rubble, the silo complex with a catwalk, elevator tower and leaking glowing drums, scorched craters, cracked nuclear ice whose cracks pulse green, junk and boulders everywhere.
- **Sky and light**: a sky that follows the day/night clock and the weather, stars, the northern lights on clear nights, sun and moon, HDR bloom and filmic tonemapping, flickering firelight, a floodlit vault door.
- **Effects**: breath vapour, snow kicked up by footsteps, chimney smoke, fire flames and embers, radioactive motes, muzzle flashes and smoke, ejected brass, fur and blood on hits (blood stains the snow), shattering ice.
- **Pip-Boy HUD**: retro monospace font, icon bars (radiation eats a red slice off your max HP), a compass marking the vault (V) and the nearest shelter (H), scanlines, vignette, frost creeping in as you freeze.
- **Audio**: an ambient "Long Winter" music loop, a Geiger counter that clicks faster the more rads you take, fire crackle near the barrels, growling and a second howl voice for the wolves, groaning ice, tinkling shell casings. All still synthesised in code.

Asset sources and licenses are listed in [assets/CREDITS.md](assets/CREDITS.md). To re-download or regenerate the assets:

```
python3 tools/fetch_assets.py     # CC0 models and textures from Poly Haven
pip install numpy pillow
python3 tools/gen_textures.py     # generated textures, signs and HUD images
```

## Code layout

```
src/
  main.rs        App setup
  sim/           Pure game rules, no Bevy (unit-tested)
    collision.rs Circles and rectangles that push movers out
    meshgen.rs   Procedural geometry (pines, wolves, moose, cars, sheds, gear door...)
    sfx.rs       Sound rules: variants, cooldowns, mixer, music director, Geiger
    loot.rs      What containers hold; weapon finds
    moose.rs     Glowmoose behaviour: graze, stare, wind up, charge, crash
    viewmodel.rs First-person weapon pose maths (hold, aim, recoil, reloads)
    mapdata.rs   Pip-Boy map picture, landmarks and fog of war
    mipmaps.rs   Mipmap chains for loaded textures
    daynight.rs  Clock, sun/moon position, night chill
    synth.rs     Procedural sound effects and WAV encoding
    survival.rs  Body Heat, rads, health, inventory, crafting
    weather.rs   Calm / siren / blizzard cycle and conditions
    terrain.rs   Height field, lakes, shelters, radiation zones, highway
    combat.rs    Pipe rifle, jams, ray-sphere hits
    wolf.rs      Frostfang behaviour decisions
    rng.rs       Small deterministic RNG
  state.rs       Shared resources (Game, weather, messages, effect queue)
  assets.rs      Loads models, textures, font; builds materials and mipmaps
  meshes.rs      Turns sim::meshgen geometry into Bevy meshes
  world.rs       Terrain, ice, craters, loot, animated lights
  landmarks.rs   Vault 143, fish houses, silos, Bullseye-Mart, cars, road, power lines
  nature.rs      Pines, snags, rocks, drifts, shrubs, junk
  sky.rs         Sky dome, stars, aurora, sun and moon
  particles.rs   Billboard particle effects
  devshot.rs     Screenshot mode for automated previews (FMN_SHOT)
  player.rs      First-person controller, survival tick, items, respawn
  weather_fx.rs  Fog, light and snow particles
  wolves.rs      Wolf spawning, AI and bites
  combat.rs      Shooting, tracers, gun animation
  audio.rs       Plays sound effects and weather ambience
  gun.rs         First-person weapon models and animation (own camera)
  enemy.rs       Shared enemy Body (health, hit sphere) and death animation
  moose.rs       The Glowmoose
  interact.rs    Containers (E) and workbenches (B), prompts
  props.rs       Camps, vehicles, vending machines, fences, lamps, caches
  pipboy.rs      The Pip-Boy screen (map, stats, inventory)
  hud.rs         Pip-Boy green HUD and overlays
assets/          Models, textures, font, HUD images (see assets/CREDITS.md)
tools/           Scripts that download and generate the assets
```

## Possible next milestones

- A real interior for Vault 143
- Dialogue system and the first faction (the Skyfolk or the Lockkeepers' Compact)
- Save/load and false-thaw events
- Interiors you can enter (the Vault 143 lobby, the fish houses, Bullseye-Mart)
- Skeletal animation and hand models for the first-person weapons
- More enemy types (rad-crow flocks, frozen raiders) and a save/load system
- A quest or two: why did the Overseer open the door?
- Distance LODs for trees and a settings menu (shadow quality, view distance)
