"""The DS Campaign (ds_campaign.rs, ds_campaign_data.rs, ds_campaign_rules.rs,
campaign_menu.rs): with the Dual Strike pack, Select Mode's Campaign opens a
sub-menu, AW2 CAMPAIGN (AW2's own, unchanged) or DS CAMPAIGN (Dual Strike's
story campaign converted at run time from the .nds). Missions are checked
against the .nds read here directly; wins advance and are saved to Flash.
Without the pack the menu is AW2's own."""

import os

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test import survival as sv
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import Skip, test

OAM = 0x07000000
LABEL_TILES = (832, 868)          # campaign_menu::TILES, one label each
GAME_LABEL_TILES = (664, 676)
PROC_CAMPAIGN = 0x0849EB34        # ProcScr_Campaign (AW2's campaign)
PROC_MISSION = 0x0849EBFC
DS_TABLE = 0x08E00000             # survival::TABLE (room for 0x100 ids)
DS_DATA = 0x08F00000              # ds_campaign::DATA ("DSCD")


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


def oam_tiles(e):
    oam = e.read(OAM, 0x400)
    out = set()
    for i in range(128):
        a0 = oam[8 * i] | oam[8 * i + 1] << 8
        if (a0 >> 8) & 3 == 2:  # hidden
            continue
        out.add((oam[8 * i + 4] | oam[8 * i + 5] << 8) & 0x3FF)
    return out


def procs(e):
    return {e.u32(p) for p in range(0x0200D610, 0x0200E418, 0x6C)}


MISSION_TITLE = 0x086165B0       # the mission card's proc (MissionTitle_*)
CARD_DISPCNT = 0x1761            # AW2's own card: BG0, BG1, BG2 and OBJ, windows


