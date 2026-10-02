"""Driving the game's saves for the save-integrity tests
(tests/test_save_integrity.py): Select Mode's modes, the map menu's Save
(a suspended game) and Continue, the War Room, and snapshots of a battle to
compare a resumed game with the one saved.

Everything is closed-loop on RAM, as the rest of the driver; the Flash image
is read from the console's exported save (`aw2test.saveimg`)."""

import os
import struct

from . import ram, saveimg
from .game import NavError, SELECT_MODE_CURSOR, MAP_TAB

# Select Mode's wheel (0x0300591C): UP steps +1.
VERSUS, CAMPAIGN, WAR_ROOM, SURVIVAL = 3, 4, 5, 6
DESIGN_ROOM = 2
WHEELS = (0x08616A08, 0x08616A40)
PROC_POOL = (0x0200D610, 0x0200E418)
# Select Map's list: ids per row, the last row, the cursor row and first row shown.
LIST_IDS = 0x02027F78
LIST_LAST = 0x02027FAB
LIST_ROW = 0x03005980
LIST_TOP = 0x03005990         # + the tab: the first row shown
SELECT_MAP_PROC = 0x08616C54
# gPlaySt, the weather block, players and units as a suspend saves them
# (CaptureBattleSaveState, sub_08016F38).
PLAYST = 0x03003FC0
WEATHER_BLOCK = 0x03004490
DAY = 0x03004080
CURRENT_ARMY = 0x030033EC
PLAYERS = 0x02023284         # five player blocks of 0x3C (army 0..4)
UNITS = 0x02022684           # army n at UNITS + 0x300 * (n - 1), 64 x 12
INVENTIONS = 0x02028360      # 16 x 8
GMAP = 0x0201E450
GMAP_TILES = GMAP + 0xA22    # u16 tiles, by row offset
SUSPEND_TAG = {1: 2, 2: 3, 3: 4}   # gPlaySt.gameMode -> slot (sub_08016D04)
DESIGN_SLOT = {0xB4: 5, 0xB5: 6, 0xB6: 7}   # design map id -> its slot


def procs(e):
    lo, hi = PROC_POOL
    b = e.read(lo, hi - lo)
    return {struct.unpack_from("<I", b, o)[0] for o in range(0, len(b), 0x6C)}


def flash(e, path):
    """The console's Flash now, exported to `path`.sav."""
    d, f = os.path.split(path)
    return saveimg.Image(e.save(os.path.join(d, f.replace(" ", "_"))))


# -- Select Mode ------------------------------------------------------------------
def to_select_mode(e):
    """From the boot to Select Mode's wheel."""
    e.wait(700)
    e.press("START", 8)
    e.wait(300)
    e.press("A", 8)
    if not e.wait_until(lambda: wheel(e) is not None, 300, step=10):
        raise NavError("Select Mode did not open")
    e.wait(60)


def wheel(e):
    lo, hi = PROC_POOL
    for p in range(lo, hi, 0x6C):
        if e.u32(p) in WHEELS:
            return p
    return None


def wheel_to(e, position):
    for _ in range(10):
        cur = e.u8(SELECT_MODE_CURSOR)
        if cur == position:
            e.wait(20)
            return
        e.press("UP", 8)
        e.wait(60)
    raise NavError(f"Select Mode cursor at {e.u8(SELECT_MODE_CURSOR)}, wanted {position}")


def box_row(e, row):
    """The open box's own cursor (Continue 0 / New 1) on `row`."""
    p = wheel(e)
    for _ in range(4):
        if p is not None and e.u16(p + 0x66) % 2 == row:
            return
        e.press("DOWN" if row else "UP", 6)
        e.wait(14)
    if p is None or e.u16(p + 0x66) % 2 != row:
        raise NavError(f"box row {row} not reached")


def back_to_select_mode(e, max_frames=3000):
    """Press B (and A through notices) until Select Mode's wheel is back."""
    n = 0
    while n < max_frames:
        if wheel(e) is not None and e.s16(wheel(e) + 0x64) <= 0:
            e.wait(30)
            return True
        e.press("B", 6)
        e.wait(30)
        n += 36
    return False


