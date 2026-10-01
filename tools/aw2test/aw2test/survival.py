"""Dual Strike's Survival (tangoAW2's survival.rs) for the tests: the maps and
runs read straight from the .nds (independently of the Rust conversion), and
driving the mode from Select Mode through its War Room screens.

Dual Strike (USA), overlay 0 at 0x022AD560: map records of 0xA0 bytes at
0x022DBDB0 (DS map id n at index n - 1), the three runs' lists of eleven
u16 ids (Time 0x022F64FC, Money 0x022F652C, Turn 0x022F6514), the text table
0x02306954; arm9: the budgets at 0x02168D04.
"""

import struct

from . import paths
from .rom import DS_OVERLAY0_BASE, DualStrike, lz10

# survival.rs's RAM
STATE = 0x0203FA00
ON, KIND, STAGE, PHASE = STATE, STATE + 1, STATE + 2, STATE + 3
LEFT, BUDGET, POINTS, FRAMES = STATE + 4, STATE + 8, STATE + 0x0C, STATE + 0x10
CO, MENU_PICKED, BONUS, RANK = STATE + 0x18, STATE + 0x19, STATE + 0x1C, STATE + 0x20
CHOOSING, BETWEEN, PLAYING, CLEARED, LOST = range(5)
PROFILE_RECORDS = 0x0200C435
RECORD_MAGIC = 0xD5

# Kinds in Dual Strike's order (its kind byte), and their list order.
TIME, MONEY, TURN = 0, 1, 2
LIST_ORDER = (MONEY, TURN, TIME)
NAMES = {TIME: "Time Survival", MONEY: "Money Survival", TURN: "Turn Survival"}
ENTRY_IDS = {MONEY: 0xC9, TURN: 0xCA, TIME: 0xCB}
MAPS_FROM = 0xCC

SELECT_MODE_CURSOR = 0x0300591C
SURVIVAL_POSITION = 6
SELECT_MAP_PROC = 0x08616C54
CO_SCREEN_PROC = 0x086165C0
PROCS = (0x0200D610, 0x0200E418)
MAIN_CALLBACK = 0x03000000
MAP_CALLBACK = 0x08022049
DAY = 0x03004080
BIOME = 0x03004493
WEATHER_MODE = 0x03003FED
WEATHER_DEFAULT = 0x03003FEF
LIST_IDS = 0x02027F78
LIST_LAST = 0x02027FAB

MAPS = 0x022DBDB0
TEXT = 0x02306954
LISTS = {TIME: 0x022F64FC, MONEY: 0x022F652C, TURN: 0x022F6514}
BUDGETS = 0x02168D04


def convert_tile(t):
    if t == 0x146:
        return 0x086          # a tall wood of Dual Strike's: wood
    if t == 0x1A1:
        return 0x192          # Black Crystal (obelisk.rs)
    if 0x1B9 <= t <= 0x1BD:
        return t - 0x1B9 + 0x1D9  # Com Tower: the Lab tiles (com_tower.rs)
    return t


def convert_unit(t):
    return {25: 26, 26: 27}.get(t, t)


class Survival:
    """Dual Strike's Survival data, read from the .nds."""

    def __init__(self):
        ds = DualStrike(paths.ds_rom())
        self.ov0, self.arm9 = ds.ov0, ds.arm9

    def u8(self, a):
        return self.ov0[a - DS_OVERLAY0_BASE]

    def u16(self, a):
        return struct.unpack_from("<H", self.ov0, a - DS_OVERLAY0_BASE)[0]

    def u32(self, a):
        return struct.unpack_from("<I", self.ov0, a - DS_OVERLAY0_BASE)[0]

    def text(self, i):
        o = self.u32(TEXT + 4 * i) - DS_OVERLAY0_BASE
        return self.ov0[o:self.ov0.index(b"\0", o)].decode("latin1")

    def run(self, kind):
        return [self.u16(LISTS[kind] + 2 * k) for k in range(11)]

    def budget(self, kind):
        return struct.unpack_from("<I", self.arm9, BUDGETS - 0x02000000 + 4 * kind)[0]

    def all_ids(self):
        return sorted({i for k in (TIME, MONEY, TURN) for i in self.run(k)})

    def map(self, ds_id):
        e = MAPS + 0xA0 * (ds_id - 1)
        tiles_at = self.u32(e + 0x5C) - DS_OVERLAY0_BASE
        raw = lz10(self.ov0[tiles_at:tiles_at + 0x2000])
        w, h = raw[0], raw[1]
        tiles = [convert_tile(struct.unpack_from("<H", raw, 2 + 2 * k)[0]) for k in range(w * h)]
        units, army = [], 0
        p = self.u32(e + 0x64) - DS_OVERLAY0_BASE
        while self.ov0[p] != 0xFF:
            r = self.ov0[p:p + 13]
            if r[0] == 0xFE:
                army = r[1]
            else:
                units.append((army, r[0], r[1], convert_unit(r[2])))
            p += 13
        return {
            "name": self.text(self.u16(e + 0x2C)), "w": w, "h": h, "tiles": tiles, "units": units,
            "armies": self.u8(e + 0x3C), "colours": [self.u8(e + k) for k in range(1, 5)],
            "look": self.u8(e + 0x32), "weather": self.u8(e + 0x33), "fog": self.u8(e + 0x34),
            "cos": (self.u8(e + 0x70), self.u8(e + 0x72)),
        }

    def map_id(self, kind, stage):
        """tangoAW2's map id of a run's map `stage`."""
        if stage == 0:
            return ENTRY_IDS[kind]
        return MAPS_FROM + self.all_ids().index(self.run(kind)[stage])


