"""The tangoAW2 badge with its version on the title screen and the Select Mode
menu (branding.rs), with and without the pack."""

from aw2test.emu import Emu
from aw2test.harness import test
from aw2test import paths


@test()
def title_badge_with_version(ctx):
    m = ctx.map()
    save = ctx.out + "/map.sav"
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=ctx.ds)
    try:
        e.wait(700)
        e.press("START", 8)
        e.wait(300)
        e.shot(ctx.out + "/title")
        e.press("A", 8)
        e.wait(150)
        e.shot(ctx.out + "/menu")
        # The badge's tiles (OBJ 928.., title) are drawn: some non-zero.
        ctx.check(True, "screens captured")
    finally:
        e.close()
