"""Save integrity, the campaigns: AW2's campaign (its progress saved after
a mission won, a mission suspended from the map menu and continued after a
reboot, with and without the Dual Strike pack, the pack's save the same as
AW2's own), and the DS Campaign over an AW2 campaign in progress: its
record (slot 15) after missions won, a lab mission's flag and the prologue
seen, continued after a reboot, nothing saved halfway through a mission,
and AW2's campaign, its suspended mission and every other byte of the
profile as they were (but the Battle Maps points a won mission earns, as
AW2's own missions and the War Room do)."""

import os
import threading

from aw2test import campaigns as cp
from aw2test import dscampaign as dc
from aw2test import paths, saveimg, saves
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import test

CAMPAIGN_SUSPEND, DS_RECORD = 2, 15
WM = 0x0202FDFC
WM_PROFILE = (0x4D0, 0x5CC)
# Battle Maps points (C420 +0x00 spendable, +0x04 the most ever held):
# every won mission earns them (AW2's campaign, the War Room, Survival, and
# a DS mission), so they change with a win.
POINTS = (saves.c420(0x00, 4), saves.c420(0x04, 4))
PROLOGUE_FLAG = 0x9E
LAB_FLAG = cp.LAB_FLAG


def boot(ctx, save, ds=None):
    e = Emu(save=save, ds=ctx.ds if ds is None else ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


_LOCK = threading.Lock()
_AW2 = {}


def aw2_campaign_image(ctx, ds):
    """From the pinned save: AW2's campaign, New, Mission 1 won (its save
    prompt answered Yes), Mission 2 started from the world map and saved
    from its map menu. Made once per mode and test run. Returns {"won": the
    image after the win, "suspended": after the Save, "snap": the battle
    saved, "wm": the world map state saved}."""
    key = "ds" if ds else "aw2"
    with _LOCK:
        if key in _AW2:
            return _AW2[key]
        e = Emu(save=paths.base_save(), ds=ds)
        g = Game(e, ctx.image)
        d = dc.DsCampaign(g)
        base = saves.flash(e, os.path.join(ctx.out, f"aw2c_{key}_base"))
        cp.aw2_new(e, d, ds)
        if not d.force_win():
            raise NavError("AW2's Mission 1 not won")
        cp.through_to_map(e, d)
        won = saves.flash(e, os.path.join(ctx.out, f"aw2c_{key}_won"))
        wm = e.read(WM, 0xFC)
        cp.aw2_next_mission(e, d)
        mission = e.u8(0x03003FC2)
        saves.suspend(g)
        snap = saves.snapshot(g)
        suspended = saves.flash(e, os.path.join(ctx.out, f"aw2c_{key}_suspended"))
        e.close()
        _AW2[key] = {"base": base, "won": won, "suspended": suspended, "snap": snap, "wm": wm, "mission": mission}
        return _AW2[key]


@test()
def save_aw2_campaign_progress_and_suspend(ctx):
    """AW2's campaign: Mission 1 won writes the profile only (its world
    map, flags, scores; no other slot); Mission 2 saved from its map menu
    writes the campaign suspend and the profile's flag; after a reboot
    Campaign's Continue brings the same battle back, which goes on. With
    the pack the saved profile is byte for byte the one AW2 saves without
    it."""
    c = aw2_campaign_image(ctx, ctx.ds)
    base, won, sus = c["base"], c["won"], c["suspended"]
    ctx.check(not won.problems(), f"after the win: every slot passes AW2's check {won.problems()}")
    d = saveimg.diff(base, won)
    ctx.eq(sorted(d), [0], "Mission 1 won: only the profile written")
    ctx.log("profile bytes the win changed: " + saveimg.fmt_offsets(saveimg.profile_changes(base, won)))
    ctx.check(won.slot(0)[WM_PROFILE[0]:WM_PROFILE[1]] == c["wm"], "the world map saved as it is shown")
    # (the world map's cursor and camera moved to Mission 2 since the win)
    saves.expect_slots(ctx, won, sus, [CAMPAIGN_SUSPEND], "Mission 2 saved from the map menu",
                       profile_allow=[saves.c420(saveimg.C420_SUSPEND[2]), WM_PROFILE] + list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.eq(sus.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[2]], 1, "the profile marks a campaign mission saved")
    if ctx.ds:
        ref = aw2_campaign_image(ctx, False)
        for label, a, b in (("after the win", ref["won"], won), ("after the Save", ref["suspended"], sus)):
            # (but the world map's camera, +0x04, mid-scroll at a frame that
            # differs: the pack's Campaign box takes a few more frames)
            r = [(s, t) for s, t in saveimg.ranges(a.slot(0), b.slot(0)) if (s, t) != (WM_PROFILE[0] + 4, WM_PROFILE[0] + 5)]
            ctx.check(not r, f"{label}: the profile the same as AW2's own without the pack (differs: "
                             f"{saveimg.fmt_offsets([k for s, t in r for k in range(s, t)])})")
    e, g, dcm = boot(ctx, sus.path)
    cp.aw2_box(e, dcm, ctx.ds)
    dcm.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 900, step=10), "Continue: the battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    saves.compare_snapshots(ctx, c["snap"], saves.snapshot(g), "after a reboot, Continue")
    ctx.eq(e.u8(0x03003FC2), c["mission"], "the mission")
    ctx.shot(g, "resumed")
    # (Mission 2 is a lesson: every unit must move before the turn ends)
    u = next(u for u in g.units(1) if not u["flags"] & 1)
    dcm.dialogue()
    g.wait_unit(u["x"], u["y"])
    ctx.check(g.unit(u["id"])["flags"] & 1, "the resumed mission goes on (a unit moved)")