def mission_card(ctx, e, d, label, presses=False):
    """Waits for the mission card and checks it is laid out as AW2's own
    campaign card: the same layers on, and nothing on BG0 (the text layer;
    Select Mode's help line once stayed there)."""
    def up():
        return MISSION_TITLE in procs(e)
    for _ in range(3000):
        if up():
            break
        if d.on_co_select() or presses:
            e.press("A", 4)
        e.wait(10)
    if not ctx.check(up(), f"{label}: the mission card"):
        return
    e.wait(90)
    ctx.eq(e.u16(0x04000000), CARD_DISPCNT, f"{label}: the card's layers")
    sbb = (e.u16(0x04000008) >> 8) & 31
    bg0 = e.read(0x06000000 + 0x800 * sbb, 0x800)
    used = [(i % 32, i // 32) for i in range(0x400) if (i % 32) < 30 and (i // 32) < 20 and bg0[2 * i] | bg0[2 * i + 1]]
    ctx.check(not used, f"{label}: nothing on the card's text layer ({len(used)} cells: {used[:6]})")
    shot(ctx, e, "mission_card")


def norm(t):
    """A text without control codes and line breaks."""
    return " ".join("".join(c if c >= " " else " " for c in t).split())


def check_mission(ctx, e, g, d, data, index, label):
    """The battle map against Dual Strike's: size, tiles (Dual Strike's ids
    are AW2's but for the few tangoAW2 converts), units."""
    m = data.mission(index)
    ctx.eq(d.size(), (m["w"], m["h"]), f"{label}: map size")
    have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
    want = sorted(m["units"])
    ctx.eq(have, want, f"{label}: the deployment")
    ctx.eq(e.u8(dc.MAP_ID), dc.DS_MAP_ID, f"{label}: played on map id 0xF0")
    name_id = e.u16(DS_TABLE + 0x5C * dc.DS_MAP_ID + 0x14)
    p = e.u32(0x08610A38 + 4 * name_id)
    raw = e.read(p, 64)
    ctx.eq(raw[:raw.index(b"\0")].decode("latin-1"), m["name"], f"{label}: the mission's name")
    return m


# -- the menu -----------------------------------------------------------------------

@test()
def ds_campaign_submenu(ctx):
    """Campaign opens AW2 CAMPAIGN / DS CAMPAIGN with the pack; AW2's own box
    without it."""
    e, g, d = boot(ctx)
    d.open_campaign_box()
    e.wait(30)
    if not ctx.ds:
        tiles = oam_tiles(e)
        ctx.check(not (tiles & set(LABEL_TILES)), "no sub-menu labels")
        ctx.check(set(GAME_LABEL_TILES) <= tiles, "the game's Continue / New labels")
        ctx.eq(e.u8(dc.MENU_LEVEL), 0, "no sub-menu state")
        shot(ctx, e, "campaign_box")
        return
    ctx.eq(e.u8(dc.MENU_LEVEL), 0, "the chooser")
    ctx.check(set(LABEL_TILES) <= oam_tiles(e), "AW2 CAMPAIGN and DS CAMPAIGN labels shown")
    shot(ctx, e, "submenu_aw2")
    d.chooser_row(1)
    e.wait(10)
    ctx.eq(e.u8(dc.MENU_CHOICE), 1, "DOWN: DS CAMPAIGN")
    shot(ctx, e, "submenu_ds")
    e.press("A", 8)
    e.wait(30)
    ctx.eq(e.u8(dc.MENU_LEVEL), 2, "A: the DS Campaign's Continue / New")
    ctx.check(set(GAME_LABEL_TILES) <= oam_tiles(e), "its box shows Continue / New")
    shot(ctx, e, "ds_box")
    e.press("B", 8)
    e.wait(30)
    ctx.eq(e.u8(dc.MENU_LEVEL), 0, "B: back to the chooser")
    ctx.eq(e.u8(dc.MENU_CHOICE), 1, "on DS CAMPAIGN")


@test()
def aw2_campaign_unchanged(ctx):
    """AW2's campaign starts as before: through AW2 CAMPAIGN with the pack,
    straight from the box without it."""
    e, g, d = boot(ctx)
    d.open_campaign_box()
    if ctx.ds:
        d.chooser_row(0)
        e.press("A", 8)
        e.wait(30)
        ctx.eq(e.u8(dc.MENU_LEVEL), 1, "AW2 CAMPAIGN: AW2's box")
    d.box_row(1)  # New
    e.press("A", 8)
    ok = e.wait_until(lambda: bool(procs(e) & {PROC_CAMPAIGN, PROC_MISSION}), 1800, step=10)
    ctx.require(ok, "AW2's campaign runs")
    ctx.eq(e.u8(dc.ACTIVE), 0, "no DS session")
    e.wait(300)
    shot(ctx, e, "aw2_campaign")
    # Through the story to its first mission's card (A on every screen).
    mission_card(ctx, e, d, "AW2's first mission", presses=True)
    ctx.eq(e.u8(dc.ACTIVE), 0, "still no DS session")
    ctx.check(e.u8(dc.MAP_ID) < 0xC0, f"an AW2 map id ({e.u8(dc.MAP_ID):#x})")


@test(modes=("ds",))
def ds_campaign_with_survival(ctx):
    """One boot: the wheel's seven entries (Survival's included), Survival's
    SELECT MAP and back, then Campaign's sub-menu and the DS Campaign."""
    e, g, d = boot(ctx)
    sv.to_select_mode(e)
    seen = set()
    for _ in range(8):
        seen.add(e.u8(sv.SELECT_MODE_CURSOR))
        e.press("UP", 8)
        e.wait(60)
    ctx.eq(sorted(seen), list(range(7)), "seven wheel positions")
    ctx.require(sv.wheel_to(e, sv.SURVIVAL_POSITION), "Survival reached")
    e.wait(30)
    ctx.eq(e.u8(dc.MENU_LEVEL), 0, "no Campaign sub-menu on Survival")
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10), "Survival's SELECT MAP")
    e.wait(90)
    e.press("B", 8)
    ctx.require(e.wait_until(lambda: e.u8(sv.ON) == 0, 600, step=10), "B leaves Survival")
    # Back on Select Mode with the War Room's box open (Survival goes through
    # it): B closes the box.
    ctx.require(e.wait_until(lambda: d.wheel() is not None, 900, step=10), "back on Select Mode")
    e.wait(120)
    e.press("B", 8)
    e.wait(60)
    ctx.require(sv.wheel_to(e, dc.CAMPAIGN), "Campaign reached")
    e.wait(30)
    e.press("A", 8)
    ctx.require(e.wait_until(d.box_open, 120, step=4), "Campaign's box")
    e.wait(20)
    ctx.eq(e.u8(dc.MENU_LEVEL), 0, "the chooser")
    ctx.check(set(LABEL_TILES) <= oam_tiles(e), "the sub-menu's labels")
    shot(ctx, e, "chooser_after_survival")
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(1)
    e.press("A", 8)
    ctx.require(e.wait_until(d.active, 900, step=10), "the DS Campaign starts")
    ctx.eq(e.u8(sv.ON), 0, "Survival stays off")
    d.wait_map()
    ctx.eq(d.mission(), 0, "Jake's Trial")
    ctx.eq(e.u8(dc.MAP_ID), dc.DS_MAP_ID, "map id 0xF0")


