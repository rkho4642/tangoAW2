#!/usr/bin/env python3
"""Build tangoAW2's 5-army Versus maps (five/maps.txt) into
src/five_map_data.rs.

Usage: map.py <rom.gba>   (the ROM is read for the game's sea-edge table only)

Each map in maps.txt is a `map NAME` line, an `armies N TAB COLOURS...` line
(N armies, the Versus tab 3 Vs. / 5 3P / 6 4P / 9 5P, and each army's
starting colour: 1 Orange Star .. 5 Black Hole), `units ARMY TYPE...` lines,
then its rows, one character per tile:
  ~ sea   r reef   . plain   f wood   ^ mountain
  1..5    the HQ of army 1..5 (Orange Star, Blue Moon, Green Earth,
          Yellow Comet, Black Hole)
  B C A P base, city, airport, port of the army whose HQ is nearest
  b c a p neutral base, city, airport, port
  Black Hole's inventions (theirs by the game's rules), each at its anchor,
  with # over the rest of its footprint:
  S N W E minicannon facing down / up / left / right
  L laser   v n Black Cannon facing down / up (3x3)
  F Black Factory (3x4, anchor on the third row)
  V Volcano (4x4, anchor second column, third row)
  D Deathray (3x3)
  X O     Black Crystal (1 tile) and Black Obelisk (3x3), tangoAW2's healing
          structures (obelisk.rs); they belong to the Black Hole army
Unit types: 1 Infantry, 2 Mech, 3 Md Tank, 5 Tank, 6 Recon, 7 APC,
8 Neotank, 10 Artillery, 11 Rockets, 14 Anti-Air, 15 Missiles, 16 Fighter,
17 Bomber, 19 Battle Copter, 20 Transport Copter, 21 Battleship, 22 Cruiser,
23 Lander, 24 Submarine. Ships start in the sea next to the army's ports,
aircraft next to its airports, the rest nearest its HQ.
"""
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

PROPS = {
    'H': [0x1C0, 0x1C5, 0x1CA, 0x1CF, 0x1D4, 0x1B4],
    'B': [0x1C1, 0x1C6, 0x1CB, 0x1D0, 0x1D5, 0x1B5],
    'C': [0x1C2, 0x1C7, 0x1CC, 0x1D1, 0x1D6, 0x1B6],
    'A': [0x1C3, 0x1C8, 0x1CD, 0x1D2, 0x1D7, 0x1B7],
    'P': [0x1C4, 0x1C9, 0x1CE, 0x1D3, 0x1D8, 0x1B8],
}
PLAIN, PLAIN_SHADE, WOOD, MOUNTAIN, SEA, REEF = 0x001, 0x021, 0x086, 0x022, 0x02A, 0x168
UNDERLAY, RIM = 0x1A4, 0x1A5
# anchor char -> (rows of tiles, anchor column, anchor row)
INVENTIONS = {
    'S': ([[0x182]], 0, 0),
    'N': ([[0x183]], 0, 0),
    'W': ([[0x184]], 0, 0),
    'E': ([[0x185]], 0, 0),
    'L': ([[0x181]], 0, 0),
    'v': ([[UNDERLAY] * 3, [0x186, 0x187, 0x188], [UNDERLAY] * 3], 1, 1),
    'n': ([[UNDERLAY] * 3, [0x189, 0x18A, 0x18B], [UNDERLAY] * 3], 1, 1),
    'F': ([[UNDERLAY, 0x143, UNDERLAY], [UNDERLAY] * 3, [0x18C, 0x18D, 0x18E], [UNDERLAY] * 3], 1, 2),
    'V': ([[RIM] * 4, [RIM, UNDERLAY, UNDERLAY, RIM], [0x1A6, 0x1A7, 0x1A8, 0x1A9], [UNDERLAY] * 4], 1, 2),
    'D': ([[UNDERLAY] * 3, [0x18F, 0x190, 0x191], [UNDERLAY] * 3], 1, 1),
    'X': ([[0x192]], 0, 0),
    'O': ([[UNDERLAY] * 3, [UNDERLAY, 0x193, UNDERLAY], [UNDERLAY] * 3], 1, 1),
}
WATER = set('~r')
SHIPS = {21, 22, 23, 24}
AIR = {16, 17, 19, 20}


