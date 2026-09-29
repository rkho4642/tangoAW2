"""Dual Strike's new units (pack only): build menus, stats, damage."""

import struct

from aw2test.harness import test

BUILD_BUFFER = 0x02023830
NEW = {4: "Megatank", 9: "Piperunner", 12: "Stealth", 13: "Black Bomb", 18: "Black Boat", 26: "Carrier", 27: "Oozium"}


def menu_at(ctx, g, x, y, name):
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
    ctx.shot(g, name)
    for _ in range(12):
        g.e.press("DOWN", 4)
        g.e.wait(6)
    g.e.wait(20)
    ctx.shot(g, name + "_end")
    g.e.press("B", 4)
    g.e.wait(30)
    g.wait_for_input()
    return ids


@test()
def build_menus(ctx):
    m = ctx.map()
    m.terrain(5, 5, "base", 1).terrain(7, 5, "airport", 1).terrain(9, 5, "port", 1)
    g = ctx.start(m, ["andy", "andy"])
    g.e.w32(g.player(1)["addr"], 90000)
    base = menu_at(ctx, g, 5, 5, "base")
    air = menu_at(ctx, g, 7, 5, "airport")
    port = menu_at(ctx, g, 9, 5, "port")
    ctx.log(f"base {base}\nairport {air}\nport {port}")
    if ctx.mode == "ds":
        ctx.eq(base, [1, 2, 6, 5, 3, 8, 4, 7, 10, 11, 14, 15, 9, 27], "base: the game's list with Megatank, Piperunner and Oozium")
        ctx.eq(air, [16, 17, 19, 20, 12, 13], "airport: with Stealth and Black Bomb")
        ctx.eq(port, [21, 22, 23, 24, 18, 26], "port: with Black Boat and Carrier")
    else:
        ctx.eq(base, [1, 2, 6, 5, 3, 8, 7, 10, 11, 14, 15], "base: AW2's list")
        ctx.eq(air, [16, 17, 19, 20], "airport: AW2's list")
        ctx.eq(port, [21, 22, 23, 24], "port: AW2's list")


@test(modes=("ds",))
def buy_and_move_new_units(ctx):
    m = ctx.map()
    m.terrain(5, 5, "base", 1).terrain(9, 5, "port", 1)
    for x in range(6, 12):
        m.terrain(x, 8, "pipe")
    m.terrain(5, 8, "base", 1)
    for x in range(10, 16):
        for y in range(3, 8):
            if (x, y) != (9, 5):
                m.terrain(x, y, "sea")
    g = ctx.start(m, ["andy", "andy"])
    g.e.w32(g.player(1)["addr"], 200000)
    g.buy(5, 5, 4)      # Megatank
    g.buy(5, 8, 9)      # Piperunner next to the pipe
    g.buy(9, 5, 26)     # Carrier
    ctx.shot(g, "bought")
    types = {(u["x"], u["y"]): u["type"] for u in g.units(1)}
    ctx.eq((types.get((5, 5)), types.get((5, 8)), types.get((9, 5))), (4, 9, 26), "units bought")
    g.end_turn()
    for (x, y), (tx, ty) in [((5, 5), (5, 3)), ((5, 8), (10, 8)), ((9, 5), (11, 5))]:
        g.select(x, y)
        g.e.wait(20)
        ctx.shot(g, f"range_{x}_{y}")
        g.move_to(tx, ty)
        g.choose("Wait", g.ACTION_MENU)
        g.wait_idle()
    moved = {(u["x"], u["y"]): u["type"] for u in g.units(1)}
    ctx.log(f"units now {moved}")
    ctx.eq(moved.get((5, 3)), 4, "Megatank moved (4 spaces of plain)")
    ctx.eq(moved.get((10, 8)), 9, "Piperunner moved along the pipe")
    ctx.eq(moved.get((11, 5)), 26, "Carrier moved on the sea")
    ctx.shot(g, "moved")


def fuel(g, x, y):
    return g.unit_at(x, y)["fuel"]


