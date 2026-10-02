"""AW2's own campaign is AW2's: its opening (New, the story, the world
map, its first mission's card and battle start) and its ending (the ending
scenes and the staff roll, until Select Mode is back) play the same with
the Dual Strike pack as without it, and the same after a DS Campaign
session in the same boot.

Each run is traced from AW2's New press, with the same presses at the same
frames: every 20 frames the text an event script shows, the song the music
player plays (its header), the screen (the BMP: palettes and VRAM as
shown) and the game's frame count. The runs compared press New at the
same frame from the boot (straight from the title: NEW_AT; after a
session: NEW_AFTER_DS, its reference a pack-off AW2 campaign session left
the same way at the same frame), and nothing is written to RAM: the game's
counters, timers and music then match on their own. Pack off is the
reference.

(The ending is reached by marking AW2's first mission as its last, as AW2
marks its final mission, `specialProperty` 0x10 of its mission table: its
win (`force_win`, at the same frame in the runs compared) then starts the
game's own ending proc, `0x084A0A3C`. The same mark is made in every run.)"""

import os
import struct

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

BGM = 0x03005AE0
EVENT_SLOTS = 0x0200C528
AW2_MISSIONS = 0x08615194
ENDING_PROC = 0x084A0A3C
PROC_CAMPAIGN = 0x0849EB34
FRAME_COUNT = 0x03004008
# The frames New is pressed at (from the boot): straight from the title,
# and after a session (a DS Campaign session, or its reference: an AW2
# campaign session, left with Yes at LEAVE_AT in both). AW2's frame counts,
# its menus' timers and music then match in the runs compared, with no
# RAM written.
NEW_AT = 1400
NEW_AFTER_DS = 6000
LEAVE_AT = 5400
# The frame the first battle is won at (the reference's, two runs' later).
WIN_AT = {1400: 6100, 6000: 10700}


def text_id(e):
    for i in range(10):
        cur = e.u32(EVENT_SLOTS + 0x18 * i + 4)
        if not e.u32(EVENT_SLOTS + 0x18 * i) or not 0x08000010 <= cur < 0x0A000000:
            continue
        for c in (cur - 16, cur):
            if e.u32(c) in (0x19, 0x1A):
                return e.u16(c + 8)
    return None


def screen(e, path):
    with open(e.shot(path), "rb") as f:
        return f.read()


def trace(e, frames, path, seen=None, until=None):
    """(text id, song header, screen) every 20 frames, A pressed every 60.
    `seen`: a set the proc scripts running at each sample are added to;
    the trace ends where `until(seen)` holds."""
    out = []
    for f in range(0, frames, 20):
        if until is not None and until(seen):
            break
        if f % 60 == 0:
            e.press("A", 4)
            e.wait(16)
        else:
            e.wait(20)
        out.append((text_id(e), e.u32(BGM), screen(e, path), e.u32(FRAME_COUNT)))
        if seen is not None:
            seen.update(procs(e))
    return out


def save_pair(ctx, ref, got, label, k):
    """The two screens that differ (in the test's output)."""
    for name, t in (("ref", ref), ("got", got)):
        with open(os.path.join(ctx.out, f"{label.replace(' ', '_')}_{k}_{name}.bmp"), "wb") as f:
            f.write(t[k][2])


def compare(ctx, ref, got, label):
    """The traces are the same: every text, song, screen and frame count;
    else the first difference (its two screens saved)."""
    n = min(len(ref), len(got))
    bad = [k for k in range(n) if ref[k] != got[k]]
    if bad:
        k = bad[0]
        what = [name for name, a, b in zip(("text", "song", "screen", "frame count"), ref[k], got[k]) if a != b]
        save_pair(ctx, ref, got, label, k)
        ctx.check(False, f"{label}: differs at sample {k} ({20 * k} frames): {', '.join(what)} "
                  f"{ref[k][:2]} vs {got[k][:2]}; {len(bad)} samples differ ({bad[:12]})")
        return
    texts = len({t[0] for t in ref if t[0] is not None})
    ctx.check(len(ref) == len(got), f"{label}: {n} samples the same ({texts} texts, songs, screens)")


def start_aw2_new(e, d, ds, new_at, from_box=False, count=None):
    """From the title (or Campaign's box, open on the chooser): Campaign,
    (AW2 CAMPAIGN with the pack,) New; then until AW2's campaign proc runs
    (the trace starts there)."""
    if not from_box:
        d.open_campaign_box()
    if ds:
        d.chooser_row(0)
        e.press("A", 8)
        e.wait(30)
    d.box_row(1)
    e.wait(30)
    assert e.frame <= new_at, f"New is late: frame {e.frame} > {new_at}"
    e.wait(new_at - e.frame)
    if count is not None:
        e.w32(FRAME_COUNT, count)
    e.press("A", 8)
    # (over a saved AW2 campaign, the game's notice first: A)
    for _ in range(20):
        if e.wait_until(lambda: PROC_CAMPAIGN in procs(e), 60, step=1):
            break
        e.press("A", 8)