def flag(p, f):
    f -= 0x20
    return (p[0x10 + f // 8] >> (f % 8)) & 1


@test(modes=("ds",))
def save_ds_campaign_saves_beside_aw2(ctx):
    """Over an AW2 campaign with a mission saved halfway: the DS Campaign's
    New (the prologue's flag saved), three missions won (one opening a lab
    mission), each saved in slot 15 only (and the profile, which AW2's
    writer rewrites with every slot: the same but its save counter and the
    points a win earns); a DS mission played (not saved) writes nothing;
    turned off mid-mission and rebooted, DS CAMPAIGN's
    Continue shows the map with three missions won and the lab mission
    open; AW2 CAMPAIGN's Continue still brings its saved mission back, its
    world map as it was."""
    c = aw2_campaign_image(ctx, True)
    start = c["suspended"]
    e, g, d = boot(ctx, start.path)
    d.start(new=True, pick=False)
    d.wait_world_map()
    img = saves.flash(e, os.path.join(ctx.out, "ds_new"))
    saves.expect_slots(ctx, start, img, [DS_RECORD], "DS Campaign New", profile_allow=[saves.MODE_BYTE])
    rec = img.slot(DS_RECORD)
    ctx.check(rec is not None and rec[:4] == b"AWDC", "the DS record in slot 15")
    ctx.eq(flag(rec, PROLOGUE_FLAG), 1, "the prologue seen, saved")
    won = []
    for k in range(3):
        before = img
        won.append(cp.win_ds_mission(e, d, lab_flag=(k == 2)))
        img = saves.flash(e, os.path.join(ctx.out, f"ds_won{k + 1}"))
        saves.expect_slots(ctx, before, img, [DS_RECORD], f"DS mission {won[-1]} won",
                           profile_allow=list(POINTS) + [saves.MODE_BYTE])
        p = img.slot(DS_RECORD)
        bits = int.from_bytes(p[8:12], "little")
        ctx.eq(bits, sum(1 << m for m in won), f"after mission {won[-1]}: the missions won, saved")
    ctx.eq(flag(img.slot(DS_RECORD), LAB_FLAG), 1, "the lab mission's flag, saved")
    ctx.check(img.slot(0)[0:0x48] == start.slot(0)[0:0x48],
              "AW2's flags and unlocks (Hard Campaign's among them) as they were")
    ctx.eq(img.slot(0)[WM_PROFILE[0]:WM_PROFILE[1]], start.slot(0)[WM_PROFILE[0]:WM_PROFILE[1]],
           "AW2's world map as it was")
    ctx.eq(img.slot(CAMPAIGN_SUSPEND), start.slot(CAMPAIGN_SUSPEND), "AW2's saved mission untouched")
    # A DS mission played, not saved: nothing written.
    d.pick_mission()
    d.wait_map()
    playing = d.mission()
    mid0 = saves.flash(e, os.path.join(ctx.out, "ds_mission_start"))
    names = g.map_menu_names()
    ctx.check("Save" in names, f"a DS mission's map menu has Save, as Dual Strike's ({names})")
    # Played a while: a unit moved, the map menu opened, time passing.
    u = next((u for u in g.units(1) if not u["flags"] & 1), None)
    if u:
        g.wait_unit(u["x"], u["y"])
    g.map_menu_names()
    e.wait(1200)
    mid1 = saves.flash(e, os.path.join(ctx.out, "ds_mission_played"))
    ctx.check(mid1.data == mid0.data, f"playing a DS mission writes nothing ({saveimg.diff(mid0, mid1)})")
    e.close()

    # Turned off mid-mission: DS CAMPAIGN's Continue, the map as saved.
    e, g, d = boot(ctx, mid1.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    ctx.eq(d.box_cursor(), 0, "after a reboot: DS CAMPAIGN offers Continue")
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(d.active, 900, step=10), "Continue starts")
    d.wait_world_map()
    e.wait(30)
    flags = d.map_flags()
    ctx.eq(sorted(m for m in range(28) if flags[m] & 2), sorted(won), "the missions won, starred")
    ctx.check(flags[25] & 1, "the lab mission (The Long March) is open")
    ctx.check(not flags[playing] & 2, "the mission turned off halfway is not won")
    ctx.eq(len(d.cleared_flags()), len(won), "the starred flags on the map")
    ctx.shot(g, "ds_continue")
    cp.leave_map(e, d)
    # AW2 CAMPAIGN: its saved mission, as saved.
    d.chooser_row(0)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 900, step=10), "AW2's Continue: its battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    saves.compare_snapshots(ctx, c["snap"], saves.snapshot(g), "AW2's mission after the DS Campaign")
    ctx.check(e.read(WM, 0xFC) == start.slot(0)[WM_PROFILE[0]:WM_PROFILE[1]], "AW2's world map state as AW2 saved it")
    ctx.shot(g, "aw2_resumed")


@test(modes=("ds",))
def save_ds_campaign_loss_keeps_aw2(ctx):
    """A DS mission lost (the player's army yields) over an AW2 campaign
    with a mission saved halfway: back on the DS map nothing is won, and
    AW2's saved mission and its flag in the profile are as they were."""
    c = aw2_campaign_image(ctx, True)
    start = c["suspended"]
    e, g, d = boot(ctx, start.path)
    d.start(new=True, pick=False)
    d.wait_world_map()
    img = saves.flash(e, os.path.join(ctx.out, "ds_new"))
    d.pick_mission()
    d.wait_map()
    # Jake's Trial (a lesson: every unit must move before the turn ends):
    # army 1 yields and its units have moved.
    for u in g.units(1):
        e.w8(g.unit_addr(u["id"]) + 1, u["flags"] | 1)
    e.w8(g.player(1)["addr"] + 0x31, 1)
    d.end_turn()
    for _ in range(400):
        e.wait(30)
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    ctx.eq(d.last_result()["result"], 2, "the mission lost")
    ctx.require(d.world_map_up(), "back on the DS map")
    after = saves.flash(e, os.path.join(ctx.out, "ds_lost"))
    saves.expect_slots(ctx, img, after, [], "a DS mission lost", profile_allow=[saves.MODE_BYTE])
    ctx.check(after.slot(CAMPAIGN_SUSPEND) == start.slot(CAMPAIGN_SUSPEND), "AW2's saved mission untouched")
    ctx.eq(after.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[2]], 1, "the profile still marks AW2's mission saved")
    ctx.eq(int.from_bytes(after.slot(DS_RECORD)[8:12], "little"), 0, "nothing won in the DS record")


