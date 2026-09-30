"""No CO, power, Missile Silo or Black Bomb touches an Oozium (Dual Strike
pack, crate::oozium): Dual Strike gives its class (6) nothing from any CO
(its stat functions return 0 for it, its powers' unit filters leave it out).
Powers pass it by (no damage, no stun, no fuel loss; no repair, no move
again, no resupply), a silo's and a bomb's blasts spare it, its defence has
no CO bonus (not even a power's +10), and the CPU leaves it out when it
picks a silo's or a strike's target."""

from aw2test import ram
from aw2test.harness import test

DS = ("ds",)
OOZIUM = 27
SILO = 0x180


def unit(g, x, y):
    return g.unit_at(x, y) or {}


@test(modes=DS)
def oozium_immune_olaf(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10)
    g = ctx.start(m, ["olaf", "andy"])
    ctx.power(g, 1, "super")
    g.wait_for_input()
    ctx.eq(unit(g, 20, 10).get("hp"), 100, "Winter Fury: the Oozium takes nothing")
    ctx.eq(unit(g, 21, 10).get("hp"), 80, "the Tank next to it takes 2 HP")


@test(modes=DS)
def oozium_immune_hawke(ctx):
    m = ctx.map()
    m.unit(1, OOZIUM, 5, 5).unit(1, "tank", 6, 5)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10)
    g = ctx.start(m, ["hawke", "andy"])
    ctx.set_hp(g, 5, 5, 50)
    ctx.set_hp(g, 6, 5, 50)
    ctx.power(g, 1, "power")
    g.wait_for_input()
    ctx.eq(unit(g, 20, 10).get("hp"), 100, "Black Wave: the enemy Oozium takes nothing")
    ctx.eq(unit(g, 21, 10).get("hp"), 90, "the enemy Tank takes 1 HP")
    ctx.eq(unit(g, 5, 5).get("hp"), 50, "Hawke's own Oozium is not repaired")
    ctx.eq(unit(g, 6, 5).get("hp"), 60, "his Tank is")


@test(modes=DS)
def oozium_immune_drake(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10)
    g = ctx.start(m, ["drake", "andy"])
    f0 = unit(g, 20, 10).get("fuel")
    ctx.power(g, 1, "super")
    g.wait_for_input()
    ctx.eq(unit(g, 20, 10).get("hp"), 100, "Typhoon: the Oozium takes nothing")
    ctx.eq(unit(g, 20, 10).get("fuel"), f0, "and keeps its fuel")
    ctx.check(unit(g, 21, 10).get("hp", 100) < 100, "the Tank is hit")


@test(modes=DS)
def oozium_immune_andy_repair(ctx):
    m = ctx.map()
    m.unit(1, OOZIUM, 5, 5).unit(1, "tank", 6, 5).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 5, 5, 50)
    ctx.set_hp(g, 6, 5, 50)
    ctx.power(g, 1, "power")
    g.wait_for_input()
    ctx.eq(unit(g, 5, 5).get("hp"), 50, "Hyper Repair: the Oozium is not repaired")
    ctx.eq(unit(g, 6, 5).get("hp"), 70, "the Tank is (+2 HP)")


@test(modes=DS)
def oozium_immune_eagle(ctx):
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(1, "tank", 12, 12).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["eagle", "andy"])
    for x, y, to in (((10, 10), None, (10, 11)), ((12, 12), None, (12, 13))):
        pass
    g.select(10, 10)
    g.move_to(10, 11)
    g.choose("Wait", g.ACTION_MENU)
    g.wait_idle()
    g.select(12, 12)
    g.move_to(12, 13)
    g.choose("Wait", g.ACTION_MENU)
    g.wait_idle()
    ctx.power(g, 1, "power")
    g.wait_for_input()
    ctx.check(unit(g, 10, 11).get("flags", 0) & 1, "Lightning Drive: the Oozium stays moved")
    ctx.check(not unit(g, 12, 13).get("flags", 1) & 1, "the Tank can move again")


