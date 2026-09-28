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

- Black Hole: the design map's spare colour byte `0x03003FF3[0]` (saved at
  record `+0x4C4`, restored on load, cached at `0x020280D4 + 0x1C*k` for the
  map list) is set to 4 when slot 4 is Black Hole. The editor's slot 4 is
  then drawn with colour 5 (player colour byte, palette rows, and the army
  list's emblem). In Versus the Teams screen starts that army as Black
  Hole, and design-map battles swap palettes to the picked colours
  (design maps load palettes from the map's own colours, `0x03003FF3`).
- Inventions are terrain tiles with their campaign footprints (anchor tile
  carries the class): minicannons `0x182..0x185`, laser `0x181`, Black
  Cannon `0x187`/`0x18A` (3x3), Black Factory `0x18D` (3x4), Volcano
  `0x1A7` (4x4), Deathray `0x190` (3x3); other cells are underlay `0x1A4`
  (Volcano rim `0x1A5`). The battle registers them by scanning the map.
  The Black Factory and the Volcano share graphics memory in battle, so
  a map may hold one of them.
- The game draws an army's units and HQ in the designs of its CO's army
  (`0x08042DE0`: player + 0x1D CO -> country, table `0x085D3DD0`), in battle
  and in the editor. The editor gives slot 4 Kanbei (Yellow Comet), so while
  slot 4 is Black Hole tangoAW2 gives it Flak (its own CO is kept at
  `0x0203FFFD`), copies Black Hole's HQ sprite top (`0x080D16C4 +
  0x100*(country-1)`) over Yellow Comet's in OBJ VRAM, holds slot 4's
  palette rows to Black Hole's, and sets bit 3 (reload graphics) on the
  tool bar's ring entries (`gDesignRing`, `0x0200B0D0`, 11 x 0x1C, flags
  first) so the bar redraws for the new CO. Back to Yellow Comet undoes it.
  Units already placed are background tiles chosen when drawn, so the
  switch also runs the game's visible-map unit redraw (`sub_08022580`)
  through a detour at the entry of the editor's per-frame handler
  (`sub_08005F4C`, void, only LR live; LR kept at `0x0203FFEC`, state at
  `0x0203FFF4`).
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
  Volcano map palette 2 holds the Volcano's colours
  and the rest use slot 4's unit palette (12).
- The aw2bhr decompilation (`src/design.c`) names most of the editor's
  drawing: `sub_08002844` (unit icon: `sub_080261A4(slot, kind)`, CO-country
  based) and `sub_0800272C` (terrain icon; HQs via `sub_0803F6BC(8, army)`).
- Buttons tangoAW2 takes are hidden from the game for as long as they
  stay held (`0x0203FFF6`), so a held press never reaches the editor as a
  new one.

- Yellow Comet and Black Hole share army slot 4; the map-wide marker
  `0x03003FF3[0]` (4 = Black Hole) picks which. Going back to Yellow Comet
  restores slot 4's colour byte and palettes, since the game only reloads
  them when the slot changes.

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

- A "tangoAW2" badge drawn by the game's sprite hardware while
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
  palette), army 5's units on BG palette 1 (pipes' palette; 5P maps have no
  pipes), the Black Hole HQ art in the building sheet's lab slot.
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
  +20 or +40 (of 100) HP, capped at 100, and full ammo and fuel from the
  unit table `0x085D5ABC` (+0x0B, +0x10). Only that army's own unit ids are
  walked, so enemies and allies are never healed.
- **Panel.** The terrain panel (`sub_0802A8DC`, cell in r8/r5) gets the
  name picture at `0x0802A914` and the picture at `0x0802A982`.

## Known limits

- Black Hole's unique buildings (Black Cannons and so on) are map
  features; ordinary Versus maps do not have them. Build them in the
  Design Room.
- Campaign and War Room are single-player. Netplay is Versus only.