def parse(path):
    maps, cur = [], None
    for line in open(path):
        line = line.rstrip('\n')
        if line.startswith('#') or not line.strip():
            continue
        if line.startswith('map '):
            cur = {'name': line[4:].strip(), 'units': {}, 'rows': [], 'armies': 5, 'tab': 9, 'colours': [1, 2, 3, 4, 5]}
            maps.append(cur)
        elif line.startswith('armies '):
            f = [int(x) for x in line.split()[1:]]
            cur['armies'], cur['tab'], cur['colours'] = f[0], f[1], f[2:]
        elif line.startswith('units '):
            f = line.split()
            cur['units'][int(f[1])] = [int(x) for x in f[2:]]
        else:
            cur['rows'].append(line.split()[0])
    return maps


def build(m, edge):
    rows = m['rows']
    W, H = len(rows[0]), len(rows)
    assert all(len(r) == W for r in rows), (m['name'], [(i, len(r)) for i, r in enumerate(rows) if len(r) != W])
    # The AI keeps a row pointer per map row in a 40-entry stack array
    # (sub_080581A4); no shipped map is taller than 39 rows.
    assert W * H <= 1288 and H <= 40 and W <= 64, (m['name'], W, H)
    ch = lambda x, y: rows[y][x] if 0 <= x < W and 0 <= y < H else '~'
    land = lambda x, y: ch(x, y) not in WATER
    hqs = {int(c): (x, y) for y in range(H) for x in range(W) for c in [ch(x, y)] if c in '12345'}
    n = m['armies']
    assert sorted(hqs) == list(range(1, n + 1)) and len(m['colours']) == n, (m['name'], hqs)

    def owner(x, y):
        return min(hqs, key=lambda a: (abs(hqs[a][0] - x) + abs(hqs[a][1] - y), a))

    tiles = [[0] * W for _ in range(H)]
    counts = {a: 0 for a in range(6)}
    for y in range(H):
        for x in range(W):
            c = ch(x, y)
            if c == '~':
                mask = 0
                for i, (dx, dy) in enumerate([(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]):
                    if land(x + dx, y + dy):
                        mask |= 0x100 >> i
                t = edge(mask)
                tiles[y][x] = t if t > 0 else SEA
            elif c == 'r':
                tiles[y][x] = REEF
            elif c == 'f':
                tiles[y][x] = WOOD
            elif c == '^':
                tiles[y][x] = MOUNTAIN
            elif c in '12345':
                tiles[y][x] = PROPS['H'][int(c)]
                counts[int(c)] += 1
            elif c in 'BCAP':
                o = owner(x, y)
                tiles[y][x] = PROPS[c][o]
                counts[o] += 1
            elif c in 'bcap':
                tiles[y][x] = PROPS[c.upper()][0]
                counts[0] += 1
            elif c in INVENTIONS or c == '#':
                tiles[y][x] = UNDERLAY
            else:
                assert c == '.', (m['name'], c, x, y)
                shaded = ch(x - 1, y) in 'f^HBCAPbcap12345#SNWELvnFVDXO'
                tiles[y][x] = PLAIN_SHADE if shaded else PLAIN
    for y in range(H):
        for x in range(W):
            c = ch(x, y)
            if c in INVENTIONS:
                shape, ax, ay = INVENTIONS[c]
                for dy, row in enumerate(shape):
                    for dx, t in enumerate(row):
                        X, Y = x - ax + dx, y - ay + dy
                        assert ch(X, Y) in '#' + c, (m['name'], c, x, y, X, Y, ch(X, Y))
                        tiles[Y][X] = t

    taken = set()
    units = []
    for army in range(1, n + 1):
        units.append(bytes([0xFE, army] + [0] * 10))
        hx, hy = hqs[army]
        near = lambda c: abs(c[0] - hx) + abs(c[1] - hy)
        mine = lambda x, y: owner(x, y) == army
        free_land = sorted(((x, y) for y in range(H) for x in range(W) if ch(x, y) in '.f' and mine(x, y)), key=near)
        ports = [(x, y) for y in range(H) for x in range(W) if ch(x, y) == 'P' and mine(x, y)]
        airports = [(x, y) for y in range(H) for x in range(W) if ch(x, y) == 'A' and mine(x, y)]
        sea = []
        for px, py in ports:
            for dx, dy in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)]:
                c = (px + dx, py + dy)
                if 0 <= c[0] < W and 0 <= c[1] < H and ch(*c) == '~' and c not in sea:
                    sea.append(c)
        sky = sorted({(ax + dx, ay + dy) for ax, ay in airports for dx in range(-2, 3) for dy in range(-2, 3)
                      if 0 <= ax + dx < W and 0 <= ay + dy < H and ch(ax + dx, ay + dy) in '.f'},
                     key=lambda c: min(abs(c[0] - a[0]) + abs(c[1] - a[1]) for a in airports))
        for kind in m['units'].get(army, []):
            pool = sea if kind in SHIPS else (sky or free_land) if kind in AIR else free_land
            spot = next((c for c in pool if c not in taken), None)
            assert spot, (m['name'], army, kind)
            taken.add(spot)
            units.append(bytes([spot[0], spot[1], kind, 0, 0x64, 0x63, 0x63, 0, 0, 4, 0, 0]))
    units.append(bytes([0xFF] + [0] * 11))

    raw = bytes([W, H]) + b''.join(struct.pack('<H', t) for row in tiles for t in row)
    # GBA BIOS LZ77 with every block a literal: flag byte 0, then 8 bytes.
    lz = bytearray(struct.pack('<I', 0x10 | (len(raw) << 8)))
    for i in range(0, len(raw), 8):
        lz.append(0)
        lz += raw[i:i + 8]
    while len(lz) % 4:
        lz.append(0)
    return lz, b''.join(units), (W, H), counts


