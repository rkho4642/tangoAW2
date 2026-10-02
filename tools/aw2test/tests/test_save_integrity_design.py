"""Save integrity, the Design Room: designs built in the editor and saved
in each of the three slots, saved again from a loaded design (every byte
of the record the same), overwritten, played in Versus after a reboot;
every save writes its own slot (and AW2's profile, which it rewrites with
every slot: byte for byte the same but for its save counter), never another
design. With the Dual Strike pack the design has the Wasteland look, Black
Hole as a fifth army with its inventions, Com Towers of all six owners and
Dual Strike's new units; without it, four armies, Black Hole's inventions
and AW2's units. A full design (every cell a unit's, 50 units an army, five
armies) is loaded and saved through the editor too."""

import os
import shutil

from aw2test import paths, saveimg, saves
from aw2test.editor import Editor, HQ, TOWER
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test
from aw2test.save import DesignMap, write_design_map

DESIGN = {1: 5, 2: 6, 3: 7}
PLAIN, SEA = 0x01, 0x07
TOWER_TILES = [0x1D9, 0x1DA, 0x1DB, 0x1DC, 0x1DD, 0x1B9]
# Inventions (the terrain bar's words): minicannons, a laser, the Black
# Factory, and with the pack the Black Crystal and Black Obelisk.
INVENTIONS = [(0x15, (4, 13)), (0x19, (7, 13)), (0x1D, (21, 4))]
INVENTIONS_DS = [(0x115, (10, 13)), (0x11A, (13, 13))]
INFANTRY, MEGATANK, TANK, RECON, BLACK_BOMB, CARRIER, OOZIUM = 1, 4, 5, 6, 13, 26, 27
# The record's name ("Design Map N" for an unnamed map: the slot's number).
NAME = range(0x4B2, 0x4C2)


