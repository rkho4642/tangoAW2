#!/usr/bin/env python3
"""Check tangoAW2's maps against Advance Wars 2's own: every pair of
neighbouring tiles (left/right, above/below, and a tile against the map's
edge) must occur somewhere in the game's built-in maps, the way the game
draws them. A pair the game never places is a tile that will look broken
(a mountain cut off, a road that does not join, a beach that ends in the
sea, ...).

Usage: tilecheck.py <rom.gba>   (checks five/maps.txt as map.py builds it)

Property tiles count by kind (an Orange Star city next to a mountain is as
good as a neutral one); Black Hole's own property tiles and the Com Towers
count as the matching neutral property. tangoAW2's own structures, the
Black Crystal and the Black Obelisk (with the ground under it), have no
counterpart in the game and are not checked.
"""
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

GAME_MAP_TABLE = 0x5C77A0  # file offset of 0x085C77A0: 0xC0 entries of 0x5C bytes
GAME_MAP_IDS = 0xC0
EDGE = -1
CRYSTAL, OBELISK = 0x192, 0x193


def lz10(b):
    size = int.from_bytes(b[1:4], 'little')
    out, p = bytearray(), 4
    while len(out) < size:
        flags = b[p]
        p += 1
        for bit in range(8):
            if len(out) >= size:
                break
            if flags & (0x80 >> bit):
                b1, b2 = b[p], b[p + 1]
                p += 2
                disp = ((b1 & 15) << 8 | b2) + 1
                for _ in range((b1 >> 4) + 3):
                    out.append(out[-disp])
            else:
                out.append(b[p])
                p += 1
    return bytes(out)


def game_maps(rom):
    """The game's built-in maps: [rows of tile ids]."""
    out, seen = [], set()
    for i in range(GAME_MAP_IDS):
        p = struct.unpack_from('<I', rom, GAME_MAP_TABLE + 0x5C * i)[0]
        if not 0x08000000 <= p < 0x08000000 + len(rom) or p in seen or rom[p - 0x08000000] != 0x10:
            continue
        seen.add(p)
        raw = lz10(rom[p - 0x08000000:])
        w, h = raw[0], raw[1]
        if w == 0 or 2 + 2 * w * h > len(raw):
            continue
        t = struct.unpack_from(f'<{w * h}H', raw, 2)
        out.append([list(t[y * w:(y + 1) * w]) for y in range(h)])
    return out


def norm(t):
    """Tiles that look alike at their edges count as one: every property
    (the buildings are sprites over the same grass tile, whoever owns them;
    Black Hole's and the Com Towers too) and the four minicannons."""
    if 0x1B4 <= t <= 0x1DD:
        return 0x1C2
    if 0x182 <= t <= 0x185:
        return 0x182
    return t


# Tiles whose edge on one side is drawn the same: the shaded tiles (a darker
# left edge where something tall stands to the left) differ only on their
# left, a plain showing a mountain's peak (0x43, 0x03) only at the bottom,
# and the mountains (map.MOUNTAIN) at the top by whether their peak is drawn
# in the cell above, at the bottom by whether a mountain stands below (their
# sides alike: Magma Crown's mountains beside the Volcano, looked at in the
# game, where the game's one Volcano has only 0x22 beside it).
SHADED = {0x021: 0x001, 0x003: 0x043, 0x086: 0x087, 0x0A1: 0x061, 0x080: 0x040, 0x081: 0x041, 0x0A0: 0x060}
MOUNTAIN_SIDES = {0x002: 0x022, 0x020: 0x022, 0x023: 0x022}
RIGHT_SIDE = {**SHADED, **MOUNTAIN_SIDES, 0x043: 0x001, 0x003: 0x001}
LEFT_SIDE = {**MOUNTAIN_SIDES, 0x043: 0x001, 0x003: 0x021}
TOP_SIDE = {**SHADED, 0x002: 0x020, 0x023: 0x022}
BOTTOM_SIDE = {**SHADED, 0x020: 0x023, 0x002: 0x022}
GRASS = {0x001, 0x021}


def pairs(rows):
    """Every (tile, direction, neighbour): 'E' the tile to the right, 'S'
    the one below; the map's edges as EDGE on either side. Each tile is
    named by its edge on that side (RIGHT_SIDE ...)."""
    h, w = len(rows), len(rows[0])
    at = lambda x, y: norm(rows[y][x]) if 0 <= x < w and 0 <= y < h else EDGE
    for y in range(-1, h):
        for x in range(-1, w):
            a = at(x, y)
            if 0 <= y < h:
                b = at(x + 1, y)
                yield (x, y), (RIGHT_SIDE.get(a, a), 'E', LEFT_SIDE.get(b, b))
            if 0 <= x < w:
                b = at(x, y + 1)
                yield (x, y), (BOTTOM_SIDE.get(a, a), 'S', TOP_SIDE.get(b, b))