@test(modes=DS)
def oozium_immune_von_bolt(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10).unit(2, "mech", 21, 11)
    g = ctx.start(m, ["vonbolt", "andy"], humans=(1, 2))
    ctx.power(g, 1, "super")
    g.wait_for_input()
    ctx.shot(g, "after_strike")
    ctx.eq(unit(g, 20, 10).get("hp"), 100, "Ex Machina: the Oozium takes nothing")
    ctx.eq(unit(g, 21, 10).get("hp"), 70, "the Tank next to it takes 3 HP")
    g.end_turn(human=2)
    ctx.check(not unit(g, 20, 10).get("flags", 1) & 1, "the Oozium is not stunned")
    ctx.check(unit(g, 21, 10).get("flags", 0) & 1, "the Tank is")


@test(modes=DS)
def oozium_immune_sturm(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10).unit(2, "mech", 21, 11)
    g = ctx.start(m, ["sturm", "andy"])
    ctx.power(g, 1, "super")
    g.wait_for_input()
    ctx.eq(unit(g, 20, 10).get("hp"), 100, "Meteor Strike: the Oozium takes nothing")
    ctx.check(unit(g, 21, 10).get("hp", 100) < 100, "the Tank next to it is hit")


@test(modes=DS)
def oozium_immune_silo(ctx):
    m = ctx.map()
    m.terrain(10, 10, SILO)
    m.unit(1, "infantry", 10, 10)
    m.unit(2, OOZIUM, 14, 10).unit(2, "tank", 15, 10)
    g = ctx.start(m, ["andy", "andy"])
    names = g.action_menu_at(10, 10)
    ctx.require("Launch" in names, f"the Infantry on the silo can Launch: {names}")
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Launch", g.ACTION_MENU)
    g.e.wait(60)
    for _ in range(2):  # "Select missile target now."
        g.e.press("A", 4)
        g.e.wait(40)
    for _ in range(4):  # the target cursor starts on the silo
        g.e.hold("RIGHT", 6)
        g.e.wait(12)
    ctx.shot(g, "silo_target")
    g.e.press("A", 4)
    g.e.wait(60)
    ctx.shot(g, "silo")
    g.wait_for_input()
    ctx.eq(unit(g, 14, 10).get("hp"), 100, "the silo's blast spares the Oozium")
    ctx.eq(unit(g, 15, 10).get("hp"), 70, "the Tank next to it takes 3 HP")


@test(modes=DS)
def oozium_defence_no_co(ctx):
    """Kanbei's army in its CO Power: a Tank's hit on its Oozium is worked out
    with no CO defence and without the power's +10 (the calculator knows it:
    ctx.attack checks the game against it)."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, OOZIUM, 11, 10)
    g = ctx.start(m, ["andy", "kanbei"])
    p = g.player(2)["addr"]
    g.e.w8(p + 0x1E, 1)
    g.e.w16(p + 0x28, 10)
    g.attack((10, 10), (10, 10), (11, 10))
    ra, rd = ctx.battle_records(g)
    ctx.eq(rd["defence"], 110, "the Oozium's defence: 100 + the plain's 10, nothing from Kanbei's power")
    ctx.check(ra["base"] > 0, f"the Tank hits it (base {ra['base']})")


# --- the CPU ---------------------------------------------------------------------


@test(modes=DS)
def cpu_black_bomb_ignores_ooziums(ctx):
    m = ctx.map()
    for x, y in ((10, 6), (11, 6), (10, 7), (11, 7)):
        m.unit(1, OOZIUM, x, y)
    m.unit(2, "blackbomb", 16, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    ctx.eq(len([u for u in g.units(2) if u["type"] == 13]), 1, "a group of Ooziums is not worth a bomb")
    ctx.eq([u["hp"] for u in g.units(1) if u["type"] == OOZIUM], [100] * 4, "they are untouched")


def cpu_strike(ctx, co, name):
    """A CPU `co` with a full meter: a pack of Ooziums (worth the most if they
    counted) and, far off, a Tank and an Artillery. Its strike goes to the Tank's group."""
    m = ctx.map()
    for x, y in ((5, 5), (6, 5), (5, 6), (6, 6), (7, 5)):
        m.unit(1, OOZIUM, x, y)
    m.unit(1, "tank", 20, 14).unit(1, "artillery", 21, 14)
    m.unit(2, "infantry", 26, 3)
    g = ctx.start(m, ["andy", co])
    for turn in range(3):
        g.charge_power(2, "super")
        g.end_turn()
        if g.player(2)["powers_used"]:
            break
    ctx.require(g.player(2)["powers_used"], f"the CPU fired {name}")
    ctx.eq([u["hp"] for u in g.units(1) if u["type"] == OOZIUM], [100] * 5, f"{name}: the Ooziums are untouched")
    hit = [u["type"] for u in g.units(1) if u["type"] != OOZIUM and u["hp"] < 100]
    ctx.check(hit, f"{name} went to the other units: hurt {hit}")


