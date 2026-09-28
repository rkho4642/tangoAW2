#!/usr/bin/env python3
"""Draw tangoAW2's 5-army maps into five/maps.txt (read by map.py).
Hand-drawn parts are text; the round and plateau maps are laid out from
shapes. Run it, then map.py."""
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
out = []


def emit(name, units, rows, armies=5, tab=9, colours=(1, 2, 3, 4, 5)):
    out.append(f'map {name}')
    out.append(f'armies {armies} {tab} ' + ' '.join(map(str, colours)))
    for a, u in units.items():
        out.append(f'units {a} ' + ' '.join(map(str, u)))
    out.extend(''.join(r) for r in rows)
    out.append('')


def grid(w, h, c):
    return [[c] * w for _ in range(h)]


def put(g, x, y, s):
    for i, row in enumerate(s):
        for j, c in enumerate(row):
            if c != ' ':
                g[y + i][x + j] = c


def mirror(q, hq_map):
    """A quadrant, mirrored right and down; HQ 1 becomes 2, 3, 4."""
    n = len(q)
    g = [list(r) + list(r[::-1]) for r in q]
    g = g + [r[:] for r in g[::-1]]
    for y in range(2 * n):
        for x in range(2 * n):
            if g[y][x] == '1':
                g[y][x] = hq_map[(x >= n, y >= n)]
    return g


# ---- 1. Five Seas: the tall naval map (as before) ----
TOP = [
    "~~~~~~~~~~~~~~~~~~~~",
    "~~~~~C..f.C..C~~~~~~",
    "~~~~C.B..1..B..~~~~~",
    "~~~~~C.f..^^.A~~~~~~",
    "~~~~~~~P....P~~~~~~~",
    "~~c.~~~~~~~~~~~~.c~~",
    "~~.p~~~~~r~~~~~~p.~~",
    "~~~~~~~~~~~~~~~~~~~~",
    "~~~~~~~~P...P~~~~~~~",
    "~~~~~~C..f...C~~~~~~",
    "~~~~~B...2.^..B~~~~~",
    "~~~~~C..f..^..A~~~~~",
    "~~~~~~C....C.~~~~~~~",
    "~~a.c~~~~r~~~~~c.a~~",
    "~~c.p~~~~~~~~~~p.c~~",
    "~~~~~~~~~~~~~~~~~~~~",
]
MIDDLE = [
    "~~~~.###.CC.###.~~~~",
    "~~~P.#n#....#n#.P~~~",
    "~~~..###.^^.###..~~~",
    "~~~.C.B..5...B.C.~~~",
    "~~~.C..f.^^.f..A.~~~",
    "~~~..###....###..~~~",
    "~~~~.#v#....#v#.~~~~",
    "~~~~.###....###.~~~~",
]
bottom = [r.replace('1', '4').replace('2', '3') for r in TOP[::-1]]
emit('Five Seas', {a: [1, 1, 5, 21, 22, 24, 23] for a in range(1, 6)}, TOP + MIDDLE + bottom)

# ---- 2. Iron Crossing: land war, four corners and a fortress ----
Q = [
    "..f....f..^^...",
    ".1.B..c....^...",
    ".B.C......f^...",
    "..C..ff.....^..",
    ".A....f..c..^..",
    "..ff........f..",
    "....c.^^..f....",
    "f.....^^.......",
    "..f.........f..",
    ".c...f..c......",
    "^^^.....f......",
    "..^^.....f.....",
    "...^.c.........",
    "....^^.....f...",
    "......f........",
]
g = mirror(Q, {(0, 0): '1', (1, 0): '2', (0, 1): '3', (1, 1): '4'})
put(g, 11, 11, [
    "..N..N..",
    ".f....f.",
    "W.B..C.E",
    "...5....",
    "..C..C..",
    "W.B....E",
    ".f....f.",
    "..S..S..",
])
emit('Iron Crossing', {a: [1, 1, 2, 5, 6, 10] for a in range(1, 6)}, g)

# ---- 3. Magma Crown: a ring island round Black Hole's volcano ----
W = H = 30
g = grid(W, H, '~')
cx = cy = 14.5
for y in range(H):
    for x in range(W):
        d = math.hypot(x - cx, y - cy)
        if 9.2 <= d <= 13.8:
            g[y][x] = '.'
        elif d <= 5.2:
            g[y][x] = '.'
for y, x in [(3, 9), (3, 20), (26, 9), (26, 20), (9, 3), (20, 3), (9, 26), (20, 26)]:
    g[y][x] = 'f'
for y, x in [(5, 6), (6, 5), (5, 23), (6, 24), (23, 5), (24, 6), (23, 24), (24, 23)]:
    g[y][x] = '^'