DS_SUSPEND = 14
DS_FLAGS = 0x0203FD20


@test(modes=("ds",))
def save_ds_campaign_mission_suspend(ctx):
    """Dual Strike's campaign lets a mission be saved halfway (its map menu's
    Save: "Save over Mission / Day data"), and so does the DS Campaign: over
    an AW2 campaign with its own mission saved halfway, a DS mission saved
    from the map menu goes to slot 14 (AW2's saved mission, slot 2, and its
    mark in the profile untouched); after a reboot DS CAMPAIGN's Continue
    brings the same battle back (the session's flags too); won, the save is
    dropped and the mission is won in the DS record; after another reboot
    DS CAMPAIGN's Continue shows the map, and AW2 CAMPAIGN's Continue still
    brings AW2's own saved mission back."""
    c = aw2_campaign_image(ctx, True)
    start = c["suspended"]
    e, g, d = boot(ctx, start.path)
    d.start(new=True, pick=False)
    d.wait_world_map()
    d.pick_mission()
    d.wait_map()
    mission = d.mission()
    # A flag of the session (as a capture's event sets it).
    f = LAB_FLAG - 0x20
    e.w8(DS_FLAGS + f // 8, e.u8(DS_FLAGS + f // 8) | 1 << (f % 8))
    before = saves.flash(e, os.path.join(ctx.out, "before"))
    names = saves.suspend(g)
    ctx.check("Save" in names, f"Save on a DS mission's map menu ({names})")
    snap = saves.snapshot(g)
    flags = e.read(DS_FLAGS, 16)
    saved = saves.flash(e, os.path.join(ctx.out, "saved"))
    saves.expect_slots(ctx, before, saved, [DS_SUSPEND], "a DS mission saved halfway",
                       profile_allow=list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.check(saved.slot(CAMPAIGN_SUSPEND) == start.slot(CAMPAIGN_SUSPEND), "AW2's saved mission untouched")
    ctx.eq(saved.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[2]], 1, "AW2's mark in the profile kept")
    e.close()

    e, g, d = boot(ctx, saved.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "DS CAMPAIGN's Continue: the battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    ctx.check(d.active(), "a DS session")
    ctx.eq((d.mission(), d.map_id()), (mission, dc.DS_MAP_ID), "the mission saved")
    saves.compare_snapshots(ctx, snap, saves.snapshot(g), "after a reboot, Continue")
    ctx.check(e.read(DS_FLAGS, 16) == flags, "the session's flags as saved")
    ctx.shot(g, "resumed")
    ctx.require(d.force_win(), "won")
    for _ in range(400):
        e.wait(30)
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    ctx.require(d.world_map_up(), "back on the DS map")
    e.wait(60)
    after = saves.flash(e, os.path.join(ctx.out, "won"))
    ctx.check(DS_SUSPEND not in after.tags(), f"the halfway save dropped with the win ({after.tags()})")
    ctx.check(not after.problems(), f"every slot passes AW2's check {after.problems()}")
    ctx.eq(int.from_bytes(after.slot(DS_RECORD)[8:12], "little") >> mission & 1, 1, "the mission won, saved")
    ctx.check(after.slot(CAMPAIGN_SUSPEND) == start.slot(CAMPAIGN_SUSPEND), "AW2's saved mission still untouched")
    ctx.eq(after.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[2]], 1, "AW2's mark in the profile still kept")
    e.close()

    e, g, d = boot(ctx, after.path)
    cp.ds_start(e, d, new=False)
    e.wait(30)
    ctx.eq(d.map_flags()[mission] & 2, 2, "after a reboot: DS CAMPAIGN's Continue shows the map, the mission won")
    cp.leave_map(e, d)
    d.chooser_row(0)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 900, step=10), "AW2's Continue: its battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    saves.compare_snapshots(ctx, c["snap"], saves.snapshot(g), "AW2's mission after the DS mission's save")