# -- missions -----------------------------------------------------------------------

@test(modes=("ds",))
def ds_campaign_first_mission(ctx):
    """New: Jake's Trial, its map, deployment and Dual Strike's dialogue (with
    its portraits) before the player gets the map."""
    data = dc.DsData()
    e, g, d = boot(ctx)
    d.start(new=True)
    ctx.eq(d.mission(), 0, "mission 0")
    ctx.eq(e.u8(dc.GAME_MODE), 1, "AW2's campaign mode")
    mission_card(ctx, e, d, "Jake's Trial")
    seen = []
    shots = 0
    idle = 0
    for _ in range(1500):
        t = d.text_shown() if d.in_battle() else None
        if t is not None and (not seen or seen[-1] != t):
            seen.append(t)
            if shots < 3:
                e.wait(40)
                shot(ctx, e, f"dialogue_{shots}")
                shots += 1
        if d.on_co_select() or not d.in_battle():
            e.press("A", 4)
        elif d.scripts_running():
            idle = 0
            e.wait(20)  # let the box fill before the next one
            e.press("A", 4)
        else:
            idle += 1
            if seen and idle > 8:
                break
        e.wait(12)
    ctx.check(len(seen) >= 4, f"the opening dialogue ran ({len(seen)} boxes)")
    # Every box is a piece of one of Dual Strike's texts for this mission
    # (its bank 0x3E), re-wrapped for AW2's boxes.
    corpus = []
    for k in range(0x60):
        try:
            t = data.text(0x3E000000 | k)
        except Exception:
            break
        if t is not None:
            corpus.append(norm(t.decode("latin-1")))
    foreign = [t for t in seen if not any(norm(t) in c for c in corpus)]
    ctx.check(not foreign, f"Dual Strike's own words ({len(foreign)} other: {foreign[:2]})")
    d.wait_control()
    m = check_mission(ctx, e, g, d, data, 0, "Jake's Trial")
    shot(ctx, e, "map")


def _spot(step, label):
    def fn(ctx):
        data = dc.DsData()
        e, g, d = boot(ctx)
        d.start(step=step)
        index = dc.ORDER[step]
        ctx.eq(d.mission(), index, f"{label}: mission {index}")
        d.wait_map()
        check_mission(ctx, e, g, d, data, index, label)
        shot(ctx, e, "map")
    fn.__name__ = "ds_campaign_mission_" + label.lower().replace(" ", "_").replace("!", "").replace("'", "")
    test(modes=("ds",))(fn)


for _step, _label in ((5, "The Ocean Blue"), (8, "Victory or Death"), (15, "Snow Hunters"), (22, "Ring of Fire"), (26, "For the Future")):
    _spot(_step, _label)


@test(modes=("ds",))
def ds_campaign_win_and_continue(ctx):
    """A win (forced) shows the results and starts the next mission; the
    progress is in Flash: after a reboot, DS CAMPAIGN's Continue resumes
    there."""
    e, g, d = boot(ctx)
    d.start(new=True)
    d.wait_map()
    ctx.require(d.force_win(), "the last enemy unit destroyed")
    results = False
    for _ in range(240):
        e.wait(30)
        if not d.in_battle() and not results:
            e.wait(60)
            shot(ctx, e, "results")
            results = True
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        if d.mission() == 1 and d.in_battle():
            break
    ctx.eq(d.mission(), 1, "the next mission (The New Black)")
    p = d.progress()
    ctx.eq((p["valid"], p["next"], p["won"]), (True, 1, 1), "progress: mission 0 won, step 1 next")
    d.wait_map()
    shot(ctx, e, "mission2")
    save = e.save(os.path.join(ctx.out, "after_win"))
    e.close()

    e, g, d = boot(ctx, save)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    ctx.eq(d.box_cursor(), 0, "Continue offered (a DS Campaign is saved)")
    ctx.eq(d.progress()["next"], 1, "the saved progress")
    e.press("A", 8)
    ctx.require(e.wait_until(d.active, 900, step=10), "Continue starts")
    d.wait_map()
    ctx.eq(d.mission(), 1, "resumed at The New Black")


