"""The Wasteland biome of design maps (Dual Strike pack only).

A design map saved with the Wasteland byte is drawn with Wasteland's colours
(tangoAW2's derived set in the ROM image at 0x08671000) with the pack, and with
AW2's own clear colours without it. The rules are the same either way.
"""

from aw2test.harness import test

PAL_BUFFER = 0x030020C0
AW2_CLEAR = 0x080BF8C4
WASTELAND_CLEAR = 0x08671000
BIOME = 0x03004493


def wasteland_map(ctx):
    m = ctx.map()
    for x in range(4, 12):
        m.terrain(x, 8, "road")
    for (x, y) in ((2, 3), (3, 3), (3, 4), (4, 5)):
        m.terrain(x, y, "wood")
    for (x, y) in ((6, 3), (7, 4), (6, 5)):
        m.terrain(x, y, "mountain")
    for x in range(9, 14):
        for y in range(1, 6):
            m.terrain(x, y, "sea")
    m.terrain(8, 6, "city", 0).terrain(12, 7, "city", 1)
    m.unit(1, "tank", 6, 9).unit(2, "tank", 9, 9)
    m.biome = 1
    return m


@test()
def wasteland_colours(ctx):
    g = ctx.start(wasteland_map(ctx), ["andy", "andy"])
    e = g.e
    ctx.shot(g, "wasteland")
    lines = e.read(PAL_BUFFER, 128)
    if ctx.mode == "ds":
        ctx.eq(e.u8(BIOME) >> 4 & 7, 1, "the biome is Wasteland")
        ctx.eq(lines, e.read(WASTELAND_CLEAR, 128), "BG palettes 0-3 are Wasteland's")
        ctx.check(lines != e.read(AW2_CLEAR, 128), "and not AW2's")
    else:
        ctx.eq(lines, e.read(AW2_CLEAR, 128), "BG palettes 0-3 are AW2's")
    # Same rules: a tank still attacks a tank for the chart's base damage.
    ctx.attack(g, (6, 9), (8, 9), (9, 9), expect_base=55)
