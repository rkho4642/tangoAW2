"""The DS Campaign (ds_campaign.rs, ds_campaign_data.rs, ds_campaign_rules.rs,
campaign_menu.rs): with the Dual Strike pack, Select Mode's Campaign opens a
sub-menu, AW2 CAMPAIGN (AW2's own, unchanged) or DS CAMPAIGN (Dual Strike's
story campaign converted at run time from the .nds). Missions are checked
against the .nds read here directly; wins advance and are saved to Flash.
Without the pack the menu is AW2's own."""

import os

from aw2test import dscampaign as dc
from aw2test import looks, paths, ram
from aw2test import rom as romlib
from aw2test import survival as sv
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import Skip, test
from aw2test.stitch import stitch

OAM = 0x07000000
LABEL_TILES = (832, 868)          # campaign_menu::TILES, one label each
GAME_LABEL_TILES = (664, 676)
PROC_CAMPAIGN = 0x0849EB34        # ProcScr_Campaign (AW2's campaign)
PROC_MISSION = 0x0849EBFC
DS_TABLE = 0x08E00000             # survival::TABLE (room for 0x100 ids)
DS_DATA = 0x08F00000              # ds_campaign::DATA ("DSCD")
MTE_CRYSTALS = [(3, 3), (9, 5), (15, 3)]  # ds_campaign_data::MTE_CRYSTALS
MTE_DAYS = 36


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
    d.pick_mission()
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
    """The world map: New shows Jake's Trial alone; picked with the cursor
    and won through the pad (aw2test.bot), it brings the results, then the map with
    Jake's Trial cleared (a starred flag on its point) and The New Black
    revealed and under the cursor,
    Max Attacks still hidden. The progress is in Flash: after a reboot,
    DS CAMPAIGN's Continue shows the same map."""
    e, g, d = boot(ctx)
    d.start(new=True, pick=False)
    d.wait_world_map()
    flags = d.map_flags()
    ctx.eq(flags, [1] + [0] * 27, "New: Jake's Trial's flag alone, every other mission hidden")
    ctx.eq(map_flags_expected(e), flags, "New: the flags Dual Strike shows")
    shot(ctx, e, "world_map_new")
    d.pick_mission()
    d.wait_map()
    r = d.play(10, **dc.plan(dc.DsData(), 0))
    ctx.eq(r["result"], 1, f"Jake's Trial won through the pad in {r['days']} days (condition {r['condition']:#x})")
    results = False
    for _ in range(400):
        e.wait(30)
        if not d.in_battle() and not results:
            e.wait(60)
            shot(ctx, e, "results")
            results = True
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    ctx.require(d.world_map_up(), "back on the world map")
    e.wait(60)
    flags = d.map_flags()
    ctx.eq((flags[0] & 2, flags[1] & 1, flags[2]), (2, 1, 0), "Jake's Trial cleared, The New Black shown, Max Attacks hidden")
    ctx.eq(flags, [2, 1] + [0] * 26, "after the win: Jake's Trial cleared, The New Black open, nothing else")
    ctx.eq(d.map_mission(), 1, "the cursor on The New Black")
    ctx.eq(len(d.cleared_flags()), 1, "a starred flag on Jake's Trial's point")
    p = d.progress()
    ctx.eq((p["valid"], p["next"], p["won"]), (True, 1, 1), "progress: mission 0 won, step 1 next")
    shot(ctx, e, "world_map_after_win")
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
    d.wait_world_map()
    flags = d.map_flags()
    ctx.eq((flags[0] & 2, flags[1] & 1, flags[2]), (2, 1, 0), "after a reboot: the same map")
    e.wait(30)
    ctx.eq(len(d.cleared_flags()), 1, "after a reboot: the starred flag")
    shot(ctx, e, "world_map_continue")
    d.pick_mission()
    d.wait_map()
    ctx.eq(d.mission(), 1, "The New Black picked on the map")


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
    """Means to an End: the Grand Bolt is Dual Strike's own picture
    (crate::grand_bolt: its cells AW2's underlay, drawn with its tiles in BG
    palette 7, its colours there), its three weak points its parts (a
    minicannon on tile 0x194, no Black Obelisk); on Black Hole's turn of
    every sixth day each weak point standing spawns an Oozium below it; the
    mission goes on until they are destroyed."""
    e, g, d = boot(ctx)
    d.start(step=27)
    d.wait_map()
    ctx.eq(d.mission(), 24, "Means to an End")
    inv = [(e.u8(0x02028360 + 8 * k), e.u8(0x02028361 + 8 * k), (e.u16(0x02028362 + 8 * k) >> 6) & 15)
           for k in range(16)]
    ctx.eq(sorted(i for i in inv if (i[0], i[1]) in [(3, 9), (9, 11), (15, 9)]), [(3, 9, 4), (9, 11, 4), (15, 9, 4)],
           "the three weak points: the Grand Bolt's parts")
    ctx.eq([i for i in inv if i[2] == 3], [], "no Black Obelisk")
    rows = lambda y: e.u16(dc.MAP + 0x417A + 2 * y)
    ctx.eq([e.u16(dc.MAP + 0xA22 + 2 * (rows(y) + x)) for x, y in [(3, 9), (9, 11), (15, 9)]], [0x194] * 3, "on tile 0x194")
    ctx.eq(e.u16(dc.MAP + 0xA22 + 2 * (rows(6) + 9)), 0x1A4, "the dome: AW2's underlay")
    g.goto(9, 6)
    e.wait(30)
    buf = e.u32(0x08499584)
    pals = {e.u16(buf + 2 * k) >> 12 for k in range(0x400)}
    ctx.check(7 in pals, f"drawn in BG palette 7 ({sorted(pals)})")
    shot(ctx, e, "grand_bolt_drawn")
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


@test(modes=("ds",))
def ds_campaign_grand_bolt_blocks(ctx):
    """Means to an End: no unit enters the Grand Bolt's cells (AW2's
    underlay): an infantry put just below the dome reaches none
    of them; the terrain panel on a dome cell reads Dual Strike's "Blocked",
    on a weak point "G Bolt", each with the cell's picture in OBJ palette 15
    (the Grand Bolt's colours, put back when the cursor leaves). (The
    infantry's place is a test aid.)"""
    e, g, d = boot(ctx)
    d.start(step=27)
    d.wait_map()
    d.wait_control()
    rows = lambda y: e.u16(dc.MAP + 0x417A + 2 * y)
    tile = lambda x, y: e.u16(dc.MAP + 0xA22 + 2 * (rows(y) + x))
    bolt = {(x, y) for y in range(12) for x in range(19) if tile(x, y) in (0x1A4, 0x194)}
    ctx.check(len(bolt) > 150, f"the dome's cells: {len(bolt)}")
    # (The player starts with none: Black Hole's infantry, its range shown.)
    mine = [u for u in g.units() if u["type"] == 1]
    ctx.require(mine, "an infantry")
    ctx.log(f"unit type {mine[0]['type']}")
    d.place_unit(mine[0], 5, 12)
    e.wait(4)
    g.goto(5, 12)
    g.select(5, 12)
    e.wait(20)
    reach = set()
    w, h = d.size()
    for y in range(h):
        r = e.read(dc.MAP + 0x2852 + rows(y), w)
        reach |= {(x, y) for x in range(w) if r[x] != 0xFF}
    ctx.check(reach and not (reach & bolt), f"the unit's range ({len(reach)} cells) has no cell of the dome ({sorted(reach & bolt)[:5]})")
    shot(ctx, e, "range_below_dome")
    e.press("B", 4)
    e.wait(20)
    g.goto(9, 6)
    e.wait(30)
    ctx.eq(e.u8(0x0203_0207), 5, "the panel on the dome: Blocked")
    shot(ctx, e, "panel_blocked")
    g.goto(9, 11)
    e.wait(30)
    ctx.eq(e.u8(0x0203_0207), 4, "on a weak point: G Bolt")
    shot(ctx, e, "panel_part")
    ctx.eq(e.u8(0x0203E3C0), 1, "its picture in OBJ palette 15, the Grand Bolt's colours")
    g.goto(5, 14)
    e.wait(30)
    ctx.eq(e.u8(0x0203E3C0), 0, "off the Grand Bolt: palette 15 put back")


