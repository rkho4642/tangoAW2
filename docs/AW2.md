# Advance Wars 2 support: how it works

## Shared console

Tango's original games link two emulated GBAs with a cable. Advance
Wars 2's multiplayer is hot-seat: players take turns on one console. So
tangoAW2 runs one console, and both peers simulate it identically.

`tango-backend-mgba/src/shared.rs` implements Tango's `Link` seam over one
mGBA core. Each tick it takes both players' inputs from the rollback
engine and asks the game (`SharedGame`) for:

- `merge`: the joypad word the console sees this tick.
- `before_tick`: runtime memory writes, applied before every frame
  (including during rollback re-simulation, so they stay deterministic).
- `conceal`: whether a seat's picture is blacked out. Presentation only.

The session's save and ROM are seat 0's, identical on both peers. Solo
play and replays use the same console and the same patches.

## Advance Wars 2 rules (`tango-gamesupport-aw2/src/pvp.rs`)

USA cartridge `AW2E`, CRC32 `5AD0E571`, 64 KiB Flash save.

| Address | Meaning |
| --- | --- |
| `0x030033EC` | Current army, 1 to 4. Stays set after a battle. |
| `0x03000004` | Battle scene function table. Nonzero only while a battle is loaded. |
| `0x030014E2` / `0x030014F0` | Map menu state (5 = item chosen) and cursor (4 = End). Both together mark the "Next turn" hand-off screen. |
| `0x03003FCD` | Fog of war, nonzero when on. |
| `0x02017C50` | Versus Teams record: `+0x08` army count, `+0x09` controllers (1 human, 2 computer), `+0x0D` army colours, `+0x32` cursor (two stops per army). The battle's player blocks are built from it. |
| task `0x08064E5D` in `0x03001500..0x03001A00` | Present only while the Teams screen is up. |
| `0x0200F920 + 0x88*g` | Sprite group `g`: VRAM base, palette slot, count, (tile, id) pairs. Group 1 is the emblems, ids `0x3E..=0x42`; the Teams screen loads only four. |
| `0x0203FFF0..` | tangoAW2's own state (previous joypad word, claimed buttons, invention pick, Teams-screen bookkeeping), in EWRAM the game never touches. |
| `0x03003FC2` | The Versus map being played; design maps are `0xB4..0xB7`. |
| `0x030033FC` | Title-menu mode: 1 Campaign, 3 Versus, 5 War Room. Kept through the mode's menus and battles. |
| `0x03000000` | Main-loop callback; `0x08043591` while the full-screen CO page is open (the battle scene is unloaded then). |
| `0x020232C0 + 0x3C*n` | Player block for army n+1. Colour byte at `+0x1A`: 1 Orange Star, 2 Blue Moon, 3 Green Earth, 4 Yellow Comet, 5 Black Hole. |
| `0x02028030`, `0x02028031` | Hard Campaign and Sound Room unlocked. |
| `0x02028040`..`0x02028059` | Battle Maps bought. |
| `0x0202805A`..`0x0202805F` | COs available, then CO colour edits. |

Input: in a battle, outside the hand-off screen, only the seat owning the
current army (odd armies seat 0, even armies seat 1) reaches the pad.
Elsewhere both seats' buttons are ORed.

Unlocks: every frame the unlock block is set. The game saves that block,
so an in-game save keeps it.

Armies: picked on Versus' Teams screen. SELECT or R moves the highlighted
army (cursor / 2) to the next colour no other army has, L to the previous
one; the game then builds the battle's armies from the Teams record, so
nothing is forced during play and Campaign and War Room are untouched.
Black Hole's emblem (sprite `0x42`) is not
loaded on that screen, so while an army is Black Hole its emblem is drawn
into the tiles of a standard emblem no army uses, and all standard
emblems are restored from ROM every frame.

Concealment: with fog on, during a battle, the seat that does not own the
current army sees a dark screen, except on the hand-off screen.

Sources: Xenesis' RAM notes and hacking threads on Wars World News, the
libretro CodeBreaker list, the aw2bhr decompilation, and probing with
`gba_probe`. `aw2_rollback_sim` checks the whole flow under rollback.

## Design Room (`tango-gamesupport-aw2/src/design.rs`, offline only)

