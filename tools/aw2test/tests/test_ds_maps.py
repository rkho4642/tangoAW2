"""tangoAW2's Dual Strike Versus maps (five/design_ds_maps.py): the Wasteland
set and the sea set, a 2P, 3P, 4P and 5P map each, listed only with the Dual
Strike pack.

Each map is opened from its Versus tab like a player would (the last entry of
the tab); the list's preview and the battle map are checked tile by tile, and
every property's owner and every unit against five/map.py's build of
five/maps.txt; the whole map is photographed (screenshots stitched as the
cursor sweeps it) with the list entry; then every army is handed to the CPU
for several days. Without the pack the maps are not listed at all.

tangoAW2's other maps (five/design_maps.py: the 5P maps, the Obelisk maps)
are opened, checked and photographed the same way (five_map_*), with the
pack on so that every one of them is listed. AW2TEST_MAP_IMAGES=<dir> keeps
the pictures."""

import importlib.util
import os
import shutil
import struct
import sys

from aw2test import looks, paths, ram, traverse
from aw2test import rom as romlib
from aw2test.emu import Emu
from aw2test.game import MAP_TAB, SELECT_MODE_CURSOR, SELECT_MODE_VERSUS, Game, NavError
from aw2test.harness import Skip, test
from aw2test.stitch import stitch

FIVE = os.path.join(paths.REPO, "tango-gamesupport-aw2", "five")
_spec = importlib.util.spec_from_file_location("five_map_py", os.path.join(FIVE, "map.py"))
mappy = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mappy)

# name -> (map id, Versus tab)
MAPS = {
    "Rust Basin": (0xC1, 3), "Dune Fork": (0xC2, 5), "Cinder Flats": (0xC3, 6), "Black Wastes": (0xC4, 9),
    "Coral Strait": (0xC5, 3), "Trident Isles": (0xC6, 5), "Harbor Cross": (0xC7, 6), "Coral Crown": (0xC8, 9),
}
# tangoAW2's own maps (crate::five_map::IDS, in five/maps.txt's order).
OWN = {
    "Five Seas": (0x00, 9), "Iron Crossing": (0xBC, 9), "Magma Crown": (0xBD, 9), "Skyreach": (0xBE, 9),
    "The Citadel": (0xBF, 9), "Obelisk Duel": (0xB8, 3), "Crystal Isles": (0xB9, 5), "Obelisk Plains": (0xBA, 6),
    "Black Monolith": (0xBB, 9), "Black Rampart": (0xC0, 9),
}
ALL = {**MAPS, **OWN}
# Maps meant to be crossed by air or sea alone in places (Skyreach's walled
# plateaus, Crystal Isles' islands without beaches).
AIRBORNE = ("Skyreach", "Crystal Isles")
WASTELAND = ("Rust Basin", "Dune Fork", "Cinder Flats", "Black Wastes")

# The Select Map screen: the highlighted map's tiles, decompressed for the
# preview (width, height, then u16 tiles).
PREVIEW = 0x02003010
PAL_BUFFER = 0x030020C0
WASTELAND_CLEAR = 0x08E97000  # crate::wasteland::look_rom(1) + AT_CLEAR
BIOME = 0x03004493
# gMap (battle): width/height at +0, camera (pixels) at +4/+6.
GMAP = 0x0201E450
DAY = 0x03004080
LAB = 0x14
PIPERUNNER = 9
CO_TABLE_POOL = 0x08042DDC
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
    mid, tab = ALL[name]
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
    ctx.eq(e.u8(ram.VS_MAP), ALL[name][0], f"{name}: map id")
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
    # Every army gets everywhere, with the game's own movement chart (the one
    # army 1's CO moves by: tangoAW2's copy with the pack, pipe row included).
    chart_at = e.u32(e.u32(CO_TABLE_POOL) + romlib.CO_RECORD * g.player(1)["co"] + 0x38 + 0x18)
    chart = [list(e.read(chart_at + 32 * r, 32)) for r in range(8)]
    grid = traverse.Grid(w, h, [classes[rows[y] + x] for y in range(h) for x in range(w)])
    bad = traverse.check(grid, chart, range(1, m["armies"] + 1),
                         [(a, x, y) for a, x, y, t in units if t == PIPERUNNER], need_piperunners=name in MAPS)
    if name in AIRBORNE:
        ctx.log(f"{name}: by design not every army gets everywhere on the ground ({len(bad)}: {bad[:6]})")
    else:
        ctx.check(not bad, f"{name}: every army can get everywhere, in play ({len(bad)} failures: {bad[:6]})")
    beach_check(ctx, name, grid, m["armies"])
    return m, w, h