@test(modes=("ds",))
def stealth_hides(ctx):
    m = ctx.map()
    m.unit(1, 12, 10, 10).unit(2, "tank", 11, 10).unit(2, "fighter", 10, 12)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    names = g.action_menu_at(10, 10)
    ctx.check("Hide" in names, f"the Stealth offers Hide: {names}")
    f0 = fuel(g, 10, 10)
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Hide", g.ACTION_MENU)
    g.wait_idle()
    ctx.check(g.unit_at(10, 10)["flags"] & 0x20, "the Stealth is hidden")
    g.end_turn(human=2)
    tank = g.action_menu_at(11, 10)
    ctx.check("Fire" not in tank, f"a Tank cannot attack a hidden Stealth: {tank}")
    fighter = g.action_menu_at(10, 12)
    ctx.log(f"fighter menu at (10,12) without moving: {fighter}")
    g.end_turn(human=1)
    ctx.eq(f0 - fuel(g, 10, 10), 8, "a hidden Stealth burns 8 fuel a day")
    names = g.action_menu_at(10, 10)
    ctx.check("Appear" in names, f"and offers Appear: {names}")


@test(modes=("ds",))
def black_boat_repairs(ctx):
    m = ctx.map()
    for x in range(9, 13):
        for y in range(8, 13):
            m.terrain(x, y, "sea")
    m.terrain(12, 10, "plain")
    m.unit(1, 18, 11, 10).unit(1, "tank", 12, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 12, 10, 50)
    t = g.unit_at(12, 10)
    g.e.w16(g.unit_addr(t["id"]) + 4, (g.e.u16(g.unit_addr(t["id"]) + 4) & ~(0xF << 7)) | (3 << 7) | 50)
    funds0 = struct.unpack_from("<I", g.player(1)["raw"], 0)[0]
    names = g.action_menu_at(11, 10)
    ctx.check("Repair" in names, f"the Black Boat offers Repair: {names}")
    g.select(11, 10)
    g.move_to(11, 10)
    g.choose("Repair", g.ACTION_MENU)
    g.wait_for_input()
    t = g.unit_at(12, 10)
    ctx.eq(t["hp"], 60, "the Tank is repaired by 1 HP")
    ctx.eq(t["ammo"], 9, "and resupplied")
    funds1 = struct.unpack_from("<I", g.player(1)["raw"], 0)[0]
    ctx.eq(funds0 - funds1, 700, "for a tenth of the Tank's price")


@test(modes=("ds",))
def oozium_destroys(ctx):
    m = ctx.map()
    m.unit(1, 27, 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(11, 10)
    g.wait_for_input()
    ctx.check(g.unit_at(11, 10) is None, "the Tank is gone")


@test(modes=("ds",))
def carrier_resupplies_cargo(ctx):
    m = ctx.map()
    for x in range(9, 14):
        for y in range(8, 13):
            m.terrain(x, y, "sea")
    m.unit(1, 26, 11, 10).unit(1, "fighter", 11, 9)
    g = ctx.start(m, ["andy", "andy"])
    fid = g.unit_at(11, 9)["id"]
    a = g.unit_addr(fid) + 6
    g.e.w8(a, (g.e.u8(a) & 0x80) | 20)
    g.select(11, 9)
    g.move_to(11, 10)
    names = g.menu()["names"]
    ctx.log(f"menu on the Carrier: {names}")
    g.choose("Load", g.ACTION_MENU)
    g.wait_idle()
    g.end_turn()
    ctx.eq(g.e.u8(a) & 0x7F, 99, "the Fighter in the Carrier is refuelled at turn start")


@test(modes=("ds",))
def black_bomb_explodes(ctx):
    m = ctx.map()
    m.unit(1, 13, 5, 10)
    m.unit(2, "tank", 10, 10).unit(2, "infantry", 11, 11).unit(1, "tank", 9, 12)
    m.unit(2, 27, 12, 10).unit(2, "tank", 14, 10)  # Oozium in range; a Tank out of range
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 11, 11, 3)
    names = g.action_menu_at(5, 10)
    ctx.check("Explode" in names, f"the Black Bomb offers Explode: {names}")
    g.select(5, 10)
    g.move_to(9, 10)
    g.choose("Explode", g.ACTION_MENU)
    g.e.wait(120)
    ctx.shot(g, "boom")
    g.wait_for_input()
    hp = lambda x, y: (g.unit_at(x, y) or {}).get("hp")
    ctx.eq(hp(10, 10), 50, "enemy Tank next to it: 100 -> 50")
    ctx.eq(hp(11, 11), 1, "a unit left with less keeps 1 HP")
    ctx.eq(hp(9, 12), 50, "its own army's units too")
    ctx.eq(hp(12, 10), 100, "Oozium is unharmed")
    ctx.eq(hp(14, 10), 100, "beyond 3 spaces nothing")
    ctx.check(g.unit_at(9, 10) is None and g.unit_at(5, 10) is None, "the bomb is gone")
    g.end_turn()
    ctx.check(not g.battle_over(), "play goes on")


