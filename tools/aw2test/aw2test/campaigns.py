"""Driving AW2's campaign and the DS Campaign from Select Mode's wheel, for
the save-integrity tests: New, through the story to the battle, a win (a
test aid: `DsCampaign.force_win`) and back on the world map, the next
mission, leaving the map."""

from . import dscampaign as dc
from . import saves
from .game import NavError

LAB_FLAG = 0x90
DS_FLAGS = 0x0203FD20          # the session's campaign flags (crate::ds_campaign::FLAGS)


def box_from_wheel(e, d):
    """Select Mode's Campaign box, open (the wheel up)."""
    saves.wheel_to(e, saves.CAMPAIGN)
    e.press("A", 8)
    if not e.wait_until(d.box_open, 120, step=4):
        raise NavError("the Campaign box did not open")
    e.wait(20)


def aw2_box(e, d, ds, from_title=True):
    """Campaign's box (with the pack: AW2 CAMPAIGN's), open."""
    if from_title:
        d.open_campaign_box()
    else:
        box_from_wheel(e, d)
    if ds:
        d.chooser_row(0)
        e.press("A", 8)
        e.wait(30)


def aw2_new(e, d, ds, from_title=True):
    """AW2's campaign, New (over a saved one: the notice, Yes), through the
    story to Mission 1's first turn."""
    aw2_box(e, d, ds, from_title)
    d.box_row(1)
    e.wait(30)
    e.press("A", 8)
    to_first_battle(e, d)


def ds_start(e, d, new, from_title=True):
    """DS CAMPAIGN's New or Continue, to its world map."""
    if from_title:
        d.open_campaign_box()
    else:
        box_from_wheel(e, d)
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    if e.u8(dc.MENU_LEVEL) != 2:
        raise NavError("not in the DS box")
    d.box_row(1 if new else 0)
    e.press("A", 8)
    for _ in range(40):
        if e.wait_until(d.active, 30, step=5):
            break
        e.press("A", 8)
    else:
        raise NavError("the DS Campaign did not start")
    d.wait_world_map()


def to_first_battle(e, d):
    for _ in range(4000):
        if d.in_battle() and e.u8(0x030033EC) == 1 and not d.scripts_running():
            break
        e.press("A", 4)
        e.wait(10)
    d.g._units_base = d.g._players_base = None
    d.wait_control()


def through_to_map(e, d):
    """After a win: the results, the story, the save prompt (Yes), until the
    world map takes the pad."""
    for _ in range(3000):
        if d.world_map_up() and not d.scripts_running():
            e.wait(30)
            if d.world_map_up() and not d.scripts_running():
                return
        e.press("A", 4)
        e.wait(10)
    raise NavError("the world map did not come back")


def aw2_next_mission(e, d):
    """On AW2's world map: the cursor (RIGHT steps through the missions) on
    an open mission not won, A, its panel, A: to its first turn."""
    for _ in range(10):
        if d.map_flags()[d.map_mission()] == 1:
            break
        e.press("RIGHT", 6)
        e.wait(30)
    for _ in range(10):
        e.press("A", 6)
        if e.wait_until(lambda: d.proc_fn_running(dc.WM_INFO_LOOP), 120, step=5):
            break
    else:
        raise NavError("AW2's mission panel did not open")
    for _ in range(12):
        e.wait(30)
        e.press("A", 6)
        if not d.proc_fn_running(dc.WM_INFO_LOOP) and not d.world_map_up():
            break
    to_first_battle(e, d)


def leave_map(e, d, close=False):
    """From the world map: B, Yes: back to Select Mode, on the Campaign
    box's chooser (with the pack); with `close`, the box closed too."""
    e.wait(30)
    e.press("B", 6)
    e.wait(90)
    e.press("LEFT", 6)
    e.wait(20)
    e.press("A", 6)
    if not e.wait_until(lambda: saves.wheel(e) is not None, 900, step=10):
        raise NavError("Select Mode did not come back")
    e.wait(120)
    if e.u8(dc.MENU_LEVEL) == 2:
        e.press("B", 6)
        e.wait(40)
    for _ in range(3):
        if not close or not d.box_open():
            break
        e.press("B", 6)
        e.wait(40)


def win_ds_mission(e, d, lab_flag=False):
    """On the DS world map: the mission under the cursor picked, won (a
    test aid: `force_win`), the results and back on the map. With
    `lab_flag`, the flag a lab city's capture sets (0x90) is set in the
    session's flags first, as its event would. Returns the mission's index."""
    d.pick_mission()
    d.wait_map()
    index = d.mission()
    if lab_flag:
        f = LAB_FLAG - 0x20
        e.w8(DS_FLAGS + f // 8, e.u8(DS_FLAGS + f // 8) | 1 << (f % 8))
    if not d.force_win():
        raise NavError(f"DS mission {index} not won")
    for _ in range(400):
        e.wait(30)
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    if not d.world_map_up():
        raise NavError("not back on the DS world map")
    e.wait(60)
    return index
