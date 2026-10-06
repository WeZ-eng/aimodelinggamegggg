# MODLOG: Lands Between Dungeons

Journal for whoever continues this (human or agent). Newest first.

## 2026-10-06: first build (cloud container, no game)

**Where:** a Linux cloud container. Neither Elden Ring nor Minecraft Dungeons II is installed here, and
`melty.gg` is blocked by the environment's network policy, so the Melty tools (game_info, search_mashups,
one_click_check) have not been called yet.

**Design agreed with the user:**
- Host game: Elden Ring. Minecraft Dungeons II is the inspiration (it has no loader Melty installs and has online cross-play).
- First minute: start or load a character, and you're already in the Dungeons view. The first kill drops an artifact in a beam.
- Everything in v1: camera, cursor aim, roll, loot beams, enchantments, artifact hotbar.
- Co-op with friends via Seamless Co-op.

**Route:** native Rust DLL via ModEngine2 + fromsoftware-rs + hudhook. Considered and rejected:
- **regulation.bin edits:** the mod would have to ship FromSoftware data; the mod uses runtime IDs instead.
- **me3:** Melty installs ModEngine2, not me3.

**Facts established (from the crate source and the UM knowledge base field note on Elden Ring 1.17.1):**
- fromsoftware-rs crates.io 0.14.0 only has RVAs for exe 2.6.2.0. Git main `59fbd3b` has 2.7.1.0 (1.17.1). Pinned to that.
- Render camera: `CSCamera.pers_cam_1` (CSCam: matrix rows right/up/forward/pos, fov, aspect).
  Follow camera: `WorldChrMan.chr_cam`.
- `CSBulletManager::spawn_bullet` takes `BulletSpawnData` with private fields → mirrored layout in `bullet.rs`
  (size and alignment asserted at compile time).
- The boot gate matters (field note gotcha 1): wait for a visible game window before touching any singleton.
- `rva::get()` panics on unknown versions → `version.rs` checks first, and the mod stays inactive.
- MinGW builds pulled in `libstdc++-6.dll` (imgui-sys). Fixed with `CXXSTDLIB=static=stdc++` + static link args.
  Imports are now system DLLs only (checked with objdump).

**Assumptions the running game must confirm** (all listed by `preflight --release`, 52 cells):
1. Writing `pers_cam_1.matrix` in `DrawParamUpdate` changes the rendered frame without jitter.
2. Pinning the follow camera's yaw makes W move up the screen. If not, rotate the movement input instead.
3. Camera row 0 is screen-right (aim must not be mirrored). Row-2 sign and handedness are learnt from the
   vanilla camera at runtime and logged.
4. The game may re-centre the hidden cursor; `input.rs` switches to a virtual cursor after 30 pinned frames (logged).
5. Spawned bullets with behavior/magic -1 and goods = the item: damage and visuals look right.
6. HP writes to 0 kill normally and drop runes; `last_hit_by` identifies the player's hits.
7. `RendMan.debug_ez_draw` renders on the retail exe (loot beams). Fallback: draw beams in the overlay
   with `camera::project`.
8. Co-op: bullets and HP changes sync between Seamless Co-op players (two-copy test).

**Tests done here:**
- Under Wine: 5 unit tests pass (camera basis and projection, enchantment rules, save round trip).
- Loading the DLL into a non-Elden-Ring process: it logs "unsupported build", stays inactive, and the process keeps running.

**Next:**
1. Allow `melty.gg` in the environment's network settings → game_info elden-ring (ModEngine2 external DLL
   config, camera/key/session notes), search_mashups, then fill `release.*.destination`.
2. The user picks the content license (`release.mod_dll.license`, `release.mod_config.license`) and the remix choice.
3. Check Seamless Co-op's permissions and the build for 1.17.1 (`coop.version`, `coop.redistribution`). If it can't
   be bundled, co-op can't be one-click: discuss with the user before listing.
4. On a Windows PC with Elden Ring 1.17.1: first launch, read `%APPDATA%\LandsBetweenDungeons\log.txt`, then work
   through `preflight --release` until it's clean.
5. Real gameplay screenshot of the Dungeons view and a loot beam.
