"""Dual Strike's unit pictures for the seven new units (pack only).

The build menu's right-hand panel and the unit information screen (R on a
unit) draw a unit's big picture from OBJ tiles 0x2E8.. (0x06015D00, 0x800
bytes), OBJ palettes 3 and 4, and an OAM layout. For a new unit all three
must be Dual Strike's (its `xinfo/` picture, its `battle/` palette for the
army's colour, its layout), in every army's colours, the human's and the
CPU's, with four and with five armies (Black Hole's Megatank is drawn
without its crew, as in Dual Strike).
"""

import struct

from aw2test import rom as romlib
from aw2test.harness import test

NEW = {4: "Megatank", 9: "Piperunner", 12: "Stealth", 13: "Black Bomb", 18: "Black Boat", 26: "Carrier", 27: "Oozium"}
SEA = (18, 26)
BUILD_BUFFER = 0x02023830
PICTURE_TILES = 0x06015D00
PICTURE_TILE = 0x2E8
OBJ_PALETTE_3 = 0x05000260
OAM = 0x07000000
CO_TABLE_POOL = 0x08042DDC
_DS = []


def ds():
    if not _DS:
        _DS.append(romlib.DualStrike())
    return _DS[0]


def ds_id(t):
    return {26: 25, 27: 26}.get(t, t)


def country(g, army):
    """The army's CO's country, 1..5 (`sub_08042DCC`): picks the picture."""
    co = g.player(army)["co"]
    return g.e.u8(g.e.u32(CO_TABLE_POOL) + romlib.CO_RECORD * co + 0x15) + 1


def picture_sprites(g):
    """The OAM entries drawing the picture: (shape, size, tile, palette bank)."""
    oam = g.e.read(OAM, 0x400)
    out = []
    for k in range(128):
        a0, a1, a2 = struct.unpack_from("<3H", oam, 8 * k)
        if a0 & 0x300 == 0x200:
            continue
        tile = a2 & 0x3FF
        if PICTURE_TILE <= tile < PICTURE_TILE + 64 and (a2 >> 12) in (3, 4):
            out.append((a0 >> 14, a1 >> 14, tile - PICTURE_TILE, (a2 >> 12) - 3))
    return out


def check_picture(ctx, g, t, army, label):
    """The panel on screen shows unit t's Dual Strike picture for `army`."""
    colour = g.player(army)["colour"]
    c = country(g, army)
    layout, tiles, _ = ds().unit_picture(ds_id(t), c - 1)
    _, _, colours = ds().unit_picture(ds_id(t), colour - 1)
    ctx.check(g.e.read(PICTURE_TILES, len(tiles)) == tiles,
              f"{label}: OBJ tiles 0x2E8.. are Dual Strike's {NEW[t]} picture (country {c})")
    got = g.e.read(OBJ_PALETTE_3, 64)
    ctx.check(got[2:32] == colours[2:32] and got[34:64] == colours[34:64],
              f"{label}: OBJ palettes 3 and 4 are Dual Strike's {NEW[t]} colours for colour {colour}")
    want = []
    for k in range(struct.unpack_from("<H", layout, 0)[0]):
        a0, a1, a2 = struct.unpack_from("<3H", layout, 2 + 6 * k)
        want.append((a0 >> 14, a1 >> 14, a2 & 0x3FF, a2 >> 12))
    ctx.eq(sorted(picture_sprites(g)), sorted(want), f"{label}: the picture's sprites are Dual Strike's layout")
    return c


def open_menu(g, x, y):
    g.wait_idle()
    g.goto(x, y)
    g.e.press("A", 4)
    g.e.wait(60)
    ids = []
    for k in range(30):
        t = g.e.u8(BUILD_BUFFER + 4 * k)
        if t == 0:
            break
        ids.append(t)
    return ids


BUILD_MENU_PROC = 0x0802DA19


def close_menu(g):
    for k in range(5):
        g.e.press("B", 4)
        g.e.wait(30)
        if not g.has_proc(BUILD_MENU_PROC):
            break
    g.wait_for_input()


def check_menus(ctx, g, army, facilities):
    """Every facility's menu, cursor on each new unit in turn."""
    for name, (x, y) in facilities.items():
        ids = open_menu(g, x, y)
        pos = 0
        for i, t in enumerate(ids):
            if t not in NEW:
                continue
            while pos < i:
                g.e.press("DOWN", 4)
                g.e.wait(8)
                pos += 1
            g.e.wait(20)
            check_picture(ctx, g, t, army, f"army {army} {name} {NEW[t]}")
            ctx.shot(g, f"army{army}_{name}_{t}")
        ctx.check(any(t in NEW for t in ids), f"army {army}: the {name} sells a new unit")
        close_menu(g)
    # Off the facilities, so ending the turn opens the map menu.
    g.goto(20, 10)


def five_army_map(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)  # Black Hole's HQ
    m.unit(5, "infantry", 16, 10)
    m.colours = [5, 1, 2, 3, 4]  # the five-army mark
    return m


FACILITY_ROWS = {1: 3, 2: 16, 3: 6, 4: 13, 5: 8}


def facilities(m, army):
    """A base, an airport and a port for `army`, the port by the sea."""
    y = FACILITY_ROWS[army]
    m.terrain(4, y, "base", army).terrain(6, y, "airport", army).terrain(8, y, "port", army)
    m.terrain(9, y, "sea").terrain(8, y + 1, "sea")
    return {"base": (4, y), "airport": (6, y), "port": (8, y)}


