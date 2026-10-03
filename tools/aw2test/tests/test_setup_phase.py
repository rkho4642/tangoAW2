"""The DS Campaign's Setup phase (setup_phase.rs, docs/AW2.md "Setup
phase"): before day 1 of a mission whose COs the player picks, the map is
open for scouting ("Setup" at the top, no unit moved, nothing built); A
opens the Setup menu (CO, Intel, Options, Front in a two-front mission,
Deploy with Dual Strike's help line); Deploy starts day 1. Missions without
a pick (Jake's Trial) start at once, as AW2's own campaign and Versus do."""

import os

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test import twofront as tf
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

JAKES_TRIAL, BLACK_BOATS, LIGHTNING_STRIKES = 0, 9, 10
MAP_STATE = 0x030032D8
CURSOR_STATE = 0xD
DAY = 0x03004080


def boot(ctx):
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.auto_deploy = False
    return e, g, d


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


@test(modes=("ds",))
def setup_phase_before_day_one(ctx):
    """Black Boats Ahoy! (the player picks its COs): the map opens in the
    Setup phase; A on one of the player's units opens the Setup menu, not
    the unit; Deploy starts day 1's turn (its title and events) and the
    phase is over."""
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(BLACK_BOATS))
    d.wait_map()
    ctx.check(d.in_setup(), f"the Setup phase (setup {e.u8(dc.SETUP)})")
    ctx.eq((e.u16(DAY), e.u16(MAP_STATE)), (1, CURSOR_STATE), "day 1, the map's cursor")
    shot(ctx, e, "setup_map")
    own = g.units(1)
    ctx.require(bool(own), "the player has units")
    u = own[0]
    g.goto(u["x"], u["y"])
    e.wait(10)
    m = d.setup_menu()
    ctx.eq(m["names"], ["CO", "Intel", "Options", "Deploy"], "A on a unit: the Setup menu (no Front: one front)")
    shot(ctx, e, "setup_menu")
    g.choose("Deploy", dc.SETUP_MENU)
    e.wait_until(lambda: e.u8(dc.SETUP) != dc.SETUP_ACTIVE, 600, step=10)
    ctx.eq(e.u8(dc.SETUP), 0, "Deploy: the phase is over")
    d.wait_control()
    ctx.eq((e.u16(DAY), e.u16(0x030033EC)), (1, 1), "day 1, the player's turn")
    ctx.check(not d.in_setup(), "not in Setup")
    # The unit moves now.
    g.select(u["x"], u["y"])
    m = g.move_to(u["x"], u["y"])
    ctx.check(bool(m and m.get("names")), f"the unit's command menu ({m and m.get('names')})")
    shot(ctx, e, "after_deploy")


@test(modes=("ds",))
def setup_phase_only_with_a_pick(ctx):
    """Jake's Trial (its COs set): no Setup phase, day 1 at once, AW2's map
    menu."""
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(JAKES_TRIAL))
    d.wait_map()
    ctx.check(not d.in_setup(), f"no Setup phase (setup {e.u8(dc.SETUP)})")
    names = g.map_menu_names()
    ctx.check("Deploy" not in names and "End" in names, f"AW2's map menu ({names})")


@test(modes=("ds",))
def setup_phase_two_fronts(ctx):
    """Lightning Strikes: the Setup menu has Front, which shows the second
    front and comes back to the Setup phase; Deploy, then the rounds."""
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(LIGHTNING_STRIKES))
    d.wait_map()
    ctx.require(d.in_setup(), "the Setup phase")
    m = d.setup_menu()
    ctx.eq(m["names"], ["CO", "Intel", "Options", "Front", "Deploy"], "the Setup menu")
    g.choose("Front", dc.SETUP_MENU)
    ok = e.wait_until(lambda: e.u8(tf.VIEW) == 1 and e.u8(tf.BUSY) == tf.VIEW_IN and e.u8(tf.MAP_LOCK) == 0, 1500, step=10)
    ctx.check(ok, "the second front shows")
    e.wait(20)
    shot(ctx, e, "setup_front_view")
    ctx.check(tf.come_back(e, g), "back")
    ctx.check(d.in_setup(), "still the Setup phase")
    d.deploy()
    d.wait_control()
    ctx.check(not d.in_setup() and e.u16(DAY) == 1, "day 1")
    seen = tf.end_round(e, d)
    ctx.check(e.u8(tf.STARTED) == 1, f"the second front's round followed ({seen})")
