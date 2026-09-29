"""Unit-vs-unit battles: the game's result against the independent calculator.

Each test runs twice: with AW2's own rules (`aw2`) and with the Dual Strike pack
on (`ds`, base damage from Dual Strike's chart in the .nds). Base damage is also
pinned to the published charts' numbers, so a wrong chart fails even if the
calculator and the game agree.
"""

from aw2test.harness import test

# Published base damage (primary unless noted): AW2 chart / Dual Strike chart.
TANK_VS_TANK = {"aw2": 55, "ds": 55}
INF_MG_VS_MECH = {"aw2": 45, "ds": 45}
ART_VS_TANK = {"aw2": 70, "ds": 70}
FIGHTER_VS_BCOPTER = {"aw2": 100, "ds": 120}
CRUISER_VS_LANDER = {"aw2": 0, "ds": 25}
AA_VS_BCOPTER = {"aw2": 120, "ds": 105}


@test()
def tank_vs_tank_plains(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=TANK_VS_TANK[ctx.mode], expect_weapon="primary")


@test()
def tank_moves_then_attacks(ctx):
    m = ctx.map()
    m.unit(1, "tank", 6, 10).unit(2, "tank", 11, 10)
    m.terrain(10, 10, "wood")
    g = ctx.start(m, ["andy", "max"])
    # moves 4 cells onto a wood (2 stars for the attacker's own defence)
    ctx.attack(g, (6, 10), (10, 10), (11, 10), expect_base=TANK_VS_TANK[ctx.mode])


@test()
def infantry_vs_mech_woods(ctx):
    m = ctx.map()
    m.terrain(11, 10, "wood")
    m.unit(1, "infantry", 10, 10).unit(2, "mech", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.eq(g.terrain_class(11, 10) & 0x1F, 4, "the mech stands on a wood")
    ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=INF_MG_VS_MECH[ctx.mode], expect_weapon="secondary")


@test()
def artillery_vs_tank_indirect(ctx):
    m = ctx.map()
    m.terrain(12, 10, "city", 0)
    m.unit(1, "artillery", 10, 10).unit(2, "tank", 12, 10)
    g = ctx.start(m, ["andy", "andy"])
    r = ctx.attack(g, (10, 10), (10, 10), (12, 10), expect_base=ART_VS_TANK[ctx.mode], expect_weapon="primary")
    ctx.check(r["counter"] is None, "no counter at range 2")


@test()
def fighter_vs_bcopter(ctx):
    m = ctx.map()
    m.terrain(11, 10, "mountain")   # air units get no terrain stars
    m.unit(1, "fighter", 10, 10).unit(2, "bcopter", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 10, 10, 45)       # a 5-HP fighter: 100 and 120 give different losses
    ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=FIGHTER_VS_BCOPTER[ctx.mode], expect_weapon="primary")


@test()
def antiair_vs_bcopter(ctx):
    m = ctx.map()
    m.unit(1, "antiair", 10, 10).unit(2, "bcopter", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=AA_VS_BCOPTER[ctx.mode], expect_weapon="primary")


@test()
def cruiser_vs_lander(ctx):
    m = ctx.map()
    for y in range(7, 14):
        for x in range(7, 15):
            m.terrain(x, y, "sea")
    m.unit(1, "cruiser", 10, 10).unit(2, "lander", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    if CRUISER_VS_LANDER[ctx.mode] == 0:
        # AW2: a Cruiser cannot hit a Lander, so its menu has no Fire.
        g.select(10, 10)
        menu = g.move_to(10, 10)
        ctx.check("Fire" not in menu["names"], f"no Fire against a Lander: menu {menu['names']}")
        g.choose("Wait", g.ACTION_MENU)
        g.wait_idle()
        ctx.eq(g.unit_at(11, 10)["hp"], 100, "Lander untouched")
    else:
        ctx.attack(g, (10, 10), (10, 10), (11, 10), expect_base=CRUISER_VS_LANDER[ctx.mode], expect_weapon="primary")


@test()
def co_firepower_hawke_vs_drake(ctx):
    """Hawke's +10% firepower on every unit; Drake's ships +10% defence."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["hawke", "drake"])
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.eq(r["first"].base, TANK_VS_TANK[ctx.mode] * 110 // 100, "Hawke's tank base damage (55 x 110%)")


@test()
def damaged_attacker(ctx):
    """A 4-HP tank hits for 4/10 of its damage; the defender counters at full strength."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "mdtank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 10, 10, 35)
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


@test()
def cpu_attacks_back(ctx):
    """End the turn; the CPU's tank attacks ours; check that battle too."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 12, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.wait_unit(10, 10)
    before = {u["id"]: dict(u) for u in g.units()}
    sides = {uid: ctx.side(g, u) for uid, u in before.items()}
    g.end_turn(human=1)
    after = {u["id"]: u for u in g.units()}
    mine = [uid for uid, u in before.items() if u["army"] == 1 and u["type"] == 5][0]
    cpu = [uid for uid, u in before.items() if u["army"] == 2 and u["type"] == 5][0]
    ctx.log(f"after the CPU turn: {after}")
    if not ctx.check(after[mine]["hp"] < 100, "the CPU attacked our tank"):
        return
    from aw2test import damage
    a = sides[cpu]
    a.terrain = g.terrain_class(after[cpu]["x"], after[cpu]["y"]) & 0x1F
    d = sides[mine]
    dist = abs(after[cpu]["x"] - after[mine]["x"]) + abs(after[cpu]["y"] - after[mine]["y"])
    first, counter = damage.battle(ctx.rules, a, d, dist, dfd_hp_after=after[mine]["hp"])
    ctx.log(f"  expected: {first.describe()}; counter {counter.describe() if counter else None}")
    ctx.check(100 - after[mine]["hp"] in first.losses, f"CPU's attack took {100 - after[mine]['hp']}, allowed {sorted(first.losses)}")
    ctx.check(100 - after[cpu]["hp"] in counter.losses, f"our counter took {100 - after[cpu]['hp']}, allowed {sorted(counter.losses)}")
