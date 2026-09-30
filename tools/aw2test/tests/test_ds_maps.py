"""tangoAW2's Dual Strike Versus maps (five/design_ds_maps.py): the Wasteland
set and the sea set, a 2P, 3P, 4P and 5P map each, listed only with the Dual
Strike pack.

Each map is opened from its Versus tab like a player would (the last entry of
the tab); the list's preview and the battle map are checked tile by tile, and
every property's owner and every unit against five/map.py's build of
five/maps.txt; the whole map is photographed (screenshots stitched as the
cursor sweeps it) with the list entry; then every army is handed to the CPU
for several days. Without the pack the maps are not listed at all."""

import importlib.util
import os
import shutil
import struct
import sys

from aw2test import paths, ram
from aw2test import rom as romlib
from aw2test.emu import Emu
from aw2test.game import MAP_TAB, SELECT_MODE_CURSOR, SELECT_MODE_VERSUS, Game, NavError
from aw2test.harness import Skip, test

FIVE = os.path.join(paths.REPO, "tango-gamesupport-aw2", "five")
_spec = importlib.util.spec_from_file_location("five_map_py", os.path.join(FIVE, "map.py"))
mappy = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mappy)

# name -> (map id, Versus tab)
MAPS = {
    "Rust Basin": (0xC1, 3), "Dune Fork": (0xC2, 5), "Cinder Flats": (0xC3, 6), "Black Wastes": (0xC4, 9),
    "Coral Strait": (0xC5, 3), "Trident Isles": (0xC6, 5), "Harbor Cross": (0xC7, 6), "Coral Crown": (0xC8, 9),
}
WASTELAND = ("Rust Basin", "Dune Fork", "Cinder Flats", "Black Wastes")

# The Select Map screen: the highlighted map's tiles, decompressed for the
# preview (width, height, then u16 tiles).
PREVIEW = 0x02003010
PAL_BUFFER = 0x030020C0
WASTELAND_CLEAR = 0x08671000
BIOME = 0x03004493
# gMap (battle): width/height at +0, camera (pixels) at +4/+6.
GMAP = 0x0201E450
DAY = 0x03004080
LAB = 0x14
IMAGES = os.environ.get("AW2TEST_MAP_IMAGES")


def built(name):
    """(width, height, tiles, units) of `name` as five/map.py builds it:
    tiles row by row, units [(army, x, y, type)]."""
    rom = open(paths.aw2_rom(), "rb").read()
    for m in mappy.parse(os.path.join(FIVE, "maps.txt")):
        if m["name"] == name:
            lz, units, (w, h), _ = mappy.build(m, mappy.sea_edges(rom))
            raw = bytearray()
            n = struct.unpack_from("<I", lz, 0)[0] >> 8
            p = 4
            while len(raw) < n:
                p += 1  # literal blocks only
                raw += lz[p:p + 8]
                p += 8
            raw = bytes(raw[:n])
            tiles = [struct.unpack_from("<H", raw, 2 + 2 * k)[0] for k in range(w * h)]
            out, army = [], 0
            for k in range(0, len(units), 12):
                r = units[k:k + 12]
                if r[0] == 0xFE:
                    army = r[1]
                elif r[0] != 0xFF:
                    out.append((army, r[0], r[1], r[2]))
            return m, w, h, tiles, out
    raise KeyError(name)


