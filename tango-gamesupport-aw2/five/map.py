#!/usr/bin/env python3
"""Build tangoAW2's 5-army Versus map into src/five_map_data.rs.

Usage: map.py <rom.gba>   (the ROM is read for the game's sea-edge table only)

The map is drawn below in text, one character per tile:
  ~ sea   r reef   . plain   f wood   ^ mountain
  H B C A P   HQ, base, city, airport, port of the army whose band the row is in
  b c a p     neutral base, city, airport, port
  n v #       Black Cannon facing up / down (the centre) and its footprint
Rows 0-7 are Orange Star (army 1), 8-15 Blue Moon (2), 16-23 Black Hole (5),
24-31 Green Earth (3) and 32-39 Yellow Comet (4). The bottom half is the top
half upside down, so every army has the same island.
"""
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

TOP = [
    "~~~~~~~~~~~~~~~~~~~~",  # 0
    "~~~~~C..f.C..C~~~~~~",  # 1
    "~~~~C.B..H..B..~~~~~",  # 2
    "~~~~~C.f..^^.A~~~~~~",  # 3
    "~~~~~~~P....P~~~~~~~",  # 4
    "~~c.~~~~~~~~~~~~.c~~",  # 5
    "~~.p~~~~~r~~~~~~p.~~",  # 6
    "~~~~~~~~~~~~~~~~~~~~",  # 7
    "~~~~~~~~P...P~~~~~~~",  # 8
    "~~~~~~C..f...C~~~~~~",  # 9
    "~~~~~B...H.^..B~~~~~",  # 10
    "~~~~~C..f..^..A~~~~~",  # 11
    "~~~~~~C....C.~~~~~~~",  # 12
    "~~a.c~~~~r~~~~~c.a~~",  # 13
    "~~c.p~~~~~~~~~~p.c~~",  # 14
    "~~~~~~~~~~~~~~~~~~~~",  # 15
]
MIDDLE = [
    "~~~~.###.CC.###.~~~~",  # 16
    "~~~P.#n#....#n#.P~~~",  # 17
    "~~~..###.^^.###..~~~",  # 18
    "~~~.C.B..H...B.C.~~~",  # 19
    "~~~.C..f.^^.f..A.~~~",  # 20
    "~~~..###....###..~~~",  # 21
    "~~~~.#v#.CC.#v#.~~~~",  # 22
    "~~~~.###....###.~~~~",  # 23
]
ROWS = TOP + MIDDLE + TOP[::-1]
W, H = 20, len(ROWS)
assert all(len(r) == W for r in ROWS), [i for i, r in enumerate(ROWS) if len(r) != W]
# The AI keeps a row pointer per map row in a 40-entry stack array
# (sub_080581A4), and no shipped map is taller than 39 rows.
assert W * H <= 1288 and H <= 40


def owner_of_row(y):
    if y <= 7:
        return 1
    if y <= 15:
        return 2
    if y <= 23:
        return 5
    if y <= 31:
        return 3
    return 4


# Property tile ids: HQ, base, city, airport, port for neutral and armies 1..5
# (army 5's are tangoAW2's own, 0x1B4..).
PROPS = {
    'H': [0x1C0, 0x1C5, 0x1CA, 0x1CF, 0x1D4, 0x1B4],
    'B': [0x1C1, 0x1C6, 0x1CB, 0x1D0, 0x1D5, 0x1B5],
    'C': [0x1C2, 0x1C7, 0x1CC, 0x1D1, 0x1D6, 0x1B6],
    'A': [0x1C3, 0x1C8, 0x1CD, 0x1D2, 0x1D7, 0x1B7],
    'P': [0x1C4, 0x1C9, 0x1CE, 0x1D3, 0x1D8, 0x1B8],
}
PLAIN, PLAIN_SHADE, WOOD, MOUNTAIN, SEA, REEF = 0x001, 0x021, 0x086, 0x022, 0x02A, 0x168
FILLER = 0x1A4
CANNON = {'n': (0x189, 0x18A, 0x18B), 'v': (0x186, 0x187, 0x188)}
WATER = set('~r')

# Units: Infantry 1, Tank 5, Battleship 21, Cruiser 22, Lander 23, Submarine 24.
LAND_UNITS = [1, 1, 5]
SEA_UNITS = [21, 22, 24, 23]


