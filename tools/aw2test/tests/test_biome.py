"""Dual Strike's map looks (Dual Strike pack only).

A design map saved with the Wasteland byte is drawn with Dual Strike's own
Wasteland terrain (crate::ds_look, converted from the .nds at run time into
the ROM image at 0x08E80000) with the pack, and with AW2's own tiles and clear
colours without it. The rules are the same either way. The look's data is
checked metatile by metatile against Dual Strike's drawing (aw2test/looks.py),
for Wasteland, Desert and Snow; the screen's tiles, colours and tilemap
against the look's data, in every weather and in fog.
"""

import json
import os
import shutil

from aw2test import looks, paths
from aw2test.editor import Editor
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test
from aw2test.rom import lz10

PAL_BUFFER = 0x030020C0
AW2_CLEAR = 0x080BF8C4
VRAM_TILES = 0x06008000
BIOME = 0x03004493
WASTELAND_CLEAR = looks.look_rom(looks.WASTELAND) + looks.AT_CLEAR


wasteland_map = looks.sample_map


@test()
def wasteland_colours(ctx):
    g = ctx.start(wasteland_map(ctx), ["andy", "andy"])
    e = g.e
    ctx.shot(g, "wasteland")
    lines = e.read(PAL_BUFFER, 128)
    if ctx.mode == "ds":
        ctx.eq(e.u8(BIOME) >> 4 & 7, 1, "the biome is Wasteland")
        data = looks.Look(e, looks.WASTELAND)
        ctx.eq(lines, data.clear[:128], "BG palettes 0-3 are Wasteland's")
        ctx.check(lines != e.read(AW2_CLEAR, 128), "and not AW2's")
        looks.check_screen(ctx, g, looks.WASTELAND)
        # Animation: the sea and river move on (every frame checked on screen
        # is one of the look's), cell by cell at several frames.
        seen = set()
        for i in range(12):
            seen.add((e.read(VRAM_TILES + 0x2000, 0x2000), e.read(VRAM_TILES + 0x4000, 0xC00)))
            if i % 4 == 3:
                looks.check_screen(ctx, g, looks.WASTELAND, f"frame {e.frame}: ")
            e.wait(10)
        ctx.check(len({s for s, _ in seen}) >= 2 and len({r for _, r in seen}) >= 2, "the sea and river are animated")
    else:
        ctx.eq(lines, e.read(AW2_CLEAR, 128), "BG palettes 0-3 are AW2's")
    # Same rules: a tank still attacks a tank for the chart's base damage.
    ctx.attack(g, (6, 10), (8, 10), (9, 10), expect_base=55)


@test(modes=("ds",))
def ds_look_data(ctx):
    """Each look's converted data against Dual Strike's own drawing."""
    g = ctx.start(wasteland_map(ctx), ["andy", "andy"])
    for look in (looks.WASTELAND, looks.DESERT, looks.SNOW):
        looks.check_data(ctx, g.e, look)


def look_in_weather(ctx, biome, weather, fog=False):
    g = ctx.start(wasteland_map(ctx, biome), ["andy", "andy"], weather=weather, fog=fog)
    ctx.shot(g, f"{looks.NAMES[biome].lower()}_{weather}{'_fog' if fog else ''}")
    ctx.eq(g.e.u8(BIOME) >> 4 & 7, biome, "the biome")
    looks.check_screen(ctx, g, biome)
    data = looks.Look(g.e, biome)
    want = {"clear": data.clear, "rain": data.rain, "snow": data.snow, "sandstorm": data.sand}[weather]
    ctx.eq(g.e.read(PAL_BUFFER, 256), want, f"the {weather} colours, fog included")


@test(modes=("ds",))
def wasteland_rain_fog(ctx):
    look_in_weather(ctx, looks.WASTELAND, "rain", fog=True)


@test(modes=("ds",))
def wasteland_snow(ctx):
    look_in_weather(ctx, looks.WASTELAND, "snow")


@test(modes=("ds",))
def wasteland_sandstorm(ctx):
    look_in_weather(ctx, looks.WASTELAND, "sandstorm")


@test(modes=("ds",))
def desert_look(ctx):
    look_in_weather(ctx, looks.DESERT, "clear")


@test(modes=("ds",))
def snow_look(ctx):
    look_in_weather(ctx, looks.SNOW, "snow")


@test(modes=("ds",))
def design_room_looks(ctx):
    """The Design Room's Wasteland switch redraws the map at once with Dual
    Strike's tiles and back with AW2's; cells scrolled in later are the
    look's too."""
    base = os.path.join(ctx.out, "base.sav")
    shutil.copyfile(paths.base_save(), base)
    e = Emu(save=base, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    ed = Editor(e)
    ed.boot()
    w, h = ed.size()
    # Mountains, woods, a sea, a river and a road near the start.
    for kind, cells in ((3, [(4, 3), (5, 3), (4, 4)]), (4, [(7, 3), (8, 3), (7, 4)]),
                        (7, [(10, 2), (11, 2), (10, 3), (11, 3)]), (2, [(3, 6), (3, 7), (3, 8)]),
                        (5, [(5, 6), (6, 6), (7, 6)])):
        for x, y in cells:
            ed.place(kind, None, x, y)
    ed.goto(7, 5)
    e.wait(20)
    rom = open(paths.aw2_rom(), "rb").read()
    aw2_tiles = lz10(rom[looks.AW2_TILES - 0x08000000:][:0x8000])
    ctx.shot(g, "editor_normal")
    ctx.check(e.read(looks.VRAM_TILES, 0x2000) == aw2_tiles[:0x2000], "Normal: AW2's tiles")
    ed.set_wasteland(True)
    e.wait(10)
    ctx.shot(g, "editor_wasteland")
    looks.check_screen(ctx, g, looks.WASTELAND, "editor: ")
    # The map as cells (for a Dual Strike reference picture of the view).
    rows = [e.u16(looks.GMAP + 0x417A + 2 * y) for y in range(h)]
    with open(os.path.join(ctx.out, "editor_map.json"), "w") as f:
        json.dump({"w": w, "h": h, "scroll": [e.s16(looks.GMAP + 4), e.s16(looks.GMAP + 6)],
                   "tiles": [e.u16(looks.GMAP + 0xA22 + 2 * (rows[y] + x)) for y in range(h) for x in range(w)]}, f)
    # Scroll away and back: the cells drawn as they come in are the look's.
    ed.goto(min(w - 1, 25), min(h - 1, 15))
    ctx.shot(g, "editor_wasteland_far")
    looks.check_screen(ctx, g, looks.WASTELAND, "editor, scrolled away: ")
    ed.goto(7, 5)
    e.wait(20)
    looks.check_screen(ctx, g, looks.WASTELAND, "editor, scrolled back: ")
    ed.set_wasteland(False)
    e.wait(10)
    ctx.shot(g, "editor_normal_again")
    ctx.check(e.read(looks.VRAM_TILES, 0x2000) == aw2_tiles[:0x2000], "Normal again: AW2's tiles")
    ctx.eq(e.read(PAL_BUFFER, 128), e.read(AW2_CLEAR, 128), "Normal again: AW2's colours")
    e.close()
