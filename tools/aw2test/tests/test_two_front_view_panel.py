"""The Front view's window (crate::two_front: "Second front / B Back", AW2's
window on BG2, its lines in priority 0 sprites): it keeps out of the
cursor's way, and no sprite behind it shows through its lines.

0.5.0 drew the window over the top rows of Means to an End's second front,
over its middle Black Crystal (8, 1), and the crystal (a priority 3 sprite)
showed through "Second front": where a sprite of the lines overlaps a sprite
of lower priority, its clear pixels lift that sprite over BG2 (the GBA's OBJ
priority quirk). The window now goes to the bottom while the cursor is in
the top rows, and the game's sprites behind it are taken out of it (rows
outside kept)."""

import os
import struct

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test import twofront as tf
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

PANEL = tf.STATE + 0x18
PANEL_VIEW, PANEL_VIEW_LOW = 2, 5
RECTS = {PANEL_VIEW: (8, 0, 14, 6), PANEL_VIEW_LOW: (8, 14, 14, 6)}   # BG2 cells
LINE_TILES = set(range(0x1F9, 0x20A)) | set(range(0x2D2, 0x2DB)) | set(range(0x2E4, 0x2E8))
SIZES = [[(8, 8), (16, 16), (32, 32), (64, 64)], [(16, 8), (32, 8), (32, 16), (64, 32)],
         [(8, 16), (8, 32), (16, 32), (32, 64)], [(8, 8)] * 4]
MTE_CRYSTALS = [(1, 1), (8, 1), (14, 1)]


def sprites(e):
    oam = e.read(0x07000000, 0x400)
    out = []
    for i in range(128):
        a0, a1, a2 = struct.unpack_from("<3H", oam, 8 * i)
        if a0 & 0x300 == 0x200:
            continue
        w, h = SIZES[a0 >> 14][a1 >> 14]
        x, y = a1 & 0x1FF, a0 & 0xFF
        x, y = x - 512 if x >= 240 else x, y - 256 if y >= 160 else y
        out.append({"i": i, "x": x, "y": y, "w": w, "h": h, "tile": a2 & 0x3FF, "prio": (a2 >> 10) & 3})
    return out


def meets(a, b):
    return a["x"] < b["x"] + b["w"] and b["x"] < a["x"] + a["w"] and a["y"] < b["y"] + b["h"] and b["y"] < a["y"] + a["h"]


@test(modes=("ds",))
def two_front_view_panel(ctx):
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(24))
    d.wait_map()
    e.wait(30)
    ctx.require(tf.look_at_other_front(e, g), "the Front view")
    e.wait(30)
    for (cx, cy) in [(1, 1), (8, 0), (8, 1), (14, 1), (7, 5), (1, 6), (8, 9), (14, 9)]:
        g.goto(cx, cy)
        e.wait(30)
        e.shot(os.path.join(ctx.out, f"view_{cx}_{cy}"))
        panel = e.u8(PANEL)
        ctx.check(panel in RECTS, f"({cx},{cy}): the view's window is up ({panel})")
        if panel not in RECTS:
            continue
        x, y, w, h = RECTS[panel]
        win = {"x": 8 * x, "y": 8 * y, "w": 8 * w, "h": 8 * h}
        vofs, hofs = e.u16(0x0400001A) & 0x1FF, e.u16(0x04000018) & 0x1FF
        cursor = {"x": 16 * cx - hofs, "y": 16 * cy - vofs, "w": 16, "h": 16}
        ctx.check(not meets(cursor, win), f"({cx},{cy}): the window is not over the cursor's cell (panel {panel})")
        if (cx, cy) in MTE_CRYSTALS:
            # (its sprite stands a cell taller)
            crystal = {"x": cursor["x"], "y": cursor["y"] - 16, "w": 16, "h": 32}
            ctx.check(not meets(crystal, win), f"({cx},{cy}): the Black Crystal is not under the window")
        bg2 = e.u16(0x0400000C) & 3
        sp = sprites(e)
        lines = [s for s in sp if s["prio"] == 0 and s["tile"] in LINE_TILES and s["w"] == 8 and s["h"] == 16]
        ctx.check(len(lines) >= 5, f"({cx},{cy}): the window's lines ({len(lines)} sprites)")
        behind = [s for s in sp if s["prio"] > bg2 and any(meets(s, l) for l in lines)]
        ctx.check(not behind, f"({cx},{cy}): no sprite behind the window under its lines "
                              f"({[(s['i'], s['x'], s['y'], hex(s['tile'])) for s in behind]})")


@test(modes=("ds",))
def two_front_help_panel(ctx):
    """The map menu's Front help line ("View the other front.", a window at
    the bottom): no sprite behind it shows through its lines (0.5.0: a
    building's top in Means to an End, a unit in Victory or Death!)."""
    for mission in (8, 24):
        e = Emu(save=paths.base_save(), ds=ctx.ds)
        g = Game(e, ctx.image)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=dc.ORDER.index(mission))
        d.wait_map()
        e.wait(30)
        g.open_map_menu()
        for _ in range(3):
            e.press("DOWN", 4)
            e.wait(6)
        e.wait(60)
        e.shot(os.path.join(ctx.out, f"help_{mission}"))
        ctx.eq(e.u8(PANEL), 1, f"mission {mission}: the help line is up")
        bg2 = e.u16(0x0400000C) & 3
        sp = sprites(e)
        lines = [s for s in sp if s["prio"] == 0 and s["tile"] in LINE_TILES and s["w"] == 8 and s["h"] == 16]
        ctx.check(len(lines) >= 5, f"mission {mission}: the help line's sprites ({len(lines)})")
        behind = [s for s in sp if s["prio"] > bg2 and any(meets(s, l) for l in lines)]
        ctx.check(not behind, f"mission {mission}: no sprite behind the window under its lines "
                              f"({[(s['i'], s['x'], s['y'], hex(s['tile'])) for s in behind]})")
        e.close()


@test(modes=("ds",))
def two_front_title_spares_game_tiles(ctx):
    """The second front's "Second front" title (OBJ tiles 0x1F9.., also drawn
    from by a capture's 64x64 picture at 0x1CA) is left out while a picture
    of the game's uses those tiles: 0.5.0 wrote it over the capture's "20"
    row. Means to an End's second front, its CPU round with a capture."""
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(24))
    d.wait_map()
    e.wait(30)
    d.end_turn()
    shared = titled = 0
    for i in range(1500):
        live = e.u8(tf.LIVE)
        if live == 1:
            sp = sprites(e)
            title = [s for s in sp if s["y"] == 1 and s["w"] == 8 and s["h"] == 16 and s["tile"] in LINE_TILES]
            titled += bool(title)
            game = [s for s in sp if s not in title and s["y"] < 160 and s["y"] + s["h"] > 0 and
                    any(s["tile"] <= t < s["tile"] + (s["w"] // 8) * (s["h"] // 8) for t in LINE_TILES)]
            if game:
                shared += 1
                if title:
                    ctx.check(False, f"step {i}: the title drawn while the game's sprites use its tiles "
                                     f"({[(s['x'], s['y'], s['w'], s['h'], hex(s['tile'])) for s in game]})")
                    e.shot(os.path.join(ctx.out, f"shared_{i}"))
        if tf.player_turn(e) and live == 0 and i > 30:
            break
        if d.scripts_running():
            e.press("A", 4)
        e.wait(12)
    ctx.log(f"{titled} steps with the title, {shared} with the game's sprites on its tiles")
    ctx.check(titled > 10, "the title shown on the second front")
    ctx.check(shared > 0, "a capture (or another picture on those tiles) seen")
