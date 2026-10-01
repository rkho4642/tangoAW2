"""No battle background leaks into the next battle (crate::ds_backdrop).

Orange Star against Black Hole (Andy and Von Bolt in a two-army map, and
Black Hole in a five-army map), normal and Wasteland looks, pack on and off:
in one turn the player fights Black Hole's tank on a Com Tower (a Lab without
the pack), then Black Hole's Piperunner on its pipe (a tank on a plain without
the pack), then Black Hole's tank in a wood. Each side of each scene is
checked against the background it should show, read from the ROMs:

- an AW2 background: the side's three BG palettes (palette RAM) are exactly
  one of AW2's backgrounds' (table 0x08555850, its set for the weather) and
  the side's tiles in VRAM are that background's, byte for byte. A wood is
  one of AW2's two woods (`sub_0804B55C` picks 4 or 0x2D at random, so a
  Com Tower battle before it, whose city pick also draws a random number,
  can change which one: both are AW2's own);
- Dual Strike's (the Piperunner, a Com Tower or anything on a Wasteland map):
  only Dual Strike's colours for that terrain (test_battle_backgrounds), and
  no AW2 background left on that side.

Whatever an earlier scene loaded (Dual Strike's city or pipe), the wood is
an exact AW2 or Dual Strike wood, and the other side an exact plain.
"""

import importlib.util
import os
import struct

from aw2test import rom as romlib
from aw2test.harness import test

_spec = importlib.util.spec_from_file_location(
    "battle_backgrounds", os.path.join(os.path.dirname(__file__), "test_battle_backgrounds.py"))
bb = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(bb)

PAL = 0x05000000
BG_TABLE = 0x08555850  # 0x18 each: +4 LZ77 tiles, +8 map, +0x0C palettes (0x60) by weather
BG_COUNT = 0x34
WOODS = {4, 0x2D}
_aw2 = {}


def aw2_backgrounds(image):
    """AW2's battle backgrounds: id -> (tiles, [palettes by weather])."""
    if not _aw2:
        for k in range(BG_COUNT):
            e = image.at(BG_TABLE + 0x18 * k, 0x18)
            tiles_at = struct.unpack_from("<I", e, 4)[0]
            pals = [image.at(struct.unpack_from("<I", e, 0x0C + 4 * w)[0], 0x60) for w in range(3)]
            _aw2[k] = (romlib.lz10(image.at(tiles_at, 0x10000)), pals)
    return _aw2


def aw2_id(ctx, g, side, weather=0):
    """Which AW2 background the side shows exactly (palettes and tiles), or None."""
    pals = g.e.read(PAL + 32 * bb.FIRST_PAL[side], 0x60)
    tiles_at = bb.SIDE_VRAM[side][0]
    for k, (tiles, sets) in aw2_backgrounds(ctx.image).items():
        if sets[weather] == pals and g.e.read(tiles_at, len(tiles)) == tiles:
            return k
    return None


FACE_PAL = (0x050000E0, 0x05000100)  # the sides' CO panel faces (sub_08057A80)
VON_BOLT = 11  # Dual Strike's id


def ds_co_palette(ds_id):
    """A Dual Strike CO's first colour scheme (appearance record +0x50)."""
    rec = bb.ds().a9(0x02152B8C + 0x54 * (ds_id - 1), 0x54)
    return bb.ds().a9(struct.unpack_from("<I", rec, 0x50)[0], 32)


def scene(ctx, g, name):
    seen = bb.sample(ctx, g, name)
    if seen is not None:
        seen["aw2"] = [aw2_id(ctx, g, s) for s in (0, 1)]
        seen["face"] = [g.e.read(a, 32) for a in FACE_PAL]
    return seen


def setup(ctx, five, biome):
    if five:
        m, cos, bh = bb.five_army_map(ctx)
        tower = 0x1B9  # Black Hole's Com Tower
    else:
        m = ctx.map()
        m.colours[1], m.colours[2] = 1, 5
        cos, bh = ["andy", "vonbolt" if ctx.ds else "sturm"], 2
        tower = bb.LAB_TILE  # a neutral one
    m.biome = biome
    m.terrain(10, 4, "plain").terrain(11, 4, tower)
    m.terrain(10, 8, "plain")
    if ctx.ds:
        m.terrain(11, 8, "pipe").terrain(12, 8, "pipe").terrain(13, 8, "pipe")
    m.terrain(10, 12, "plain").terrain(11, 12, "wood")
    m.unit(1, "tank", 10, 4).unit(bh, "tank", 11, 4)
    m.unit(1, "artillery", 10, 8).unit(bh, "piperunner" if ctx.ds else "tank", 12, 8)
    m.unit(1, "tank", 10, 12).unit(bh, "tank", 11, 12)
    return ctx.start(m, cos, visuals="a")


