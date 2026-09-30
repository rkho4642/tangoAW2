"""The new COs' own Dual Strike themes (crate::ds_music): with the pack each new
CO's turn plays its song (converted from the .nds, ids from 505), human or CPU,
in two- and five-army games, and it loops; across a power and a battle scene
nothing breaks; AW2's COs keep their music. Recordings of each theme go to the
test's output folder, or to $AW2TEST_MUSIC_OUT."""

import os

from aw2test import rom as romlib
from aw2test.harness import test

NEW = ["jugger", "koal", "kindle", "vonbolt", "grimm", "javier", "sasha", "jake", "rachel"]
SONG = 0x030005CA  # the song id the game last started (sub_0803B524)
BGM = 0x03005AE0  # the CO themes' music player: song header, status
CO_TABLE = 0x086A0000  # tangoAW2's CO table with the pack (co_roster.rs)
AW2_CO_TABLE = 0x085D3DD0
ROW = 0x104
FIRST_SONG = 505
SONG_TABLE_POOL = 0x080704A0


def row_song(e, co, table=CO_TABLE):
    return e.u16(table + ROW * romlib.co_id(co) + 4)


def header_of(e, song):
    return e.u32(e.u32(SONG_TABLE_POOL) + 8 * song)


def wait_song(e, song, frames=1200):
    for _ in range(frames):
        if e.u16(SONG) == song:
            return True
        e.wait(1)
    return False


def playing(e, song):
    """The music player plays `song` and has live tracks."""
    return e.u32(BGM) == header_of(e, song) and e.u32(BGM + 4) & 0x80000000 == 0 and e.u32(BGM + 4) & 0xFFFF != 0


def channels(e):
    return e.u8(e.u32(0x03007FF0) + 6)


def out_dir(ctx):
    return os.environ.get("AW2TEST_MUSIC_OUT") or ctx.out


def two_armies(ctx, cos, humans, visuals="off"):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    return ctx.start(m, cos, humans=humans, visuals=visuals)


@test(modes=("ds",))
def new_cos_have_their_own_songs(ctx):
    g = two_armies(ctx, ["andy", "sami"], (1,))
    e = g.e
    songs = [row_song(e, co) for co in NEW]
    ctx.log(f"songs: {dict(zip(NEW, songs))}")
    ctx.eq(sorted(songs), list(range(FIRST_SONG, FIRST_SONG + 9)), "nine songs of their own, from 505")
    for co, s in zip(NEW, songs):
        h = header_of(e, s)
        ctx.check(0x08800000 <= h < 0x0A000000, f"{co}: song {s} header {h:08x} after the cartridge")
        ctx.check(1 <= e.u8(h) <= 8, f"{co}: {e.u8(h)} tracks")
    for co in range(19):
        ctx.eq(row_song(e, co), row_song(e, co, AW2_CO_TABLE), f"{romlib.co_name(co)} keeps AW2's music")
    for s in (1, 200, 219, 412, 503):
        ctx.eq(header_of(e, s), e.u32(0x0824238C + 8 * s), f"song {s} is AW2's")


@test(modes=("aw2",))
def co_music_without_the_pack(ctx):
    g = two_armies(ctx, ["andy", "sturm"], (1,))
    e = g.e
    ctx.eq(e.u32(SONG_TABLE_POOL), 0x0824238C, "AW2's song table")
    ctx.eq(e.u16(SONG), row_song(e, "andy", AW2_CO_TABLE), "Andy's music")
    ctx.eq(channels(e), 8, "8 channels mixed")


