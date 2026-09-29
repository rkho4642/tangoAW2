"""The CPU under Sandstorm and on Wasteland maps (Dual Strike pack): its indirect
units lose a square of range in a sandstorm as the player's do, and it plays a
Wasteland map with the same rules while the map keeps Wasteland's colours."""

from aw2test import damage
from aw2test.harness import test

from tests.test_biome import wasteland_map, PAL_BUFFER, AW2_CLEAR, WASTELAND_CLEAR, BIOME


def cpu_indirect(ctx, weather, unit, dist):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, unit, 10 + dist, 10)
    g = ctx.start(m, ["andy", "andy"], weather=weather)
    g.end_turn()
    return g.unit_at(10, 10)["hp"]


@test(modes=("ds",))
def cpu_artillery_sandstorm(ctx):
    clear = cpu_indirect(ctx, "clear", "artillery", 3)
    sand = cpu_indirect(ctx, "sandstorm", "artillery", 3)
    ctx.check(clear < 100, f"clear: the CPU's artillery fires at 3 squares (tank {clear})")
    ctx.eq(sand, 100, "sandstorm: at 3 squares it can't (range 2-2)")
    near = cpu_indirect(ctx, "sandstorm", "artillery", 2)
    ctx.check(near < 100, f"sandstorm: it still fires at 2 squares (tank {near})")


@test(modes=("ds",))
def cpu_rockets_sandstorm(ctx):
    clear = cpu_indirect(ctx, "clear", "rockets", 5)
    sand = cpu_indirect(ctx, "sandstorm", "rockets", 5)
    ctx.check(clear < 100, f"clear: the CPU's rockets fire at 5 squares (tank {clear})")
    ctx.eq(sand, 100, "sandstorm: at 5 squares they can't (range 3-4)")


def cpu_turn_on_wasteland(ctx, weather):
    m = wasteland_map(ctx)
    g = ctx.start(m, ["andy", "andy"], weather=weather)
    before = {u["id"]: dict(u) for u in g.units()}
    sides = {uid: ctx.side(g, u) for uid, u in before.items()}
    seen = {}

    def during(gg):
        gg.e.wait(60)
        seen["palette"] = gg.e.read(PAL_BUFFER, 128)
        ctx.shot(gg, f"cpu_turn_{weather}")
    g.end_turn(observe=during)
    ctx.eq(g.e.u8(BIOME) >> 4 & 7, 1, "still Wasteland")
    ctx.check(seen["palette"] != g.e.read(AW2_CLEAR, 128), "the CPU's turn is drawn in Wasteland's colours")
    if weather == "clear":
        ctx.eq(seen["palette"], g.e.read(WASTELAND_CLEAR, 128), "Wasteland's clear colours during the CPU's turn")
    after = {u["id"]: u for u in g.units()}
    mine = [uid for uid, u in before.items() if u["army"] == 1 and u["type"] == 5][0]
    cpu = [uid for uid, u in before.items() if u["army"] == 2 and u["type"] == 5][0]
    if ctx.check(after[mine]["hp"] < 100, "the CPU attacked on the Wasteland map"):
        a = sides[cpu]
        a.terrain = g.terrain_class(after[cpu]["x"], after[cpu]["y"]) & 0x1F
        dist = abs(after[cpu]["x"] - after[mine]["x"]) + abs(after[cpu]["y"] - after[mine]["y"])
        first, _ = damage.battle(ctx.rules, a, sides[mine], dist, dfd_hp_after=after[mine]["hp"])
        ctx.check(100 - after[mine]["hp"] in first.losses, f"same rules: took {100 - after[mine]['hp']}, allowed {sorted(first.losses)}")


@test(modes=("ds",))
def cpu_plays_wasteland(ctx):
    cpu_turn_on_wasteland(ctx, "clear")


@test(modes=("ds",))
def cpu_plays_wasteland_in_sandstorm(ctx):
    cpu_turn_on_wasteland(ctx, "sandstorm")
