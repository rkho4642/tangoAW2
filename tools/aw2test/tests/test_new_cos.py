"""Dual Strike's nine new COs (crate::co_new, ids 72..80) with the pack: they are
on the Teams list in their army's group, play a battle with Dual Strike's
numbers (checked against the calculator), and show their pictures."""

from aw2test import rom as romlib
from aw2test.harness import test

NEW = ["jugger", "koal", "kindle", "vonbolt", "grimm", "javier", "sasha", "jake", "rachel"]


@test(modes=("ds",))
def new_cos_on_the_teams_list(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.boot_teams(m)
    lst = g.teams()["co_list"]
    ctx.log(f"Teams list: {[romlib.co_name(c) for c in lst]}")
    for name in NEW:
        ctx.check(romlib.co_id(name) in lst, f"{name} on the Teams list")
    ctx.eq(len(lst), 28, "28 COs")
    order = [romlib.co_name(c) for c in lst]
    ctx.check(order.index("Jake") == order.index("Hachi") + 1, "Jake after Hachi")
    ctx.check(order.index("Grimm") == order.index("Sensei") + 1, "Grimm after Sensei")


@test(modes=("ds",))
def grimm_vs_jugger(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["grimm", "jugger"])
    ctx.shot(g, "map")
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    # Tank vs tank 55, Grimm +30%.
    ctx.eq(r["first"].base, 71, "Grimm's tank base damage")
    ctx.eq(r["first"].luck, (10, 0), "Grimm's luck")
    if r["counter"]:
        ctx.eq(r["counter"].luck, (30, 15), "Jugger's luck")


@test(modes=("ds",))
def every_new_co_starts(ctx):
    for k in range(0, 9, 2):
        pair = [NEW[k], NEW[(k + 1) % 9]]
        m = ctx.map()
        m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
        g = ctx.start(m, pair)
        ctx.eq(g.player(1)["co"], romlib.co_id(pair[0]), f"army 1 plays {pair[0]}")
        ctx.eq(g.player(2)["co"], romlib.co_id(pair[1]), f"army 2 plays {pair[1]}")
        ctx.shot(g, f"map_{pair[0]}")
