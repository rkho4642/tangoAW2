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
by tangoAW2 to `0x0203FF80`, see below). The
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
- In a tool bar SELECT only swapped bars, like L/R; tangoAW2 turns a SELECT
  press into UP (next army, Black Hole included).
- Inventions in the terrain bar (`design_bar.rs`): the bar's list is built
  from the template `0x08488810` by `sub_080078E4` into 17 (terrain) or 20
  (units) 4-byte entries (type or unit word, tile placed) at `0x0200B224`,
  and the HBlank buffer follows at `0x0200B274`. tangoAW2 repoints the nine
  literal-pool words for the list (`0x08001CFC`, `0x08001D58`,
  `0x08001D88`, `0x080062B8`, `0x08006340`, `0x08007750`, `0x08007844`,
  `0x080078D0`, `0x08007918`) to `0x0203FF80`, patches the terrain list's
  length 17/16/0x44 to 27/26/0x6C where the editor wraps it (`0x08000CEA`,
  `0x08001D4E`, `0x0800626E/72`, `0x080062E6`, `0x08006460/68`,
  `0x08006562`, `0x08007798/9C/9E`), all in the ROM image in memory, and a
  trap at the builder's exit (`0x080079B2`) inserts the ten inventions
  (types `0x15..0x1E` with their anchor tiles) after the Silo. Icons: the
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
  palette `0x080D3EC4`). In the editor tangoAW2 loads them into OBJ tiles
  289..512 and palette 2 (never used by the editor) and appends each placed
  invention's sprites at the game's VBlank sprite flush (`0x0801BBC4`; the
  frame's list at `[0x03002F2C]` inside the area described at
  `0x03000268`). On a Volcano map palette 2 holds the Volcano's colours
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

## Known limits

- Black Hole's unique buildings (Black Cannons and so on) are map
  features; ordinary Versus maps do not have them. Build them in the
  Design Room.
- Campaign and War Room are single-player. Netplay is Versus only.
