"""Dual Strike's battle backgrounds (pack only, crate::ds_backdrop).

A Piperunner fights in front of Dual Strike's background with the pipe it
stands on; every ground or naval unit on a Wasteland map fights in front of
Dual Strike's Wasteland background for its terrain; on normal maps a Com
Tower gets AW2's city background (Dual Strike's choice). Each is checked from
the side's BG palettes while the scene runs: Dual Strike's backgrounds are
drawn only in Dual Strike's own colours (read from the .nds), AW2's are not.
The player's and the CPU's battles, every weather, all five army colours
(Black Hole in a five-army map), the animations off, and a netplay replay.
"""

import struct

from aw2test import ram
from aw2test.harness import test
from aw2test.rom import DualStrike

PAL = 0x05000000
MAIN_CALLBACK = 0x03000000
FIRST_PAL = (1, 4)  # the sides' three BG palettes
SIDE_VRAM = ((0x06008000, 0x06003000), (0x0600C000, 0x06003800))  # tiles, map
WEATHERS = ("clear", "snow", "rain", "sandstorm")
COLOURS = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}
WASTELAND = 3  # Dual Strike's tileset
LAB_TILE = 0x1D9  # + owner (0 neutral .. 4): a Com Tower in Versus

_ds = None


def ds():
    global _ds
    if _ds is None:
        _ds = DualStrike()
    return _ds


def colours_of(name, n=16):
    b = ds().file("battle/" + name, decompress=False)
    return {struct.unpack_from("<H", b, 2 * i)[0] & 0x7FFF for i in range(1, n) if 2 * i + 1 < len(b)} - {0}


GROUP = {2: 4, 3: 2, 4: 1, 5: 3, 6: 8, 17: 8, 18: 8, 22: 8, 7: 5, 19: 5, 10: 10, 11: 11, 12: 7, 13: 6, 14: 9, 20: 9}
CLASS = {"plain": 1, "river": 2, "mountain": 3, "wood": 4, "road": 5, "city": 6, "sea": 7, "hq": 8, "airport": 10,
         "port": 11, "bridge": 12, "shoal": 13, "base": 14, "pipe": 15, "reef": 19, "lab": 20}


RAIN = [[188, -6, 58], [-31, 198, 0], [40, -1, 157], [364, 387, 432]]  # ds_backdrop::RAIN