@test(modes=("ds",))
def save_ds_campaign_new_drops_mission_suspend(ctx):
    """A DS mission saved halfway, then a new DS Campaign (New, over the
    saved one): the halfway save is gone with the old campaign; Continue
    shows the new campaign's map."""
    e, g, d = boot(ctx, paths.base_save())
    d.start(new=True, pick=False)
    d.wait_world_map()
    d.pick_mission()
    d.wait_map()
    saves.suspend(g)
    saved = saves.flash(e, os.path.join(ctx.out, "saved"))
    ctx.check(DS_SUSPEND in saved.tags(), "the DS mission saved halfway")
    e.close()
    e, g, d = boot(ctx, saved.path)
    cp.ds_start(e, d, new=True)
    after = saves.flash(e, os.path.join(ctx.out, "new"))
    ctx.check(DS_SUSPEND not in after.tags(), f"New: the halfway save dropped ({after.tags()})")
    ctx.check(not after.problems(), f"every slot passes AW2's check {after.problems()}")
    cp.leave_map(e, d)
    e.close()
    e, g, d = boot(ctx, after.path)
    cp.ds_start(e, d, new=False)
    ctx.check(d.world_map_up() and not d.in_battle(), "Continue: the new campaign's map, no battle")


RECORD_LEN = 0x20 + 8 * 32       # the DS record: the progress, then the missions' records


