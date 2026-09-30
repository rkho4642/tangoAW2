"""A base builds Piperunners only while a pipe or an intact pipe seam is next
to it (Dual Strike pack; crate::cpu_tactics `build_domains` for the build
menu, `next_to_pipes` for the CPU). The Dual Strike maps join each army's
Piperunner base to its pipe through a seam: breaking the seam ends that
base's Piperunners, and a Piperunner cannot cross the broken seam."""

from aw2test import ram, rom as romlib
from aw2test.game import NavError
from aw2test.harness import test

SEAM_ACROSS, PIPE_ACROSS, BROKEN_SEAM = 0x162, 0x142, 0x180
PIPE, SEAM = 15, 16
PIPERUNNER = 9
BUILD_LIST = 0x02023830


def menu(g, x, y):
    """The unit ids the build menu at (x, y) offers (then closed)."""
    g.wait_idle()
    g.goto(x, y)
    g.e.press("A", 4)
    g.e.wait(60)
    ids = []
    for k in range(30):
        t = g.e.u8(BUILD_LIST + 4 * k)
        if t == 0:
            break
        ids.append(t)
    g.e.press("B", 4)
    g.wait_for_input()
    return ids


def seam_map(ctx):
    """Army 1: a base joined to a pipe only through a seam (5,5)-(6,5)-(7..11,5),
    two Md Tanks by the seam, a Piperunner on the pipe; a base with no pipe."""
    m = ctx.map()
    m.terrain(5, 5, "base", 1).terrain(6, 5, SEAM_ACROSS)
    for x in range(7, 12):
        m.terrain(x, 5, PIPE_ACROSS)
    m.terrain(5, 12, "base", 1)
    m.unit(1, "mdtank", 6, 6).unit(1, "mdtank", 6, 4)
    if ctx.ds:
        m.unit(1, "piperunner", 8, 5)
    return m


def reach(g, x, y, to):
    """Whether the unit at (x, y) can move to `to` (its action menu opens)."""
    g.select(x, y)
    try:
        g.move_to(*to)
    except NavError:
        g.e.press("B", 4)
        g.e.wait(20)
        g.e.press("B", 4)
        g.wait_for_input()
        return False
    g.e.press("B", 4)
    g.e.wait(20)
    g.e.press("B", 4)
    g.wait_for_input()
    return True


@test()
def piperunner_base_needs_intact_seam(ctx):
    g = ctx.start(seam_map(ctx), ["andy", "andy"])
    e = g.e
    e.w32(g.player(1)["addr"], 90000)
    ctx.eq(g.terrain_class(6, 5) & 0x1F, SEAM, "the seam is intact")
    by_seam, lone = menu(g, 5, 5), menu(g, 5, 12)
    ctx.log(f"by the seam {by_seam}\nno pipe {lone}")
    if ctx.mode == "ds":
        ctx.check(PIPERUNNER in by_seam, f"a base next to an intact seam offers the Piperunner: {by_seam}")
        ctx.check(PIPERUNNER not in lone, f"a base with no pipe next to it does not: {lone}")
        ctx.check(reach(g, 8, 5, (5, 5)), "the Piperunner crosses the intact seam to its base")
    else:
        ctx.check(PIPERUNNER not in by_seam + lone, "no Piperunner without the pack")
    # Break the seam: two Md Tank shots (99 HP).
    for tx, ty in ((6, 6), (6, 4)):
        if g.terrain_class(6, 5) & 0x1F != SEAM:
            break
        g.select(tx, ty)
        g.move_to(tx, ty)
        g.choose("Fire", g.ACTION_MENU)
        g.pick_target(6, 5)
        g.wait_for_input()
    broken = g.terrain_class(6, 5) & 0x1F
    ctx.check(broken not in (PIPE, SEAM), f"the seam is broken (class {broken}: rubble, walked on like plain)")
    ctx.shot(g, "broken")
    after = menu(g, 5, 5)
    ctx.log(f"after {after}")
    ctx.check(PIPERUNNER not in after, f"the base no longer offers the Piperunner: {after}")
    if ctx.mode == "ds":
        co = e.u32(e.u32(0x08042DDC) + romlib.CO_RECORD * g.player(1)["co"] + 0x38 + 0x18)
        ctx.eq(e.u8(co + 32 * 7 + broken), 0xFF, "the pipe row: a broken seam is impassable")
        ctx.check(not reach(g, 8, 5, (5, 5)), "the Piperunner cannot cross the broken seam")


def cpu_map(ctx, seam_tile):
    """Army 2 (CPU, rich) has two bases, each joined to a pipe by a seam
    (`seam_tile`: intact or broken)."""
    m = ctx.map()
    for bx in (20, 24):
        m.terrain(bx, 4, "base", 2).terrain(bx, 5, 0x163 if seam_tile == SEAM_ACROSS else seam_tile)
    for x in range(19, 27):
        m.terrain(x, 6, PIPE_ACROSS)
    m.unit(1, "tank", 4, 4).unit(1, "mdtank", 5, 4).unit(1, "artillery", 4, 6)
    for x, y in ((1, 1), (2, 1), (1, 2), (2, 2), (3, 1)):
        m.unit(1, "neotank", x, y)
    return m


def cpu_piperunners(ctx, seam_tile, days=8):
    g = ctx.start(cpu_map(ctx, seam_tile), ["andy", "andy"])
    seen = set()
    for day in range(days):
        g.e.w32(g.player(2)["addr"] + ram.P_FUNDS, 100000)
        try:
            g.end_turn()
        except NavError:
            break
        seen |= {u["id"] for u in g.units(2) if u["type"] == PIPERUNNER}
    ctx.shot(g, f"cpu_{seam_tile:x}")
    return len(seen)


@test(modes=("ds",))
def cpu_piperunners_stop_at_broken_seam(ctx):
    """The CPU buys Piperunners at bases joined to the pipe by intact seams,
    and none where the seams are broken."""
    intact = cpu_piperunners(ctx, SEAM_ACROSS)
    broken = cpu_piperunners(ctx, BROKEN_SEAM)
    ctx.log(f"Piperunners bought: seams intact {intact}, broken {broken}")
    ctx.check(intact > 0, f"the CPU buys Piperunners behind intact seams ({intact})")
    ctx.eq(broken, 0, "the CPU buys none behind broken seams")
