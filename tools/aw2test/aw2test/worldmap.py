"""Dual Strike's world map as Dual Strike draws it, and the DS Campaign's
map layer as the game shows it (read from VRAM), to compare them: PSNR, the
mean colour distance, and how much more the colour jumps across tile edges
than Dual Strike's own picture does (8x8 patches show there)."""

import struct

from . import rom as romlib


def _lz_len(b, start=0):
    size = int.from_bytes(b[start + 1:start + 4], "little")
    out, p = 0, start + 4
    while out < size:
        f = b[p]
        p += 1
        for bit in range(8):
            if out >= size:
                break
            if f & (0x80 >> bit):
                out += (b[p] >> 4) + 3
                p += 2
            else:
                out += 1
                p += 1
    return p


def _rgb(c):
    return (c & 31, (c >> 5) & 31, (c >> 10) & 31)


def ds_picture(ds=None):
    """480x240 rows of (r, g, b) 5-bit: ohashi/res_gmap_map1 and _map2 (LZ
    tiles, then an LZ 32x32 tilemap each) with res_gmap's ten palettes."""
    ds = ds or romlib.DualStrike()
    res = ds.files["ohashi/res_gmap"]
    pal = [_rgb(c) for c in struct.unpack_from("<160H", res, 0x3B08)]
    img = [[(0, 0, 0)] * 480 for _ in range(240)]
    for half, name in enumerate(("res_gmap_map1", "res_gmap_map2")):
        b = ds.files["ohashi/" + name]
        tiles = romlib.lz10(b)
        ents = struct.unpack("<1024H", romlib.lz10(b[(_lz_len(b) + 3) & ~3:]))
        for i, e in enumerate(ents):
            tx, ty = half * 32 + i % 32, i // 32
            if tx >= 60 or ty >= 30:
                continue
            k, hf, vf, p = e & 0x3FF, e & 0x400, e & 0x800, min(e >> 12, 9)
            for y in range(8):
                for x in range(8):
                    sx, sy = (7 - x if hf else x), (7 - y if vf else y)
                    v = (tiles[32 * k + 4 * sy + sx // 2] >> (4 * (sx & 1))) & 15
                    img[8 * ty + y][8 * tx + x] = pal[16 * p + v]
    return img


def from_vram(vram, palram, char_base=0x8000, screen=0xF000):
    """BG3 of the map screen (4bpp, 64x32 in two screens) from a VRAM and a
    BG palette dump, 480x240."""
    pal = [_rgb(c) for c in struct.unpack("<256H", palram[:0x200])]
    tm = struct.unpack("<2048H", vram[screen:screen + 0x1000])
    img = [[(0, 0, 0)] * 480 for _ in range(240)]
    for scr in range(2):
        for i in range(1024):
            e = tm[scr * 1024 + i]
            tx, ty = scr * 32 + i % 32, i // 32
            if tx >= 60 or ty >= 30:
                continue
            k, hf, vf, p = e & 0x3FF, e & 0x400, e & 0x800, e >> 12
            t = vram[char_base + 32 * k:char_base + 32 * k + 32]
            for y in range(8):
                for x in range(8):
                    sx, sy = (7 - x if hf else x), (7 - y if vf else y)
                    v = (t[4 * sy + sx // 2] >> (4 * (sx & 1))) & 15
                    img[8 * ty + y][8 * tx + x] = pal[16 * p + v]
    return img


def compare(a, b):
    """(PSNR, mean colour distance on 0..255, the extra jump across tile
    edges) of picture b against picture a."""
    s = n = dist = 0.0
    for ra, rb in zip(a, b):
        for ca, cb in zip(ra, rb):
            d = [(x - y) * 255 / 31 for x, y in zip(ca, cb)]
            q = sum(v * v for v in d)
            s += q
            dist += q ** 0.5
            n += 1
    mse = s / (3 * n)
    import math
    psnr = 99.0 if mse == 0 else 10 * math.log10(255 ** 2 / mse)

    def seams(img):
        t = c = 0.0
        for y in range(240):
            for x in range(8, 480, 8):
                t += sum(abs(p - q) for p, q in zip(img[y][x], img[y][x - 1])) * 255 / 31 / 3
                c += 1
        for y in range(8, 240, 8):
            for x in range(480):
                t += sum(abs(p - q) for p, q in zip(img[y][x], img[y - 1][x])) * 255 / 31 / 3
                c += 1
        return t / c
    return psnr, dist / n, seams(b) - seams(a)
