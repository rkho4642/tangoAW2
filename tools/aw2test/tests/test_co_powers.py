"""What Dual Strike's COs do beyond numbers (crate::co_powers), with the pack:
Ex Machina's stun, Covering Fire, Urban Blight, High Society, Sasha's income,
Market Crash and War Bonds, Rachel's repairs, Javier's defence against indirect
attacks, Eagle's Lightning Drive."""

import struct

from aw2test import ram
from aw2test.harness import test

DS = ("ds",)


def funds(g, army):
    return struct.unpack_from("<I", g.player(army)["raw"], ram.P_FUNDS)[0]


@test(modes=DS)
def von_bolt_ex_machina_stuns(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, "tank", 20, 10).unit(2, "mech", 21, 10).unit(2, "infantry", 20, 11)
    g = ctx.start(m, ["vonbolt", "andy"], humans=(1, 2))
    before, after = ctx.power(g, 1, "super")
    hit = [i for i, u in before.items() if u["army"] == 2 and after[i]["hp"] < u["hp"]]
    ctx.check(len(hit) >= 2, f"the strike hit enemy units: {hit}")
    for i in hit:
        ctx.eq(before[i]["hp"] - after[i]["hp"], 30, f"unit {i}: 3 HP")
    g.end_turn(human=2)
    ctx.shot(g, "army2_turn")
    for u in g.units():
        if u["id"] in hit:
            ctx.check(u["flags"] & 1, f"unit {u['id']} is held on army 2's turn")
    g.end_turn(human=1)
    g.end_turn(human=2)
    for u in g.units():
        if u["id"] in hit:
            ctx.check(not u["flags"] & 1, f"unit {u['id']} is free on the turn after")


@test(modes=DS)
def rachel_covering_fire(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    for x in (8, 16, 24):
        m.unit(2, "tank", x, 10).unit(2, "mech", x + 1, 10)
    g = ctx.start(m, ["rachel", "andy"])
    before, after = ctx.power(g, 1, "super")
    lost = sum(u["hp"] - after[i]["hp"] for i, u in before.items() if u["army"] == 2)
    ctx.log(f"enemy HP lost: {lost}")
    ctx.check(lost >= 3 * 30, f"three strikes: {lost} internal HP taken")
    ctx.shot(g, "after")


@test(modes=DS)
def kindle_urban_blight(ctx):
    m = ctx.map()
    m.terrain(20, 10, "city", 0).terrain(22, 10, "city", 2)
    m.unit(1, "tank", 2, 2)
    m.unit(2, "infantry", 20, 10).unit(2, "infantry", 22, 10).unit(2, "tank", 24, 10)
    g = ctx.start(m, ["kindle", "andy"])
    before, after = ctx.power(g, 1, "power")
    at = {(u["x"], u["y"]): (u["hp"], after[i]["hp"]) for i, u in before.items() if u["army"] == 2}
    ctx.eq(at[(20, 10)][0] - at[(20, 10)][1], 30, "on a neutral city: 3 HP")
    ctx.eq(at[(22, 10)][0] - at[(22, 10)][1], 30, "on its own city: 3 HP")
    ctx.eq(at[(24, 10)][0] - at[(24, 10)][1], 0, "on plains: nothing")


@test(modes=DS)
def kindle_high_society(ctx):
    m = ctx.map()
    for x in range(2, 8):
        m.terrain(x, 2, "city", 1)
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["kindle", "andy"])
    ctx.power(g, 1, "super")
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    # 7 properties (6 cities and the HQ): +21% (and the power's 10).
    ctx.eq(r["first"].acc, 121, "High Society firepower")


@test(modes=DS)
def sasha_income(ctx):
    m = ctx.map()
    m.terrain(2, 2, "city", 1).terrain(4, 2, "base", 1)
    m.unit(1, "infantry", 10, 6).unit(2, "infantry", 20, 6)
    g = ctx.start(m, ["sasha", "andy"])
    f0 = funds(g, 1)
    g.end_turn()
    # HQ, city, base: 3000, and Sasha's 100 each.
    ctx.eq(funds(g, 1) - f0, 3300, "a day's income")


