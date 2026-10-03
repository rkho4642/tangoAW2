"""Crystal Calamity's Black Onyx (crate::onyx), as Dual Strike has it.

Dual Strike (melonDS; scratchpad onyxtimer/ds): the mission's first script
starts a 50-minute countdown (op 0x5A, 180000 frames, MM:SS on the top
screen); the satellite's laser charges one a frame while the countdown runs
(36000: ten minutes), warns at 90% ("Yo! Check out Black Onyx!" the first
time), fires when full (8 HP within 2 squares of Black Hole's best spot,
"What happened?!" the first time on Normal, the charge emptied); a missile
silo's Launch hits the satellite instead of the map (the silo spent, the
charge emptied); nine hits destroy it ("It can't be..."), which stops the
countdown; the countdown at 0 is the mission lost (its barrier field up,
Black Hole wins). Here the same, on AW2's map: the state in RAM (onyx::STATE),
a panel on the map, the laser AW2's meteor strike drawn as Dual Strike's (its beam,
flash, shake and rings: crate::power_anim)."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, saves
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

CRYSTAL_CALAMITY = 18
# crate::onyx's RAM.
STATE = 0x0203FFC8
HITS = STATE + 1
PHASE = STATE + 2
CHARGE = STATE + 6
CLOCK = STATE + 8
# crate::power_anim's state while the strike plays: kind (3 the laser),
# the target, frames played; the display shadows.
STRIKE = 0x0203F7A0
BLDCNT = 0x030030E0
DISPCNT = 0x030030CC
COUNTDOWN = 0x0203FD18
FULL = 36000
CHARGING, WARNING, FIRING, HIT, DESTROYED, GONE = 1, 2, 3, 4, 5, 0
# The panel's sprites: the satellite (OBJ tile 0x1F9, palette 15).
SAT_TILE = 0x1F9
OAM = 0x07000000
# The map's nine missile silos (tile 0x180; spent: 0x1A0), Dual Strike's.
SILOS = [(6, 2), (18, 10), (19, 11), (18, 12), (3, 16), (1, 18), (3, 19), (11, 19), (7, 20)]
SILO, SPENT = 0x180, 0x1A0
TILES = dc.MAP + 0xA22
ROWS = dc.MAP + 0x417A
LOCAL_FLAGS = 0x030033F4


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def start(ctx, hard=False):
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(CRYSTAL_CALAMITY), hard=hard)
    d.wait_map()
    d.wait_control()
    return e, g, d


def tile(e, x, y):
    return e.u16(TILES + 2 * (e.u16(ROWS + 2 * y) + x))


def charge(e):
    return e.u16(CHARGE)


def sprites(e, tile_no):
    """Shown sprites drawing OBJ tile `tile_no`: (x, y, palette)."""
    oam = e.read(OAM, 0x400)
    out = []
    for i in range(128):
        a0 = oam[8 * i] | oam[8 * i + 1] << 8
        a1 = oam[8 * i + 2] | oam[8 * i + 3] << 8
        a2 = oam[8 * i + 4] | oam[8 * i + 5] << 8
        if (a0 >> 8) & 3 == 2:
            continue
        if a2 & 0x3FF == tile_no and (a0 & 0xFF) < 160:
            out.append((a1 & 0x1FF, a0 & 0xFF, a2 >> 12))
    return out


def obj_sprites(e):
    """Shown sprites: (x, y, palette)."""
    oam = e.read(OAM, 0x400)
    out = []
    for i in range(128):
        a0 = oam[8 * i] | oam[8 * i + 1] << 8
        a1 = oam[8 * i + 2] | oam[8 * i + 3] << 8
        a2 = oam[8 * i + 4] | oam[8 * i + 5] << 8
        if (a0 >> 8) & 3 != 2 and (a0 & 0xFF) < 160:
            out.append((a1 & 0x1FF, a0 & 0xFF, a2 >> 12))
    return out


def wait_text(e, d, want, frames=600):
    """Presses through dialogue until a box shows `want`; the texts seen."""
    seen = []
    for _ in range(frames // 10):
        t = d.text_shown()
        if t and (not seen or seen[-1] != t):
            seen.append(t)
        if t and want in t.replace("\r", " "):
            return True, seen
        if d.scripts_running():
            e.press("A", 4)
        e.wait(6)
    return False, seen


def silo_infantry(e, g, d, at):
    """One of army 1's units made an Infantry, ready on the silo at `at`."""
    u = g.units(1)[0]
    a = g.unit_addr(u["id"])
    e.w8(a, 1)
    e.w8(a + 1, 0)
    if g.unit_at(*at):
        d.remove_unit(g.unit_at(*at))
    d.place_unit(u, *at)
    e.wait(5)
    return u


