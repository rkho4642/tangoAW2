"""The Versus Teams screen's CO faces, in two- and five-army games, opened
straight from the title and after Survival and the Campaign box (with the
pack: its DS CAMPAIGN sub-menu), and the five-army battle's Com Tower
picture.

Each column's face is 12 sprites (32x8 and 16x8, six rows) in its own 36 OBJ
tiles (400 + 36 * column) and OBJ palette; every column's tiles are checked
against the face its CO has in the ROM image (the presentation table,
`+0x0C`, LZ77; tangoAW2's copy with the pack, with the new COs' faces), for
every CO of the list in the first column, and in the last one of a
five-army game. Until 0.4.0 a five-army game with the pack drew the Com
Tower's picture over the first column's face (OBJ tiles 410..417, the
Crystal's battle tiles, crate::com_tower::after_sheet), from the moment its
map was picked."""

import os
import struct

from aw2test import survival as sv
from aw2test.dscampaign import DsCampaign
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import test
from aw2test.rom import lz10
from aw2test import paths, ram

OAM = 0x07000000
OBJ_VRAM = 0x06010000
OBJ_PALETTES = 0x05000200
PRESENTATION_POOL = 0x08039B7C  # the presentation table's pointer (moved with the pack)
PRESENTATION_ROW = 0x44
FACE = 0x0C
FACE_TILES = 36
SELECT_MODE_CURSOR = 0x0300591C
VERSUS, CAMPAIGN, SURVIVAL = 3, 4, 6
MAP_TAB = 0x0300596C
TAB_DESIGN, TAB_5P = 8, 9
CRYSTAL_OBJ_TILE = 0x19A        # crate::obelisk
LAB_OBJ_TILE = 0x90             # crate::com_tower
T5_CO_INDEX = 0xA8              # five-army Teams record: each army's index into the CO list
TOWER_TILES = [0x1D9, 0x1DA, 0x1DB, 0x1DC, 0x1DD, 0x1B9]


def face_columns(e):
    """The Teams screen's face sprites by column (left to right): the first
    tile of each column's 6x6 tiles, after checking the sprites' layout."""
    oam = e.read(OAM, 0x400)
    groups = {}
    for i in range(128):
        a0, a1, a2 = struct.unpack_from("<HHH", oam, 8 * i)
        if (a0 >> 8) & 3 == 2 or (a2 >> 10) & 3 != 3 or a0 >> 14 != 1:
            continue
        size = a1 >> 14
        if size not in (0, 1):  # 16x8, 32x8
            continue
        groups.setdefault(a2 >> 12, []).append((a0 & 0xFF, a1 & 0x1FF, size, a2 & 0x3FF))
    cols = []
    for pal, sps in groups.items():
        sps.sort()
        base = sps[0][3]
        want = []
        for row in range(6):
            want.append((sps[0][0] + 8 * row, sps[0][1], 1, base + 6 * row))
            want.append((sps[0][0] + 8 * row, sps[0][1] + 32, 0, base + 6 * row + 4))
        cols.append((sps[0][1], pal, base, sorted(sps) == sorted(want)))
    cols.sort()
    return cols


class Faces:
    """CO faces from the ROM image the console runs (pack patches included)."""

    def __init__(self, e):
        self.e = e
        self.table = e.u32(PRESENTATION_POOL)
        self.cache = {}

    def face(self, co):
        if co not in self.cache:
            p = self.e.u32(self.table + PRESENTATION_ROW * co + FACE)
            self.cache[co] = lz10(self.e.read(p, 0x700)) if 0x08000000 <= p < 0x0A000000 else None
        return self.cache[co]


def column_cos(g):
    """The CO each column shows: the Teams record's CO list and indices (with
    five armies the moved record's, crate::five: the indices at +0xA8)."""
    t = g.teams()
    if g.e.u8(ram.FIVE_ON) == 1:
        return [t["co_list"][i] for i in g.e.read(ram.TEAMS_FIVE + T5_CO_INDEX, t["armies"])]
    return [t["co_list"][t["co_index"][a]] for a in range(t["armies"])]


def check_faces(ctx, g, faces, label, only=None):
    e = g.e
    t = g.teams()
    cols = face_columns(e)
    ctx.eq(len(cols), t["armies"], f"{label}: face columns")
    cos = column_cos(g)
    ok = True
    for c, (x, pal, base, layout) in enumerate(cols):
        if only is not None and c not in only:
            continue
        ok &= ctx.check(layout, f"{label}: column {c + 1} face sprites in 6 rows of 32x8 + 16x8")
        tiles = e.read(OBJ_VRAM + 32 * base, 32 * FACE_TILES)
        co = cos[c]
        ok &= ctx.check(tiles == faces.face(co), f"{label}: column {c + 1} (tiles {base}..{base + 35}) is CO {co}'s face")
    return ok


def wheel(e, pos):
    for _ in range(10):
        if e.u8(SELECT_MODE_CURSOR) == pos:
            return
        e.press("UP", 8)
        e.wait(60)
    raise NavError(f"Select Mode cursor stuck at {e.u8(SELECT_MODE_CURSOR)}")


