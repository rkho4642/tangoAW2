"""Reclaim the Skies' time limit (crate::onyx's countdown, without the
satellite), as Dual Strike has it.

Dual Strike (melonDS; scratchpad rel051/rts): the day-1 script after the
opening dialogue starts a 30-minute countdown (op 0x5A, 108000 frames),
shown on the top screen as MM:SS over the missile's flight; it counts the
frames the map runs (a menu open too; not the START screen); the header's
seventh list tests `0x02350900` (the countdown at 0, the same function as
Crystal Calamity's `0x023516C8`) every frame and its script (op 0x41) gives
Black Hole the win: "Noooo! The missile...", DEFEAT. Here the same on AW2's
map, the time left in a small panel under the CO window."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, saves
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

RECLAIM_THE_SKIES = 3
STATE = 0x0203FFC8
CLOCK = STATE + 8
COUNTDOWN = 0x0203FD18
ON_CLOCK = 2
TEXT_TILE = 0x2D2
SAT_TILE = 0x1F9
OAM = 0x07000000


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def start(ctx, hard=False):
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(RECLAIM_THE_SKIES), hard=hard)
    d.wait_map()
    d.wait_control()
    return e, g, d


def sprites(e, tile_no):
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


@test(modes=("ds",))
def ds_reclaim_timer_clock(ctx):
    """The 30 minutes: the countdown runs from the day-1 script (108000
    frames), a frame each while the map runs (a menu open too), stops on the
    CO screen; the time shows (no satellite)."""
    e, g, d = start(ctx)
    ctx.eq(e.u8(STATE), ON_CLOCK, "the countdown alone is on")
    n0 = e.u32(COUNTDOWN)
    ctx.check(100000 < n0 <= 108000, f"the countdown runs from 30 minutes ({n0})")
    ctx.eq(e.u8(CLOCK) & 1, 1, "the clock runs")
    e.wait(300)
    ctx.eq(n0 - e.u32(COUNTDOWN), 300, "300 frames on the map: 300 off the countdown")
    t = sprites(e, TEXT_TILE)
    ctx.check(len(t) == 1 and t[0][2] == 15, f"the time shows ({t})")
    ctx.eq(sprites(e, SAT_TILE), [], "no satellite")
    e.shot(os.path.join(ctx.out, "timer"))
    g.open_map_menu()
    n1 = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq(n1 - e.u32(COUNTDOWN), 120, "with the map menu open the clock runs")
    g.choose("CO", g.MAP_MENU)
    e.wait(90)
    n2 = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq(n2 - e.u32(COUNTDOWN), 0, "on the CO screen the clock stops")
    left = e.u32(COUNTDOWN) // 60
    ctx.log(f"time left {left // 60:02d}:{left % 60:02d}")


def run_out(ctx, e, g, d, frames_left):
    """The clock left to run its last `frames_left` frames (no more
    pokes): it reaches 00:00 on time; then the defeat as Dual Strike's
    (scratchpad rel051/rts, onyx2/dst/rts): "Oh... I didn't expect this.",
    "Noooo! The missile...", DEFEAT, the world map with the mission not
    cleared (its flag shown, not starred; the campaign's record as before)."""
    before = d.progress()
    flags0 = d.map_flags() if d.world_map_up() else None
    f0 = e.frame
    e.shot(os.path.join(ctx.out, "start"))
    ctx.require(e.wait_until(lambda: e.u32(COUNTDOWN) <= 300, frames_left + 60, step=1), "the clock runs down")
    e.shot(os.path.join(ctx.out, "five_seconds"))
    ctx.require(e.wait_until(lambda: e.u32(COUNTDOWN) == 0, 400, step=1), "the clock reaches 00:00")
    ctx.eq(e.frame - f0, frames_left, f"00:00 after {frames_left} frames of the map")
    ctx.eq(e.u8(CLOCK) & 2, 2, "the countdown has run out")
    t = sprites(e, TEXT_TILE)
    ctx.check(len(t) == 1, f"the time shows ({t})")
    e.shot(os.path.join(ctx.out, "zero"))
    r = d.follow_defeat(ctx.out)
    ctx.log(f"texts: {r['texts']}")
    ctx.check(any("I didn't expect this" in x for x in r["texts"]), f"Dual Strike's first line ({r['texts'][:2]})")
    ctx.check(any("Noooo! The missile" in x for x in r["texts"]), f"\"Noooo! The missile...\" ({r['texts'][:3]})")
    ctx.eq(r["box_left"], [], "no terrain box window or darkening left over the dialogue")
    ctx.check(r["banner"], "the DEFEAT banner")
    res = d.last_result()
    ctx.eq((res["result"], res["mission"]), (2, RECLAIM_THE_SKIES), "the mission lost (the last result)")
    ctx.check(r["world_map"], "back on the world map")
    ctx.eq(d.map_flags()[RECLAIM_THE_SKIES] & 2, 0, "Reclaim the Skies not cleared (no star)")
    ctx.eq(d.progress(), before, "the campaign's record as before (nothing won)")
    return r


@test(modes=("ds",))
def ds_reclaim_timer_time_out(ctx):
    """The 30 minutes run out: the clock set to 00:10 once, then left to
    run 600 frames of the map; Black Hole wins as in Dual Strike."""
    e, g, d = start(ctx)
    e.w32(COUNTDOWN, 600)
    run_out(ctx, e, g, d, 600)


@test(modes=("ds",))
def ds_reclaim_timer_full(ctx):
    """The whole 30 minutes from the mission's start, nothing poked: 108000
    frames of the map (less those the opening dialogue took), then the
    defeat. Logs the emulated and the real time it took."""
    import time
    e, g, d = start(ctx)
    left = e.u32(COUNTDOWN)
    ctx.check(100000 < left <= 108000, f"the countdown from 30 minutes ({left} frames left)")
    t0 = time.time()
    r = run_out(ctx, e, g, d, left)
    ctx.log(f"the full countdown: {left} frames ({left / 60 / 60:.1f} emulated minutes) in {time.time() - t0:.0f} s real time, then the defeat in {r['frames']} frames")


@test(modes=("ds",))
def ds_reclaim_timer_saved_halfway(ctx):
    """A mission saved halfway keeps its time: after a reboot DS CAMPAIGN's
    Continue has the countdown as saved, and it runs on."""
    e, g, d = start(ctx)
    e.w32(COUNTDOWN, 54321)
    n0 = e.u32(COUNTDOWN)
    saves.suspend(g)
    n = e.u32(COUNTDOWN)
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
    ctx.eq(e.u8(STATE), ON_CLOCK, "the countdown is back")
    m = e.u32(COUNTDOWN)
    ctx.check(n <= m <= n0, f"the countdown as saved ({m}; before the save {n0}, after {n})")
    g.wait_for_input()
    m0 = e.u32(COUNTDOWN)
    e.wait(120)
    ctx.eq(m0 - e.u32(COUNTDOWN), 120, "the clock runs on")
    t = sprites(e, TEXT_TILE)
    ctx.check(len(t) == 1, f"the time shows ({t})")
    e.shot(os.path.join(ctx.out, "continued"))
