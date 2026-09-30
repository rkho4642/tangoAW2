"""Com Towers built in the Design Room (Dual Strike pack): the terrain bar's
Tower entry changes army with UP, DOWN and SELECT like the editor's own
properties (neutral, the four armies, Black Hole), shows and places the army
it shows, the tower keeps its owner through saving and loading, and a Versus
battle on the design gives each army its tower (the firepower boost for its
owner, a CPU's too; neutral does nothing). Four- and five-army designs, the
Normal and Wasteland looks.

Screenshots per owner (bar_owner<o>, map_owner<o>, loaded, battle_owner<o>)
go to the test's output folder."""

import os
import shutil

from aw2test import paths, save as savelib
from aw2test.editor import Editor, HQ, TOWER, TERRAIN_ARMY, TOOL
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

PLAIN = 0x01
TOWER_TILES = [0x1D9, 0x1DA, 0x1DB, 0x1DC, 0x1DD, 0x1B9]  # neutral, OS, BM, GE, YC, Black Hole
TOWER_ROW = 3
INFANTRY, TANK = 1, 5
SANDSTORM_STATE = 0x03004493  # the battle's biome in bits 4..6 (crate::wasteland)


def tower_x(owner):
    return 3 + 2 * owner


def design_record(sav):
    """The newest design slot 1 record in a cartridge save."""
    data = open(sav, "rb").read()
    best = None
    for s, tag, gen in savelib.sectors(data):
        if tag == 5 and (best is None or gen > best[1]):
            best = (s, gen)
    if best is None:
        return None
    base = best[0] * savelib.SECTOR + 0x52
    return data[base:base + savelib.RECORD_LEN]