Mode 8 with sub-mode 5 (`0x03003FC1`) is the map editor; its state block
is at `0x0200B000` (tool bar open at `+0x04 == 2`, bar type `+0x07`,
cursor `+0x08/+0x0A`, terrain tool `+0x2A`, colour slots `+0x2E/+0x2F`,
first visible bar entry `+0x36/+0x38`, bar entries at `0x0200B224`, moved
by tangoAW2 to `0x0203FF00`, see below). The
editor map is at `0x0201E450` (size, camera at `+4/+6`, tiles `+0xA22`,
classes `+0x1432`, units `+0x12`, row offsets `+0x417A`); tile classes come
from the ROM table `0x080C1BC4`.

- Black Hole is a fifth army in the editor (`design5.rs`), with five's
  player table (`0x02030000`, see Five armies) and its patches on while
  the editor runs. On entry the editor's four player blocks are copied
  there and player 5 is set up as colour 5 with Flak, so its units and HQ
  have Black Hole's designs (`0x08042DE0`: CO -> country). The bars' army
  cycle goes to 5 (`0x08006C10/32`, `0x08006CC0/E2`), properties of owner
  5 take classes `0xA6..0xAE` (tiles `0x1B4..0x1B8`, `GetDefaultTileForTerrain`
  at `0x080012DC`, owner table repointed to `0x08660000`), units take ids
  `205..254` (`0x0800894E`, `0x08008B8A`), and saving writes army 5's
  units as `0xE0 | type` (`0x0803D09C`), read back at `0x0803D28C`.
  A map with army-5 content carries the five-army mark (5) in its spare
  colour byte `0x03003FF3[0]` (saved at record `+0x4C4`); the save's army
  count (`+0x4C3`) stays 4 for the Versus list and the property totals
  are put right (`0x0803CF7A`). The HQ panel draws the four armies'
  emblems as an X around Black Hole's (OBJ tiles 532..535, the lanes'
  palette 4; the lanes are double-size affine sprites, drawn 8 pixels in).
  A plain's look comes from the cell to its left (`sub_08001704`, jump
  table for classes `0x03..0x8E`); owner-5 classes are looked up as owner
  4's (`0x08001750`), so Black Hole's buildings cast the same shadow.
  Maps saved by older versions with Black Hole in slot 4 (mark 4) still
  start that army as Black Hole on the Teams screen.
- In a tool bar SELECT only swapped bars, like L/R; tangoAW2 turns a SELECT
  press into UP (next army, Black Hole included).
- Inventions in the terrain bar (`design_bar.rs`): the bar's list is built
  from the template `0x08488810` by `sub_080078E4` into 17 (terrain) or 20
  (units) 4-byte entries (type or unit word, tile placed) at `0x0200B224`,
  and the HBlank buffer follows at `0x0200B274`. tangoAW2 repoints the nine
  literal-pool words for the list (`0x08001CFC`, `0x08001D58`,
  `0x08001D88`, `0x080062B8`, `0x08006340`, `0x08007750`, `0x08007844`,
  `0x080078D0`, `0x08007918`) to `0x0203FF00`, patches the terrain list's
  length 17/16/0x44 to 27/26/0x6C, or 29/28/0x74 with the Crystal and
  Obelisk, where the editor wraps it (`0x08000CEA`,
  `0x08001D4E`, `0x0800626E/72`, `0x080062E6`, `0x08006460/68`,
  `0x08006562`, `0x08007798/9C/9E`), all in the ROM image in memory, and a
  trap at the builder's exit (`0x080079B2`) inserts the ten inventions
  (types `0x15..0x1E` with their anchor tiles) and the Black Crystal and
  Black Obelisk (words `0x115` and `0x11A`: a minicannon's and a Black
  Cannon's type plus bit 8, which the editor's own code masks off) after
  the Silo. The editor keeps only the picked class (`+0x2A`), so the word
  picked last is kept at `0x0203FF7C` to tell a Crystal from a minicannon.
  The bar's icon call (`0x080027A6`) still has the whole word in r6 and its
  name call returns at `0x08002998` with it in r3; traps there give the two
  their own picture and name. Icons: the
  bar's sprite loader `sub_0803F6BC` already loads a type's terrain-panel
  picture (`0x08104464 + 0x100*(type-1)`) for types it has no case for;
  traps at its entry and exit (`0x0803F7FE`, r4 kind, r5 dest) remap the
  colours from the panel palette (`0x08106864`, entry 13; the base under
  every icon, entry 2) to the palette the icon is drawn with, which a trap
  on the palette lookup's return (`0x08001D20`) picks (the Volcano's is the
  editor's mountain palette, 7). Names are the game's own. A on the map
  with an invention picked places its footprint.
