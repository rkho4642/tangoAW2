"""CO Powers and Super CO Powers fired from the map menu, their effects checked.

Expected effects are the games' published ones (AW2 manual / CO pages):
  Andy   Hyper Repair  (COP)  every own unit +2 HP (whole HP, rounded up: RepairUnit)
         Hyper Upgrade (SCOP) every own unit +5 HP, firepower +20% (incl. the power bonus)
  Hawke  Black Wave    (COP)  every enemy -1 HP, every own unit +1 HP
         Black Storm   (SCOP) every enemy -2 HP, every own unit +2 HP
  Drake  Tsunami       (COP)  every enemy -1 HP, enemy fuel halved
         Typhoon       (SCOP) every enemy -2 HP, enemy fuel halved, rain
Powers never destroy a unit (1 internal HP is the floor) and heal up to 10 HP.
Every power gives its army +10% defence (gPlayers[].tempDefense = 10).
After the power an attack is made and checked with the power's stats.
"""

from aw2test import damage
from aw2test.harness import test

TANK = {"aw2": 55, "ds": 55}


def power_map(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(1, "mech", 5, 5)
    m.unit(2, "tank", 11, 10).unit(2, "artillery", 20, 12)
    return m


def run_power(ctx, cos, which, own_delta, enemy_delta, halve_fuel=False, weather=None, repair=False):
    g = ctx.start(power_map(ctx), cos)
    ctx.set_hp(g, 5, 5, 45)      # own mech hurt: heals are visible
    ctx.set_hp(g, 20, 12, 15)    # enemy artillery nearly dead: the 1-HP floor
    before, after = ctx.power(g, 1, which)
    ctx.expect_hp_change(before, after, {1: own_delta, 2: enemy_delta}, f"{cos[0]} {which}", repair=repair)
    for uid, u0 in before.items():
        if u0["army"] == 2:
            want = u0["fuel"] >> 1 if halve_fuel else u0["fuel"]
            ctx.eq(after[uid]["fuel"], want, f"enemy {u0['type']} at {(u0['x'], u0['y'])} fuel")
    p = g.player(1)
    ctx.eq(p["temp_defence"], 10, "power's +10% defence (tempDefense)")
    ctx.eq(p["temp_firepower"], 0, "tempFirepower")
    if weather is not None:
        ctx.eq(g.playst()["weather"], weather, "weather after the power")
    ctx.shot(g, "after_power")
    return g


@test()
def andy_hyper_repair(ctx):
    g = run_power(ctx, ["andy", "max"], "power", +20, 0, repair=True)
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.eq(r["first"].base, TANK[ctx.mode], "Hyper Repair leaves firepower alone")


@test()
def andy_hyper_upgrade(ctx):
    g = run_power(ctx, ["andy", "max"], "super", +50, 0, repair=True)
    r = ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.eq(r["first"].base, TANK[ctx.mode] * 120 // 100, "Hyper Upgrade: tank base damage x 120%")


@test()
def hawke_black_wave(ctx):
    g = run_power(ctx, ["hawke", "andy"], "power", +10, -10)
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


@test()
def hawke_black_storm(ctx):
    g = run_power(ctx, ["hawke", "andy"], "super", +20, -20)
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


@test()
def drake_tsunami(ctx):
    g = run_power(ctx, ["drake", "andy"], "power", 0, -10, halve_fuel=True)
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


@test()
def drake_typhoon(ctx):
    g = run_power(ctx, ["drake", "andy"], "super", 0, -20, halve_fuel=True, weather=2)
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


@test()
def power_meter_fills_from_battles(ctx):
    """No pokes: the meter fills from real battles, and the map menu offers Power
    exactly when the charge reaches the COP cost."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.check("Power" not in g.map_menu_names(), "no Power before any battle")
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    p = g.player(1)
    cop, scop = ctx.image.co_stars(p["co"])
    need = 9000 * cop
    names = g.map_menu_names()
    ctx.eq("Power" in names, p["charge"] >= need, f"Power offered iff charge {p['charge']} >= {need}: {names}")
