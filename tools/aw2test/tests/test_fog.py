"""Fog of War: woods hide units (but next to one), Sonja sees further and, in
her powers, into woods; with the Dual Strike pack rain brings fog, as in
Dual Strike. The map's visibility plane (+0x1E42 per cell, 1 = seen) is
read for army 1."""

from aw2test.harness import test

MAP = 0x0201E450


def seen(g, x, y):
    row = g.e.u16(MAP + 0x417A + 2 * y)
    return g.e.u8(MAP + 0x1E42 + row + x)


def fog_map(ctx):
    m = ctx.map()
    for x, y in ((12, 6), (11, 6), (10, 8)):
        m.terrain(x, y, "wood")
    m.unit(1, "tank", 10, 6).unit(2, "infantry", 12, 6).unit(2, "infantry", 11, 6).unit(2, "infantry", 13, 7)
    return m


@test()
def fog_woods_hide_units(ctx):
    g = ctx.start(fog_map(ctx), ["andy", "andy"], fog=True)
    ctx.eq(seen(g, 11, 6), 1, "woods next to the tank: seen")
    ctx.eq(seen(g, 12, 6), 0, "woods 2 away: hidden")
    ctx.eq(seen(g, 10, 8), 0, "empty woods 2 away: hidden")
    ctx.eq(seen(g, 10, 9), 1, "plains 3 away: seen (tank vision 3)")
    ctx.eq(seen(g, 13, 7), 0, "plains 4 away: not seen")


@test()
def fog_sonja_vision(ctx):
    g = ctx.start(fog_map(ctx), ["sonja", "andy"], fog=True)
    ctx.eq(seen(g, 13, 7), 1, "Sonja: plains 4 away seen (+1 vision)")
    ctx.eq(seen(g, 12, 6), 0, "Sonja day to day: woods 2 away still hidden")


@test()
def fog_sonja_cop_sees_into_woods(ctx):
    g = ctx.start(fog_map(ctx), ["sonja", "andy"], fog=True)
    ctx.power(g, 1, "power")
    g.e.wait(30)
    ctx.eq(seen(g, 12, 6), 1, "Enhanced Vision: woods 2 away seen")
    ctx.eq(seen(g, 10, 8), 1, "Enhanced Vision: empty woods seen")
    ctx.shot(g, "cop")


@test()
def fog_sonja_scop_sees_into_woods(ctx):
    g = ctx.start(fog_map(ctx), ["sonja", "andy"], fog=True)
    ctx.power(g, 1, "super")
    g.e.wait(30)
    ctx.eq(seen(g, 12, 6), 1, "Counter Break: woods 2 away seen")


@test()
def rain_brings_fog(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "infantry", 20, 6)
    g = ctx.start(m, ["andy", "andy"], weather="rain")
    fog = g.e.u8(0x03003FCD)
    ctx.eq(fog, {"aw2": 0, "ds": 1}[ctx.mode], "fog with rain (Dual Strike: yes, AW2: no)")
    ctx.eq(seen(g, 20, 6), {"aw2": 1, "ds": 0}[ctx.mode], "a far unit in the rain")


@test(netplay=True)
def netplay_fog_and_rain(ctx):
    """Fog (and, with the pack, rain's fog) over netplay: an attack and CPU
    turns, identical on both peers and the replay."""
    g = ctx.start(fog_map(ctx), ["sonja", "andy"], fog=True, weather="rain")
    g.end_turn(human=1)
    g.end_turn(human=1)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")