def fight(ctx, g, src, target, name):
    bb.fire(g, src, target)
    seen = scene(ctx, g, name)
    g.wait_for_input()
    return seen


def check_side(ctx, seen, side, label, terrain, ds_bg, biome, pipe=False, woods=False):
    if seen is None:
        return
    if ds_bg:
        colours, ground = bb.expected(terrain, bb.WASTELAND if biome else 0, pipe=pipe)
        got = seen["colours"][side]
        ctx.check(len(got) >= 20 and got <= colours,
                  f"{label}: side {side}'s background is Dual Strike's ({len(got)} colours, "
                  f"{len(got - colours)} not Dual Strike's)")
        # A Piperunner's pipe takes palette room from the ground (its colours weigh more).
        ctx.check(len(got & ground) >= (6 if pipe else 8), f"{label}: with its ground's colours ({len(got & ground)})")
        ctx.check(seen["aw2"][side] is None, f"{label}: no AW2 background left on side {side}")
    else:
        k = seen["aw2"][side]
        ctx.check(k is not None, f"{label}: side {side} is exactly one of AW2's backgrounds (id {k})")
        if woods:
            ctx.check(k in WOODS, f"{label}: AW2's wood (id {k})")


def run(ctx, label, five, biome):
    ds_look = bool(ctx.ds and biome)
    tag = f"{'five' if five else 'two'}_{'wl' if biome else 'normal'}"
    g = setup(ctx, five, biome)
    tower = fight(ctx, g, (10, 4), (11, 4), f"{tag}_1_tower")
    pipe = fight(ctx, g, (10, 8), (12, 8), f"{tag}_2_pipe")
    wood = fight(ctx, g, (10, 12), (11, 12), f"{tag}_3_wood")
    g.e.close()
    ctx.check(None not in (tower, pipe, wood), f"{label}: all three battle scenes ran")
    check_side(ctx, tower, 0, f"{label}, Com Tower battle: Orange Star's plain", "plain", ds_look, biome)
    check_side(ctx, tower, 1, f"{label}, Com Tower battle: Black Hole's tower", "lab", ds_look, biome)
    check_side(ctx, pipe, 0, f"{label}, pipe battle: Orange Star's plain", "plain", ds_look, biome)
    check_side(ctx, pipe, 1, f"{label}, pipe battle: Black Hole's {'Piperunner' if ctx.ds else 'tank'}",
               "pipe" if ctx.ds else "plain", ctx.ds, biome, pipe=ctx.ds)
    check_side(ctx, wood, 0, f"{label}, wood battle after them: Orange Star's plain", "plain", ds_look, biome)
    check_side(ctx, wood, 1, f"{label}, wood battle after them: Black Hole's wood", "wood", ds_look, biome,
               woods=True)
    if ctx.ds and not five and wood:
        # Von Bolt's panel face is his own mini portrait in his own colours:
        # Dual Strike draws him in a mint-green glass dome (not a stray tile).
        ctx.check(wood["face"][1][2:] == ds_co_palette(VON_BOLT)[2:],
                  f"{label}: Von Bolt's panel face in Dual Strike's own colours for him")


@test(modes=("ds", "aw2"))
def battle_bg_no_leak_two_armies(ctx):
    """Andy against Von Bolt (Sturm without the pack): Com Tower, pipe, then wood."""
    run(ctx, "Andy vs Von Bolt", False, 0)


@test(modes=("ds",))
def battle_bg_no_leak_two_armies_wasteland(ctx):
    """Andy against Von Bolt on a Wasteland map."""
    run(ctx, "Andy vs Von Bolt, Wasteland", False, 1)


@test(modes=("ds",))
def battle_bg_no_leak_five_armies(ctx):
    """Orange Star against Black Hole in a five-army map, both looks."""
    run(ctx, "five armies", True, 0)
    run(ctx, "five armies, Wasteland", True, 1)
