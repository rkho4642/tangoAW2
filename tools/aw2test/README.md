# aw2test: scripted battle tests

Real Versus battles on tangoAW2's console: a design map is written into a copy
of a cartridge save, the game boots, the harness picks COs and rules on the
Teams and Rules screens, then plays through the menus (move, Fire, CO Power,
End turn, CPU turns) and checks what the game did against numbers it works
out on its own.

Nothing from the games is kept in the repo: tables are read at run time from
your ROMs, and every save, script, screenshot and log goes to
`tools/aw2test/out/` (ignored).

## Running

```
cargo build --release -p tango-gamesupport-aw2 --examples
python3 tools/aw2test/run.py                # every test, AW2 rules and Dual Strike pack
python3 tools/aw2test/run.py --mode aw2     # without the pack
python3 tools/aw2test/run.py -k power -v    # tests whose name contains "power", with their logs
```

Needs Python 3 (no packages). Paths, all overridable:

| | default | env |
|---|---|---|
| AW2 ROM | `~/Documents/TangoAW2/roms/Advance_wars_2.gba` | `AW2TEST_ROM` |
| Dual Strike ROM (`ds` mode) | `~/Documents/TangoAW2/roms/Advance Wars - Dual Strike (USA).nds` | `TANGOAW2_DS_ROM` |
| base save (read only) | `tools/aw2test/out/base.sav`, copied once from `~/Documents/TangoAW2/saves/Advance Wars 2.sav` | `AW2TEST_BASE_SAVE` |
| output | `tools/aw2test/out` | `AW2TEST_OUT` |

The base save can be any save past the campaign prologue (with no DS Campaign
saved: delete `out/base.sav` to take a new copy); the player's own save is
never written, and the pinned copy keeps runs alike while they play. The map goes into
design slot 1 of a copy. Each test runs once per mode: `aw2` (no pack) and `ds`
(the pack from the .nds). A test takes 1 to 5 seconds; they run in parallel.

Save integrity (`tests/test_save_integrity*.py`, `run.py -k save_`): every
mode's data saved, rebooted from the written save and checked, and each step's
Flash diffed slot by slot with AW2's own rules (`aw2test/saveimg.py`; driving
the saves and Continues: `aw2test/saves.py`, `aw2test/campaigns.py`; see
docs/AW2.md "Saves").

`AW2TEST_COMPARE_RUNNER=<older aw2_script>` turns on `compat_aw2_byte_identical`,
which replays a whole battle on this build and the older one and compares all of
EWRAM and IWRAM (see `tests/test_compat.py` for building an older commit).

## Writing a test

A test is a function in `tests/test_*.py` taking a context:

```python
from aw2test.harness import test

@test()                                   # modes=("aw2", "ds") by default
def tank_vs_tank_plains(ctx):
    m = ctx.map()                         # 30x20 plains, HQs at (0,0) and (29,19), a spare Infantry each
    m.terrain(11, 10, "wood")             # plain river mountain wood road sea shoal reef, or hq/city/base/airport/port with an owner
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "drake"])   # COs by army; army 1 human, the rest CPU; fog off, clear, animations off
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=55)   # from, to, target
```

`ctx.attack` moves the unit, picks Fire and the target, and checks: the base
damage and weapon the game used (its BattleUnit record) against the
calculator, both HP losses within the luck range, ammo, and both power meters.
Other helpers:

- `ctx.start(m, cos, humans=(1,), fog=False, weather="clear"|"rain"|"snow"|"random"|"sandstorm", power=True, visuals="off", trace=None)`: `trace` is a file of ROM addresses (`AW2_TRACE`); each time the game reaches one the runner prints a `trap` line, collected in `g.e.traps` (no tangoAW2 trap may sit at the same address)
- `ctx.power(g, army, "power"|"super")`: fills the meter to the exact cost, fires the power from the map menu, checks the mode and the spent meter; returns the units before and after. `ctx.expect_hp_change(before, after, {army: +/-hp}, label, repair=False)`.
- `ctx.set_hp(g, x, y, hp)` (internal HP 0..100), `ctx.side(g, unit)` for the calculator, `ctx.check/eq/log`, `ctx.shot(g, name)`.
- `ctx.netplay_replay(g, [(addr, len), ...])`: replays the run so far on two rollback peers.
- The game object `g`: `units()`, `unit_at(x, y)`, `player(army)`, `playst()`, `terrain_class(x, y)`,
  `select`, `move_to`, `choose("Fire"|"Wait"|...)`, `action_menu_at(x, y)`, `map_menu_names()`,
  `wait_unit`, `end_turn(human=1, observe=fn)`, `charge_power`, and `g.e` for raw `read/u8/u16/u32/w8/w16/w32/wait/press`.