def learn(rom):
    """The pairs the game's maps place, and the sides of tiles the game puts
    grass against (two sides that each meet grass meet each other too)."""
    seen = set()
    for rows in game_maps(rom):
        for _, p in pairs(rows):
            seen.add(p)
    grassy = {(g, d) for g in GRASS for d in 'ESWN'}
    for a, d, b in seen:
        if b in GRASS:
            grassy.add((a, d))
        if a in GRASS:
            grassy.add((b, 'W' if d == 'E' else 'N'))
    return seen, grassy


def pipe_links():
    """Pipe tile -> the sides it joins (N 8, E 4, S 2, W 1)."""
    import map as mappy
    links = {t: link for link, t in mappy.PIPE.items()}
    links[mappy.SEAM_ACROSS], links[mappy.SEAM_DOWN] = 0b0101, 0b1010
    return links


RIVERS = {0x18, 0x19, 0x1A, 0x1B, 0x3A, 0x58, 0x5B, 0x78, 0x7B, 0x9D, 0xDA, 0x14, 0x36}
BASES = {0x1C1, 0x1C6, 0x1CB, 0x1D0, 0x1D5, 0x1B5}
# The Deathray's ends: the game's one Deathray stands between roads, ours on
# open ground, where its ends show grass (looked at in the game).
DEATHRAY_ENDS = {(0x18F, 'W'), (0x191, 'E')}


def allowed(rows, x, y, d):
    """Pairs the game's maps happen not to hold but that join properly:
    pipes that run into each other (a seam right after an end cap, a bend
    beside a cap); a pipe or seam feeding a base (Dual Strike's Piperunner
    bases); a river's spring, its dry end against land (the game's own
    rivers spring from plains, pipes, mountains and roads); the Deathray's
    ends on grass."""
    h, w = len(rows), len(rows[0])
    X, Y = (x + 1, y) if d == 'E' else (x, y + 1)
    if not (0 <= x < w and 0 <= y < h and 0 <= X < w and 0 <= Y < h):
        return False
    a, b = rows[y][x], rows[Y][X]
    links = pipe_links()
    toward, back = (0b0100, 0b0001) if d == 'E' else (0b0010, 0b1000)
    if links.get(a, 0) & toward and (links.get(b, 0) & back or b in BASES):
        return True
    if links.get(b, 0) & back and a in BASES:
        return True

    def spring(cx, cy, ox, oy):
        """(cx, cy) a river whose one wet neighbour is opposite (ox, oy)."""
        wet = [(cx + dx, cy + dy) for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0))
               if 0 <= cx + dx < w and 0 <= cy + dy < h and rows[cy + dy][cx + dx] in RIVERS]
        return rows[cy][cx] in RIVERS and wet == [(2 * cx - ox, 2 * cy - oy)]
    if spring(x, y, X, Y) or spring(X, Y, x, y):
        return True
    return (a, 'E' if d == 'E' else 'S') in DEATHRAY_ENDS or (b, 'W' if d == 'E' else 'N') in DEATHRAY_ENDS


def violations(rows, learned, skip=()):
    """[(x, y, pair)] for every neighbouring pair the game never places and
    that does not meet as grass would (or join as allowed() lets); `skip` is
    a set of cells not checked (tangoAW2's own structures, the Design Room's
    sea)."""
    seen, grassy = learned
    out = []
    for (x, y), p in pairs(rows):
        other = (x + 1, y) if p[1] == 'E' else (x, y + 1)
        if (x, y) in skip or other in skip or p in seen:
            continue
        a, d, b = p
        if (a, d) in grassy and (b, 'W' if d == 'E' else 'N') in grassy:
            continue
        if allowed(rows, x, y, d):
            continue
        out.append((x, y, p))
    return out


