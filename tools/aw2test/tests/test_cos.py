"""AW2's COs with the Dual Strike pack take Dual Strike's numbers (tangoAW2's
co_roster): firepower and defence per unit class and kind of combat, move and
range, luck, counter, Sonja's lost enemy terrain stars. Each attack is checked
against the calculator, which reads them from the .nds in ds mode and from the
AW2 ROM in aw2 mode, and a few values are pinned by hand."""

from aw2test import rom as romlib
from aw2test.harness import test


def duel(ctx, cos, att="tank", dfd="tank", terrain=None):
    m = ctx.map()
    if terrain:
        m.terrain(11, 6, terrain)
    m.unit(1, att, 10, 6).unit(2, dfd, 11, 6)
    return m, ctx.start(m, cos)


@test()
def co_jess_vehicles(ctx):
    _, g = duel(ctx, ["jess", "andy"])
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    # Tank vs tank 55: Jess's vehicles +20% in Dual Strike, +10% in AW2.
    ctx.eq(r["first"].base, {"aw2": 60, "ds": 66}[ctx.mode], "Jess's tank base damage")


@test()
def co_sonja_counter_and_terrain(ctx):
    """Dual Strike's Sonja has no counter bonus, and her enemies lose a terrain star."""
    m = ctx.map()
    m.terrain(11, 6, "wood")
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["andy", "sonja"])
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    counter = r["counter"]
    ctx.check(counter is not None, "Sonja's tank counters")
    want = {"aw2": 150, "ds": 100}[ctx.mode]
    ctx.eq(counter.acc, want, "Sonja's counter firepower")


@test()
def co_kanbei_defence(ctx):
    _, g = duel(ctx, ["andy", "kanbei"])
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    ctx.eq(r["first"].defence >= 120, True, "Kanbei's tank defends at 120% or more")


@test()
def co_sensei_copter(ctx):
    _, g = duel(ctx, ["sensei", "andy"], att="bcopter")
    ctx.attack(g, (10, 6), (10, 6), (11, 6))


@test()
def co_max_artillery_range(ctx):
    """Max's indirect units lose a square of range (both games)."""
    m = ctx.map()
    m.unit(1, "artillery", 10, 6).unit(2, "tank", 13, 6)
    g = ctx.start(m, ["max", "andy"])
    ctx.eq(ctx.rules.co_bonus(2, 0, romlib.UNIT_IDS["artillery"], 3), -1, "calculator: Max's range")
    g.select(10, 6)
    menu = g.move_to(10, 6)
    ctx.check("Fire" not in menu["names"], f"no Fire at 3 squares: {menu['names']}")


@test()
def co_eagle_lightning_drive(ctx):
    """Dual Strike's Lightning Drive lowers firepower (its units move again)."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["eagle", "andy"])
    ctx.power(g, 1, "power")
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10))
    # 55 x (100 - 50 + 10) %; AW2's Lightning Drive leaves ground units alone.
    ctx.eq(r["first"].base, {"aw2": 55, "ds": 33}[ctx.mode], "tank base damage in Lightning Drive")


@test()
def co_andy_hyper_upgrade_ds(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "max"])
    ctx.power(g, 1, "super")
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.eq(r["first"].base, {"aw2": 66, "ds": 71}[ctx.mode], "Hyper Upgrade tank base damage")
