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
| `0x0203FFF0..` | tangoAW2's own state (the previous joypad word), in EWRAM the game never touches. |
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

## Known limits

- Black Hole's unique buildings (Black Cannons and so on) are map
  features; ordinary Versus maps do not have them.
- Campaign and War Room are single-player. Netplay is Versus only.
