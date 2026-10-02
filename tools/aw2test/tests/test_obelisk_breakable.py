"""Black Obelisks (and Crystals) stay breakable everywhere: only Means to an
End's Grand Bolt weak points (crate::grand_bolt, their own tile) are ever
closed. An attack takes an Obelisk's hit points, and hit points it lost stay
lost: on a Versus design map, in DS Campaign missions with Obelisks
(Crystal Calamity, For the Future!), and in Means to an End only the closed
weak points keep theirs."""

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

INVENTIONS = 0x02028360
OBELISK, CRYSTAL = 0x193, 0x192


def entries(e):
    out = []
    for k in range(16):
        a = INVENTIONS + 8 * k
        kind = (e.u16(a + 2) >> 6) & 15
        if kind == 0:
            break
        out.append((e.u8(a), e.u8(a + 1), kind, a))
    return out


def lowered_stays(ctx, e, a, label):
    hp = e.u8(a + 4)
    e.w8(a + 4, hp - 20)
    e.wait(30)
    ctx.eq(e.u8(a + 4), hp - 20, f"{label}: hit points it lost stay lost")


@test()
def obelisk_breakable_versus(ctx):
    """A Versus battle on a design map with a Black Obelisk and a Crystal: a
    tank's shot takes the Obelisk's hit points; damage to either stays."""
    m = ctx.map()
    m.terrain(18, 6, OBELISK)
    m.terrain(8, 6, CRYSTAL)
    m.unit(1, "tank", 18, 9).unit(2, "infantry", 25, 15)
    g = ctx.start(m, ["andy", "andy"])
    e = g.e
    obelisk = next(a for x, y, k, a in entries(e) if k == 3)
    crystal = next(a for x, y, k, a in entries(e) if k == 4)
    hp = e.u8(obelisk + 4)
    g.select(18, 9)
    g.move_to(18, 8)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(18, 7)
    g.wait_for_input()
    ctx.check(e.u8(obelisk + 4) < hp, f"the tank's shot: the Obelisk {hp} -> {e.u8(obelisk + 4)}")
    lowered_stays(ctx, e, obelisk, "the Obelisk")
    lowered_stays(ctx, e, crystal, "the Crystal")


def _ds_mission(index):
    def fn(ctx):
        e = Emu(save=paths.base_save(), ds=True)
        g = Game(e, ctx.image)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=dc.ORDER.index(index))
        d.wait_map()
        obelisks = [a for x, y, k, a in entries(e) if k == 3]
        ctx.require(obelisks, f"{dc.DsData().mission(index)['name']}: its Obelisks")
        for a in obelisks:
            lowered_stays(ctx, e, a, f"Obelisk at ({e.u8(a)}, {e.u8(a + 1)})")
    fn.__name__ = f"obelisk_breakable_ds_mission_{index}"
    test(modes=("ds",))(fn)


for _i in (18, 23):   # Crystal Calamity, For the Future!
    _ds_mission(_i)


@test(modes=("ds",))
def grand_bolt_only_closed_parts_resist(ctx):
    """Means to an End: its weak points are the Grand Bolt's parts (a
    minicannon on tile 0x194 at (3, 9), (9, 11), (15, 9)); with their
    crystals standing they keep their hit points; its crystals (Black
    Crystals) lose theirs."""
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(24))
    d.wait_map()
    inv = entries(e)
    parts = {(x, y): a for x, y, k, a in inv if k == 4 and (x, y) in [(3, 9), (9, 11), (15, 9)]}
    ctx.eq(sorted(parts), [(3, 9), (9, 11), (15, 9)], "three parts")
    ctx.eq([k for _, _, k, _ in inv if k == 3], [], "no Black Obelisk on the map")
    for a in parts.values():
        hp = e.u8(a + 4)
        e.w8(a + 4, hp - 20)
        e.wait(4)
        ctx.eq(e.u8(a + 4), hp, "a closed weak point keeps its hit points")
    crystals = [a for x, y, k, a in inv if k == 4 and (x, y) not in parts]
    for a in crystals:
        lowered_stays(ctx, e, a, "a Black Crystal")