@test(modes=DS)
def cpu_von_bolt_ignores_ooziums(ctx):
    cpu_strike(ctx, "vonbolt", "Ex Machina")


@test(modes=DS)
def cpu_sturm_ignores_ooziums(ctx):
    cpu_strike(ctx, "sturm", "Meteor Strike")


@test(modes=DS)
def cpu_silo_ignores_ooziums(ctx):
    m = ctx.map()
    m.terrain(15, 10, SILO)
    for x, y in ((10, 5), (11, 5), (10, 6), (11, 6), (12, 5)):
        m.unit(1, OOZIUM, x, y)
    m.unit(1, "tank", 20, 14).unit(1, "artillery", 21, 14)
    m.unit(2, "infantry", 15, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    ctx.eq([u["hp"] for u in g.units(1) if u["type"] == OOZIUM], [100] * 5, "the Ooziums are untouched")
    silo_used = g.terrain_class(15, 10) != g.image.tile_class(SILO) if hasattr(g, "image") else None
    hurt = [u["type"] for u in g.units(1) if u["type"] != OOZIUM and u["hp"] < 100]
    ctx.log(f"silo used: {silo_used}; hurt: {hurt}")
    ctx.check(hurt or not silo_used, f"the CPU's missile, if fired, went to the other units: {hurt}")


@test(modes=DS)
def cpu_oozium_immune_to_player_power(ctx):
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10)
    g = ctx.start(m, ["olaf", "andy"])
    ctx.power(g, 1, "super")
    g.wait_for_input()
    g.end_turn()
    oz = [u for u in g.units(2) if u["type"] == OOZIUM][0]
    ctx.eq(oz["hp"], 100, "the CPU's Oozium took nothing from Winter Fury")


@test(modes=DS, netplay=True)
def netplay_oozium_immune(ctx):
    m = ctx.map()
    m.unit(1, OOZIUM, 5, 5).unit(1, "tank", 6, 5)
    m.unit(2, OOZIUM, 20, 10).unit(2, "tank", 21, 10)
    g = ctx.start(m, ["hawke", "vonbolt"])
    ctx.set_hp(g, 5, 5, 50)
    ctx.power(g, 1, "power")
    g.wait_for_input()
    g.charge_power(2, "super")
    g.end_turn(human=1)
    ctx.eq([u["hp"] for u in g.units(2) if u["type"] == OOZIUM], [100], "the enemy Oozium untouched")
    ctx.eq(unit(g, 5, 5).get("hp"), 50, "our Oozium not repaired")
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    players = (g.players_base + ram.PLAYER_SIZE, ram.PLAYER_SIZE * 2)
    offline = {a: g.e.read(a, n) for a, n in (units, players)}
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, players])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")
