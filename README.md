# Fallout: Minnesota — Prototype (Milestone 1)

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

If the build reports an error, copy the error text back to Claude and it will fix it.

## Controls

| Key | Action |
| --- | --- |
| WASD / Mouse | Move / look |
| Shift | Sprint (keeps you warmer, but cracks nuclear ice) |
| Space | Jump |
| Left mouse | Fire pipe rifle |
| R | Reload, or clear a cold-weather jam. When dead, respawn |
| H / X / F | Stimpak / RadAway / Vault 143 Hotdish |
| C | Craft a Frostfang coat (3 pelts, at a fish-house shelter) |
| Esc | Free the mouse (click to recapture) |

## What's in Milestone 1

- **Body Heat**: drains with air temperature and wind chill. Shelters, fire barrels, hotdish, sprinting and insulation warm you up. At 0 Heat you get frostbite and start losing health.
- **Radiation**: comes from the glowing snow during blizzards, the nuclear-ice lakes, a crater with a dud warhead, and the leaking Golden Atomic Mills silos. Rads lower your max HP, as in mainline Fallout.
- **Rad-blizzard cycle**: calm, then a 20-second siren warning, then the blizzard, which brings green whiteout fog, heavy snow, -35°F air, radiation and wolf hunts.
- **Nuclear ice**: lakes are fast shortcuts, but sprinting on them for about 2 seconds cracks the ice. You fall in and lose Heat and gain rads.
- **Pipe rifle**: hitscan with tracers, recoil and reloads. Below -20°F each shot has a chance to jam.
- **Frostfang wolves**: translucent fur and glowing eyes. In calm weather they wander. In blizzards they stalk and chase. Each blizzard brings a new pack. Killing them drops pelts, which craft the Frostfang coat (+0.35 insulation).
- **World**: the Vault 143 door, four fish-house shelters, pines, the Bullseye-Mart ruin, rusted cars, and loot (Stimpaks, RadAway, ammo, hotdish).

## Code layout

```
src/
  main.rs        App setup
  sim/           Pure game rules, no Bevy (unit-tested)
    survival.rs  Body Heat, rads, health, inventory, crafting
    weather.rs   Calm / siren / blizzard cycle and conditions
    terrain.rs   Height field, lakes, shelters, radiation zones
    combat.rs    Pipe rifle, jams, ray-sphere hits
    wolf.rs      Frostfang behaviour decisions
    rng.rs       Small deterministic RNG
  state.rs       Shared resources (Game, weather, messages)
  world.rs       Builds the map and handles loot
  player.rs      First-person controller, survival tick, items, respawn
  weather_fx.rs  Fog, light and snow particles
  wolves.rs      Wolf spawning, AI and bites
  combat.rs      Shooting, tracers, gun animation
  hud.rs         Pip-Boy green HUD and overlays
```

## Possible next milestones

- Collision with trees and buildings, and a real interior for Vault 143
- Dialogue system and the first faction (the Skyfolk or the Lockkeepers' Compact)
- Save/load, a day/night cycle and false-thaw events
- Replace the box models with real 3D assets (glTF) and add audio (siren, wind, wolves)
