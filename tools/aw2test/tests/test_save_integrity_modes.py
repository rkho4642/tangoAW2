"""Save integrity, the War Room and Survival: a War Room map won keeps its
score in the map's own row of AW2's War Room table (and the map's played
bit, and the points a win earns), nothing else; Dual Strike's map ids are
never listed there (the table has 30 rows, ids 0x6C..0x89). A Survival map
won writes the profile with nothing of the War Room's (its rows are
tangoAW2's own while Survival is on), a cleared run its record, in the
profile's spare bytes; nothing else changes, in RAM (AW2's score tables,
the event slots past them) or in Flash. A War Room map saved halfway and
continued, and kept through Survival. Every mode in one boot, then rebooted;
every slot in use at once."""

import os

from aw2test import campaigns as cp
from aw2test import dscampaign as dc
from aw2test import paths, saveimg, saves
from aw2test import survival as sv
from aw2test.editor import Editor, HQ, STATE as EDITOR_STATE
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import test
from aw2test.save import DesignMap, write_design_map

PLAYERS = 0x020232C0  # army 1's block
P_FUNDS, P_YIELD = 0x00, 0x31
WAR_ROOM_ROWS = (0x48, 0x2A0)       # the profile's War Room table: 30 maps x 0x14
PLAYED_BITS = (0x30, 0x48)          # the profile's played bits (unlocks +0x30)
POINTS = (saves.c420(0x00, 4), saves.c420(0x04, 4))
SURVIVAL_RECORDS = saves.c420(0x15, 10)
# RAM AW2's score code writes: the War Room table, the campaign's and the
# options block; then the event script slots (10 x 0x18 from 0x0200C528),
# where a score row past the War Room's table would land (as a DS
# mission's id once did, crate::ds_campaign::best_score).
SCORE_RAM = (0x0200C078, 0x0200C528)
EVENT_SLOTS = 0x0200C510


def event_slots_idle(e):
    """No event script left in a slot (each slot's script pointer, +0)."""
    b = e.read(EVENT_SLOTS, 0x18 * 11)
    return [hex(EVENT_SLOTS + 0x18 * k) for k in range(11) if int.from_bytes(b[0x18 * k:0x18 * k + 4], "little")]


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g


def player(army):
    return PLAYERS + 0x3C * (army - 1)


def war_room_open(e):
    saves.wheel_to(e, saves.WAR_ROOM)
    e.press("A", 8)
    e.wait(60)
    saves.box_row(e, 1)
    e.press("A", 8)
    if not e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10):
        raise NavError("the War Room's SELECT MAP did not open")
    e.wait(90)


def pick_and_play(e, g, row=0):
    """A map from SELECT MAP, its CO screen, the battle with the pad."""
    for _ in range(row):
        e.press("DOWN", 8)
        e.wait(30)
    e.press("A", 8)
    for _ in range(80):
        if e.u32(0x03000000) == 0x08022049:
            break
        if sv.running(e, sv.CO_SCREEN_PROC):
            e.wait(60)
            e.press("A", 8)
            e.wait(100)
            e.press("A", 8)
        e.wait(20)
    if not e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1500, step=20):
        raise NavError("the battle did not start")
    g._units_base = g._players_base = None
    g.wait_for_input()


def win(e, g, armies):
    """Every computer army yields; ending the turn wins the map; then the
    results and the War Room's save prompt (Yes) to SELECT MAP."""
    for a in range(2, armies + 1):
        e.w8(player(a) + P_YIELD, 1)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    if not sv.to_select_map(e):
        raise NavError("SELECT MAP did not come back")


def armies(e):
    return sum(1 for a in range(1, 5) if e.u8(player(a) + 0x1B))


