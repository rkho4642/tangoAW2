"""The Black Crystal and Black Obelisk heal Black Hole's units at its turn start
(obelisk.rs), with Dual Strike's heal effect on the map (heal_effect.rs)."""

from aw2test.harness import test

CRYSTAL, OBELISK = 0x192, 0x193


def heal_map(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)   # Black Hole's HQ
    m.terrain(8, 6, CRYSTAL)
    m.terrain(18, 6, OBELISK)
    m.unit(5, "infantry", 16, 11).unit(5, "tank", 8, 7).unit(5, "mech", 20, 9).unit(5, "recon", 2, 15)
    m.colours = [5, 1, 2, 3, 4]
    return m


def watch_heal(ctx, g, prefix):
    """End army 1's turn and watch Black Hole's turn start: each structure's
    animation (every other frame saved as {prefix}_crystal_*, {prefix}_obelisk_*)
    and the heal."""
    for x, y in ((8, 7), (20, 9), (2, 15)):
        ctx.set_hp(g, x, y, 40)
    g.goto(12, 8)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(g.e.wait_until(lambda: g.current_army() == 5, 20000, step=10), "Black Hole's turn")
    # tangoAW2's heal state (heal_effect.rs): start clock, structure (1 Crystal,
    # 2 Obelisk; 0 once its animation is over), x, y.
    shows = []
    last = None
    hp = None
    names = {1: "crystal", 2: "obelisk"}
    n = 0
    for k in range(400):
        g.e.wait(2)
        state = g.e.read(0x0203FD80, 8)
        kind = state[4]
        if kind and kind != last:
            shows.append((k, kind, state[5], state[6]))
            n = 0
            if hp is None:
                hp = {p: (g.unit_at(*p) or {}).get("hp") for p in ((8, 7), (20, 9), (2, 15))}
        last = kind
        if kind in names:
            ctx.shot(g, f"{prefix}_{names[kind]}_{n:03d}")
            n += 1
        elif shows and k - shows[-1][0] > 150:
            break
    ctx.log(f"shows: {shows}")
    ctx.eq([(kind, x, y) for _, kind, x, y in shows], [(1, 8, 6), (2, 17, 5)],
           "the heal is shown at the Crystal and at the Obelisk (its top left)")
    ctx.eq(g.e.read(0x0203FD84, 1)[0], 0, "the animation is over")
    ctx.require(hp is not None, "units read during the heal")
    ctx.eq(hp[(8, 7)], 60, "Crystal: +2 HP within 2")
    ctx.eq(hp[(20, 9)], 80, "Obelisk: +4 HP within 4")
    ctx.eq(hp[(2, 15)], 40, "far away: nothing")
    return shows


@test(modes=("ds",))
def crystal_and_obelisk_heal_with_effect(ctx):
    """Black Hole played by the CPU."""
    g = ctx.start(heal_map(ctx), None)
    watch_heal(ctx, g, "bh")


@test(modes=("ds",))
def crystal_and_obelisk_heal_with_effect_human(ctx):
    """Black Hole played by a person: the same animations and heal, then its
    turn is theirs."""
    g = ctx.start(heal_map(ctx), None, humans=(1, 5))
    watch_heal(ctx, g, "human")
    g.e.wait(600)
    ctx.eq(g.current_army(), 5, "Black Hole's turn stays, waiting for its player")
    ctx.shot(g, "human_turn")


@test(modes=("ds",), netplay=True)
def netplay_crystal_and_obelisk_heal(ctx):
    """The heal and its shown effect hold Black Hole's turn the same way on both
    peers and the replay."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)
    m.terrain(8, 6, CRYSTAL).terrain(18, 6, OBELISK)
    m.unit(5, "infantry", 16, 11).unit(5, "tank", 8, 7).unit(5, "mech", 20, 9)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    g.end_turn(human=1)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")
