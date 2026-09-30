"""Dual Strike's battle animations for the new units (pack only, crate::ds_battle).

A battle with animations on (Rules "Visuals A", which shows every battle, the
CPU's too) where one side is a new unit: mid-scene the donor's figure sprites
on that side are gone and the unit's own are drawn from the side's figure
tiles, its Dual Strike effects (muzzle flashes, missiles and shells, hits) are
drawn in the effects' palettes (12, 13), and each army colour has its own
palette; the scene ends, control comes back and the units end exactly as the
same battle with animations off; an Oozium's attack has no scene (as in Dual
Strike); the run replays identically on two rollback peers.

Every new unit attacks and is attacked in all five army colours (Black Hole in
a five-army map), by the player and by the CPU.
"""

import struct

from aw2test import ram
from aw2test.harness import test

STATE = 0x0203F800  # tangoAW2's per-side state (0x40 each): +0 unit + 1
FX = 0x0203F880  # effects in flight
OAM = 0x07000000
PAL = 0x05000200  # OBJ palettes
MAIN_CALLBACK = 0x03000000  # 0 while a battle scene runs
SIDE_TILES = 256
FX_PALS = {12, 13}
COLOURS = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}

CARRIER, OOZIUM = 26, 27
# (new unit, AW2 target, distance, terrain of the new unit, of the target)
ATTACKS = {
    "megatank": ("megatank", "tank", 1, "plain", "plain"),
    "piperunner": ("piperunner", "tank", 2, "pipe", "plain"),
    "stealth": ("stealth", "tank", 1, "plain", "plain"),
    "carrier": (CARRIER, "fighter", 3, "sea", "plain"),
}
# (new unit, AW2 attacker, distance, terrain of the new unit, of the attacker)
DEFENCES = {
    "megatank": ("megatank", "tank", 1, "plain", "plain"),
    "piperunner": ("piperunner", "artillery", 2, "pipe", "plain"),
    "stealth": ("stealth", "fighter", 1, "plain", "plain"),
    "blackbomb": ("blackbomb", "antiair", 1, "plain", "plain"),
    "blackboat": ("blackboat", "battleship", 2, "sea", "sea"),
    "carrier": (CARRIER, "bomber", 1, "sea", "plain"),
    "oozium": (OOZIUM, "tank", 1, "plain", "plain"),
}
# The CPU's attacks: (new unit, the player's unit, distance, terrains) as ATTACKS,
# at a distance the CPU closes.
CPU_ATTACKS = {
    "megatank": ("megatank", "tank", 2, "plain", "plain"),
    "piperunner": ("piperunner", "tank", 3, "pipe", "plain"),
    "stealth": ("stealth", "tank", 2, "plain", "plain"),
    "carrier": (CARRIER, "fighter", 4, "sea", "plain"),
}


def fire(g, src, target):
    g.select(*src)
    g.move_to(*src)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(*target)


def figure_sprites(g, side):
    """Visible OAM entries using the side's tiles (its unit's, and its effects'):
    (count, palettes)."""
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


def fx_sprites(g):
    raw = g.e.read(OAM, 0x400)
    return sum(1 for k in range(128) if (struct.unpack_from("<3H", raw, 8 * k)[2] >> 12) in FX_PALS
               and not (struct.unpack_from("<H", raw, 8 * k)[0] & 0x300) == 0x200)


def watch_scene(ctx, g, name, max_frames=1500, step=6):
    """Sample a battle scene while it runs: which sides drew a Dual Strike
    unit, their most figure sprites and palettes, effect sprites seen, the
    sides' first OBJ palettes, and a few screenshots."""
    e = g.e
    seen = {"frames": 0, "units": [0, 0], "sprites": [0, 0], "pals": [set(), set()], "fx": 0, "palette": [None, None],
            "fx_state": 0}
    waited = 0
    shots = 0
    while waited < max_frames:
        e.wait(step)
        waited += step
        if e.u32(MAIN_CALLBACK) != 0:
            if seen["frames"]:
                break
            continue
        seen["frames"] += 1
        st = e.read(STATE, 0x80)
        for side in (0, 1):
            if st[0x40 * side]:
                seen["units"][side] = st[0x40 * side]
                n, pals = figure_sprites(g, side)
                seen["sprites"][side] = max(seen["sprites"][side], n)
                seen["pals"][side] |= pals
                seen["palette"][side] = e.read(PAL + 32 * 4 * side, 32)
        seen["fx"] = max(seen["fx"], fx_sprites(g))
        fx = e.read(FX, 0x180)
        seen["fx_state"] = max(seen["fx_state"], sum(1 for k in range(24) if fx[16 * k]))
        if seen["frames"] in (20, 40) and shots < 2:
            ctx.shot(g, f"{name}_{seen['frames']}")
            shots += 1
    return seen


