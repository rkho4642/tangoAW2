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
| `0x02028030`.. | AW2's campaign flags 0x20.. (a bit each; `IsCampaignCompletionFlagSet`): 0x20 Hard Campaign, 0x28 the Sound Room, 0x21 the campaign won, 0x23..0x26 set by its missions; 0x60.. from `0x02028038`. |
| `0x02028040`..`0x02028059` | Battle Maps bought. |
| `0x0202805A`..`0x0202805F` | COs available, then CO colour edits. |

Input: in a battle, outside the hand-off screen, only the seat owning the
current army (odd armies seat 0, even armies seat 1) reaches the pad.
Elsewhere both seats' buttons are ORed.

Unlocks: every frame the unlock block is set. The game saves that block,
so an in-game save keeps it. Hard Campaign and the Sound Room are campaign
flags 0x20 and 0x28, bit 0 of `0x02028030` and `0x02028031`: only those bits
are set, the bytes' other flags (0x21 the campaign won, 0x23..0x26 its
missions', ...) are kept (until 0.4.0 the whole bytes were written as 1,
and those flags were lost at every save; `save_keeps_aw2_completion_flags`).

Armies: picked on Versus' Teams screen. R moves the highlighted
army (cursor / 2) to the next colour no other army has, L to the previous
one (without the Dual Strike pack SELECT does as R, as in 0.4.0; with it
SELECT opens the Set Skills panel); the game then builds the battle's
armies from the Teams record, so nothing is forced during play and
Campaign and War Room are untouched.
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
  palette), army 5's units on BG palette 11, the Black Hole HQ art in the building sheet's lab slot.
- **Unit colours.** The map draws army n's units with BG palette 11 + n and
  moved units with BG 11 (icon palette entry 0), where `sub_0801A57C`
  loads the current army's grey (`0x0810E6E0 + (colour + 4) * 32`) at each
  turn start and after a power's portrait or the battle scene. Every BG
  palette is in use on the map (0-7 terrain and its fog shades, 8 the CO
  panel, 9 the turn banner and power portrait, 10 windows), so with five
  armies the grey goes to BG 9 (the banner shows before any unit has moved;
  the power portrait covers the map, and the game reloads the grey after
  it), Black Hole's colours go back in 11 after each grey load, and a moved
  unit's draw puts the grey back in 9 if the banner left its colours there.
  Before (0.3.4) Black Hole's units took the current army's grey: Green
  Earth's tint on Green Earth's turn, red-brown at Orange Star's, and moved
  units took Black Hole's colours. Colour 15 (the outline, pulsing while a
  CO power is on) is set every frame by `sub_08024720` for armies 1..4;
  army 5 is added (row 11). The CO power dialog and portrait load the CO's
  face colours into BG 9 and bring the map back without redrawing it, so
  the grey goes back when their proc (`0x0848A3EC`) ends. The turn banner's
  colours (`0x080A1238`) exist for four colours only (colour 5 read past
  the table: a near-black stripe); Black Hole's turn gets a purple one in
  the same layout (`five.rs` `BANNER5`). Tests:
  `tests/test_five_unit_palettes.py`.
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
  of 0xBF, and to 0xC8 while the Dual Strike maps can be listed (eight more
  ids to walk shift the menus' timing by a frame, so without the pack the
  loops stay as they were). Black Rampart is id 0xC0, the Dual Strike maps 0xC1..0xC8 (see
  below). A tab lists its maps by id, so new maps come last. Each map's
  tiles and units take 4 KiB: the first ten at `0x08622000`, the rest at
  `0x08655000` (after the moved table).
- **Terrain in `five/map.py`.** Roads, pipes and pipe seams pick their
  tile from which neighbours connect, as the game's own maps do (learned
  from every built-in map): roads have straights, bends, T-junctions,
  crossroads and shaded variants; pipes have straights, bends and end caps
  (no junctions). Rivers (`-`), bridges (`=`) and shoals (`,`) take the tile
  the game's own maps use most for the same neighbours (sea, shoal, river,
  bridge or road on each side); roads join bridges.
- **Switching.** A RAM flag set when a 5P map is picked (trap on
  `sub_0803BCD0`) decides; each frame the ROM is switched to match, so
  rollback (which restores RAM, not ROM) stays deterministic. Resuming a
  suspended game switches the patches off first.

## The Dual Strike maps (`five/design_ds_maps.py`)

Eight Versus maps for the Dual Strike pack, drawn by `five/design_ds_maps.py`
(called from `design_maps.py`) into `five/maps.txt`, built like the others by
`five/map.py`. Every Com Tower on them starts neutral.

| Id | Map | Armies, size | Tab | Look | What is on it |
|---|---|---|---|---|---|
| 0xC1 | Rust Basin | 2P, 25x17 | Vs. | Wasteland | a river with three bridges down the middle, a sea ring joining the two bays, beaches, a pipe along each coast, 2 Com Towers, 2 Black Crystals |
| 0xC2 | Dune Fork | 3P, 29x22 | 3P | Wasteland | a river forking between the three armies, bridges, three bays joined by a sea ring, pipes, 3 Com Towers (one between each pair of armies), 3 Crystals |
| 0xC3 | Cinder Flats | 4P, 29x29 | 4P | Wasteland | four corners inside a sea ring, rivers from four bays to a Black Obelisk ringed by 4 Crystals and 4 Com Towers, pipes |
| 0xC4 | Black Wastes | 5P, 29x29 | 5P | Wasteland | four corners round Black Hole's fortress: a Black Cannon facing north and one facing south, 2 Lasers, 4 minicannons, 4 Crystals; rivers, lakes (no ports: they would not reach each other), pipes, 4 Com Towers on the axes |
| 0xC5 | Coral Strait | 2P, 27x17 | Vs. | AW2 | two islands across a strait inside a sea ring, bridged to a middle isle with 2 Com Towers (and one on each island); each side's two bases sit on a pipe that runs through the sea to the middle isle |
| 0xC6 | Trident Isles | 3P, 29x20 | 3P | AW2 | three home islands round a middle isle with 3 Com Towers and a beach north and south, a pipe from a base on each island to it |
| 0xC7 | Harbor Cross | 4P, 29x29 | 4P | AW2 | four corner islands inside a sea ring, a pipe from each one's base across the channel to the middle isle, its 4 Com Towers and 4 beaches |
| 0xC8 | Coral Crown | 5P, 29x29 | 5P | AW2 | four corner islands and Black Hole's middle island, a Com Tower on each of the four islets between them, reached by the corner islands' pipes |

- **Fair.** The 2P maps turn about their centre, the 3P maps are mirrored
  left to right with army 3 on the middle line (the three HQs about as far
  from each other), the 4P and 5P maps are mirrored both ways (Black Hole,
  army 5, in the middle of the 5P ones). On a map every army but Black
  Hole starts with the same properties (HQ, two bases, an airport, ports,
  cities) and units (two Infantry, a Mech, a Recon, a Piperunner on its
  pipe, and on the Wasteland maps a Tank and Artillery, on the sea maps a
  Lander and a Cruiser); Black Hole's middle holds fewer, with its fortress
  or its ports, and a Piperunner base too.
- **Everyone gets everywhere.** Checked with the game's own movement chart
  (`tools/aw2test/aw2test/traverse.py`): foot, treads and tires reach every
  enemy HQ and Com Tower overland or by Lander (beach or port to beach or
  port; foot every property too), ships reach every enemy port (bridges
  stop ships, so the seas meet round a ring of sea where needed), no base
  or port is boxed in, and every Piperunner has something in range on its
  pipe.
- **Beaches where they matter.** On the sea maps beach stretches are placed
  by hand (`beach` in `design_ds_maps.py`, mirrored with the map): landings
  facing each neighbour's island, the Com Tower isles and the middle isle,
  and near each HQ, with cliff coast between so a beach is worth holding.
  The test checks that the armies the symmetry maps onto each other get as
  many beaches and an enemy beach as far from their HQ (`beach_balance`;
  a 3P map's army 3, on the mirror line, is matched to the other two by
  hand; Black Hole in the middle of a 5P map is left out).
