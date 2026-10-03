"""Fog of war in every DS Campaign mission, Normal and Hard, against Dual
Strike's own flag.

Dual Strike's mission record (0xA0 bytes at 0x022DBD28 + 0xA0 * map id)
holds the fog at +0x1C: its battle setup (ARM9 0x020D3BC0 and 0x020E9374)
reads the byte at 0x022DBD44 + 0xA0 * map id and, when nonzero, sets the
battle's fog (+0x38 of its battle state). Checked in melonDS: Fog Rolls In
starts in fog, Jake's Trial does not, and Jake's Trial with the byte poked
to 1 does. The record has one fog byte for both difficulties (the Hard
map and deployment are +0x48/+0x50 of the same record) and each second
front its own record (ids 0xFC..0x100, all without fog).

Four missions have fog: Fog Rolls In, Verdant Hills, Into the Woods and
The Long March. Each mission is started (Normal, then Hard) and the fog
flag (0x03003FCD) must be the record's at the first turn and still after a
whole round; on a two-front mission the second front's round must have
its own record's fog."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, ram
from aw2test import twofront as tf
from aw2test.emu import Emu
from aw2test.bot import Bot
from aw2test.game import Game
from aw2test.harness import test

DS_FLAGS = 0x0203FD20             # the session's campaign flags 0x20..0x9F
# Missions whose End waits until every unit has orders (Jake's Trial .. Fog Rolls In).
WAIT_ALL = {0, 1, 2, 3, 4, 5, 6}
FOGGY = {"Fog Rolls In", "Verdant Hills", "Into the Woods", "The Long March"}


def session_hard(e):
    f = dc.HARD_FLAG - 0x20
    return (e.u8(DS_FLAGS + f // 8) >> (f % 8)) & 1


def ds_fog(data, index):
    """Dual Strike's fog byte for mission `index` (record 0xE0 + index)."""
    return data.bytes(dc.DS_RECORDS + 0xA0 * index + 0x1C, 1)[0]


def _mission_fog(step, hard):
    def fn(ctx):
        data = dc.DsData()
        index = dc.ORDER[step]
        m = data.mission(index, hard=hard)
        label = f"{m['name']}{' (Hard)' if hard else ''}"
        want = ds_fog(data, index) != 0
        ctx.eq(want, m["name"] in FOGGY, f"{label}: Dual Strike's record says fog {want}")
        e = Emu(save=paths.base_save(), ds=ctx.ds)
        g = Game(e, ctx.image)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=step, hard=hard)
        d.wait_map()
        ctx.eq(d.mission(), index, label)
        ctx.eq(session_hard(e), int(hard), f"{label}: the session's difficulty")
        ctx.eq(d.size(), (m["w"], m["h"]), f"{label}: the {'hard' if hard else 'normal'} map")
        ctx.eq(e.u8(ram.FOG) != 0, want, f"{label}: fog at the first turn")
        e.shot(os.path.join(ctx.out, "first_turn"))
        # A whole round (on a two-front mission the second front's armies on
        # its own map too), the fog sampled all through it, back to the
        # player's next turn.
        second = data.u32(dc.DS_RECORDS + 0xA0 * index + 0x10)
        seen = {}

        def each():
            if e.u8(tf.BUSY) == 0 and e.u16(tf.MAP_STATE) not in (0, 3):
                seen.setdefault(e.u8(tf.LIVE), set()).add(e.u8(ram.FOG) != 0)
        day = e.u16(dc.DAY)
        humans = [a + 1 for a, c in enumerate(d.controllers()) if c == 1]

        def other_human_waits():
            # Another army of the player's (they pick its CO) at its turn.
            return (e.u8(tf.LIVE) == 0 and e.u8(tf.BUSY) == 0 and e.u16(tf.CURRENT_ARMY) in humans[1:]
                    and e.u16(tf.MAP_STATE) == tf.CURSOR_STATE)
        for _ in range(len(humans)):
            d.wait_control()
            army = e.u16(tf.CURRENT_ARMY)
            if index in WAIT_ALL:
                # Dual Strike's early missions refuse End while a unit
                # awaits orders ("You still have units awaiting orders."):
                # the bot gives every unit its orders and ends the turn.
                Bot(d, log=ctx.log).play_turn(army)
            else:
                d.end_turn()
            e.wait(60)
            if e.u16(tf.CURRENT_ARMY) == army and e.u16(tf.MAP_STATE) != tf.CURSOR_STATE:
                e.press("A", 4)  # the map menu still up on End (the first press lost)
                e.wait(30)
            tf.until(e, d, lambda: (tf.player_turn(e) and e.u16(dc.DAY) > day) or e.u8(dc.LAST_RESULT) != 0
                     or other_human_waits(), each=each)
            if not other_human_waits():
                break
            d.wait_control()
        ctx.check(e.u16(dc.DAY) > day or e.u8(dc.LAST_RESULT), f"{label}: a whole round (day {e.u16(dc.DAY)})")
        if not e.u8(dc.LAST_RESULT):
            seen.setdefault(0, set()).add(e.u8(ram.FOG) != 0)
        ctx.eq(seen.get(0), {want}, f"{label}: the main front's fog all through the round")
        if second:
            want2 = ds_fog(data, second - 0xE0) != 0
            ctx.eq(seen.get(1), {want2}, f"{label}: the second front's fog all through its round "
                                          f"(its record, {second:#x}: {want2})")
        e.shot(os.path.join(ctx.out, "next_turn"))
    fn.__name__ = f"ds_campaign_fog_{step:02d}{'_hard' if hard else ''}"
    test(modes=("ds",))(fn)


for _s in range(len(dc.ORDER)):
    _mission_fog(_s, False)
    _mission_fog(_s, True)