def beach_check(ctx, name, grid, armies):
    """Every army the map's symmetry maps onto another has as many beaches,
    and an enemy beach as far from its HQ."""
    bal = traverse.beach_balance(grid)
    ctx.log(f"{name}: beaches (count, HQ to nearest enemy beach) per army {bal}")
    same = {bal[a] for a in traverse.symmetric_armies(armies)}
    ctx.check(len(same) == 1, f"{name}: every army gets the same beaches ({bal})")


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
    ctx.require(found, f"{name} is on its tab ({ALL[name][1]}) with its preview")
    export(ctx, ctx.shot(g, "list"), f"{fn}_list")
    to_teams(g)
    export(ctx, ctx.shot(g, "teams"), f"{fn}_teams")
    start(ctx, g, humans=(1,))
    g.wait_for_input()
    m, w, h = check_map(ctx, g, name)
    if name in WASTELAND:
        ctx.eq(g.e.u8(BIOME) >> 4 & 7, 1, f"{name}: the biome is Wasteland")
        ctx.check(g.e.read(PAL_BUFFER, 128) == g.e.read(WASTELAND_CLEAR, 128), f"{name}: drawn in Wasteland's colours")
        looks.check_screen(ctx, g, looks.WASTELAND, f"{name}: ")
    else:
        ctx.eq(g.e.u8(BIOME) >> 4 & 7, 0, f"{name}: AW2's own look")
    export(ctx, ctx.shot(g, "battle"), f"{fn}_battle")
    sweep = looks.Sweep(ctx, g, looks.WASTELAND, f"{name}: ") if name in WASTELAND else None
    stitch(ctx, g, name, w, h, each=sweep)
    if sweep:
        sweep.done(w, h)
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


def _open(name, prefix="ds_map_"):
    def fn(ctx):
        open_and_check(ctx, name)
    fn.__name__ = prefix + name.lower().replace(" ", "_")
    return test(modes=("ds",))(fn)


def _cpu(name, netplay=False):
    def fn(ctx):
        cpu_days(ctx, name, netplay=netplay)
    fn.__name__ = ("netplay_" if netplay else "") + "cpu_ds_map_" + name.lower().replace(" ", "_")
    return test(modes=("ds",), netplay=netplay)(fn)


for _name in MAPS:
    _open(_name)
    _cpu(_name)
for _name in OWN:
    _open(_name, "five_map_")
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


@test(modes=("aw2",))
def ds_maps_traversal_static(ctx):
    """The same traversal check on five/map.py's build, with the movement
    chart from the ROM file (no pack needed): ports reach ports, every island
    has a beach, bases are not boxed in, Piperunner bases sit behind seams."""
    rom = open(paths.aw2_rom(), "rb").read()
    chart = traverse.aw2_chart(rom)
    army5 = {0x1B4: 0xA8, 0x1B5: 0xAE, 0x1B6: 0xA6, 0x1B7: 0xAA, 0x1B8: 0xAB, 0x1B9: 0xB4}  # five_map::ARMY5_TILES
    for name in ALL:
        if name in AIRBORNE:
            continue
        m, w, h, tiles, units = built(name)
        grid = traverse.Grid(w, h, [army5.get(t, rom[romlib.TILE_CLASS - romlib.ROM_BASE + t]) for t in tiles])
        bad = traverse.check(grid, chart, range(1, m["armies"] + 1),
                             [(a, x, y) for a, x, y, t in units if t == PIPERUNNER], need_piperunners=name in MAPS)
        ctx.check(not bad, f"{name}: every army can get everywhere ({len(bad)} failures: {bad[:6]})")
        beach_check(ctx, name, grid, m["armies"])


_spec = importlib.util.spec_from_file_location("five_tilecheck_py", os.path.join(FIVE, "tilecheck.py"))
tilecheck = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tilecheck)


@test(modes=("aw2",))
def five_map_tiles_as_aw2_draws_them(ctx):
    """Every tangoAW2 map, as five/map.py builds it, drawn the way the game's
    own maps are (five/tilecheck.py, learned from every built-in map): each
    pair of neighbouring tiles is one the game places (or meets as grass
    does, or joins as a pipe, spring or base does), every mountain is the
    game's tile for what stands above and below it, and every plain above a
    mountain shows its peak."""
    rom = open(paths.aw2_rom(), "rb").read()
    for name, bad in tilecheck.check_all(rom, os.path.join(FIVE, "maps.txt")).items():
        ctx.check(not bad, f"{name}: every tile as the game draws it ({len(bad)} not: "
                           f"{[tilecheck.describe(v) for v in bad[:6]]})")
    # The check catches v0.4.0's broken mountains (every one 0x22): a 2x2
    # block under and over plains.
    rows = [[0x001, 0x001], [0x022, 0x022], [0x022, 0x022], [0x001, 0x001]]
    bad = tilecheck.mountain_violations(rows, tilecheck.mountain_rules(rom))
    ctx.check(len(bad) == 4, f"a 2x2 block of 0x22 mountains is caught: the lower two, the plains above ({len(bad)} tiles)")