def procs(e):
    return {e.u32(p) for p in range(0x0200D610, 0x0200E418, 0x6C)}


def leave_map(e, d, leave_at):
    """From the world map: B, Yes (at frame `leave_at`): back to Select
    Mode, Campaign's box on its chooser."""
    e.wait(30)
    e.press("B", 6)
    e.wait(90)
    e.press("LEFT", 6)
    e.wait(10)
    assert e.frame <= leave_at, f"the map is late: frame {e.frame}"
    e.wait(leave_at - e.frame)
    e.press("A", 6)
    e.wait(300)
    if e.u8(dc.MENU_LEVEL) == 2:
        e.press("B", 6)
        e.wait(40)


def ds_session_first(e, d):
    """A DS Campaign session: New, the prologue (pictures, songs), the world
    map, then back to Select Mode."""
    d.start(new=True, pick=False)
    d.wait_world_map()
    leave_map(e, d, LEAVE_AT)


def aw2_session_first(e, d, cont):
    """Its reference (pack off): an AW2 campaign session left the same way
    at the same frame. With `cont`, Continue (AW2's world map entered from
    the menu, as the DS session enters it: the proc pool is left as the DS
    session leaves it, so AW2's next campaign starts its procs in the same
    slots and its world map opening runs the same); else New, its story,
    its world map (no AW2 campaign loaded, as in the DS session: the
    results' running total starts the same)."""
    d.open_campaign_box()
    d.box_row(0 if cont else 1)
    e.wait(30)
    e.press("A", 8)
    for _ in range(3000):
        if d.world_map_up() and not d.scripts_running():
            break
        e.press("A", 4)
        e.wait(10)
    leave_map(e, d, LEAVE_AT)


_SAVE = {}


def campaign_save(ctx):
    """A save with an AW2 campaign to continue (pack off, from the pinned
    save: New, the story, Mission 1 won, AW2's world map, back to Select
    Mode), made once per test run in the test's output."""
    if "path" not in _SAVE:
        e = Emu(save=paths.base_save(), ds=False)
        d = dc.DsCampaign(Game(e))
        d.open_campaign_box()
        d.box_row(1)
        e.wait(30)
        e.press("A", 8)
        for _ in range(4000):
            if d.in_battle() and e.u8(0x030033EC) == 1 and not d.scripts_running():
                break
            e.press("A", 4)
            e.wait(10)
        assert d.force_win(), "AW2's Mission 1 not won"
        for _ in range(3000):
            if d.world_map_up() and not d.scripts_running():
                break
            e.press("A", 4)
            e.wait(10)
        leave_map(e, d, e.frame + 200)
        _SAVE["path"] = e.save(os.path.join(ctx.out, "aw2_campaign"))
        e.close()
    return _SAVE["path"]


def profile(e):
    """AW2's profile in Flash: the newest slot-0 sector's payload."""
    best = None
    for s in range(16):
        h = e.read(0x0E000000 + 0x1000 * s, 0x54)
        if h[:4] == b"2ars" and h[0x0D] == 0:
            gen = struct.unpack_from("<I", h, 8)[0]
            if best is None or gen > best[0]:
                best = (gen, s, struct.unpack_from("<H", h, 0x50)[0])
    if best is None:
        return None
    return e.read(0x0E000000 + 0x1000 * best[1] + 0x52, best[2])


OPENING_FRAMES = 10000


def opening(ctx, ds, after_ds=False, new_at=NEW_AT, frames=OPENING_FRAMES):
    e = Emu(save=campaign_save(ctx) if after_ds else paths.base_save(), ds=ds)
    g = Game(e)
    d = dc.DsCampaign(g)
    if after_ds and ds:
        ds_session_first(e, d)
    elif after_ds:
        aw2_session_first(e, d, cont=True)
    start_aw2_new(e, d, ds, new_at, from_box=after_ds)
    t = trace(e, frames, os.path.join(ctx.out, f"s{int(ds)}{int(after_ds)}{new_at}"))
    active = e.u8(dc.ACTIVE)
    battle = d.in_battle() and e.u8(dc.MAP_ID) < 0xC0
    e.close()
    return t, (active, battle)


@test(modes=("ds",))
def aw2_campaign_opening_vanilla(ctx):
    """AW2 CAMPAIGN, New, with the pack and after a DS Campaign session in
    the same boot: AW2's opening (its story, world map, first mission) the
    same as without the pack, text for text, song for song, palettes and
    VRAM included."""
    ref, (_, battle) = opening(ctx, False)
    ctx.check(battle, "the trace reaches AW2's first battle")
    got, (active, _) = opening(ctx, True)
    ctx.eq(active, 0, "with the pack: no DS session")
    compare(ctx, ref, got, "with the pack")
    ref, _ = opening(ctx, False, after_ds=True, new_at=NEW_AFTER_DS)
    after, (active, _) = opening(ctx, True, after_ds=True, new_at=NEW_AFTER_DS)
    ctx.eq(active, 0, "after a DS session: none now")
    compare(ctx, ref, after, "after a DS Campaign session")