def detours(ctx, g):
    """From the title: Survival's SELECT MAP and back (pack only), then the
    Campaign box (with the pack: DS CAMPAIGN's Continue / New) and back."""
    e = g.e
    sv.to_select_mode(e)
    if ctx.ds:
        wheel(e, SURVIVAL)
        e.press("A", 8)
        ctx.require(e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10), "Survival's SELECT MAP")
        e.wait(90)
        e.press("B", 8)   # leaves Survival: the War Room's box is open
        e.wait(200)
        e.press("B", 8)
        e.wait(60)
    c = DsCampaign(g)
    wheel(e, CAMPAIGN)
    e.press("A", 8)
    ctx.require(e.wait_until(c.box_open, 120, step=4), "the Campaign box")
    e.wait(20)
    if ctx.ds:
        c.chooser_row(1)
        e.press("A", 8)
        e.wait(30)
        ctx.eq(e.u8(0x0203FD13), 2, "in the DS CAMPAIGN box")
        e.press("B", 8)
        e.wait(30)
    e.press("B", 8)
    e.wait(60)


def to_teams(ctx, g, tab, detour):
    """Versus -> New -> the tab's first map -> Teams."""
    e = g.e
    if detour:
        detours(ctx, g)
    else:
        sv.to_select_mode(e)
    wheel(e, VERSUS)
    e.press("A", 8)
    e.wait(150)
    e.press("A", 8)
    e.wait(150)
    if not g.press_until("LEFT", lambda: e.u8(MAP_TAB) == tab, tries=14, hold=8, settle=50):
        raise NavError(f"tab {tab} not reached ({e.u8(MAP_TAB)})")
    e.wait(30)
    e.press("A", 8)
    ctx.require(e.wait_until(g.on_teams, 400, step=10), "the Teams screen")
    e.wait(60)


def boot(ctx, m=None):
    save = paths.base_save()
    if m is not None:
        save = os.path.join(ctx.out, "map.sav")
        m.write(paths.base_save(), save)
    e = Emu(save=save, ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g


def cycle(ctx, g, faces, column, label):
    """Every CO of the list in `column` (from the first column's CO stop), each face checked."""
    e = g.e
    for _ in range(column):
        e.press("RIGHT", 6)
        e.wait(14)
        e.press("RIGHT", 6)
        e.wait(14)
    n = len(g.teams()["co_list"])
    bad = 0
    for k in range(n):
        e.press("DOWN", 6)
        e.wait(20)
        if not check_faces(ctx, g, faces, f"{label}, column {column + 1} CO {k + 1}/{n}", only=(column,)):
            bad += 1
            if bad == 1:
                ctx.shot(g, f"{label}_col{column + 1}_bad")
    ctx.eq(bad, 0, f"{label}: column {column + 1}: faces wrong over its {n} COs")
    for _ in range(column):
        e.press("LEFT", 6)
        e.wait(14)
        e.press("LEFT", 6)
        e.wait(14)


def run(ctx, armies, detour):
    label = f"{armies} armies" + (", after Survival and Campaign" if detour else "")
    if armies == 2:
        e, g = boot(ctx, ctx.map())
        to_teams(ctx, g, TAB_DESIGN, detour)
    else:
        e, g = boot(ctx)
        to_teams(ctx, g, TAB_5P, detour)
    ctx.eq(g.teams()["armies"], armies, "armies on the Teams screen")
    faces = Faces(e)
    name = f"teams{armies}" + ("_detour" if detour else "")
    ctx.shot(g, name)
    check_faces(ctx, g, faces, label)
    e.wait(120)  # every frame: nothing draws over the faces later
    check_faces(ctx, g, faces, label + ", two seconds on")
    cycle(ctx, g, faces, 0, label)
    if armies == 5:
        cycle(ctx, g, faces, 4, label)
    check_faces(ctx, g, faces, label + ", after cycling")
    ctx.shot(g, name + "_end")


@test()
def teams_faces_two_armies(ctx):
    run(ctx, 2, detour=False)


@test()
def teams_faces_five_armies(ctx):
    run(ctx, 5, detour=False)


@test()
def teams_faces_two_armies_after_survival_and_campaign(ctx):
    run(ctx, 2, detour=True)


@test()
def teams_faces_five_armies_after_survival_and_campaign(ctx):
    run(ctx, 5, detour=True)


def tower_map(ctx, five):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    if five:
        m.terrain(15, 10, 0x1B4)  # Black Hole's HQ
        m.unit(5, "infantry", 16, 10)
        m.colours = [5, 1, 2, 3, 4]  # the five-army mark
    m.terrain(6, 3, TOWER_TILES[1])
    return m


@test(modes=("ds",))
def five_army_com_tower_picture(ctx):
    """The battle side of the fix: in a five-army battle the Com Tower is still
    drawn from the Crystal's tiles (no Crystal on the map), with the picture a
    four-army battle has in the building sheet's Lab tiles."""
    e, g = boot(ctx, tower_map(ctx, five=False))
    g.setup(None, humans=(1,))
    e.wait(30)
    lab = e.read(OBJ_VRAM + 32 * LAB_OBJ_TILE, 256)
    e.close()
    e, g = boot(ctx, tower_map(ctx, five=True))
    g.setup(None, humans=(1,))
    ctx.eq(e.u8(ram.FIVE_ON), 1, "a five-army battle")
    e.wait(30)
    ctx.check(e.read(OBJ_VRAM + 32 * CRYSTAL_OBJ_TILE, 256) == lab, "the tower's picture in the Crystal's tiles")
    g.goto(6, 3)
    e.wait(30)
    ctx.shot(g, "five_tower")
    g.end_turn(human=1)
    ctx.check(e.read(OBJ_VRAM + 32 * CRYSTAL_OBJ_TILE, 256) == lab, "... and after the computers' turns")
    ctx.shot(g, "five_tower_after")
