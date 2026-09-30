"""The CO screen's unit grid (map menu > CO, its last page; tangoAW2's
co_grid.rs): every army's page is read while it is drawn (traps on the grid's
code) and checked against the calculator's CO model.

With the Dual Strike pack the grid has two pages, in the build menus' order
(ground; air and naval), with the seven new units: each icon is the unit's
map picture from its army's sheet in that army's colours (the new units in
the sheet slots of the other countries' Infantry and Mech, never the viewed
country's own), each bar the CO's firepower bonus now (power included) at the
nearest bar AW2 has, and the move / range line the CO's changes. Without the
pack the grid is AW2's: one page of 19 units.

Every army is viewed, human and CPU, in two-, four- and five-army games, all
five colours (Black Hole as a colour of a two-army game and as army 5 of a
five-army game), AW2 and Dual Strike COs, day to day and in powers."""

import os

from aw2test.harness import test
from aw2test import rom as romlib

PAGE = 0x03005940
AW2_LIST = 0x08616B22
AW2_SLOTS = 0x08499608
AW2_SHEET = 0x0810BE60
UNIT_PALETTES = 0x0810E6E0
OBJ_PALETTE_0 = 0x05000200
SHEET_VRAM = 0x06012000
SHEETS = 0x087F0000
SHEET_SIZE = 0x6C * 32
NEW_UNITS = [4, 9, 12, 13, 18, 26, 27]
NEW_ART = 0x08688000  # roster.rs ART_MAP: frame 0, 128 bytes per new unit
FIRST_NEW_SLOT = 59
GROUND = [1, 2, 6, 5, 3, 8, 4, 7, 10, 11, 14, 15, 9, 27, 0, 0, 0, 0, 0, 0]
AIR_SEA = [16, 17, 19, 20, 12, 13, 0, 0, 21, 22, 23, 24, 18, 26, 0, 0, 0, 0, 0, 0]

# Grid code (sub_08085708 and its helpers), traced (no tangoAW2 trap there):
T_GRID = 0x08085714    # r1 the army
T_SLOT = 0x08085738    # r0 the list entry's address
T_TILE = 0x080859BE    # r0 the icon's sheet tile (4 * slot)
T_BAR = 0x0808578A     # r0 the firepower bar's sprite
T_RANGE = 0x080857EC   # r0 the range sprite
T_MOVE = 0x0808584E    # r0 the move sprite
T_SHEET = 0x08085960   # r0 the sheet copied for the icons
TRACE = [T_GRID, T_SLOT, T_TILE, T_BAR, T_RANGE, T_MOVE, T_SHEET]

BAR_SPRITE = {-30: 0x9D, -20: 0x9C, -10: 0x9B, 0: 0x9E, 10: 0x9F, 15: 0xA0, 20: 0xA1, 30: 0xA2,
              40: 0xA3, 50: 0xA4, 60: 0xA5, 75: 0xA6, 80: 0xA7}
LINE_SPRITE = {-3: 0x97, -2: 0x96, -1: 0x95, 1: 0x98, 2: 0x99, 3: 0x9A}
COLOURS = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}


def bar_level(v):
    """co_grid.rs bar_level: the nearest bar, away from 0 between two."""
    return min(BAR_SPRITE, key=lambda l: (abs(l - v), -abs(l)))


def trace_file(ctx):
    p = os.path.join(ctx.out, "trace.txt")
    with open(p, "w") as f:
        f.write("".join(f"{a:08x}\n" for a in TRACE))
    return p


def parse(line):
    parts = line.split()
    addr = int(parts[1], 16)
    regs = dict(p.split("=") for p in parts[3:])
    return addr, {k: int(v, 16) for k, v in regs.items()}


def draw(g):
    """One frame of the grid: (army, sheet, [slot records])."""
    sheet = None
    for line in g.e.traps:  # the sheet is copied when an army's page opens
        a, r = parse(line)
        if a == T_SHEET:
            sheet = r["r0"]
    g.e.traps.clear()
    g.e.wait(1)
    army, slots = None, []
    for line in g.e.traps:
        a, r = parse(line)
        if a == T_GRID:
            army, slots = r["r1"], []
        elif a == T_SHEET:
            sheet = r["r0"]
        elif a == T_SLOT:
            slots.append({"entry": r["r0"]})
        elif slots and a == T_TILE:
            slots[-1]["tile"] = r["r0"]
        elif slots and a == T_BAR:
            slots[-1]["bar"] = r["r0"]
        elif slots and a == T_RANGE:
            slots[-1]["range"] = r["r0"]
        elif slots and a == T_MOVE:
            slots[-1]["move"] = r["r0"]
    return army, sheet, slots