def check_unit(ctx, seen, side, label, effects):
    ctx.check(seen["frames"] > 0, f"{label}: the battle scene ran")
    ctx.check(seen["units"][side] != 0, f"{label}: side {side} drew Dual Strike's unit")
    ctx.check(seen["sprites"][side] > 3, f"{label}: its sprites are in the side's figure tiles ({seen['sprites'][side]})")
    ctx.check(seen["pals"][side] <= {4 * side, 4 * side + 1, 4 * side + 2, 4 * side + 3} | FX_PALS,
              f"{label}: in the side's palettes {sorted(seen['pals'][side])}")
    if effects:
        ctx.check(seen["fx_state"] > 0 and seen["fx"] > 0,
                  f"{label}: Dual Strike's effects flew ({seen['fx_state']}) and were drawn ({seen['fx']} sprites)")


def two_army_map(ctx, colours):
    m = ctx.map()
    m.colours[1], m.colours[2] = colours
    return m, ["andy", "olaf"], 2


def five_army_map(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 15, 0x1B4)  # Black Hole's HQ
    m.colours = [5, 1, 2, 3, 4]  # the five-army mark
    return m, None, 5


def other_colour(c):
    return 1 if c != 1 else 2


def player_attack(ctx, unit, colour):
    """The player's new unit (army 1 in `colour`, 1..4) attacks an AW2 unit."""
    new, target, dist, tn, tt = ATTACKS[unit]
    m, cos, enemy = two_army_map(ctx, (colour, other_colour(colour)))
    m.terrain(10, 10, tn).terrain(10 + dist, 10, tt)
    m.unit(1, new, 10, 10).unit(enemy, target, 10 + dist, 10)
    g = ctx.start(m, cos, visuals="a")
    fire(g, (10, 10), (10 + dist, 10))
    seen = watch_scene(ctx, g, f"{unit}_attacks_{colour}")
    g.wait_for_input()
    g.e.close()
    return seen


def attacked(ctx, unit, colour):
    """The player's AW2 unit attacks the new unit (of army 2 in `colour`, or of
    Black Hole, army 5, when `colour` is 5)."""
    new, attacker, dist, tn, ta = DEFENCES[unit]
    if colour == 5:
        m, cos, enemy = five_army_map(ctx)
    else:
        m, cos, enemy = two_army_map(ctx, (other_colour(colour), colour))
    m.terrain(10, 10, ta).terrain(10 + dist, 10, tn)
    m.unit(1, attacker, 10, 10).unit(enemy, new, 10 + dist, 10)
    g = ctx.start(m, cos, visuals="a")
    fire(g, (10, 10), (10 + dist, 10))
    seen = watch_scene(ctx, g, f"{unit}_attacked_{colour}")
    g.wait_for_input()
    g.e.close()
    return seen


def cpu_attack(ctx, unit, colour):
    """The CPU's new unit (army 2 in `colour`, or Black Hole) attacks the player's
    unit during its turn."""
    new, target, dist, tn, tt = CPU_ATTACKS[unit]
    if colour == 5:
        m, cos, enemy = five_army_map(ctx)
    else:
        m, cos, enemy = two_army_map(ctx, (other_colour(colour), colour))
    m.terrain(10, 10, tt).terrain(10 + dist, 10, tn)
    m.unit(1, target, 10, 10).unit(enemy, new, 10 + dist, 10)
    g = ctx.start(m, cos, visuals="a")
    out = {}
    g.end_turn(human=1, observe=lambda gg: out.update(seen=watch_scene(ctx, gg, f"cpu_{unit}_{colour}", 3000)))
    g.e.close()
    return out["seen"]


def distinct_palettes(ctx, label, palettes):
    got = [p for p in palettes if p]
    ctx.eq(len(set(got)), len(got), f"{label}: each army colour has its own palette")