# the four ring armies (N, E, S, W) and their towns
ring = {
    '1': (14, 2, [(12, 2, 'B'), (16, 2, 'C'), (13, 3, 'C'), (15, 3, 'A'), (14, 5, 'P')]),
    '2': (27, 14, [(27, 12, 'B'), (27, 16, 'C'), (26, 13, 'C'), (26, 15, 'A'), (24, 14, 'P')]),
    '3': (14, 27, [(16, 27, 'B'), (12, 27, 'C'), (15, 26, 'C'), (13, 26, 'A'), (14, 24, 'P')]),
    '4': (2, 14, [(2, 16, 'B'), (2, 12, 'C'), (3, 15, 'C'), (3, 13, 'A'), (5, 14, 'P')]),
}
for hq, (x, y, towns) in ring.items():
    g[y][x] = hq
    for tx, ty, c in towns:
        g[ty][tx] = c
for x, y in [(5, 5), (24, 5), (5, 24), (24, 24), (9, 1), (20, 28)]:
    g[y][x] = 'c'
# the central isle: the Volcano, Black Hole's HQ, a port each way
put(g, 12, 11, [
    "####",
    "####",
    "#V##",
    "####",
])
for x, y, c in [(14, 16, '5'), (12, 16, 'B'), (17, 16, 'C'), (11, 14, 'C'), (18, 14, 'A'), (14, 19, 'P')]:
    g[y][x] = c
emit('Magma Crown', {a: [1, 2, 5, 10, 23, 22] for a in range(1, 6)}, g)

# ---- 4. Skyreach: plateaus walled by mountains; the sky decides ----
W, H = 32, 36
g = grid(W, H, '^')
for y in range(H):
    for x in range(W):
        if (x + y * 3) % 7 == 0 or (x * y) % 11 == 3:
            g[y][x] = 'f'
plateaus = {'1': (6, 6), '2': (25, 6), '5': (15, 17), '3': (6, 29), '4': (25, 29)}
for hq, (px, py) in plateaus.items():
    for y in range(py - 4, py + 5):
        for x in range(px - 5, px + 6):
            if 0 <= x < W and 0 <= y < H and abs(x - px) / 5.6 + abs(y - py) / 4.6 <= 1.3:
                g[y][x] = '.'
    g[py][px] = hq
    for dx, dy, c in [(-3, -2, 'A'), (3, -2, 'A'), (0, 3, 'A'), (-4, 1, 'C'), (4, 1, 'C'), (-2, 2, 'B')]:
        g[py + dy][px + dx] = c
for x, y in [(15, 3), (15, 32), (2, 17), (29, 17), (10, 12), (21, 22)]:
    g[y][x] = 'a'
for x, y in [(15, 8), (15, 26), (6, 17), (25, 17)]:
    g[y][x] = 'c'
# Black Hole's plateau guns: cannons north and south
put(g, 14, 12, ["###", "#n#", "###"])
put(g, 14, 22, ["###", "#v#", "###"])
emit('Skyreach', {a: [16, 17, 19, 19, 14, 15, 1] for a in range(1, 6)}, g)

# ---- 5. The Citadel: four armies storm Black Hole's fortress ----
C = [
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
    "~~^^^^...###.....###^^^^^~~~",
    "~~^^.C...###..5..#D#...C^^~~",
    "~~^..B...#F#.....###...B.^~~",
    "~~^.C....###..C.....C....^~~",
    "~~^..W..L.......A.....E..^~~",
    "~~^^..S..S..S..S..S..S..^^~~",
    "~~~^^.................^^^~~~",
    "~~~~^^^^ff.......ff^^^^~~~~~",
    "~~~~~~^^.............^^~~~~~",
    "~~c~~~~~.....f.f.....~~~~c~~",
    "~~..~~~~.............~~~~..~",
    "~~.a~~~~~~....r....~~~~~a.~~",
    "~~~~~~~~~~~.......~~~~~~~~~~",
    "~~~~~~^^^..........^^^~~~~~~",
    "~~~~^^^.....c...c.....^^~~~~",
    "~~~f.......f.......f.....~~~",
    "~~......b..........b.....~~~",
    "~~..^^......^^.......^^..~~~",
    "~~~.^^.f....^^....f..^^.~~~~",
    "~~~.......c......c.......~~~",
    "~~~ff.....................~~",
    "~~~" + "..C.C." * 4 + "~",
    "~~~" + "..B..." * 4 + "~",
    "~~~" + "...1.." + "...2.." + "...3.." + "...4.." + "~",
    "~~~" + ".A...." * 4 + "~",
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
]
emit('The Citadel', {1: [1, 2, 5, 10, 6], 2: [1, 2, 5, 10, 6], 3: [1, 2, 5, 10, 6], 4: [1, 2, 5, 10, 6], 5: [1, 1, 3, 14]}, C)

# ---- Black Crystal and Black Obelisk maps (obelisk.rs) ----


def obelisk(g, x, y):
    put(g, x - 1, y - 1, ["###", "#O#", "###"])


# Obelisk Duel (2P): Orange Star against Black Hole's healing line.
g = grid(24, 16, '.')
for y in range(16):
    g[y][0] = g[y][23] = '~'
for x in range(24):
    g[0][x] = g[15][x] = '~'
for y in (1, 2, 3, 6, 9, 12, 13, 14):
    g[y][11] = '^'