def country(g, co):
    return g.e.u8(g.e.u32(g.CO_TABLE_POOL) + romlib.CO_RECORD * co + 0x15) + 1


def check_page(ctx, g, army, slots, want_types, label):
    p = g.player(army)
    co, mode, colour = p["co"], p["co_mode"], p["colour"]
    c = country(g, co)
    types = [g.e.u16(s["entry"]) for s in slots]
    ctx.eq(types, want_types, f"{label}: the units, in order")
    own = [17 + 2 * (c - 1), 18 + 2 * (c - 1)]
    free = [s for s in range(17, 27) if s not in own]
    new_tiles = []
    for s, t in zip(slots, types):
        if not t:
            ctx.check("tile" not in s, f"{label}: an empty slot draws nothing")
            continue
        name = romlib.UNIT_NAMES[t]
        if t in NEW_UNITS:
            want_slot = free[NEW_UNITS.index(t)]
            new_tiles.append(s.get("tile"))
        else:
            want_slot = g.e.u16(AW2_SLOTS + 0x32 * (c - 1) + 2 * t)
        ctx.eq(s.get("tile"), 4 * want_slot, f"{label}: {name}'s icon slot (country {c})")
        fp = ctx.rules.co_bonus(co, mode, t, 0)
        rng = ctx.rules.co_bonus(co, mode, t, 3)
        mv = ctx.rules.co_bonus(co, mode, t, 2)
        if "bar" in s or t not in (7, 13, 18, 20, 23, 27):
            want = BAR_SPRITE[bar_level(fp)] if ctx.ds else BAR_SPRITE.get(fp, 0x9E)
            ctx.eq(s.get("bar"), want, f"{label}: {name}'s firepower bar ({fp:+d}%)")
        else:
            ctx.check("bar" not in s, f"{label}: {name} (no weapon) has no bar")
        if rng:
            ctx.eq(s.get("range"), LINE_SPRITE.get(rng, 0x98), f"{label}: {name}'s range {rng:+d}")
        elif mv:
            ctx.eq(s.get("move"), LINE_SPRITE.get(mv, 0x98), f"{label}: {name}'s move {mv:+d}")
        else:
            ctx.check("range" not in s and "move" not in s, f"{label}: {name} has no move/range line")
    # The icons: the army's colours, the sheet's pictures, the new units in
    # slots of other countries' Infantry/Mech only.
    ctx.eq(g.e.read(OBJ_PALETTE_0, 32), g.e.read(UNIT_PALETTES + 32 * (colour - 1), 32),
           f"{label}: the icons' palette is {COLOURS.get(colour, colour)}'s")
    for t in set(types) - {0}:
        slot = (free[NEW_UNITS.index(t)] if t in NEW_UNITS
                else g.e.u16(AW2_SLOTS + 0x32 * (c - 1) + 2 * t))
        got = g.e.read(SHEET_VRAM + 128 * slot, 128)
        want = (g.e.read(NEW_ART + 128 * NEW_UNITS.index(t), 128) if t in NEW_UNITS
                else g.e.read(AW2_SHEET + 128 * slot, 128))
        ctx.check(got == want, f"{label}: {romlib.UNIT_NAMES[t]}'s picture is in its slot {slot}")
    ctx.check(len(set(new_tiles)) == len(new_tiles) and not any(t // 4 in own for t in new_tiles),
              f"{label}: new units use their own slots, not {COLOURS.get(c, c)}'s Infantry/Mech")
    return co, mode, colour


def view_all(ctx, g, prefix=""):
    """Map menu > CO, to the grid, then every army in turn (RIGHT). Returns
    the colours seen."""
    g.open_map_menu()
    g.choose("CO", g.MAP_MENU)
    g.e.wait(90)
    for _ in range(4):
        g.e.press("DOWN", 6)
        g.e.wait(45)
    seen = []
    for _ in range(6):
        g.e.wait(30)
        army, sheet, slots = draw(g)
        if army is None or army in [a for a, _ in seen]:
            break
        p = g.player(army)
        who = "human" if p["raw"][0x1B] == 1 else "CPU"
        label = f"army {army} ({COLOURS.get(p['colour'])}, {who}, {romlib.co_name(p['co'])} mode {p['co_mode']})"
        ctx.eq(g.e.u32(PAGE), 4, f"{label}: grid page")
        if ctx.ds:
            c = country(g, p["co"])
            ctx.eq(sheet, SHEETS + SHEET_SIZE * (c - 1), f"{label}: tangoAW2's sheet for country {c}")
            check_page(ctx, g, army, slots, GROUND, label + " ground page")
            ctx.shot(g, f"{prefix}a{army}_p4")
            g.e.press("DOWN", 6)
            g.e.wait(45)
            ctx.eq(g.e.u32(PAGE), 5, f"{label}: DOWN goes on to the second grid page")
            army2, _, slots = draw(g)
            ctx.eq(army2, army, f"{label}: same army")
            check_page(ctx, g, army, slots, AIR_SEA, label + " air/sea page")
            ctx.shot(g, f"{prefix}a{army}_p5")
            g.e.press("UP", 6)
            g.e.wait(45)
            ctx.eq(g.e.u32(PAGE), 4, f"{label}: UP goes back")
        else:
            ctx.eq(sheet, AW2_SHEET, f"{label}: the game's sheet")
            want = [g.e.u16(AW2_LIST + 2 * i) for i in range(20)]
            check_page(ctx, g, army, slots, want, label)
            ctx.shot(g, f"{prefix}a{army}_p4")
            g.e.press("DOWN", 6)
            g.e.wait(45)
            ctx.eq(g.e.u32(PAGE), 4, f"{label}: one grid page without the pack")
        seen.append((army, p["colour"]))
        g.e.press("RIGHT", 6)
        g.e.wait(90)
    ctx.log(f"viewed {seen}")
    return seen


def set_mode(g, army, mode):
    g.e.w8(g.player(army)["addr"] + 0x1E, mode)


@test()
def co_grid_aw2_cos(ctx):
    """Kanbei (every unit up) and Drake (naval up, air down; a CPU)."""
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["kanbei", "drake"], trace=trace_file(ctx))
    seen = view_all(ctx, g)
    ctx.eq([a for a, _ in seen], [1, 2], "both armies viewed")