@test()
def save_war_room_score_saved(ctx):
    """A War Room map won: its row of the War Room table, its played bit and
    the points; after a reboot the War Room has the score. With the pack the
    War Room lists AW2's maps only."""
    e, g = boot(ctx)
    saves.to_select_mode(e)
    war_room_open(e)
    ids = list(e.read(saves.LIST_IDS, e.u8(saves.LIST_LAST) + 1))
    ctx.check(ids and all(0x6C <= i <= 0x89 for i in ids), f"the War Room lists its own maps only ({[hex(i) for i in ids]})")
    before = saves.flash(e, os.path.join(ctx.out, "before"))
    ram0 = e.read(*SCORE_RAM[:1], SCORE_RAM[1] - SCORE_RAM[0])
    pick_and_play(e, g)
    mid = e.u8(0x03003FC2)
    ctx.check(0x6C <= mid <= 0x89, f"a War Room map ({mid:#x})")
    row = mid - 0x6C
    win(e, g, armies(e))
    after = saves.flash(e, os.path.join(ctx.out, "after"))
    lo = WAR_ROOM_ROWS[0] + 0x14 * row
    saves.expect_slots(ctx, before, after, [], "the War Room's save",
                       profile_allow=[(lo, lo + 0x14), PLAYED_BITS] + list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.check(after.slot(0)[lo:lo + 0x14] != before.slot(0)[lo:lo + 0x14], f"map {mid:#x}'s scores saved (row {row})")
    ram1 = e.read(SCORE_RAM[0], SCORE_RAM[1] - SCORE_RAM[0])
    off = 0x14 * row
    bad = [hex(SCORE_RAM[0] + s) for s, t in saveimg.ranges(ram0, ram1)
           if not (off <= s and t <= off + 0x14) and not 0x0200C420 <= SCORE_RAM[0] + s < 0x0200C500]
    ctx.check(not bad, f"AW2's score tables in RAM: only the map's row written (else {bad[:8]})")
    ctx.check(not event_slots_idle(e), f"no event script slot left running {event_slots_idle(e)}")
    e.close()
    e, g = boot(ctx, after.path)
    saves.to_select_mode(e)
    ctx.eq(e.read(0x0200C078 + off, 0x14), after.slot(0)[lo:lo + 0x14], "after a reboot: the War Room has the score")


@test(modes=("ds",))
def save_survival_saves_nothing_of_the_war_rooms(ctx):
    """Survival (Money): map 1 won, then the run's last map cleared; each
    War Room save prompt writes the profile: the War Room table and played
    bits untouched (in RAM too), the record in the profile's spare bytes,
    nothing else; after a reboot the record is shown."""
    e, g = boot(ctx)
    ctx.require(sv.open_survival(e), "Survival's SELECT MAP")
    before = saves.flash(e, os.path.join(ctx.out, "before"))
    ram0 = e.read(SCORE_RAM[0], SCORE_RAM[1] - SCORE_RAM[0])
    ctx.require(sv.pick(e, sv.LIST_ORDER.index(sv.MONEY)), "Money Survival starts")
    g._units_base = g._players_base = None
    g.wait_for_input()
    n = armies(e)
    win(e, g, n)
    ctx.eq(sv.state(e)["stage"], 1, "map 1 won")
    mid = saves.flash(e, os.path.join(ctx.out, "map1"))
    saves.expect_slots(ctx, before, mid, [], "map 1 won, saved", profile_allow=list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.check(sv.pick(e, 0), "map 2 starts")
    g._units_base = g._players_base = None
    g.wait_for_input()
    e.w8(sv.STAGE, 10)            # the run's last map
    e.w32(player(1) + P_FUNDS, 123400)
    win(e, g, armies(e))
    st = sv.state(e)
    ctx.eq(st["phase"], sv.CLEARED, "the run cleared")
    e.press("A", 8)
    e.wait(30)
    after = saves.flash(e, os.path.join(ctx.out, "cleared"))
    saves.expect_slots(ctx, mid, after, [], "the run cleared, saved",
                       profile_allow=[SURVIVAL_RECORDS] + list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    rec = after.slot(0)[SURVIVAL_RECORDS[0]:SURVIVAL_RECORDS[1]]
    ctx.eq(rec[0], sv.RECORD_MAGIC, "the records' mark")
    ram1 = e.read(SCORE_RAM[0], SCORE_RAM[1] - SCORE_RAM[0])
    bad = [hex(SCORE_RAM[0] + s) for s, t in saveimg.ranges(ram0, ram1)
           if not 0x0200C420 <= SCORE_RAM[0] + s < 0x0200C500]
    ctx.check(not bad, f"AW2's score tables in RAM untouched (else {bad[:8]})")
    ctx.check(not event_slots_idle(e), f"no event script slot left running {event_slots_idle(e)}")
    e.close()
    e, g = boot(ctx, after.path)
    ctx.require(sv.open_survival(e), "Survival after a reboot")
    ctx.eq(sv.records(e).get(sv.MONEY), (5, st["co"], 123400), "the record after a reboot")


# -- every mode in one boot ------------------------------------------------------------------

CAMPAIGN_SCORES = (0x2A0, 0x3F0)
RESULTS = saves.c420(0x38, 0xA8)
WM_PROFILE = (0x4D0, 0x5CC)
SAVE_DIRECTORY = 0x0200CC38      # gUnknown_0200CC38: +0 each sector's tag, +0x20 its load flags


def open_survival_from_wheel(e):
    saves.wheel_to(e, saves.SURVIVAL)
    e.press("A", 8)
    if not e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10):
        raise NavError("Survival's SELECT MAP did not open")
    e.wait(90)


def editor_from_wheel(e):
    saves.wheel_to(e, saves.DESIGN_ROOM)
    e.press("A", 8)
    e.wait(150)
    e.press("A", 8)
    e.wait(150)
    for _ in range(30):
        if e.u8(EDITOR_STATE) == 1:
            break
        e.press("A", 8)
        e.wait(40)
    if e.u8(EDITOR_STATE) != 1:
        raise NavError("the editor did not open")
    e.wait(30)
    return Editor(e)


def leave_editor(e, ed):
    """The editor's menu, End, Yes: Select Mode."""
    ed.close_bar()
    e.press("SELECT", 8)
    if not e.wait_until(lambda: e.u8(EDITOR_STATE) == 3, 90, step=6):
        raise NavError("the editor's menu did not open")
    e.wait(30)
    for _ in range(4):
        e.press("DOWN", 6)
        e.wait(20)
    e.press("A", 8)
    e.wait(60)
    e.press("LEFT", 8)
    e.wait(20)
    e.press("A", 8)
    if not e.wait_until(lambda: saves.wheel(e) is not None, 900, step=10):
        raise NavError("Select Mode did not come back from the editor")
    e.wait(60)


def aw2_load_check(ctx, e, img, label):
    """After a boot: every sector the directory lists passed AW2's own load
    check (sub_0801B018: no read failure, bit 0, nor rejection, bit 2)."""
    b = e.read(SAVE_DIRECTORY, 0x40)
    d = img.directory()
    bad = [i for i in range(16) if d[i] != 0xFF and b[0x20 + i] & 5]
    ctx.check(not bad, f"{label}: AW2's boot accepted every listed sector (rejected: {bad})")
    ctx.eq(list(b[:16]), d, f"{label}: AW2's directory as read at boot")


@test(modes=("ds",))
def save_every_mode_one_boot(ctx):
    """In one boot: the DS Campaign (New, a mission won), Survival (map 1
    won, a run cleared), the Design Room (a design saved in slot 2), AW2's
    campaign (New, Mission 1 won, saved), the War Room (a map won, saved),
    Versus (design 2, saved from the map menu). After each, only that mode's
    slot and bytes changed. Rebooted: every slot passes AW2's check (and its
    own load at boot), only the slots used are there, and each mode has its
    data: Versus continues the battle, DS CAMPAIGN continues its map, AW2
    CAMPAIGN its, Survival shows the record, the War Room has the score,
    the editor loads the design; the options set are kept."""
    e, g = boot(ctx)
    d = dc.DsCampaign(g)
    saves.to_select_mode(e)
    img = [saves.flash(e, os.path.join(ctx.out, "f0_start"))]

    def step(name, changed, allow):
        img.append(saves.flash(e, os.path.join(ctx.out, f"f{len(img)}_{name}")))
        saves.expect_slots(ctx, img[-2], img[-1], changed, name, profile_allow=allow)
        return img[-1]

    # 1. The DS Campaign.
    cp.ds_start(e, d, new=True, from_title=False)
    ds_won = cp.win_ds_mission(e, d)
    cp.leave_map(e, d, close=True)
    f1 = step("ds_campaign", [15], list(POINTS) + [saves.MODE_BYTE])
    # 2. Survival.
    open_survival_from_wheel(e)
    ctx.require(sv.pick(e, sv.LIST_ORDER.index(sv.MONEY)), "Money Survival starts")
    g._units_base = g._players_base = None
    g.wait_for_input()
    win(e, g, armies(e))
    ctx.require(sv.pick(e, 0), "Survival's map 2")
    g._units_base = g._players_base = None
    g.wait_for_input()
    e.w8(sv.STAGE, 10)
    e.w32(player(1) + P_FUNDS, 123400)
    win(e, g, armies(e))
    run = sv.state(e)
    ctx.eq(run["phase"], sv.CLEARED, "the run cleared")
    e.press("A", 8)
    e.wait(30)
    ctx.require(saves.back_to_select_mode(e), "Select Mode after Survival")
    step("survival", [], [SURVIVAL_RECORDS] + list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    # 3. The Design Room: a two-army design in slot 2.
    ed = editor_from_wheel(e)
    w, h = ed.size()
    ed.place(HQ, 1, 0, 0)
    ed.place(HQ, 2, w - 1, h - 1)
    ed.place(0x01, None, 5, 5)
    ed.place(0x01, None, 6, 5)
    ed.place_unit(1, 5, 5, 5)
    ed.place_unit(2, 5, 6, 5)
    ed.save(2)
    leave_editor(e, ed)
    f3 = step("design", [6], [saves.MODE_BYTE])
    # 4. AW2's campaign: New, Mission 1 won, saved.
    cp.aw2_new(e, d, True, from_title=False)
    ctx.require(d.force_win(), "AW2's Mission 1 won")
    cp.through_to_map(e, d)
    aw2_flags = d.map_flags()
    cp.leave_map(e, d, close=True)
    f4 = step("aw2_campaign", [], [PLAYED_BITS, CAMPAIGN_SCORES, RESULTS, WM_PROFILE] + list(POINTS)
              + list(saves.OPTIONS) + [saves.MODE_BYTE, (0, 0x10)])
    # 5. The War Room.
    war_room_open(e)
    pick_and_play(e, g)
    wr = e.u8(0x03003FC2) - 0x6C
    win(e, g, armies(e))
    ctx.require(saves.back_to_select_mode(e), "Select Mode after the War Room")
    lo = WAR_ROOM_ROWS[0] + 0x14 * wr
    f5 = step("war_room", [], [(lo, lo + 0x14), PLAYED_BITS] + list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    # 6. Versus on design 2, saved from the map menu.
    saves.versus_select_map(g, 8, 0xB5)
    saves.versus_start(g)
    g.attack((5, 5), (5, 5), (6, 5))
    saves.suspend(g)
    snap = saves.snapshot(g)
    f6 = step("versus", [4, 8], [saves.c420(saveimg.C420_SUSPEND[4])] + list(saves.OPTIONS) + [saves.MODE_BYTE])
    e.close()

    # Rebooted.
    ctx.eq(f6.tags(), [0, 4, 6, 8, 15], "the slots in use: the profile, the Versus suspend and its map, design 2, the DS record")
    ctx.check(not f6.problems(), f"every slot passes AW2's check {f6.problems()}")
    p = f6.slot(0)
    ctx.check(p[SURVIVAL_RECORDS[0]:SURVIVAL_RECORDS[1]] == img[2].slot(0)[SURVIVAL_RECORDS[0]:SURVIVAL_RECORDS[1]],
              "the Survival records as Survival saved them")
    ctx.check(p[WM_PROFILE[0]:WM_PROFILE[1]] == f4.slot(0)[WM_PROFILE[0]:WM_PROFILE[1]], "AW2's world map as AW2 saved it")
    ctx.check(p[lo:lo + 0x14] == f5.slot(0)[lo:lo + 0x14], "the War Room score as saved")
    ctx.check(f6.slot(15) == f1.slot(15), "the DS record as the DS Campaign saved it")
    ctx.check(f6.slot(6) == f3.slot(6), "design 2 as saved")

    e, g = boot(ctx, f6.path)
    saves.to_select_mode(e)
    aw2_load_check(ctx, e, f6, "after a reboot")
    ctx.eq(e.read(0x0200C420 + 0x0E, 1)[0], p[saveimg.P_C420 + 0x0E], "the animation option as saved")
    ctx.eq(e.read(0x0200C078 + 0x14 * wr, 0x14), p[lo:lo + 0x14], "the War Room's score loaded")
    saves.versus_continue(g)
    saves.compare_snapshots(ctx, snap, saves.snapshot(g), "Versus continued")
    e.close()

    e, g = boot(ctx, f6.path)
    d = dc.DsCampaign(g)
    cp.ds_start(e, d, new=False)
    e.wait(30)
    ctx.eq([m for m in range(28) if d.map_flags()[m] & 2], [ds_won], "DS CAMPAIGN continued: the mission won")
    ctx.shot(g, "ds_map")
    cp.leave_map(e, d, close=True)
    cp.aw2_box(e, d, True, from_title=False)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(d.world_map_up, 1500, step=10), "AW2 CAMPAIGN continued: its world map")
    e.wait(60)
    ctx.eq(d.map_flags()[:4], aw2_flags[:4], "AW2's world map: Mission 1 won")
    ctx.shot(g, "aw2_map")
    e.close()

    e, g = boot(ctx, f6.path)
    ctx.require(sv.open_survival(e), "Survival after a reboot")
    ctx.eq(sv.records(e).get(sv.MONEY), (5, run["co"], 123400), "the Survival record")
    e.close()

    e, g = boot(ctx, f6.path)
    ed = Editor(e)
    ed.boot()
    ed.load(2)
    ctx.eq(ed.unit_at(5, 5), (1, 5), "the editor loads design 2")


# -- the War Room's suspend ------------------------------------------------------------------
WAR_ROOM_SUSPEND = 3


def war_room_suspended(ctx, e, g, label):
    """A War Room map started and saved from its map menu: the War Room
    suspend (slot 3) and the profile's flag. Returns (image, snapshot)."""
    war_room_open(e)
    pick_and_play(e, g)
    before = saves.flash(e, os.path.join(ctx.out, f"{label}_before"))
    saves.suspend(g)
    snap = saves.snapshot(g)
    img = saves.flash(e, os.path.join(ctx.out, f"{label}_saved"))
    saves.expect_slots(ctx, before, img, [WAR_ROOM_SUSPEND], f"{label}: the War Room map saved",
                       profile_allow=[saves.c420(saveimg.C420_SUSPEND[3])] + list(saves.OPTIONS) + [saves.MODE_BYTE])
    return img, snap


def war_room_continue(e, g):
    saves.wheel_to(e, saves.WAR_ROOM)
    e.press("A", 8)
    e.wait(60)
    saves.box_row(e, 0)
    e.press("A", 8)
    if not e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 900, step=10):
        raise NavError("the War Room's Continue did not bring the map back")
    g._units_base = g._players_base = None
    g.wait_for_input()


@test()
def save_war_room_suspend_resume(ctx):
    """A War Room map saved from its map menu, rebooted, continued: the same
    battle."""
    e, g = boot(ctx)
    saves.to_select_mode(e)
    img, snap = war_room_suspended(ctx, e, g, "war_room")
    e.close()
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    war_room_continue(e, g)
    saves.compare_snapshots(ctx, snap, saves.snapshot(g), "after a reboot, the War Room's Continue")


@test(modes=("ds",))
def save_survival_keeps_war_room_suspend(ctx):
    """A War Room map saved halfway, then a Survival map won (Survival plays
    on the War Room's screens and its end of map): the War Room's saved map
    is still there, after a reboot too."""
    e, g = boot(ctx)
    saves.to_select_mode(e)
    img, snap = war_room_suspended(ctx, e, g, "war_room")
    e.close()
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    open_survival_from_wheel(e)
    ctx.require(sv.pick(e, sv.LIST_ORDER.index(sv.MONEY)), "Money Survival starts")
    g._units_base = g._players_base = None
    g.wait_for_input()
    names = g.map_menu_names()
    ctx.check("Save" not in names, f"no Save on a Survival map ({names})")
    win(e, g, armies(e))
    after = saves.flash(e, os.path.join(ctx.out, "survival_won"))
    saves.expect_slots(ctx, img, after, [], "a Survival map won",
                       profile_allow=list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.check(after.slot(WAR_ROOM_SUSPEND) == img.slot(WAR_ROOM_SUSPEND), "the War Room's saved map kept")
    ctx.eq(after.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[3]], 1, "the profile still marks it")
    ctx.require(saves.back_to_select_mode(e), "Select Mode after Survival")
    e.close()
    e, g = boot(ctx, after.path)
    saves.to_select_mode(e)
    war_room_continue(e, g)
    saves.compare_snapshots(ctx, snap, saves.snapshot(g), "after Survival and a reboot, the War Room's Continue")


# -- every slot in use at once ------------------------------------------------------------------

ALL_SLOTS = [0, 2, 3, 4, 5, 6, 7, 8, 14, 15]


def two_army_design(name, x):
    m = DesignMap(name=name)
    m.terrain(0, 0, "hq", 1).terrain(29, 19, "hq", 2)
    m.unit(1, "tank", x, 5).unit(2, "tank", x + 1, 5).unit(1, "infantry", 1, 1).unit(2, "infantry", 28, 18)
    return m


@test(modes=("ds",))
def save_every_slot_at_once(ctx):
    """Every slot in use at once: three designs, an AW2 mission, a War Room
    map and a Versus game (on design 1) saved halfway, the DS Campaign with
    a mission saved halfway: ten slots of Flash's sixteen sectors. Each save
    keeps the others; rebooted, every slot passes AW2's check and its own
    boot, and each saved game continues (and can be saved again)."""
    path = os.path.join(ctx.out, "designs.sav")
    with open(paths.base_save(), "rb") as f:
        data = f.read()
    for slot in (1, 2, 3):
        data = write_design_map(data, two_army_design(f"SLOT{slot}", 4 + 2 * slot).record(), slot)
    with open(path, "wb") as f:
        f.write(data)
    img = saveimg.Image(path)
    snaps = {}

    def saved(label, changed, allow):
        nonlocal img
        new = saves.flash(e, os.path.join(ctx.out, label))
        saves.expect_slots(ctx, img, new, changed, label, profile_allow=allow + list(saves.OPTIONS) + [saves.MODE_BYTE])
        img = new

    # AW2's campaign: Mission 1 saved from its map menu.
    e, g = boot(ctx, img.path)
    d = dc.DsCampaign(g)
    cp.aw2_new(e, d, True)
    saves.suspend(g)
    snaps[2] = saves.snapshot(g)
    # (a new campaign: its world map in the profile)
    saved("aw2_mission", [2], [WM_PROFILE, saves.c420(saveimg.C420_SUSPEND[2])])
    e.close()
    # The War Room.
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    war_room_open(e)
    pick_and_play(e, g)
    saves.suspend(g)
    snaps[3] = saves.snapshot(g)
    saved("war_room", [3], [saves.c420(saveimg.C420_SUSPEND[3])])
    e.close()
    # Versus on design 1.
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    saves.versus_select_map(g, 8, 0xB4)
    saves.versus_start(g)
    saves.suspend(g)
    snaps[4] = saves.snapshot(g)
    saved("versus", [4, 8], [saves.c420(saveimg.C420_SUSPEND[4])])
    e.close()
    # The DS Campaign: New, its first mission saved halfway.
    e, g = boot(ctx, img.path)
    d = dc.DsCampaign(g)
    cp.ds_start(e, d, new=True)
    d.pick_mission()
    d.wait_map()
    saves.suspend(g)
    snaps[14] = saves.snapshot(g)
    saved("ds_mission", [14, 15], [])
    e.close()

    ctx.eq(img.tags(), ALL_SLOTS, "ten slots in use")
    ctx.check(not img.problems(), f"every slot passes AW2's check {img.problems()}")
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    aw2_load_check(ctx, e, img, "every slot in use")
    e.close()

    def resumed(slot, label):
        g._units_base = g._players_base = None
        g.wait_for_input()
        saves.compare_snapshots(ctx, snaps[slot], saves.snapshot(g), label)

    e, g = boot(ctx, img.path)
    d = dc.DsCampaign(g)
    cp.aw2_box(e, d, True)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 900, step=10), "AW2's Continue")
    resumed(2, "AW2's mission continued")
    e.close()
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    war_room_continue(e, g)
    resumed(3, "the War Room's map continued")
    e.close()
    e, g = boot(ctx, img.path)
    d = dc.DsCampaign(g)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "DS CAMPAIGN's Continue")
    resumed(14, "the DS mission continued")
    e.close()
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    saves.versus_continue(g)
    resumed(4, "the Versus game continued")
    # Saved again with every slot in use.
    g.end_turn(human=1)
    saves.suspend(g)
    saved("versus_again", [4, 8], [saves.c420(saveimg.C420_SUSPEND[4])])
    ctx.eq(img.tags(), ALL_SLOTS, "still ten slots")