# Dual Strike's tiles tangoAW2 converts (Com Towers, tall woods, Black
# Crystals, mega missile silos, Black Obelisks and their frames).
CONVERTED = set(range(0x1B9, 0x1BE)) | {0x146, 0x147, 0x1A1, 0x1A2, 0x1A3, 0x1A4, 0x1A6, 0x1A8, 0x1A9,
                                        0x186, 0x187, 0x188, 0x18C, 0x18D, 0x18E}
STRUCTURE_PICTURES = {"0a5": 0x080D2DA8, "0a6": 0x080D38AC}


def _every_mission(step):
    def fn(ctx):
        """Each mission in battle against the .nds: size, every tile (but
        those tangoAW2 converts) and its terrain, the deployment, fog,
        weather and look, and a 4x4 structure drawn with its own picture
        (the header names it; without one the game draws whatever OBJ VRAM
        holds there)."""
        data = dc.DsData()
        index = dc.ORDER[step]
        m = data.mission(index)
        e, g, d = boot(ctx)
        d.start(step=step)
        d.wait_map()
        label = m["name"]
        ctx.eq(d.mission(), index, label)
        ctx.eq(d.size(), (m["w"], m["h"]), f"{label}: size")
        w, h = m["w"], m["h"]
        rows = [e.u16(dc.MAP + 0x417A + 2 * y) for y in range(h)]
        tiles = [e.u16(dc.MAP + 0xA22 + 2 * (rows[y] + x)) for y in range(h) for x in range(w)]
        # (Means to an End's second-front crystals stand on its map: a
        # documented compromise, ds_campaign_means_to_an_end)
        moved = {y * w + x for x, y in MTE_CRYSTALS} if index == 24 else set()
        classes = [e.u8(dc.MAP + 0x1432 + rows[y] + x) for y in range(h) for x in range(w)]
        grand_bolt = index == 24
        bad = [(i % w, i // w) for i, (t, want) in enumerate(zip(tiles, m["tiles"]))
               if t != want and want not in CONVERTED and not (grand_bolt and t in (0x01, 0x194, 0x1A4)) and i not in moved]
        ctx.check(not bad, f"{label}: every tile as Dual Strike's ({len(bad)} differ: {bad[:6]})")
        none = [(i % w, i // w) for i, c in enumerate(classes) if c == 0]
        ctx.check(not none, f"{label}: every tile has a terrain ({len(none)} without: {none[:6]})")
        have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
        ctx.eq(have, sorted(m["units"]), f"{label}: the deployment")
        # The player has army 1 and the armies whose CO they pick; the
        # computer every other (Jake's Trial's Rachel once was the player's).
        want = [0, 0, 0, 0]
        for a in range(m["armies"]):
            want[a] = 1 if a == 0 or m["cos"][a] == 0x1C else 2
        ctx.eq(d.controllers(), want, f"{label}: who plays each army")
        ctx.eq(e.u8(ram.FOG) != 0, m["fog"] != 0, f"{label}: fog")
        want_weather = {1: 1, 2: 2}.get(m["weather"], 0)
        ctx.eq(e.u8(ram.WEATHER), want_weather, f"{label}: weather")
        if m["weather"] == 3:
            ctx.eq(e.u8(sv.WEATHER_MODE), 3, f"{label}: sandstorm (fixed weather)")
        biome = {0: looks.NORMAL, 1: looks.SNOW, 2: looks.DESERT, 3: looks.WASTELAND}[m["look"]]
        if index == 24:  # Means to an End (crate::ds_campaign_data::MEANS_TO_AN_END)
            # Dual Strike's own palette for its map (bmap/00b, crate::wasteland).
            biome = looks.GRAND_BOLT
        ctx.eq((e.u8(sv.BIOME) >> 4) & 7, biome, f"{label}: the look")
        if biome != looks.NORMAL:
            looks.check_screen(ctx, g, biome, f"{label}: ")
        header = e.u32(0x080196EC) + 0x5C * e.u8(dc.MAP_ID)
        structure = m["structure"] or ("0a5" if any(0x1AA <= t <= 0x1AD for t in m["tiles"]) else
                                       "0a6" if any(0x1AE <= t <= 0x1B1 for t in m["tiles"]) else None)
        want = STRUCTURE_PICTURES.get(structure, 0)
        ctx.eq(e.u32(header + 0x10), want, f"{label}: the structure's picture in the header")
        if want:
            picture = romlib.lz10(e.read(want, 0x1000))
            ctx.check(picture in e.read(0x06010000, 0x8000), f"{label}: the structure's picture is in OBJ VRAM")
        shot(ctx, e, "map")
        if biome != looks.NORMAL:
            # The whole map, photographed and checked cell by cell against
            # Dual Strike's drawing.
            sweep = looks.Sweep(ctx, g, biome, f"{label}: ")
            stitch(ctx, g, label, w, h, each=sweep)
            sweep.done(w, h)
    fn.__name__ = f"ds_campaign_map_{step:02d}"
    test(modes=("ds",))(fn)


for _s in range(len(dc.ORDER)):
    _every_mission(_s)


@test(modes=("ds",))
def ds_campaign_new_black_no_early_defeat(ctx):
    """The New Black: the player loses only with their own last Infantry
    (Dual Strike reads army 1's units whoever moves); Black Hole having
    none once ended the mission after the first turn."""
    e, g, d = boot(ctx)
    d.start(step=1)
    d.wait_map()
    e.w8(dc.LAST_RESULT, 0)
    d.end_turn()
    for _ in range(600):
        if e.u16(dc.DAY) >= 3 or e.u8(dc.LAST_RESULT):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(20)
    infantry = [u for u in g.units(army=1) if u["type"] == 1]
    r = d.last_result()
    if infantry:
        ctx.eq(r["result"], 0, f"no end with the Infantry alive (day {e.u16(dc.DAY)}, {r})")
    else:
        ctx.eq(r["result"], 2, "the Infantry lost: the mission is lost")
    shot(ctx, e, "day3")



@test(modes=("ds",))
def ds_campaign_com_tower_capture(ctx):
    """Ring of Fire: an Infantry finishing the capture of a Black Hole Com
    Tower takes the tower; Black Hole stays in the battle (a Com Tower is
    AW2's Lab tile, and capturing a Lab defeats its owner: in 0.4.0 it won
    the mission). The capture points are set to need one more Capture."""
    data = dc.DsData()
    m = data.mission(21)
    w = m["w"]
    towers = [(i % w, i // w) for i, t in enumerate(m["tiles"]) if 0x1B9 <= t <= 0x1BD]
    e, g, d = boot(ctx)
    d.start(step=24)
    d.wait_map()
    occupied = {(u["x"], u["y"]) for u in g.units()}
    cell = next(c for c in towers if c not in occupied)
    inf = next(u for u in g.units(army=1) if u["type"] == 1)
    d.place_unit(inf, *cell)
    a = g.unit_addr(inf["id"])
    e.w8(a + 5, (e.u8(a + 5) & 7) | (19 << 3))  # one Capture from done
    e.wait(4)
    g.select(*cell)
    e.wait(8)
    d.dialogue()
    menu = g.move_to(*cell)
    d.dialogue()
    ctx.require(any(n.lower().startswith("capt") for n in menu["names"]), f"Capture offered ({menu['names']})")
    g.choose("Capt", g.ACTION_MENU)
    e.wait(120)
    d.dialogue()
    row = e.u16(dc.MAP + 0x417A + 2 * cell[1])
    owner = e.u8(dc.MAP + 0x1432 + row + cell[0]) >> 5
    ctx.eq(owner, 1, "the tower is the player's")
    e.w8(dc.LAST_RESULT, 0)
    d.wait_control()
    d.end_turn()
    for _ in range(400):
        if e.u8(0x030033EC) == 1 and d.in_battle() and e.u16(dc.DAY) >= 2:
            break
        if d.scripts_running():
            e.press("A", 4)
        e.wait(30)
    p = d.players()
    ctx.check(d.in_battle() and d.mission() == 21 and e.u16(dc.DAY) >= 2,
              f"Ring of Fire goes on to day 2 (mission {d.mission()}, day {e.u16(dc.DAY)})")
    ctx.eq(e.u16(p + 0x3C * 2 + 0x14), 0, "Black Hole is still in the battle")
    ctx.eq(e.u8(dc.LAST_RESULT), 0, "the mission goes on")
    shot(ctx, e, "after_capture")


@test(modes=("ds",))
def ds_campaign_lab_flags_not_hard(ctx):
    """A 0.4.0 record with a lab mission open kept its flag at 0x60, AW2's
    Hard Campaign flag: every mission then used its hard deployment. The
    flag moves to 0x90 as the record loads, and Max Attacks (which has a
    hard deployment) gets its normal one."""
    data = dc.DsData()
    e, g, d = boot(ctx)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    e.w32(dc.P_MAGIC, dc.PROGRESS_MAGIC)
    e.w8(dc.P_NEXT, 2)
    e.w32(dc.P_WON, 0b11)
    e.w8(dc.PROGRESS + 0x10 + (0x60 - 0x20) // 8, 1 << ((0x60 - 0x20) % 8))
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(d.active, 900, step=10), "Continue starts")
    flags = e.read(dc.PROGRESS + 0x10, 16)
    ctx.eq((flags[(0x60 - 0x20) // 8] >> ((0x60 - 0x20) % 8)) & 1, 0, "flag 0x60 cleared")
    ctx.eq((flags[(0x90 - 0x20) // 8] >> ((0x90 - 0x20) % 8)) & 1, 1, "moved to 0x90")
    d.pick_mission()
    d.wait_map()
    have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
    ctx.eq(have, sorted(data.mission(2)["units"]), "Max Attacks: the normal deployment")


@test(modes=("ds",))
def ds_campaign_no_score_overwrite(ctx):
    """A DS mission's end leaves the event script slots alone (AW2's best
    score for map id 0xF0 would be written at 0x0200C600, the tenth slot,
    and the save prompt before the world map would wait on it)."""
    e, g, d = boot(ctx)
    d.start(new=True)
    d.wait_map()
    ctx.require(d.force_win(), "the last enemy unit destroyed")
    seen = set()
    for _ in range(400):
        e.wait(30)
        seen.add(e.u32(0x0200C600))
        if d.proc_fn_running(dc.WM_CURSOR_LOOP):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
    ctx.require(d.proc_fn_running(dc.WM_CURSOR_LOOP), "back on the world map after the save prompt")
    ctx.eq(sorted(seen), [0], "the tenth event slot untouched through the results")



@test(modes=("ds",))
def ds_campaign_map_exit(ctx):
    """B on the DS world map asks AW2's "Return to Select Mode menu?"; Yes
    ends the session: the Campaign box comes back on DS CAMPAIGN, and AW2
    CAMPAIGN's Continue shows AW2's own map with
    AW2's own progress (before the fix the session stayed on and AW2's
    Continue showed the DS map). (A save with an AW2 campaign to continue:
    the pinned one has none.)"""
    e, g, d = boot(ctx, dc.aw2_campaign_save(ctx.out))
    # AW2's map flags as its profile keeps them (the world map state is the
    # profile's last part, +0x4D0; its newest slot-0 sector in Flash).
    newest = max((e.u32(0x0E000000 + 0x1000 * k + 8), k) for k in range(16)
                 if e.read(0x0E000000 + 0x1000 * k, 4) == b"2ars" and e.u8(0x0E000000 + 0x1000 * k + 0x0D) == 0)[1]
    aw2_flags = e.read(0x0E000000 + 0x1000 * newest + 0x52 + 0x4D0 + 0x12, 0x2A)
    d.start(step=3, pick=False)
    d.wait_world_map()
    e.wait(30)
    e.press("B", 6)
    e.wait(90)
    shot(ctx, e, "prompt")
    e.press("LEFT", 6)
    e.wait(10)
    e.press("A", 6)
    e.wait(300)
    ctx.eq(e.u8(dc.ACTIVE), 0, "back in Select Mode: the session is over")
    level = e.u8(dc.MENU_LEVEL)
    ctx.check(level in (0, 2) and e.u8(dc.MENU_CHOICE) == 1, f"the Campaign box on DS CAMPAIGN (level {level})")
    if level == 2:
        e.press("B", 6)
        e.wait(40)
    d.chooser_row(0)
    e.press("A", 8)
    e.wait(40)
    e.press("A", 8)
    for _ in range(60):
        e.wait(30)
        if d.world_map_up():
            break
        if d.scripts_running():
            e.press("A", 4)
    ctx.require(d.world_map_up(), "AW2 CAMPAIGN's Continue: its map")
    e.wait(60)
    ctx.eq(e.u8(dc.ACTIVE), 0, "no DS session")
    ctx.eq(e.read(dc.WM_STATE + 0x12, 0x2A), aw2_flags, "AW2's own missions on the map")
    ctx.eq(e.u32(0x080769B4), 0x081CC5F0, "AW2's map art")
    shot(ctx, e, "aw2_map")



def _cells(e):
    rows = lambda y: e.u16(dc.MAP + 0x417A + 2 * y)
    return (lambda x, y: e.u16(dc.MAP + 0xA22 + 2 * (rows(y) + x)),
            lambda x, y: e.u8(dc.MAP + 0x1432 + rows(y) + x),
            lambda x, y: e.u8(dc.MAP + 0x12 + rows(y) + x))


def _until_army(e, d, army, frames=18000, each=None):
    """Answers dialogue with A until `army` moves (a frame budget); `each`
    is called every 20 frames. The texts shown on the way."""
    told = []
    for _ in range(frames // 20):
        t = d.text_shown()
        if t and (not told or told[-1] != t):
            told.append(t)
        if each:
            each()
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(20)
        if e.u8(0x030033EC) == army and not d.scripts_running() and d.in_battle() and not told[-1:] == [d.text_shown()]:
            break
    return told


RING_OF_FIRE_CELLS = [(0, 6), (1, 3), (2, 1), (5, 0), (8, 0), (12, 0), (19, 12), (18, 14), (17, 18), (14, 19), (10, 19), (6, 19)]
RING_OF_FIRE_CITIES = [(7, 7), (7, 12), (12, 7), (12, 12)]


@test(modes=("ds",))
def ds_campaign_ring_of_fire_volcano(ctx):
    """Ring of Fire's Volcano (docs/AW2.md): Dual Strike's structure kind 2
    becomes AW2's own Volcano (anchor 0x1A7, the game's invention kind 2),
    not a Black Obelisk; it erupts once a day (from day 3, the game's
    countdown) on Dual Strike's twelve cells
    round the map's edge (its ARM9 list for the main map), and Dual Strike's
    rule stills it once Black Hole's four cities round it are the player's
    ("we found a Black Hole unit hiding out in the city!"). (A unit is put on
    an eruption cell and the cities' owner is set here as test aids.)"""
    e, g, d = boot(ctx)
    d.start(step=24)
    d.wait_map()
    ctx.eq(d.mission(), 21, "Ring of Fire")
    tile, cls, unit_at = _cells(e)
    ctx.eq(tile(9, 10), 0x1A7, "the Volcano's anchor at Dual Strike's (9, 10)")
    ctx.eq([tile(x, 8) for x in range(8, 12)], [0x1A5] * 4, "its rim")
    inv = [(e.u8(0x02028360 + 8 * k), e.u8(0x02028361 + 8 * k), (e.u16(0x02028362 + 8 * k) >> 6) & 15) for k in range(16)]
    ctx.check((8, 8, 2) in inv, f"the game's Volcano invention (kind 2) at (8, 8): {[i for i in inv if i[2]]}")
    ctx.check(not any(k == 3 for _, _, k in inv), "no Black Obelisk")
    shot(ctx, e, "ring_of_fire_volcano.png")
    ground = lambda: [u for u in g.units(army=1) if u["type"] not in (12, 13, 16, 17, 19, 20)]
    hp = lambda u: e.u8(g.unit_addr(u["id"]) + 4) & 0x7F

    def day(watch):
        """The player's turn ended, the others played, the player's next
        turn begun (its start's eruption and rules seen): the lowest HP
        `watch` had on the way, and the texts shown."""
        seen = []
        d.wait_control()
        d.end_turn()
        told = _until_army(e, d, 1, each=lambda: seen.append(hp(watch)))
        d.wait_control()
        seen.append(hp(watch))
        return min(h for h in seen if h), told

    def probe_on_cell(k):
        free = [c for c in RING_OF_FIRE_CELLS if unit_at(*c) == 0]
        u = ground()[k]
        d.place_unit(u, *free[0])
        e.wait(4)
        return u, free[0]

    u, at = probe_on_cell(0)
    hp0 = hp(u)
    # (it first erupts as day 3 begins: the game's countdown, byte 6)
    low = min(day(u)[0], day(u)[0])
    ctx.check(low < hp0, f"the unit on {at} hit by the eruption (HP {hp0} -> {low})")
    shot(ctx, e, "ring_of_fire_erupted.png")
    # Black Hole's four cities the player's: Dual Strike's rule stills it.
    for x, y in RING_OF_FIRE_CITIES:
        row = e.u16(dc.MAP + 0x417A + 2 * y)
        e.w8(dc.MAP + 0x1432 + row + x, (1 << 5) | 6)
    _, told = day(u)
    ctx.check(any("hiding out in the city" in norm(t) for t in told), f"Dual Strike's dialogue: {[norm(t)[:40] for t in told][:4]}")
    ctx.check(e.u8(0x0203F704) == 1, "the Volcano stilled")
    u2, at2 = probe_on_cell(1)
    hp1 = hp(u2)
    low, _ = day(u2)
    ctx.eq(low, hp1, f"no eruption on {at2} any more")


@test(modes=("ds",))
def ds_campaign_world_map_edges(ctx):
    """The DS world map scrolls to Dual Strike's picture's edges (480x240 in
    AW2's 512x256 layer): the camera reaches x 240 (AW2 stops its own at
    191, which hid the picture's right side) and stops short of y 81 (AW2's
    95 showed blank rows under it)."""
    e, g, d = boot(ctx)
    d.start(step=24, pick=False)
    d.wait_world_map()
    e.wait(60)
    cam = lambda: (e.s16(0x0202FDFC), e.s16(0x0202FDFE))
    for key in ("RIGHT", "DOWN"):
        for _ in range(40):
            e.hold(key, 20)
            e.wait(4)
    x, y = cam()
    shot(ctx, e, "corner.png")
    ctx.eq(x, 240, "the camera at the picture's right edge")
    ctx.check(70 <= y <= 80, f"the camera at its bottom edge, no further ({y})")


STAR_TILE = 76   # AW2's world map LEVEL star (OBJ tile)


def ds_stars(index):
    """ds_worldmap::stars: a mission's (Normal, Hard) stars by its place in
    the campaign (Dual Strike has none of its own)."""
    step = dc.ORDER.index(index)
    normal = 1 + step * 7 // len(dc.ORDER)
    return normal, min(10, normal + 1 + step * 3 // len(dc.ORDER))


def level_stars(e):
    oam = e.read(0x07000000, 0x400)
    return sum(1 for i in range(128) if (oam[8 * i + 1] >> 0) & 3 != 2 and (oam[8 * i + 4] | oam[8 * i + 5] << 8) & 0x3FF == STAR_TILE)


@test(modes=("ds",))
def ds_campaign_world_map_stars(ctx):
    """The DS world map shows stars beside LEVEL under the cursor's mission
    as AW2's does (AW2's own code, from the DS mission table's +3 Normal /
    +4 Hard): Dual Strike has no difficulty of its own, so a mission's stars
    follow its place in the campaign over AW2's ranges (Normal 1..7, Hard up
    to 10). Normal at three points of the campaign, and Hard."""
    for step in (0, 12, 24):
        e, g, d = boot(ctx)
        d.start(step=step, pick=False)
        d.wait_world_map()
        e.wait(120)
        want = ds_stars(dc.ORDER[step])[0]
        ctx.eq(level_stars(e), want, f"step {step}: {want} stars beside LEVEL")
        shot(ctx, e, f"stars_{step}.png")
        e.close()
    e, g, d = boot(ctx)
    to_ds_box(e, d, cleared=True)
    e.press("A", 8)
    e.wait(20)
    e.press("DOWN", 6)
    e.wait(20)
    e.press("A", 8)
    for _ in range(40):
        if e.wait_until(d.active, 30, step=5):
            break
        e.press("A", 8)
    ctx.require(d.active() and session_hard(e), "a Hard campaign")
    d.wait_world_map()
    e.wait(120)
    want = ds_stars(dc.ORDER[0])[1]
    ctx.eq(level_stars(e), want, f"Hard, the first mission: {want} stars")
    shot(ctx, e, "stars_hard.png")


@test(modes=("ds",))
def ds_campaign_property_win(ctx):
    """Spiral Garden's "Whoever captures 15 properties wins" (Dual
    Strike's record +0x34, tested by its engine, not a script): the player
    owning 14 properties goes on; a 15th wins the mission (the after-action
    triggers, crate::ds_campaign_data::property_win). (Properties are given
    to the player here as a test aid; the capture that ends it is the
    pad's.)"""
    e, g, d = boot(ctx)
    d.start(step=16)
    d.wait_map()
    ctx.eq(d.mission(), 27, "Spiral Garden")
    tile, cls, unit_at = _cells(e)
    w, h = d.size()
    # (every property counts, the HQ and Com Towers too)
    props = [(x, y) for y in range(h) for x in range(w) if cls(x, y) & 0x1F in (8, 6, 0xE, 0xA, 0xB, 0x14)]
    mine = [c for c in props if cls(*c) >> 5 == 1]
    capturer = [u for u in g.units(army=1) if u["type"] in (1, 2)][0]
    # A neutral city next to the capturer's reach: the 15th.
    others = [c for c in props if cls(*c) >> 5 == 0 and unit_at(*c) == 0 and cls(*c) & 0x1F == 6]
    last = min(others, key=lambda c: abs(c[0] - capturer["x"]) + abs(c[1] - capturer["y"]))
    give = [c for c in props if cls(*c) >> 5 != 1 and c != last and unit_at(*c) == 0 and cls(*c) & 0x1F not in (8, 0x14)][:14 - len(mine)]
    for x, y in give:
        row = e.u16(dc.MAP + 0x417A + 2 * y)
        e.w8(dc.MAP + 0x1432 + row + x, (1 << 5) | (cls(x, y) & 0x1F))
    owned = lambda: sum(1 for c in props if cls(*c) >> 5 == 1)
    ctx.eq(owned(), 14, "14 properties the player's")
    d.wait_control()
    d.end_turn()
    _until_army(e, d, 1)
    ctx.eq(e.u8(dc.LAST_RESULT), 0, "14: the battle goes on")
    # The capture of the 15th, through the pad: a capturer put beside it,
    # Capt twice (two turns).
    capturer = [u for u in g.units(army=1) if u["type"] in (1, 2)][0]
    d.place_unit(capturer, *last)
    e.wait(4)
    for _ in range(3):
        if e.u8(dc.LAST_RESULT):
            break
        try:
            d.wait_control()
        except NavError:
            break
        g.select(*last)
        e.wait(8)
        m = g.move_to(*last)
        if any(n.lower().startswith("capt") for n in (g.menu() or m)["names"]):
            g.choose("Capt", g.ACTION_MENU)
        e.wait(60)
        for _ in range(200):
            if e.u8(dc.LAST_RESULT) or not d.scripts_running():
                break
            e.press("A", 4)
            e.wait(10)
        if e.u8(dc.LAST_RESULT):
            break
        try:
            d.wait_control()
        except NavError:
            break
        d.end_turn()
        _until_army(e, d, 1)
    for _ in range(300):
        if e.u8(dc.LAST_RESULT):
            break
        e.press("A", 4)
        e.wait(10)
    ctx.eq(e.u8(dc.LAST_RESULT), 1, f"15 properties: won ({owned()} owned)")


@test(modes=("ds",))
def ds_campaign_victory_or_death_bomb(ctx):
    """Victory or Death!'s Black Arc (Dual Strike's 0x02350D44: its bomb at
    (13, 5) on Black Hole's turns while its rule holds): every unit within 2
    spaces of (13, 5) but Black Hole's (and Ooziums, loaded units) is left
    with 1 HP; one further away is not touched by it. (Two units are put
    there as test aids.)"""
    e, g, d = boot(ctx)
    d.start(step=8)
    d.wait_map()
    ctx.eq(d.mission(), 8, "Victory or Death!")
    tile, cls, unit_at = _cells(e)
    land = lambda c: cls(*c) & 0x1F in (1, 2, 3, 4, 6, 8, 0xE)
    near = [c for c in [(13, 6), (12, 5), (14, 5), (13, 7), (12, 6), (14, 6), (11, 5), (15, 5)] if unit_at(*c) == 0 and land(c)]
    mine = [u for u in g.units(army=1) if u["type"] in (3, 4, 5, 8)] or g.units(army=1)
    probe = mine[0]
    d.place_unit(probe, *near[0])
    e.wait(4)
    hp0 = e.u8(g.unit_addr(probe["id"]) + 4) & 0x7F
    seen = []
    d.wait_control()
    d.end_turn()
    _until_army(e, d, 1, each=lambda: seen.append(e.u8(g.unit_addr(probe["id"]) + 4) & 0x7F))
    shot(ctx, e, "victory_or_death_bomb.png")
    ctx.check(1 in seen, f"the unit at {near[0]} left with 1 HP by the Black Arc ({hp0} -> {sorted(set(seen))[:4]})")


def map_flags_expected(e):
    """The world map's flags Dual Strike shows for the record in RAM: a won
    mission cleared (2); open (1) the first story mission not won, and each
    lab mission whose map was found (its campaign flag) and not won; every
    other mission hidden (0)."""
    won = e.u32(dc.P_WON)
    flags = e.read(dc.P_FLAGS, 16)
    out = [2 if won >> m & 1 else 0 for m in range(28)]
    story = [m for m in dc.ORDER if m not in dc.LAB_FLAGS and not won >> m & 1]
    if story:
        out[story[0]] = 1
    for m, f in dc.LAB_FLAGS.items():
        if not won >> m & 1 and flags[(f - 0x20) // 8] >> ((f - 0x20) % 8) & 1:
            out[m] = 1
    return out


def continue_after_reboot(ctx, e, name):
    """The record saved, the game rebooted, DS CAMPAIGN's Continue: the
    world map's flags. Returns them (and the new Emu)."""
    save = e.save(os.path.join(ctx.out, name))
    e.close()
    e, g, d = boot(ctx, save)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(d.active, 900, step=10), "Continue starts")
    d.wait_world_map()
    e.wait(30)
    return shown_cleared(d.map_flags()), e, d


def shown_cleared(flags):
    """The flags' shown (1) and cleared (2) bits (AW2 keeps more of its own
    in the byte: 4 on a mission revealed or cleared since, 8 on the last)."""
    return [f & 3 for f in flags]


# Every mission won as a player wins it: picked on the world map, COs picked
# on the CO screen, the battle played through the pad only (aw2test.bot with
# the mission's plan in dscampaign.PLANS; no unit, funds or flag is written),
# won by Dual Strike's own condition, then the results and the world map:
# the mission cleared and kept in the record, the campaign moved on.
WIN_DAYS = 40


# Dual Strike's two-front missions (Victory or Death!, Lightning Strikes,
# Omens and Signs, Ring of Fire, Means to an End): the user plays them by
# hand (no pad-won test while their two fronts are being built).
TWO_FRONT = {8, 10, 14, 21, 24}


def _win(step):
    def fn(ctx):
        if dc.ORDER[step] in TWO_FRONT:
            raise Skip("a two-front mission: played by hand (the user's decision)")
        e, g, d = boot(ctx)
        index = dc.ORDER[step]
        name = dc.DsData().mission(index)["name"]
        r = d.win_mission(step, WIN_DAYS, log=ctx.log)
        ctx.log(f"{name}: COs {r['cos']}, {r['days']} days, won by {r['reason']}")
        ctx.eq((r["result"], r["mission"]), (1, index), f"{name} won through the pad in {r['days']} days")
        ctx.require(r["map"], "back on the world map after the results")
        p = r["progress"]
        ctx.check(p["won"] >> index & 1, "the win is in the record")
        ctx.eq(r["flags"][index] & 2, 2, "the mission is cleared on the map")
        story = dc.story_texts(dc.DsData(), index)
        if story:
            # Dual Strike's story after this win, every text in its order,
            # after the results and before the map takes the pad.
            shown = r["texts_after"]
            k = 0
            for t in shown:
                if k < len(story) and dc.same_text(t, story[k]):
                    k += 1
            ctx.eq(k, len(story), f"Dual Strike's story after the win: {len(story)} texts in order on the world map")
        if step + 1 < len(dc.ORDER):
            shown = [m for m in range(28) if r["flags"][m] & 1 and not p["won"] >> m & 1]
            ctx.check(bool(shown), f"a next mission is open on the map ({shown})")
            ctx.check(p["next"] > step, f"the record moves on (next step {p['next']})")
        want = map_flags_expected(e)
        ctx.eq(shown_cleared(r["flags"]), want, "the map's flags: the won missions cleared, the ones the win opened, nothing locked")
        ctx.check(len(d.cleared_flags()) <= bin(p["won"]).count("1"), "starred flags only on won missions")
        shot(ctx, e, "world_map")
        if step + 1 < len(dc.ORDER):
            # (After the last, the campaign is over: the credits.)
            flags, e, d = continue_after_reboot(ctx, e, "after_win")
            ctx.eq(flags, want, "Continue after a reboot: the same flags")
            shot(ctx, e, "world_map_continue")
    fn.__name__ = f"ds_campaign_win_{step:02d}"
    test(modes=("ds",))(fn)


for _s in range(len(dc.ORDER)):
    _win(_s)



@test(modes=("ds",))
def ds_campaign_prologue(ctx):
    """New: Dual Strike's prologue (its three narration texts, bank 0x21,
    each over its picture with a narration box) on the world map before the
    player has the pad, then the map back; Continue after a reboot does not
    show it again."""
    data = dc.DsData()
    want = [data.text(r) for r in dc.PROLOGUE]
    e, g, d = boot(ctx)
    d.start(new=True, pick=False)
    seen = []
    for i in range(1500):
        t = d.text_shown()
        if t and (not seen or seen[-1] != t):
            seen.append(t)
            if len(seen) == 1:
                e.wait(80)
                shot(ctx, e, "prologue")
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and not d.scripts_running() and i > 20:
            break
        if d.scripts_running():
            e.press("A", 4)
        e.wait(15)
    ctx.eq(len(seen), len(want), "the prologue's texts")
    for k, (s, w) in enumerate(zip(seen, want)):
        ctx.check(dc.same_text(s, w), f"text {k + 1}: Dual Strike's own words ({s[:40]!r})")
    ctx.require(d.world_map_up(), "then the world map")
    e.wait(30)
    from aw2test import worldmap
    psnr, _, _ = worldmap.compare(worldmap.ds_picture(), worldmap.from_vram(e.read(0x06000000, 0x10000), e.read(0x05000000, 0x200)))
    ctx.check(psnr >= 31.0, f"the map back on its layer after the pictures ({psnr:.1f} dB)")
    ctx.check(e.u16(0x030030CC) & 0x1000, "the map's sprites back on")
    save = e.save(os.path.join(ctx.out, "after_prologue"))
    e.close()
    e, g, d = boot(ctx, save)
    d.start(new=False, pick=False)
    shown = False
    for i in range(400):
        if d.text_shown():
            shown = True
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and i > 20:
            break
        e.wait(15)
    ctx.check(not shown, "Continue: no prologue")



@test(modes=("ds",))
def ds_campaign_world_map_picture(ctx):
    """The world map's layer as the game shows it against Dual Strike's own
    picture: the fit (768 tiles with flips, nine palettes, shade-weighted
    folding, refined) keeps it within 31 dB with no 8x8 patches to speak of
    (0.4.1's first fit: 29.3 dB, colour jumps across tile edges +3.2 over
    Dual Strike's; this one 31.4 dB, +1.6), and the tiles past AW2's 704 are
    the map's (BG1's tilemap is written over them by the screen's setup)."""
    from aw2test import worldmap
    e, g, d = boot(ctx)
    d.start(step=3, pick=False)
    d.wait_world_map()
    e.wait(30)
    shown = worldmap.from_vram(e.read(0x06000000, 0x10000), e.read(0x05000000, 0x200))
    psnr, de, seam = worldmap.compare(worldmap.ds_picture(), shown)
    ctx.log(f"PSNR {psnr:.2f} dB, mean colour distance {de:.2f}, tile-edge jump {seam:+.2f}")
    ctx.check(psnr >= 31.0, f"PSNR {psnr:.2f} dB")
    ctx.check(seam <= 2.0, f"colour jumps across tile edges {seam:+.2f} over Dual Strike's")
    shot(ctx, e, "world_map")



@test(modes=("ds",))
def ds_campaign_story_music(ctx):
    """Dual Strike's own songs, converted from the .nds (crate::ds_music::
    STORY_SONGS): its opening behind the prologue, its world map song on the
    map after it (AW2's songs are in the cartridge's first 8 MB, the
    converted ones after it)."""
    BGM = 0x03005AE0
    e, g, d = boot(ctx)
    d.start(new=True, pick=False)
    during = None
    for i in range(400):
        if d.text_shown():
            e.wait(60)
            during = e.u32(BGM)
            break
        e.wait(10)
    ctx.check(during is not None and 0x08800000 <= during < 0x0A000000, f"the prologue plays a converted song (header {during or 0:#x})")
    for i in range(1500):
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and not d.scripts_running() and i > 20:
            break
        if d.scripts_running():
            e.press("A", 4)
        e.wait(15)
    e.wait(120)
    after = e.u32(BGM)
    ctx.check(0x08800000 <= after < 0x0A000000 and after != during, f"the world map plays another converted song (header {after:#x})")


# -- The staff credits (crate::ds_credits) -----------------------------------------
DS_CREDIT_SECTIONS = 0x0236A418   # overlay 5: Dual Strike's staff roll sections
OV5 = 0x02350560
AW2_PAGES = 0x0858265C            # AW2's staff roll page list
PAGE_POOLS = (0x0806BFEC, 0x0806C0B0, 0x0806C108, 0x0806C134)
CREDITS = 0x0203FD17
ROLL_FNS = (0x0806C075, 0x0806C0E5)  # the roll's page procs (typing, page time)


def ds_credit_lines():
    """Every line of Dual Strike's staff roll, read from the .nds (overlay 5),
    in order: (kind, text), 1 a name, 2/3 headings."""
    import struct
    rom = open(paths.ds_rom(), "rb").read()
    ovt = struct.unpack_from("<I", rom, 0x50)[0]
    fat = struct.unpack_from("<I", rom, 0x48)[0]
    fid = struct.unpack_from("<I", rom, ovt + 32 * 5 + 0x18)[0]
    a, b = struct.unpack_from("<II", rom, fat + 8 * fid)
    ov5 = rom[a:b]
    u32 = lambda x: struct.unpack_from("<I", ov5, x - OV5)[0]
    text = lambda x: ov5[x - OV5:ov5.index(b"\0", x - OV5)].decode("latin-1")
    out, at = [], DS_CREDIT_SECTIONS
    while u32(at):
        p = u32(at)
        while True:
            k = u32(p)
            if k == 0:
                p += 4
            elif k in (1, 2, 3):
                out.append((k, text(u32(p + 4))))
                p += 8
            else:
                break
        at += 4
    return out


def roll_lines(e, pages):
    """The lines of the page list the roll reads: (kind, text), 1 a heading, 2
    a name."""
    out, k = [], 0
    while True:
        page = e.u32(pages + 4 * k)
        if not page:
            return out
        for s in range(6):
            kind, ptr = e.u32(page + 8 * s), e.u32(page + 8 * s + 4)
            if kind:
                t = e.read(ptr, 40)
                out.append((kind, t[:t.index(0)].decode("latin-1")))
        k += 1


@test(modes=("ds",))
def ds_campaign_credits(ctx):
    """Means to an End won (a test aid ends it: the battle routed), its
    ending scenes, then Dual Strike's staff credits in AW2's staff roll: every
    line of Dual Strike's roll (read from the .nds) in its order, headings
    between stars and split when wide, Dual Strike's staff roll music (a
    converted song, not AW2's), the copyright screen, then Select Mode with
    the session over and AW2's own pages back."""
    e, g, d = boot(ctx)
    data = dc.DsData()
    d.start(step=27)
    d.choose_cos(dc.co_picks(data, dc.ORDER[27]), dc.CO_PREFS)
    d.autoplay(max_days=2)
    e.w8(d.players() + 0x3C + 0x1B, 1)
    for _ in range(600):
        if d.in_battle() and e.u8(0x030033EC) == 1 and not d.scripts_running() and g.idle():
            break
        e.wait(10)
    ctx.require(d.force_win(), "Means to an End won (test aid)")
    for f in range(80000):
        if e.u8(CREDITS) >= 3:
            break
        if f % 20 == 0 and (e.u8(CREDITS) == 0 or not d.world_map_up()):
            e.press("A", 4)
        e.wait(1)
    ctx.require(e.u8(CREDITS) == 3, "the map left for the credits after the ending")
    pages = e.u32(PAGE_POOLS[0])
    ctx.check(pages != AW2_PAGES and all(e.u32(a) == pages for a in PAGE_POOLS), f"the roll reads Dual Strike's pages ({pages:#x})")
    got = roll_lines(e, pages)
    want = ds_credit_lines()
    # Names: AW2 names (an apostrophe is AW2's '~'), every one in order.
    names_want = [t.replace("'", "~") for k, t in want if k == 1]
    ctx.eq([t for k, t in got if k == 2], names_want, f"every name of Dual Strike's roll, in order ({len(names_want)})")
    # Headings: between stars, a wide one in two lines (the heading lines
    # of a page joined back here); a section over two pages repeats them.
    heads, run = [], []
    for k, t in got + [(0, "")]:
        if k == 1:
            run.append(t.strip("*"))
        elif run:
            heads.append(" ".join(run))
            run = []
    heads_want = [t for k, t in want if k != 1]
    joined = " / ".join(heads)
    ctx.check(all(h in joined for h in heads_want) and all(t.startswith("*") and t.endswith("*") for k, t in got if k == 1),
              f"every heading, between stars ({len(set(heads_want))})")
    ctx.check(all(len(t) <= 21 for k, t in got), "every line fits the page")
    music = set()
    rolled = False
    for k in range(200):
        e.wait(60)
        fns = {e.u32(0x0200D610 + 0x6C * j + 0x10) for j in range(32)}
        if fns & set(ROLL_FNS):
            rolled = True
            music.add(e.u32(0x03005AE0))
        if k in (6, 40):
            shot(ctx, e, f"credits_{k}.png")
        if rolled and any(e.u32(0x0200D610 + 0x6C * j) in dc.WHEELS for j in range(32)):
            break
    ctx.check(rolled, "the roll ran")
    ctx.check(music and all(m >= 0x09000000 for m in music), f"Dual Strike's staff roll music (headers {[hex(m) for m in music]})")
    e.wait(60)
    ctx.eq(e.u8(dc.ACTIVE), 0, "back on Select Mode: the session is over")
    ctx.check(all(e.u32(a) == AW2_PAGES for a in PAGE_POOLS), "AW2's own pages back")
    ctx.eq(e.u8(dc.P_NEXT + 1), 1, "the campaign recorded as over")


# -- Hard Campaign ---------------------------------------------------------------
DS_FLAGS = 0x0203FD20             # the session's campaign flags 0x20..0x9F
LEVEL_HARD_CHOICE = 3             # campaign_menu: the Normal / Hard choice
HELP_WORDS = (0x08613140, 0x08613144)  # the box's help lines' text table words
AW2_HELP = [0x08607684, 0x086076A8]
RANK_POOL = 0x0807758C            # the map panel's results table word


def session_hard(e):
    f = dc.HARD_FLAG - 0x20
    return (e.u8(DS_FLAGS + f // 8) >> (f % 8)) & 1


def to_ds_box(e, d, cleared):
    """Select Mode, Campaign, DS CAMPAIGN: the DS box; with `cleared` the
    record says a Normal campaign was cleared (a test aid: the record's
    clears byte, as Means to an End's win sets it)."""
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    if cleared:
        e.w32(dc.P_MAGIC, dc.PROGRESS_MAGIC)
        e.w8(dc.P_CLEARS, 1)
    d.box_row(1)
    e.wait(20)


@test(modes=("ds",))
def ds_campaign_hard_locked(ctx):
    """Before a Normal campaign is cleared, DS CAMPAIGN's New starts a
    Normal campaign directly (no Normal / Hard choice), with the normal
    deployment."""
    data = dc.DsData()
    e, g, d = boot(ctx)
    to_ds_box(e, d, cleared=False)
    e.press("A", 8)
    levels = set()
    for _ in range(60):
        levels.add(e.u8(dc.MENU_LEVEL))
        if d.active():
            break
        e.wait(10)
        if not d.active():
            e.press("A", 8)
    ctx.require(d.active(), "New starts the DS Campaign")
    ctx.check(LEVEL_HARD_CHOICE not in levels, "no Normal / Hard choice")
    ctx.eq((e.u8(dc.P_HARD), session_hard(e)), (0, 0), "a Normal campaign")


@test(modes=("ds",))
def ds_campaign_hard(ctx):
    """Once a Normal campaign has been cleared, DS CAMPAIGN's New asks
    Normal or Hard (the chooser's style: two labels, the cursor, A takes
    it, B goes back). Hard: AW2's Hard Campaign flag in the session, Jake's
    Trial on Dual Strike's hard map with its hard deployment; Continue after
    a reboot is Hard again. Normal from the same choice is Normal."""
    data = dc.DsData()
    e, g, d = boot(ctx)
    to_ds_box(e, d, cleared=True)
    e.press("A", 8)
    e.wait(20)
    ctx.require(e.u8(dc.MENU_LEVEL) == LEVEL_HARD_CHOICE, "New asks Normal or Hard")
    ctx.check(e.u32(HELP_WORDS[0]) != AW2_HELP[0] and e.u32(HELP_WORDS[1]) != AW2_HELP[1], "the choice's help lines")
    shot(ctx, e, "normal_or_hard.png")
    tiles = e.read(0x06010000 + 832 * 32, 72 * 32)
    e.press("B", 6)
    e.wait(20)
    ctx.eq(e.u8(dc.MENU_LEVEL), 2, "B: back to the DS box")
    e.wait(2)
    ctx.eq([e.u32(w) for w in HELP_WORDS], AW2_HELP, "AW2's help lines back")
    d.box_row(1)
    e.press("A", 8)
    e.wait(20)
    e.press("DOWN", 6)
    e.wait(20)
    shot(ctx, e, "hard_highlighted.png")
    ctx.check(e.read(0x06010000 + 832 * 32, 72 * 32) == tiles, "the labels stay drawn")
    e.press("A", 8)
    for _ in range(40):
        if e.wait_until(d.active, 30, step=5):
            break
        e.press("A", 8)
    ctx.require(d.active(), "Hard: the session starts")
    ctx.eq((e.u8(dc.P_HARD), session_hard(e)), (1, 1), "a Hard campaign (the record and AW2's flag)")
    d.wait_world_map()
    d.pick_mission()
    d.wait_map()
    m = data.mission(0, hard=True)
    ctx.eq(d.size(), (m["w"], m["h"]), "Jake's Trial: the hard map's size")
    have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
    ctx.eq(have, sorted(m["units"]), "Jake's Trial: the hard deployment")
    ctx.check(sorted(m["units"]) != sorted(data.mission(0)["units"]), "(which is not the normal one)")
    shot(ctx, e, "jakes_trial_hard.png")
    # Continue after a reboot: Hard again.
    save = e.save(os.path.join(ctx.out, "hard"))
    e2, g2, d2 = boot(ctx, save)
    d2.start(new=False, pick=False)
    ctx.eq((e2.u8(dc.P_HARD), session_hard(e2)), (1, 1), "Continue: Hard again")
    # Normal from the choice.
    e3, g3, d3 = boot(ctx)
    to_ds_box(e3, d3, cleared=True)
    e3.press("A", 8)
    e3.wait(20)
    e3.press("A", 8)
    for _ in range(40):
        if e3.wait_until(d3.active, 30, step=5):
            break
        e3.press("A", 8)
    ctx.eq((e3.u8(dc.P_HARD), session_hard(e3)), (0, 0), "Normal from the choice: a Normal campaign")


@test(modes=("ds",))
def ds_campaign_records(ctx):
    """A won mission's result is the DS Campaign's own record (AW2's layout:
    score, days, CO; Normal's word), shown by the world map's mission panel
    (its results table is the DS Campaign's in a session), saved with the
    progress and back after a reboot; AW2's own results untouched."""
    e, g, d = boot(ctx)
    aw2_results = e.read(0x0200C2D0, 0x150)
    d.start(step=0)
    d.wait_map()
    ctx.require(d.force_win(), "Jake's Trial won (test aid)")
    for _ in range(3000):
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and not d.scripts_running():
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(10)
    e.wait(60)
    rec = e.u32(dc.RECORDS)
    ctx.check(rec >> 20 > 0 and (rec >> 8) & 0xFFF > 0, f"Jake's Trial's record: score {rec >> 20}, days {(rec >> 8) & 0xFFF}")
    ctx.eq(e.u32(dc.RECORDS + 4), 0, "(the Hard word untouched)")
    ctx.eq(e.u32(RANK_POOL), dc.RECORDS, "the panel reads the DS records")
    # Dual Strike shows no rank on its map (its map graphics have none; the
    # rank is on the results screen and the mission's panel): no letter.
    ranks = [t for t in oam_tiles(e) if 904 <= t < 912]
    ctx.eq(ranks, [], "no rank letter on the map")
    shot(ctx, e, "map_after_win.png")
    ctx.eq(e.read(0x0200C2D0, 0x150), aw2_results, "AW2's results untouched")
    save = e.save(os.path.join(ctx.out, "records"))
    e2, g2, d2 = boot(ctx, save)
    d2.start(new=False, pick=False)
    ctx.eq(e2.u32(dc.RECORDS), rec, "the record after a reboot")


@test(modes=("ds",))
def ds_campaign_means_to_an_end(ctx):
    """Means to an End on one front (docs/AW2.md): Dual Strike's second
    front's three Black Crystals stand on the main map, in the north, each
    in its weak point's column; a weak point whose crystal stands takes no
    damage (its force field); a crystal shattered plays Dual Strike's
    "We have shattered one of the black crystals!" and opens its weak
    point; every crystal shattered wins. Its day limit is 36 (the header's
    counter; Black Hole's win on day 36, not 24). (The crystals and the
    weak point's hits are set here as a test aid; the legitimate win is
    ds_campaign_win_27.)"""
    e, g, d = boot(ctx)
    d.start(step=27)
    ctx.eq(e.u16(DS_TABLE + 0x5C * dc.DS_MAP_ID + 0x24), MTE_DAYS, "the day limit: 36")
    d.wait_map()
    ctx.eq(d.mission(), 24, "Means to an End")
    shot(ctx, e, "means_to_an_end_start.png")
    rows = lambda y: e.u16(dc.MAP + 0x417A + 2 * y)
    tile = lambda x, y: e.u16(dc.MAP + 0xA22 + 2 * (rows(y) + x))
    ctx.eq([tile(x, y) for x, y in MTE_CRYSTALS], [0x192] * 3, "three Black Crystals on the map")
    inv = lambda: [(e.u8(0x02028360 + 8 * k), e.u8(0x02028361 + 8 * k), (e.u16(0x02028362 + 8 * k) >> 6) & 15, 0x02028360 + 8 * k)
                   for k in range(16)]
    crystals = {(x, y): a for x, y, k, a in inv() if k == 4 and (x, y) in MTE_CRYSTALS}
    obelisks = {(x, y): a for x, y, k, a in inv() if k == 4 and (x, y) in [(3, 9), (9, 11), (15, 9)]}
    ctx.eq(sorted(crystals), sorted(MTE_CRYSTALS), "each crystal an invention (the game's own)")
    west = obelisks[(3, 9)]
    hp = e.u8(west + 4)
    e.w8(west + 4, hp - 30)
    e.wait(2)
    ctx.eq(e.u8(west + 4), hp, "the west weak point's crystal stands: no damage")
    g.goto(3, 3)
    e.wait(20)
    shot(ctx, e, "crystals.png")
    # The west crystal shattered.
    e.w8(crystals[(3, 3)] + 4, 0)
    d.end_turn()
    told = []
    for _ in range(900):
        t = d.text_shown()
        if t and (not told or told[-1] != t):
            told.append(t)
        if d.scripts_running():
            if t and "shattered" in norm(t):
                shot(ctx, e, "crystal_shattered.png")
            e.press("A", 4)
        e.wait(20)
        if e.u8(0x030033EC) == 1 and told:
            break
    ctx.check(any("shattered" in norm(t) for t in told), f"Dual Strike's dialogue: {[norm(t)[:50] for t in told][:3]}")
    hp = e.u8(west + 4)
    e.w8(west + 4, hp - 30)
    e.wait(2)
    ctx.eq(e.u8(west + 4), hp - 30, "the west weak point open: it takes damage")
    centre = obelisks[(9, 11)]
    hp = e.u8(centre + 4)
    e.w8(centre + 4, hp - 30)
    e.wait(2)
    ctx.eq(e.u8(centre + 4), hp, "the centre one still closed")
    # Every crystal shattered: the win.
    for c in MTE_CRYSTALS:
        e.w8(crystals[c] + 4, 0)
    e.w8(dc.LAST_RESULT, 0)
    d.wait_control()
    d.end_turn()
    for _ in range(900):
        if e.u8(dc.LAST_RESULT):
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(20)
    ctx.eq(e.u8(dc.LAST_RESULT), 1, "every crystal shattered: won")