- **Piperunner bases behind seams.** Each army's Piperunner base touches
  its pipe only through a pipe seam (in `five/map.py` a seam's straight
  run may end at a base: `B Z I I`). A base offers the Piperunner only
  while a pipe or an intact seam is next to it: the build menu
  (`sub_0802D5E8`, trap at `0x0802D65E` in `cpu_tactics.rs`: the pipe
  domain bit is dropped from the base's mask) as the CPU's buying already
  did. A broken seam is rubble (walked on like plain, never a pipe), so
  breaking the seam ends that base's Piperunners, and no Piperunner
  crosses it.
- **With the pack only.** Com Towers, Piperunners and the Wasteland look
  are the pack's, so these maps (`ds` in `five_map_data.rs`) are listed
  only when the pack is on (for a match, when both players have it) and the
  Crystal's art too; otherwise they sit on the hidden tab like the obelisk
  maps without their art (`five_map::show_maps`), so without the pack every
  map list is as before.
- **The Wasteland look** (`wasteland: true`): at every map start
  (`wasteland::map_start`) a tangoAW2 Wasteland map sets the biome to
  Wasteland, any other map that is not a design map (0xB4..0xB7) to Normal.
- The Crystals on the 2P-4P maps heal a Black Hole army (a player who picks
  Black Hole on the Teams screen); by default no army is Black Hole there,
  and they are only obstacles to shoot at.
- `factory.rs` gives these maps Factory Blues' factory table too (the map
  header table the computer's turn reads has no entry for them).
- Tests: `tools/aw2test/tests/test_ds_maps.py` opens each map from its tab
  (the list's preview checked tile by tile), checks every tile, owner and
  unit against `five/map.py`'s build and the traversal check on the map in
  play (and on the build, `ds_maps_traversal_static`), photographs the whole
  map (screenshots stitched as the cursor sweeps it), plays six all-CPU days
  on each (two in netplay), and checks the maps are not listed without the
  pack; `test_pipe_seams.py` checks the build menu and the CPU against intact
  and broken seams.

## Dual Strike's looks (`wasteland.rs`, `ds_look.rs`)

Dual Strike draws a map in one of four looks: Normal, Snow, Desert and
Wasteland. With the pack, Wasteland is a per-map setting of design maps (the
Design Room's Waste entry) and the look of tangoAW2's Wasteland Versus maps;
Survival's maps and the DS Campaign's missions bring their own look (Dual
Strike's look byte, `wasteland::set_ds_look`). Normal stays AW2's own; Snow,
Desert and Wasteland are drawn with Dual Strike's own terrain graphics,
converted from the .nds at run time.

- **Dual Strike's terrain.** Two tilesets of 736 tiles: `bmap/000` (Normal,
  Snow) and `bmap/001` (Desert, Wasteland), coloured by the look's palette
  file (`bmap/006` Normal, `00a` Snow, `008` Desert, `009` Wasteland; arm9
  `0x02167DD4` names them per look): 9 sub-palettes, 0-4 terrain, 5 one grey
  ramp all fog uses, 6-8 buildings (with per-look building colours,
  `0x02147F40`..). The metatile table (arm9 `0x02143F40`) is laid out as
  AW2's (AW2's tile ids, four quadrants each); a second table (`0x02145F40`)
  holds what reaches into the cell above (mountain peaks, treetops, roofs),
  drawn on a second layer (`0x020F6E74`). Every mountain cell is drawn as one
  of three mountains by position (`0x020`, `0x146`, `0x147`: table
  `0x02169E58` at `(x + x/4 + 2y + y/8) & 15`, `0x020F6F34`); woods are
  `0x086`/`0x087`. The tile layout is AW2's: `0x100..0x1FF` the sea's,
  `0x200..0x25F` the river's; the Normal tileset's frames are `bmap/004`
  (4 sea frames) and `bmap/005` (8 river frames), laid out as AW2's own
  (`0x080C1FC4`, `0x080C9FC4`). The `bmap/001` looks have no frames of their
  own in the ROM.
- **Conversion** (`ds_look::build`, once per look at the first frame with the
  pack). Per AW2 metatile: cells AW2 draws as plain under a building's sprite
  (every metatile with plain's quadrants; the Black Crystal's and Obelisk's
  `0x192`/`0x193`; army 5's properties `0x1B4..0x1B9`) get Dual Strike's
  plain; mountains get Dual Strike's mountain for the cell's position, woods
  its two woods by id parity; AW2's bridges `0x13`/`0x14`/`0x36`, which Dual
  Strike lacks, its bridges `0x15`/`0x16`, and AW2's plains with the peak or
  treetops of the cell below drawn in (`0x03`, `0x43`, `0x106`, `0x107`,
  `0x126`, `0x127`) its plain (the peak comes from Dual Strike's upper part
  instead, below); everything else Dual Strike has (sea, shoals, reefs,
  rivers, roads, pipes, seams, ...) its own metatile. What it lacks (89
  metatiles: some road corners, the class-0 strips `0x200..0x245`, `0x280`,
  `0x282`) keeps AW2's tiles, each AW2 colour taking the look's colour it
  most often lands on where both games draw a metatile; these are static
  (they live in the tiles never animated). The volcano rim's upper part
  (`0x1A5`) is not drawn (AW2 draws the volcano as a sprite).
- **Peaks and treetops.** A mountain's upper part (its bottom 4 rows) and a
  wood's (2 rows) are drawn over the bottom half of the cell above, as Dual
  Strike's second layer does, whatever that cell is (plain, road, another
  mountain, a wood, sea...), and not past the map's top edge. AW2 has one map
  layer, so tangoAW2 draws the cells itself while a Dual Strike look is on:
  traps at `BlitMapRow` (`0x08023BAC`) and `BlitMapColumn` (`0x08023A4C`)
  draw the row or column the game asked for into BG3's tilemap buffer and
  return (the game's own code otherwise). A cell under which a mountain or
  wood stands gets composite tiles: its bottom quadrant with the upper part
  over it, in the palette of the 4 that draws it best (`Look::composite`).
  Composites go into the tiles no metatile uses (about 230 per look): one
  already there is used again, else the first one no tilemap entry uses; all
  the bookkeeping is VRAM and the tilemap buffer, so it is the same on every
  console and after a rollback. A sea or river quadrant under a peak keeps
  its first frame.
- **Animation.** The Snow look uses `bmap/004`/`005` as they are. Desert and
  Wasteland derive theirs: each pixel of the look's tile equal to the Normal
  tileset's frame-0 pixel follows the Normal frames, the rest (shores and
  banks in the look's own shapes) stays. AW2's own timing plays them
  (`UpdateTerrainAnimation`).
- **Colours.** AW2 has 4 terrain palettes (BG 0-3; 4-7 the same darkened for
  fog); Dual Strike's terrain uses 5. They are grouped into 4 by trying every
  partition and keeping the one with the least mean colour error per tile
  (frames and upper parts included), each group cut to 15 colours by folding
  together the two closest colours (the less used goes into the other: the
  colours stay Dual Strike's own, and one unlike the rest, a wood's green,
  stays). Mean colour error per pixel is under 1.5 (squared, 5-bit
  channels) on every metatile. BG palettes 8-15 are untouched. Fog, rain and
  snow sets come from the clear set by AW2's own colour relations
  (least-squares fits of AW2's clear set to its fog, rain and snow sets); the
  Snow look keeps its colours in snow weather. Sandstorm blows sand over the
  clear set.
- **Roads.** Dual Strike's Wasteland and Desert roads are faint tracks;
  `ds_look::ROAD_SHADE` (0: Dual Strike's own) draws every road this many
  5-bit steps darker (the darker colours go into the palettes with the
  rest), and the tests follow it.
- **Drawing.** Each look's data goes in the ROM image's free space
  (`0x08E80000 + 0x20000 * (look - 1)`: metatiles, the tiles as LZ77, sea
  frames, river frames, the clear, rain, snow and sandstorm sets). The game
  reads its terrain through nine literal-pool words (the tiles
  `LoadGameplayGraphics` decompresses, `0x080234D8`; the metatiles
  `BlitMapColumn`/`BlitMapRow` draw from, `0x08023B08`, `0x08023C6C` and
  their `+6` words `0x08023B1C`, `0x08023BA8`, `0x08023C80`, `0x08023D10`;
  the frames `LoadSeaAnimFrame`/`LoadRiverAnimFrame` copy, `0x08021D94`,
  `0x08021DCC`). `wasteland::sync` points them at the look's data while one
  is drawn and at AW2's otherwise, from RAM alone, at each reader's entry
  (traps `0x08023360`, `0x08023A4C`, `0x08023BAC`, `0x08021D64`,
  `0x08021DA0`) and every frame. With the pack off nothing is written and
  the game draws its own map. `sub_08035020`'s trap (`sandstorm.rs`) gives
  the look's colour set for the weather.
- **The Design Room.** A on the map with the Waste entry switches Normal and
  Wasteland: the next frame the colours, the terrain tiles in VRAM and the
  map's tilemap (`RenderMap` done in Rust) are the new look's.
- **Kept AW2's.** Buildings and structures (sprites in AW2; Dual Strike draws
  them in its map layers), the battle backgrounds of Desert and Snow
  (Wasteland's are Dual Strike's, `ds_backdrop.rs`), the terrain panel's
  pictures, the mini maps.
- Tests: `tools/aw2test/aw2test/looks.py` renders Dual Strike's own drawing
  from the .nds (lower layer, upper layer into the cell above, the mountain
  by position, the frame on screen, the weather's and fog's colours) and
  compares the terrain layer read back from VRAM (BG3's tilemap, tiles,
  palette RAM: no units, cursor or windows) cell by cell, within the colour
  reduction's tolerance (mean squared error 6 per pixel). `test_biome.py`
  (each look's data metatile by metatile; the screen in clear, rain with fog,
  snow and sandstorm, at several animation frames; the Design Room's switch
  and scrolling), and whole maps swept with the cursor and checked at every
  view: tangoAW2's four Wasteland Versus maps (`test_ds_maps.py`) and every
  Survival map in a Dual Strike look (`test_survival.py`); every DS Campaign
  mission's first view (`test_ds_campaign.py`).

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
  a tab no list shows (`five_map::show_maps`) and the Design
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
- **Heal animation and sound** (`heal_effect.rs`, with the pack). Dual
  Strike's animation (arm9 `0x0213E078` / `0x0213E2A0`) is drawn as
  sprites in tiles the map leaves free and in OBJ palette 15 (saved and put
  back). Not palette 8: building sprites use OBJ palette 8 + owner, so 8 is
  the neutral (and fogged) buildings' palette, and the effect's colours
  there turned those buildings dark while it played. It starts two frames
  after the turn-start loop reaches the structure, once the camera has
  stopped (the camera sets off a frame later). With its first frame comes
  Dual Strike's sound: its animation start (`0x020D84B8`) plays sequence
  175 `SE_BLACKSTONE` (Crystal, `mov r0, #0xAF` at `0x020D85D8`) or 176
  `SE_BLACKCRYSTAL` (Obelisk, `0x020D86E0`) through `0x0200B76C`, one note
  that plays its whole sample (1.9 s, 3.1 s). `ds_music.rs` converts them
  as songs 514 and 515 (after the themes) for player 2 (the player AW2's
  turn-start cannon shot, song 457, uses), priority 10, no reverb; the
  note without a length gets its sample's length (a `TIE`, then `EOT` and
  `FINE` after it), and the sequence its default tempo (120). The draw
  marks the song pending in RAM (`0x0203FDAC`); the turn's wait function
  (called by the game each frame while it waits) plays it with AW2's own
  sound-effect call `sub_0803B4DC`, so the game's sound flag
  (`0x030005CC`) applies, and the music is untouched. A Crystal's sound
  is cut short (by about 0.15 s) when the next structure's starts on the
  same player. Tests: `tools/aw2test/tests/test_heal_sound.py` (2, 4 and 5
  armies, human and CPU, fog on and off, normal and Wasteland looks: each
  sound starts on the effect's first frame, the music plays on, and no
  colour or sprite tile but the effect's changes while it plays, beyond
  what the game animates itself).
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
| CPU | `cpu_tactics.rs` | The CPU buys every new unit (Carrier, Oozium and Piperunner in place of a like AW2 unit at its three `BuyUnit` calls), explodes Black Bombs, hides Stealths, repairs with Black Boats, eats with Ooziums (and moves them towards enemies), and leaves Ooziums out when it aims a silo or a strike; a base builds Piperunners (for the CPU and in the build menu) only by a pipe or an intact seam |
| Terrain | `com_tower.rs`, `wasteland.rs`, `ds_look.rs`, `sandstorm.rs` | Com Tower (the Versus Lab), Dual Strike's Wasteland, Desert and Snow looks drawn with its own terrain (below), the Sandstorm weather (Dual Strike's sand, `bmap/0b2`) |
| Structures | `obelisk.rs`, `heal_effect.rs` | Black Crystal / Obelisk heal with Dual Strike's own animation and sound for each (arm9 0x0213E078 / 0x0213E2A0; SE 175 / 176), the camera visiting each |
| Music | `ds_music.rs` | The nine new COs' own map themes, Dual Strike's, converted to AW2's sound engine (below) |
| Maps | `five_map.rs`, `five/design_ds_maps.py` | Eight Versus maps (2P to 5P, a Wasteland set and a sea set) with Com Towers, Piperunner pipes and Black Hole's structures (above) |
| Survival | `survival.rs`, `survival_maps.rs`, `mode_menu.rs` | Dual Strike's Survival mode (Money, Turn, Time) on its own 33 maps, a seventh entry on Select Mode (below) |
| DS Campaign | `ds_campaign.rs`, `ds_campaign_data.rs`, `ds_campaign_rules.rs`, `campaign_menu.rs` | Dual Strike's story campaign in AW2's campaign engine, behind a Campaign sub-menu (below) |

Free ROM used: 0x08620000.. (text slots), 0x0862C000.. (new CO text ids 0x6D72..),
0x08640000..0x08672FFF (earlier features; the map table and the maps past the tenth at
0x08650000..0x0865CFFF), 0x08680000..0x08691FFF (units),
0x086A0000..0x086AFFFF (CO table), 0x08740000..0x0877FFFF (CO pictures, texts,
powers' code, heal wait), 0x087C0000..0x087C0FFF (power animations),
0x087C1000..0x087C3FFF (map animations), 0x087D0000..0x087DFFFF (unit pictures),
0x087F0000..0x087F4FFF (CO screen grid: the map sheet per country, the page lists),
0x08800000..0x08D2FFFF (music, past the 8 MB cartridge: mGBA grows the image when it is written),
0x09000000.. (the DS Campaign's story songs, their own range so the music above never runs into the
Survival and campaign data),
0x0862D000..0x0862D0FF (Survival's text ids 0x7172..), 0x08E00000..0x08E4FFFF (Survival: the map
table with room for 0x100 ids, its maps, strings, the Select Mode wheel's data),
0x0862DA38..0x08630A37 (the DS Campaign's text ids 0x7400..0x7FFF),
0x08F00000..0x08FFFFFF (the DS Campaign, about 360 KB used),
0x08E80000..0x08EDFFFF (Dual Strike's looks: 0x20000 each for Wasteland, Desert, Snow).
Free RAM used: 0x0203FA00..0x0203FD0F (Survival), 0x0203FD10..0x0203FD5F (DS Campaign), 0x0203F600..0x0203F6FF (the DS
Campaign's records; 0x0203E000..0x0203F73F was found unwritten at the title, Select Mode, in AW2 and DS battles), 0x0203F740..0x0203F79F
(map animations), 0x0203F7A0..0x0203F7DF (power animations), 0x0203F800..0x0203F9FF (battle
scenes), 0x0203FD60..0x0203FEFF (CPU tactics, heal effect, the Oozium's eat
0x0203FDC8..0x0203FDFB, stun, battle distance, Teams list),
0x0203FF00.. (earlier features). `factory.rs` has a test that no two traps share
an address.

The new COs' music (`ds_music.rs`): each new CO's turn plays its own Dual
Strike theme, converted at run time for AW2's sound engine (MP2K, "Sappy").

- **Which theme.** Dual Strike's CO record (arm9 `0x0215360C + 0x220*id`)
  names the CO's map music at `+0x14`, a sequence of `data/sound_data.sdat`
  (whose symbols confirm it: Jugger `BGM_ZIPO1` 36, Koal `BGM_CHAKKA1` 39,
  Kindle `BGM_CANDLE1` 27, Von Bolt `BGM_HAGEVOLT1` 38, Grimm `BGM_KOUZOU1`
  34, Javier `BGM_BITTMANN1` 40, Sasha `BGM_SASHA1` 37, Jake `BGM_JOHN` 5,
  Rachel `BGM_RACHEL1` 24; no two share one). The pack keeps those
  sequences (SSEQ), their banks (SBNK) and sample archives (SWAR) as
  `sound/seq/<id>`, `sound/bank/<id>`, `sound/wave/<id>` (about 4 MB;
  with the Crystal's and Obelisk's heal sounds, 175 and 176, nothing else
  of the 18 MB archive), and the DS Campaign's 15 story songs
  (`STORY_SONGS`) and the staff roll's stream (`sound/strm/0`). The pack's
  version is 5 (3: the heal sounds, 4: the story songs, 5: the stream): a saved pack older than 3 is rebuilt from the .nds on the
  next scan (or ignored without it); a version 3 pack still loads, its
  story songs stood in for by AW2's like ones, so both netplay peers with the pack have the same sounds. Power music stays AW2's
  (Dual Strike's is shared too).
- **Sequence.** Each SSEQ track is walked (calls inlined, loops and jumps
  followed; the jump back is the loop) into timed notes and controls, and
  written as an MP2K track that plays the intro, then the loop, then GOTOs
  back. Dual Strike counts 48 ticks a beat, MP2K 24: the song's TEMPO byte is
  the full beat count (not half), so every tick is kept (tempo accurate to
  0.2%). Volume, expression and the sequence's volume are Dual Strike's
  squared curves made linear (`VOL`), velocities likewise; pan, pitch bend
  (halved to MP2K's range), modulation carry over; notes longer than `N96`
  are `TIE`d and ended by `EOT`. The themes' music player (`0x03005AE0`,
  songs with player 1) has 8 tracks: songs with 9 to 11 tracks (Jugger,
  Koal, Kindle, Von Bolt, Grimm, Sasha, Rachel) have the tracks that sound
  together least merged (VOICE/VOL/PAN switched before the other track's
  notes).
- **Instruments.** Each program played is an MP2K rhythm voice (type
  `0x80`, 128 sub-voices) so every key plays its own region's sample at its
  own root (sub-voice key `60 + key - root`); region pan is a forced pan.
  Dual Strike's PSG square and noise (Kindle's theme) play made-up
  DirectSound samples (a square of the region's duty, an LFSR noise), so
  the themes never use the GB sound channels AW2's sound effects share.
  Samples (IMA-ADPCM, 22 kHz) are decoded, low-passed and resampled to AW2's
  mixing rate (13379 Hz; a loop keeps a whole number of samples and the
  rate follows it) and stored 8-bit; one copy per sample. Envelopes: Dual
  Strike steps every 5.2 ms in decibels, MP2K once a frame in linear
  amplitude: decay and release become the per-frame factor for the same
  dB fall, sustain the squared level, attack the step that reaches full in
  the same time.
- **Where.** From `0x08800000`: a mark, AW2's song table (`0x0824238C`, 505
  entries) copied with the nine new songs after it (ids 505..513, player 1),
  then voice groups, samples and tracks (about 5 MB). The table's six
  literal-pool words (`0x080704A0`, `0x080704D4`, `0x08070520`,
  `0x08070574`, `0x080705A8`, `0x08072B9C`) are switched while the pack is
  on, and the new COs' rows name their song (row `+0x04`). The game starts
  a CO's music with `sub_0803B524(id)` (id at `0x030005CA`).
- **Channels.** Dual Strike plays up to 16 notes at once; AW2 mixes 8
  DirectSound channels (SoundInfo `[0x03007FF0]+6`) of the 12 the engine
  has (the other 4 sit unused before the PCM buffer at `0x03004AE0`). While
  one of the new themes is the music (player `0x03005AE0`'s song), 12 are
  mixed, 8 otherwise (the extra 4 stopped when it goes back).
- **Compromises.** Notes past 12 at once are cut by MP2K's channel
  stealing; merged tracks share one volume and pan at a time; the samples
  are 8-bit at 13 kHz; Dual Strike's portamento, tie mode, sweep and
  per-track envelope changes (none used by these nine) are not converted;
  each loop sets its tracks' state in full where it starts, so its first
  pass starts from the state later passes have;
  the per-song level is Dual Strike's, scaled once (`GAIN`) to sit with
  AW2's themes.
- Tests: `tools/aw2test/tests/test_co_music.py` (every new CO's turn, human
  and CPU, starts its song; Von Bolt's loops, survives his power and a
  battle scene, and plays in a five-army game; AW2's COs and songs
  unchanged; without the pack nothing changes). `aw2_script`'s `audio` /
  `audioend FILE` record the console's sound (`Emu.audio_start/audio_end`).

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

In a five-army battle the Lab's tiles hold Black Hole's HQ, so the tower is
drawn from the Crystal's battle tiles (OBJ 0x19A..0x1A1; the Obelisk's
0x176.. with a Crystal on the map; `com_tower::after_sheet`, after the sheet
loads and every frame). A five-army game is on from the moment its map is
picked, and on the Teams screen tiles 400..435 are the first column's face:
0.3.0 to 0.4.0 wrote the tower over 410..417 there (a band across the face's
eyes, with the pack, on 5P maps and five-army design maps). The tiles are now
taken only while they hold what `obelisk::load_tiles` put there (the
editor's 524.. as before). Tests: `tools/aw2test/tests/test_teams_faces.py`
(each column's face tiles against its CO's face in the ROM image, every CO
of the list in the first and fifth columns, 2 and 5 armies, straight from
the title and after Survival and the Campaign box; and the five-army
battle's tower picture).

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

## Survival (`survival.rs`, `survival_maps.rs`, `mode_menu.rs`)

Dual Strike's Survival mode, with the Dual Strike pack, offline. Everything
below about Dual Strike was read from its code and data (the USA ROM; overlay
0 is loaded at `0x022AD560`, the survival state is `0x022A7B80`).

**Dual Strike's Survival, as found.** Three kinds (the state's kind byte, +7):
Time (0), Money (1), Turn (2); 3..5 are the Champion courses of the same three.
Each basic run is eleven maps fought in a row against the computer, the same
eleven every time, from a list per kind (u16 map ids, overlay 0 `0x022F64FC`
Time, `0x022F652C` Money, `0x022F6514` Turn; `sub_020EAC50` picks the list):

| # | Money (500,000 G) | Turn (99 days) | Time (25 minutes) |
|---|---|---|---|
| 1 | Silo Sweep | Convoy Cape | Red Heart |
| 2 | Bad Pangaea | Cannon Land | Frozen Pipes |
| 3 | Chokepoint | Mr. Fix-It | Cape Splinter |
| 4 | Cold Shoulder | Aircraft Hunt | Lake Fever |
| 5 | Crowded Plain | Rain of Pain | Open Road |
| 6 | Narrow Road | Lone Wolf | The Middleman |
| 7 | Triple Threat | Fenced In | Stealth Fight |
| 8 | The Gooping | River Raid | Tactical Decoy |
| 9 | Single File Isle | Crystal Field | Last Stand |
| 10 | The Swarm | Five Mile Isle | Pursuit Plains |
| 11 | Grit's Gambit | Forest Frenzy | Fog Hunter |

- **Budgets** (arm9 `0x02168D04`, a word per kind): 90000 frames, 500000 G,
  99 days; Champion 108000, 600000, 120.
- **Running out** (`sub_020EA944`, every frame of a battle): Money, the
  player's funds reach 0; Turn, the day passes what is left; Time, the
  player's own clock (it only runs on the player's turns) reaches what is
  left. Then the run is over.
- **What a cleared map costs** (`sub_020EA87C`): Money, the funds spent;
  Turn, the day it was won on less one; Time, the player's time in whole
  seconds. Money has no income at all: properties, joined units' extra HP,
  Colin's and Sasha's powers earn nothing.
- **Points**: each map's battle score added up (at most 9999); a cleared run
  adds a bonus for what is left (`sub_020EB024`): a point per 2 seconds, 10
  per day, 1 per 200 G. **Rank** (`sub_020EAD98`) by what is left: S from 9
  minutes / 50,000 G / 25 days, A from 6 / 25,000 / 15, B from 3 / 10,000 / 5,
  else C. **Records** (`sub_020EAE84`): per kind, the best leftover with the
  run's two COs.
- **COs**: picked once for the run (a tag pair in Dual Strike: the survival
  state's +0/+1, copied into the player at every map start, `sub_020EACC4`).
- **Maps**: Dual Strike's own Survival maps, in AW2's map format (Dual Strike
  grew AW2's map header to 0xA0 bytes, overlay 0 `0x022DBDB0`, id n at n - 1;
  tiles are AW2's LZ77 blob of AW2 tile ids; units 13-byte records). Each
  brings its fog (+0x34), weather (+0x33: clear, snow, rain, sandstorm), look
  (+0x32: normal, snow, desert, wasteland), the computer's COs (+0x70 army 2,
  +0x72 army 3) and its pre-deployed units.

**In tangoAW2.**

- **Select Mode** (`mode_menu.rs`): a seventh entry on the wheel, SURVIVAL,
  between War Room and Battle Maps, with labels built at run time from the
  game's own label art (the letters of DESIGN ROOM, VERSUS, BATTLEMAPS,
  CAMPAIGN and LINK, in a teal of Campaign's and Link's colours) and a help
  line. Hook points (for merging other Select Mode work): every `DivRem(x, 6)`
  of the wheel's code (`movs r1, #6` before `bl 0x0808AAB0` in
  `0x08080F00..0x08084C00`, found by scanning) becomes 7; the position wraps
  `0x08081DF0`, `0x08081E2C`, `0x0808280C`, `0x08082830`;
  `SetMainMenuCarouselPosition` `0x08080F68` ((i + 5) % 7) and `0x08080F7E`;
  the 34 words pointing at the item table `0x0861696C` point at a 7-entry copy
  `[5, 8, 2, 4, 3, 1, 0]` (item 8 is Survival); help lines: the table
  `0x08616FA4` (pool `0x08084680`) and the choices' table `0x08616FB4` (pool
  `0x080846C0`) are copied with Survival at line index 6 (Sound Room 7, Hard
  Campaign 8: `0x08084630`, `0x0808464E`; Campaign's index `0x0808464A`).
  Traps: `0x080845A8` (centre label), `0x08084858`, `0x08084864`,
  `0x0808488C` (tile complete, palettes), `0x08081498` and `0x08081E94`
  (item 8 goes like Link: A starts it), `0x08081EEE` (picked) and `0x08081EF6`
  (the mode store: War Room New, 5). The small label sits in OBJ tiles
  0x320.. with OBJ palette 13; `remap` (at the sprite flush) points the game's
  item-8 sprites (tile `0x1D8 + 32 * 8`, palette 10) at them. The wheel is the
  game's own with the pack off (every patch put back, nothing traps). One more
  entry would need its own OBJ palette and tiles and the table, wraps and
  `DivRem` sites for eight.
- **Screens.** Picking it opens the War Room's own screens on Survival's maps:
  SELECT MAP lists Money, Turn and Time Survival (the highlighted kind's
  budget and record in a panel under the preview), the War Room's CO screen,
  LET'S GO, the battle, the War Room's results and its save prompt; back on
  SELECT MAP only the run's next map is listed (with what is left and the
  points), until the run is cleared or lost: then the results (kind, CLEAR! or
  GAME OVER, maps cleared, what is left, bonus, points, rank) cover the
  preview until A or B. From the second map on, the CO screen offers only the
  run's CO (one group with that CO, as the campaign's restricted CO screens
  build theirs: trap `0x0807C588`). B on SELECT MAP leaves Survival (a run in
  progress is given up).
- **Maps** (`survival_maps.rs`): the 33 maps are read from the pack and
  converted at the first frame with the pack: ids 0xC9..0xCB are the three
  runs' entries (each the run's first map, named after its kind), 0xCC..0xEC
  the maps in Dual Strike's id order. Tiles are AW2's own ids except Dual
  Strike's Black Crystal (0x1A1 -> tangoAW2's 0x192), Com Towers (0x1B9..0x1BD
  -> the Labs 0x1D9..0x1DD) and its other two mountains (0x146, 0x147 ->
  AW2's mountain 0x022, as the DS Campaign's maps: Dual Strike draws every
  mountain cell as one of 0x020, 0x146, 0x147 by position). Their 4x4 structures (Convoy Cape, Lone Wolf,
  Silo Sweep) are AW2's own tiles, as on AW2's T Minus 15 and Sea Fortress,
  and are drawn as a sprite whose picture the map header names
  (`tileGraphic4x4`, +0x10, loaded by `LoadInventionGraphics`
  `0x0803FD80`): Dual Strike's header names its picture at +0x24, a `bmap`
  file ("0a5" the missile pad, "0a6" the fortress), byte for byte AW2's
  `0x080D2DA8` and `0x080D38AC`, which the converted header names. (The
  first Survival left +0x10 empty, and those three maps showed whatever OBJ
  VRAM held there, a CO portrait among it.)
  Units: Carrier and Oozium become tangoAW2's 26 and 27; every unit gets the
  AI byte tangoAW2's Versus maps use (4). Headers: AW2's 0x5C bytes, with the
  map's fog, armies, colours, computer COs (Dual Strike's ids to tangoAW2's),
  speed rank day limit, on a tab of their own (0x0A).
- **Map table.** With the pack the game reads tangoAW2's copy of the map table
  with room for 0x100 ids (`0x08E00000`; the 37 words of
  `five_map.rs` switch to it, and five_map
  writes its tabs into whichever table is in use); the map list's loops walk
  to 0xEC while Survival is on. The War Room lists tab 0x0A instead of 7 while
  Survival is on (`BuildMapListForMode`'s table, `0x08090EF2`), and its map list
  draws only the rows the list has (`DrawMapList` trap `0x08086A58`; the War
  Room never had fewer than seven maps).
- **Rules per map** (at every map start, `sandstorm.rs`'s trap `0x08035490`):
  the weather as fixed weather (sandstorm as tangoAW2's fixed sandstorm), the
  map's look (Snow, Desert and Wasteland drawn with Dual Strike's own terrain,
  `wasteland::set_ds_look`; Normal is AW2's), the run's CO, and for Money the funds (the
  pool) with no income (`propertyFunds` 0, and any funds gained are taken
  back every frame), and the map's fog (the War Room sets gPlaySt's fog from
  the header of the map it opened with, `sub_080346FC`, before one is
  picked).
- **The budget in battle**: the map number and what is left (funds, days left
  today included, or the time as m:ss) at the top of the battle map, in AW2's
  own glyph font (OBJ tiles 0x3C0.., palette 0, as `PutAsciiGlyphSprite` draws).
  Out of budget, the player's army yields (`unk31`, as the map menu's Yield
  does) and loses at the game's next rules check.
- **End of a map** (`EndOfGame_Finish`, trap `0x0803832C`): won, the map's
  cost is taken off, its score added, the next map listed; the last map
  cleared works out the bonus, rank and record. Lost, the run is over.
  The War Room keeps its records by `id - 0x6C`: while Survival is on its
  record readers and writer (pools `0x0808759C`, `0x08087664`, `0x08087B18`,
  `0x08087C6C`, `0x080177E4`) read a zeroed block of ours; `SetMapPlayed`
  (`0x0803CA28`) skips Survival's ids (its bits stop at 0xBF). The map menu
  hides Save on a Survival map (`0x0802C646`): a suspended map would come back
  without its run. The War Room's end of a map asks its save question with
  the War Room's suspend slot (`sub_0803D73C(3, ..)`), which clears the
  profile's "War Room game saved" flag and deletes slot 3 when it saves; a
  Survival map asks with slot 6 instead (the prompt's "profile only", as
  `sub_0803D960` uses it; trap `0x0803D746`), so a War Room game saved
  halfway survives a Survival run (until this, the first Survival map ended
  took it away; `save_survival_keeps_war_room_suspend`).
- **Records** in the profile the game saves (so the save's own checksum covers
  them): `0x0200C435..0x0200C43E`, ten of the eleven bytes between
  `0x0200C420`'s +0x14 and +0x20 that no code of the game reads or writes
  (`PackProfileRecord` saves 0xE0 bytes from `0x0200C420`; a test checked the
  bytes stay untouched through boot and battles): a mark (0xD5), then three
  bytes per kind (Time, Money, Turn): rank (3 bits), CO (7), what was left (14:
  seconds, hundreds of G, days). Kept when a run is cleared with more left than
  the record.
- **RAM**: `0x0203FA00..0x0203FA3F` (the run: on, kind, maps cleared, phase,
  left, budget, points, the map's time, the funds cap, CO, the menu's pick,
  bonus, rank), `0x0203FA40..0x0203FD0F` (the War Room record rows Survival
  reads). **ROM**: `0x08E00000..0x08E05BFF` (map table), `0x08E08000..`
  (map data, 0x800 per map), `0x08E30000..` (strings), `0x08E40000..0x08E40FFF`
  (the wheel's data and labels); text ids 0x7172.. (pointers at `0x0862D000`).

**Compromises.**

- AW2 has no tag battles: the run keeps one CO (the one picked for the first
  map) where Dual Strike keeps a pair; the record keeps that CO.
- The Champion courses (endless, unlocked by clearing the basic ones) are not
  included.
- A run cannot be suspended mid-map, and turning the console off loses a run
  in progress (Dual Strike saves its survival state); records are saved.
- The War Room's CO screen colours the player's army by its CO's country and
  moves a computer army off that colour, as it does for its own maps; two
  armies Dual Strike gives one colour (Single File Isle's allies) get two.
- Points are AW2's War Room scores (its Speed, Power and Technique), not Dual
  Strike's.
- The results and records are a panel of AW2's glyph font over SELECT MAP, not
  a screen of their own.

Tests: `tools/aw2test/tests/test_survival.py` (Select Mode with and without
the pack, each kind's first map checked tile by tile, unit by unit and for
fog, weather, look and colours against the .nds directly
(`aw2test/survival.py`), every one of the 33 maps likewise in battle with its
structure's picture named in its header and loaded into OBJ VRAM, the budget carried to map 2 and the CO kept, losing
each kind by running out, a cleared run's rank, bonus and record, saved and
read back after a reboot, every army a CPU for days on six maps, nothing of it
without the pack).

## DS Campaign (`ds_campaign.rs`, `ds_campaign_data.rs`, `ds_campaign_rules.rs`, `campaign_menu.rs`)

**The campaign model** (`campaign_model.rs`). The engine (`ds_campaign.rs`:
sessions, the map table entry, flags, saves, the world map's flow, the
mission end, the staff roll's flow, Hard and records) plays a `Model`, not
Dual Strike: the missions compiled for AW2 (`Built`: map headers, maps,
deployments, AW2 event scripts and trigger lists, texts, magic stubs, per
mission its `MissionInfo`), how many missions there are (at most 32), the
play order, the side missions with the flag that opens each, the last
mission, the story (prologue, scenes after wins), the staff roll, the
narration pictures, and the rules (`Source::rules`: what the scripts' magic
functions answer). A `Source` (label, available, load, rules) loads its
campaign into a model; `SOURCES` lists them, and the Campaign sub-menu
lists AW2's own campaign and then each source whose campaign is there, any
number of them (two rows show at a time; UP and DOWN go through them all,
wrapping, the window following the cursor). Dual Strike's is today's only
source (`ds_campaign_data::load`, with `ds_worldmap.rs`, `ds_story_art.rs`,
`ds_credits.rs`, `ds_music.rs`). A custom campaign is another source: its
missions laid out with `Built::add`/`Built::add_magic` in AW2's event
format, its order and rules its own (it shares the engine's flows: the
save, the world map, the staff roll, Hard, records).

Dual Strike's story campaign, played in AW2's own campaign engine, with the
Dual Strike pack, offline. Everything of Dual Strike's is read from the
player's .nds and converted at run time; none of it is in the repository.
Names of AW2's functions and data below are the aw2bhr decompilation's
(`github.com/Mad-Man-Dan/aw2bhr`; it has no licence, so it was read as a
reference only: no code, tables or text of it are copied).

**Dual Strike's campaign, as found** (USA ROM; overlay 0 at `0x022AD560`,
overlay 1, the campaign's code, at `0x02350560`).

- **Missions.** Map records of 0xA0 bytes at `0x022DBD28 + 0xA0 * id`; the
  campaign is ids 0xE0..0xFB, 28 missions, then 0xFC..0x100, five second
  fronts. A record: +0x00 the event header (six trigger lists), +0x04 the
  objective script, +0x10 the second front's id, +0x14 the name (text bank
  0xC0), +0x20 the CO pool the player picks from (an ARM9 list), +0x24 the
  armies, +0x0C the 4x4 structure's picture (a `bmap` name: "0a5" missile
  pad, "0a6" fortress), +0x1A/+0x1B/+0x1C look, weather, fog,
  +0x2C/+0x30 rank days and day limits (normal, hard), +0x41 the
  mission's number, +0x44/+0x48 the map (normal, hard: AW2's LZ77 blob of
  AW2 tile ids), +0x4C/+0x50 the units (13-byte records, FE army, FF end),
  +0x56 (CO, tag CO) per army (0x1C: the player picks), +0x88 colours, +0x8D
  teams.
- **The 25 story missions**: Jake's Trial, The New Black, Max Attacks,
  Reclaim the Skies, Neverending War, The Ocean Blue, Fog Rolls In, Tag
  Battle, Victory or Death!, Black Boats Ahoy!, Lightning Strikes, Frozen
  Fortress, Verdant Hills, Snow Hunters, Omens and Signs, Into the Woods,
  Muck Amok!, Healing Touch, Crystal Calamity, Dark Ambition, Pincer Strike,
  Ring of Fire, Surrounded!, For the Future!, Means to an End; and three
  research-lab missions, The Long March, Lash's Test, Spiral Garden, each
  opened by capturing the city that hides its lab's map in the mission
  before (Black Boats Ahoy!, Frozen Fortress, Snow Hunters: their scripts set
  campaign flags 0x60..0x62; tangoAW2 keeps them at 0x90..0x92, since 0x60
  is AW2's Hard Campaign flag and with it set every mission used its hard
  deployment, as 0.4.0 did; a 0.4.0 record's flags move as it loads). Second fronts: Victory or Death!, Lightning
  Strikes, Omens and Signs, Ring of Fire, Means to an End.
- **Events** are AW2's, grown. Trigger records are 8 bytes, op = 3 * AW2's
  op + the front (0 main, 1 second, 2 either), 0x15/0x16 open a block for
  normal/hard only, 0x1A ends a list. Scripts are AW2's 16-byte commands
  with ops renumbered (Dual Strike's handler table at ARM9 `0x021585C0`,
  0x5D ops); conditions and actions are calls into overlay 1 (about 40
  conditions: units moved, out of fuel, a type gone, a property owned, a
  structure destroyed, a day; actions: weather, unlocks, spawns). Dialogue
  is text by reference (bank << 24 | index; the banks table at
  `0x022F6BF0`, mission banks 0x21..0x40), ASCII with `\r`, `\x0e` (pause),
  `\x0f` (end of box); faces are `co | expression << 8`.

**In tangoAW2.**

- **Select Mode** (`campaign_menu.rs`): Campaign opens a small sub-menu in
  the box Campaign's Continue / New use, AW2 CAMPAIGN and DS CAMPAIGN (80x16
  labels in the game's style, OBJ tiles 832..903, palettes 8 and 10, put in
  place of the box's own label sprites at the sprite flush). AW2 CAMPAIGN
  then shows AW2's own Continue / New, unchanged; DS CAMPAIGN shows them for
  the DS Campaign (Continue when a DS Campaign is saved); B goes back. It
  works on both wheel procs (`0x08616A08` on entering Select Mode,
  `0x08616A40` when coming back from a mode) and on Survival's seven-entry
  wheel (`mode_menu::item` reads the item at a position). Without the pack
  the box is AW2's own.
- **Start**: the DS box sets a request (`0x0203FD11`); the game's own
  Campaign New / Continue (`sub_0803BA4C` / `sub_0803BA88`, trapped) then
  starts the DS session instead: campaign mode, map id 0xF0 (the mission's
  header written into that entry of Survival's 0x100-id map table), and a
  proc of ours: save the progress, AW2's CO select (`0x086165C0`) with Dual
  Strike's pool grouped by country when the mission has a player-picked CO,
  BG0 emptied (AW2's own campaign reaches its mission card through screens
  that clear it), `ResetRulesAfterCampaignMap`, then AW2's mission proc
  (`0x0849EBFC`: mission card, battle).
- **Conversion** (`ds_campaign_data.rs`, at the first frame with the pack,
  into ROM `0x08F00000..`, about 360 KB): every mission's map, deployment
  (AI behaviours 0, 1 and 5 kept, others hold), header, trigger lists and
  every script reachable from them, and 1,700 texts (text ids 0x7400..,
  pointers in the text table's free tail), Dual Strike's lines re-wrapped
  for AW2's two-line boxes (a box that needs more is spread evenly). Script
  ops: text, faces (Dual Strike's ids to AW2's and tangoAW2's new COs;
  soldiers to AW2's troopers), window frames, cursor, camera, waits, jumps,
  "unless CO", flags, wins and losses become AW2's; conditions and actions
  become magic stubs (Thumb: `ldr r3, =id; ldr r2, =0x0803CC5E; bx r2`, the
  landing trapped) run in Rust (`ds_campaign_rules.rs`, one entry per Dual
  Strike function, read from its code).
- **Mission end**: AW2's best-score record (`InsertBestScoreRecord`
  `0x08017720`) is skipped for map id 0xF0: its slot would land on the event
  script slots (`0x0200C600`) and stall the save prompt. Com Towers capture
  as in Versus (the battle goes on), except a lab mission's lab cells, which
  end it as Dual Strike's labs do.
- **Maps**: Dual Strike's tiles are AW2's but for Com Towers (the Lab
  tiles), Black Crystals (0x192), its Black Obelisks (drawn on AW2's Black
  Cannon tiles, which would fire: tangoAW2's Obelisk), Ring of Fire's
  Volcano (below) and the Grand Bolt (below).
- **Rules per mission**: the header names AW2's picture of a 4x4
  structure (+0x10, the same bytes as Dual Strike's) and the fog; at each
  mission start (`crate::sandstorm`'s map-start trap `0x08035490`) the fog,
  the weather as fixed weather (sandstorm as tangoAW2's) and the look are set
  (Snow, Desert and Wasteland drawn with Dual Strike's own terrain,
  `wasteland::set_ds_look`).
- **Means to an End**: the Grand Bolt is a picture of tiles (laid out as a
  sheet) AW2 has no art for; it becomes plains with a Black Obelisk on each
  of its three weak points ((3, 9), (9, 11), (15, 9), where Dual Strike's
  code tests its structure kinds 0xB..0xD). On Black Hole's turn of every
  sixth day each standing weak point destroys the unit below it and spawns
  an Oozium there (AW2's `CreateUnitAt`); destroying all three wins. Its
  second front's three Black Crystals stand on the main map (below).
- **Flags**: AW2 keeps its campaign progress in campaign flags 0x20..;
  during a session the game's flag get/set (`0x0803CBD8` / `0x0803CBA0`,
  trapped) use the DS Campaign's own (`0x0203FD20`, 16 bytes).
- **World map** (`ds_worldmap.rs`): New and Continue open AW2's own
  campaign map screen (its `WorldMap*` procs: cursor, scrolling, flags,
  mission panel, reveal, music and sounds) on Dual Strike's Omega Land: its
  touch-screen map (`ohashi/res_gmap_map1`/`_map2`, LZ77 4bpp tiles and a
  32x32 tilemap each, ten palettes at `res_gmap` +0x3B08), fitted at the
  first DS session into 768 map tiles (AW2's 704 and the block of BG1's
  tilemap, `0x0600D800`, put back each frame on the DS map since the
  screen's setup writes BG1's tilemap there; BG1 is off on the DS map) and
  nine palettes (BG 6..14; tile 0 blank): flips folded, the most alike
  tiles of a palette folded together (shade weighted 4x over detail), the
  kept tiles refined (4 rounds); 31.4 dB against Dual Strike's picture,
  colour jumps across tile edges +1.6 over its own (the first fit: 29.3 dB,
  +3.2; `ds_campaign_world_map_picture`), with Dual Strike's mission points (ARM9 `0x0215BA04`). AW2's
  mission table (`0x08615194`) and reveal table (`0x0861500C`) get DS copies
  (ROM `0x08FC0000..`), and while a session is on, the literal-pool words
  that point at AW2's art and tables point at the copies. New shows Jake's
  Trial's flag alone; a mission's flag appears once it opens (the next
  story mission after a win, a lab mission once its map is found) and stays,
  cleared, once won (`ds_campaign_win_*` check every flag after each win
  and after Continue). Beside LEVEL under the cursor are AW2's difficulty
  stars (the table's +3 Normal, +4 Hard): Dual Strike has no difficulty
  value (none in its mission records or map points, none on its map), so
  they follow the mission's place in the campaign over AW2's ranges:
  Normal 1 + step x 7 / 28 (1..7), Hard Normal + 1 + step x 3 / 28 (2..10)
  (`ds_worldmap::stars`, `ds_campaign_world_map_stars`). Open missions have
  AW2's flag; A opens the mission's panel (its objective, AW2's info window),
  A again starts it (the CO screen when the player picks). Back on the map
  after a win the mission is cleared and the missions it opens are revealed
  as AW2 reveals its own; a won mission's point keeps AW2's starred flag
  (OBJ tiles 40..43, added at the sprite flush: AW2 paints a won mission's
  part of its continent instead). AW2's story steps on the way (nation
  panel, scenes after missions, bonus and alternative missions, the switch
  in `WorldMapReturn_Init` on its own mission ids, the save prompt) are
  skipped in a session, AW2's sea-and-grid layer (BG1) is held off on the
  DS map, and AW2's map state (`0x0202FDFC`, 0xFC bytes, part of its
  campaign save) is put aside and restored when the session ends.
- **The story outside the battles** (`ds_campaign_data::Story`): Dual
  Strike plays its story inside the missions (the turn-start lists' scenes,
  the match-end lists' victory and defeat scenes: converted with the rest)
  and, from overlay 5, the scenes its game flow starts after certain wins
  (ARM9 `0x020D63B0`, by map record id): the narration after Victory or
  Death! (0xE8: bank 0x21 text 3, over pictures), the victory party after
  Crystal Calamity (0xF2: overlay 5's proc script `0x023682E0`, scripts
  `0x023683C8`, `0x023686E8`, `0x023684E8`) and the ending after Means to an
  End (0xF8: proc script `0x02368AF8`, scripts `0x02369180`, `0x02368CA0`,
  `0x02368E40`, `0x02368FE0`, `0x02368B60`), and the prologue before the
  first map (bank 0x21 texts 0..2). Overlay 5's scripts are Dual Strike's
  event format and are converted as the battles' are (their calls are its
  picture screen's, left out), each one's end a jump to the next; the
  narration is AW2's speaker-less text (`ShowTextOnBg0`, op 0x1A) over
  the picture Dual Strike shows with it (`ds_story_art.rs`: its
  `rikiishi/` files, LZ77 tiles at 4 or 8 bits a pixel, an LZ77 map, a
  256-colour palette; 240x160 of the 256x192 picture with a light box
  where the text goes, put into the map layer's form: nine palettes by
  k-means over its cells, flips folded), put on the map's layer by a magic
  call before the text (BG3's tiles, tilemap and palettes 6..14, its
  scroll, the map's sprites off) and the map put back after. AW2
  plays a mission record's +0x18 on its map after the mission is won
  (`StartWorldMapAfterMissionScript`): the DS records of those three
  missions hold the scenes. The prologue: the session's copy of AW2's world
  map script from the menu (`0x0861485C`; the words pointing at it,
  `0x0807814C`, `0x0807817C`, `0x080781E4`, `0x0849EB90`, `0x08614750`,
  point at the copy during a session) calls a magic stub before the map
  takes the pad, which starts the prologue (`StartBlockingEventScript`) on
  a new campaign once (flag 0x9E of the record, saved right after).
- **Means to an End's choice**: its victory scene asks the player (a text
  ending in Dual Strike's choice code 0x16, then a jump on its answer,
  `0x020199A4`): the text ends in AW2's choice code (0x0E 0x17, as AW2's
  "Do you really yield?") and the jump asks AW2's answer
  (`IsTwoOptionChoiceFirst`, `0x080457BD`): Yes, Jake destroys the chair;
  No, Hawke does.
- **End of a mission** (`EndOfGame_FinishCampaignMap`, trapped past its
  prologue at `0x08038488`): the outcome is recorded (a loss leaves the
  flags as they were), the missions the win opens are written for the
  reveal, and the function's own tail runs (`ResetRulesAfterCampaignMap`,
  `StartCampaignAfterMap`: AW2's return to the map). After Means to an End
  the map stays up with every mission cleared. The mission card's number (`GetCampaignResultCountPlusOne`
  `0x0803840C`, trapped) counts DS missions won.
- **Music**: the maps play their COs' themes as AW2 does (the new COs'
  Dual Strike themes, `ds_music.rs`). Dual Strike's event songs (SDAT
  sequences 0x16, 0x17, 0x19, 0x1A, 0x23, 0x2C, 0x2D, 0x3D, 0x3E), its
  opening (`BGM_OPENING1` 0x29, the prologue), world map (GMAP1 0x06) and
  ending (`NML_ENDING1` 0x36) are converted as the CO themes are
  (`STORY_SONGS`, written from `0x09000000`) and played by the session's
  scripts; with a version 3 pack AW2's like songs stand in (allies' scenes
  and crises 413, Black Hole's 411, Von Bolt's 220). The songs sit
  after AW2's 505 in the song table but only the converted DS scripts name
  them (`aw2_song`), so AW2's own scripts never play one. Test: `ds_campaign_story_music`.
- **Credits** (`ds_credits.rs`): after Means to an End's ending scenes the
  map is left as its "Return to Select Mode" leaves it (the cursor loop,
  trapped at `0x0807703C`, calls `Proc_Goto(map, 6)` with the answer
  `0x030030F2` = 0, Yes; the menu's start `0x0803B83C`, trapped, starts the
  session's ending proc instead, which runs the roll and then starts the
  menu). The roll is AW2's staff roll (proc script `0x08581AC8`) from a
  session copy without AW2's epilogue (its "War is over" paper, its last
  mission's recap) and without its "Campaign Clear" and campaign rank,
  reading Dual Strike's pages: Dual Strike's 28 sections (overlay 5,
  `0x0236A418`: words 0 blank, 1 name, 2/3 heading, then the time) as AW2
  pages (six (kind, text) slots and the time; AW2's list `0x0858265C`, its
  four pool words switched during a session), headings between stars and in
  two lines when wide, a section with more lines than a page over two. The
  music is Dual Strike's `STRM_STAFF_ROLL1`, a stream (IMA-ADPCM, stereo,
  22767 Hz, 106 s) kept in the pack (version 5) and converted as one
  sample at the mixing rate, mono, played as one held note
  (`ds_music::staff_roll_song`; the roll's `PlaySong(416)` at `0x0806BC84`
  gets it in a session). Dual Strike's top-screen pictures during its roll
  are not shown. Test: `ds_campaign_credits` (every name of Dual Strike's
  roll, read from the .nds, in order; every heading; the music; Select
  Mode after with the session over and AW2's pages back).
- **Hard Campaign**: once a Normal campaign has been cleared (Means to
  an End won: the record's clears byte, kept by New), DS CAMPAIGN's New asks
  Normal or Hard in the chooser's style (`campaign_menu` level 3: two
  labels, A takes the choice and goes on with New, B goes back to the box;
  the box's help lines, text ids 0x9C2/0x9C3, are the choice's while it
  shows); before that New starts Normal directly. Continue resumes the
  saved difficulty. Hard is AW2's own Hard Campaign flag (0x60) in the
  session's flags, set from the record's difficulty byte (the record's
  flags never keep it: a 0.4.0 record kept a lab flag there): AW2's code
  reads it for a mission's hard map and deployment (the converted header's
  +0x30/+0x38, Dual Strike's +0x48/+0x50) and for the results' Normal/Hard
  record. Dual Strike's normal-only (0x15) and hard-only (0x16) trigger
  records are both converted, each testing the difficulty first (AW2 kinds
  6/5 on a pseudo predicate, `HARD_CAMPAIGN`). AW2 CAMPAIGN's own SELECT for
  Hard is untouched. Tests: `ds_campaign_hard_locked`, `ds_campaign_hard`.
- **Records**: a won mission's result goes to the DS Campaign's own table
  (`0x0203F600`, AW2's layout of `gUnknown_0200C2D0`: per mission a Normal
  and a Hard word of CO, days << 8, score << 20; `InsertBestScoreRecord`
  `0x08017720`, trapped, writes it in a session), which the map panel's
  results word (`0x0807758C`) points at in a session. No rank is drawn on
  the map: Dual Strike's map shows none (its map graphics, `res_gmap` and
  `res_gmap_lang_E`, have no rank letters; its ranks are on the results
  screen). Saved with the progress. Test: `ds_campaign_records`.
- **Save**: the progress (`0x0203FD30`, 0x20 bytes: "AWDC", next step,
  campaign over, difficulty, campaigns cleared, missions won (bits), flags
  0x20..0x9F) and the records (0xE0 bytes) are written at each
  mission start through AW2's own save writer (`sub_0801A7D8`) into Flash
  slot 15 (AW2: 0 profile, 2..4 suspends, 5..7 design maps, 8 the design
  map a suspended Versus game is on), so AW2's
  profile and its checksum are untouched; read back from the newest slot-15
  sector.
- **A mission saved halfway** (Dual Strike's campaign has the map menu's
  Save: "Save over Mission / Day data"): the map menu's Save is AW2's in a
  DS mission too, and saves it in Flash slot 14, not AW2's campaign slot 2,
  so an AW2 mission saved halfway and its mark in the profile
  (`0x0200C429`) stay (`crate::suspend`: `sub_08016D30` trapped past its
  prologue, `0x08016D3A`, and at its writer call, `0x08016D8E`: the slot made
  14 and AW2's mark put back before the profile is serialized). The block's
  tail carries the session (after "DS" at +0xE04: the mission, the op 0x5A
  countdown, the flags 0x20..0x9F with Hard's 0x60, Means to an End's state). DS CAMPAIGN's Continue, with slot 14 in
  AW2's sector directory, sets the session up as for the world map (the
  cursor and the map table's entry on the saved mission) and then resumes
  it with AW2's own `sub_08017688(14)`. Slot 14 leaves the directory (as
  AW2's delete, `sub_0801ABF8`, without its write: the next save writes
  it) when the mission ends, won or lost, and on a new DS Campaign; turned
  off before any save after a loss, Continue resumes the saved mission, as
  AW2's own does. Until this the item was hidden in a DS mission (the stub
  at `0x0849AB64`, put back to AW2's test now): a suspended mission would
  have come back as an AW2 one, in AW2's slot. Tests:
  `save_ds_campaign_mission_suspend`,
  `save_ds_campaign_new_drops_mission_suspend`.
- **Hook points** (for merging other work): traps `0x08016BA0` (the profile
  serializer's end), `0x0807703C`, `0x0803B83C`, `0x0806BC84` (the credits),
  `0x0803BA4C`,
  `0x0803BA88`, `0x08038484`, `0x0803CBA0`, `0x0803CBD8`, `0x0803BC7C`
  (`GetCampaignSaveFlag`: the DS box's Continue), `0x0803840C`, `0x0803CC5E`
  (the stubs' landing); `SetMapPlayed` (`0x0803CA28`, Survival's trap) also
  skips map id 0xF0 in a session; `crate::suspend`'s `0x08016D3A`,
  `0x08016D8E`, `0x08016D88`, `0x08016DD0` (a mission saved halfway; its
  hooks in `ds_campaign.rs`: `start`'s Continue, `end_of_battle`,
  `new_progress`). RAM `0x0203FD10..0x0203FD5F` (`0x0203FFAD`: a DS
  mission being saved, `crate::suspend`); ROM `0x08F00000..0x08FFFFFF`;
  text ids 0x7400..0x7FFF; map id 0xF0; Flash slots 15 and 14.

**Compromises.**

- Second fronts are not played: the five two-front missions are their main
  front, Dual Strike's second-front triggers dropped, but in Means to an End.
- **Means to an End on one front** (the user's choices). Dual Strike's
  second front (record 0x100) holds three Black Crystals, each guarding one
  of the Grand Bolt's weak points; destroying all three also wins. Here
  they stand on the main map, on the plain north of the bolt, each in the
  column of the weak point it guards: (3, 3), (9, 5), (15, 3)
  (`ds_campaign_data::MTE_CRYSTALS`, tangoAW2's Crystals: they heal Black
  Hole's units around them as on Crystal Calamity). The second front's
  trigger records are kept as the main map's: a crystal shattered plays
  Dual Strike's "Commander! We have shattered one of the black crystals!"
  and "Well done! Now we can strike one of the Grand Bolt's weak points!"
  (once per crystal: `0x02351C58` answers once for each crystal down, the
  count at `0x0203F700`), and every crystal shattered wins (its op 0x44,
  Black Hole loses). A weak point whose crystal stands takes no damage: it
  is no target where a unit picks one (`GetInventionAt` `0x0803DE94`,
  trapped for its two targeting callers, `0x0802B3DC` and `0x0802E2EA`) and
  its hit points are kept each frame (`0x0203F701..`) whatever hits it.
  The day limit is 36, not 24: the header's limit (the "Day(s) Left"
  counter), the trigger on which Black Hole wins (day 24 becomes 36), and
  the texts that state it (`ds_campaign_data::compromise_text`: the
  briefing, bank 0xC0 text 682, now "shatter the three black crystals and
  defeat Von Bolt within 36 days", no top or touch screen; Von Bolt's "In
  36 days" and "On the 36th day", bank 0x26 text 70; Lash's crystals "to
  the north", text 69). Every other mission keeps Dual Strike's limits and
  texts (`day_limits_are_dual_strikes`, with the .nds: every mission's
  limit, trigger days and day texts). The Grand Bolt's charge stays every
  sixth day. Tests: `ds_campaign_means_to_an_end`, `ds_campaign_map_27`.
- AW2 armies have one CO: a tag pair is its first CO, the "CO pair" tests
  check that CO only, and there are no tag or Dual Strike powers. CO skills
  are tangoAW2's (below); the computer's Hard skill lists of Dual Strike's
  records (+0x60) are empty in the missions read and are not used.
- The world map is Dual Strike's bottom screen only (no top-screen
  displays); a won mission is not played again (its flag stays, starred).
- The player's CO is picked on AW2's CO screen from Dual Strike's pool for
  the mission.
- Results are AW2's results screen (AW2's scoring and ranks).
- Dual Strike's sound effects, screen effects and top-screen displays in
  scripts (ops 0x20, 0x21, 0x2D, 0x2E, 0x46, 0x4B, 0x59) are left out, as
  are its waits on a scene's proc script (ops 0x55, 0x57: `0x0201D298` /
  `0x0201D158` with the script as operand) and its presentation-only
  functions: camera pans (`0x02351334`, `0x02351538`), sounds, flashes and
  fades (`0x023517D4`, `0x023517E4`, `0x02351D50`..`0x02351E20`,
  `0x02003F8C`), the second front's eruption scene (`0x02351A4C`,
  `0x02351AF0`), the scene-skip handler (`0x0201993C`, `0x02019950`).
  `the_campaign_converts` (an ignored test) lists any function not
  handled; `ds_campaign_rules::KNOWN` names those handled or left out.
- **Rule functions with an effect** (`ds_campaign_rules::call`):
  - Victory or Death!'s Black Arc (`0x02350D44`, Dual Strike's
    `0x020EEE1C(13, 5, 100, 0)`, on each of Black Hole's turns while its
    trigger holds): every unit within 2 spaces of (13, 5), but Black
    Hole's team's, Ooziums and loaded units, is left with 1 HP (no
    explosion drawn; Missile Guard takes 10 off, as Dual Strike's 0x2A).
  - Ring of Fire's Volcano: Dual Strike's structure kind 2 (4x4, anchor
    0x1A2 on its third row, at (8, 8)) becomes AW2's own Volcano (anchor
    0x1A7, rim 0x1A5; invention kind 2). It erupts as AW2's does (the
    turn-start loop, once a day from day 3, `sub_0803E764(cells, 50)`),
    but on Dual Strike's cells: the trap `ds_campaign_rules::eruption` at
    `0x0803EE3C` hands it Dual Strike's list for the main map (ARM9
    `0x02167E98`, lists by the volcano's owner; list 1, twelve cells round
    the map's edge), copied to `0x0203F708` in AW2's format. Taking Black
    Hole's four cities round it (`0x02351804`, cities, not Com Towers)
    runs `0x02351988`, which clears the volcano's owner in Dual Strike
    (`0x020DA938`): here `0x0203F704` is set and `crate::obelisk`'s
    turn-start trap skips the Volcano. The second front's win
    (`0x023518CC`) does the same; the second front is not played.
  - Means to an End's choice: `0x02351D3C` / `0x02351D28` set / clear
    campaign flag 0x3C (Dual Strike's `0x021017F4(0x3C, 1 / 0)`).
  - Muck Amok!'s (14, 1) (`0x02351640`) is army 3's HQ; its capture
    routing army 3 (`0x023516A4`) is AW2's own HQ capture.
  - Spiral Garden's "Whoever captures 15 properties wins" is not a script
    but the record's +0x34 (Normal) / +0x36 (Hard), tested by Dual
    Strike's engine. `ds_campaign_data::property_win` adds trigger records
    to the after-action list (3), one per army: the pseudo predicate
    `PROPERTY_COUNT | army << 8 | n` (the army owns n properties or more:
    HQs, cities, bases, airports, ports, Com Towers and labs alike) fires a
    script ending the match with that army's win (op 0x40). As Dual
    Strike's code has it: the battle's setup copies +0x34/+0x36 to its state
    (+0x81, `0x020E9400`); `0x020C4498` counts each player's properties
    from the map (classes 6, 8, 10, 11, 14, 20, 22 by the table at
    `0x022F45B8`) and `0x020CEDF4` returns the first player 1..4 with that
    many or more, whose team then wins (`0x02019A6C`): the computer's
    armies too.
  - Crystal Calamity's `0x0235172C` is the Black Onyx's real-time laser
    charge (90% or more on Dual Strike's top screen); with the 50-minute
    count left out it never holds.

Tests: `tools/aw2test/tests/test_ds_campaign.py` (the sub-menu with and
without the pack, AW2's campaign unchanged to its first mission card,
Survival and the DS Campaign in one boot, Jake's Trial against the .nds
(card laid out as AW2's, dialogue all Dual Strike's own words, map,
deployment, name), every mission in battle (tiles, terrain, deployment,
fog, weather, look, structure picture), five later missions, the world
map (Jake's Trial picked with the cursor and won through the pad, the map
after the win, saved to Flash and continued after a reboot), the Com Tower
capture, the lab flags and the score slot fixes, the Grand Bolt's spawns, the computer playing
three days on four missions with no army dropping out);
`aw2test/dscampaign.py` drives it and reads Dual Strike's missions directly.

AW2's own campaign stays AW2's (`tools/aw2test/tests/test_aw2_campaign_vanilla.py`):
its opening (New, the story, the world map, Mission 1's card, the first
battle) and its ending (`specialProperty` 0x10 put on its first mission so
its win starts AW2's ending proc `0x084A0A3C`; the scenes and staff roll
until Select Mode is back) are traced every 20 frames (the text shown, the
song, the screen, the game's frame count) without the pack, with it, and
after a DS Campaign session in the same boot, and must match exactly. The
runs press New at the same frame from the boot and write nothing to RAM.
A session's reference is a pack-off AW2 campaign session left with Yes at
the same frame: for the opening one entered with Continue (AW2's world map
from the menu, as the DS session enters it; AW2's proc pool is then left
the same way, and the next campaign starts its procs in the same slots,
which the world map's opening zoom and stamp depend on), for the ending
one entered with New (no AW2 campaign loaded, as in a DS session, so the
results' running total starts the same). Every sample matches (500 for
the opening, 683 for the ending).

**AW2's profile during a session.** AW2's save writer (`sub_0801A7D8`)
serializes the profile (`0x08016B2C`: 0x02028030, 0x0200C078, 0x0200C2D0,
0x0200C420 and the world map state 0x0202FDFC, 0x5CC bytes) into a new
slot-0 sector with every slot it writes, since the profile's header lists
the other slots' sectors. The DS Campaign's own record (slot 15) is written
so too, while the world map state holds the DS map's: before this fix
that took AW2's campaign away (no Continue, and New no longer warned).
The serializer is trapped at its end (`0x08016BA0`): while AW2's world map
state is in the session's backup (`ds_worldmap::backup_aw2_state`), the
buffer gets it from there, so the profile in Flash stays AW2's
(`aw2_campaign_kept_by_ds_session`: the profile byte for byte, Continue
and New's notice in the same boot and after a reboot, with and without
the pack).

## CO skills (`co_skills.rs`, `skills_panel.rs`)

Dual Strike's CO skills, from its code (overlay 0's skill table at
`0x022F5ECC`: 12-byte records by id, {rank, name `0x7C<<24|i`, description
`0x7B<<24|i`}; its per-CO bitmap test `0x020E7EE0`):

- **Skills.** Its 43 player skills (ids 0x20..0x4A) less the three tag
  skills (0x35..0x37: no tag pairs here). Each takes a slot; their effects
  stack. Names, ranks and descriptions come from the pack.
- **In battle.** Each army's skills are a bitmap in RAM (`ACTIVE`,
  `0x0203F7E0`, 6 bytes an army), cleared at every map start and set by
  the mode (`battle_start`): the player's armies get their CO's set of the
  mode (DS Campaign and AW2's campaign: Campaign; Survival; War Room);
  Versus with its Skills rule on gives every army, the computer's too, its
  CO's Versus set. A set gives only the skills open to the CO, as many as
  its slots. Everything reads that bitmap: with no skill on a battle plays
  as it always did.
- **Effects** (Dual Strike's numbers; its functions in the module's docs):
  attack (direct/indirect +5/+8, terrain +10, Backstab +15, weather +20),
  defence (direct/indirect +8/+12, APC Guard +10) through the firepower
  and defence hooks the CO code already has; funds (Gold Rush +100 per
  earning property, Combat Pay 2% of the value hit), repairs (+1/+2),
  Missile Guard (silo blasts and the Black Arc 10 less), Cannon Guard
  (structure shots 20 less, `0x0803ED12`); and traps in AW2's code: move
  (`GetUnitMovementWithCoBonus` 0x08042D42, APC Boost), vision
  (0x08042DA6), capture points (0x0804269A), price (`GetCoPriceMultiplier`
  0x08042CC0; not for the power meter, as Dual Strike), luck (0x08042E64 /
  0x08042E7A), the meter (0x080440E0, Star Power x1.1), hidden fuel (with
  crate::ds_weather's fuel trap), move costs (`CacheUnitMovementCosts`'
  end 0x0801F91E), and the special-ability bits in a Super Power
  (`GetPlayerSpecialAbilities` 0x08043050 / 0x08043066: Mistwalker
  strikes first, Soul of Hachi deploys from cities; the computer never
  builds at cities).
- **Rank and slots.** A CO's rank is its EXP / 1000 (up to 100); a skill
  opens at its rank; slots = min(rank, 4). The rank-10 skills open with
  Means to an End won instead (Eagle Eye, Gear Head, Conquerer on Normal;
  Mistwalker and Soul of Hachi on Hard), as Dual Strike's flags 0x21/0x22.
- **EXP** (a won battle; the humans' COs): the DS Campaign the mission's
  score, x2 (x1 in Dual Strike's first eight missions), x2 on Hard
  (`ds_campaign`'s best-score trap); at `EndOfGame_Finish` (0x0803832C):
  Survival half the score, the War Room x2.5 (x2 with skills on), AW2's
  campaign as the DS Campaign's but only once the player has set skills
  for some CO (until then it stays AW2's own); Versus none. Dual Strike's
  few extra points for its battle counters are left out.
- **Save.** Per CO (AW2's 19 and the nine new): EXP and seven sets
  (Campaign, Survival, War Room, four Versus), 32 bytes, at `0x0203E000`
  after a magic word; saved in Flash slot 15 after the DS Campaign's
  progress and records (one 0x4A4-byte record). The DS Campaign's save
  writes it; after any profile write (`sub_0801A7D8(0, ..)` returning at
  0x08016E2C, 0x0801AC40, 0x0801AE2E) it is written again if it changed.
- **The panel.** On the CO screen (War Room, Survival, the campaigns:
  `ProcScr_CoSelect`) SELECT opens it for the CO highlighted; on Versus'
  Teams screen SELECT on an army's CO stop, for its Versus set (R and L
  there change the army's colour), and in the panel L turns the Skills rule
  on or off. UP
  and DOWN pick a slot, LEFT and RIGHT the skill (none, or one open to the
  CO), A keeps the set, B closes the panel as it was; the game gets no
  button meanwhile. Drawn in AW2's glyph font in OBJ tiles each screen
  leaves unused while it is up (CO screen 0x1EC.., Teams 0x090..), OBJ
  palette 14, put first in the sprite list.
- **Netplay.** The console boots from seat 0's save and both seats' buttons
  reach the Teams screen: both peers have the same sets and rule (the
  host's skill data).
- **Tests:** `tools/aw2test/tests/test_co_skills.py`: each effect against
  the damage calculator (its `skill_attack`/`skill_defence`) or the game's
  numbers (move, capture, price, income, repair, meter), EXP and sets in
  the DS Campaign (across a reboot) and the War Room (in Flash), the panel
  on both screens, the Versus rule.

## Suspended games (`suspend.rs`)

The map menu's Save (`sub_08016D30`) writes the 0xE28-byte block
`CaptureBattleSaveState` (`sub_08016F38`) fills at `0x02000000`: day, army,
gPlaySt, the weather block, players, units, the tiles changed from the
map's own, the inventions; up to +0xDAC. The rest of the block goes to
Flash but is never read back (`sub_08017208`). With the Dual Strike pack,
tangoAW2's own battle state that lasts past a turn rides there: a mark
("TAW2", version 1) at +0xDAC, the Rules' fog flag kept while rain forces
fog on (`ds_weather`) at +0xDB1, Ex Machina's stun bits (`co_powers`,
pending then held, 40 bytes each) from +0xDB4. Traps: `0x08016D88`
(`sub_08016D30` after the capture, before the write) and `0x08016DD0`
(Continue, `sub_08016DB8` after `sub_08017208`, before a design map's own
slot is loaded over the buffer). Before this, a game continued after
Ex Machina had every marked unit free, and one saved while rain was coming
kept fog on for good once the rain stopped. The sandstorm and the map's
look are in the weather block (`0x03004490` +3), which AW2 saves itself;
Com Towers are counted on the map. Without the pack nothing is written.
Tests: `save_versus_suspend_keeps_ex_machina_stun`,
`save_versus_suspend_in_rain_keeps_fog_rule`.

## Saves

AW2's 64 KiB Flash is 16 sectors of 0x1000 bytes. A sector is one part of a
slot (save tag): "2ars", 0x55/0xAA at +4 and its opposite at +0xFFF, 0x0F
at +5, the sector's 8-bit sum at +6 and its complement at +7 (AW2's check,
`sub_0801B09C`), the generation at +8, the part at +0xC, the slot at +0xD,
the payload's place at +0xE and length at +0x50, the payload from +0x52.
The newest profile's +0xFEF lists every sector's slot (the directory,
`sub_0801B2FC`). The writer (`sub_0801A7D8`) puts a slot's new copy in free
sectors, then a new profile serialized from RAM (`sub_08016B2C`), whose
directory drops the old copy; a delete (`sub_0801ABF8`) only drops the slot
from the directory.

| Slot | What | Written by |
| --- | --- | --- |
| 0 | profile, 0x5CC: unlocks and campaign flags (`0x02028030`), War Room scores (`0x0200C078`, 30 maps), campaign scores (`0x0200C2D0`), options (`0x0200C420`: points, save count, suspend marks +9..+B, options, results; tangoAW2's Survival records +0x15..+0x1E), AW2's world map (`0x0202FDFC`) | every write |
| 2 / 3 / 4 | Campaign / War Room / Versus game saved halfway, 0xE28 (tangoAW2's tail: `suspend.rs`) | map menu Save |
| 5..7 | design maps 1..3, 0x724 (tangoAW2: +0x4C4 the five-army mark, +0x723 the look) | Design Room Save |
| 8 | the design map a saved Versus game is on (its current terrain and units) | map menu Save on a design map |
| 14 | a DS Campaign mission saved halfway (tangoAW2) | map menu Save in a DS mission |
| 15 | the DS Campaign's record, 0x20 (tangoAW2) | DS Campaign New, mission start, after a win |

Ten slots at most; a write needs two free sectors. The Design Room has no
delete for one map: saving over a slot replaces it. The Battle Maps points
(options +0x00, +0x04) grow with every map won, a DS mission's and a
Survival map's too (as the War Room's). A netplay match runs
on player 1's save on both consoles and never writes either player's file
(only single-player sessions persist their save: `tango/src/session/launch.rs`).

**Tests** (`tools/aw2test/tests/test_save_integrity*.py`, `-k save_`; the
Flash read with `aw2test/saveimg.py`, AW2's own rules): every step exports
the Flash before and after and checks that every sector the directory lists
passes AW2's check and that only the expected slots and profile bytes
changed (AW2's save counter aside), then reboots a fresh console from the
written save: Versus saved and continued on 2P, 4P and design maps,
tangoAW2's maps (Wasteland in a sandstorm, Com Towers, Obelisk maps),
a Wasteland design with Dual Strike's units and COs, after Ex Machina, in
rain; Save hidden and nothing written on five-army maps; the Design Room's
three slots (normal and Wasteland, five armies, Black Hole's inventions,
towers of every owner, Dual Strike's units, a full design of 250 units),
every record byte for byte through save, load and reboot, played and saved
in Versus; AW2's campaign (a win, a mission saved and continued, the pack's
profile byte for byte AW2's own); the DS Campaign over an AW2 campaign in
progress (wins, a lab flag, the prologue flag, a mission saved halfway and
continued, a loss, New), AW2's data untouched; the War Room (a score, a
map saved and continued; its list only AW2's maps); Survival (each kind's
record, nothing of the War Room's, a War Room game saved halfway kept);
AW2's completion flags; every mode in one boot; every slot in use at once;
a game saved over netplay the same on both peers.

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