@test(modes=DS)
def sasha_market_crash(ctx):
    m = ctx.map()
    m.unit(1, "infantry", 10, 6).unit(2, "infantry", 20, 6)
    g = ctx.start(m, ["sasha", "andy"])
    g.e.w32(g.player(1)["addr"] + ram.P_FUNDS, 25000)
    g.e.w32(g.player(2)["addr"] + ram.P_CHARGE, 40000)
    ctx.power(g, 1, "power")
    # 25000 funds: 50% of Andy's full meter (6 stars x 9000 = 54000).
    ctx.eq(g.player(2)["charge"], 40000 - 27000, "Andy's meter after Market Crash")


@test(modes=DS)
def sasha_war_bonds(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["sasha", "andy"])
    ctx.power(g, 1, "super")
    f0 = funds(g, 1)
    r = ctx.attack(g, (10, 6), (10, 6), (11, 6))
    bars = lambda hp: (hp - 1) // 10 + 1 if hp > 0 else 0
    lost = bars(r["before"][1]["hp"]) - bars(r["after"][1]["hp"])
    ctx.eq(funds(g, 1) - f0, lost * 7000 // 10 // 2, f"War Bonds for {lost} HP of a Tank")


@test(modes=DS)
def rachel_repairs(ctx):
    m = ctx.map()
    m.terrain(5, 5, "city", 1)
    m.unit(1, "infantry", 5, 5).unit(2, "infantry", 20, 6)
    g = ctx.start(m, ["rachel", "andy"])
    ctx.set_hp(g, 5, 5, 40)
    g.end_turn()
    ctx.eq(g.unit_at(5, 5)["hp"], 70, "4 HP + 3 at the turn start")


@test(modes=DS)
def javier_indirect_defence(ctx):
    m = ctx.map()
    m.unit(1, "artillery", 10, 6).unit(2, "tank", 12, 6)
    g = ctx.start(m, ["andy", "javier"])
    r = ctx.attack(g, (10, 6), (10, 6), (12, 6))
    ctx.eq(r["first"].defence, 130, "Javier's tank vs artillery (plains 10 + 20)")


@test(modes=DS)
def javier_tower_of_power_indirect_defence(ctx):
    """Tower of Power: +80 against indirects on top of the power's +10, and the
    defence stops at 200 as in Dual Strike (0x020C34C8): no damage, no healing."""
    m = ctx.map()
    m.terrain(12, 6, "wood")
    m.unit(1, "artillery", 10, 6).unit(1, "infantry", 2, 2).unit(2, "tank", 12, 6).unit(2, "infantry", 20, 10)
    g = ctx.start(m, ["andy", "javier"], humans=(1, 2))
    g.end_turn(human=2)
    ctx.power(g, 2, "super")
    g.end_turn(human=1)
    r = ctx.attack(g, (10, 6), (10, 6), (12, 6))
    ctx.eq(r["first"].defence, 200, "Javier's tank in a wood vs artillery (20 + 100 + 10 + 80, capped)")
    ctx.eq(r["after"][1]["hp"], r["before"][1]["hp"], "no damage and no healing")


@test(modes=DS)
def eagle_lightning_drive_moves_again(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(1, "infantry", 12, 12).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["eagle", "andy"])
    g.select(10, 10)
    g.move_to(11, 10)
    g.choose("Wait", g.ACTION_MENU)
    g.wait_idle()
    ctx.check(g.unit_at(11, 10)["flags"] & 1, "the tank has moved")
    ctx.power(g, 1, "power")
    ctx.check(not g.unit_at(11, 10)["flags"] & 1, "Lightning Drive: the tank can move again")