for x, y, c in [(3, 8, '1'), (2, 5, 'B'), (2, 11, 'B'), (5, 3, 'C'), (5, 12, 'C'), (6, 8, 'A'),
                (20, 8, '2'), (21, 5, 'B'), (21, 11, 'B'), (18, 3, 'C'), (18, 12, 'C'), (21, 14, 'A'),
                (8, 3, 'c'), (8, 12, 'c'), (13, 5, 'c'), (13, 10, 'c'), (9, 8, 'c'), (15, 1, 'c')]:
    g[y][x] = c
for x, y in [(4, 6), (7, 5), (7, 10), (15, 3), (16, 13), (10, 7), (12, 9)]:
    g[y][x] = 'f'
obelisk(g, 17, 8)
g[4][14] = g[11][14] = 'X'      # forward Crystals
g[8][14] = 'W'                  # a real minicannon, facing west
put(g, 20, 1, ["###", "#v#", "###"])  # a real Black Cannon
emit('Obelisk Duel', {1: [1, 1, 2, 5, 5, 6, 10], 2: [1, 1, 2, 5, 10]}, g, armies=2, tab=3, colours=(1, 5))

# Crystal Isles (3P): three islands; Black Hole's heals at its ports.
W, H = 30, 26
g = grid(W, H, '~')
isles = {'1': (6, 6), '2': (23, 6), '3': (15, 19)}
for hq, (cx, cy) in isles.items():
    for y in range(H):
        for x in range(W):
            if (x - cx) ** 2 / 30 + (y - cy) ** 2 / 20 <= 1:
                g[y][x] = '.'
    g[cy][cx] = hq
    for dx, dy, c in [(-3, -2, 'B'), (3, -2, 'C'), (-3, 2, 'C'), (2, 2, 'A'), (0, -3, 'C')]:
        g[cy + dy][cx + dx] = c
g[6][11] = 'P'; g[6][18] = 'P'; g[15][15] = 'P'
g[9][6] = 'P'; g[9][23] = 'P'; g[22][15] = 'P'
for x, y in [(14, 3), (4, 16), (26, 16), (15, 11)]:
    for dy in range(-1, 2):
        for dx in range(-2, 3):
            if 0 <= x + dx < W and 0 <= y + dy < H:
                g[y + dy][x + dx] = '.'
    g[y][x] = 'c'
g[4][15] = 'a'
obelisk(g, 12, 19)              # Black Hole is army 3 (colour 5)
g[16][13] = 'X'; g[22][18] = 'X'
g[19][19] = 'E'                 # a real minicannon, facing east
emit('Crystal Isles', {a: [1, 2, 5, 10, 23, 22] for a in (1, 2, 3)}, g, armies=3, tab=5, colours=(1, 2, 5))

# Obelisk Plains (4P): four corners; Black Hole's has the Obelisk.
Q = [
    "..f.....f.....",
    ".1.B...c......",
    ".B.C......f...",
    "..C..ff.......",
    ".A....f..c....",
    "..ff..........",
    "....c....f....",
    "f.............",
    "..f.......^^..",
    ".c...f....^...",
    "..........c...",
    "......f.......",
    "...^^.........",
    "..............",
]
g = mirror(Q, {(0, 0): '1', (1, 0): '2', (0, 1): '3', (1, 1): '4'})
obelisk(g, 23, 23)              # in Black Hole's corner (army 4)
for x, y in [(13, 13), (14, 14), (18, 9), (9, 18)]:
    g[y][x] = 'X'
g[20][24] = 'N'; g[24][20] = 'W'  # real minicannons
emit('Obelisk Plains', {a: [1, 1, 2, 5, 6, 10] for a in (1, 2, 3, 4)}, g, armies=4, tab=6, colours=(1, 2, 3, 5))

# Black Monolith (5P): Black Hole's fortress round a great Obelisk.
Q = [
    "..f....f..^^...",
    ".1.B..c....^...",
    ".B.C......f^...",
    "..C..ff.....^..",
    ".A....f..c..^..",
    "..ff........f..",
    "....c.^^..f....",
    "f.....^^.......",
    "..f.........f..",
    ".c...f..c......",
    "^^^.....f......",
    "..^^...........",
    "...^.c.........",
    "....^^.........",
    "...............",
]
g = mirror(Q, {(0, 0): '1', (1, 0): '2', (0, 1): '3', (1, 1): '4'})
put(g, 10, 10, [
    "....N.....",
    ".X......X.",
    "..B....C..",
    "....###...",
    "W...#O#..E",
    "..C.###.B.",
    ".....5....",
    ".X......X.",
    "....S.....",
    "......A...",
])
emit('Black Monolith', {a: [1, 1, 2, 5, 6, 10] for a in range(1, 6)}, g)

open(os.path.join(HERE, 'maps.txt'), 'w').write(
    '# tangoAW2 5-army maps, generated by design_maps.py (legend in map.py).\n\n' + '\n'.join(out))
print('wrote', len([l for l in out if l.startswith('map ')]), 'maps')
