"""The world map under the DS Campaign's story scenes (crate::ds_worldmap,
crate::ds_campaign): every scene Dual Strike plays on the world map (the
prologue and the interlude after Victory or Death!, narration boxes over its
pictures; the party after Crystal Calamity and the ending after Means to an
End, dialogue boxes with faces) leaves the map's layer intact under its
boxes.

A box with a face splits the screen (BG1 is the face below the box's top
row, from an HBlank switch), so the BG1 control register at the end of a
frame is the box's. 0.5.0 took that for "not the map screen" and turned the
map's BG1 back on, whose tilemap block holds the map's last 64 tiles: the
map was scattered tiles above every box. Each sampled frame on the map here
is compared with the map's layer drawn from VRAM at the game's scroll (rows
above the box), and the map's tiles and BG1 are checked."""

import os
import struct

from aw2test import dscampaign as dc
from aw2test import paths, worldmap
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

DISPCNT_SHADOW = 0x030030CC
BG1_ON = 1 << 9
OBJ_ON = 1 << 12
WINDOWS = 0xE000
BG3CNT = 0x0400000E
WORLD_MAP_BG3 = 0x5E0B
BG1CNT_SHADOW = 0x03001FE8
WORLD_MAP_BG1 = 0x1B02
BG3_SCROLL_X = 0x0300200C
BG3_SCROLL_Y = 0x03002000
CREDITS = 0x0203FD17
ROWS = 96                 # above any box
MAX_WRONG = 0.15          # sprites (flags, cursor) are not in the layer; a scroll in progress


def read_bmp(path):
    """rows of (r, g, b) 8-bit from an uncompressed 24/32-bit BMP."""
    b = open(path, "rb").read()
    off = struct.unpack_from("<I", b, 10)[0]
    w, h = struct.unpack_from("<ii", b, 18)
    bpp = struct.unpack_from("<H", b, 28)[0] // 8
    stride = (w * bpp + 3) & ~3
    rows = []
    for y in range(abs(h)):
        sy = abs(h) - 1 - y if h > 0 else y
        r = b[off + sy * stride: off + sy * stride + w * bpp]
        rows.append([(r[bpp * x + 2], r[bpp * x + 1], r[bpp * x]) for x in range(w)])
    return rows


def visible_wrong(e, shot_path):
    """The share of pixels in the rows above a box that are not the map's
    layer as VRAM draws it at the game's scroll."""
    layer = worldmap.from_vram(e.read(0x06000000, 0x10000), e.read(0x05000000, 0x200))
    sx, sy = e.u16(BG3_SCROLL_X) & 0x1FF, e.u16(BG3_SCROLL_Y) & 0xFF
    shown = read_bmp(e.shot(shot_path))
    wrong = n = 0
    for y in range(ROWS):
        for x in range(240):
            ly, lx = sy + y, sx + x
            if ly >= 240 or lx >= 480:
                continue
            want = layer[ly][lx]
            got = tuple(c >> 3 for c in shown[y][x])
            n += 1
            if any(abs(a - b) > 1 for a, b in zip(want, got)):
                wrong += 1
    return wrong / max(n, 1)


def scene(ctx, e, d, done, press):
    """Steps through a scene; on every sampled frame the map's layer is up
    (its BG3, its sprites on: not one of the narration's pictures), checks
    BG1 is held off, the map's tiles are its own and the screen shows the
    layer. Returns (frames on the map, of them with a box)."""
    tiles = None
    on_map = boxed = 0
    worst = (0.0, None)
    for i in range(6000):
        if done(i):
            break
        disp = e.u16(DISPCNT_SHADOW)
        # (Not during a wipe: the windows are on.)
        if i % 2 == 0 and e.u16(BG3CNT) == WORLD_MAP_BG3 and disp & OBJ_ON and not disp & WINDOWS:
            text = d.text_shown()
            if e.u16(BG1CNT_SHADOW) == WORLD_MAP_BG1:
                ctx.check(not disp & BG1_ON, f"frame {i}: the map's BG1 held off ({disp:#06x}, box {bool(text)})")
            t = e.read(0x06008000, 0x6000)
            if tiles is None:
                tiles = t
            elif t != tiles:
                ctx.check(False, f"frame {i}: the map's tiles changed (box {bool(text)})")
                tiles = t
            if on_map % 4 == 0 or (text and boxed % 4 == 0):
                wrong = visible_wrong(e, os.path.join(ctx.out, f"map_{i:04d}"))
                if wrong > worst[0]:
                    worst = (wrong, i)
                ctx.check(wrong <= MAX_WRONG, f"frame {i}: the map shown as its layer ({wrong:.1%} off, box {bool(text)})")
            on_map += 1
            boxed += bool(text)
        if press(i):
            e.press("A", 4)
        e.wait(10)
    ctx.log(f"{on_map} map frames sampled, {boxed} with a box; worst {worst[0]:.1%} (frame {worst[1]})")
    return on_map, boxed