def editor_sea(rows, rom):
    """Sea cells drawn as the game's own Design Room draws them: the edge
    tile its table (0x08485DC4) gives for the land around, or the opening
    for a river running in (map.RIVER_INTO_SEA). The table fits the sea to
    any land, so these are not checked against their neighbours."""
    import map as mappy
    h, w = len(rows), len(rows[0])
    cls = lambda t: rom[0x0C1BC4 + t] & 0x1F if t < 0x1B4 else 0
    mouths = {0x11C, 0x11D, 0xFC, 0xFD}
    land = lambda x, y: 0 <= x < w and 0 <= y < h and cls(rows[y][x]) not in (7, 12, 13, 19) and rows[y][x] not in mouths
    edge = mappy.sea_edges(rom)
    out = set()
    for y in range(h):
        for x in range(w):
            if rows[y][x] > 0x1B3 or cls(rows[y][x]) != 7:
                continue
            mask = 0
            for i, (dx, dy) in enumerate([(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]):
                if land(x + dx, y + dy):
                    mask |= 0x100 >> i
            want = edge(mask) if edge(mask) > 0 else mappy.SEA
            if rows[y][x] == want or rows[y][x] in {v.get(want) for v in mappy.RIVER_INTO_SEA.values()}:
                out.add((x, y))
    return out


def structure_cells(rows):
    """The Crystal and the Obelisk's 3x3 footprint."""
    out = set()
    for y, row in enumerate(rows):
        for x, t in enumerate(row):
            if t == CRYSTAL:
                out.add((x, y))
            elif t == OBELISK:
                out |= {(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)}
    return out


MOUNTAINS = {0x002, 0x020, 0x022, 0x023}
PLAINS = {0x001: False, 0x021: True, 0x043: False, 0x003: True}  # tile: shaded


def mountain_key(rows, x, y):
    """A mountain's neighbourhood: what is above it (a mountain 'M', a plain
    'P', anything else 'o', the map's edge 'e') and whether a mountain is
    below it."""
    h = len(rows)
    above = 'e' if y == 0 else 'M' if rows[y - 1][x] in MOUNTAINS else 'P' if rows[y - 1][x] in PLAINS else 'o'
    return above, y + 1 < h and rows[y + 1][x] in MOUNTAINS


def mountain_rules(rom):
    """The tile the game's own maps use most for each mountain neighbourhood,
    and for a plain above a mountain (by whether it is shaded)."""
    import collections
    mtn, peak = collections.defaultdict(collections.Counter), collections.defaultdict(collections.Counter)
    for rows in game_maps(rom):
        for y, row in enumerate(rows):
            for x, t in enumerate(row):
                if t in MOUNTAINS:
                    mtn[mountain_key(rows, x, y)][t] += 1
                    if y and rows[y - 1][x] in PLAINS:
                        peak[PLAINS[rows[y - 1][x]]][rows[y - 1][x]] += 1
    return ({k: v.most_common(1)[0][0] for k, v in mtn.items()},
            {k: v.most_common(1)[0][0] for k, v in peak.items()})


def mountain_violations(rows, rules):
    """[(x, y, (tile, 'is', wanted))] for every mountain, and every plain
    above one, not drawn as the game's own maps draw it."""
    mtn, peak = rules
    out = []
    for y, row in enumerate(rows):
        for x, t in enumerate(row):
            if t not in MOUNTAINS:
                continue
            want = mtn[mountain_key(rows, x, y)]
            if t != want:
                out.append((x, y, (t, 'is', want)))
            if y and rows[y - 1][x] in PLAINS and rows[y - 1][x] != peak[PLAINS[rows[y - 1][x]]]:
                out.append((x, y - 1, (rows[y - 1][x], 'is', peak[PLAINS[rows[y - 1][x]]])))
    return out


def check_all(rom, maps_txt=os.path.join(HERE, 'maps.txt')):
    """{map name: [violations]} for every map in maps.txt, built by map.py:
    pairs the game never places, and mountains (with the plain above) not as
    the game draws them."""
    import map as mappy
    seen = learn(rom)
    rules = mountain_rules(rom)
    edge = mappy.sea_edges(rom)
    out = {}
    for m in mappy.parse(maps_txt):
        rows = mappy.tiles(m, edge)
        out[m['name']] = (violations(rows, seen, structure_cells(rows) | editor_sea(rows, rom))
                          + mountain_violations(rows, rules))
    return out


def describe(v):
    x, y, (a, d, b) = v
    f = lambda t: 'edge' if t == EDGE else f'0x{t:03X}'
    if d == 'is':
        return f'({x},{y}) {f(a)} where the game draws {f(b)}'
    return f'({x},{y}) {f(a)} {"left of" if d == "E" else "above"} {f(b)}'


def main():
    rom = open(sys.argv[1], 'rb').read()
    bad = 0
    for name, vs in check_all(rom).items():
        print(f'{name}: {len(vs)} tiles not as the game draws them')
        for v in vs:
            print('   ', describe(v))
        bad += len(vs)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