def main():
    rom = open(sys.argv[1], 'rb').read()
    edge = lambda mask: struct.unpack_from('<h', rom, 0x485DC4 + 2 * mask)[0]
    maps = parse(os.path.join(HERE, 'maps.txt'))
    out = os.path.join(HERE, '..', 'src', 'five_map_data.rs')
    with open(out, 'w') as o:
        o.write('// Generated from five/maps.txt by five/map.py; do not edit.\n\n')
        o.write('pub struct Map {\n    pub name: &\'static str,\n    /// Armies (5: a 5-army map), the Versus tab, the armies\' colours.\n    pub armies: u8,\n    pub tab: u16,\n    pub colours: &\'static [u8],\n    /// The tiles, LZ77 (literal blocks) as the game loads them.\n'
                '    pub tiles: &\'static [u8],\n    /// Pre-deployed units (12-byte records; FE army, FF end).\n    pub units: &\'static [u8],\n}\n\n')
        o.write('pub const MAPS: &[Map] = &[\n')
        for m in maps:
            lz, units, (W, H), counts = build(m, edge)
            o.write(f'    // {m["name"]}: {W}x{H}, properties {counts}\n')
            o.write(f'    Map {{\n        name: "{m["name"]}",\n        armies: {m["armies"]},\n        tab: {m["tab"]},\n        colours: &{m["colours"]},\n        tiles: &[\n')
            for i in range(0, len(lz), 16):
                o.write('            ' + ', '.join(f'0x{b:02X}' for b in lz[i:i + 16]) + ',\n')
            o.write('        ],\n        units: &[\n')
            for i in range(0, len(units), 12):
                o.write('            ' + ', '.join(f'0x{b:02X}' for b in units[i:i + 12]) + ',\n')
            o.write('        ],\n    },\n')
            print(f'{m["name"]}: {W}x{H}, {len(lz)} bytes, {(len(units) // 12) - 6} units, properties {counts}')
        o.write('];\n')


main()