- Inventions drawn in the editor (`invention_art.rs`): in battle they are
  only sprites (their map tiles draw as plain; the metatile table
  `0x080BFBC4` maps them to grass), placed each frame with Black Hole's unit
  palette `0x080D3E84` at priority 3. The sources: minicannons and laser
  raw at `0x080D02C4..0x080D07C4` (16x32, one tile above the cell); Black
  Cannons LZ77 at `0x080D24E0` (down, tiles 36..71) and `0x080D2AE8` (up,
  0..35; the Deathray uses it too), 48x48 from four sprites; Black Factory
  `0x080D22C4` (48x64, three sprites); Volcano `0x080D3268` (one 64x64,
  palette `0x080D3EC4`); the Crystal and Obelisk are tangoAW2's own art.
  In the editor tangoAW2 loads them into OBJ tiles 289..535 (the editor
  uses 536 up) and palette 2 (never used by the editor) and appends each placed
  invention's sprites at the game's VBlank sprite flush (`0x0801BBC4`; the
  frame's list at `[0x03002F2C]` inside the area described at
  `0x03000268`). Everything does not fit in those tiles, but a map has
  either the Black Factory or the Volcano, so only that one is loaded. On a
  Volcano map the Volcano borrows palette 15 (used by the editor only on
  its save and load screens; the game's own is put back off the map), so
  palette 2 stays Black Hole's.
- The aw2bhr decompilation (`src/design.c`) names most of the editor's
  drawing: `sub_08002844` (unit icon: `sub_080261A4(slot, kind)`, CO-country
  based) and `sub_0800272C` (terrain icon; HQs via `sub_0803F6BC(8, army)`).
- Buttons tangoAW2 takes are hidden from the game for as long as they
  stay held (`0x0203FFF6`), so a held press never reaches the editor as a
  new one.

## Inventions in battle (`tango-gamesupport-aw2/src/factory.rs`)

- Inventions list: `0x02028360`, 8 bytes each (x, y, tile, HP, ...,
  counter). The Deathray's counter (byte 6) counts 7..1 and it fires on
  Black Hole's turn when it wraps; its area (`0x0801FCE0`) is columns
  x..x+2 from row y+3 to the map's bottom edge, enemies only. The laser
  hits every unit in its row and column.
- Units: 12-byte records, army n (1-based) at `0x02022390 + 0x300*n`
  (type, state, x, y, HP|flags, ammo, fuel, ...). Create-unit is
  `0x08025C5C` (x, y, type), wrapped by `0x08025CC8`. Player blocks are
  `0x02023284 + 0x3C*army` (colour +0x1A, 1 human / 2 AI at +0x1B, unit
  count +0x3A).
- The factory spawner `0x080607E8` finds the factory (`0x0803E354(7)`)
  and, for each of the three tiles on the row y+4 that is empty, creates
  the unit type at `table[(day & 0x1F)*3 + i]` (0 = none), the table
  pointer being `[0x030046B4]`. Only the AI turn setup calls it
  (`0x08061900`, colour 5), after storing the map header's table pointer
  (`0x08615194 + (map-0x8A)*0x30 + 0x24`, hard `+0x28`). Design maps have
  no header entry: the computer got garbage types, a human got nothing.
- tangoAW2 traps `0x080618AE` (right after that store) to give design maps
  Factory Blues' table (`0x08576F23`), and `0x08026810` in start-of-turn
  (after the per-player turn-start call, scratch registers dead) to detour
  a human Black Hole army's turn through the spawner once; `0x0203FFFC`
  marks the detour so the return passes. A trap handler runs before its
  instruction.
- The battle loads the Volcano's colours (`0x080D3FC4`) into sprite
  palette 12 (`0x0803FE0A`), the fourth army's buildings' palette: fine in
  the campaign, but a Versus map with a Volcano and Yellow Comet drew
  Yellow Comet's buildings in them. In Versus `volcano.rs` sends them to
  palette 2 (unused by the battle map) and recolours the Volcano's sprite
  at the sprite flush.
- `aw2_script` can trace this kind of thing: `AW2_TRACE=<file>` traps
  listed addresses and logs registers, `stepuntil8 ADDR` single-steps
  until a byte changes and prints the last instructions, `steplog N`
  prints every function entry for N instructions.

## CO panel on design maps

- The CO panel's palettes are loaded at the start of each turn from the
  army's colour: BG row 8 from `0x080D4188 + (colour-1)*32` (`0x0801A548` ->
  `0x0802D5CC`) and OBJ row 7 (palette row 23, the header with the funds)
  from `0x08104264 + (colour-1)*32` (`0x08043834`). On a design map the
  first turn's are loaded before the colours are held to the Teams pick,
  so the current army's rows are swapped from its map colour to its pick
  where they still hold the map colour's.

