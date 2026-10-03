"""Ring of Fire's Volcano in Dual Strike's colours (crate::volcano): AW2's
Volcano sprite drawn with Dual Strike's Volcano palette (bmap/00e
sub-palette 13, read from the .nds here), not AW2's teal one (0.5.0)."""

import os
import struct

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test import rom as romlib
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

AW2_VOLCANO_COLOURS = 0x080D3FC4
OBJ_PALETTE = 12


@test(modes=("ds",))
def ds_campaign_volcano_colours(ctx):
    want = romlib.DualStrike().file("bmap/00e")[32 * 13 + 2:32 * 13 + 32]
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(21))
    d.wait_map()
    g.goto(9, 9)
    e.wait(40)
    e.shot(os.path.join(ctx.out, "ring_of_fire_volcano"))
    oam = e.read(0x07000000, 0x400)
    volcano = [struct.unpack_from("<3H", oam, 8 * i) for i in range(128)]
    volcano = [a for a in volcano if (a[0] >> 8) & 3 != 2 and a[0] >> 14 == 0 and a[1] >> 14 == 3]
    ctx.check(len(volcano) == 1 and volcano[0][2] >> 12 == OBJ_PALETTE, f"the Volcano's 64x64 sprite in palette 12 ({volcano})")
    shown = e.read(0x05000200 + 32 * OBJ_PALETTE + 2, 30)
    ctx.eq(shown.hex(), want.hex(), "Dual Strike's Volcano colours (bmap/00e sub-palette 13)")
    ctx.check(shown != e.read(AW2_VOLCANO_COLOURS + 2, 30), "not AW2's")
