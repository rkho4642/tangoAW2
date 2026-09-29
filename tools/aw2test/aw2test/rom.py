"""Tables read from the Advance Wars 2 (USA) ROM image and the Dual Strike (USA) .nds.

Nothing here is copied into the repo: every number is read at run time from the
player's own ROM files. Offsets are from the aw2bhr decompilation (include/unit.h,
include/co.h, src/battle.c, src/unit.c, src/map.c).
"""

import os
import struct

from . import paths

ROM_BASE = 0x08000000

# Unit table: 25 records x 0x5C at 0x085D5ABC (record = unit type id).
UNIT_TABLE = 0x085D5ABC
UNIT_RECORD = 0x5C
# Terrain info: 32 x 0x14 at 0x085D583C, +0x10 = int defence stars.
TERRAIN_INFO = 0x085D583C
# Tile id -> terrain class (u8 x 0x400) in ROM (copied to RAM 0x020233B0).
TILE_CLASS = 0x080C1BC4
# CO data: 19 x 0x104 at 0x085D3DD0; power[3] x 0x44 at +0x38.
CO_TABLE = 0x085D3DD0
CO_RECORD = 0x104
# Versus Teams-screen CO order (19 ids).
VS_CO_ORDER = 0x084A077C

UNIT_NAMES = {
    1: "Infantry", 2: "Mech", 3: "Md Tank", 4: "Megatank", 5: "Tank", 6: "Recon", 7: "APC",
    8: "Neotank", 9: "Piperunner", 10: "Artillery", 11: "Rockets", 12: "Stealth",
    13: "Black Bomb", 14: "Anti-Air", 15: "Missiles", 16: "Fighter", 17: "Bomber",
    18: "Black Boat", 19: "B Copter", 20: "T Copter", 21: "Battleship", 22: "Cruiser",
    23: "Lander", 24: "Sub", 25: "dived Sub",
}
UNIT_IDS = {v.lower().replace(" ", "").replace("-", ""): k for k, v in UNIT_NAMES.items()}

CO_NAMES = [
    "Nell", "Andy", "Max", "Olaf", "Sami", "Grit", "Kanbei", "Sonja", "Eagle", "Drake",
    "Sturm", "Flak", "Lash", "Adder", "Hawke", "Hachi", "Colin", "Jess", "Sensei",
]
CO_IDS = {n.lower(): i for i, n in enumerate(CO_NAMES)}

TERRAIN_CLASSES = {
    "plain": 1, "river": 2, "mountain": 3, "wood": 4, "road": 5, "city": 6, "sea": 7,
    "hq": 8, "airport": 10, "port": 11, "bridge": 12, "shoal": 13, "base": 14,
    "pipe": 15, "reef": 19, "lab": 20,
}
# Property tiles: 0x1C0 + kind + 5*owner (owner 0 neutral .. 4), kinds below.
PROPERTY_KIND = {"hq": 0, "base": 1, "city": 2, "airport": 3, "port": 4}


def unit_id(name_or_id):
    if isinstance(name_or_id, int):
        return name_or_id
    return UNIT_IDS[name_or_id.lower().replace(" ", "").replace("-", "").replace("_", "")]


def co_id(name_or_id):
    if isinstance(name_or_id, int):
        return name_or_id
    return CO_IDS[name_or_id.lower()]