ENDING_FRAMES = 24000


def ending(ctx, ds, after_ds=False, new_at=NEW_AT):
    e = Emu(save=paths.base_save(), ds=ds)
    g = Game(e)
    d = dc.DsCampaign(g)
    if after_ds and ds:
        ds_session_first(e, d)
    elif after_ds:
        aw2_session_first(e, d, cont=False)
    # AW2's first mission marked as its last (as its final mission is).
    sp = e.u8(AW2_MISSIONS + 2)
    e.w8(AW2_MISSIONS + 2, sp | 0x10)
    start_aw2_new(e, d, ds, new_at, from_box=after_ds)
    # Through the story to the first battle, then its end.
    for _ in range(4000):
        if d.in_battle() and e.u8(0x030033EC) == 1 and not d.scripts_running():
            break
        e.press("A", 4)
        e.wait(10)
    assert e.frame <= WIN_AT[new_at], f"the battle is late: frame {e.frame}"
    e.wait(WIN_AT[new_at] - e.frame)
    won = d.force_win()
    # Until Select Mode is back after the staff roll (it has the pack's
    # Survival entry with the pack: not AW2's campaign).
    seen = set()
    back = lambda seen: ENDING_PROC in seen and any(w in procs(e) for w in dc.WHEELS)
    t = trace(e, ENDING_FRAMES, os.path.join(ctx.out, f"e{int(ds)}{int(after_ds)}{new_at}"), seen, back)
    e.close()
    return t, (won, ENDING_PROC in seen)


@test(modes=("ds",))
def aw2_campaign_ending_vanilla(ctx):
    """AW2's ending and staff roll the same with the pack (and after a DS
    Campaign session) as without it: none of the DS Campaign's scenes,
    pictures, songs, world map or choice box."""
    ref, (won, ended) = ending(ctx, False)
    ctx.require(won, "the battle won (pack off)")
    ctx.check(ended, "AW2's ending proc ran (0x084A0A3C)")
    texts = [t for t in ref if t[0] is not None]
    ctx.check(len({t[0] for t in texts}) >= 5, f"AW2's ending scenes ({len({t[0] for t in texts})} texts)")
    got, _ = ending(ctx, True)
    compare(ctx, ref, got, "with the pack")
    ref, _ = ending(ctx, False, after_ds=True, new_at=NEW_AFTER_DS)
    after, _ = ending(ctx, True, after_ds=True, new_at=NEW_AFTER_DS)
    compare(ctx, ref, after, "after a DS Campaign session")


def continue_offered(e, d):
    try:
        d.box_row(0)
        return True
    except Exception:
        return False


@test(modes=("ds",))
def aw2_campaign_kept_by_ds_session(ctx):
    """An AW2 campaign in progress survives a DS Campaign session: its
    Continue is still offered in the same boot and after a reboot, New
    still warns before overwriting it, and AW2's profile in Flash is byte
    for byte what it was. (AW2's save writer serializes the profile with
    every slot it writes, the DS record's too; the profile's last part is
    the world map state, which holds the DS map's during a session: until
    this was found the profile was written so, and the AW2 campaign lost.)"""
    save = campaign_save(ctx)
    e = Emu(save=save, ds=True)
    d = dc.DsCampaign(Game(e))
    before = profile(e)
    ctx.require(before is not None, "the save has AW2's profile")
    ds_session_first(e, d)
    ctx.eq(e.u8(dc.ACTIVE), 0, "the DS session is over")
    ctx.check(profile(e) == before, "AW2's profile in Flash unchanged by the DS session")
    d.chooser_row(0)
    e.press("A", 8)
    e.wait(30)
    ctx.check(continue_offered(e, d), "AW2 CAMPAIGN's Continue offered after the session")
    d.box_row(1)
    e.wait(20)
    e.press("A", 8)
    warned = not e.wait_until(lambda: PROC_CAMPAIGN in procs(e), 90, step=1)
    ctx.check(warned, "AW2 CAMPAIGN's New warns before overwriting it")
    after = e.save(os.path.join(ctx.out, "after_ds"))
    e.close()
    for ds in (False, True):
        e = Emu(save=after, ds=ds)
        d = dc.DsCampaign(Game(e))
        d.open_campaign_box()
        if ds:
            d.chooser_row(0)
            e.press("A", 8)
            e.wait(30)
        ctx.check(continue_offered(e, d), f"after a reboot ({'with' if ds else 'without'} the pack): AW2's Continue offered")
        e.close()
