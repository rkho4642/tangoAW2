"""Dual Strike's map looks (crate::wasteland, crate::ds_look): where tangoAW2
puts each look's data, and checks of it against Dual Strike's own terrain,
read straight from the .nds.

The reference is Dual Strike's own drawing (bmap/000 or 001 tiles, the look's
palette, the lower metatile table at arm9 0x02143F40, the upper one at
0x02145F40 drawn into the cell above, the mountain picked by position from
0x02169E58, the sea's and river's frames), with each AW2 tile id standing for
the Dual Strike metatile tangoAW2's doc says (`source`). Two checks:
- `check_data`: every metatile of the look's data in the ROM image against
  Dual Strike's drawing (up to the colour reduction to AW2's 4 palettes);
- `check_terrain`: the map on screen, cell by cell, read back from VRAM (BG3's
  tilemap, the terrain tiles and BG palette RAM: the terrain layer alone, no
  units, cursor or windows) against Dual Strike's drawing of the same map,
  peaks and treetops of the cells below included."""

import os
import re
import struct

from . import paths
from .rom import DualStrike, lz10

NORMAL, WASTELAND, DESERT, SNOW, GRAND_BOLT = 0, 1, 2, 3, 4
NAMES = {WASTELAND: "Wasteland", DESERT: "Desert", SNOW: "Snow", GRAND_BOLT: "Grand Bolt (Means to an End)"}
SOURCES = {WASTELAND: ("bmap/001", "bmap/009"), DESERT: ("bmap/001", "bmap/008"), SNOW: ("bmap/000", "bmap/00a"),
           GRAND_BOLT: ("bmap/001", "bmap/00b")}
# Dual Strike's building colours (sub-palettes 6-8) per look (arm9 0x02167E24..: one
# record per look, the sixth word): only for pictures, buildings being sprites in AW2.
BUILDING_COLOURS = {NORMAL: 0x02147F40, SNOW: 0x02148060, DESERT: 0x02147FA0, WASTELAND: 0x02148000,
                    GRAND_BOLT: 0x02147FA0}
LOOK_DATA, LOOK_SIZE = 0x08E80000, 0x20000
AT_METATILES, AT_TILES, AT_SEA, AT_RIVER = 0, 0x2000, 0x9000, 0x11000
AT_CLEAR, AT_RAIN, AT_SNOW, AT_SAND = 0x17000, 0x17100, 0x17200, 0x17300
AT_POOL = 0x17FF8
AW2_TILES, AW2_METATILES, AW2_CLASSES, AW2_CLEAR = 0x080BD1EC, 0x080BFBC4, 0x080C1BC4, 0x080BF8C4
PAL_BUFFER = 0x030020C0
PAL_RAM = 0x05000000
VRAM_TILES = 0x06008000
BG3CNT = 0x0400000E
BIOME = 0x03004493
GMAP = 0x0201E450
BG3_BUFFER_POINTER = 0x08499584
EMPTY = 0x100
PLAIN = 0x01
PLAIN_CELLS = (0x192, 0x193, 0x1B4, 0x1B5, 0x1B6, 0x1B7, 0x1B8, 0x1B9)
MOUNTAINS = (0x20, 0x146, 0x147)
WOODS = (0x86, 0x87)
# What tangoAW2 draws an upper part of over the cell above (crate::ds_look::TALL).
TALL = MOUNTAINS + WOODS
AS = {0x13: 0x15, 0x14: 0x15, 0x36: 0x16, 0x03: PLAIN, 0x43: PLAIN, 0x106: PLAIN, 0x107: PLAIN, 0x126: PLAIN, 0x127: PLAIN}
SEA0, RIVER0 = 0x100, 0x200
# Means to an End's Grand Bolt (its underlay, weak points and the Crystals on it), drawn by
# crate::grand_bolt with Dual Strike's own textures: not terrain metatiles.
BOLT_TILES = (0x1A4, 0x194, 0x192)
ROAD = 5


def _road_shade():
    """crate::ds_look::ROAD_SHADE: roads this many 5-bit steps darker."""
    src = open(os.path.join(paths.REPO, "tango-gamesupport-aw2", "src", "ds_look.rs")).read()
    return int(re.search(r"pub const ROAD_SHADE: u16 = (\d+);", src).group(1))


