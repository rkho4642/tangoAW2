"""Dual Strike's battle animations for the new units (pack only, crate::ds_battle).

A battle with animations on (Rules "Visuals A") where one side is a new unit:
mid-scene the donor's figure sprites on that side are gone and the unit's own
are drawn from the side's figure tiles; the scene ends, control comes back and
the units end exactly as the same battle with animations off; the run replays
identically on two rollback peers.
"""

import struct

from aw2test import ram
from aw2test.harness import test

STATE = 0x0203F800  # tangoAW2's per-side state (0x40 each): +0 unit + 1
OAM = 0x07000000
SIDE_TILES = 256


def fire(g, src, target):
    g.select(*src)
    g.move_to(*src)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(*target)


def figure_sprites(g, side):
    """Visible OAM entries using the side's figure tiles: (count, palettes)."""
    raw = g.e.read(OAM, 0x400)
    n, pals = 0, set()
    for k in range(128):
        a0, a1, a2 = struct.unpack_from("<3H", raw, 8 * k)
        if (a0 & 0x300) == 0x200 or (not a0 & 0x100 and (a0 & 0xFF) >= 160):
            continue
        if (a2 & 0x3FF) // SIDE_TILES == side:
            n += 1
            pals.add(a2 >> 12)
    return n, pals


def battle(ctx, att, dfd, visuals, dist=1, terrain=("plain", "plain"), sample=None):
    m = ctx.map()
    m.terrain(10, 10, terrain[0]).terrain(10 + dist, 10, terrain[1])
    m.unit(1, att, 10, 10).unit(2, dfd, 10 + dist, 10)
    g = ctx.start(m, ["andy", "olaf"], visuals=visuals)
    fire(g, (10, 10), (10 + dist, 10))
    if sample:
        sample(g)
    g.wait_for_input()
    return g, [(u["type"], u["hp"], u["x"]) for u in g.units()]


MATCHES = [
    ("megatank", "tank", 1, ("plain", "plain"), 0),
    ("tank", 27, 1, ("plain", "plain"), 1),
    ("piperunner", "bcopter", 2, ("pipe", "plain"), 0),
    (26, "fighter", 3, ("sea", "plain"), 0),
]


@test(modes=("ds",))
def battle_scenes_new_units(ctx):
    for att, dfd, dist, terrain, side in MATCHES:
        label = f"{att} vs {dfd}"
        seen = {}

        def sample(g):
            g.e.wait(150)
            seen["unit"] = g.e.u8(STATE + 0x40 * side)
            seen["other"] = g.e.u8(STATE + 0x40 * (1 - side))
            seen["sprites"] = figure_sprites(g, side)
            ctx.shot(g, f"{att}_{dfd}")

        g, anim = battle(ctx, att, dfd, "a", dist, terrain, sample)
        g.e.close()
        g2, off = battle(ctx, att, dfd, "off", dist, terrain)
        g2.e.close()
        ctx.check(seen["unit"] != 0, f"{label}: side {side} draws Dual Strike's unit")
        ctx.eq(seen["other"], 0, f"{label}: the other side keeps AW2's figures")
        n, pals = seen["sprites"]
        ctx.check(n > 5, f"{label}: the unit's sprites are in the side's figure tiles ({n})")
        ctx.check(pals <= {4 * side, 4 * side + 1, 4 * side + 2, 4 * side + 3}, f"{label}: in the side's palettes {pals}")
        ctx.eq(anim, off, f"{label}: the units end as with animations off")


@test(modes=("ds",))
def netplay_battle_scene(ctx):
    """A Megatank's animated battle replayed on two rollback peers."""
    m = ctx.map()
    m.unit(1, "megatank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "hawke"], visuals="a")
    fire(g, (10, 10), (11, 10))
    g.wait_for_input()
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    state = (STATE, 0x80)
    offline = {a: g.e.read(a, n) for a, n in (units, state)}
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, state])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")
