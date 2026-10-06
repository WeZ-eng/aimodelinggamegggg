# Lands Between Dungeons

**Minecraft Dungeons inside Elden Ring.** Play Elden Ring from a Dungeons-style overhead camera, aim with the
mouse, fire Dungeons artifacts from a hotbar, and pick up enchanted loot from coloured beams, alone or with
solo for now (co-op is planned, see below).

> **Status: built, not yet tested in the game.** The DLL compiles, its unit tests pass, and it loads safely
> (an unsupported game build leaves it inactive). Nothing has run inside Elden Ring yet, and nothing is on Melty.
> `python3 tools/sheets.py preflight --release` lists every cell still to confirm in the running game.

## What a player gets

| | |
|---|---|
| **Required game** | Elden Ring **1.17.1** (Steam, exe 2.7.1.0). Minecraft Dungeons II is the inspiration only and isn't required. |
| **How it starts** | Melty starts Elden Ring through ModEngine2, **offline, with Easy Anti-Cheat off**. Elden Ring's official online play is never used. |
| **Dungeons view** | Overhead camera at 55°, fixed diagonal, zoom with `=` / `-`, pulls in under ceilings in caves and catacombs. `F5` switches to the normal Elden Ring camera. |
| **Moving and aiming** | WASD moves relative to the screen. Your character turns toward the mouse cursor when you attack (`LMB`/`RMB`) or use an artifact. `Space` is Elden Ring's own dodge-roll. |
| **Artifact hotbar** | `1` `2` `3` fire the artifact in that slot; `I` opens the satchel to slot them (`Tab` selects, then `1`/`2`/`3`). |
| **Enchanted loot** | Killing enemies can drop a Common / Rare / Unique loot beam. Walk into it for an artifact or an enchantment on your weapon or chest armour. Your first kill always drops an artifact. |
| **Co-op** | Not in this version. Seamless Co-op can't be bundled (its author doesn't allow redistribution), and Melty only publishes what installs in one click. Co-op comes in an update if LukeYui allows bundling, or through co-op built into the mod. |

### Artifacts (Dungeons artifact → the Elden Ring item it's made from)

| Artifact | Made from | Does |
|---|---|---|
| Fireworks Arrow | Giantsflame Fire Pot | Throws an exploding pot toward the cursor |
| Lightning Rod | Ancient Dragons' Lightning Strike | Lightning on the cursor (10 FP) |
| Corrupted Beacon | Comet Azur | Hold for a beam toward the cursor (12 FP/s) |
| Harvester | Explosive Ghostflame | Ghostflame burst around you (15 FP) |
| Wind Horn | Rejection | Blasts enemies away |
| Totem of Regeneration | Blessing's Boon | Heal over time for you and nearby friends |
| Soul Healer | Great Heal | Heal you and nearby friends (25 FP) |
| Iron Hide Amulet | Boiled Crab | Take less damage for a while |

### Enchantments

Weapon: Sharpness, Committed, Leeching, Swirling, Fire Aspect, Echo. Chest armour: Thorns, Cool Down,
Health Synergy. Up to 3 per item, levels I–III by drop rarity.

The mod keeps its own data in `%APPDATA%\LandsBetweenDungeons\` (one JSON file per character, plus `log.txt`).
It never edits Elden Ring's or Seamless Co-op's save files, and it ships no game files: everything it uses from
Elden Ring is read from the player's own install at runtime.

## How it's built

- **Route:** a native Rust DLL loaded by **ModEngine2**, which Melty installs for Elden Ring. It uses
  [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) (pinned to main `59fbd3b`, the first revision with
  1.17.1 support; crates.io 0.14.0 only knows 2.6.2.0) and [hudhook](https://github.com/veeenu/hudhook) for
  the ImGui overlay.
- **Design sheets are the source of truth.** `sheets/*.json` holds every system the mashup touches: camera,
  controls, artifacts, enchantments, rarity, loot, effects (vanilla data IDs), hooks, ui, coop and release. Each row
  generates one Rust struct in `mod/src/generated.rs`.

```sh
python3 tools/sheets.py preflight            # unfilled cells, broken references, rule checks
python3 tools/sheets.py preflight --release  # also everything not yet verified in the running game
python3 tools/sheets.py codegen              # sheets -> mod/src/generated.rs
cd mod && cargo build --release              # -> target/x86_64-pc-windows-gnu/release/lands_between_dungeons.dll
```

The build cross-compiles from Linux with MinGW (`rustup target add x86_64-pc-windows-gnu`, `mingw-w64`).
`.cargo/config.toml` links the C++ runtime statically, so the DLL needs only Windows system libraries.
To run the unit tests, use `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine cargo test --release --lib`.

## Credits

- fromsoftware-rs by vswarte and contributors (MIT OR Apache-2.0)
- hudhook by veeenu (MIT), Dear ImGui by Omar Cornut (MIT)
- ModEngine2 by the soulsmods team (installed by Melty)
- Seamless Co-op by LukeYui (co-op; see `sheets/coop.json` for the open bundling question)
- Row names from soulsmods/Paramdex
- Elden Ring © FromSoftware / Bandai Namco. Minecraft Dungeons © Mojang / Microsoft. No assets from either are included.
- Designed and written with Claude Code (AI).
