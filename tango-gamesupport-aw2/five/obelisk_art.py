#!/usr/bin/env python3
"""Draw tangoAW2's Black Crystal (16x32) and Black Obelisk (48x64), after
Advance Wars: Dual Strike's healing structures, into src/obelisk_art.rs.
The art is tangoAW2's own, in Black Hole's invention palette (0x080D3E84):
o dark (5), p purple (4), l lavender (8), c pale (10), w white (1),
r red (14), g grey (3), s shadow (15), . transparent.

Usage: obelisk_art.py [preview.png]
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
KEY = {'.': 0, 'o': 5, 'p': 4, 'l': 8, 'c': 10, 'w': 1, 'r': 14, 'g': 3, 's': 15, 'F': 15, 'y': 6, 'b': 7}

def crystal():
    """The Black Crystal after Dual Strike: a clear hexagonal prism standing in
    a grey cog-shaped mount. 16x32, one tile above its cell."""
    g = [['.'] * 16 for _ in range(32)]

    def put(x, y, c):
        if 0 <= x < 16 and 0 <= y < 32:
            g[y][x] = c
    # the mount's back half, then the prism, then the mount's front
    def ring(front):
        for y in range(20, 31):
            for x in range(16):
                dx, dy = (x + 0.5 - 8) / 7.9, (y + 0.5 - 25) / 4.8
                d = dx * dx + dy * dy
                if d > 1 or (y + 0.5 < 25) == front:
                    continue
                inner = ((x + 0.5 - 8) / 4.6) ** 2 + ((y + 0.5 - 25) / 2.4) ** 2
                c = 'o' if d > 0.78 else ('s' if inner < 1 else 'g')
                if d > 0.78 and (x in (1, 5, 10, 14) or y in (20, 30)):
                    c = 'g'
                put(x, y, c)
    ring(False)
    body = [(3, 7), (6, 1), (10, 1), (13, 7), (13, 23), (10, 25), (6, 25), (3, 23)]
    for y in range(32):
        for x in range(16):
            px, py = x + 0.5, y + 0.5
            ok = True
            sign = 0
            for i in range(len(body)):
                (x1, y1), (x2, y2) = body[i], body[(i + 1) % len(body)]
                c = (x2 - x1) * (py - y1) - (y2 - y1) * (px - x1)
                if c:
                    if sign and (c > 0) != (sign > 0):
                        ok = False
                    sign = c
            if not ok:
                continue
            edge_l, edge_r = x == 4 or (y < 8 and x <= 11 - y), x == 11 or (y < 8 and x >= 4 + y)
            face = 'w' if x < 6 else 'b' if x < 9 else 'c' if x < 11 else 'g'
            put(x, y, face)
    # outline the prism
    snap = [row[:] for row in g]
    for y in range(32):
        for x in range(16):
            if snap[y][x] in 'wbcg' and y < 24 and any(
                    not (0 <= x + i < 16 and 0 <= y + j < 32) or snap[y + j][x + i] in '.os'
                    for i, j in ((1, 0), (-1, 0), (0, -1))):
                g[y][x] = 'o'
    for y in range(7, 23):
        put(6, y, 'l')
        put(9, y, 'l')
    for x, y in [(5, 6), (6, 5), (7, 4), (8, 4), (9, 5), (10, 6), (6, 7), (7, 7), (8, 7), (9, 7)]:
        put(x, y, 'l')
    for x, y in [(4, 10), (4, 11), (5, 14)]:
        put(x, y, 'b')
    ring(True)
    return [''.join(r) for r in g]


CRYSTAL = crystal()


def obelisk():
    """The Black Obelisk after Dual Strike: a pale, jagged crystal cluster on a
    dark metal platform with a yellow dotted line down its front. 48x64: the
    3x3 footprint is the bottom 48 rows, the crystal rises 16 above it. Drawn
    as three sprites (`split`): the middle 32x64 and two 8x32 side strips of
    the platform, so the crystal stays within x 8..39."""
    W, H = 48, 64
    g = [['.'] * W for _ in range(H)]

    def put(x, y, c):
        if 0 <= x < W and 0 <= y < H:
            g[y][x] = c

    # platform: rows 30..61, a grey metal frame seen from above
    for y in range(30, 62):
        for x in range(1, 47):
            put(x, y, 'g')
    for x in range(1, 47):
        put(x, 30, 'o'); put(x, 61, 'o'); put(x, 31, 'b')
    for y in range(30, 62):
        put(1, y, 'o'); put(46, y, 'o'); put(2, y, 'b')
    # the dark well inside the frame
    for y in range(35, 57):
        for x in range(7, 41):
            put(x, y, 'o' if (x + y) % 5 else 's')
    for x in range(7, 41):
        put(x, 35, 'o'); put(x, 56, 'b')
    for y in range(35, 57):
        put(7, y, 'o'); put(40, y, 'b')
    # bars on the frame's sides and front
    for y in range(34, 58, 4):
        for x in (3, 4, 5):
            put(x, y, 'o'); put(x + 39, y, 'o')
    for x in range(10, 38, 5):
        put(x, 58, 'o'); put(x + 1, 58, 'o')
    # yellow dotted line down the front
    for y in range(50, 61, 3):
        put(23, y, 'y'); put(24, y, 'y')

    def inside(px, py, pts):
        sign = 0
        for i in range(len(pts)):
            (x1, y1), (x2, y2) = pts[i], pts[(i + 1) % len(pts)]
            c = (x2 - x1) * (py - y1) - (y2 - y1) * (px - x1)
            if c:
                if sign and (c > 0) != (sign > 0):
                    return False
                sign = c
        return True

    # prisms, back to front: outline points and (lit, front, shaded) faces
    prisms = [
        ([(10, 21), (15, 15), (20, 20), (20, 50), (11, 50)], ('b', 'c', 'g')),
        ([(29, 17), (34, 12), (39, 18), (37, 50), (29, 50)], ('b', 'c', 'g')),
        ([(15, 14), (20, 3), (25, 0), (28, 7), (33, 12), (33, 54), (15, 54)], ('w', 'b', 'g')),
    ]
    for pts, (lit, front, shade) in prisms:
        mask = {(x, y) for y in range(H) for x in range(9, 39) if inside(x + 0.5, y + 0.5, pts)}
        for (x, y) in mask:
            row = [xx for (xx, yy) in mask if yy == y]
            x0, x1 = min(row), max(row)
            f = (x - x0) / max(1, x1 - x0)
            c = lit if f < 0.4 else front if f < 0.75 else shade
            edge = any((x + i, y + j) not in mask for i, j in ((1, 0), (-1, 0), (0, 1), (0, -1)))
            put(x, y, 'o' if edge else c)
    # facet edges of the big prism: its broken top and the vertical ridges
    for y in range(12, 53):
        put(22, y, 'b')
        put(28, y, 'l')
    for x, y in [(20, 8), (21, 9), (22, 10), (23, 11), (27, 6), (27, 7), (28, 8), (28, 9), (28, 10), (28, 11)]:
        put(x, y, 'l')
    # a crack across it
    for i in range(7):
        put(17 + i, 30 + i // 2, 'g')
    return [''.join(r) for r in g]


def split(rows):
    """The three sprites: middle 32x64, left and right 8x32 (rows 32..63)."""
    core = [r[8:40] for r in rows]
    left = [r[0:8] for r in rows[32:]]
    right = [r[40:48] for r in rows[32:]]
    return core, left, right


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
    """The 48x64 obelisk shrunk to 16x32 for the terrain panel and the
    Design Room's bar: every third column, every second row."""
    return [''.join(rows[y][x] for x in range(1, 48, 3)) for y in range(0, 64, 2)]


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
                                  ('OBELISK', None, 'Black Obelisk: a 32x64 sprite (4x8 tiles, 1D), then the platform\'s left and right 8x32 strips (1x4 tiles each), 4bpp'),
                                  ('OBELISK_SMALL', half(ob), 'Black Obelisk at half size, 16x32, for the terrain panel'),
                                  ('CRYSTAL_NAME', name('Crystal'), 'Terrain-panel name, 32x16 (4x2 tiles), font colours'),
                                  ('OBELISK_NAME', name('Obelisk'), 'Terrain-panel name, 32x16 (4x2 tiles), font colours')]:
            data = tiles(rows) if rows is not None else b''.join(tiles(r) for r in split(ob))
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
        im = Image.new('RGB', (128, 80), (160, 200, 120))
        for img_rows, ox, oy in [(CRYSTAL, 4, 32), (ob, 28, 0), (half(ob), 84, 44), (name('Crystal'), 84, 4), (name('Obelisk'), 84, 24)]:
            for y, r in enumerate(img_rows):
                for x, c in enumerate(r):
                    if c != '.':
                        col = (40, 40, 60) if c == 'F' else rgb(pal[KEY[c]])
                        im.putpixel((ox + x, oy + y), col)
        im.resize((768, 480), Image.NEAREST).save(sys.argv[1])
    print('ok')


main()