@test(modes=("ds",))
def cpu_plays_new_units(ctx):
    """A CPU army with every new unit: its turns run, and its units act."""
    m = ctx.map()
    for x in range(20, 30):
        for y in range(0, 6):
            m.terrain(x, y, "sea")
    for x in range(12, 20):
        m.terrain(x, 9, "pipe")
    m.terrain(11, 9, "base", 2)
    m.unit(2, 4, 16, 12).unit(2, 9, 14, 9).unit(2, 12, 18, 14).unit(2, 13, 20, 12)
    m.unit(2, 18, 24, 3).unit(2, 26, 26, 3).unit(2, 27, 15, 14).unit(2, "infantry", 23, 6)
    m.unit(1, "tank", 10, 12).unit(1, "tank", 12, 14).unit(1, "fighter", 14, 16).unit(1, "infantry", 10, 10)
    g = ctx.start(m, ["andy", "andy"], humans=(1,))
    before = {u["id"]: (u["x"], u["y"], u["hp"]) for u in g.units()}
    for day in range(3):
        g.end_turn()
        ctx.shot(g, f"day{day}")
    after = {u["id"]: (u["x"], u["y"], u["hp"]) for u in g.units()}
    moved = [t for t, (i, u) in ((u["type"], (u["id"], u)) for u in g.units(2)) if before.get(i, (0, 0, 0))[:2] != (u["x"], u["y"])]
    ctx.log(f"CPU units that moved: {moved}")
    hurt = [u["type"] for u in g.units(1) if before.get(u["id"], (0, 0, 100))[2] > u["hp"]]
    ctx.log(f"our units the CPU hurt: {hurt}")
    ctx.check(not g.battle_over(), "no hang, the battle goes on")
    ctx.check(len(moved) >= 3, f"several new units moved: {moved}")


@test(modes=("ds",))
def cpu_attacks_with_new_units(ctx):
    """Each attacking new unit, played by the CPU with our unit in reach, hits
    it (a CPU Tank is the control)."""
    cases = [("control Tank", "tank", (12, 10), "tank"), ("Megatank", 4, (13, 10), "tank"),
             ("Piperunner", 9, (12, 12), "tank"), ("Stealth", 12, (13, 11), "tank"),
             ("Oozium", 27, (11, 10), "tank"), ("Carrier", 26, (14, 10), "fighter")]
    for name, t, pos, target in cases:
        m = ctx.map()
        if t == 9:
            for x in range(8, 16):
                m.terrain(x, 12, "pipe")
        if t == 26:
            for x in range(13, 17):
                for y in range(8, 13):
                    m.terrain(x, y, "sea")
        m.unit(1, target, 10, 10).unit(2, t, *pos)
        g = ctx.start(m, ["andy", "andy"], humans=(1,))
        g.wait_unit(10, 10)
        g.end_turn()
        u = g.unit_at(10, 10)
        ctx.check(u is None or u["hp"] < 100, f"{name} attacked: our {target} now {u['hp'] if u else 'destroyed'}")
        g.e.close()


@test(modes=("ds",))
def cpu_builds_new_units(ctx):
    """A rich CPU with a base, an airport and a port over several days builds
    some of the new units (Megatank, Stealth, Black Boat), and its turns run."""
    m = ctx.map()
    for x in range(20, 30):
        for y in range(0, 8):
            m.terrain(x, y, "sea")
    m.terrain(25, 12, "base", 2).terrain(23, 14, "airport", 2).terrain(21, 8, "port", 2)
    m.unit(1, "tank", 10, 10).unit(1, "bomber", 5, 5).unit(1, "battleship", 25, 2)
    g = ctx.start(m, ["andy", "andy"], humans=(1,))
    built = set()
    for day in range(6):
        g.e.w32(g.player(2)["addr"], 90000)
        g.end_turn()
        built |= {u["type"] for u in g.units(2)}
    ctx.log(f"types the CPU has: {sorted(built)}")
    ctx.check(built & {4, 12, 18}, f"the CPU built a new unit: {sorted(built)}")