def run(ctx, five, wasteland):
    owners = list(range(6 if five else 5))
    armies = [1, 2, 3, 4] + ([5] if five else [])
    base = os.path.join(ctx.out, "base.sav")
    shutil.copyfile(paths.base_save(), base)
    e = Emu(save=base, ds=True)
    g0 = Game(e, ctx.image)
    ctx.games.append(g0)
    ed = Editor(e)
    ed.boot()
    w, h = ed.size()
    ctx.log(f"editor map {w}x{h}")
    if wasteland:
        ed.set_wasteland(True)
    ctx.eq(ed.biome(), 1 if wasteland else 0, "the editor's look")

    # HQs in the corners (Black Hole's in the middle), an Infantry by each,
    # and a Tank each for armies 1 and 2 side by side, all on plain.
    hqs = {1: (0, 0), 2: (w - 1, h - 1), 3: (w - 1, 0), 4: (0, h - 1), 5: (w // 2, h // 2)}
    for a in armies:
        ed.place(HQ, a, *hqs[a])
    spare = {a: (x + (1 if x < w // 2 else -1), y + (1 if y < h // 2 else -1)) for a, (x, y) in hqs.items()}
    spare[5] = (w // 2 + 1, h // 2)
    tanks = {1: (10, 8), 2: (11, 8)}
    for a in armies:
        ed.place(PLAIN, None, *spare[a])
    for a in (1, 2):
        ed.place(PLAIN, None, *tanks[a])
    for a in armies:
        ed.place_unit(a, INFANTRY, *spare[a])
    for a in (1, 2):
        ed.place_unit(a, TANK, *tanks[a])

    # The towers, one per owner, army chosen on the tower's own entry.
    ed.to_entry(TOWER)
    ed.set_army(0)
    # DOWN from neutral wraps to the last army, SELECT and UP step forward.
    ctx.check(ed.step_army("DOWN"), "DOWN on the tower changes the bar's army")
    ctx.eq(e.u8(TERRAIN_ARMY), 5, "DOWN from neutral wraps to Black Hole")
    ctx.check(ed.step_army("SELECT"), "SELECT on the tower changes the bar's army")
    ctx.eq(e.u8(TERRAIN_ARMY), 0, "SELECT from Black Hole wraps to neutral")
    for o in owners:
        ed.to_entry(TOWER)
        ed.set_army(o, "UP")
        word = TOWER | o << 5
        ctx.eq(ed.highlighted(), (word, TOWER_TILES[o]), f"owner {o}: the bar's tower entry")
        ctx.eq(ed.highlighted_shown(), word, f"owner {o}: the tower as the bar shows it")
        ctx.shot(g0, f"bar_owner{o}")
        ed.pick()
        ctx.eq(e.u8(TOOL), word, f"owner {o}: the picked tool")
        ed.stamp(tower_x(o), TOWER_ROW)
        ctx.eq(ed.tile(tower_x(o), TOWER_ROW), TOWER_TILES[o], f"owner {o}: tile placed")
        ctx.eq(ed.terrain_class(tower_x(o), TOWER_ROW), word, f"owner {o}: class placed")
        e.wait(10)
        ctx.shot(g0, f"map_owner{o}")
    ed.goto(tower_x(owners[-1]) + 3, TOWER_ROW + 2)
    e.wait(20)
    ctx.shot(g0, "map_all")

    # Save to design slot 1 and check the record.
    ed.save(1)
    sav = e.save(os.path.join(ctx.out, "design"))
    e.close()
    rec = design_record(sav)
    ctx.require(rec is not None, "design slot 1 saved")
    rw = rec[0]
    for o in owners:
        k = 2 + 2 * (TOWER_ROW * rw + tower_x(o))
        ctx.eq(int.from_bytes(rec[k:k + 2], "little") & 0x1FF, TOWER_TILES[o], f"owner {o}: saved tile")
    ctx.eq(rec[0x4C4], 5 if five else 0, "the five-army mark")
    # The look: 0xB0 | biome (crate::wasteland); a Normal map may keep 0.
    ctx.eq(rec[0x723] == 0xB1, wasteland, "the saved look is Wasteland")

    # Load it back in a fresh editor.
    e = Emu(save=sav, ds=True)
    g1 = Game(e, ctx.image)
    ctx.games.append(g1)
    ed = Editor(e)
    ed.boot()
    ed.load(1)
    ctx.eq(ed.biome(), 1 if wasteland else 0, "loaded look")
    for o in owners:
        ctx.eq(ed.tile(tower_x(o), TOWER_ROW), TOWER_TILES[o], f"owner {o}: loaded tile")
        ctx.eq(ed.terrain_class(tower_x(o), TOWER_ROW), TOWER | o << 5, f"owner {o}: loaded class")
    ed.goto(tower_x(owners[-1]) + 3, TOWER_ROW + 2)
    e.wait(20)
    ctx.shot(g1, "loaded")
    e.close()

    # Play it: army 1 human, the others CPUs.
    e = Emu(save=sav, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.setup(None, humans=(1,))
    ctx.eq(g.playst()["map"], 0xB4, "Versus map is design map 1")
    ctx.eq((e.u8(SANDSTORM_STATE) >> 4) & 7, 1 if wasteland else 0, "the battle's look")
    for o in owners:
        ctx.eq(g.terrain_class(tower_x(o), TOWER_ROW), TOWER | o << 5, f"owner {o}: the battle's tower")
    for a in armies:
        ctx.eq(ctx.towers(g, a), 1, f"army {a} owns one tower")
    for o in owners:
        g.goto(tower_x(o), TOWER_ROW)
        e.wait(30)
        ctx.shot(g, f"battle_owner{o}")
    # Army 1 attacks army 2's tank: both sides +10% (the calculator counts
    # each army's towers, ctx.side); army 2 is a CPU with its own tower.
    ctx.attack(g, tanks[1], tanks[1], tanks[2], expect_base=55)
    # The CPUs play their turns on the design and hand back to army 1.
    g.end_turn(human=1)
    ctx.check(not g.battle_over(), "the battle goes on after the CPUs' turns")
    ctx.shot(g, "after_cpu")
    for a in armies:
        ctx.log(f"army {a}: towers {ctx.towers(g, a)}, units {len(g.units(a))}")


@test(modes=("ds",))
def design_tower_owners_four_armies(ctx):
    run(ctx, five=False, wasteland=False)


@test(modes=("ds",))
def design_tower_owners_four_armies_wasteland(ctx):
    run(ctx, five=False, wasteland=True)


@test(modes=("ds",))
def design_tower_owners_five_armies(ctx):
    run(ctx, five=True, wasteland=False)


@test(modes=("ds",))
def design_tower_owners_five_armies_wasteland(ctx):
    run(ctx, five=True, wasteland=True)
