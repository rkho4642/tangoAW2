"""Weather: the Rules screen's choices, and Dual Strike's Sandstorm (pack only).

Sandstorm (Dual Strike): every unit that fires from a distance loses 1 of its
maximum range (never below its minimum), except under Max and Grit (and
Jugger). Direct units are unchanged. Fixed Sandstorm is gPlaySt weather 0,
mode 3, next 0, default 0 (0x03003FEC..EF = 00 03 00 00); a random one-day
sandstorm sets 0x03004493 to 2.

Range is checked through the game itself: an attacker with one enemy at a
chosen distance either gets Fire in its action menu or not.
"""

from aw2test import rom as romlib
from aw2test.harness import test

WEATHER_BYTES = 0x03003FEC      # weather, mode, next, default
SANDSTORM_BYTES = bytes([0, 3, 0, 0])
RAIN_CHANCE = 0x03004491
SNOW_CHANCE = 0x03004492
SANDSTORM_DAY = 0x03004493
ONE_DAY = 2
EXEMPT = ("max", "grit")

# attacker, its cell, the enemy it aims at, and the direction of the enemy
PAIRS = [
    ("artillery", (2, 3), "tank", +1),
    ("rockets", (2, 10), "tank", +1),
    ("missiles", (27, 3), "bcopter", -1),
    ("battleship", (27, 14), "tank", -1),
    ("tank", (14, 17), "tank", +1),
]


def clear_range(ctx, unit, co):
    """(min, max) range in clear weather: the unit table plus the CO's range bonus."""
    from aw2test import damage
    t = romlib.unit_id(unit)
    u = ctx.image.unit(t)
    side = damage.Side(type=t, hp=100, terrain=1, co=romlib.co_id(co))
    return u["min_range"], u["max_range"] + damage.range_bonus(ctx.rules, side)


def range_map(ctx, co, delta):
    """Each attacker with one enemy at its clear-weather max range + delta."""
    m = ctx.map(hq=((1, 0, 19), (2, 29, 0)))
    for y in range(12, 17):
        for x in range(23, 30):
            m.terrain(x, y, "sea")
    cases = []
    for unit, (x, y), target, sign in PAIRS:
        lo, hi = clear_range(ctx, unit, co)
        d = hi + delta
        if d < lo:
            continue
        m.unit(1, unit, x, y).unit(2, target, x + sign * d, y)
        cases.append((unit, (x, y), d, lo, hi))
    return m, cases


def check_ranges(ctx, g, cases, co, sandstorm):
    """Fire is offered iff an enemy the unit can hit is within its range
    (distances are read from RAM, so CPU moves are allowed for)."""
    for unit, pos, d, lo, hi in cases:
        eff = hi
        if sandstorm and hi > 1 and co not in EXEMPT:
            eff = max(lo, hi - 1)
        me = g.unit_at(*pos)
        if me is None:
            ctx.log(f"{unit} at {pos} is gone; not checked")
            continue
        dists = sorted(abs(u["x"] - pos[0]) + abs(u["y"] - pos[1]) for u in g.units()
                       if u["army"] != me["army"] and max(ctx.rules.raw(me["type"], u["type"], w) for w in (0, 1)) > 0)
        in_range = any(lo <= x <= eff for x in dists)
        names = g.action_menu_at(*pos)
        ctx.eq("Fire" in names, in_range,
               f"{unit} ({co}, range {lo}-{hi} clear, {lo}-{eff} here), enemies at {dists[:3]}: Fire offered; menu {names}")
        greyed = lo > 1 and not in_range
        ctx.eq("Fire (greyed)" in names, greyed,
               f"{unit} ({co}): greyed-out Fire (indirect, no target)")


def range_test(co, weather, delta, modes):
    name = f"range_{weather}_{co}" + {0: "", -1: "_minus1", 1: "_plus1"}[delta]

    @test(name=name, modes=modes)
    def fn(ctx):
        m, cases = range_map(ctx, co, delta)
        g = ctx.start(m, [co, "andy"], weather=weather)
        if weather == "sandstorm":
            ctx.eq(g.e.read(WEATHER_BYTES, 4), SANDSTORM_BYTES, "fixed Sandstorm in gPlaySt (weather, mode, next, default)")
        check_ranges(ctx, g, cases, co, weather == "sandstorm")
    return fn


range_test("andy", "clear", 0, ("aw2", "ds"))
range_test("andy", "sandstorm", 0, ("ds",))
range_test("andy", "sandstorm", -1, ("ds",))
range_test("max", "sandstorm", 0, ("ds",))
range_test("grit", "sandstorm", 0, ("ds",))
range_test("andy", "clear", +1, ("aw2", "ds"))
range_test("max", "clear", +1, ("aw2", "ds"))