# -- Versus ---------------------------------------------------------------------------
def versus_select_map(g, tab, map_id):
    """Select Mode's Versus > New > the tab, down to `map_id`, A: the Teams
    screen."""
    e = g.e
    wheel_to(e, VERSUS)
    e.press("A", 8)
    e.wait(60)
    box_row(e, 1)
    e.press("A", 8)
    e.wait(150)
    if not g.press_until("LEFT", lambda: e.u8(MAP_TAB) == tab, tries=14, hold=8, settle=50):
        raise NavError(f"tab {tab} not reached (tab {e.u8(MAP_TAB)})")
    e.wait(40)
    for _ in range(40):
        if highlighted(e) == map_id:
            break
        e.press("DOWN", 8)
        e.wait(24)
    if highlighted(e) != map_id:
        raise NavError(f"map {map_id:#x} not in tab {tab} ({list(e.read(LIST_IDS, e.u8(LIST_LAST) + 1))})")
    e.press("A", 8)
    if not e.wait_until(g.on_teams, 400, step=10):
        raise NavError("Teams screen did not open")
    e.wait(40)


def highlighted(e):
    n = e.u8(LIST_LAST) + 1
    k = e.u8(LIST_ROW) + e.u8(LIST_TOP + e.u8(MAP_TAB))
    return e.u8(LIST_IDS + k) if k < n else None


def versus_start(g, humans=(1,), weather="clear", fog=False):
    g.set_teams(None, set(humans))
    g.teams_to_rules()
    g.set_rules(fog=fog, weather=weather, power=True, visuals="off")
    g.start_battle()
    g.wait_for_input()


def versus_continue(g):
    """Select Mode's Versus > Continue: the suspended game, back on its map."""
    e = g.e
    wheel_to(e, VERSUS)
    e.press("A", 8)
    e.wait(60)
    box_row(e, 0)
    e.press("A", 8)
    if not e.wait_until(lambda: e.u32(ram.MAIN_CALLBACK) == 0x08022049, 900, step=10):
        raise NavError("Continue did not bring the map back")
    g._units_base = g._players_base = None
    g.wait_for_input()


def suspend(g):
    """The map menu's Save, Yes: the game is saved and goes on. Returns the
    map menu's items as offered."""
    e = g.e
    names = g.open_map_menu()["names"]
    if "Save" not in names:
        raise NavError(f"no Save on the map menu ({names})")
    g.choose("Save", g.MAP_MENU)
    # The question's text, then Yes / No (on Yes): A once it waits.
    for _ in range(40):
        e.wait(20)
        e.press("A", 6)
        if not g.has_proc(0x08014401):
            break
    if not e.wait_until(lambda: not g.has_proc(0x08014401), 600, step=10):
        raise NavError("the save message stayed up")
    g.wait_for_input()
    return names


# -- what a suspend keeps, read back ----------------------------------------------------
def snapshot(g):
    """The battle as a suspend keeps it: day, army, gPlaySt, the weather block
    (tangoAW2's sandstorm and biome byte included), players, units, the
    inventions, the map's tiles."""
    e = g.e
    w, h = e.u16(GMAP), e.u16(GMAP + 2)
    rows = [e.u16(ram.MAP_ROW_OFFSETS + 2 * y) for y in range(h)]
    tiles = e.read(GMAP_TILES, 2 * (max(rows) + w)) if h else b""
    grid = [struct.unpack_from("<H", tiles, 2 * (rows[y] + x))[0] for y in range(h) for x in range(w)]
    units = sorted((u["army"], u["type"], u["x"], u["y"], u["hp"], u["ammo"], u["fuel"], u["flags"])
                   for u in g.units())
    playst = bytearray(e.read(PLAYST, 0x48))
    return {
        "day": e.u16(DAY), "army": e.u8(CURRENT_ARMY), "map": playst[2],
        "playst": bytes(playst), "weather": e.read(WEATHER_BLOCK, 12),
        "players": [e.read(g.players_base + 0x3C * a, 0x3C) for a in range(1, 5)],
        "units": units, "inventions": e.read(INVENTIONS, 0x80), "size": (w, h), "tiles": grid,
    }