ROAD_SHADE = _road_shade()


def shade(c, k=ROAD_SHADE):
    return sum(max(0, ((c >> (5 * j)) & 31) - k) << (5 * j) for j in range(3))


def look_rom(b):
    return LOOK_DATA + LOOK_SIZE * (b - 1)


def u16s(b):
    return list(struct.unpack("<%dH" % (len(b) // 2), b))


def dist(a, b):
    return sum((((a >> 5 * j) & 31) - ((b >> 5 * j) & 31)) ** 2 for j in range(3))


def _tpx(tiles, t, x, y):
    o = 32 * t + 4 * y + x // 2
    return (tiles[o] >> (4 * (x & 1))) & 15 if o < len(tiles) else 0


def _px(tiles, v, x, y):
    sx = 7 - x if v >> 10 & 1 else x
    sy = 7 - y if v >> 11 & 1 else y
    return _tpx(tiles, v & 0x3FF, sx, sy)


def draw(tiles, table, pal, m, quads=None):
    """Metatile `m` (or the four entries `quads`) as 16 rows of 16 colours
    (None where transparent)."""
    q = quads or table[4 * m:4 * m + 4]
    out = []
    for y in range(16):
        row = []
        for x in range(16):
            v = q[(y // 8) * 2 + x // 8]
            i = _px(tiles, v, x % 8, y % 8) if v != EMPTY else 0
            row.append(pal[16 * (v >> 12) + i] if i else None)
        out.append(row)
    return out


def derive_frames(normal, look, frames, start, n, size):
    """The frames of a look whose tileset is not the Normal one (crate::ds_look)."""
    out = bytearray(len(frames))
    for f in range(len(frames) // size):
        for k in range(n):
            for y in range(8):
                for x in range(8):
                    b0 = _tpx(normal, start + k, x, y)
                    bf = _tpx(frames[f * size:], k, x, y)
                    l = _tpx(look, start + k, x, y)
                    out[f * size + 32 * k + 4 * y + x // 2] |= (bf if l == b0 else l) << (4 * (x & 1))
    return bytes(out)


class Reference:
    """Dual Strike's terrain for one look, and AW2's ROM tables."""

    _ds = None

    def __init__(self, look):
        if Reference._ds is None:
            Reference._ds = DualStrike(paths.ds_rom())
        ds = Reference._ds
        self.look = look
        ts, ps = SOURCES[look]
        self.tiles = ds.file(ts)
        self.pal = u16s(ds.files[ps])
        self.pal[16 * 6:16 * 9] = u16s(ds.a9(BUILDING_COLOURS[look], 0x60))
        self.lower = u16s(ds.a9(0x02143F40, 0x2000))
        self.upper = u16s(ds.a9(0x02145F40, 0x2000))
        self.pick = u16s(ds.a9(0x02169E58, 32))
        normal = ds.file("bmap/000")
        sea, river = ds.files["bmap/004"], ds.files["bmap/005"]
        if ts == "bmap/000":
            self.sea, self.river = sea, river
        else:
            self.sea = derive_frames(normal, self.tiles, sea, SEA0, 256, 0x2000)
            self.river = derive_frames(normal, self.tiles, river, RIVER0, 96, 0xC00)
        rom = open(paths.aw2_rom(), "rb").read()
        self.aw2_meta = u16s(rom[AW2_METATILES - 0x08000000:][:0x2000])
        self.classes = rom[AW2_CLASSES - 0x08000000:][:0x400]

    def mountain_at(self, x, y):
        return self.pick[(x + x // 4 + 2 * y + y // 8) & 15]

    def source(self, m, aw2_meta=None):
        """How tangoAW2 draws AW2 metatile m: ('ds', id), ('aw2', m) or None
        (unused). Mountains are Dual Strike's 0x20 here; on the map, the one
        of the three for the cell's position."""
        meta = aw2_meta or self.aw2_meta
        q = meta[4 * m:4 * m + 4]
        if EMPTY in q:
            return None
        if q == meta[4 * PLAIN:4 * PLAIN + 4] or m in PLAIN_CELLS:
            return ("ds", PLAIN)
        if self.classes[m] == 3:
            return ("ds", MOUNTAINS[0])
        if self.classes[m] == 4:
            return ("ds", WOODS[m & 1])
        if m in AS:
            return ("ds", AS[m])
        if EMPTY not in self.lower[4 * m:4 * m + 4]:
            return ("ds", m)
        return ("aw2", m)

    def ds_id(self, t, x, y, aw2_meta=None):
        """The Dual Strike metatile drawn for AW2 tile t at (x, y), or None."""
        src = self.source(t, aw2_meta)
        if src is None or src[0] != "ds":
            return None
        if self.classes[t] == 3:
            return self.mountain_at(x, y)
        return src[1]

    def picture(self, d):
        return draw(self.tiles, self.lower, self.pal, d)

    def render_cell(self, d, below, tiles=None, road=False):
        """Dual Strike's drawing of a cell of metatile d (16x16 colours), the
        upper part of the cell below (metatile `below`) over it. Where an
        upper part lands on a quadrant, that quadrant is drawn at the first
        frame (tangoAW2's composite tiles are not animated: a sea or river
        cell under a peak keeps its first frame there)."""
        cell = draw(tiles or self.tiles, self.lower, self.pal, d)
        if road and ROAD_SHADE:
            cell = [[None if c is None else shade(c) for c in row] for row in cell]
        if below is None:
            return cell
        up = draw(self.tiles, self.upper, self.pal, below)
        still = draw(self.tiles, self.lower, self.pal, d)
        if road and ROAD_SHADE:
            still = [[None if c is None else shade(c) for c in row] for row in still]
        for side in (0, 1):
            cols = range(8 * side, 8 * side + 8)
            if not any(up[yy][xx] is not None for yy in range(8, 16) for xx in cols):
                continue
            for yy in range(8, 16):
                for xx in cols:
                    cell[yy][xx] = up[yy][xx] if up[yy][xx] is not None else still[yy][xx]
        return cell

    def render_map(self, ids, w, h):
        """Dual Strike's drawing of a whole map of its metatile ids (None:
        not drawn) as {(x, y): 16x16 colours}, upper parts drawn over the
        cell above (however high they reach)."""
        cells = {}
        for y in range(h):
            for x in range(w):
                d = ids[y * w + x]
                if d is not None:
                    cells[(x, y)] = draw(self.tiles, self.lower, self.pal, d)
        for y in range(1, h):
            for x in range(w):
                d = ids[y * w + x]
                if d is None or (x, y - 1) not in cells:
                    continue
                up = draw(self.tiles, self.upper, self.pal, d)
                for yy in range(16):
                    for xx in range(16):
                        if up[yy][xx] is not None:
                            cells[(x, y - 1)][yy][xx] = up[yy][xx]
        return cells

    def frame_tiles(self, sea_half, river_frame):
        """The tileset with the sea's halves and river's frame in place
        (sea_half: the half-frame index 0..7 of tiles 0x100.. and 0x180..)."""
        t = bytearray(self.tiles)
        lo, hi = sea_half
        t[0x2000:0x3000] = self.sea[lo * 0x1000:(lo + 1) * 0x1000]
        t[0x3000:0x4000] = self.sea[hi * 0x1000:(hi + 1) * 0x1000]
        t[0x4000:0x4C00] = self.river[river_frame * 0xC00:(river_frame + 1) * 0xC00]
        return bytes(t)


class Look:
    """A look's data as tangoAW2 put it in the ROM image (read through the
    emulator)."""

    def __init__(self, e, look):
        base = look_rom(look)
        self.metatiles = u16s(e.read(base + AT_METATILES, 0x2000))
        self.tiles = lz10(e.read(base + AT_TILES, 0x7000))
        self.sea = e.read(base + AT_SEA, 0x8000)
        self.river = e.read(base + AT_RIVER, 0x6000)
        self.clear = e.read(base + AT_CLEAR, 256)
        self.rain = e.read(base + AT_RAIN, 256)
        self.snow = e.read(base + AT_SNOW, 256)
        self.sand = e.read(base + AT_SAND, 256)
        self.clear_colours = u16s(self.clear)
        self.pool = e.u32(base + AT_POOL)


def picture_error(a, b):
    """Mean squared colour distance (5-bit channels) over the pixels both
    draw, and the share of pixels that are exactly the same."""
    total, same, n = 0, 0, 0
    for ra, rb in zip(a, b):
        for ca, cb in zip(ra, rb):
            if ca is None or cb is None:
                continue
            n += 1
            d = dist(ca, cb)
            total += d
            same += d == 0
    return (total / n if n else 0), (same / n if n else 1)


# The colour reduction's tolerance: mean squared distance per pixel, in 5-bit
# channels (the reduction folds Dual Strike's 5 terrain palettes into AW2's 4).
MAX_ERROR = 6.0


def check_data(ctx, e, look, max_err=MAX_ERROR):
    """Every metatile of the look's data drawn against Dual Strike's own
    drawing of what it stands for; AW2's leftovers keep AW2's shapes."""
    ref = Reference(look)
    data = Look(e, look)
    aw2_meta = u16s(e.read(AW2_METATILES, 0x2000))
    worst, kinds = (0, None), {}
    bad = []
    for m in range(1024):
        src = ref.source(m, aw2_meta)
        if src is None:
            continue
        kinds[src[0]] = kinds.get(src[0], 0) + 1
        if src[0] == "aw2":
            continue
        ours = draw(data.tiles, data.metatiles, data.clear_colours, m)
        want = ref.render_cell(src[1], None, road=ref.classes[m] == ROAD and src[1] == m)
        err, same = picture_error(ours, want)
        if err > worst[0]:
            worst = (err, m)
        if err > max_err:
            bad.append((hex(m), round(err, 2), round(same, 2)))
    ctx.log(f"{NAMES[look]}: metatiles by source {kinds}; worst mean colour error {worst[0]:.2f} at {worst[1]}; "
            f"{data.pool} free tiles for the peaks' and treetops' composites")
    ctx.check(not bad, f"{NAMES[look]}: every metatile drawn as Dual Strike draws it (bad: {bad[:8]})")
    ctx.check(kinds.get("ds", 0) > 300, f"{NAMES[look]}: Dual Strike's terrain drawn ({kinds})")
    ctx.check(data.pool >= 64, f"{NAMES[look]}: room for the composites ({data.pool} tiles)")
    return data


def check_screen(ctx, g, look, label=""):
    """The map on screen is drawn from the look: the terrain tiles in VRAM
    (those never animated exactly, the sea's and river's one of the look's
    frames) and the map colours; then cell by cell (`check_terrain`)."""
    e = g.e
    data = Look(e, look)
    name = f"{label}{NAMES[look]}"
    vram = e.read(VRAM_TILES, 0x6000)
    # Tiles the look leaves free (zero) hold the composites.
    static = [s for s in list(range(0x100)) + list(range(0x260, 0x300)) if any(data.tiles[32 * s:32 * s + 32])]
    ctx.check(all(vram[32 * s:32 * s + 32] == data.tiles[32 * s:32 * s + 32] for s in static),
              f"{name}: the terrain tiles in VRAM are the look's")
    halves = [data.sea[k * 0x1000:(k + 1) * 0x1000] for k in range(8)]
    ctx.check(vram[0x2000:0x3000] in halves and vram[0x3000:0x4000] in halves, f"{name}: the sea is one of the look's frames")
    rivers = [data.river[k * 0xC00:(k + 1) * 0xC00] for k in range(8)]
    ctx.check(vram[0x4000:0x4C00] in rivers, f"{name}: the river is one of the look's frames")
    ctx.check(e.read(PAL_BUFFER, 128) in (data.clear[:128], data.rain[:128], data.snow[:128], data.sand[:128]),
              f"{name}: BG palettes 0-3 are the look's")
    check_terrain(ctx, g, look, label)


# AW2's colour relations as crate::wasteland has them (x256 per 5-bit channel;
# rows r, g, b, 1) and the sandstorm's (crate::sandstorm::sand_colour).
FITS = {
    "fog": [[136, 22, 14], [40, 156, 101], [19, 4, 117], [-305, -2, -335]],
    "rain": [[211, -12, -6], [26, 251, 85], [7, 18, 171], [-296, -367, 89]],
    "rain fog": [[106, 4, 12], [70, 157, 112], [7, 6, 107], [-219, 316, -442]],
    "snow": [[170, -23, -60], [45, 256, 143], [-16, 10, 21], [1697, 715, 4306]],
    "snow fog": [[143, -3, -11], [22, 199, 140], [-21, 2, 2], [1337, 68, 2424]],
}
SAND = (27, 21, 12)


def fit(m, c):
    v = [c & 31, (c >> 5) & 31, (c >> 10) & 31]
    out = 0
    for j in range(3):
        x = v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + m[3][j]
        out |= max(0, min(31, (x + 128) // 256)) << (5 * j)
    return out


def sand(c):
    return sum((((((c >> (5 * j)) & 31) * 5 + SAND[j] * 3) // 8) << (5 * j)) for j in range(3))


def weather_colour(weather, fogged, look):
    """Clear colour -> the colour on screen in this weather (and fog)."""
    if weather == "sand":
        clear_fog = lambda c: fit(FITS["fog"], c)
        return (lambda c: sand(clear_fog(c))) if fogged else sand
    if weather == "snow" and look == SNOW:
        weather = "clear"
    if weather == "clear":
        return (lambda c: fit(FITS["fog"], c)) if fogged else (lambda c: c)
    key = weather + (" fog" if fogged else "")
    return lambda c: fit(FITS[key], c)


_REFS = {}


def reference(look):
    if look not in _REFS:
        _REFS[look] = Reference(look)
    return _REFS[look]


def terrain_cells(e, look, max_err=MAX_ERROR):
    """The visible cells of the terrain layer (BG3 as VRAM has it: tilemap,
    tiles, palette RAM) against Dual Strike's drawing of the same map (its
    mountain for the cell's position, the upper part of the cell below over
    it, the sea and river at the frame on screen), in the weather's and fog's
    colours (AW2's relations, `FITS`). Returns (cells compared, cells over
    `max_err` as ((x, y), tile, error), fogged cells among them, cells
    tangoAW2 draws with AW2's own tiles, which are left out), or None when the
    map's colours are none of the look's sets."""
    data = Look(e, look)
    # Palettes 0-3 tell the weather (4-7 are their fogged copies; the Grand
    # Bolt draws with palette 7, crate::grand_bolt).
    now = e.read(PAL_BUFFER, 128)
    weather = {data.clear[:128]: "clear", data.rain[:128]: "rain", data.snow[:128]: "snow",
               data.sand[:128]: "sand"}.get(now)
    if weather is None:
        return None
    ref = reference(look)
    w, h = e.u16(GMAP), e.u16(GMAP + 2)
    rows = u16s(e.read(GMAP + 0x417A, 2 * h))
    span = rows[h - 1] + w
    raw_tiles = u16s(e.read(GMAP + 0xA22, 2 * span))
    raw_seen = e.read(GMAP + 0x234A, span)
    tiles = [raw_tiles[rows[y] + x] for y in range(h) for x in range(w)]
    seen = [raw_seen[rows[y] + x] for y in range(h) for x in range(w)]
    aw2_meta = u16s(e.read(AW2_METATILES, 0x2000))
    sx, sy = e.s16(GMAP + 4) >> 4, e.s16(GMAP + 6) >> 4
    cx, cy = e.s16(GMAP + 0xC), e.s16(GMAP + 0xE)
    # The frames on screen.
    vram = e.read(VRAM_TILES, 0x6000)
    halves = [data.sea[k * 0x1000:(k + 1) * 0x1000] for k in range(8)]
    rivers = [data.river[k * 0xC00:(k + 1) * 0xC00] for k in range(8)]
    lo = halves.index(vram[0x2000:0x3000]) if vram[0x2000:0x3000] in halves else 0
    hi = halves.index(vram[0x3000:0x4000]) if vram[0x3000:0x4000] in halves else 1
    rf = rivers.index(vram[0x4000:0x4C00]) if vram[0x4000:0x4C00] in rivers else 0
    ds_tiles = ref.frame_tiles((lo, hi), rf)
    pal = u16s(e.read(PAL_RAM, 0x100))
    block = (e.u16(BG3CNT) >> 8) & 0x1F
    tilemap = u16s(e.read(0x06000000 + 0x800 * block, 0x800))
    xs = range(max(0, sx), min(sx + 15, w))
    ys = range(max(0, sy), min(sy + 10, h))
    ids = {}
    for y in range(max(0, sy), min(sy + 11, h)):
        for x in xs:
            ids[(x, y)] = ref.ds_id(tiles[y * w + x], x, y, aw2_meta)
    checked, bad, fogged, aw2 = [], [], 0, 0
    # Sand blown over fogged colours: the sandstorm set is the clear set's,
    # fog half included (crate::wasteland).
    for y in ys:
        for x in xs:
            d = ids[(x, y)]
            if d is None or (look == GRAND_BOLT and tiles[y * w + x] in BOLT_TILES):
                aw2 += 1
                continue
            below = ids.get((x, y + 1))
            fog = not seen[y * w + x]
            fogged += fog
            to = weather_colour(weather, fog, look)
            want = [[None if c is None else to(c) for c in row]
                    for row in ref.render_cell(d, below if below in TALL else None, ds_tiles,
                                               road=ref.classes[tiles[y * w + x]] == ROAD)]
            px, py = ((x - cx) & 15) * 2, ((y - cy) & 15) * 2
            q = [tilemap[(py + dy) * 32 + px + dx] for dy in (0, 1) for dx in (0, 1)]
            err, _ = picture_error(draw(vram, None, pal, None, quads=q), want)
            checked.append(((x, y), err))
            if err > max_err:
                bad.append(((x, y), hex(tiles[y * w + x]), round(err, 1)))
    return checked, bad, fogged, aw2


def check_terrain(ctx, g, look, label=""):
    """`terrain_cells` on the view now: every cell within the tolerance."""
    name = f"{label}{NAMES[look]}"
    r = terrain_cells(g.e, look)
    if r is None:
        ctx.log(f"{name}: the colours are none of the look's sets: cells not compared")
        return
    checked, bad, fogged, aw2 = r
    worst = max(checked, key=lambda c: c[1], default=(None, 0))
    ctx.log(f"{name}: {len(checked)} cells against Dual Strike's drawing, worst {worst[1]:.2f} at {worst[0]}; "
            f"{fogged} of them fogged; {aw2} drawn with AW2's tiles left out")
    ctx.check(checked and not bad, f"{name}: every cell on screen drawn as Dual Strike draws it ({len(bad)} differ: {bad[:6]})")


class Sweep:
    """`terrain_cells` at every view of a sweep over the whole map
    (aw2test.stitch's `each`): every cell of the map checked once at least."""

    def __init__(self, ctx, g, look, label=""):
        self.ctx, self.g, self.look, self.label = ctx, g, look, label
        self.cells, self.bad, self.aw2, self.skipped = {}, {}, set(), 0

    def __call__(self):
        r = terrain_cells(self.g.e, self.look)
        if r is None:
            self.skipped += 1
            return
        checked, bad, _, _ = r
        for c, err in checked:
            self.cells[c] = max(err, self.cells.get(c, 0))
        for c, t, err in bad:
            self.bad[c] = (t, err)

    def done(self, w, h):
        name = f"{self.label}{NAMES[self.look]}"
        worst = max(self.cells.items(), key=lambda c: c[1], default=(None, 0))
        self.ctx.log(f"{name}: {len(self.cells)} of {w * h} cells against Dual Strike's drawing over the sweep, "
                     f"worst {worst[1]:.2f} at {worst[0]}")
        self.ctx.check(len(self.cells) > 0 and not self.bad,
                       f"{name}: the whole map drawn as Dual Strike draws it ({len(self.bad)} differ: {sorted(self.bad.items())[:6]})")


def sample_map(ctx, biome=WASTELAND):
    """A design map with a bit of every terrain the looks redraw: woods and
    mountains next to each other, below plains, a road and the top edge."""
    m = ctx.map()
    for x in range(4, 12):
        m.terrain(x, 8, "road")
    for (x, y) in ((2, 3), (3, 3), (3, 4), (4, 5), (5, 0), (6, 1)):
        m.terrain(x, y, "wood")
    for (x, y) in ((6, 3), (7, 4), (6, 5), (7, 3), (4, 9), (5, 9), (6, 9), (8, 0), (9, 0), (6, 4)):
        m.terrain(x, y, "mountain")
    for x in range(9, 14):
        for y in range(1, 6):
            m.terrain(x, y, "sea")
    for y in range(9, 14):
        m.terrain(2, y, "river")
    m.terrain(8, 6, "city", 0).terrain(12, 7, "city", 1).terrain(7, 5, "city", 0)
    m.unit(1, "tank", 6, 10).unit(2, "tank", 9, 10)
    m.biome = biome
    return m
