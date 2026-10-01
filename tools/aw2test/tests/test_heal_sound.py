"""The Black Crystal's and Black Obelisk's heal animations (heal_effect.rs) play
Dual Strike's heal sounds (SE_BLACKSTONE, SE_BLACKCRYSTAL, converted by
ds_music.rs to songs 514 and 515 on AW2's sound-effect player 2) as they start,
and change nothing on screen but the effect: every other sprite palette and the
tiles of every other sprite stay as they were (the effect once used OBJ palette
8, the neutral and fogged buildings' colours, and turned them dark).

Recordings and frame strips go to the test's output folder, or to
$AW2TEST_HEAL_OUT (crystal_tangoaw2.wav, obelisk_tangoaw2.wav, *_strip.png)."""

import os
import shutil

from aw2test.harness import test

CRYSTAL, OBELISK = 0x192, 0x193
STATE = 0x0203FD80  # heal_effect.rs: start clock, kind, x, y, sounded, palette, scroll, pending song
SE_PLAYER = 0x03005B20  # MusicPlayerInfo of player 2 (AW2's cannon shot plays there)
BGM_PLAYER = 0x03005AE0
MAP_POINTER = 0x08499590  # the map's state; its scroll at +4
SONG_TABLE_POOL = 0x080704A0
HEAL_SONGS = {1: 514, 2: 515}
EFFECT_PALETTE = 15
RUNS = [(0x1F9, 17), (0x2D2, 9), (0x2E4, 4), (0x2EC, 4), (0x2F4, 4), (0x2FC, 4), (0x309, 9)]
SIZES = [[(8, 8), (16, 16), (32, 32), (64, 64)], [(16, 8), (32, 8), (32, 16), (64, 32)], [(8, 16), (8, 32), (16, 32), (32, 64)]]
NAMES = {1: "crystal", 2: "obelisk"}


def ours(tile):
    return any(a <= tile < a + n for a, n in RUNS)


def out_dir(ctx):
    d = os.environ.get("AW2TEST_HEAL_OUT") or ctx.out
    os.makedirs(d, exist_ok=True)
    return d


def heal_map(ctx, armies, bh, biome=0):
    """A map with Black Hole as army `bh`, a Crystal and an Obelisk, and
    buildings of every owner (and neutral ones) round both."""
    corners = [(1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)]
    if armies == 5:
        m = ctx.map(hq=tuple(corners))
        m.terrain(15, 10, 0x1B4)  # Black Hole's HQ
        m.colours = [5, 1, 2, 3, 4]
    else:
        m = ctx.map(hq=tuple(corners[:armies]))
        colours = [1, 2, 3, 4]
        colours[bh - 1] = 5
        m.colours = [0] + colours
    m.biome = biome
    m.terrain(8, 6, CRYSTAL)
    m.terrain(18, 6, OBELISK)
    kinds = ["city", "base", "airport", "port", "city"]
    owners = [0] + list(range(1, armies + 1))
    spots = [(6, 5), (7, 5), (9, 5), (10, 5), (6, 8), (7, 8), (9, 8), (10, 8), (8, 4),
             (16, 4), (17, 4), (21, 5), (22, 7), (16, 9), (20, 9), (21, 9), (22, 4)]
    for i, (x, y) in enumerate(spots):
        k = kinds[i % len(kinds)]
        m.terrain(x, y, "city" if k == "port" else k, owners[i % len(owners)])
    m.unit(bh, "tank", 8, 7).unit(bh, "mech", 20, 10).unit(bh, "infantry", 12, 12)
    for a in range(1, armies + 1):
        if a != bh:
            m.unit(a, "infantry", 10 + a, 14)
    return m


def header_of(e, song):
    return e.u32(e.u32(SONG_TABLE_POOL) + 8 * song)