def launch(e, g, d, at):
    g.select(*at)
    names = g.move_to(*at)["names"]
    g.choose("Launch", g.ACTION_MENU)
    return names


@test(modes=("ds",))
def ds_onyx_clock(ctx):
    """The 50 minutes and the laser's charge: the countdown runs from the
    mission's first script (180000 frames, Dual Strike's op 0x5A), both count
    a frame each while the map runs (a menu open too), stop while a script
    shows its dialogue; the panel shows the satellite, the time left, nine
    hits and the charge."""
    e, g, d = start(ctx)
    ctx.eq(e.u8(STATE), 1, "the satellite is in the battle")
    ctx.eq(e.u8(HITS), 9, "nine hits to take")
    n0, c0 = e.u32(COUNTDOWN), charge(e)
    ctx.check(170000 < n0 <= 180000, f"the countdown runs from 50 minutes ({n0})")
    ctx.eq(n0 + c0, 180000, "the charge counts the countdown's frames")
    e.wait(300)
    n1, c1 = e.u32(COUNTDOWN), charge(e)
    ctx.eq((n0 - n1, c1 - c0), (300, 300), "300 frames on the map: 300 off the countdown, 300 on the charge")
    sat = sprites(e, SAT_TILE)
    ctx.check(len(sat) == 1 and sat[0][2] == 15, f"the panel's satellite shows ({sat})")
    e.shot(os.path.join(ctx.out, "panel"))
    # A menu open: the clock runs (as on Dual Strike's map).
    g.open_map_menu()
    n2 = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq(n2 - e.u32(COUNTDOWN), 120, "with the map menu open the clock runs")
    # The CO screen (a full screen, as Dual Strike's): the clock stops.
    g.choose("CO", g.MAP_MENU)
    e.wait(90)
    n2 = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq(n2 - e.u32(COUNTDOWN), 0, "on the CO screen the clock stops")
    for _ in range(3):
        e.press("B", 4)
        e.wait(40)
    g.wait_for_input()
    # A dialogue (the warning's): the clock stops while it shows.
    e.w16(CHARGE, FULL * 9 // 10 - 5)
    ctx.require(e.wait_until(lambda: d.scripts_running(), 120, step=2), "the warning's dialogue")
    n3, c3 = e.u32(COUNTDOWN), charge(e)
    e.wait(200)
    ctx.eq((e.u32(COUNTDOWN), charge(e)), (n3, c3), "the clock stops during the dialogue")
    left = e.u32(COUNTDOWN) // 60
    ctx.log(f"time left {left // 60:02d}:{left % 60:02d}")


@test(modes=("ds",))
def ds_onyx_warning_and_laser(ctx):
    """At 90% the warning (Dual Strike's state 2, "Yo! Check out Black
    Onyx!" once); full, the laser: the charge emptied, an 8 HP strike within
    two squares of Black Hole's target (never below 1 HP), Dual Strike's beam
    on the map, then "What happened?!" (the first time, Normal)."""
    e, g, d = start(ctx)
    e.w16(CHARGE, FULL * 9 // 10 - 3)
    ok, seen = wait_text(e, d, "Check out Black Onyx")
    ctx.check(ok, f"the warning's dialogue ({seen})")
    ctx.eq(e.u8(PHASE), WARNING, "the warning (state 2)")
    # (the map waits for the cursor under this dialogue: the terrain box
    # stays, whole, above it; back on the map it has its darkening)
    d.wait_control()
    ctx.check(d.box_effects_left(), "back on the map: the terrain box's window and darkening on")
    e.shot(os.path.join(ctx.out, "warning"))
    before = {(u["army"], u["x"], u["y"]): u["hp"] for u in g.units()}
    e.w16(CHARGE, FULL - 3)
    ctx.require(e.wait_until(lambda: e.u8(PHASE) == FIRING, 120, step=1), "the laser fires (state 3)")
    ctx.check(charge(e) < 10, f"the charge emptied ({charge(e)})")
    # The list's script starts the laser while the terrain box shows: from
    # the next frame (when the box's tiles hide) to the strike's end, the
    # box's window is off (its dark rectangle showed through the panel's
    # beam and on the strike's first frame).
    box_on = []
    e.wait(1)

    def strike_starts():
        if d.box_window_on():
            box_on.append(e.frame)
        return e.u8(STRIKE) == 3

    # Dual Strike's beam (crate::power_anim, kind 3): after the panel's
    # beam (102 frames), on BG0 coming down to the target, additive; the
    # flash, the rings (OBJ palette 3), the fade.
    ctx.require(e.wait_until(strike_starts, 400, step=1), "the laser's strike on the map (Dual Strike's beam)")
    e.shot(os.path.join(ctx.out, "strike_first_frame"))
    at = (e.u8(STRIKE + 1), e.u8(STRIKE + 2))
    blend = set()
    seen = {}
    for k in range(200):
        if e.u8(STRIKE) != 3:
            break
        if d.box_window_on():
            box_on.append(e.frame)
        t = e.u16(STRIKE + 4)
        blend.add(e.u16(BLDCNT))
        if e.u16(BLDCNT) == 0x3E41 and e.u16(DISPCNT) & 0x100 and "beam" not in seen:
            seen["beam"] = t
            e.shot(os.path.join(ctx.out, "beam"))
        if e.u16(BLDCNT) == 0x00BF and "flash" not in seen:
            seen["flash"] = t
            e.shot(os.path.join(ctx.out, "flash"))
        if "flash" in seen and t >= seen["flash"] + 12 and "rings" not in seen and any(s[2] == 3 for s in obj_sprites(e)):
            seen["rings"] = t
            e.shot(os.path.join(ctx.out, "rings"))
        e.wait(1)
    ctx.log(f"the strike at {at}: {seen}, BLDCNT {sorted(hex(b) for b in blend)}")
    ctx.check({"beam", "flash", "rings"} <= set(seen), f"the beam (additive), the flash, the rings ({seen})")
    ctx.eq(box_on, [], "the terrain box's window off from the laser's first frame to the strike's end")
    ok, seen = wait_text(e, d, "What happened", 1200)
    ctx.check(ok, f"the first laser's dialogue ({seen})")
    d.wait_control()
    after = {(u["army"], u["x"], u["y"]): u["hp"] for u in g.units()}
    hit = {k: (before[k], after.get(k)) for k in before if after.get(k) != before[k]}
    ctx.check(bool(hit), f"units hit at {at}: {hit}")
    for (army, x, y), (b, a) in hit.items():
        ctx.check(abs(x - at[0]) + abs(y - at[1]) <= 2, f"unit at {(x, y)} within 2 squares of {at}")
        ctx.eq(a, max(1, b - 80), f"army {army}'s unit at {(x, y)}: 8 HP, never below 1")
    ctx.check(all(k[0] != 4 for k in hit), "Black Hole's own units are not its target")
    # The second laser: no dialogue.
    e.w16(CHARGE, FULL - 3)
    ctx.require(e.wait_until(lambda: e.u8(PHASE) == FIRING, 120, step=1), "the second laser")
    texts = []
    for _ in range(60):
        t = d.text_shown()
        if t:
            texts.append(t)
        e.wait(10)
    ctx.eq(texts, [], "the second laser: no dialogue")


@test(modes=("ds",))
def ds_onyx_silo_hit(ctx):
    """A silo's Launch hits the satellite: the hits left go down by one, the
    charge is emptied, the silo is spent (AW2's launch: tile 0x1A0) and the
    unit's turn is over."""
    e, g, d = start(ctx)
    ctx.eq([tile(e, x, y) for x, y in SILOS], [SILO] * 9, "the nine silos")
    at = SILOS[7]
    u = silo_infantry(e, g, d, at)
    e.wait(600)
    ctx.check(charge(e) > 500, "some charge")
    names = launch(e, g, d, at)
    ctx.check("Launch" in names, f"Launch offered on the silo ({names})")
    ctx.require(e.wait_until(lambda: e.u8(PHASE) == HIT, 300, step=2), "the missile on its way (state 4)")
    ctx.check(charge(e) < 300, f"the charge emptied ({charge(e)})")
    e.shot(os.path.join(ctx.out, "hit"))
    ctx.require(e.wait_until(lambda: e.u8(HITS) == 8, 300, step=2), "a hit landed: 8 to go")
    g.wait_for_input()
    ctx.eq(tile(e, *at), SPENT, "the silo is spent")
    ctx.eq(e.u8(g.unit_addr(u["id"]) + 1) & 1, 1, "the unit has acted")
    ctx.eq(e.u8(PHASE), CHARGING, "charging again")


@test(modes=("ds",))
def ds_onyx_destroyed(ctx):
    """The ninth hit destroys the satellite (state 5): "It can't be. I would
    never have believed Black Onyx could be destroyed.", the countdown stops
    and the panel goes; the mission goes on (the Black Obelisk wins it)."""
    e, g, d = start(ctx)
    e.w8(HITS, 1)
    at = SILOS[7]
    silo_infantry(e, g, d, at)
    launch(e, g, d, at)
    ctx.require(e.wait_until(lambda: e.u8(HITS) == 0, 400, step=2), "the last hit")
    ctx.eq(e.u8(PHASE), DESTROYED, "destroyed (state 5)")
    e.wait(110)
    e.shot(os.path.join(ctx.out, "destroyed"))
    ok, seen = wait_text(e, d, "Black Onyx could be destroyed", 2400)
    ctx.check(ok, f"its dialogue ({seen})")
    d.dialogue()
    g.wait_for_input()
    n = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq((n, e.u32(COUNTDOWN)), (0, 0), "the countdown stopped")
    ctx.require(e.wait_until(lambda: e.u8(PHASE) == GONE, 600, step=10), "the satellite gone")
    ctx.eq(sprites(e, SAT_TILE), [], "no panel")
    ctx.check(e.u8(dc.LAST_RESULT) == 0 and d.in_battle(), "the battle goes on")


@test(modes=("ds",))
def ds_onyx_time_out(ctx):
    """The 50 minutes run out: the clock set to 00:10 once, then left to run
    600 frames of the map (no more pokes); it reaches 00:00 on time and the
    defeat comes as Dual Strike's (onyx2/dst/cc): "Dude. WEAK!" and
    Rachel's advice, DEFEAT, the world map with Crystal Calamity not cleared
    and the campaign's record as before."""
    e, g, d = start(ctx)
    before = d.progress()
    e.w32(COUNTDOWN, 600)
    f0 = e.frame
    ctx.require(e.wait_until(lambda: e.u32(COUNTDOWN) <= 300, 700, step=1), "the clock runs down")
    e.shot(os.path.join(ctx.out, "five_seconds"))
    ctx.require(e.wait_until(lambda: e.u32(COUNTDOWN) == 0, 400, step=1), "the clock reaches 00:00")
    ctx.eq(e.frame - f0, 600, "00:00 after 600 frames of the map")
    ctx.eq(e.u8(CLOCK) & 2, 2, "the countdown has run out")
    e.shot(os.path.join(ctx.out, "zero"))
    r = d.follow_defeat(ctx.out)
    ctx.log(f"texts: {r['texts']}")
    ctx.check(any("Dude. WEAK!" in x for x in r["texts"]), f"Dual Strike's defeat lines ({r['texts'][:3]})")
    ctx.eq(r["box_left"], [], "no terrain box window or darkening left over the dialogue")
    ctx.check(r["banner"], "the DEFEAT banner")
    res = d.last_result()
    ctx.eq((res["result"], res["mission"]), (2, CRYSTAL_CALAMITY), "the mission lost (the last result)")
    ctx.eq(res["condition"], 0x023516C8, "on Dual Strike's condition: the 50 minutes run out")
    ctx.check(r["world_map"], "back on the world map")
    ctx.eq(d.map_flags()[CRYSTAL_CALAMITY] & 2, 0, "Crystal Calamity not cleared (no star)")
    ctx.eq(d.progress(), before, "the campaign's record as before (nothing won)")


@test(modes=("ds",))
def ds_onyx_saved_halfway(ctx):
    """A mission saved halfway (map menu > Save) keeps the satellite: after a
    reboot DS CAMPAIGN's Continue has its hits, charge and countdown as
    saved, and the clock runs on."""
    e, g, d = start(ctx)
    at = SILOS[7]
    silo_infantry(e, g, d, at)
    launch(e, g, d, at)
    ctx.require(e.wait_until(lambda: e.u8(HITS) == 8, 400, step=2), "a hit")
    g.wait_for_input()
    e.w16(CHARGE, 12345)
    ch0, n0 = charge(e), e.u32(COUNTDOWN)
    saves.suspend(g)
    hits, ch, n = e.u8(HITS), charge(e), e.u32(COUNTDOWN)
    img = saves.flash(e, os.path.join(ctx.out, "saved"))
    e.close()
    e, g, d = boot(ctx, img.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "Continue: the battle")
    g._units_base = g._players_base = None
    ctx.eq(e.u8(STATE), 1, "the satellite is back")
    ctx.eq(e.u8(HITS), hits, "its hits left")
    # (saved while the map menu's Save asked: between before and after)
    c, m = charge(e), e.u32(COUNTDOWN)
    ctx.check(ch0 <= c <= ch and n <= m <= n0 and c - ch0 == n0 - m, f"its charge and countdown as saved ({c}, {m}; before the save {ch0}, {n0}, after {ch}, {n})")
    g.wait_for_input()
    c0 = charge(e)
    e.wait(120)
    ctx.eq(charge(e) - c0, 120, "the clock runs on")
    ctx.eq(tile(e, *at), SPENT, "the spent silo stays spent")


@test(modes=("ds",))
def ds_onyx_intel(ctx):
    """The mission's objective (the world map's panel and Intel) is Dual
    Strike's: "Shatter the black obelisk to win. The satellite will fire in
    50 minutes, so you should take it out first.\""""
    data = dc.DsData()
    m = data.mission(CRYSTAL_CALAMITY)
    ctx.log(f"mission {m['name']}")
    e, g, d = start(ctx)
    g.open_map_menu()
    g.choose("Intel", g.MAP_MENU)
    e.wait(60)
    g.choose("Terms")
    e.wait(60)
    e.shot(os.path.join(ctx.out, "intel"))
    found = []
    for _ in range(12):
        t = d.text_shown()
        if t:
            found.append(t.replace("\r", " "))
        e.press("A", 4)
        e.wait(30)
    ctx.check(any("50 minutes" in t for t in found), f"Intel's objective says the 50 minutes ({found})")