## Title and menu badge (`tango-gamesupport-aw2/src/branding.rs`)

- A "tangoAW2" badge, with the app's version on a plate under it (read
  from `tango/Cargo.toml` at build time), drawn by the game's sprite hardware while
  `ProcScr_TitleScreen` (`0x08581CF8`) or `ProcScr_MainMenu` (`0x0849E818`)
  is running (the process pool is `sProcArray`, `0x0200D610`, 0x6C bytes
  each, script pointer first; names from aw2bhr): its tiles in unused OBJ
  tiles 928.. (title) and 992.. (menu), its colours in OBJ palette 15, its
  64x32 sprites appended at the VBlank sprite flush like the Design Room's
  inventions.

## Five armies (`five.rs`, `five_map.rs`, `five/`)

Advance Wars 2 has room for four armies. For the 5P maps tangoAW2 adds a
fifth, Black Hole, by patching the ROM image in memory while a 5P map is
played (and restoring it otherwise, so every other game runs the game's own
code). The patch list is `five/patches.txt`; `five/gen.py` checks every
original instruction against the ROM and writes `src/five_patches.rs`.

- **Unit ids.** One byte, army = `id >> 6` (64 per army, 50 used). For five
  armies army a owns ids `(a-1)*51 + 1..50`; about 150 places that encode
  or decode the army (shifts, masks, `(p - gUnits)` pointer chains, 64-id
  walks, the AI's `(t*128 + u)*4` pointers, the army-base table
  `0x084995FE`) are patched. Hooks are emulator breakpoints whose handler
  sets the register and skips the instruction.
- **Moved to free RAM with a fifth slot.** The player table (pointer word
  `0x08499598` -> `0x02030000`), the AI's threat maps (`0x02029ED8` ->
  `0x02031000`), the Teams screen's record (`0x08580934` -> `0x02030300`,
  its per-army arrays at +0x90..).
- **Loops and tables.** About 80 army-count bounds, the turn wrap, vision
  (an unrolled 1..4 call gets a detour for 5), capture tiles (six owners per
  kind; Black Hole's property tiles `0x1B4..0x1B9`), owner bits, building
  sprites (army 5 on OBJ palette 13; fogged buildings use the neutral
  palette), army 5's units on BG palette 11 (the game's neutral-unit palette; no
  neutral units exist, and 1 is the pipes'), the Black Hole HQ art in the building sheet's lab slot.
- **Screens.** The Teams screen (five 48-px columns, `5P`, `E Team`), CO
  screen, Intel, results, capture-limit panel. The map menu hides Save (the
  suspend block holds four armies).
- **Maps.** Map-table entry 0 and ids `0xB8..0xBF` (design ids only
  multi-cartridge link uses; the game's design range is narrowed to
  `0xB4..0xB7` and the rest made ordinary by edits and a helper in dead
  code). Each map's header carries its own tab and armies, so the same
  ids also hold the 2-, 3- and 4-army obelisk maps; the 5-army ones are in
  category 9, the 5P tab. `five/design_maps.py` draws them,
  `five/map.py` builds them (sea edges from the Design Room's table
  `0x08485DC4`). The AI keeps a row pointer per map row in a 40-entry stack
  array, so maps are at most 40 rows.
- **More map ids.** The game's map table (`0x085C77A0`, 0xC0 entries of
  0x5C bytes) is copied to `0x08650000` with room for more; its 37
  literal-pool pointers (the table, and +0x3C/+0x40 of entry 0) are
  repointed, and the two loops that walk it (`sub_080206B0`, find a map by
  its tiles; the map list builder at `0x08037482`) go up to 0xC0 instead
  of 0xBF. Black Rampart is id 0xC0.
- **Terrain in `five/map.py`.** Roads, pipes and pipe seams pick their
  tile from which neighbours connect, as the game's own maps do (learned
  from every built-in map): roads have straights, bends, T-junctions,
  crossroads and shaded variants; pipes have straights, bends and end caps
  (no junctions).
