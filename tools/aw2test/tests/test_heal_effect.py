"""The Black Crystal and Black Obelisk heal Black Hole's units at its turn start
(obelisk.rs), with Dual Strike's heal effect on the map (heal_effect.rs)."""

from aw2test.harness import test

CRYSTAL, OBELISK = 0x192, 0x193


@test(modes=("ds",))
def crystal_and_obelisk_heal_with_effect(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)   # Black Hole's HQ
    m.terrain(8, 6, CRYSTAL)
    m.terrain(18, 6, OBELISK)
    m.unit(5, "infantry", 16, 11).unit(5, "tank", 8, 7).unit(5, "mech", 20, 9).unit(5, "recon", 2, 15)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    for x, y in ((8, 7), (20, 9), (2, 15)):
        ctx.set_hp(g, x, y, 40)
    g.goto(12, 8)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(g.e.wait_until(lambda: g.current_army() == 5, 20000, step=10), "Black Hole's turn")
    shows = []
    last = None
    hp = None
    for k in range(120):
        g.e.wait(6)
        state = g.e.read(0x0203FD80, 8)
        start = int.from_bytes(state[:4], "little")
        if (state[4] or state[5]) and start != last:
            last = start
            shows.append((k, state.hex()))
            if hp is None:
                hp = {p: (g.unit_at(*p) or {}).get("hp") for p in ((8, 7), (20, 9), (2, 15))}
        if shows and k - shows[-1][0] < 16:
            ctx.shot(g, f"bh_{k:03d}")
    ctx.log(f"shows: {shows}")
    ctx.eq(len(shows), 2, "the heal is shown at the Crystal and at the Obelisk")
    ctx.eq(hp[(8, 7)], 60, "Crystal: +2 HP within 2")
    ctx.eq(hp[(20, 9)], 80, "Obelisk: +4 HP within 4")
    ctx.eq(hp[(2, 15)], 40, "far away: nothing")


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