def main():
    rom = open(sys.argv[1], 'rb').read()
    edge = lambda mask: struct.unpack_from('<h', rom, 0x485DC4 + 2 * mask)[0]
    ch = lambda x, y: ROWS[y][x] if 0 <= x < W and 0 <= y < H else '~'
    land = lambda x, y: ch(x, y) not in WATER

    tiles = [[0] * W for _ in range(H)]
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
            elif c.upper() in PROPS:
                owner = owner_of_row(y) if c.isupper() else 0
                tiles[y][x] = PROPS[c.upper()][owner]
            elif c in 'nv':
                left, mid, right = CANNON[c]
                tiles[y][x - 1], tiles[y][x], tiles[y][x + 1] = left, mid, right
            elif c == '#':
                if tiles[y][x] == 0:
                    tiles[y][x] = FILLER
            else:
                assert c == '.', c
                shaded = ch(x - 1, y) in 'fHBCAPbcap^#'
                tiles[y][x] = PLAIN_SHADE if shaded else PLAIN
    # cannons overwrite their row's footprint after it was filled
    for y in range(H):
        for x in range(W):
            if ch(x, y) in 'nv':
                left, mid, right = CANNON[ch(x, y)]
                tiles[y][x - 1], tiles[y][x], tiles[y][x + 1] = left, mid, right

    # Pre-deployed units: land units next to the HQ, ships next to the ports.
    taken = set()
    units = []
    for army in (1, 2, 3, 4, 5):
        units.append(bytes([0xFE, army] + [0] * 10))
        cells = [(x, y) for y in range(H) for x in range(W) if owner_of_row(y) == army]
        hq = next((x, y) for x, y in cells if ch(x, y) == 'H')
        ports = [(x, y) for x, y in cells if ch(x, y) == 'P']
        near = sorted(((x, y) for x, y in cells if ch(x, y) == '.'), key=lambda c: abs(c[0] - hq[0]) + abs(c[1] - hq[1]))
        for kind, (x, y) in zip(LAND_UNITS, [c for c in near if c not in taken]):
            taken.add((x, y))
            units.append(bytes([x, y, kind, 0, 0x64, 0x63, 0x63, 0, 0, 4, 0, 0]))
        sea = []
        for px, py in ports:
            for dx, dy in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)]:
                c = (px + dx, py + dy)
                if ch(*c) == '~' and c not in taken and c not in sea and 0 <= c[0] < W and 0 <= c[1] < H:
                    sea.append(c)
        for kind, (x, y) in zip(SEA_UNITS, sea):
            taken.add((x, y))
            units.append(bytes([x, y, kind, 0, 0x64, 0x63, 0x63, 0, 0, 4, 0, 0]))
        assert len(units) and len(sea) >= len(SEA_UNITS), f'army {army}: not enough sea by its ports'
    units.append(bytes([0xFF] + [0] * 11))

    raw = bytes([W, H]) + b''.join(struct.pack('<H', t) for row in tiles for t in row)
    # GBA BIOS LZ77 with every block a literal: flag byte 0, then 8 bytes.
    lz = bytearray(struct.pack('<I', 0x10 | (len(raw) << 8)))
    for i in range(0, len(raw), 8):
        lz.append(0)
        lz += raw[i:i + 8]
    while len(lz) % 4:
        lz.append(0)

    out = os.path.join(HERE, '..', 'src', 'five_map_data.rs')
    with open(out, 'w') as o:
        o.write('// Generated by five/map.py; do not edit.\n\n')
        o.write('/// The tiles, LZ77 (literal blocks) as the game loads them.\n')
        o.write('pub const TILES_LZ77: &[u8] = &[\n')
        for i in range(0, len(lz), 16):
            o.write('    ' + ', '.join(f'0x{b:02X}' for b in lz[i:i + 16]) + ',\n')
        o.write('];\n\n/// Pre-deployed units (12-byte records; FE army, FF end).\n')
        o.write('pub const UNITS: &[u8] = &[\n')
        for u in units:
            o.write('    ' + ', '.join(f'0x{b:02X}' for b in u) + ',\n')
        o.write('];\n')
    print(f'{W}x{H}, {len(units) - 6} units, {len(lz)} bytes -> {os.path.relpath(out)}')
    for y, row in enumerate(ROWS):
        print(f'{y:2} {row}')


main()