def select_map(ctx, name, save=None):
    """Boot, open Versus > New > the map's tab, walk down to it, check the
    preview, and press A: the Teams screen. Returns the Game."""
    mid, tab = MAPS[name]
    _, w, h, tiles, _ = built(name)
    if save is None:
        save = os.path.join(ctx.out, "map.sav")
        shutil.copy(paths.base_save(), save)
    e = Emu(save=save, ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    e.wait(700)
    e.press("START", 8)
    e.wait(300)
    e.press("A", 8)
    e.wait(150)
    for _ in range(8):
        cur = e.u8(SELECT_MODE_CURSOR)
        if cur == SELECT_MODE_VERSUS:
            break
        e.press("UP" if cur < SELECT_MODE_VERSUS else "DOWN", 8)
        e.wait(50)
    e.press("A", 8)
    e.wait(150)
    e.press("A", 8)
    e.wait(150)
    if not g.press_until("LEFT", lambda: e.u8(MAP_TAB) == tab, tries=12, hold=8, settle=50):
        raise NavError(f"tab {tab} not reached (tab {e.u8(MAP_TAB)})")
    e.wait(30)

    def previewed():
        b = e.read(PREVIEW, 2 + 2 * w * h)
        return b[0] == w and b[1] == h and list(struct.unpack_from(f"<{w * h}H", b, 2)) == tiles

    found = False
    for _ in range(40):
        if previewed():
            found = True
            break
        e.press("DOWN", 8)
        e.wait(24)
    return g, found


def to_teams(g):
    g.e.press("A", 8)
    if not g.e.wait_until(g.on_teams, 400, step=10):
        raise NavError("Teams screen did not open")
    g.e.wait(40)


def start(ctx, g, humans):
    g.set_teams(None, set(humans))
    g.teams_to_rules()
    g.set_rules(fog=False, weather="clear", power=True, visuals="off")
    g.start_battle()


def check_map(ctx, g, name):
    """The battle map against the build: tiles' classes and owners, units."""
    m, w, h, tiles, units = built(name)
    e = g.e
    ctx.eq((e.u16(GMAP), e.u16(GMAP + 2)), (w, h), f"{name}: map size")
    ctx.eq(e.u8(ram.VS_MAP), MAPS[name][0], f"{name}: map id")
    classes = e.read(ram.MAP_TERRAIN, 0x4000)
    live = e.read(romlib.TILE_CLASS, 0x400)
    rows = [e.u16(ram.MAP_ROW_OFFSETS + 2 * y) for y in range(h)]
    bad = []
    towers = 0
    for y in range(h):
        for x in range(w):
            want = live[tiles[y * w + x]]
            got = classes[rows[y] + x]
            if want != got:
                bad.append((x, y, hex(want), hex(got)))
            towers += (got & 0x1F) == LAB
    ctx.check(not bad, f"{name}: every tile's terrain and owner as built ({len(bad)} differ: {bad[:6]})")
    ctx.check(towers == sum(r.count("t") + r.count("T") for r in m["rows"]), f"{name}: {towers} Com Towers")
    ctx.check(all(classes[rows[y] + x] & 0x1F != LAB or classes[rows[y] + x] >> 5 == 0
                  for y in range(h) for x in range(w)), f"{name}: every Com Tower starts neutral")
    have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
    ctx.eq(have, sorted(units), f"{name}: the pre-deployed units")
    return m, w, h


def stitch(ctx, g, name, w, h):
    """The whole map as one picture: the cursor sweeps it and a screenshot is
    taken every two cells. Each cell is then the medoid of its views (the
    view nearest all the others), so the cursor and the panels, which move
    with the cursor, drop out. Needs PIL and numpy (else skipped)."""
    try:
        import numpy as np
        from PIL import Image
    except ImportError:
        ctx.log("no PIL/numpy: full-map picture skipped")
        return None
    e = g.e
    views = [[[] for _ in range(w)] for _ in range(h)]
    shots = 0
    ys = sorted(set(list(range(0, h, 2)) + [h - 1]))
    xs = sorted(set(list(range(0, w, 2)) + [w - 1]))
    for i, cy in enumerate(ys):
        for cx in (xs if i % 2 == 0 else xs[::-1]):
            g.goto(cx, cy)
            e.wait(8)
            for _ in range(20):
                camx, camy = e.s16(GMAP + 4), e.s16(GMAP + 6)
                if camx % 16 == 0 and camy % 16 == 0:
                    break
                e.wait(2)
            e.wait(2)
            path = e.shot(os.path.join(ctx.out, "sweep"))
            shots += 1
            img = np.asarray(Image.open(path).convert("RGB")).astype(np.int32)
            for ty in range(10):
                for tx in range(15):
                    mx, my = camx // 16 + tx, camy // 16 + ty
                    if 0 <= mx < w and 0 <= my < h and max(abs(mx - cx), abs(my - cy)) > 1:
                        same = (tx < 7.5) == ((cx * 16 - camx) < 120)
                        views[my][mx].append((same, img[16 * ty:16 * ty + 16, 16 * tx:16 * tx + 16]))
    full = np.zeros((16 * h, 16 * w, 3), dtype=np.uint8)
    missing = 0
    for my in range(h):
        for mx in range(w):
            v = [c for s, c in views[my][mx] if s] or [c for _, c in views[my][mx]]
            if not v:
                missing += 1
                continue
            best = min(range(len(v)), key=lambda k: sum(int(np.abs(v[k] - o).sum()) for o in v))
            full[16 * my:16 * my + 16, 16 * mx:16 * mx + 16] = v[best]
    full = Image.fromarray(full)
    ctx.log(f"{name}: {shots} screenshots stitched")
    ctx.check(missing <= 2, f"{name}: the sweep saw the whole map ({missing} cells unseen)")
    fn = name.lower().replace(" ", "_")
    out = os.path.join(ctx.out, f"{fn}_full.png")
    full.save(out)
    if IMAGES:
        os.makedirs(IMAGES, exist_ok=True)
        full.save(os.path.join(IMAGES, f"{fn}_full.png"))
    return out


def export(ctx, bmp, label):
    if not IMAGES:
        return
    try:
        from PIL import Image
    except ImportError:
        return
    os.makedirs(IMAGES, exist_ok=True)
    im = Image.open(bmp).convert("RGB")
    im.resize((im.width * 2, im.height * 2), Image.NEAREST).save(os.path.join(IMAGES, label + ".png"))


def open_and_check(ctx, name):
    if not ctx.ds:
        raise Skip("the pack's maps")
    g, found = select_map(ctx, name)
    fn = name.lower().replace(" ", "_")
    ctx.require(found, f"{name} is on its tab ({MAPS[name][1]}) with its preview")
    export(ctx, ctx.shot(g, "list"), f"{fn}_list")
    to_teams(g)
    export(ctx, ctx.shot(g, "teams"), f"{fn}_teams")
    start(ctx, g, humans=(1,))
    g.wait_for_input()
    m, w, h = check_map(ctx, g, name)
    if name in WASTELAND:
        ctx.eq(g.e.u8(BIOME) >> 4 & 7, 1, f"{name}: the biome is Wasteland")
        ctx.check(g.e.read(PAL_BUFFER, 128) == g.e.read(WASTELAND_CLEAR, 128), f"{name}: drawn in Wasteland's colours")
    else:
        ctx.eq(g.e.u8(BIOME) >> 4 & 7, 0, f"{name}: AW2's own look")
    export(ctx, ctx.shot(g, "battle"), f"{fn}_battle")
    stitch(ctx, g, name, w, h)
    return g


def cpu_days(ctx, name, days=6, netplay=False):
    """Every army a CPU: several days pass with no hang, and the armies act."""
    if not ctx.ds:
        raise Skip("the pack's maps")
    g, found = select_map(ctx, name)
    ctx.require(found, f"{name} is listed")
    to_teams(g)
    start(ctx, g, humans=())
    e = g.e
    armies = m_armies = built(name)[0]["armies"]
    before = {u["id"]: (u["x"], u["y"], u["hp"]) for u in g.units()}
    start_day = e.u8(DAY)
    for _ in range(days * m_armies * 12):
        e.wait(300)
        if e.u8(DAY) >= start_day + days or g.battle_over():
            break
    ctx.shot(g, "cpu")
    day = e.u8(DAY)
    ctx.check(day >= start_day + days or g.battle_over(), f"{name}: {day - start_day} CPU days passed ({armies} armies)")
    moved = {u["army"] for u in g.units() if before.get(u["id"]) != (u["x"], u["y"], u["hp"])}
    ctx.check(len(moved) == armies, f"{name}: every army's units acted ({sorted(moved)})")
    if netplay:
        identical, _, text = ctx.netplay_replay(g, [(ram.MAP_TERRAIN, 0x400)])
        ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
        ctx.check(identical, f"{name}: netplay: both peers and the straight replay identical")
    return g


def _open(name):
    def fn(ctx):
        open_and_check(ctx, name)
    fn.__name__ = "ds_map_" + name.lower().replace(" ", "_")
    return test(modes=("ds",))(fn)


def _cpu(name, netplay=False):
    def fn(ctx):
        cpu_days(ctx, name, netplay=netplay)
    fn.__name__ = ("netplay_" if netplay else "") + "cpu_ds_map_" + name.lower().replace(" ", "_")
    return test(modes=("ds",), netplay=netplay)(fn)


for _name in MAPS:
    _open(_name)
    _cpu(_name)
_cpu("Dune Fork", netplay=True)
_cpu("Coral Crown", netplay=True)


@test(modes=("aw2",))
def ds_maps_hidden_without_pack(ctx):
    """Without the pack no tab lists them: every tab's last entry is the
    game's (or an older tangoAW2 map)."""
    for name in MAPS:
        g, found = select_map(ctx, name)
        ctx.check(not found, f"{name} is not listed without the pack")
        ctx.shot(g, name.lower().replace(" ", "_"))
        g.e.close()