def boot(ctx):
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def after_win(ctx, step, ending=False):
    """The mission at `step` won: for the ending as test_ds_campaign's
    credits test wins Means to an End, else by [`rout`]."""
    e, g, d = boot(ctx)
    data = dc.DsData()
    d.start(step=step)
    if ending:
        d.choose_cos(dc.co_picks(data, dc.ORDER[step]), dc.CO_PREFS)
        d.autoplay(max_days=2)
        e.w8(d.players() + 0x3C + 0x1B, 1)
        for _ in range(600):
            if d.in_battle() and e.u8(0x030033EC) == 1 and not d.scripts_running() and g.idle():
                break
            e.wait(10)
        ctx.require(d.force_win(), "the mission won (test aid)")
    else:
        d.choose_cos(dc.co_picks(data, dc.ORDER[step]))
        d.wait_map()
        e.wait(30)
        ctx.require(rout(e, g, d), "the mission won (test aid)")
    return e, g, d


def rout(e, g, d):
    """The other armies' units gone but one Infantry on 1 HP (of the last
    army: Black Hole; an earlier one can be the player's ally) beside a
    player's direct unit, which destroys it (the game's own rout)."""
    from aw2test import twofront as tf
    enemy = sorted({u["army"] for u in g.units() if u["army"] != 1})
    for m in [u for u in g.units(1) if u["type"] in dc.DIRECT]:
        spot = tf.free_land_next_to(d, m["x"], m["y"])
        if spot and enemy:
            for u in [u for u in g.units() if u["army"] != 1]:
                d.remove_unit(u)
            tf.make_unit(d, enemy[-1], 1, spot[0], spot[1], hp=1)
            e.wait(4)
            d.fire((m["x"], m["y"]), spot)
            return True
    return d.force_win()


def back_on_map(e, d):
    return lambda i: i > 20 and d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and not d.scripts_running()


@test(modes=("ds",))
def ds_story_map_prologue(ctx):
    """The prologue (narration over pictures), then the map."""
    e, g, d = boot(ctx)
    d.start(new=True, pick=False)
    on_map, _ = scene(ctx, e, d, back_on_map(e, d), lambda i: d.scripts_running())
    ctx.check(on_map > 0, "the map sampled")


@test(modes=("ds",))
def ds_story_map_interlude(ctx):
    """After Victory or Death!: its narration, then the map."""
    step = dc.ORDER.index(8)
    e, g, d = after_win(ctx, step)
    on_map, _ = scene(ctx, e, d, back_on_map(e, d), lambda i: d.scripts_running() or not d.in_battle())
    ctx.check(on_map > 0, "the map sampled")


@test(modes=("ds",))
def ds_story_map_party(ctx):
    """After Crystal Calamity: the party, dialogue boxes with faces on the map."""
    step = dc.ORDER.index(18)
    e, g, d = after_win(ctx, step)
    on_map, boxed = scene(ctx, e, d, back_on_map(e, d), lambda i: d.scripts_running() or not d.in_battle())
    ctx.check(boxed >= 10, f"the party's boxes on the map ({boxed} frames)")


@test(modes=("ds",))
def ds_story_map_ending(ctx):
    """After Means to an End: the ending's boxes on the map, up to the credits."""
    step = dc.ORDER.index(24)
    e, g, d = after_win(ctx, step, ending=True)
    on_map, boxed = scene(ctx, e, d, lambda i: e.u8(CREDITS) >= 3,
                          lambda i: i % 2 == 0 and (e.u8(CREDITS) == 0 or not d.world_map_up()))
    ctx.check(boxed >= 10, f"the ending's boxes on the map ({boxed} frames)")