def make_attack_test(unit):
    @test(name=f"battle_scene_{unit}_attacks_every_colour", modes=("ds",))
    def t(ctx):
        palettes = []
        for colour in (1, 2, 3, 4):
            seen = player_attack(ctx, unit, colour)
            check_unit(ctx, seen, 0, f"{unit} ({COLOURS[colour]}) attacks", True)
            palettes.append(seen["palette"][0])
        seen = cpu_attack(ctx, unit, 5)
        side = 0 if seen["units"][0] else 1
        check_unit(ctx, seen, side, f"{unit} ({COLOURS[5]}, CPU) attacks", True)
        palettes.append(seen["palette"][side])
        distinct_palettes(ctx, unit, palettes)


def make_defence_test(unit):
    @test(name=f"battle_scene_{unit}_attacked_every_colour", modes=("ds",))
    def t(ctx):
        palettes = []
        for colour in (1, 2, 3, 4, 5):
            seen = attacked(ctx, unit, colour)
            check_unit(ctx, seen, 1, f"{unit} ({COLOURS[colour]}) attacked", False)
            palettes.append(seen["palette"][1])
        distinct_palettes(ctx, unit, palettes)


def make_cpu_test(unit):
    @test(name=f"battle_scene_cpu_{unit}_attacks", modes=("ds",))
    def t(ctx):
        for colour in (2, 3):
            seen = cpu_attack(ctx, unit, colour)
            side = 0 if seen["units"][0] else 1
            check_unit(ctx, seen, side, f"CPU {unit} ({COLOURS[colour]}) attacks", True)


for _u in ATTACKS:
    make_attack_test(_u)
    make_cpu_test(_u)
for _u in DEFENCES:
    make_defence_test(_u)


@test(modes=("ds",))
def battle_scene_oozium_attack_on_map(ctx):
    """An Oozium's attack has no battle scene (Dual Strike resolves it on the
    map), by the player or by the CPU; the target is destroyed."""
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "olaf"], visuals="a")
    fire(g, (10, 10), (11, 10))
    seen = watch_scene(ctx, g, "oozium_attacks", 600)
    g.wait_for_input()
    ctx.eq(seen["frames"], 0, "the player's Oozium attack: no battle scene")
    ctx.check(g.unit_at(11, 10) is None, "the Oozium's target is destroyed")
    g.e.close()
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, OOZIUM, 11, 10)
    g = ctx.start(m, ["andy", "olaf"], visuals="a")
    out = {}
    g.end_turn(human=1, observe=lambda gg: out.update(seen=watch_scene(ctx, gg, "cpu_oozium", 2000)))
    ctx.eq(out["seen"]["frames"], 0, "the CPU's Oozium attack: no battle scene")
    ctx.check(g.unit_at(10, 10) is None, "the CPU Oozium's target is destroyed")


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
    ("tank", OOZIUM, 1, ("plain", "plain"), 1),
    ("piperunner", "bcopter", 2, ("pipe", "plain"), 0),
    (CARRIER, "fighter", 3, ("sea", "plain"), 0),
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
        ctx.check(pals <= {4 * side, 4 * side + 1, 4 * side + 2, 4 * side + 3} | FX_PALS,
                  f"{label}: in the side's palettes and the effects' {pals}")
        ctx.eq(anim, off, f"{label}: the units end as with animations off")


@test(modes=("ds",))
def netplay_battle_scene(ctx):
    """A Megatank's animated battle (its volley and effects) replayed on two
    rollback peers."""
    m = ctx.map()
    m.unit(1, "megatank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "hawke"], visuals="a")
    fire(g, (10, 10), (11, 10))
    g.wait_for_input()
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    state = (STATE, 0x200)
    offline = {a: g.e.read(a, n) for a, n in (units, state)}
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, state])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")


@test(modes=("ds",))
def netplay_cpu_battle_scene(ctx):
    """The CPU's Carrier launching its missiles at the player's Fighters, and a
    Piperunner's volley, replayed on two rollback peers."""
    m = ctx.map()
    m.terrain(14, 10, "sea").terrain(8, 12, "pipe")
    m.unit(1, "fighter", 10, 10).unit(2, CARRIER, 14, 10)
    m.unit(1, "piperunner", 8, 12).unit(2, "tank", 10, 12)
    g = ctx.start(m, ["andy", "hawke"], visuals="a")
    fire(g, (8, 12), (10, 12))
    g.wait_for_input()
    g.end_turn(human=1)
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    state = (STATE, 0x200)
    offline = {a: g.e.read(a, n) for a, n in (units, state)}
    ctx.check(offline[STATE][0x40] != 0 or offline[STATE][0] != 0, "a Dual Strike scene ran")
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, state])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")
