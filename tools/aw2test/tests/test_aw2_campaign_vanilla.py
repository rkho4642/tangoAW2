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
# and after a DS Campaign session (its reference waits on the menu until
# then). AW2's frame counts, its menus' timers and the console's own state
# then match in the runs compared, with no RAM written: the world map's
# pan, palette cycles and the results' shimmer run from them.
NEW_AT = 1400
NEW_AFTER_DS = 6000
LEAVE_AT = 5400
DS_LEAVE_EARLY = 2
# After a DS session the world map's opening zoom and the spinning Mission
# Start stamp differ from the reference's by a pixel here and there for 9
# samples (180 frames, 375..383): the same pictures to the eye, texts,
# songs and frame counts equal, everything after it (Mission 1 and its
# battle) the same. The cause is not in RAM, VRAM, palettes, OAM or the
# ROM's code (copying the reference's RAM, VRAM, palettes and OAM over at
# the New press leaves it; AW2's code bytes are the pack's own after the
# session), so it is console state outside memory (the sound hardware's,
# timers) after the session's songs. This one window is allowed, nothing
# else.
MOVED = 12
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


def compare(ctx, ref, got, label, moved=0):
    """The traces are the same: every text, song, screen and frame count.
    With `moved`, the screens (only) may differ in one window of up to that
    many samples (see MOVED); texts, songs and frame counts still match
    everywhere."""
    n = min(len(ref), len(got))
    bad = [k for k in range(n) if ref[k] != got[k]]
    # (the game's frame count too: no frame lost or gained)
    hard = [k for k in bad if ref[k][:2] != got[k][:2] or ref[k][3] != got[k][3]]
    span = bad[-1] - bad[0] + 1 if bad else 0
    if hard or span > moved:
        k = (hard or bad)[0]
        what = [name for name, a, b in zip(("text", "song", "screen", "frame count"), ref[k], got[k]) if a != b]
        save_pair(ctx, ref, got, label, k)
        ctx.check(False, f"{label}: differs at sample {k} ({20 * k} frames): {', '.join(what)} {ref[k][:2]} vs {got[k][:2]}; "
                  f"{len(bad)} samples differ ({bad[:12]}), {len(hard)} in text, song or frame count")
        return
    texts = len({t[0] for t in ref if t[0] is not None})
    note = f"; screens differ in samples {bad[0]}..{bad[-1]} only (allowed: {moved})" if bad else ""
    if bad:
        save_pair(ctx, ref, got, label, bad[0])
    ctx.check(len(ref) == len(got), f"{label}: {n} samples the same ({texts} texts, songs, screens){note}")


def start_aw2_new(e, d, ds, new_at, from_box=False):
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
    e.press("A", 8)
    e.wait_until(lambda: PROC_CAMPAIGN in procs(e), 1800, step=1)


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
    # (the session's end takes Select Mode 3 frames longer to come back:
    # Yes 3 frames earlier, so its music and timers start where the
    # reference's do)
    leave_map(e, d, LEAVE_AT - DS_LEAVE_EARLY)


def aw2_session_first(e, d):
    """The reference for it (pack off): an AW2 campaign session, New, its
    story, AW2's world map, then back to Select Mode at the same frame. (Select
    Mode's music starts again there in both, and AW2's mission start waits
    on it.)"""
    d.open_campaign_box()
    d.box_row(1)
    e.wait(30)
    e.press("A", 8)
    d.wait_world_map()
    leave_map(e, d, LEAVE_AT)


OPENING_FRAMES = 10000


def opening(ctx, ds, after_ds=False, new_at=NEW_AT, frames=OPENING_FRAMES):
    e = Emu(save=paths.base_save(), ds=ds)
    g = Game(e)
    d = dc.DsCampaign(g)
    if after_ds:
        (ds_session_first if ds else aw2_session_first)(e, d)
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
    compare(ctx, ref, after, "after a DS Campaign session", moved=MOVED)


ENDING_FRAMES = 24000


def ending(ctx, ds, after_ds=False, new_at=NEW_AT):
    e = Emu(save=paths.base_save(), ds=ds)
    g = Game(e)
    d = dc.DsCampaign(g)
    if after_ds:
        (ds_session_first if ds else aw2_session_first)(e, d)
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