@test(modes=("ds",))
def each_new_co_turn_plays_its_song(ctx):
    """Human on army 2: ending army 1's turn starts the new CO's song; 20 s of
    it are recorded (<co>_tangoaw2.wav; Von Bolt 60 s)."""
    for co in NEW:
        g = two_armies(ctx, ["andy", co], (1, 2))
        e = g.e
        song = row_song(e, co)
        ctx.eq(e.u16(SONG), row_song(e, "andy"), f"{co}: Andy's music first")
        g.open_map_menu()
        g.choose("End", g.MAP_MENU)
        ctx.check(wait_song(e, song), f"{co}'s turn starts song {song}")
        e.audio_start()
        e.wait((60 if co == "vonbolt" else 20) * 60)
        n, rate = e.audio_end(os.path.join(out_dir(ctx), f"{co}_tangoaw2.wav"))
        ctx.check(playing(e, song), f"{co}: song {song} playing after 20 s")
        ctx.eq(channels(e), 12, f"{co}: 12 channels while it plays")
        ctx.log(f"{co}: {n / rate:.1f} s recorded")


@test(modes=("ds",))
def cpu_new_cos_play_their_songs(ctx):
    for co in NEW:
        g = two_armies(ctx, ["andy", co], (1,))
        e = g.e
        song = row_song(e, co)
        seen = []
        g.end_turn(observe=lambda gg: seen.append(wait_song(gg.e, song, 600)))
        ctx.check(seen == [True], f"the CPU {co}'s turn plays song {song}")
        ctx.check(wait_song(e, row_song(e, "andy"), 600), f"back to Andy's music after the CPU {co}")
        e.wait(2)
        ctx.eq(channels(e), 8, "8 channels with AW2's music")


@test(modes=("ds",))
def von_bolt_song_loops(ctx):
    """Von Bolt's theme reaches its loop point after about 1:40: after 3
    minutes it still plays."""
    g = two_armies(ctx, ["vonbolt", "andy"], (1,))
    e = g.e
    song = row_song(e, "vonbolt")
    ctx.check(playing(e, song), f"Von Bolt's song {song} plays")
    e.wait(180 * 60)
    ctx.check(playing(e, song), "still playing after 3 minutes (looped)")
    ctx.eq(channels(e), 12, "12 channels")


@test(modes=("ds",))
def von_bolt_power_and_battle(ctx):
    """Von Bolt's Super CO Power (AW2's Black Hole power music), a battle scene,
    then his next turn starts his song again."""
    g = two_armies(ctx, ["vonbolt", "andy"], (1,), visuals="a")
    e = g.e
    song = row_song(e, "vonbolt")
    ctx.check(playing(e, song), "Von Bolt's song")
    ctx.power(g, 1, "super")
    ctx.log(f"after the power: song {e.u16(SONG)}")
    ctx.check(e.u16(SONG) != song, "the power's music")
    ctx.attack(g, (10, 6), (10, 6), (11, 6))
    ctx.log(f"after the battle: song {e.u16(SONG)}")
    g.end_turn()
    ctx.check(wait_song(e, song, 600), "his next turn plays his song again")
    e.wait(2)
    ctx.eq(channels(e), 12, "12 channels")


@test(modes=("ds",))
def von_bolt_five_armies(ctx):
    hq = ((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19))
    m = ctx.map(hq=hq)
    m.terrain(15, 2, 0x1B4)
    for a, x in ((1, 3), (2, 26), (3, 25), (4, 4), (5, 15)):
        m.unit(a, "infantry", x, 10)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    e = g.e
    # Every army a CPU; army 1 (Black Hole) plays Von Bolt from its next turn.
    vb = romlib.co_id("vonbolt")
    e.w8(g.player(1)["addr"] + 0x1D, vb)
    ctx.log(f"armies' COs: {[g.player(a)['co'] for a in range(1, 6)]}")
    ctx.eq(g.player(1)["co"], vb, "army 1 is Von Bolt")
    song = row_song(e, "vonbolt")
    seen = []
    g.end_turn(observe=lambda gg: seen.append(gg.e.u16(SONG)))
    ctx.check(wait_song(e, song, 3000) or e.u16(SONG) == song, f"army 1's turn: Von Bolt's song {song}")
    ctx.log(f"now {e.u16(SONG)}, day's songs {seen}")
    ctx.check(e.u8(0x030033EC) == 1, "army 1's turn")
    ctx.check(not g.battle_over(), "the battle goes on")