def sprites(oam):
    """The visible OAM entries: (tile, palette, size in tiles)."""
    out = []
    for i in range(128):
        a0 = oam[8 * i] | oam[8 * i + 1] << 8
        a1 = oam[8 * i + 2] | oam[8 * i + 3] << 8
        a2 = oam[8 * i + 4] | oam[8 * i + 5] << 8
        shape, size = a0 >> 14, a1 >> 14
        if a0 & 0x300 == 0x200 or shape == 3:
            continue
        w, h = SIZES[shape][size]
        if (a0 & 0xFF) >= 160 and (a0 & 0xFF) + h <= 256:
            continue
        out.append((a2 & 0x3FF, a2 >> 12, w * h // 64))
    return out


def watch(ctx, g, bh, record=False, strips=False, press_a=False):
    """Black Hole's turn start after army 1 ends its turn: each heal's sound
    and what the screen keeps. Returns the shows [(kind, x, y)].

    While an effect plays, every colour (BG and OBJ) but its palette's, and
    the tiles of every sprite but its own, must be as they were the frame
    before it drew, except what the game animates itself: colours and tiles
    seen changing from frame to frame while no effect is drawn (the
    camera's travel, between the heals and the 150 frames after them: the
    cursor's colours, the buildings' blinking yellow, units' idle frames)."""
    e = g.e
    g.goto(12, 8)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    # With fog and two people playing, each turn opens on a "Next turn"
    # screen waiting for a button (shown before the army changes): A every
    # second until it is Black Hole's turn.
    for _ in range(2000):
        if g.current_army() == bh:
            break
        if press_a:
            e.press("A")
        e.wait(10 if not press_a else 50)
    ctx.require(g.current_army() == bh, "Black Hole's turn")
    shows = []
    last_kind = 0
    base = None  # (palettes, {tile: bytes}, screenshot, scroll) the frame before the effect drew
    prev = None
    changed_colours, changed_tiles, bad_use = {}, {}, set()
    animated_colours, animated_tiles = set(), set()
    started = {}  # show number -> frame its effect first drew
    sounded_at = {}  # show number -> frames from then until its song played
    moving = []  # per show: the camera still moved when the effect first drew

    def scroll():
        return e.read(e.u32(MAP_POINTER) + 4, 4)

    recorded = set()
    recording = None
    shots = []
    bgm = None
    done = None
    for f in range(2000):
        st = e.read(STATE, 0x2E)
        kind, sounded = st[4], st[7]
        if kind and kind != last_kind:
            shows.append((kind, st[5], st[6]))
            base = None
        drawing = bool(kind and sounded)
        if drawing and base is None and prev is not None:
            base = prev
            started[len(shows)] = f
            bgm = e.u32(BGM_PLAYER)
            moving.append(base[3] != scroll())
            if record and recording is None and kind not in recorded:
                recorded.add(kind)
                e.audio_start()
                recording = (kind, f)
            if strips:
                before = os.path.join(ctx.out, f"heal_{NAMES[kind]}_before.bmp")
                shutil.copy(base[2], before)
                shots.append((f"{NAMES[kind]}_before", before))
        spr = sprites(e.read(0x07000000, 1024))
        pal = e.read(0x05000000, 1024)
        tiles = {t: e.read(0x06010000 + 32 * t, 32 * n) for t, p, n in spr if not ours(t)} if (not drawing or f % 3 == 0) else None
        if drawing and base is not None:
            k = len(shows)
            # The sound: on the effect's first frames, player 2 starts the song.
            if k not in sounded_at and f - started[k] <= 4:
                if e.u32(SE_PLAYER) == header_of(e, HEAL_SONGS[kind]) and e.u32(SE_PLAYER + 4) & 0x80000000 == 0:
                    sounded_at[k] = f - started[k]
            for c in range(512):
                if c // 16 != 16 + EFFECT_PALETTE and pal[2 * c:2 * c + 2] != base[0][2 * c:2 * c + 2]:
                    changed_colours.setdefault(c, (NAMES[kind], f - started[k], base[0][2 * c:2 * c + 2].hex(), pal[2 * c:2 * c + 2].hex()))
            for t, p, n in spr:
                if ours(t) != (p == EFFECT_PALETTE):
                    bad_use.add((t, p))
            for t, b in (tiles or {}).items():
                if t in base[1] and base[1][t] != b:
                    changed_tiles.setdefault(t, (NAMES[kind], f - started[k]))
            if strips and f - started[k] in (8, 30, 60):
                shots.append((f"{NAMES[kind]}_{f - started[k]:03d}", ctx.shot(g, f"heal_{NAMES[kind]}_{f - started[k]:03d}")))
        # The sound plays its whole sample (1.9 s, 3.1 s), past the animation,
        # until the next heal's sound takes the player over.
        if recording and (drawing and kind != recording[0] or f - recording[1] >= (115 if recording[0] == 1 else 190)):
            path = os.path.join(out_dir(ctx), f"{NAMES[recording[0]]}_tangoaw2.wav")
            frames, rate = e.audio_end(path)
            ctx.log(f"recorded {path}: {frames} frames at {rate} Hz")
            recording = None
        if not kind and last_kind:
            ctx.eq(e.u32(BGM_PLAYER), bgm, f"{NAMES[last_kind]}: the music plays on through the heal")
            after = e.read(0x05000200 + 32 * EFFECT_PALETTE, 32)
            ctx.eq(after.hex(), base[0][0x200 + 32 * EFFECT_PALETTE:0x220 + 32 * EFFECT_PALETTE].hex(),
                   f"{NAMES[last_kind]}: palette {EFFECT_PALETTE} put back")
            if strips:
                shots.append((f"{NAMES[last_kind]}_after", ctx.shot(g, f"heal_{NAMES[last_kind]}_after")))
        if not drawing:
            if prev is not None and shows:
                animated_colours.update(c for c in range(512) if pal[2 * c:2 * c + 2] != prev[0][2 * c:2 * c + 2])
                animated_tiles.update(t for t, b in tiles.items() if t in prev[1] and prev[1][t] != b)
            prev = (pal, tiles, ctx.shot(g, "heal_prev") if strips and kind else None, scroll())
        if done is None and not kind and len(shows) >= 2 and recording is None:
            done = f
        if done is not None and f - done >= 150:
            break
        last_kind = kind
        e.wait(1)
    ctx.log(f"shows {shows}, effect first drawn at {started}, songs started {sounded_at} frames later")
    ctx.eq([s[0] for s in shows], [1, 2], "a Crystal heal, then an Obelisk heal")
    ctx.eq(moving, [False, False], "each effect (and its sound) starts once the camera has stopped")
    ctx.eq(sorted(sounded_at), [1, 2], "each heal's sound starts on player 2 within 4 frames of its first frame")
    ctx.log(f"animated by the game itself: colours {sorted(divmod(c, 16) for c in animated_colours)}, tiles {sorted(animated_tiles)}")
    bad_colours = {divmod(c, 16): v for c, v in changed_colours.items() if c not in animated_colours}
    ctx.eq(bad_colours, {}, "colours ((BG palette | 16 + OBJ palette, colour): first change) other than the effect's palette unchanged while it plays")
    building_colours = {c: v for c, v in changed_colours.items() if 0x100 + 16 * 8 <= c < 0x100 + 16 * 14}
    ctx.eq({divmod(c, 16): v for c, v in building_colours.items() if c % 16 != 6}, {},
           "the buildings' palettes (OBJ 8..13) unchanged but their blinking yellow (colour 6)")
    ctx.eq({t: v for t, v in changed_tiles.items() if t not in animated_tiles}, {}, "tiles of every other sprite unchanged while it plays")
    ctx.eq(sorted(bad_use), [], "only the effect's sprites use its palette and tiles")
    ctx.check(e.u32(BGM_PLAYER) not in [header_of(e, s) for s in HEAL_SONGS.values()], "the music player never plays a heal sound")
    if strips:
        make_strips(ctx, shots)
    return shows


def make_strips(ctx, shots):
    try:
        from PIL import Image
    except ImportError:
        ctx.log("no PIL: no strips")
        return
    for name in ("crystal", "obelisk"):
        files = [(n, p) for n, p in shots if n.startswith(name) and p and str(p).endswith(".bmp")]
        if not files:
            continue
        ims = [Image.open(p).convert("RGB") for _, p in files]
        w, h = ims[0].size
        s = Image.new("RGB", (len(ims) * (w + 2), h), (255, 0, 255))
        for i, im in enumerate(ims):
            s.paste(im, (i * (w + 2), 0))
        s = s.resize((s.width * 2, s.height * 2), Image.NEAREST)
        path = os.path.join(out_dir(ctx), f"{name}_strip.png")
        s.save(path)
        ctx.log(f"strip {path}: {[n for n, _ in files]}")


@test(modes=("ds",))
def heal_sound_and_screen_five_armies_cpu(ctx):
    """Five armies, Black Hole the CPU (army 5), normal look, no fog: the
    sounds recorded and the frame strips saved."""
    g = ctx.start(heal_map(ctx, 5, 5), None)
    watch(ctx, g, 5, record=True, strips=True)


@test(modes=("ds",))
def heal_sound_and_screen_five_armies_human_fog_wasteland(ctx):
    """Five armies, Black Hole a person, fog, the Wasteland look."""
    g = ctx.start(heal_map(ctx, 5, 5, biome=1), None, humans=(1, 5), fog=True)
    watch(ctx, g, 5, press_a=True)


@test(modes=("ds",))
def heal_sound_and_screen_two_armies_wasteland(ctx):
    """Two armies, Black Hole the CPU as army 2, the Wasteland look."""
    g = ctx.start(heal_map(ctx, 2, 2, biome=1), ["andy", "hawke"])
    watch(ctx, g, 2)


@test(modes=("ds",))
def heal_sound_and_screen_four_armies_fog(ctx):
    """Four armies, Black Hole the CPU as army 3, fog."""
    g = ctx.start(heal_map(ctx, 4, 3), ["andy", "olaf", "hawke", "eagle"], fog=True)
    watch(ctx, g, 3)

