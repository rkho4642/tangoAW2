#!/usr/bin/env python3
"""Draw tangoAW2's Black Crystal (16x32) and Black Obelisk (32x64), after
Advance Wars: Dual Strike's healing structures, into src/obelisk_art.rs.
The art is tangoAW2's own, in Black Hole's invention palette (0x080D3E84):
o dark (5), p purple (4), l lavender (8), c pale (10), w white (1),
r red (14), g grey (3), s shadow (15), . transparent.

Usage: obelisk_art.py [preview.png]
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
KEY = {'.': 0, 'o': 5, 'p': 4, 'l': 8, 'c': 10, 'w': 1, 'r': 14, 'g': 3, 's': 15, 'F': 15}

CRYSTAL = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    ".......oo.......",
    "......ollo......",
    "......olco......",
    ".....olcclo.....",
    ".....olcwlo.....",
    "....olcclppo....",
    "....olclpppo....",
    "...olllpppppo...",
    "...olcppppppo...",
    "....olpppppo....",
    "....olpppppo....",
    ".....olpppo.....",
    ".....olpppo.....",
    "......olpo......",
    "......oppo......",
    ".......oo.......",
    "....l......l....",
    "......l..l......",
    ".....gggggg.....",
    "...gggggggggg...",
    "..gssggggggssg..",
    "..gssssssssssg..",
    "...gggggggggg...",
    "....ssssssss....",
    "................",
    "................",
]


def obelisk():
    w, h = 32, 64
    g = [['.'] * w for _ in range(h)]
    cx = 16
    # pyramidion (rows 2..11), shaft (12..49), base (50..61)
    for y in range(2, 50):
        if y < 12:
            half = 1 + (y - 2) * 7 // 10
        else:
            half = 7 + (y - 12) * 3 // 38
        for x in range(cx - half, cx + half):
            g[y][x] = 'p' if x < cx else 'o'
        g[y][cx - half] = 'o'
        g[y][cx + half - 1] = 'o'
        if y >= 12:
            g[y][cx - half + 1] = 'l' if y % 6 else 'c'
    # the eye and the runes
    for y, x, c in [(16, 15, 'r'), (16, 16, 'r'), (15, 15, 'w'), (17, 16, 'r')]:
        g[y][x] = c
    for y in range(22, 46, 4):
        g[y][14] = 'l'
        g[y][17] = 'c'
        g[y + 1][15] = 'l'
        g[y + 1][16] = 'l'
    g[3][16] = 'w'
    g[4][15] = 'c'
    # base: two steps of stone
    for y in range(50, 62):
        half = 11 if y < 56 else 14
        for x in range(cx - half, cx + half):
            g[y][x] = 'g'
        g[y][cx - half] = 's'
        g[y][cx + half - 1] = 's'
    for x in range(cx - 11, cx + 11):
        g[55][x] = 's'
    for x in range(cx - 14, cx + 14):
        g[61][x] = 's'
    # glow round the foot
    for x in (3, 5, 26, 28):
        g[48][x] = 'l'
    return [''.join(r) for r in g]


# Terrain-panel names (32x16, the font's style: a big capital, then small
# letters at a 4-pixel pitch, white (1) outlined (f)). Fill pixels only; the
# outline is added round them.
BIG = {
    'C': [".1111.", "11..11", "11....", "11....", "11....", "11....", "11..11", ".1111."],
    'O': [".1111.", "11..11", "11..11", "11..11", "11..11", "11..11", "11..11", ".1111."],
}
SMALL = {  # rows 8..11 (and a descender row 12), 3 wide
    'r': ["1.1", "11.", "1..", "1.."],
    'y': ["1.1", "1.1", "111", "..1", "11."],
    's': [".11", "11.", ".11", "11."],
    't': [".1.", "111", ".1.", ".11"],
    'a': [".11", "1.1", "1.1", ".11"],
    'e': [".1.", "111", "1..", ".11"],
    'i': [".1.", ".1.", ".1.", ".1."],
    'k': ["1.1", "11.", "11.", "1.1"],
}
TALL = {'l': 4, 'b': 4, 'k': 4, 'i': 0}  # extra rows above for tall letters


def name(word):
    g = [[0] * 32 for _ in range(16)]
    for dy, row in enumerate(BIG[word[0]]):
        for dx, c in enumerate(row):
            if c == '1':
                g[3 + dy][1 + dx] = 1
    for n, ch in enumerate(word[1:]):
        x0 = 8 + 4 * n
        if ch == 'l':
            for y in range(4, 12):
                g[y][x0 + 1] = 1
            g[11][x0 + 2] = 1
            continue
        if ch == 'b':
            for y in range(4, 12):
                g[y][x0] = 1
            rows = ["11.", "1.1", "1.1", "11."]
        else:
            rows = SMALL[ch]
        if ch == 'k':
            for y in range(4, 8):
                g[y][x0] = 1
        if ch == 'i':
            g[6][x0 + 1] = 1
        for dy, row in enumerate(rows):
            for dx, c in enumerate(row):
                if c == '1':
                    g[8 + dy][x0 + dx] = 1
    out = [[0] * 32 for _ in range(16)]
    for y in range(16):
        for x in range(32):
            if g[y][x]:
                out[y][x] = 1
            elif any(0 <= y + j < 16 and 0 <= x + i < 32 and g[y + j][x + i] for j in (-1, 0, 1) for i in (-1, 0, 1)):
                out[y][x] = 15
    return [''.join('.' if v == 0 else ('w' if v == 1 else 'F') for v in r) for r in out]


def half(rows):
    """The 32x64 obelisk at half size, 16x32, for the terrain panel."""
    return [''.join(rows[y][x] for x in range(0, 32, 2)) for y in range(0, 64, 2)]


def tiles(rows):
    h, w = len(rows), len(rows[0])
    out = bytearray()
    for ty in range(h // 8):
        for tx in range(w // 8):
            for y in range(8):
                for x in range(0, 8, 2):
                    a = KEY[rows[ty * 8 + y][tx * 8 + x]]
                    b = KEY[rows[ty * 8 + y][tx * 8 + x + 1]]
                    out.append(a | (b << 4))
    return bytes(out)


def main():
    ob = obelisk()
    assert all(len(r) == 16 for r in CRYSTAL) and len(CRYSTAL) == 32
    out = os.path.join(HERE, '..', 'src', 'obelisk_art.rs')
    with open(out, 'w') as o:
        o.write('// Generated by five/obelisk_art.py; do not edit.\n\n')
        for name_, rows, what in [('CRYSTAL', CRYSTAL, 'Black Crystal: one 16x32 sprite (2x4 tiles, 1D), 4bpp'),
                                  ('OBELISK', ob, 'Black Obelisk: one 32x64 sprite (4x8 tiles, 1D), 4bpp'),
                                  ('OBELISK_SMALL', half(ob), 'Black Obelisk at half size, 16x32, for the terrain panel'),
                                  ('CRYSTAL_NAME', name('Crystal'), 'Terrain-panel name, 32x16 (4x2 tiles), font colours'),
                                  ('OBELISK_NAME', name('Obelisk'), 'Terrain-panel name, 32x16 (4x2 tiles), font colours')]:
            data = tiles(rows)
            o.write(f'/// {what}.\npub const {name_}: [u8; {len(data)}] = [\n')
            for i in range(0, len(data), 16):
                o.write('    ' + ', '.join(f'0x{b:02X}' for b in data[i:i + 16]) + ',\n')
            o.write('];\n\n')
    if len(sys.argv) > 1:
        from PIL import Image
        import struct
        rom = open(os.path.expanduser('~/Documents/TangoAW2/roms/Advance_wars_2.gba'), 'rb').read()
        pal = struct.unpack_from('<16H', rom, 0xD3E84)
        rgb = lambda c: ((c & 31) * 8, ((c >> 5) & 31) * 8, ((c >> 10) & 31) * 8)
        im = Image.new('RGB', (128, 64), (160, 200, 120))
        for img_rows, ox, oy in [(CRYSTAL, 4, 32), (ob, 28, 0), (half(ob), 64, 32), (name('Crystal'), 84, 8), (name('Obelisk'), 84, 30)]:
            for y, r in enumerate(img_rows):
                for x, c in enumerate(r):
                    if c != '.':
                        col = (40, 40, 60) if c == 'F' else rgb(pal[KEY[c]])
                        im.putpixel((ox + x, oy + y), col)
        im.resize((768, 384), Image.NEAREST).save(sys.argv[1])
    print('ok')


main()