@test(modes=("ds",))
def unit_pictures_build_menus_four_armies(ctx):
    """Orange Star's and Blue Moon's build menus: each CO's country's
    pictures in its army's colours."""
    m = ctx.map()
    fac = {a: facilities(m, a) for a in (1, 2)}
    g = ctx.start(m, ["andy", "olaf"], humans=(1, 2))
    for army in (1, 2):
        ctx.eq(g.current_army(), army, "army's turn")
        check_menus(ctx, g, army, fac[army])
        if army == 1:
            g.end_turn(human=2)


@test(modes=("ds",))
def unit_pictures_build_menus_five_armies(ctx):
    """All five armies' base, airport and port menus in a five-army game:
    Orange Star, Blue Moon, Green Earth, Yellow Comet and Black Hole."""
    m = five_army_map(ctx)
    fac = {a: facilities(m, a) for a in range(1, 6)}
    g = ctx.start(m, None, humans=(1, 2, 3, 4, 5))
    colours = [g.player(a)["colour"] for a in range(1, 6)]
    ctx.log(f"colours {colours}, countries {[country(g, a) for a in range(1, 6)]}")
    ctx.eq(colours, [1, 2, 3, 4, 5], "the five armies' colours")
    for army in range(1, 6):
        ctx.eq(g.current_army(), army, "army's turn")
        check_menus(ctx, g, army, fac[army])
        if army < 5:
            g.end_turn(human=army + 1)
    ctx.eq(country(g, 5), 5, "Black Hole's CO")


def inspect(ctx, g, units, army, prefix):
    """R on each of `army`'s new units (the unit information screen)."""
    for t, (x, y) in units.items():
        g.wait_idle()
        g.goto(x, y)
        g.e.wait(10)
        g.e.press("R", 4)
        g.e.wait(60)
        check_picture(ctx, g, t, army, f"{prefix} {NEW[t]} at {(x, y)}")
        ctx.shot(g, f"{prefix}_{t}")
        g.e.press("B", 4)
        g.e.wait(30)
        g.wait_for_input()


def place_new_units(m, army, row):
    units = {}
    for k, t in enumerate(NEW):
        x = 3 + 3 * k
        if t in SEA:
            m.terrain(x, row, "sea")
        m.unit(army, t, x, row)
        units[t] = (x, row)
    return units


@test(modes=("ds",))
def unit_pictures_cpu_units(ctx):
    """The information screen of a CPU army's new units (R on them): Dual
    Strike's pictures in the CPU's colours; the human's own too."""
    m = ctx.map()
    cpu = place_new_units(m, 2, 12)
    mine = place_new_units(m, 1, 6)
    g = ctx.start(m, ["andy", "olaf"])
    inspect(ctx, g, cpu, 2, "cpu_blue_moon")
    inspect(ctx, g, mine, 1, "own_orange_star")


@test(modes=("ds",))
def unit_pictures_cpu_black_hole(ctx):
    """Five armies: Black Hole (army 5, CPU) owns every new unit; the human
    inspects them: Black Hole's colours, and its crewless Megatank."""
    m = five_army_map(ctx)
    cpu = place_new_units(m, 5, 14)
    g = ctx.start(m, None)
    ctx.eq(g.player(5)["colour"], 5, "army 5 is Black Hole's colour")
    ctx.eq(country(g, 5), 5, "army 5's CO is Black Hole's")
    inspect(ctx, g, cpu, 5, "cpu_black_hole")
    _, crewless, _ = ds().unit_picture(4, 4)
    _, crewed, _ = ds().unit_picture(4, 0)
    ctx.check(crewless != crewed, "Black Hole's Megatank picture differs from the others'")


INTEL_LIST = 0x02028DD8
INTEL_FIRST_SLOT = 0x03003F2C


def open_unit_list(ctx, g, army, prefix):
    """Intel > Unit: every one of the army's units is listed, the new ones too."""
    g.goto(20, 10)
    g.open_map_menu()
    g.choose("Intel", g.MAP_MENU)
    g.e.wait(60)
    g.e.write(INTEL_LIST, bytes(64))
    g.choose("Unit")
    g.e.wait(90)
    ctx.shot(g, f"{prefix}_units")
    mine = g.units(army)
    first = g.e.u16(INTEL_FIRST_SLOT)
    idx = g.e.read(INTEL_LIST, len(mine))
    listed = sorted(g.e.u8(g.unit_addr(first + i)) for i in idx)
    ctx.eq(listed, sorted(u["type"] for u in mine), f"{prefix}: the Intel unit list has every unit")
    for _ in range(len(mine)):
        g.e.press("DOWN", 4)
        g.e.wait(12)
    g.e.wait(30)
    ctx.shot(g, f"{prefix}_units_end")
    for _ in range(6):
        g.e.press("B", 4)
        g.e.wait(40)
        if g.idle():
            break
    g.wait_for_input()


@test(modes=("ds",))
def unit_pictures_intel_list(ctx):
    """The Intel unit list shows every new unit, with its own map icon, for
    Orange Star and for Black Hole in a five-army game."""
    m = five_army_map(ctx)
    place_new_units(m, 1, 6)
    place_new_units(m, 5, 14)
    g = ctx.start(m, None, humans=(1, 2, 3, 4, 5))
    open_unit_list(ctx, g, 1, "orange_star")
    for army in range(1, 5):
        g.end_turn(human=army + 1)
    open_unit_list(ctx, g, 5, "black_hole")