@test(modes=("ds",))
def ds_campaign_cpu_plays(ctx):
    """The computer plays DS Campaign maps: three days on four missions, the
    player's armies handed to the computer too (+0x1B: 1 human, 2 computer),
    event dialogue answered as it comes. No army drops out on its own (Crystal
    Calamity's Black Hole once lost at the first action: its "Obelisk
    destroyed" test looked at the wrong cell)."""
    for step in (2, 9, 18, 21):
        e, g, d = boot(ctx)
        d.start(step=step)
        d.wait_map()
        index = dc.ORDER[step]
        players = e.u32(0x08499598)
        armies = [a for a in range(1, 5) if e.u8(players + 0x3C * a + 0x1B) != 0]
        start_day = e.u16(dc.DAY)
        before = {u["id"]: (u["x"], u["y"]) for u in g.units()}
        for a in armies:
            e.w8(players + 0x3C * a + 0x1B, 2)
        d.wait_control()
        try:
            d.end_turn()
        except NavError:
            e.shot(os.path.join(ctx.out, f'stuck_{index}'))
            raise
        for _ in range(1500):
            if e.u16(dc.DAY) >= start_day + 3:
                break
            if d.scripts_running():
                e.press("A", 4)
            e.wait(30)
        after = {u["id"]: (u["x"], u["y"]) for u in g.units()}
        moved = sum(1 for k in before if after.get(k) != before[k])
        ctx.check(e.u16(dc.DAY) >= start_day + 3, f"mission {index}: three days played (day {e.u16(dc.DAY)})")
        ctx.check(moved > 0, f"mission {index}: units moved ({moved}), {len(after)} units now")
        ctx.check(d.active() and d.mission() == index, f"mission {index}: still the DS session")
        lost = [a for a in armies if e.u16(players + 0x3C * a + 0x14) != 0]
        ctx.eq(lost, [], f"mission {index}: no army out")
        shot(ctx, e, f"cpu_{index}")
        e.close()


@test(modes=("ds",))
def ds_campaign_grand_bolt(ctx):
    """Means to an End: the Grand Bolt (a picture of tiles AW2 has no art
    for) is plains with a Black Obelisk on each of its three weak points;
    on Black Hole's turn of every sixth day each weak point standing spawns
    an Oozium below it; the mission goes on until they are destroyed."""
    e, g, d = boot(ctx)
    d.start(step=27)
    d.wait_map()
    ctx.eq(d.mission(), 24, "Means to an End")
    inv = [(e.u8(0x02028360 + 8 * k), e.u8(0x02028361 + 8 * k), (e.u16(0x02028362 + 8 * k) >> 6) & 15)
           for k in range(16)]
    ctx.eq(sorted(i for i in inv if i[2]), [(2, 7, 3), (8, 9, 3), (14, 7, 3)], "three Obelisks on the weak points")
    cells = [(3, 10), (9, 12), (15, 10)]
    at = lambda: [next(((u["army"], u["type"]) for u in g.units() if (u["x"], u["y"]) == c), None) for c in cells]
    ctx.eq(at(), [None, None, None], "nothing below the weak points")
    e.w16(dc.DAY, 6)
    d.end_turn()
    spawned = False
    for _ in range(600):
        if d.scripts_running():
            e.press("A", 4)
        e.wait(20)
        if at() == [(2, 27)] * 3:
            spawned = True
        if e.u8(0x030033EC) == 1 and spawned:
            break
    ctx.check(spawned, "day 6: an Oozium of Black Hole's below each weak point")
    ctx.check(d.active() and d.mission() == 24 and d.in_battle(), "the mission goes on")
    d.wait_control()
    g.goto(9, 8)
    e.wait(30)
    shot(ctx, e, "grand_bolt")
