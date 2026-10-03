"""Black Hole's structures in every DS Campaign mission against Dual Strike's.

Dual Strike builds a battle's structure list (`0x02183B08 + 0x48DC`, ARM9
`0x020DA650`) from each cell's terrain class: 0x15 a minicannon (kind 4),
0x17 a Black Crystal (9), 0x18 a Grand Bolt part (0xB + n), 0x1A a Black
Cannon (3, 3x3), 0x1C a Volcano (2, 4x4), 0x1D a Black Obelisk (0xA, 3x3),
0x1F a 4x4 structure picture (8: the missile pad, the fortress; no hit
points). DS_STRUCTURES below is that list as melonDS showed it at the
start of each mission's battle (kind, top-left x, y).

Here each is the invention AW2's game registered (`0x02028360`, kind in
bits 6..9 of +2, top-left cell): a minicannon kind 4, a Black Crystal a
minicannon on tile 0x192, a Black Obelisk a Black Cannon (kind 3) with
tile 0x193 in its middle, a Black Cannon kind 3 on AW2's own Black
Cannon tiles, a Volcano kind 2, a Grand Bolt part a minicannon on 0x194,
a 4x4 picture AW2's kind 8."""

from aw2test import dscampaign as dc
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test import paths
from aw2test.harness import test

INVENTIONS = 0x02028360
TILES = dc.MAP + 0xA22
ROWS = dc.MAP + 0x417A

# Dual Strike's battle structure lists (mission index -> (kind, x, y)),
# read in melonDS (scratchpad onyx/dsmission.py).
DS_STRUCTURES = {
    8: [(9, 21, 6)],
    13: [(4, 2, 4), (4, 2, 5), (4, 2, 6)],
    14: [(4, 16, 4), (4, 21, 4), (8, 17, 5), (4, 16, 9), (4, 21, 9)],
    16: [(4, 4, 2)],
    17: [(9, 5, 4), (9, 11, 4), (9, 8, 7), (9, 4, 9), (9, 12, 9), (9, 8, 11)],
    18: [(3, 9, 1), (0xA, 9, 10)],
    19: [(4, 15, 2), (4, 15, 4), (4, 15, 12), (4, 15, 14)],
    21: [(2, 8, 8)],
    22: [(8, 0, 0), (8, 17, 0), (8, 0, 14), (8, 17, 14)],
    23: [(0xA, 7, 1), (4, 3, 12), (4, 13, 12), (9, 3, 14), (9, 8, 14), (9, 13, 14), (4, 3, 16), (4, 13, 16)],
    24: [(0xB, 3, 9), (0xD, 15, 9), (0xC, 9, 11)],
}


def tile(e, x, y):
    return e.u16(TILES + 2 * (e.u16(ROWS + 2 * y) + x))


def ours(e):
    """AW2's invention list as Dual Strike's kinds."""
    out = []
    for k in range(16):
        a = INVENTIONS + 8 * k
        kind = (e.u16(a + 2) >> 6) & 0xF
        if kind == 0:
            break
        x, y = e.u8(a), e.u8(a + 1)
        if kind == 4:
            t = tile(e, x, y)
            ds = 9 if t == 0x192 else 0xB if t == 0x194 else 4
        elif kind == 3:
            ds = 0xA if tile(e, x + 1, y + 1) == 0x193 else 3
        else:
            ds = kind
        out.append((ds, x, y))
    return out


def _mission(step):
    index = dc.ORDER[step]

    def fn(ctx):
        """The mission's Black Hole structures are Dual Strike's: the same
        kinds at the same cells (a Black Cannon stays a Black Cannon, an
        Obelisk only where Dual Strike has one)."""
        e = Emu(save=paths.base_save(), ds=ctx.ds)
        g = Game(e, ctx.image)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=step)
        d.wait_map()
        have = ours(e)
        want = DS_STRUCTURES.get(index, [])
        # (the Grand Bolt's three parts all read as kind 0xB here)
        norm = lambda l: sorted((0xB if k in (0xB, 0xC, 0xD) else k, x, y) for k, x, y in l)
        ctx.eq(norm(have), norm(want), f"{dc.DsData().mission(index)['name']}: the structures (kind, x, y)")

    fn.__name__ = f"ds_structures_{step:02d}"
    test(modes=("ds",))(fn)


for _s in range(len(dc.ORDER)):
    _mission(_s)