class Image:
    """A GBA ROM image (the file, or the patched image dumped from the running game)."""

    def __init__(self, data):
        self.data = data

    @classmethod
    def load(cls, path=None):
        with open(path or paths.aw2_rom(), "rb") as f:
            return cls(f.read())

    def at(self, addr, n):
        o = addr - ROM_BASE
        return self.data[o:o + n]

    def u8(self, addr):
        return self.data[addr - ROM_BASE]

    def s8(self, addr):
        v = self.u8(addr)
        return v - 256 if v >= 128 else v

    def u16(self, addr):
        return struct.unpack_from("<H", self.data, addr - ROM_BASE)[0]

    def s16(self, addr):
        return struct.unpack_from("<h", self.data, addr - ROM_BASE)[0]

    def u32(self, addr):
        return struct.unpack_from("<I", self.data, addr - ROM_BASE)[0]

    # -- units ------------------------------------------------------------
    def unit(self, t):
        a = UNIT_TABLE + UNIT_RECORD * t
        return {
            "cost": self.u16(a + 0x06) * 10,
            "move": self.u8(a + 0x0A),
            "ammo": self.u8(a + 0x0B),
            "vision": self.u8(a + 0x0C),
            "min_range": self.u8(a + 0x0E),
            "max_range": self.u8(a + 0x0F),
            "fuel": self.u8(a + 0x10),
            "unit_class": self.u8(a + 0x18),
            "move_type": self.u8(a + 0x19),
            "domain": self.u8(a + 0x1A),
        }

    def damage_row(self, t, weapon):
        """weapon 0 primary, 1 secondary: u8[26] by defender type (25 = dived Sub)."""
        return list(self.at(UNIT_TABLE + UNIT_RECORD * t + 0x1E + 0x1A * weapon, 26))

    # -- terrain ----------------------------------------------------------
    def terrain_stars(self, terrain_class):
        return struct.unpack_from("<i", self.data, TERRAIN_INFO - ROM_BASE + 0x14 * (terrain_class & 0x1F) + 0x10)[0]

    def tile_class(self, tile):
        return self.u8(TILE_CLASS + tile)

    def tile_for(self, kind, owner=0):
        """A tile id of the given terrain kind (name or class number)."""
        if isinstance(kind, str) and kind in PROPERTY_KIND:
            return 0x1C0 + PROPERTY_KIND[kind] + 5 * owner
        cls = TERRAIN_CLASSES[kind] if isinstance(kind, str) else kind
        preferred = {1: 0x001, 3: 0x022, 7: 0x008}
        if cls in preferred and self.tile_class(preferred[cls]) == cls:
            return preferred[cls]
        for t in range(0x400):
            if self.tile_class(t) == cls:
                return t
        raise KeyError(kind)

    # -- COs ----------------------------------------------------------------
    def co_stars(self, co):
        a = CO_TABLE + CO_RECORD * co
        return self.u32(a + 0x0C), self.u32(a + 0x10)

    def co_mode(self, co, mode):
        """CoModeData for mode 0 d2d / 1 COP / 2 SCOP."""
        a = CO_TABLE + CO_RECORD * co + 0x38 + 0x44 * mode
        rows = []
        for i in range(8):
            p = self.u32(a + 0x24 + 4 * i)
            rows.append(tuple(self.s16(p + 2 * k) for k in range(4)))
        return {
            "abilities": self.u32(a + 0x08),
            "luck": self.s16(a + 0x0E),
            "neg_luck": self.s16(a + 0x10),
            "counter": self.s16(a + 0x12),
            "stats": rows,  # [class 0..4, 5 direct, 6 indirect, 7 non-combat] x (fp, def, move, range)
        }

    def text(self, i):
        p = self.u32(0x08610A38 + 4 * i) - ROM_BASE
        return self.data[p:self.data.index(b"\0", p)]

    def menu_label(self, table, i):
        """A menu entry's label (text id at +0x1C of a 0x20-byte entry), without its icon code."""
        t = self.text(self.u32(table + 0x20 * i + 0x1C))
        if t[:1] in (b"\t", b"\n"):
            t = t[2:]
        return t.decode("latin-1").split("\x19")[0]

    def vs_co_order(self):
        return list(self.at(VS_CO_ORDER, 19))


def _combat_column(min_range):
    if min_range > 1:
        return 6
    if min_range == 1:
        return 5
    return 7


def co_stat(image, co, mode, unit_type, which):
    """GetCoAttackBonus (which=0) / GetCoDefenceBonus (which=1)."""
    u = image.unit(unit_type)
    m = image.co_mode(co, mode)
    p = m["stats"][u["unit_class"]][which]
    if u["unit_class"] == 0:
        return p
    return p + m["stats"][_combat_column(u["min_range"])][which]


# ---------------------------------------------------------------------------
# Dual Strike (USA): unit records in overlay 0, read straight from the .nds.
DS_OVERLAY0_BASE = 0x022AD560
DS_UNITS = DS_OVERLAY0_BASE + 0x47A58
DS_RECORD = 0x6C
DS_SUBMERGED_SUB = 27


class DualStrike:
    def __init__(self, path=None):
        with open(path or paths.ds_rom(), "rb") as f:
            rom = f.read()
        if rom[0x0C:0x10] != b"AWRE":
            raise ValueError("not Advance Wars: Dual Strike (USA)")
        fat = struct.unpack_from("<I", rom, 0x48)[0]
        ovt = struct.unpack_from("<I", rom, 0x50)[0]
        file_id = struct.unpack_from("<I", rom, ovt + 0x18)[0]
        a, b = struct.unpack_from("<II", rom, fat + 8 * file_id)
        self.ov0 = rom[a:b]

    def record(self, t):
        o = DS_UNITS - DS_OVERLAY0_BASE + DS_RECORD * t
        return self.ov0[o:o + DS_RECORD]

    def damage_row(self, t, weapon):
        """As AW2's row: index = AW2 defender type 0..25 (25 = dived Sub)."""
        r = self.record(t)
        base = 0x24 if weapon == 0 else 0x44
        out = []
        for d in range(26):
            slot = DS_SUBMERGED_SUB if d == 25 else d
            out.append(0 if slot == 0 else r[base + slot - 1])
        return out