@test(modes=("ds",))
def save_survival_records_each_kind(ctx):
    """A run of each kind cleared in one boot (Money, Turn, Time): each
    saves its own record's three bytes (and the records' mark), the other
    kinds' records untouched; after a reboot all three are shown."""
    e, g = boot(ctx)
    ctx.require(sv.open_survival(e), "Survival's SELECT MAP")
    img = saves.flash(e, os.path.join(ctx.out, "start"))
    want = {}
    for kind in sv.LIST_ORDER:
        ctx.require(sv.pick(e, sv.LIST_ORDER.index(kind)), f"{sv.NAMES[kind]} starts")
        g._units_base = g._players_base = None
        g.wait_for_input()
        e.w8(sv.STAGE, 10)
        win(e, g, armies(e))
        st = sv.state(e)
        ctx.eq(st["phase"], sv.CLEARED, f"{sv.NAMES[kind]} cleared")
        e.press("A", 8)
        e.wait(30)
        new = saves.flash(e, os.path.join(ctx.out, f"cleared_{kind}"))
        rec = SURVIVAL_RECORDS[0] + 1 + 3 * kind
        saves.expect_slots(ctx, img, new, [], f"{sv.NAMES[kind]} cleared, saved",
                           profile_allow=[(SURVIVAL_RECORDS[0], SURVIVAL_RECORDS[0] + 1), (rec, rec + 3)]
                           + list(POINTS) + list(saves.OPTIONS) + [saves.MODE_BYTE])
        want[kind] = sv.records(e).get(kind)
        ctx.check(want[kind] is not None, f"{sv.NAMES[kind]}'s record {want[kind]}")
        img = new
    e.close()
    e, g = boot(ctx, img.path)
    ctx.require(sv.open_survival(e), "Survival after a reboot")
    ctx.eq(sv.records(e), want, "every kind's record after a reboot")