@test()
def rules_weather_choices(ctx):
    """AW2: Random, Clear, Rain, Snow. With the pack: Sandstorm as a fifth."""
    from aw2test.emu import Emu
    from aw2test.game import Game
    import os
    from aw2test import paths
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 12, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    g.set_teams(["andy", "andy"], {1})
    g.teams_to_rules()
    items = g.rules_items()
    ctx.eq(e.u8(items[1] + 0x4B), 5 if ctx.ds else 4, "Weather choices on the Rules screen")


@test(modes=("ds",))
def olaf_snow_during_sandstorm(ctx):
    """Olaf's Blizzard makes it snow for a day; then the Sandstorm is back."""
    m, cases = range_map(ctx, "olaf", 0)
    g = ctx.start(m, ["olaf", "andy"], weather="sandstorm")
    art = [c for c in cases if c[0] == "artillery"]
    ctx.eq(g.e.read(WEATHER_BYTES, 4), SANDSTORM_BYTES, "fixed Sandstorm at the start")
    ctx.power(g, 1, "power")
    ctx.eq(g.e.u8(WEATHER_BYTES), 1, "snow after Blizzard")
    check_ranges(ctx, g, art, "olaf", sandstorm=False)
    seen = {}
    g.end_turn(human=1, observe=lambda gg: seen.update(cpu=gg.e.read(WEATHER_BYTES, 4)))
    ctx.log(f"weather during the CPU's turn: {seen['cpu'].hex()}")
    ctx.eq(g.e.read(WEATHER_BYTES, 4), SANDSTORM_BYTES, "Sandstorm again on Olaf's next day")
    check_ranges(ctx, g, art, "olaf", sandstorm=True)


@test(modes=("aw2", "ds"))
def random_sandstorm_day(ctx):
    """Random weather: with the snow chance forced to 100 a turn end brings a
    one-day sandstorm (pack) or snow (AW2), never both."""
    m, cases = range_map(ctx, "andy", 0)
    g = ctx.start(m, ["andy", "andy"], weather="random")
    art = [c for c in cases if c[0] == "artillery"]
    ctx.eq(g.e.u8(SANDSTORM_DAY), 0, "no sandstorm at the start")
    g.e.w8(SNOW_CHANCE, 100)
    seen = {}

    def cpu_turn(gg):
        gg.e.wait(30)
        seen["cpu"] = (gg.e.u8(SANDSTORM_DAY), gg.e.u8(WEATHER_BYTES), gg.e.u8(WEATHER_BYTES + 2))

    g.end_turn(human=1, observe=cpu_turn)
    day2 = (g.e.u8(SANDSTORM_DAY), g.e.u8(WEATHER_BYTES))
    ctx.log(f"(sandstorm byte, weather, next weather): CPU's turn {seen['cpu']}, our next turn {day2}")
    if ctx.ds:
        ctx.eq(seen["cpu"], (ONE_DAY, 0, 0), "sandstorm from the turn end, and no snow or rain now or next")
        ctx.eq(day2, (ONE_DAY, 0), "still a sandstorm on our next turn (the same day)")
        check_ranges(ctx, g, art, "andy", sandstorm=True)
    else:
        ctx.eq(seen["cpu"][0], 0, "AW2: no sandstorm byte")
        ctx.eq(seen["cpu"][2], 1, "AW2: snow comes next instead")
        ctx.eq(day2, (0, 1), "AW2: snow on our next turn")
    g.e.w8(SNOW_CHANCE, 0)
    g.e.w8(RAIN_CHANCE, 0)
    g.end_turn(human=1, observe=cpu_turn)
    day3 = (g.e.u8(SANDSTORM_DAY), g.e.u8(WEATHER_BYTES))
    ctx.log(f"next day: CPU's turn {seen['cpu']}, our turn {day3}")
    if ctx.ds:
        ctx.eq(seen["cpu"][0], 0, "the sandstorm lasted one day")
        ctx.eq(day3[0], 0, "no sandstorm the day after")
        check_ranges(ctx, g, art, "andy", sandstorm=False)


@test(modes=("ds",))
def random_sandstorm_resets_at_map_start(ctx):
    """A leftover one-day sandstorm byte is cleared when a map starts."""
    from aw2test.emu import Emu
    from aw2test.game import Game
    import os
    from aw2test import paths
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 12, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    g.set_teams(["andy", "andy"], {1})
    g.teams_to_rules()
    g.set_rules(weather="random")
    e.w8(SANDSTORM_DAY, ONE_DAY)
    g.start_battle()
    g.wait_for_input()
    ctx.eq(e.u8(SANDSTORM_DAY), 0, "sandstorm byte after the map start")