@test()
def co_grid_powers(ctx):
    """Eagle's COP (air) as a human, Grit's SCOP (indirects, range) as a CPU."""
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["eagle", "grit"], trace=trace_file(ctx))
    set_mode(g, 1, 1)
    set_mode(g, 2, 2)
    view_all(ctx, g)


@test(modes=("ds",))
def co_grid_new_cos(ctx):
    """Javier (indirect defence: no bar change) as a human in his SCOP, Von
    Bolt (all units up) as a CPU in his SCOP."""
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["javier", "vonbolt"], trace=trace_file(ctx))
    set_mode(g, 1, 2)
    set_mode(g, 2, 2)
    view_all(ctx, g)


@test(modes=("ds",))
def co_grid_new_cos_day(ctx):
    """Grimm and Jake (plains; Jake's SCOP moves vehicles) day to day, then Jake
    in his SCOP; Sasha a CPU."""
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["jake", "grimm"], trace=trace_file(ctx))
    view_all(ctx, g, "day_")
    g.e.press("B", 6)
    g.wait_for_input()
    set_mode(g, 1, 2)
    set_mode(g, 2, 1)
    view_all(ctx, g, "power_")


@test()
def co_grid_four_armies(ctx):
    """Four armies in Green Earth, Yellow Comet, Orange Star, Blue Moon;
    humans 1 and 3, CPUs 2 and 4."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.colours = [0, 3, 4, 1, 2]
    cos = ["sensei", "hawke", "colin", "sonja"] if not ctx.ds else ["sensei", "koal", "colin", "rachel"]
    g = ctx.start(m, cos, humans=(1, 3), trace=trace_file(ctx))
    set_mode(g, 4, 1)
    seen = view_all(ctx, g)
    ctx.eq(sorted(c for _, c in seen), [1, 2, 3, 4], "every colour viewed")


@test()
def co_grid_black_hole(ctx):
    """Black Hole as army 1's colour (human), Yellow Comet the CPU."""
    m = ctx.map()
    m.colours = [0, 5, 4, 2, 3]
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["adder", "kindle" if ctx.ds else "lash"], trace=trace_file(ctx))
    seen = view_all(ctx, g)
    ctx.eq([c for _, c in seen], [5, 4], "Black Hole and Yellow Comet viewed")


@test()
def co_grid_five_armies(ctx):
    """A five-army game: Black Hole is army 5 (a CPU, in its COP)."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 15, 0x1B4)  # Black Hole's HQ
    m.colours = [5, 1, 2, 3, 4]  # the five-army mark
    g = ctx.start(m, None, trace=trace_file(ctx))
    set_mode(g, 5, 1)
    set_mode(g, 2, 2)
    seen = view_all(ctx, g)
    ctx.eq(sorted(c for _, c in seen), [1, 2, 3, 4, 5], "all five colours viewed")