def hard_flag(e):
    f = dc.HARD_FLAG - 0x20
    return (e.u8(DS_FLAGS + f // 8) >> (f % 8)) & 1


@test(modes=("ds",))
def save_ds_campaign_hard_and_records(ctx):
    """A Hard DS Campaign (opened by a Normal campaign cleared: the record's
    clears byte, set here as Means to an End's win sets it) over an AW2
    campaign in progress: New Hard, a mission won (its record in the Hard
    word), saved in slot 15 only; a Hard mission saved halfway and
    continued after a reboot is still Hard (the session's flag 0x60, AW2's
    Hard Campaign flag, which the DS Campaign keeps as its difficulty); won,
    its record is saved. AW2's own flag 0x60 and campaign records in the
    profile are untouched throughout; after a reboot Continue is Hard with
    the records shown."""
    c = aw2_campaign_image(ctx, True)
    start = c["suspended"]
    aw2_profile = start.slot(0)
    e, g, d = boot(ctx, start.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    e.w32(dc.P_MAGIC, dc.PROGRESS_MAGIC)
    e.w8(dc.P_CLEARS, 1)
    d.box_row(1)
    e.wait(20)
    e.press("A", 8)
    e.wait(20)
    e.press("DOWN", 6)            # the Normal / Hard choice: Hard
    e.wait(20)
    e.press("A", 8)
    for _ in range(40):
        if e.wait_until(d.active, 30, step=5):
            break
        e.press("A", 8)
    ctx.require(d.active(), "Hard: the session starts")
    ctx.eq((e.u8(dc.P_HARD), hard_flag(e)), (1, 1), "a Hard campaign")
    d.wait_world_map()
    img = saves.flash(e, os.path.join(ctx.out, "hard_new"))
    saves.expect_slots(ctx, start, img, [DS_RECORD], "Hard New", profile_allow=[saves.MODE_BYTE])
    ctx.eq(len(img.slot(DS_RECORD)), RECORD_LEN, "the DS record: progress and records")
    won = cp.win_ds_mission(e, d)
    after = saves.flash(e, os.path.join(ctx.out, "hard_won"))
    saves.expect_slots(ctx, img, after, [DS_RECORD], f"Hard mission {won} won", profile_allow=list(POINTS) + [saves.MODE_BYTE])
    rec = after.slot(DS_RECORD)
    word = lambda r, m, hard: int.from_bytes(r[0x20 + 8 * m + 4 * hard:0x24 + 8 * m + 4 * hard], "little")
    ctx.check(word(rec, won, 1) >> 20 > 0 and word(rec, won, 0) == 0, f"its record in the Hard word ({word(rec, won, 1):#x})")
    ctx.eq(rec[6], 1, "the record says Hard")
    # A Hard mission saved halfway.
    d.pick_mission()
    d.wait_map()
    mission = d.mission()
    saves.suspend(g)
    snap = saves.snapshot(g)
    saved = saves.flash(e, os.path.join(ctx.out, "hard_saved"))
    saves.expect_slots(ctx, after, saved, [DS_SUSPEND], "a Hard mission saved halfway",
                       profile_allow=list(saves.OPTIONS) + [saves.MODE_BYTE])
    e.close()
    e, g, d = boot(ctx, saved.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "Continue: the Hard mission")
    g._units_base = g._players_base = None
    g.wait_for_input()
    ctx.eq((d.mission(), hard_flag(e), e.u8(dc.P_HARD)), (mission, 1, 1), "the mission, Hard")
    saves.compare_snapshots(ctx, snap, saves.snapshot(g), "the Hard mission continued")
    ctx.require(d.force_win(), "won")
    for _ in range(400):
        e.wait(30)
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    e.wait(60)
    end = saves.flash(e, os.path.join(ctx.out, "hard_won2"))
    ctx.check(DS_SUSPEND not in end.tags() and not end.problems(), f"the halfway save dropped, every slot valid {end.tags()}")
    rec2 = end.slot(DS_RECORD)
    ctx.check(word(rec2, mission, 1) >> 20 > 0 and word(rec2, won, 1) == word(rec, won, 1),
              "both Hard records saved (the first unchanged)")
    p = end.slot(0)
    ctx.check(p[0:0x48] == aw2_profile[0:0x48], "AW2's flags (its own Hard Campaign flag 0x60 among them) untouched")
    ctx.check(p[0x2A0:0x3F0] == aw2_profile[0x2A0:0x3F0], "AW2's campaign records untouched")
    e.close()
    e, g, d = boot(ctx, end.path)
    cp.ds_start(e, d, new=False)
    ctx.eq((hard_flag(e), e.u8(dc.P_HARD)), (1, 1), "after a reboot: Continue is Hard")
    ctx.check(e.read(dc.RECORDS, 8 * 32) == rec2[0x20:], "the records loaded")