The calculator (`aw2test/damage.py`) re-implements `src/battle.c` of the aw2bhr
decompilation. Its tables come from the AW2 ROM file (units, COs, terrain) and,
in `ds` mode, the damage chart from the Dual Strike .nds (overlay 0). Tests also
pin base damage to the published charts (e.g. Fighter vs B Copter 100 / 120).

The Design Room's editor has its own driver, `aw2test/editor.py` (`Editor(emu)`:
`boot`, `place(kind, owner, x, y)` from the terrain bar, `place_unit`,
`set_wasteland`, `save`/`load` through the File menu); see
`tests/test_design_com_tower.py`, which builds a map there, saves it, loads it
back and plays it in Versus.

## How the driver works

`aw2_script <rom> - --save <sav>` reads commands from stdin and answers each
with `@ok FRAME`; the driver (`aw2test/emu.py`) reads RAM between presses.
Navigation never relies on frame counts alone: each step waits for its RAM
sign (the Select Mode and Select Map cursors, the Teams and Rules procs, the
menu builder proc and its cursor, the selected unit), and after every action
the map cursor is nudged one cell and back to confirm the player has control.
Every input is recorded (`g.e.timeline`), so a run can be replayed by
`aw2_netplay_script` or by another build.

## RAM and ROM addresses used

| Address | What |
|---|---|
| `0x0300591C` | Select Mode cursor (3 = Versus) |
| `0x0300596C` | Select Map tab (8 = Design Maps) |
| `0x02017C50` | Teams record: `+0x08` armies, `+0x09` controllers, `+0x17` CO count, `+0x18` CO list pointer, `+0x1C` CO index per army |
| proc fn `0x08064E5D` | Teams screen is up |
| procs with fn `0x08064739/75/7BD/86D/919/9D1` | Rules items (0x60 bytes: value `+0x48`, count `+0x4B`); cursor `0x02017C83` |
| `0x03003FC0..` | gPlaySt: `+0x02` map (0xB4 = design 1), `+0x07` powers on, `+0x08` CO abilities, `+0x09` animations, `+0x0D` fog, `+0x2C..2F` weather, mode, next, default |
| `0x03004490..93` | weather block: rain and snow chances, `+3` one-day sandstorm (pack) |
| `0x03000000` | main callback, `0x08022049` on the battle map |
| procs in `0x03000C00..0x03001F00`, `0x0200C000..0x0200E000` | map UI procs (two script pointers + a Thumb function); `0x08034F8D` runs whenever air units are on the map |
| proc fn `0x08019D0D` | menu builder: `+0x20` table (`0x0849AE28` actions, `0x0849AAC0` map menu), `+0x24` flags (0 shown, 1 hidden, 2 greyed), `+0x31` shown entries, `+0x41` count, `+0x44` menu proc (cursor at `+0x20`) |
| proc fn `0x080228D9` | a unit is selected (`0x030040D8` = its Unit*) |
| `0x030033E4/E6` | map cursor x, y |
| `0x030033EC` | current army |
| `0x030013D0`, `0x030013B0` | attacker and defender BattleUnit (`+0x06` terrain defence, `+0x0C` damage, `+0x0E` defence, `+0x10` base damage, `+0x18` weapon 1/5) |
| `*0x08499594` | units: 12 bytes, id = (army-1)*64 + slot; `+0` type, `+1` flags, `+2/3` x y, `+4` hp:7 ammo:4, `+6` fuel |
| `*0x08499598` | players, 0x3C each from army 1: `+0x00` funds, `+0x1D` CO, `+0x1E` power mode, `+0x20` charge, `+0x25` powers used, `+0x26/28` temp firepower/defence |
| `0x0201E450` | gMap: terrain `+0x1432`, row offsets `+0x417A` |
| ROM `0x085D5ABC` / `0x085D3DD0` / `0x085D583C` / `0x080C1BC4` | unit table, CO table, terrain info (stars), tile classes |

## Not automated

- Battle animations are turned off (Rules "Visuals"); the animated battle scene is not exercised.
- Moving along a chosen path (the cursor walks straight: right/left first), loading and dropping, capture, building, Join, Dive are not wrapped yet; `select`/`move_to`/`choose` cover them by name when needed.
- Sonja's Counter Break (the defender strikes first) is not modelled by the calculator.
- Netplay replays cannot contain RAM pokes (a poke would not be rolled back), so they use CPU turns and real battles only.
