"""Dual Strike's power animations on the map (power_anim.rs, with the pack):
Ex Machina (bolt, sparks, flashes, shake), Covering Fire (three missiles and
impacts), Urban Blight (Dual Strike's overlay), for a human and for the CPU,
and in every army colour. The powers' effects themselves are pinned by
test_co_powers.py; here the animation must actually play (its RAM state
advances, the bolt's map and the overlay's map are Dual Strike's) and the
battle go on. Frames are kept in the test's output folder."""

from aw2test import rom as romlib
from aw2test.harness import test

DS = ("ds",)
STATE = 0x0203F7A0            # power_anim::STATE: kind, x, y, loaded, u16 t
BG0_MAP_BUFFER_PTR = 0x08499578
WAVE_FIRST_TILE = 0x2B0       # the overlay's tiles (charblock + 0x5600)
URBAN_TILES = 12              # Dual Strike's Urban Blight overlay: 12 tiles
NAMES = {1: "Ex Machina", 2: "Covering Fire"}
ARMY_COLOURS = ["os", "bm", "ge", "yc", "bh"]


def bg0_map(g):
    buf = g.e.u32(BG0_MAP_BUFFER_PTR)
    raw = g.e.read(buf, 2048)
    return [raw[2 * k] | raw[2 * k + 1] << 8 for k in range(1024)]


def watch(ctx, g, frames, prefix, step=2, shots_every=4):
    """Sample the animation's state for `frames` frames; returns what was seen."""
    seen = {"kinds": set(), "max_t": {}, "strikes": {}, "bolt": False, "overlay": set(), "shots": 0}
    last_t = {}
    for k in range(0, frames, step):
        g.e.wait(step)
        st = g.e.read(STATE, 6)
        kind, t = st[0], st[4] | st[5] << 8
        m = [v for v in bg0_map(g) if v]
        overlay = False
        if m:
            tiles = {v & 0x3FF for v in m}
            if all(WAVE_FIRST_TILE <= x < WAVE_FIRST_TILE + URBAN_TILES for x in tiles) and (m[0] >> 12) == 8:
                seen["overlay"].add(len(tiles))
                overlay = True
            if kind == 1:
                seen["bolt"] = True
        if kind:
            seen["kinds"].add(kind)
            seen["max_t"][kind] = max(seen["max_t"].get(kind, 0), t)
            if t < last_t.get(kind, 1 << 16):
                seen["strikes"][kind] = seen["strikes"].get(kind, 0) + 1
            last_t[kind] = t
        if (kind or overlay) and k % shots_every == 0:
            ctx.shot(g, f"{prefix}_{k:04d}")
            seen["shots"] += 1
    return seen


def fire(ctx, g, army, which, frames=2400):
    g.charge_power(army, which)
    g.open_map_menu()
    g.choose("Super" if which == "super" else "Power", g.MAP_MENU)
    return watch(ctx, g, frames, "anim")


def expect(ctx, seen, kind, strikes=1, overlay=False):
    ctx.log(f"seen: {seen}")
    if overlay:
        ctx.check(seen["overlay"], "Dual Strike's Urban Blight overlay was on BG0")
        return
    ctx.check(kind in seen["kinds"], f"{NAMES[kind]}'s animation played")
    ctx.check(seen["max_t"].get(kind, 0) >= 60, f"{NAMES[kind]}: it ran its frames ({seen['max_t'].get(kind)})")
    ctx.eq(seen["strikes"].get(kind, 0), strikes, f"{NAMES[kind]}: strikes shown")
    if kind == 1:
        ctx.check(seen["bolt"], "the bolt's map was on BG0")


def map_two(ctx):
    m = ctx.map()
    m.terrain(20, 10, "city", 0).terrain(22, 10, "city", 2).terrain(12, 8, "base", 2)
    m.unit(1, "tank", 4, 4).unit(1, "infantry", 5, 5)
    m.unit(2, "infantry", 20, 10).unit(2, "infantry", 22, 10).unit(2, "tank", 21, 10)
    m.unit(2, "mech", 12, 8).unit(2, "tank", 13, 12)
    return m


POWERS = [("vonbolt", "super", 1), ("rachel", "super", 2), ("kindle", "power", 3)]