def rain(c):
    v = [c & 31, (c >> 5) & 31, (c >> 10) & 31]
    out = 0
    for j in range(3):
        x = v[0] * RAIN[0][j] + v[1] * RAIN[1][j] + v[2] * RAIN[2][j] + RAIN[3][j]
        out |= max(0, min(31, (x + 128) // 256)) << (5 * j)
    return out


def expected(terrain, tileset, pipe=False, naval=False, hq_colour=1, weather="clear"):
    """Dual Strike's colours for a side: ground, horizon and sky palettes (and
    the pipe's), as ds_backdrop picks them (snow: Dual Strike's snow tileset;
    rain: AW2's rain colours of them)."""
    if weather == "snow":
        tileset = 1
    if weather == "rain":
        colours, ground = expected(terrain, tileset, pipe, naval, hq_colour)
        return {rain(c) for c in colours}, {rain(c) for c in ground}
    cls = CLASS[terrain]
    if cls == 20:
        cls = 22  # a Lab is a Com Tower in Versus
    g = 5 if naval else (11 + hq_colour if cls == 8 else GROUP.get(cls, 0))
    base = 0x11A + 12 * g
    sky_cls = 7 if g == 5 else cls
    t = tileset
    if sky_cls == 3:
        sky = [0x1FA, 0x1FB, 0x1FE, 0x1FF][t]
    elif sky_cls == 4:
        sky = [0x1F2, 0x1F3, 0x1F6, 0x1F7][t]
    elif sky_cls in (6, 8, 10, 11, 14, 17, 18, 20, 22):
        sky = [0x202, 0x203, 0x206, 0x207][t]
    else:
        sky = [0x1EA, 0x1EB, 0x1EA, 0x1EA][t]
    ground = colours_of("%03x" % (base + 4 + t))
    out = ground | colours_of("%03x" % (base + 8 + t)) | colours_of("%03x" % sky, 64)
    if pipe:
        out |= colours_of("1e7")
    return out, ground


def side_colours(g, side):
    raw = g.e.read(PAL + 32 * FIRST_PAL[side], 96)
    return {struct.unpack_from("<H", raw, 2 * i)[0] & 0x7FFF for i in range(48) if i % 16} - {0}


def side_background(g, side):
    tiles, bgmap = SIDE_VRAM[side]
    return g.e.read(PAL + 32 * FIRST_PAL[side], 96) + g.e.read(tiles, 0x3000) + g.e.read(bgmap, 0x800)


def sample(ctx, g, name, max_frames=900):
    """Wait for the battle scene, then read both sides' backgrounds."""
    e = g.e
    waited = 0
    while waited < max_frames:
        e.wait(6)
        waited += 6
        if e.u32(MAIN_CALLBACK) == 0:
            e.wait(30)
            ctx.shot(g, name)
            return {"colours": [side_colours(g, s) for s in (0, 1)], "bg": [side_background(g, s) for s in (0, 1)]}
    return None


def check_ds(ctx, seen, side, label, want):
    colours, ground = want
    ctx.check(seen is not None, f"{label}: the battle scene ran")
    if seen is None:
        return
    got = seen["colours"][side]
    ctx.check(len(got) >= 20 and got <= colours,
              f"{label}: side {side}'s background is Dual Strike's ({len(got)} colours, {len(got - colours)} not Dual Strike's)")
    ctx.check(len(got & ground) >= 8, f"{label}: with its ground's colours ({len(got & ground)})")


def check_aw2(ctx, seen, side, label, want):
    colours, _ = want
    got = seen["colours"][side]
    ctx.check(not got <= colours, f"{label}: side {side} keeps AW2's background")


def two_army_map(ctx, colours):
    m = ctx.map()
    m.colours[1], m.colours[2] = colours
    return m, ["andy", "olaf"], 2


def five_army_map(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 15, 0x1B4)  # Black Hole's HQ
    m.colours = [5, 1, 2, 3, 4]
    return m, None, 5


def other(c):
    return 1 if c != 1 else 2


def fire(g, src, target):
    g.select(*src)
    g.move_to(*src)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(*target)


def battle(ctx, name, new_side, unit, terrain, enemy, enemy_terrain, dist, colour=1, weather="clear", biome=0,
           cpu=False, owner=0):
    """A battle where the unit under test (army 1 in `colour`, or the CPU's
    army 2 in `colour`, Black Hole in a five-army map) fights `enemy`. Returns
    the sample and the side it is on (sides are the armies': the player's on
    the left)."""
    if colour == 5 and cpu:
        m, cos, army2 = five_army_map(ctx)
    elif cpu:
        m, cos, army2 = two_army_map(ctx, (other(colour), colour))
    else:
        m, cos, army2 = two_army_map(ctx, (colour, other(colour)))
    m.biome = biome
    x0, x1 = 10, 10 + dist
    if cpu:
        # The CPU's unit on the right: it fires where it stands.
        m.terrain(x1, 10, terrain, owner).terrain(x0, 10, enemy_terrain)
        m.unit(army2, unit, x1, 10).unit(1, enemy, x0, 10)
    else:
        m.terrain(x0, 10, terrain, owner).terrain(x1, 10, enemy_terrain)
        m.unit(1, unit, x0, 10).unit(army2, enemy, x1, 10)
    g = ctx.start(m, cos, visuals="a", weather=weather)
    if cpu:
        out = {}
        g.end_turn(human=1, observe=lambda gg: out.update(seen=sample(ctx, gg, name, 8000)))
        seen = out.get("seen")
    elif new_side == "attacks":
        fire(g, (x0, 10), (x1, 10))
        seen = sample(ctx, g, name)
        g.wait_for_input()
    g.e.close()
    return seen, (1 if cpu else 0)


def attacked(ctx, name, unit, terrain, attacker, attacker_terrain, dist, colour=1, weather="clear", biome=0, owner=0):
    """The player's AW2 unit attacks the unit under test (army 2 in `colour`,
    Black Hole in a five-army map). Returns the sample; the unit is on side 1."""
    if colour == 5:
        m, cos, army2 = five_army_map(ctx)
    else:
        m, cos, army2 = two_army_map(ctx, (other(colour), colour))
    m.biome = biome
    m.terrain(10, 10, attacker_terrain).terrain(10 + dist, 10, terrain, owner)
    m.unit(1, attacker, 10, 10).unit(army2, unit, 10 + dist, 10)
    g = ctx.start(m, cos, visuals="a", weather=weather)
    fire(g, (10, 10), (10 + dist, 10))
    seen = sample(ctx, g, name)
    g.wait_for_input()
    g.e.close()
    return seen


@test(modes=("ds",))
def battle_bg_piperunner_on_pipe(ctx):
    """A Piperunner on a pipe, attacking and attacked, in every weather: Dual
    Strike's plain background with its pipe; the other side keeps AW2's."""
    for weather in WEATHERS:
        want = expected("pipe", 0, pipe=True, weather=weather)
        seen, side = battle(ctx, f"pipe_attacks_{weather}", "attacks", "piperunner", "pipe", "tank", "plain", 2,
                            weather=weather)
        check_ds(ctx, seen, side, f"Piperunner attacks ({weather})", want)
        if seen:
            check_aw2(ctx, seen, 1 - side, f"Piperunner attacks ({weather})", want)
        seen = attacked(ctx, f"pipe_attacked_{weather}", "piperunner", "pipe", "tank", "plain", 1, weather=weather)
        check_ds(ctx, seen, 1, f"Piperunner attacked ({weather})", want)


@test(modes=("ds",))
def battle_bg_piperunner_armies(ctx):
    """Every army's Piperunner (the player's attacking, the CPU's attacking,
    Black Hole in a five-army map) on its pipe."""
    want = expected("pipe", 0, pipe=True)
    for colour in (1, 2, 3, 4):
        seen, side = battle(ctx, f"pipe_{colour}", "attacks", "piperunner", "pipe", "tank", "plain", 3, colour=colour)
        check_ds(ctx, seen, side, f"{COLOURS[colour]} Piperunner", want)
    for colour in (3, 5):
        seen, side = battle(ctx, f"cpu_pipe_{colour}", "attacks", "piperunner", "pipe", "tank", "plain", 3,
                            colour=colour, cpu=True)
        check_ds(ctx, seen, side, f"CPU {COLOURS[colour]} Piperunner", want)


@test(modes=("ds",))
def battle_bg_cpu_attacks_piperunner(ctx):
    """The CPU's artillery shells the player's Piperunner on its pipe."""
    m, cos, _ = two_army_map(ctx, (1, 2))
    m.terrain(10, 10, "pipe")
    m.unit(1, "piperunner", 10, 10).unit(2, "artillery", 12, 10)
    g = ctx.start(m, cos, visuals="a")
    out = {}
    g.end_turn(human=1, observe=lambda gg: out.update(seen=sample(ctx, gg, "cpu_shells_pipe", 3000)))
    g.e.close()
    check_ds(ctx, out.get("seen"), 0, "the CPU shells a Piperunner", expected("pipe", 0, pipe=True))


def tower_tile(owner):
    return 0x1B9 if owner == 5 else LAB_TILE + owner


def city_tile(owner):
    return 0x1B6 if owner == 5 else 0x1C2 + 5 * owner


@test(modes=("ds", "aw2"))
def battle_bg_com_tower(ctx):
    """A unit on a Com Tower (neutral, or owned by each army) gets the city
    background of that owner, as Dual Strike gives a Com Tower its city's; the
    player's and the CPU's. Without the pack a Lab keeps AW2's own."""
    # (owner, the tank's army colour, the CPU's tank); Black Hole's in a
    # five-army map.
    cases = [(0, 1, False), (1, 1, False), (2, 2, False), (3, 3, False), (4, 4, True), (0, 3, True), (5, 5, None)]
    for owner, colour, cpu in cases:
        if cpu is None:
            # Black Hole's tank on its tower, attacked by the player.
            tower = attacked(ctx, f"tower_{owner}", "tank", tower_tile(owner), "tank", "plain", 1, colour=colour)
            city = attacked(ctx, f"city_{owner}", "tank", city_tile(owner), "tank", "plain", 1, colour=colour)
            side = side2 = 1
        else:
            tower, side = battle(ctx, f"tower_{owner}_{int(cpu)}", "attacks", "tank", tower_tile(owner), "infantry",
                                 "plain", 1, colour=colour, cpu=cpu)
            city, side2 = battle(ctx, f"city_{owner}_{int(cpu)}", "attacks", "tank", city_tile(owner), "infantry",
                                 "plain", 1, colour=colour, cpu=cpu)
        label = f"{'CPU ' if cpu else ''}tank on a Com Tower (owner {owner})"
        ctx.check(tower is not None and city is not None, f"{label}: both scenes ran")
        if tower and city:
            same = tower["bg"][side] == city["bg"][side2]
            if ctx.ds:
                ctx.check(same, f"{label}: the city background of that owner")
            else:
                ctx.check(not same, f"{label}: without the pack, AW2's Lab background")


WASTELAND_CASES = [
    ("infantry", "plain", "infantry", "river", 1),
    ("infantry", "mountain", "infantry", "wood", 1),
    ("tank", "road", "tank", "city", 1),
    ("tank", "airport", "tank", "port", 1),
    ("tank", "bridge", "tank", "base", 1),
    ("infantry", "shoal", "infantry", "lab", 1),
]


@test(modes=("ds",))
def battle_bg_wasteland_terrains(ctx):
    """On a Wasteland map every terrain's background is Dual Strike's
    Wasteland one, on both sides."""
    for att, ta, dfd, td, dist in WASTELAND_CASES:
        seen, _ = battle(ctx, f"wl_{ta}_{td}", "attacks", att, ta, dfd, td, dist, biome=1)
        check_ds(ctx, seen, 0, f"Wasteland {ta}", expected(ta, WASTELAND))
        check_ds(ctx, seen, 1, f"Wasteland {td}", expected(td, WASTELAND))
    # Ships over Dual Strike's Wasteland sea; planes keep AW2's sky.
    seen, _ = battle(ctx, "wl_sea", "attacks", "battleship", "sea", "lander", "reef", 2, biome=1)
    check_ds(ctx, seen, 0, "Wasteland Battleship", expected("sea", WASTELAND, naval=True))
    check_ds(ctx, seen, 1, "Wasteland Lander", expected("reef", WASTELAND, naval=True))
    seen, _ = battle(ctx, "wl_air", "attacks", "fighter", "plain", "bomber", "plain", 1, biome=1)
    check_aw2(ctx, seen, 0, "Wasteland Fighter", expected("plain", WASTELAND))


@test(modes=("ds",))
def battle_bg_wasteland_weathers_armies(ctx):
    """Wasteland backgrounds in every weather, for every army, the player's
    and the CPU's battles, and the Piperunner there."""
    for weather in WEATHERS:
        seen, _ = battle(ctx, f"wl_{weather}", "attacks", "tank", "plain", "tank", "wood", 1, weather=weather, biome=1)
        check_ds(ctx, seen, 0, f"Wasteland plain ({weather})", expected("plain", WASTELAND, weather=weather))
        check_ds(ctx, seen, 1, f"Wasteland wood ({weather})", expected("wood", WASTELAND, weather=weather))
    for colour in (1, 2, 3, 4):
        seen, side = battle(ctx, f"wl_army_{colour}", "attacks", "tank", "road", "tank", "plain", 1, colour=colour,
                            biome=1)
        check_ds(ctx, seen, side, f"Wasteland {COLOURS[colour]} tank", expected("road", WASTELAND))
    for colour in (2, 5):
        seen, side = battle(ctx, f"wl_cpu_{colour}", "attacks", "infantry", "mountain", "infantry", "plain", 1,
                            colour=colour, cpu=True, biome=1)
        check_ds(ctx, seen, side, f"Wasteland CPU {COLOURS[colour]}", expected("mountain", WASTELAND))
    seen, side = battle(ctx, "wl_pipe", "attacks", "piperunner", "pipe", "tank", "plain", 3, biome=1)
    check_ds(ctx, seen, side, "Wasteland Piperunner", expected("pipe", WASTELAND, pipe=True))
    seen, side = battle(ctx, "wl_tower", "attacks", "tank", "lab", "infantry", "plain", 1, biome=1, owner=1)
    check_ds(ctx, seen, side, "Wasteland Com Tower", expected("lab", WASTELAND))


@test(modes=("ds",))
def battle_bg_animations_off(ctx):
    """With the battle animations off these battles resolve as usual."""
    for unit, ta, enemy, td, dist, biome in (("piperunner", "pipe", "tank", "plain", 3, 0),
                                              ("tank", "lab", "infantry", "plain", 1, 0),
                                              ("tank", "road", "tank", "city", 1, 1)):
        m, cos, _ = two_army_map(ctx, (1, 2))
        m.biome = biome
        m.terrain(10, 10, ta).terrain(10 + dist, 10, td)
        m.unit(1, unit, 10, 10).unit(2, enemy, 10 + dist, 10)
        g = ctx.start(m, cos, visuals="off")
        hp = g.unit_at(10 + dist, 10)["hp"]
        fire(g, (10, 10), (10 + dist, 10))
        g.wait_for_input()
        after = g.unit_at(10 + dist, 10)
        ctx.check(after is None or after["hp"] < hp, f"{unit} on {ta}: the battle resolved with animations off")
        g.e.close()


@test(modes=("ds",))
def netplay_battle_backgrounds(ctx):
    """A Piperunner's battle on a Wasteland map, replayed on two rollback
    peers mid-scene: the same backgrounds (palettes, tiles, maps)."""
    m, cos, _ = two_army_map(ctx, (1, 2))
    m.biome = 1
    m.terrain(10, 10, "pipe").terrain(13, 10, "wood")
    m.unit(1, "piperunner", 10, 10).unit(2, "tank", 13, 10)
    g = ctx.start(m, cos, visuals="a")
    fire(g, (10, 10), (13, 10))
    seen = sample(ctx, g, "netplay_bg")
    ctx.check(seen is not None, "the battle scene ran")
    peeks = [(PAL + 32, 192), (0x06008000, 0x3000), (0x0600C000, 0x3000), (0x06003000, 0x1000)]
    identical, values, text = ctx.netplay_replay(g, peeks)
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    if seen:
        want, _ = expected("pipe", WASTELAND, pipe=True)
        raw = values.get(PAL + 32, b"")
        got = {struct.unpack_from("<H", raw, 2 * i)[0] & 0x7FFF for i in range(48) if i % 16 and 2 * i < len(raw)} - {0}
        ctx.check(got and got <= want, "netplay peer 0 shows Dual Strike's background")
