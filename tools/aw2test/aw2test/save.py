"""Cartridge save (64 KiB flash) and Design Room map records.

Sectors of 0x1000 bytes: u32 magic "2ars" at +0, 0x55/0xAA at +4 with its
complement at +0xFFF, 0x0F at +5, 8-bit sum of the whole sector at +6 and its
complement at +7, u32 generation at +8, part at +0xC, tag at +0xD (design slots
1..3 = tags 5..7), destination offset u16 at +0xE, payload length u16 at +0x50,
payload at +0x52. The newest generation of a tag wins.

A design map record (0x724 bytes): width, height, 600 x u16 tiles (row-major),
name at +0x4B2, army count +0x4C3, colours +0x4C4 (5 bytes; [0] is tangoAW2's
five-army marker), property counts +0x4C9/+0x4CA, 600 unit bytes at +0x4CB
(type | (army-1) << 6; army 5 = 0xE0 | type).
"""

import struct

from . import rom as romlib

MAGIC = 0x73726132
SECTOR = 0x1000
RECORD_LEN = 0x724
PROPERTY_CLASSES = (6, 8, 10, 11, 14, 20)


def _fix_checksum(buf, s):
    base = s * SECTOR
    sec = buf[base:base + SECTOR]
    s0 = (sum(sec) - sec[6] - sec[7]) & 0xFF
    b6 = (s0 - 1) & 0xFF  # sum(all) including b6 and ~b6 (0xFF) == b6
    buf[base + 6] = b6
    buf[base + 7] = (~b6) & 0xFF
    assert sum(buf[base:base + SECTOR]) & 0xFF == buf[base + 6]


def sectors(buf):
    for s in range(len(buf) // SECTOR):
        b = buf[s * SECTOR:(s + 1) * SECTOR]
        if struct.unpack_from("<I", b, 0)[0] == MAGIC:
            yield s, b[0xD], struct.unpack_from("<I", b, 8)[0]


def write_design_map(save, record, slot=1):
    """Return a copy of `save` whose design slot `slot` (1..3) holds `record`."""
    assert len(record) == RECORD_LEN
    tag = slot + 4
    buf = bytearray(save)
    best = None
    for s, t, gen in sectors(buf):
        if t == tag and (best is None or gen > best[1]):
            best = (s, gen)
    if best is None:
        # A fresh sector: the first one that is still erased.
        free = [s for s in range(len(buf) // SECTOR) if set(buf[s * SECTOR:(s + 1) * SECTOR]) == {0xFF}]
        if not free:
            raise ValueError("no free sector for the design map")
        s = free[-1]
        base = s * SECTOR
        buf[base:base + SECTOR] = bytes(SECTOR)
        struct.pack_into("<I", buf, base, MAGIC)
        buf[base + 4] = 0xAA
        buf[base + 5] = 0x0F
        buf[base + 0xFFF] = 0x55
        struct.pack_into("<I", buf, base + 8, 1)
        buf[base + 0xD] = tag
        buf[base + 0xFEF:base + 0xFFF] = b"\xff" * 16
    else:
        s = best[0]
    base = s * SECTOR
    gen = struct.unpack_from("<I", buf, base + 8)[0]
    struct.pack_into("<I", buf, base + 0x10 + 4 * s, gen)
    struct.pack_into("<H", buf, base + 0x50, RECORD_LEN)
    buf[base + 0x52:base + 0x52 + RECORD_LEN] = record
    _fix_checksum(buf, s)
    # The sector directory (tag per sector at +0xFEF, generations at +0x10) is
    # read from the newest main save (tag 0) (sub_0801B2FC): list this sector there.
    mains = [(g, m) for m, t, g in sectors(buf) if t == 0]
    if not mains:
        raise ValueError("no main save (tag 0) in the base save")
    for g, m in mains:
        mb = m * SECTOR
        buf[mb + 0xFEF + s] = tag
        struct.pack_into("<I", buf, mb + 0x10 + 4 * s, gen)
        _fix_checksum(buf, m)
    return bytes(buf)


class DesignMap:
    """A Versus design map: terrain, owners and units, as the Design Room saves it."""

    def __init__(self, width=30, height=20, name="aw2test", image=None, fill="plain"):
        assert width * height <= 600
        self.w, self.h = width, height
        self.name = name
        self.image = image or romlib.Image.load()
        t = self.image.tile_for(fill)
        self.tiles = [t] * (width * height)
        self.units = [0] * (width * height)
        self.colours = [0, 1, 2, 3, 4]
        # tangoAW2's biome (0 Normal, 1 Wasteland), in the record's last
        # byte as 0xB0 | biome; read only with the Dual Strike pack.
        self.biome = 0

    def terrain(self, x, y, kind, owner=0):
        """Set a cell's terrain by name ('wood', 'city', 'hq', ...) or tile id (int)."""
        tile = kind if isinstance(kind, int) else self.image.tile_for(kind, owner)
        self.tiles[y * self.w + x] = tile
        return self

    def terrain_class(self, x, y):
        return self.image.tile_class(self.tiles[y * self.w + x])

    def unit(self, army, kind, x, y):
        t = romlib.unit_id(kind)
        assert 1 <= t <= 0x1F and 1 <= army <= 5
        self.units[y * self.w + x] = (0xE0 | t) if army == 5 else (((army - 1) << 6) | t)
        return self

    def record(self):
        p = bytearray(RECORD_LEN)
        p[0], p[1] = self.w, self.h
        for k, t in enumerate(self.tiles):
            struct.pack_into("<H", p, 2 + 2 * k, t)
        name = self.name.encode("ascii")[:16]
        p[0x4B2:0x4B2 + len(name)] = name
        counts = [0] * 6
        hqs = 0
        armies = set()
        for t in self.tiles:
            c = self.image.tile_class(t)
            if (c & 0x1F) in PROPERTY_CLASSES:
                counts[min(c >> 5, 5)] += 1
            if (c & 0x1F) == 8:
                hqs += 1
        for u in self.units:
            if u:
                armies.add(5 if u & 0xE0 == 0xE0 else (u >> 6) + 1)
        p[0x4C3] = max(hqs, len(armies))
        p[0x4C4:0x4C9] = bytes(self.colours)
        p[0x4C9] = sum(counts)
        p[0x4CA] = max(counts[1:6]) + 1
        p[0x4CB:0x4CB + len(self.units)] = bytes(self.units)
        if self.biome:
            p[0x723] = 0xB0 | self.biome
        return bytes(p)

    def write(self, base_save_path, out_path, slot=1):
        with open(base_save_path, "rb") as f:
            base = f.read()
        data = write_design_map(base, self.record(), slot)
        with open(out_path, "wb") as f:
            f.write(data)
        return out_path
