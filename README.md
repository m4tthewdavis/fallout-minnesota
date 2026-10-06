# Fallout: Minnesota — Prototype (Milestone 5)

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

The game opens on a title screen (Continue picks up your most recent save; New Game starts at the vault door). Settings has three pages: **Graphics** (shadows, view distance, volumetric fog, ambient occlusion, frame-rate display, UI size), **Sound** (four volume sliders) and **Controls** (mouse sensitivity, invert mouse, field of view, and **key bindings**: choose an action, press Enter, then press the new key; a key that's already in use swaps over). The keys below are the defaults.

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
| Tab or M | Raise the Pip-Boy (pauses the game) |
| In the Pip-Boy: 1 2 3 | STATS / ITEMS / DATA (or click the buttons under the screen) |
| In the Pip-Boy: Q / E | Previous / next page along the bottom (or click them) |
| In the Pip-Boy: W S / arrows | Move through a list; on the map, pan (also drag; wheel zooms, C centres) |
| In the Pip-Boy: Enter | Use the selected aid item |
| F4 | Quicksave (also: pause menu, or the Pip-Boy's SAVES page: ENTER saves, L loads) |
| H / X / F | Stimpak / RadAway / Vault 143 Hotdish |
| C | Craft a Frostfang coat (3 pelts, at a fish-house shelter) |
| Esc | Pause menu: Resume, Save, Load, Settings, Quit. Esc also closes the Pip-Boy |
| E | Use a door, bunk, stove or terminal (a prompt shows what's in reach). Looking at a container opens its loot list: Up/Down or the wheel to choose, E takes the highlighted entry, T takes all |
| F3 | God mode on / off (a cheat: nothing hurts you; set `FMN_GOD=1` to start with it on) |
| G | Geiger counter on / off |
| F9 | Mute / unmute all sound |
| F10 / F11 | Master volume down / up |
| F5 / F6 | Effects volume down / up |
| F7 / F8 | Music volume down / up |

## Look and sound (Milestone 7)

- **One winter palette** (`src/theme.rs`): ice blue ink, cyan for what's selected, frost white for emphasis, deep navy panels, used by the HUD, Pip-Boy, pause menu, dialogue, loot list and XP bar. The Pip-Boy keeps its scanlines, static and flicker, now in frosty cyan, and its map is recoloured through an ice ramp.
- **The sky** is painted from a single-scattering atmosphere model: deep blue overhead, pale at the horizon, a haze that glows round the sun, orange then red as it sinks, a violet dusk and a moonlit night. It also sets the fog colour, the sunlight's colour and the ambient light, so they always agree. It's midwinter at 46 degrees north, so the noon sun only reaches 24 degrees and shadows are long all day.
- **Atmosphere effects** (both toggles in the pause menu's settings): volumetric fog and light shafts (ground-hugging banks that thicken and drift in blizzards, a faint haze at other times) and ambient occlusion (darkens creases, drifts and tree bases; SMAA replaces MSAA while it's on). Fog toward the sun glows with its light.
- **Weapons** get a normal-mapped steel texture, scanned painted-steel pipes, and hoar frost that builds on barrels, receivers and stocks in the cold (and melts indoors). Every shot throws a tongue of flame, sparks, powder smoke and condensing vapour.
- **People** (survivors and raiders) wear a shared winter kit: quilted fleece parka, belt, fur-trimmed hood, cuffs and hem, beanies, trapper hats, balaclavas and goggles, mittens, boots, packs, bandoliers, scarves and frost, in scanned CC0 cloth, fur, wool and leather.
- **Sound.** Positioned sounds fade with distance and get a muffled version when trees, walls or a doorway are in the way. Gunshots have a sub-bass boom and leave an echo that depends on where you are: a long rolling answer off the treeline outdoors, a short bright slap in a room. The pipe rifle's bolt and the revolver's hammer click after a shot. Wind is five layers (breeze, rumble, rush, hiss and a blizzard howl) and goes dull behind walls. Boots crunch harder when you run and squeak in deep cold; raiders' footsteps carry. Rooms murmur: a stove crackling and timber creaking in the fish houses, machinery hum in the vault, drips in the stockroom.
- **Loot and XP.** Look at a container and corner brackets close round the reticle while a list opens beside it: a title, tagged entries ([Ammo] Pipe rounds (12)) with the highlight lit solid, and the buttons below. Picked-up things stack up in a feed at the left. Earning XP slides up a level bar with the amount and what it was for; a new level gets a LEVEL UP banner, then the perk choice.

## Interiors

Walk up to a door and press E: the screen fades, you step inside, and the weather and daylight are swapped for the room's own light.

- **Fish houses** (four): a bunk (sleep eight hours: full health, warmed through, the storm blows over, frostbite eases, and the game autosaves), a stove (heat a hotdish for +60 Heat and +10 HP) and a stash. Entering one autosaves too.
- **Vault 143 lobby**: the heavy cog door, the Overseer's terminal desk and a locker.
- **Bullseye-Mart stockroom**: reach it by the loading-dock door in the ruin's north wall. Shelving, caches, and the military footlocker with the scrap shotgun, which used to sit out in the open.

Wolves and the moose stay out of the rooms and are frozen while you're inside. Saving inside a room puts you back inside when you load.

## More things that want you dead

- **Rad-crows** wheel in flocks over four roosts. A gunshot scatters the flock, and if you were close, the birds regroup over you and come down one at a time to peck, climb away, circle and dive again for about half a minute. They're fragile (one hit) and fall out of the sky. Crows don't count towards your kills or XP.
- **Frozen Raiders** hold the two old camps and the wrecked convoy, three or two to a fire, each with a hunting rifle, a revolver or a scrap shotgun. They shoot only what they can see: trees, walls and cars between you and them block their aim, a blizzard cuts their sight to 18 m (35 m at night), their guns jam in the cold like yours, they miss more at range and more when you run, and a badly hurt one falls back to the fire (and shoots from there). A shot from you within about 90 m brings them to look, and one raider spotting you alerts his friends. They take hits, flinch, die and leave boot prints like the other enemies, and their pockets have ammo for their gun and scrap.
- Enemies aren't saved; they return when you load. Both are paused while you're indoors.

## The quest: Why Did the Overseer Open the Door?

Play the Overseer's recording on the terminal in the Vault 143 lobby (E). It sends you to do three things, in any order (and any you've already done count):

1. **Investigate the missing supply convoy** on the old US-169 highway, west of the lake road. Search the wreck (E). Lundgren, in the first fish house, saw the trucks go by.
2. **Restore power at Golden Atomic Mills.** Rewire the breaker panel by the silos (3 scrap, E). The beacon on the tallest silo lights up. Olson has the details.
3. **Defeat the territorial Glowmoose** east of Sven's Shanty (a bigger, tougher bull than the ones that wander). Sven will tell you how.

Then report back to the Overseer, and choose what to tell the vault: the truth, or a cover story. Ole reacts to your choice. The NOTES page of the Pip-Boy (DATA tab) keeps the log and says what to do next.

Conversations are keyboard-driven: Up/Down (or W/S) to move, Enter or 1-4 to choose, Esc to leave. Survivors give a gift once.

**XP and perks.** XP comes from kills, places found, weapon mods, the coat and the quest. Each level after the first earns a perk pick, offered when nothing dangerous is near (Esc puts it off for a minute): Frost Hardy (lose heat 20% slower), Rad Resistant (30% less radiation), Quick Hands (reload 25% faster), Scrounger (+1 scrap on every scrap find), Field Medic (Stimpaks, hotdish and the stove heal 50% more).

## Saving

Five slots: Autosave, Quicksave (F4) and three manual slots, kept as `save_<n>.json` in `%APPDATA%\FalloutMinnesota` on Windows (`~/.local/share/fallout-minnesota` elsewhere; set `FMN_DATA_DIR` to change it). A save remembers where you are, your health, heat, rads, inventory, weapons and upgrades, the map you've seen, the crates you've opened and items you've taken, the time and weather, and your quest progress. Wolves and moose aren't saved: they repopulate when you load. The world itself is the same every game. Saves carry a format version; a file from a newer version of the game is refused with a message instead of being misread.

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

## What's new in Milestone 5

- **Fixes.** Sign lettering always fits inside its border (with a test in CI), and sign posts stand behind the boards. The HUD's status and weapon panels can no longer overlap at any window size. Messages and the HUD hide while the Pip-Boy is up. Cars and snowmobiles no longer float.
- **Vehicles.** The sedans are rebuilt in detail: real wheel arches, fins, roof and pillars, frosted or smashed glass, chrome bumpers, grille, headlights, door seams and handles, a licence plate, bench seats and a steering wheel, an underbody, and tyres with tread. Faded paint and rust have their own texture, normal and roughness maps. Each wreck is different: flat tyres, bare rims, a missing wheel, doors hanging open, one on its roof. Snowmobiles are one machine with a seat, cowl, windshield, a lugged track on its wheels, and skis on proper suspension. Every vehicle is fitted to the ground under its wheels, skis or track, then sunk a little into the snow.
- **Snow with depth.** A custom snow shader adds powder, wind-packed and icy-crust areas, wind ripples, scattered pits, sun glints, and hides texture tiling. The twig streaks in the old snow texture are gone. Drifts grow out of the ground behind trees, rocks and fish houses and along the highway. You leave boot prints (wolves and moose leave theirs), old animal trails cross the map, and fresh snow fills prints in. Soft contact shadows sit under trees, rocks, vehicles and buildings, plain props have grime, and calm days are clearer with stronger sunlight.
- **A Minnesota forest.** White pine, red pine, balsam fir, white spruce, paper birch, quaking aspen and tamarack, built from needle and twig cards with snow on the branches. Conifers grow in groves, birch and aspen in stands, tamarack and spruce by the lakes. Underneath: red osier dogwood, staghorn sumac, juniper, prairie grass, and cattails and reeds round every lake.
- **The Pip-Boy, New Vegas style.** A physical Pip-Boy on your wrist that raises into view, an ice-blue CRT that powers on with scanlines, curved glass, flicker and static. The pages are STATS (STATUS, S.P.E.C.I.A.L.), ITEMS (WEAPONS, APPAREL, AID) and DATA (WORLD MAP, NOTES), and the header shows level, HP, heat, rads, XP and the date. An original Vault 143 mascot shivers when you're cold, wears a bandage when you're hurt and glows when you're irradiated. It has its own sounds, and the HUD is the same cold blue to match.
- **Hands and reloads you can see.** Gloved hands and sleeves hold every weapon. The left hand pulls the rifle's magazine and seats a fresh one, then works the bolt. It loads shells into the open shotgun one at a time, and swings out and loads the revolver's cylinder. Sounds are timed to the hands. The rifle reload takes 2.2 s (was 1.6 s).
- **Fire that looks like fire.** Animated flame tongues that flicker and lean in the wind, sparks and showers, embers, smoke, and a warm glow on the snow, all pulsing with the light. One camp's fire is still burning.

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
    meshgen.rs   Procedural geometry (wolves, moose, gloves, drifts, sheds, gear door...)
    vehicles.rs  Sedans and snowmobiles, and fitting a vehicle to the ground
    flora.rs     Minnesota trees and undergrowth, and where each grows
    progress.rs  XP and level
    sfx.rs       Sound rules: variants, cooldowns, mixer, music director, Geiger
    loot.rs      What containers hold; weapon finds
    moose.rs     Glowmoose behaviour: graze, stare, wind up, charge, crash
    viewmodel.rs First-person weapon and hand poses (hold, aim, recoil, reloads)
    mapdata.rs   Pip-Boy map picture, landmarks and fog of war
    mipmaps.rs   Mipmap chains for loaded textures
    daynight.rs  Clock, sun/moon position, night chill
    synth.rs     Procedural sound effects and WAV encoding
    survival.rs  Body Heat, rads, health, inventory, crafting
    weather.rs   Calm / siren / blizzard cycle and conditions
    terrain.rs   Height field, lakes, shelters, radiation zones, highway
    interiors.rs Rooms: layout, sleeping, cooking, door fade timing
    atmosphere.rs  Sky scattering, sunlight colour, fog density field
    soundscape.rs  Occlusion, echoes, footsteps, wind layers, room murmurs
    outfit.rs    Winter clothing meshes for people
    lootmenu.rs  Loot list entries, highlight and scrolling
    quest.rs     The quest line: stages, flags, XP, perks
    dialogue.rs  Conversation graphs and the replies you can pick
    combat.rs    Pipe rifle, jams, ray-sphere hits
    wolf.rs      Frostfang behaviour decisions
    crow.rs      Rad-crow flocks: circling, scattering, diving
    raider.rs    Frozen Raider guns, sight, jams and fighting
    rng.rs       Small deterministic RNG
  state.rs       Shared resources (Game, weather, messages, effect queue)
  assets.rs      Loads models, textures, font; builds materials and mipmaps
  meshes.rs      Turns sim::meshgen geometry into Bevy meshes
  world.rs       Terrain, ice, craters, loot, lights, drifts, contact shadows
  landmarks.rs   Vault 143, fish houses, silos, Bullseye-Mart, road, power lines
  vehicles.rs    The wrecked cars and snowmobiles
  nature.rs      Snags, rocks, drifts, junk
  flora.rs       Plants the forest, undergrowth and lake shores
  snow.rs        The snow ground material (shaders/snow.wgsl)
  tracks.rs      Footprints and old animal trails
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
  pipboy.rs      The Pip-Boy (STATS / ITEMS / DATA)
  hud.rs         HUD and overlays
assets/          Models, textures, font, HUD images (see assets/CREDITS.md)
tools/           Scripts that download and generate the assets
```

## Possible next milestones

- More of the vault: the reactor level, and the replacement pump
- The first faction (the Skyfolk or the Lockkeepers' Compact)
- False-thaw events
- Raider camps to clear for loot, and follow-up quests from what the survivors said
- Crows that roost on the dead trees and leave when it snows