- **Switching.** A RAM flag set when a 5P map is picked (trap on
  `sub_0803BCD0`) decides; each frame the ROM is switched to match, so
  rollback (which restores RAM, not ROM) stays deterministic. Resuming a
  suspended game switches the patches off first.

## Black Crystal and Black Obelisk (`obelisk.rs`, `five/obelisk_art.py`)

Dual Strike's healing structures, built on two of the game's inventions so
the game registers, targets and destroys them: the Crystal is a
minicannon (class `0x15`, invention kind 4) and the Obelisk a Black Cannon
(class `0x1A`, kind 3, 3x3 over `0x1A4` underlay). Each has its own map
tile, `0x192` and `0x193` (unused by the game; tangoAW2 sets their classes
in the ROM table `0x080C1BC4` and its RAM copy `0x020233B0`, and their
metatiles to plain grass), and every trap tells them apart by that tile,
so real minicannons and Black Cannons run the game's code unchanged.

- **Sprites.** `sub_0803F908(x, y, def, army, fog)` places a building's or
  invention's sprite; called from `0x0803FB92` (minicannons) or
  `0x0803FD08` (Black Cannons, defs `0x0849FA08`/`0x0849FA22`) on our
  tiles it gets tangoAW2's definitions at `0x08640000` (ROM image free
  space): the Obelisk is a 32x64 sprite over the middle of its 3x3 rect and
  two 8x32 strips for its platform's sides, the Crystal one 16x32. Their 48
  tiles go to OBJ tiles `0x176..0x1A5` after the building sheet loads
  (`0x0803F6A0`); nothing in battle uses those.
