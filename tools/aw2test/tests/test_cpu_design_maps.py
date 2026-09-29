"""CPUs play Design Room maps with everything the Dual Strike pack adds: the new
units on both sides, Com Towers, pipes, a Black Crystal and Obelisk, the
Wasteland look; two armies and five (Black Hole, army 5, a CPU). Several days of
CPU turns: they move, attack, capture and use the new units, and the battle
goes on (netplay too)."""

from aw2test.game import NavError
from aw2test.harness import test

LAB = {0: 0x1D9, 1: 0x1DA, 2: 0x1DB}
CRYSTAL, OBELISK = 0x192, 0x193


def everything_map(ctx, five=False):
    hq = ((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)) if five else ((1, 0, 0), (2, 29, 19))
    m = ctx.map(hq=hq)
    for x in range(12, 20):
        for y in range(15, 20):
            m.terrain(x, y, "sea")
    for x in range(4, 10):
        m.terrain(x, 12, "pipe")
    m.terrain(3, 12, "base", 1).terrain(25, 16, "base", 2).terrain(10, 12, "base", 2)
    m.terrain(14, 14, "port", 2).terrain(18, 14, "port", 1)
    m.terrain(22, 4, "airport", 2).terrain(6, 4, "airport", 1)
    for x, y in ((8, 8), (15, 6), (22, 10)):
        m.terrain(x, y, LAB[0])
    m.terrain(20, 13, LAB[2])
    m.unit(1, "megatank", 4, 3).unit(2, "megatank", 22, 12)
    m.unit(1, "piperunner", 5, 12).unit(2, "piperunner", 8, 12)
    m.unit(1, "stealth", 7, 6).unit(2, "stealth", 20, 8)
    m.unit(1, "blackbomb", 9, 4).unit(2, "blackbomb", 21, 6)
    m.unit(1, "blackboat", 13, 16).unit(2, "blackboat", 17, 17)
    m.unit(1, "carrier", 14, 17).unit(2, "carrier", 18, 18)
    m.unit(1, "oozium", 11, 9).unit(2, "oozium", 18, 9)
    m.unit(1, "infantry", 7, 8).unit(2, "infantry", 16, 6).unit(2, "infantry", 22, 11)
    m.unit(1, "tank", 12, 8).unit(2, "tank", 17, 10)
    if five:
        m.terrain(15, 2, 0x1B4)
        m.terrain(12, 3, CRYSTAL).terrain(24, 2, OBELISK)
        m.unit(5, "megatank", 14, 3).unit(5, "stealth", 16, 3).unit(5, "blackbomb", 13, 2).unit(5, "infantry", 13, 4)
        m.colours = [5, 1, 2, 3, 4]
    m.biome = 1
    return m


def play_days(ctx, g, days, human=1):
    moved = set()
    start = {u["id"]: (u["x"], u["y"], u["hp"]) for u in g.units()}
    for day in range(days):
        try:
            g.end_turn(human=human)
        except NavError as e:
            ctx.log(f"day {day}: {e}")
            break
        ctx.shot(g, f"day{day}")
        for u in g.units():
            if u["army"] != human and start.get(u["id"]) not in (None, (u["x"], u["y"], u["hp"])):
                moved.add(u["type"])
    return moved


@test(modes=("ds",))
def cpu_plays_design_map_with_everything(ctx):
    g = ctx.start(everything_map(ctx), ["andy", "vonbolt"])
    moved = play_days(ctx, g, 4)
    ctx.log(f"CPU unit types that moved or changed: {sorted(moved)}")
    ctx.check(len(moved & {4, 9, 12, 13, 18, 26, 27}) >= 4, f"the CPU played its new units: {sorted(moved)}")
    towers = sum(1 for y in range(20) for x in range(30) if g.terrain_class(x, y) == (2 << 5) | 0x14)
    ctx.check(towers >= 1, f"the CPU holds Com Towers ({towers})")
    ctx.check(not g.battle_over(), "the battle goes on")


@test(modes=("ds",))
def cpu_plays_five_army_design_map_with_everything(ctx):
    g = ctx.start(everything_map(ctx, five=True), None)
    moved = play_days(ctx, g, 3)
    ctx.log(f"CPU unit types that moved or changed: {sorted(moved)}")
    ctx.check(len(moved) >= 5, f"the CPUs played (five armies): {sorted(moved)}")
    ctx.check(not g.battle_over(), "the battle goes on")


@test(modes=("ds",), netplay=True)
def netplay_cpu_design_map_with_everything(ctx):
    g = ctx.start(everything_map(ctx), ["andy", "rachel"])
    play_days(ctx, g, 2)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")