def compare_snapshots(ctx, a, b, label):
    for k in ("day", "army", "map", "size"):
        ctx.eq(b[k], a[k], f"{label}: {k}")
    diff = [i for i in range(0x48) if a["playst"][i] != b["playst"][i]]
    ctx.check(not diff, f"{label}: gPlaySt (rules, weather, fog) the same (bytes {[hex(i) for i in diff]})")
    ctx.eq(b["weather"].hex(), a["weather"].hex(), f"{label}: weather block (sandstorm and biome byte)")
    for k in range(4):
        # +0x10 is the game's own Lab count, which its property recount (run
        # again on Continue) adds to without resetting (AW2's own; tangoAW2's
        # Com Towers are counted on the map, crate::com_tower).
        pa, pb = bytearray(a["players"][k]), bytearray(b["players"][k])
        pa[0x10] = pb[0x10] = 0
        d = [(hex(s), pa[s:t].hex(), pb[s:t].hex()) for s, t in saveimg.ranges(pa, pb)]
        ctx.check(not d, f"{label}: army {k + 1}'s player block (funds, CO, power) the same {d}")
    missing = [u for u in a["units"] if u not in b["units"]]
    extra = [u for u in b["units"] if u not in a["units"]]
    ctx.check(not missing and not extra, f"{label}: all {len(a['units'])} units the same (lost {missing}, new {extra})")
    ctx.check(a["inventions"] == b["inventions"], f"{label}: inventions the same")
    bad = [k for k in range(min(len(a["tiles"]), len(b["tiles"]))) if a["tiles"][k] != b["tiles"][k]]
    ctx.check(not bad and len(a["tiles"]) == len(b["tiles"]), f"{label}: every tile ({len(bad)} differ)")


# -- checks on images -------------------------------------------------------------------------
DS_RECORD = 15            # crate::ds_campaign::SAVE_SLOT
DS_RECORD_HEAD = 0x20 + 8 * 32   # its progress and missions' records (the skill data after them)

def expect_slots(ctx, before, after, changed, label, profile_allow=()):
    """Only the slots in `changed` differ between two images (the profile
    only in `profile_allow` and AW2's save counter); every listed sector
    passes AW2's check."""
    ctx.check(not after.problems(), f"{label}: every slot AW2 lists passes its check ({after.problems()})")
    d = saveimg.diff(before, after)
    # The DS Campaign's slot also keeps the COs' skill data (EXP, sets:
    # crate::co_skills) after its progress and records: a battle in any
    # mode may write it, its progress and records untouched.
    if DS_RECORD in d and DS_RECORD not in changed:
        old, new = before.slot(DS_RECORD), after.slot(DS_RECORD)
        # A record saved before the skill data (shorter) reads as zeros past
        # its end, as the game reads it.
        head = lambda b: bytes(b[:DS_RECORD_HEAD]).ljust(DS_RECORD_HEAD, b"\0") if b else bytes(DS_RECORD_HEAD)
        if new is not None and head(old) == head(new):
            d = {t: v for t, v in d.items() if t != DS_RECORD}
    others = {t: v for t, v in d.items() if t != 0 and t not in changed}
    ctx.check(not others, f"{label}: no other slot written ({ {saveimg.TAG_NAMES.get(t, t): v for t, v in others.items()} })")
    for t in changed:
        if t != 0:
            ctx.check(t in d, f"{label}: {saveimg.TAG_NAMES.get(t, t)} written")
    bad = saveimg.profile_changes(before, after, profile_allow)
    ctx.check(not bad, f"{label}: the profile changed only where expected (else: {saveimg.fmt_offsets(bad)})")
    return d


def c420(off, n=1):
    """A profile range of 0x0200C420's bytes (for profile_allow)."""
    return (saveimg.P_C420 + off, saveimg.P_C420 + off + n)


# Profile bytes AW2 itself writes with every save made from a battle: the
# Rules' animation and music options (sub_08034780's pair, C420 +0x0E/+0x14).
OPTIONS = (c420(0x0E), c420(0x14))
# Set by Select Mode's choice of mode (sub_0803BBA8, C420 +0x0D; read by
# nothing) and saved with the next profile: it changes whenever the base save
# was last saved from another mode (the pinned base.sav is the player's own
# save, whatever they did last), so every save made after Select Mode allows it.
MODE_BYTE = c420(0x0D)