- **Art: from the player's Dual Strike ROM** (`ds_art.rs`). tangoAW2 ships
  none of it. The library scan offers every file in the ROMs folder; a
  Dual Strike (USA, AWRE) ROM gives `bmap/015` of its file system (LZ77;
  4bpp bitmaps, rows of pixels: the Crystal 16x32 at 0x1600, the Obelisk
  64x64 at 0x3F00 with its footprint at x 8..56, y 0..48) and sub-palette
  12 of `bmap/00e` (Dual Strike's Black Hole palette, each colour mapped to
  the nearest of AW2's `0x080D3E84`). The scan saves the result next to
  the ROMs (`Dual Strike Black Obelisk art.tangoaw2`, 1672 bytes) and
  loads that on later scans, so the `.nds` is needed once.
- **Hidden without the art.** `ds_art::features(mode)`: played alone, when
  this player has the art; for a netplay match or its replay, as the
  match says: each player sets bit 7 of their match subtype when they
  have it (`SHARED_CONTENT`, tango-net-protocol), the lobby ignores the
  bit when comparing match types, and the terms keep it only when both
  set it, so both peers and every replay agree. Off, the four maps sit on
  a tab no list shows (`five_map::show_obelisk_maps`) and the Design
  Room's bar is 27 entries long instead of 29 (`design_bar::patch_rom`).
  A console without the art in a match that has it (someone else's
  replay) draws the structures as nothing; the sprite layout is the same
  either way, so nothing in RAM differs.
- **No firing.** The turn-start loop over the invention list
  (`0x02028360`, 16 entries of 8 bytes: x, y, kind in bits 6..9 of the
  halfword at +2, HP at +4) is trapped per entry at `0x0803ED7A` (r2) and
  skips ours (`0x0803EEAC`); the range display (`sub_0803E9F8`, entry r5)
  skips to `0x0803EAC4`.
- **Healing.** A trap at `sub_0803EAD0` (turn start, before inventions
  act): if the army moving now has colour 5 (Black Hole), each of its
  units within 2 of a live Crystal or 4 of a live Obelisk's footprint gets
  +20 (of 100) HP, as in Dual Strike, capped at 100, and full ammo and fuel from the
  unit table `0x085D5ABC` (+0x0B, +0x10). Only that army's own unit ids are
  walked, so enemies and allies are never healed.
- **Panel.** The terrain panel (`sub_0802A8DC`, cell in r8/r5) gets the
  name picture at `0x0802A914` and the picture at `0x0802A982`.

## The Dual Strike pack (0.3.0)

With a Dual Strike (USA) ROM imported (`ds_pack.rs`: its ARM9, overlays and
files, saved next to the ROMs), Dual Strike's content is added to AW2. Nothing
from either game is in the repository; everything is read from the player's
ROMs at run time. Without the pack the game runs byte-identical to 0.2.2 (the
comparison battery in CONTRIBUTING.md), and online it is on only when both
players have it (the match's `SHARED_ART` flag, kept in replays).

Every change is switched every frame from ROM and RAM state (`pvp.rs`
`before_tick`), so both peers and every replay agree: tables are copied to free
ROM and their literal-pool words pointed at the copies, and code is changed by
traps (a trap runs before the instruction it replaces; setting the PC skips it).

| Part | Module | What it changes |
|---|---|---|
| Units | `roster.rs`, `ds_units.rs`, `unit_actions.rs`, `oozium.rs`, `unit_names.rs`, `ds_unit_art.rs`, `ds_unit_pictures.rs`, `ds_battle.rs`, `ds_backdrop.rs`, `map_anim.rs` | Unit table grown to 64 rows (0x08680000), 7 new units (ids 4, 9, 12, 13, 18, 26, 27), Dual Strike's stats and damage chart, their actions (Hide, Explode, Repair, Carrier; the Oozium eats: no weapon, moving onto a unit of another team next to it destroys that unit with the game's own destruction, and no CO, power, silo or Black Bomb touches it), map art, their own information pictures (build menu panel, R on a unit) in each army's colours, every unit in the Intel unit list, battle scenes with Dual Strike's figures, effects and volleys, Dual Strike's battle backgrounds (a Piperunner on its pipe; every battle on a Wasteland map; a Com Tower's city), and Dual Strike's map animations played through AW2's own map effects (a Black Bomb's explosion, a Stealth hiding and appearing, a Black Boat's REPAIR label, Oozium's death in its army's colours; for the CPU at its turn's end, before the turn passes) |
| COs | `co_roster.rs`, `co_new.rs`, `co_powers.rs`, `ds_co_art.rs`, `ds_power_art.rs`, `power_anim.rs` | CO table grown to 96 rows (0x086A0000), Dual Strike's numbers for AW2's COs (and its 200% defence cap), 9 new COs at ids 72..80 (face ids stay unambiguous), their pictures, texts, powers and Dual Strike's power animations (Ex Machina, Covering Fire, Urban Blight), and Dual Strike's choice of power effect on their units |
| CO screen | `co_grid.rs` | The unit grid (map menu > CO, its last page) gets a second page: ground units, then air and naval units, in the build menus' order, every unit with its icon in the viewed army's colours (the new units in the map sheet's slots for other countries' Infantry and Mech) and its firepower bar (Dual Strike's bonuses take the nearest of AW2's 13 bars) and move / range change |
| CPU | `cpu_tactics.rs` | The CPU buys every new unit (Carrier, Oozium and Piperunner in place of a like AW2 unit at its three `BuyUnit` calls), explodes Black Bombs, hides Stealths, repairs with Black Boats, eats with Ooziums (and moves them towards enemies), and leaves Ooziums out when it aims a silo or a strike |
| Terrain | `com_tower.rs`, `wasteland.rs`, `sandstorm.rs` | Com Tower (the Versus Lab), the Wasteland look, the Sandstorm weather (Dual Strike's sand, `bmap/0b2`) |
| Structures | `obelisk.rs`, `heal_effect.rs` | Black Crystal / Obelisk heal with Dual Strike's own animation for each (arm9 0x0213E078 / 0x0213E2A0), the camera visiting each |

Free ROM used: 0x08620000.. (text slots), 0x0862C000.. (new CO text ids 0x6D72..),
0x08640000..0x08672FFF (earlier features), 0x08680000..0x08691FFF (units),
0x086A0000..0x086AFFFF (CO table), 0x08740000..0x0877FFFF (CO pictures, texts,
powers' code, heal wait), 0x087C0000..0x087C0FFF (power animations),
0x087C1000..0x087C3FFF (map animations), 0x087D0000..0x087DFFFF (unit pictures),
0x087F0000..0x087F4FFF (CO screen grid: the map sheet per country, the page lists).
Free RAM used: 0x0203F740..0x0203F79F
(map animations), 0x0203F7A0..0x0203F7DF (power animations), 0x0203F800..0x0203F9FF (battle
scenes), 0x0203FD60..0x0203FEFF (CPU tactics, heal effect, the Oozium's eat
0x0203FDC8..0x0203FDFB, stun, battle distance, Teams list),
0x0203FF00.. (earlier features). `factory.rs` has a test that no two traps share
an address.

Com Towers (`com_tower.rs`, `design_bar.rs`): a Lab is a Com Tower in Versus
and in the Design Room. In battle its sprite comes from `gProperty`
(0x03003150, the buildings `RecountArmyProperties` lists; `sub_0803F990` draws
them), so the tower stays in that list; only in the editor, which tangoAW2
draws the towers in itself (OBJ tiles 524..531) and where the towers skip the
editor's property bookkeeping, is a Lab left out of it (trap `0x08021C4C`;
0.3.0 and 0.3.1 left it out in battle too, and no tower was drawn on the
battle map). Capturing one never ends the battle (`0x0804281E`). In the
terrain bar the Tower entry (word `0x14 | owner << 5`, tiles `0x1D9..0x1DD`,
Black Hole's `0x1B9`) is one of the editor's properties: the editor's "is a
property" (`sub_0800C7E8`) answers 1 for a Lab to the bar's and the Feature
panel's twelve calls (checked by return address; its map-cell callers
`sub_0800C840`/`sub_0800C608` are left alone), so UP, DOWN and SELECT change
the bar's army on it (neutral, the four armies, Black Hole) and it is redrawn
in that army's colours. The army change is committed by `sub_080077EC`
(`0x0800701A`), which rewrites the list's five property entries (9..13) and
then places them among the shown entries (`0x0200B0D0`, 0x1C each, word at
+4) by the highlighted word's kind; a trap after the list is rewritten
(`0x0800782A`) gives the tower's list entry the new army too, and with the
tower highlighted writes only its shown entry and returns (`0x080078C2`),
since the game's placement has no case for a Lab. The list builder's trap
(`0x080079B2`) builds the tower for the builder's army (r5). A on the map
places the picked tool's army (`0x0200B02A`, class | owner << 5). 0.3.1 kept
its own army for the tower entry, which the shown entry, the picked tool and
the placed tile did not follow: the bar showed and placed the army it was
opened with.

The Oozium (`oozium.rs`, the Dual Strike rule): it has no weapon (no Fire, no
counter-attack). Its move takes in the squares next to it holding a unit of
another team, any unit (air units and ships in port too) on a square it can
enter; moving onto one and choosing Wait destroys that unit with the game's
destruction (`sub_0804018C`: explosion, units lost, rout), charges both power
meters as a battle in which the victim lost all its HP, and counts a unit
destroyed for the eater's army. A hidden unit on that square is eaten too
(no Trap!). The CPU eats with its Ooziums as its turn starts (the most
valuable unit next to one) and moves the others a square towards the enemy.
Nothing from a CO changes an Oozium, as in Dual Strike, whose CO stat
functions (arm9 0x020E5678, 0x020E57AC, 0x020E5A58, 0x020E5C40, 0x020E5D8C)
return 0 for its unit class (6) before the power's +10 defence (player +0x28)
is added, and whose CO blocks' unit filters (block +0x34: 0x020E25E8,
0x020E2610) leave class 6 out: no CO stats (not even a power's +10 defence,
nor Com Towers' or Javier's), no power's damage, stun or fuel loss, no
repair, move-again or resupply; a Missile Silo's and a Black Bomb's blasts
spare it, and the CPU's silo and strike scoring leave it out.

`tools/aw2test` plays real battles in both modes and checks them against its
own damage calculator (Dual Strike's numbers read from the .nds in `ds` mode),
fires every CO's COP and SCOP, and replays netplay runs on two rollback peers.

## Known limits

- Black Hole's unique buildings (Black Cannons and so on) are map
  features; ordinary Versus maps do not have them. Build them in the
  Design Room.
- Campaign and War Room are single-player. Netplay is Versus only.
- The CO screen's unit grid shows what AW2's shows: firepower, move and
  range. Defence (Javier's, Grimm's) and terrain firepower (Koal, Jake,
  Kindle) are not on it; AW2 has 13 bar lengths, so Dual Strike's other
  bonuses show the nearest (below -30 as -30, above 80 as 80).
- The game's mini maps (the map list's preview, the editor's overview)
  have colours for four armies; Black Hole's buildings show there as
  neutral grey. The editor's Intel screen counts the four armies and
  neutral, not Black Hole.
- Com Towers in the Design Room are not counted in the editor's "Surplus"
  (its 60-property limit) nor on its Intel screen (cities, bases, airports
  and ports only); the overview map shows them in their army's colour
  (Black Hole's dark).
