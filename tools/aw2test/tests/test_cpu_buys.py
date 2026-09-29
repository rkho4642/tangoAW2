"""The CPU buys every unit, the new ones included (crate::cpu_tactics, unit_actions),
each at its own kind of factory: bases (Megatank, Piperunner by pipes, Oozium),
airports (Stealth, Black Bomb), ports (Black Boat, Carrier)."""

from aw2test import ram, rom as romlib
from aw2test.game import NavError
from aw2test.harness import test

NAMES = {4: "Megatank", 9: "Piperunner", 12: "Stealth", 13: "Black Bomb", 18: "Black Boat", 26: "Carrier", 27: "Oozium"}
FACTORY = {4: 14, 9: 14, 27: 14, 12: 10, 13: 10, 18: 11, 26: 11}


def big_map(ctx):
    m = ctx.map()
    for x in range(14, 30):
        for y in range(12, 20):
            m.terrain(x, y, "sea")
    for x, y in ((20, 4), (22, 4), (24, 4), (26, 4)):
        m.terrain(x, y, "base", 2)
    for x in range(20, 27):
        m.terrain(x, 5, "pipe")
    for x, y in ((20, 8), (23, 8)):
        m.terrain(x, y, "airport", 2)
    for x, y in ((16, 11), (20, 11), (24, 11)):
        m.terrain(x, y, "port", 2)
    # Army 1: ground, air and sea, so the CPU wants a bit of everything.
    m.unit(1, "tank", 4, 4).unit(1, "mdtank", 5, 4).unit(1, "artillery", 4, 6).unit(1, "fighter", 6, 8)
    m.unit(1, "bomber", 7, 8).unit(1, "bcopter", 8, 6).unit(1, "battleship", 16, 16).unit(1, "cruiser", 18, 18)
    m.unit(1, "sub", 20, 17).unit(1, "lander", 22, 18)
    for x, y in ((1, 1), (2, 1), (1, 2), (2, 2), (3, 1)):
        m.unit(1, "neotank", x, y)
    return m


def play(ctx, funds, bought, seen_all):
    g = ctx.start(big_map(ctx), ["andy", "andy"])
    for day in range(10):
        g.e.w32(g.player(2)["addr"] + ram.P_FUNDS, funds)
        try:
            g.end_turn()
        except NavError:
            ctx.log(f"funds {funds}: the battle ended on day {day}")
            break
        for u in g.units(2):
            key = (funds, u["id"])
            seen_all.setdefault(key, u["type"])
            if u["type"] in NAMES and key not in bought:
                bought[key] = (u["type"], g.terrain_class(u["x"], u["y"]) & 0x1F, day)
    ctx.shot(g, f"army_{funds}")


@test(modes=("ds",))
def cpu_buys_every_new_unit(ctx):
    """Two battles, a CPU with 60000 and with 100000 funds a day (what it buys
    depends on what it can pay): every new unit is bought, at its factory."""
    bought, seen_all = {}, {}
    for funds in (60000, 100000):
        play(ctx, funds, bought, seen_all)
    got = {}
    for t, cls, day in bought.values():
        got.setdefault(t, []).append((cls, day))
    ctx.log(f"all army 2 types: {sorted(seen_all.values())}")
    ctx.log("bought: " + ", ".join(f"{NAMES[t]} x{len(v)} (factories {sorted({c for c, _ in v})})" for t, v in sorted(got.items())))
    for t, name in NAMES.items():
        ctx.check(t in got, f"the CPU bought a {name}")
        for cls, day in got.get(t, []):
            ctx.check(cls == FACTORY[t], f"{name} bought at terrain {cls} (want {FACTORY[t]}) on day {day}")