def human(ctx, co, which, kind):
    g = ctx.start(map_two(ctx), [co, "andy"])
    hp0 = sum(u["hp"] for u in g.units(2))
    seen = fire(ctx, g, 1, which)
    expect(ctx, seen, kind, strikes=3 if kind == 2 else 1, overlay=kind == 3)
    g.wait_for_input()
    ctx.check(sum(u["hp"] for u in g.units(2)) < hp0, "the power still hurt the enemy")
    ctx.eq(g.e.read(STATE, 1)[0], 0, "the animation's state is cleared")
    ctx.shot(g, "after")
    g.end_turn()
    ctx.eq(g.current_army(), 1, "the turn comes back to army 1")


def cpu(ctx, co, which, kind):
    m = map_two(ctx)
    # The CPU's targets: army 1's units near its own.
    m.unit(1, "tank", 20, 12).unit(1, "mech", 21, 13).unit(1, "infantry", 19, 12)
    m.terrain(19, 12, "city", 0)
    g = ctx.start(m, ["andy", co])
    seen = None
    for turn in range(3):
        g.charge_power(2, which)
        box = {}

        def observe(gg):
            box["seen"] = watch(ctx, gg, 2400, f"cpu{turn}", step=2, shots_every=6)

        g.end_turn(observe=observe)
        seen = box["seen"]
        if g.player(2)["powers_used"]:
            break
    ctx.eq(g.player(2)["powers_used"], 1, f"the CPU fired {co}'s power")
    expect(ctx, seen, kind, strikes=3 if kind == 2 else 1, overlay=kind == 3)
    ctx.eq(g.e.read(STATE, 1)[0], 0, "the animation's state is cleared")


def make(co, which, kind):
    def h(ctx):
        human(ctx, co, which, kind)

    def c(ctx):
        cpu(ctx, co, which, kind)
    h.__name__ = f"power_anim_{co}"
    c.__name__ = f"power_anim_cpu_{co}"
    test(modes=DS)(h)
    test(modes=DS)(c)


for co, which, kind in POWERS:
    make(co, which, kind)


def colours(ctx, co, which, kind, colour):
    m = map_two(ctx)
    # Record colours: [0] the five-army mark, then each army's country
    # (1 Orange Star .. 5 Black Hole).
    other = 2 if colour == 1 else 1
    m.colours = [0, colour, other, 3 if colour != 3 and other != 3 else 4, 4 if colour != 4 else 3]
    g = ctx.start(m, [co, "andy"])
    ctx.eq(g.player(1)["colour"], colour, "army 1's colour")
    seen = fire(ctx, g, 1, which, frames=1800)
    expect(ctx, seen, kind, strikes=3 if kind == 2 else 1, overlay=kind == 3)


def make_colours(co, which, kind, colour):
    def fn(ctx):
        colours(ctx, co, which, kind, colour)
    fn.__name__ = f"power_anim_{co}_{ARMY_COLOURS[colour - 1]}"
    test(modes=DS)(fn)


for co, which, kind in POWERS:
    for colour in range(1, 6):
        make_colours(co, which, kind, colour)


@test(modes=DS)
def power_anim_five_armies(ctx):
    """A five-army battle: Von Bolt (army 1) fires Ex Machina among four enemy armies."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)
    m.unit(5, "infantry", 16, 11).unit(5, "tank", 17, 11)
    m.unit(2, "tank", 16, 10).unit(3, "mech", 17, 10).unit(4, "infantry", 18, 11)
    m.unit(1, "tank", 4, 4)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    # The Teams screen's five-army stops are not driven by the harness:
    # army 1 takes Von Bolt directly.
    g.e.w8(g.player(1)["addr"] + 0x1D, romlib.co_id("vonbolt"))
    seen = fire(ctx, g, 1, "super")
    expect(ctx, seen, 1)
    g.wait_for_input()
    ctx.shot(g, "after")


@test(modes=DS)
def power_anim_sturm_keeps_aw2_meteor(ctx):
    """Sturm's Meteor Strike is AW2's: no Dual Strike animation."""
    g = ctx.start(map_two(ctx), ["sturm", "andy"])
    seen = fire(ctx, g, 1, "super", frames=1500)
    ctx.log(f"seen: {seen}")
    ctx.check(not seen["kinds"] and not seen["bolt"], "nothing of Dual Strike's played")
