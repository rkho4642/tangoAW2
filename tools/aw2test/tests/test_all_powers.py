"""Every CO's CO Power and Super CO Power, fired from the map menu: AW2's 19 in
both modes (AW2's numbers, then Dual Strike's with the pack), and Dual Strike's
nine new COs with the pack. Each power must start (mode, meter spent, uses +1);
then a Tank attack is checked against the calculator in that power's numbers,
the CPU's turn is played and the battle comes back to army 1 (a power that
breaks a later turn fails here). The effects themselves are pinned by
test_powers.py, test_cos.py and test_co_powers.py."""

from aw2test import rom as romlib
from aw2test.harness import test

AW2_COS = [n.lower() for n in romlib.CO_NAMES]
NEW_COS = ["jugger", "koal", "kindle", "vonbolt", "grimm", "javier", "sasha", "jake", "rachel"]
NO_COP = {"sturm", "vonbolt"}


def power_battle(ctx, co, which):
    m = ctx.map()
    m.terrain(4, 4, "city", 1).terrain(6, 4, "city", 2)
    m.unit(1, "tank", 10, 10).unit(1, "mech", 5, 5).unit(1, "artillery", 3, 12)
    m.unit(2, "tank", 11, 10).unit(2, "infantry", 6, 4).unit(2, "artillery", 20, 12)
    foe = "max" if co == "andy" else "andy"
    g = ctx.start(m, [co, foe])
    ctx.set_hp(g, 5, 5, 45)
    before, after = ctx.power(g, 1, which)
    ctx.shot(g, "after_power")
    for i, u in after.items():
        ctx.check(u["hp"] >= 1 or u["type"] == 0, f"unit {i} survives the power (a power never destroys)")
    tank = g.unit_at(10, 10)
    if tank and tank["hp"] > 0 and g.unit_at(11, 10):
        if tank["flags"] & 1:
            ctx.log("the tank is held (a stun or a moved flag): no attack")
        else:
            ctx.attack(g, (10, 10), (10, 10), (11, 10))
    g.end_turn()
    ctx.eq(g.current_army(), 1, "the turn comes back to army 1")
    ctx.shot(g, "next_turn")


def make(co, which, modes):
    def fn(ctx):
        power_battle(ctx, co, which)
    fn.__name__ = f"all_powers_{co}_{'cop' if which == 'power' else 'scop'}"
    test(modes=modes)(fn)


for co in AW2_COS + NEW_COS:
    modes = ("aw2", "ds") if co in AW2_COS else ("ds",)
    for which in ("power", "super"):
        if which == "power" and co in NO_COP:
            continue
        make(co, which, modes)


def netplay_power(ctx, co, which):
    m = ctx.map()
    m.terrain(4, 4, "city", 1).terrain(6, 4, "city", 2)
    m.unit(1, "tank", 10, 10).unit(1, "mech", 5, 5)
    m.unit(2, "tank", 11, 10).unit(2, "infantry", 6, 4).unit(2, "artillery", 20, 12)
    g = ctx.start(m, [co, "andy"])
    ctx.power(g, 1, which)
    if not g.unit_at(10, 10)["flags"] & 1:
        ctx.attack(g, (10, 10), (10, 10), (11, 10))
    g.end_turn(human=1)
    g.end_turn(human=1)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")


def make_netplay(co, which):
    def fn(ctx):
        netplay_power(ctx, co, which)
    fn.__name__ = f"netplay_powers_{co}_{'cop' if which == 'power' else 'scop'}"
    test(modes=("ds",), netplay=True)(fn)


for co in NEW_COS:
    for which in ("power", "super"):
        if which == "power" and co in NO_COP:
            continue
        make_netplay(co, which)