def boot(ctx, save):
    e = Emu(save=save, ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g


def record_cells(rec):
    """(tiles, units) of a record: w*h u16 tiles (9 bits), w*h unit bytes."""
    w, h = rec[0], rec[1]
    tiles = [int.from_bytes(rec[2 + 2 * k:4 + 2 * k], "little") & 0x1FF for k in range(w * h)]
    return tiles, list(rec[0x4CB:0x4CB + w * h])


def build(ctx, ed, five):
    """The design, built with the editor's bars. Returns {(x, y): (army,
    type)} of the units placed."""
    w, h = ed.size()
    hqs = {1: (0, 0), 2: (w - 1, h - 1), 3: (w - 1, 0), 4: (0, h - 1), 5: (w // 2, h // 2)}
    armies = [1, 2, 3, 4] + ([5] if five else [])
    units = {}
    if ctx.ds:
        ed.set_wasteland(True)
    for a in armies:
        ed.place(HQ, a, *hqs[a])
    cells = {1: (1, 1), 2: (w - 2, h - 2), 3: (w - 2, 1), 4: (1, h - 2), 5: (w // 2 + 1, h // 2)}
    kinds = {1: OOZIUM if ctx.ds else INFANTRY, 2: MEGATANK if ctx.ds else TANK, 3: INFANTRY, 4: INFANTRY, 5: BLACK_BOMB if ctx.ds else RECON}
    for a in armies:
        ed.place(PLAIN, None, *cells[a])
        ed.place_unit(a, kinds[a], *cells[a])
        units[cells[a]] = (a, kinds[a])
    if ctx.ds:
        ed.place(SEA, None, 20, 15)
        ed.place_unit(3, CARRIER, 20, 15)
        units[(20, 15)] = (3, CARRIER)
        for o in range(6 if five else 5):
            ed.to_entry(TOWER)
            ed.set_army(o)
            ed.pick()
            ed.stamp(3 + 2 * o, 3)
            ctx.eq(ed.tile(3 + 2 * o, 3), TOWER_TILES[o], f"tower of owner {o} placed")
    for word, (x, y) in INVENTIONS + (INVENTIONS_DS if ctx.ds else []):
        ed.place(word, None, x, y)
    return units


def check_record(ctx, rec, units, five, label):
    ctx.require(rec is not None, f"{label}: saved")
    tiles, cells = record_cells(rec)
    w = rec[0]
    for (x, y), (a, t) in units.items():
        b = cells[y * w + x]
        want = (0xE0 | t) if a == 5 else ((a - 1) << 6 | t)
        ctx.eq(b, want, f"{label}: the unit at {(x, y)} (army {a}, type {t})")
    ctx.eq(sum(1 for b in cells if b), len(units), f"{label}: {len(units)} units")
    ctx.eq(rec[0x4C4], 5 if five else 0, f"{label}: the five-army mark")
    if ctx.ds:
        ctx.eq(rec[0x723], 0xB1, f"{label}: the Wasteland look")
        for o in range(6 if five else 5):
            ctx.eq(tiles[3 * w + 3 + 2 * o], TOWER_TILES[o], f"{label}: the tower of owner {o}")


def same_but_name(ctx, a, b, label):
    d = [r for r in saveimg.ranges(a, b or b"") if not (r[0] in NAME and r[1] - 1 in NAME)]
    ctx.check(b is not None and not d, f"{label} byte for byte but its default name (differs at {d[:6]})")


def editor_save(ctx, e, ed, slot, before, label, changed=True):
    """Saves to `slot` and checks that only that design slot (and the
    profile's save counter) changed. Returns the new image."""
    ed.save(slot)
    after = saves.flash(e, os.path.join(ctx.out, label))
    saves.expect_slots(ctx, before, after, [DESIGN[slot]] if changed else [], f"{label}: Save to slot {slot}",
                       profile_allow=[saves.MODE_BYTE])
    return after


def run(ctx, five):
    base = os.path.join(ctx.out, "base.sav")
    shutil.copyfile(paths.base_save(), base)
    e, g = boot(ctx, base)
    ed = Editor(e)
    ed.boot()
    img0 = saves.flash(e, os.path.join(ctx.out, "start"))
    units = build(ctx, ed, five)
    ctx.shot(g, "built")
    img1 = editor_save(ctx, e, ed, 1, img0, "slot1")
    rec1 = img1.slot(DESIGN[1])
    check_record(ctx, rec1, units, five, "slot 1")
    # The same map in slot 2: the same record, byte for byte.
    img2 = editor_save(ctx, e, ed, 2, img1, "slot2")
    same_but_name(ctx, rec1, img2.slot(DESIGN[2]), "slot 2 holds slot 1's record")
    # A change (one more unit), saved in slot 3.
    ed.place(PLAIN, None, 12, 8)
    ed.place_unit(2, INFANTRY, 12, 8)
    img3 = editor_save(ctx, e, ed, 3, img2, "slot3")
    rec3 = img3.slot(DESIGN[3])
    d = saveimg.ranges(rec1, rec3)
    cell = 8 * rec1[0] + 12
    ok = {0x4CB + cell, 2 + 2 * cell, 3 + 2 * cell} | set(range(0x4C3, 0x4CB)) | set(NAME)
    ctx.check(d and all(k in ok for s, t in d for k in range(s, t)),
              f"slot 3 differs from slot 1 only at the cell changed {d[:8]}")
    # Load slot 1, save it over slot 3: slot 3 is slot 1 again.
    ed.load(1)
    img4 = editor_save(ctx, e, ed, 3, img3, "overwrite3")
    same_but_name(ctx, rec1, img4.slot(DESIGN[3]), "slot 1 loaded and saved to slot 3: the record")
    ctx.check(img4.slot(DESIGN[1]) == rec1 and img4.slot(DESIGN[2]) == img2.slot(DESIGN[2]), "slots 1 and 2 untouched")
    e.close()

    # After a reboot: load slot 2 in the editor and save it back.
    e, g = boot(ctx, img4.path)
    ed = Editor(e)
    ed.boot()
    ed.load(2)
    ctx.eq(ed.biome(), 1 if ctx.ds else 0, "after a reboot: the loaded look")
    for (x, y), (a, t) in units.items():
        ctx.eq(ed.unit_at(x, y), (a, t), f"after a reboot: the unit at {(x, y)}")
    before = saves.flash(e, os.path.join(ctx.out, "reboot_start"))
    img5 = editor_save(ctx, e, ed, 2, before, "reboot_slot2", changed=False)
    ctx.check(max(s["gen"] for s in img5.sectors if s and s["tag"] == DESIGN[2]) >
              max(s["gen"] for s in before.sectors if s and s["tag"] == DESIGN[2]), "after a reboot: slot 2 written again")
    d = saveimg.ranges(img4.slot(DESIGN[2]), img5.slot(DESIGN[2]))
    ctx.check(not d, f"after a reboot, slot 2 loaded and saved back: the record byte for byte (differs at {d[:6]})")
    e.close()

    # Played in Versus (design 1) after a reboot: its map, units and look.
    e, g = boot(ctx, img5.path)
    g.setup(None, humans=(1,))
    ctx.eq(g.playst()["map"], 0xB4, "Versus on design 1")
    ctx.eq((e.u8(0x03004493) >> 4) & 7, 1 if ctx.ds else 0, "the battle's look")
    have = {(u["x"], u["y"]): (u["army"], u["type"]) for u in g.units()}
    ctx.eq(have, units, "the battle's units are the design's")
    if ctx.ds:
        for o in range(6 if five else 5):
            ctx.eq(g.terrain_class(3 + 2 * o, 3), TOWER | o << 5, f"the battle's tower of owner {o}")
    ctx.shot(g, "battle")
    if not five:
        # Saved from the map menu with Black Hole's inventions on the map,
        # rebooted, continued: the same battle, the inventions' list too.
        inv = e.read(saves.INVENTIONS, 0x80)
        ctx.check(any(inv[8 * k] or inv[8 * k + 1] for k in range(16)), "the inventions are in the battle's list")
        before = saves.flash(e, os.path.join(ctx.out, "versus_before"))
        saves.suspend(g)
        snap = saves.snapshot(g)
        after = saves.flash(e, os.path.join(ctx.out, "versus_saved"))
        saves.expect_slots(ctx, before, after, [4, 8], "Versus on the design, saved",
                           profile_allow=[saves.c420(saveimg.C420_SUSPEND[4])] + list(saves.OPTIONS) + [saves.MODE_BYTE])
        e.close()
        e, g = boot(ctx, after.path)
        saves.to_select_mode(e)
        saves.versus_continue(g)
        saves.compare_snapshots(ctx, snap, saves.snapshot(g), "the design's battle continued")
        ctx.eq((e.u8(0x03004493) >> 4) & 7, 1 if ctx.ds else 0, "the continued battle's look")
        g.end_turn(human=1)
        ctx.check(not g.battle_over(), "the continued battle goes on")
    return img5


@test()
def save_design_slots_round_trip(ctx):
    """Four armies (with the pack: Wasteland, towers, a Carrier and an Oozium)."""
    run(ctx, five=False)


@test()
def save_design_slots_round_trip_five(ctx):
    """Five armies, Black Hole's units and HQ (and with the pack its tower)."""
    run(ctx, five=True)


def full_design():
    """Every cell has a unit: 50 per army (the most an army may have), the
    rest of the cells a fifth army's... 30 x 20 = 600 cells: five armies x
    50 units = 250 units, the others plain; HQs, cities and towers on a row."""
    m = DesignMap(name="FULL")
    m.terrain(0, 0, "hq", 1).terrain(29, 19, "hq", 2).terrain(29, 0, "hq", 3).terrain(0, 19, "hq", 4)
    m.terrain(15, 0, 0x1B4)
    m.colours = [5, 1, 2, 3, 4]
    k = 0
    kinds = [1, 2, 5, 6, 7, 10, 11, 14, 15, 8]
    for a in range(1, 6):
        for n in range(50):
            x, y = 1 + k % 28, 1 + k // 28
            m.unit(a, kinds[n % len(kinds)], x, y)
            k += 1
    return m


@test()
def save_design_full_record_round_trip(ctx):
    """A design with every army at 50 units (five armies) written into slot
    3, loaded in the editor and saved to slot 1: the same map (the counts
    the game works out itself aside); loaded again and saved to slot 2: the
    same record byte for byte."""
    m = full_design()
    if ctx.ds:
        m.biome = 1
    base = os.path.join(ctx.out, "full.sav")
    m.write(paths.base_save(), base, 3)
    e, g = boot(ctx, base)
    ed = Editor(e)
    ed.boot()
    ed.load(3)
    before = saves.flash(e, os.path.join(ctx.out, "loaded"))
    rec = before.slot(DESIGN[3])
    after = editor_save(ctx, e, ed, 1, before, "full_slot1")
    got = after.slot(DESIGN[1])
    d = [r for r in saveimg.ranges(rec, got) if not set(range(*r)) <= ({0x4C3, 0x4C9, 0x4CA} | set(NAME))]
    ctx.check(not d, f"the full design saved back: tiles, units, colours, look the same (differs at {d[:8]})")
    ctx.log(f"counts written by the game: armies {got[0x4C3]}, properties {got[0x4C9]}, most {got[0x4CA]} "
            f"(the test's record: {rec[0x4C3]}, {rec[0x4C9]}, {rec[0x4CA]})")
    tiles, cells = record_cells(got)
    ctx.eq(sum(1 for b in cells if b), 250, "250 units saved")
    ctx.eq(sum(1 for b in cells if b and b & 0xE0 == 0xE0), 50, "50 of them Black Hole's")
    ctx.eq(got[0x723], 0xB1 if ctx.ds else rec[0x723], "the look byte")
    ed.load(1)
    again = editor_save(ctx, e, ed, 2, after, "full_slot2")
    same_but_name(ctx, got, again.slot(DESIGN[2]), "loaded from slot 1 and saved to slot 2")
