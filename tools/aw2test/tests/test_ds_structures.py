"""Black Hole's structures in every DS Campaign mission against Dual Strike's.

Dual Strike builds a battle's structure list (`0x02183B08 + 0x48DC`, ARM9
`0x020DA650`) from each cell's terrain class: 0x15 a minicannon (kind 4),
0x17 a Black Crystal (9), 0x18 a Grand Bolt part (0xB + n), 0x1A a Black
Cannon (3, 3x3), 0x1C a Volcano (2, 4x4), 0x1D a Black Obelisk (0xA, 3x3),
0x1F a 4x4 structure picture (8: the missile pad, the fortress; no hit
points). DS_STRUCTURES below is that list as melonDS showed it at the
start of each mission's battle (kind, top-left x, y).

Here each is the invention AW2's game registered (`0x02028360`, kind in
bits 6..9 of +2, top-left cell): a minicannon kind 4, a Black Crystal a
minicannon on tile 0x192, a Black Obelisk a Black Cannon (kind 3) with
tile 0x193 in its middle, a Black Cannon kind 3 on AW2's own Black
Cannon tiles, a Volcano kind 2, a Grand Bolt part a minicannon on 0x194,
a 4x4 picture AW2's kind 8."""

from aw2test import dscampaign as dc
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test import paths
from aw2test.harness import test

INVENTIONS = 0x02028360
TILES = dc.MAP + 0xA22
ROWS = dc.MAP + 0x417A

# Dual Strike's battle structure lists (mission index -> (kind, x, y)),
# read in melonDS (scratchpad onyx/dsmission.py).
DS_STRUCTURES = {
    8: [(9, 21, 6)],
    13: [(4, 2, 4), (4, 2, 5), (4, 2, 6)],
    14: [(4, 16, 4), (4, 21, 4), (8, 17, 5), (4, 16, 9), (4, 21, 9)],
    16: [(4, 4, 2)],
    17: [(9, 5, 4), (9, 11, 4), (9, 8, 7), (9, 4, 9), (9, 12, 9), (9, 8, 11)],
    18: [(3, 9, 1), (0xA, 9, 10)],
    19: [(4, 15, 2), (4, 15, 4), (4, 15, 12), (4, 15, 14)],
    21: [(2, 8, 8)],
    22: [(8, 0, 0), (8, 17, 0), (8, 0, 14), (8, 17, 14)],
    23: [(0xA, 7, 1), (4, 3, 12), (4, 13, 12), (9, 3, 14), (9, 8, 14), (9, 13, 14), (4, 3, 16), (4, 13, 16)],
    24: [(0xB, 3, 9), (0xD, 15, 9), (0xC, 9, 11)],
}


def tile(e, x, y):
    return e.u16(TILES + 2 * (e.u16(ROWS + 2 * y) + x))


def ours(e):
    """AW2's invention list as Dual Strike's kinds."""
    out = []
    for k in range(16):
        a = INVENTIONS + 8 * k
        kind = (e.u16(a + 2) >> 6) & 0xF
        if kind == 0:
            break
        x, y = e.u8(a), e.u8(a + 1)
        if kind == 4:
            t = tile(e, x, y)
            ds = 9 if t == 0x192 else 0xB if t == 0x194 else 4
        elif kind == 3:
            ds = 0xA if tile(e, x + 1, y + 1) == 0x193 else 3
        else:
            ds = kind
        out.append((ds, x, y))
    return out


def _mission(step):
    index = dc.ORDER[step]

    def fn(ctx):
        """The mission's Black Hole structures are Dual Strike's: the same
        kinds at the same cells (a Black Cannon stays a Black Cannon, an
        Obelisk only where Dual Strike has one)."""
        e = Emu(save=paths.base_save(), ds=ctx.ds)
        g = Game(e, ctx.image)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=step)
        d.wait_map()
        have = ours(e)
        want = DS_STRUCTURES.get(index, [])
        # (the Grand Bolt's three parts all read as kind 0xB here)
        norm = lambda l: sorted((0xB if k in (0xB, 0xC, 0xD) else k, x, y) for k, x, y in l)
        ctx.eq(norm(have), norm(want), f"{dc.DsData().mission(index)['name']}: the structures (kind, x, y)")

    fn.__name__ = f"ds_structures_{step:02d}"
    test(modes=("ds",))(fn)


for _s in range(len(dc.ORDER)):
    _mission(_s)


@test(modes=("ds",))
def ds_crystal_calamity_black_cannon(ctx):
    """Crystal Calamity: the structure at the top is Dual Strike's Black
    Cannon facing down (99 HP, 5 HP a shot, every day: its battle entry
    0901db0463010132, the same bytes as AW2's), not a second Obelisk; it
    fires on a player unit in front of it on Black Hole's turn. The centre
    holds the one Black Obelisk."""
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(18))
    d.wait_map()
    d.wait_control()
    ctx.eq(e.read(INVENTIONS, 8).hex(), "0901db0463010132", "the Black Cannon's entry is Dual Strike's")
    ctx.eq(tile(e, 10, 2), 0x187, "its middle is AW2's Black Cannon tile")
    ctx.eq(sum(1 for k, _, _ in ours(e) if k == 0xA), 1, "one Black Obelisk")
    u = [u for u in g.units(1)][0]
    if g.unit_at(10, 7):
        d.remove_unit(g.unit_at(10, 7))
    d.place_unit(u, 10, 7)
    # (no enemy unit near enough to reach it: the shot alone hurts it)
    for o in g.units():
        if o["army"] != 1 and abs(o["x"] - 10) + abs(o["y"] - 7) <= 12:
            d.remove_unit(o)
    e.wait(5)
    hp = g.unit(u["id"])["hp"]
    shot_seen = False
    for _ in range(4):
        d.end_turn()
        e.wait(600)
        d.wait_control(60000)
        e.wait(400)
        d.wait_control(60000)
        now = g.unit(u["id"])
        if now["hp"] != hp:
            shot_seen = now["hp"] == hp - 50
            break
    ctx.check(shot_seen, f"the cannon's shot took 5 HP off the unit in front of it ({hp} -> {g.unit(u['id'])['hp']})")


@test(modes=("ds",))
def ds_surrounded_fortresses(ctx):
    """Surrounded!: two missile pads and two fortresses (Dual Strike's 4x4
    pictures, tiles 0x1AA..0x1AD / 0x1AE..0x1B1), each drawn with its own
    picture though the map header names one (crate::obelisk::second_picture:
    the fortress in OBJ tiles 0xC4..0x103, a 64x64 sprite from them)."""
    from aw2test import rom as romlib
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(22))
    d.wait_map()
    d.wait_control()
    g.goto(19, 1)
    e.wait(30)
    rom = e.read(0x080D38AC, 0x1000)
    fortress = romlib.lz10(rom)
    ctx.eq(e.read(0x06010000 + 0xC4 * 32, len(fortress)) == fortress, True, "the fortress picture is in OBJ tiles 0xC4..")
    oam = e.read(0x07000000, 0x400)
    tiles = [int.from_bytes(oam[8 * k + 4:8 * k + 6], "little") & 0x3FF for k in range(128)]
    ctx.check(0xC4 in tiles, "a sprite draws it (the fortress at (17, 0))")
    pad = romlib.lz10(e.read(0x080D2DA8, 0x1000))
    ctx.eq(e.read(0x06010000 + 0x130 * 32, len(pad)) == pad, True, "the header's missile pad stays in its own tiles")
    shot = e.shot(ctx.out + "/top_right")