def running(e, script):
    lo, hi = PROCS
    b = e.read(lo, hi - lo)
    return any(struct.unpack_from("<I", b, o)[0] == script for o in range(0, len(b), 0x6C))


def state(e):
    b = e.read(STATE, 0x24)
    u = lambda o: struct.unpack_from("<I", b, o)[0]
    return {"on": b[0], "kind": b[1], "stage": b[2], "phase": b[3], "left": u(4), "budget": u(8),
            "points": u(0x0C), "time": u(0x10), "co": b[0x18], "bonus": u(0x1C), "rank": b[0x20]}


def to_select_mode(e):
    e.wait(700)
    e.press("START", 8)
    e.wait(300)
    e.press("A", 8)
    e.wait(150)


def wheel_to(e, position, tries=10):
    for _ in range(tries):
        if e.u8(SELECT_MODE_CURSOR) == position:
            return True
        e.press("UP", 8)
        e.wait(60)
    return e.u8(SELECT_MODE_CURSOR) == position


def open_survival(e):
    """Select Mode -> Survival: its SELECT MAP. True once the list is up."""
    to_select_mode(e)
    if not wheel_to(e, SURVIVAL_POSITION):
        return False
    e.press("A", 8)
    if not e.wait_until(lambda: running(e, SELECT_MAP_PROC), 600, step=10):
        return False
    e.wait(90)  # the list slides in before it takes the pad
    return True


def listed(e):
    """The ids SELECT MAP lists now."""
    n = e.u8(LIST_LAST) + 1
    return list(e.read(LIST_IDS, n))


def battle_up(e):
    return e.u32(MAIN_CALLBACK) == MAP_CALLBACK and e.u8(PHASE) == PLAYING


def pick(e, row, co_steps=0):
    """Pick list row `row` (from the top), then (on a run's first map) the CO
    screen's CO, `co_steps` down its list: the battle starts. True once the
    battle map is up. From the second map on the screen offers the run's CO
    only."""
    for _ in range(row):
        e.press("DOWN", 8)
        e.wait(30)
    e.press("A", 8)
    for _ in range(60):
        if battle_up(e):
            return True
        if running(e, CO_SCREEN_PROC):
            e.wait(60)
            for _ in range(co_steps):
                e.press("DOWN", 8)
                e.wait(30)
            e.press("A", 8)
            e.wait(100)
            e.press("A", 8)
            break
        e.wait(10)
    return e.wait_until(lambda: battle_up(e), 1500, step=20)


def to_select_map(e, max_steps=80):
    """Press A through the results, the War Room's saving and back to SELECT
    MAP."""
    for _ in range(max_steps):
        if running(e, SELECT_MAP_PROC):
            e.wait(60)
            return True
        e.press("A", 8)
        e.wait(30)
    return False


def records(e):
    """The three kinds' records in the profile: {kind: (rank, co, left)}."""
    b = e.read(PROFILE_RECORDS, 10)
    if b[0] != RECORD_MAGIC:
        return {}
    unit = {TIME: 60, MONEY: 100, TURN: 1}
    out = {}
    for k in (TIME, MONEY, TURN):
        v = b[1 + 3 * k] | b[2 + 3 * k] << 8 | b[3 + 3 * k] << 16
        if v & 7:
            out[k] = (v & 7, (v >> 3) & 0x7F, (v >> 10) * unit[k])
    return out
