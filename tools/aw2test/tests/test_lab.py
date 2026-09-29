"""Capturing a Lab in Versus: the battle goes on (the Lab ends battles only in the
campaign, through the campaign's own map rules). With the Dual Strike pack the
Lab is the Com Tower (crate::com_tower)."""

from aw2test.harness import test

LAB_NEUTRAL = 0x1D9
LAB_CLASS = 0x14


def capture(g, x, y):
    g.select(x, y)
    g.move_to(x, y)
    g.choose("Capt", g.ACTION_MENU)
    g.wait_for_input()


@test()
def lab_capture_does_not_end_versus(ctx):
    m = ctx.map()
    m.terrain(10, 10, LAB_NEUTRAL)
    m.unit(1, "infantry", 10, 10).unit(2, "infantry", 25, 15)
    g = ctx.start(m, ["andy", "andy"], humans=(1,))
    ctx.eq(g.terrain_class(10, 10), LAB_CLASS, "a neutral Lab")
    capture(g, 10, 10)
    ctx.log(f"after first capture: class {g.terrain_class(10, 10):#x}")
    g.end_turn()
    capture(g, 10, 10)
    g.e.wait(240)
    ctx.shot(g, "captured")
    ctx.eq(g.terrain_class(10, 10), 0x20 | LAB_CLASS, "the Lab is Orange Star's")
    ctx.check(not g.battle_over(), "the battle goes on")
    ctx.eq(g.e.u8(g.e.u32(0x08499598) + 0x3C + 0x10), 1, "army 1 counts one Lab")
    # And play continues: another full turn round.
    g.end_turn()
    ctx.check(not g.battle_over(), "still going after the next turn")
