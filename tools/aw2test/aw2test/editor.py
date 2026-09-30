"""Driving the Design Room's map editor (Select Mode > Design Room > Map).

Closed-loop like the rest of the driver: every step waits for or checks its
RAM sign (the editor's state block, `0x0200B000`; docs/AW2.md "Design Room").
The editor's File menu has no cursor the driver reads, so saving and loading
are checked by what they do (the record in the exported save, the loaded map).
"""

from .game import NavError, SELECT_MODE_CURSOR

EDITOR = 0x0200B000
STATE = EDITOR + 0x04        # 1 on the map, 2 a tool bar open, 3 menu
BAR = EDITOR + 0x07          # 0 terrain bar, 1 unit bar
CURSOR_X = EDITOR + 0x08
CURSOR_Y = EDITOR + 0x0A
TOOL = EDITOR + 0x2A         # the picked terrain: class | owner << 5
TERRAIN_ARMY = EDITOR + 0x2E  # 0 neutral .. 5 Black Hole
UNIT_ARMY = EDITOR + 0x2F    # 1 .. 5
TERRAIN_WINDOW = EDITOR + 0x36
UNIT_WINDOW = EDITOR + 0x38
SHOWN = 0x0200B0D0           # the bar's ten shown entries, 0x1C bytes (word at +4)
LIST = 0x0203FF00            # the bar's list, tangoAW2's (crate::design_bar): (word, tile)
SELECT_MODE_DESIGN_ROOM = 2
MAP = 0x0201E450

TOWER = 0x14                 # a Lab: the Com Tower with the Dual Strike pack
WASTELAND = 0x103            # tangoAW2's Wasteland switch entry
HQ = 0x08
BIOME = 0x03004493           # crate::sandstorm::STATE: bits 4..6 the biome (crate::wasteland)


class Editor:
    def __init__(self, emu):
        self.e = emu

    # -- getting there ------------------------------------------------------
    def boot(self):
        e = self.e
        e.wait(700)
        e.press("START", 8)
        e.wait(300)
        e.press("A", 8)
        e.wait(150)
        for _ in range(8):
            if e.u8(SELECT_MODE_CURSOR) == SELECT_MODE_DESIGN_ROOM:
                break
            e.press("DOWN", 8)
            e.wait(50)
        if e.u8(SELECT_MODE_CURSOR) != SELECT_MODE_DESIGN_ROOM:
            raise NavError(f"Select Mode cursor at {e.u8(SELECT_MODE_CURSOR)}")
        e.press("A", 8)  # Design Room -> Map / CO
        e.wait(150)
        e.press("A", 8)  # Map
        e.wait(150)
        # The designer's welcome, then the map.
        for _ in range(30):
            if e.u8(STATE) == 1:
                break
            e.press("A", 8)
            e.wait(40)
        if e.u8(STATE) != 1:
            raise NavError(f"editor state {e.u8(STATE)}")
        e.wait(30)

    # -- reading ------------------------------------------------------------
    def entries(self):
        """The terrain bar's length as the editor has it (crate::design_bar)."""
        return self.e.u16(0x08000CEA) & 0xFF

    def highlighted(self):
        """The terrain bar's highlighted (word, tile)."""
        i = (self.e.u8(TERRAIN_WINDOW) + 4) % self.entries()
        return self.e.u16(LIST + 4 * i), self.e.u16(LIST + 4 * i + 2)

    def highlighted_shown(self):
        """The highlighted entry as the bar shows it (its shown copy's word)."""
        first = self.e.s16(EDITOR + 0x3A) + 4
        if first > 9:
            first -= 10
        return self.e.u16(SHOWN + 0x1C * first + 4)

    def list(self):
        n = self.entries()
        raw = self.e.read(LIST, 4 * n)
        return [(int.from_bytes(raw[4 * i:4 * i + 2], "little"), int.from_bytes(raw[4 * i + 2:4 * i + 4], "little"))
                for i in range(n)]

    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)

    def tile(self, x, y):
        row = self.e.u16(MAP + 0x417A + 2 * y)
        return self.e.u16(MAP + 0xA22 + 2 * (row + x)) & 0x1FF

    def terrain_class(self, x, y):
        row = self.e.u16(MAP + 0x417A + 2 * y)
        return self.e.u8(MAP + 0x1432 + row + x)

    def biome(self):
        """0 Normal, 1 Wasteland."""
        return (self.e.u8(BIOME) >> 4) & 7

    def cursor(self):
        return self.e.u16(CURSOR_X), self.e.u16(CURSOR_Y)

    # -- bars ---------------------------------------------------------------
    def open_bar(self, units=False):
        e = self.e
        if e.u8(STATE) != 2:
            e.press("R", 8)
            e.wait(40)
        want = 1 if units else 0
        for _ in range(3):
            if e.u8(BAR) == want:
                break
            e.press("L", 8)
            e.wait(40)
        if e.u8(STATE) != 2 or e.u8(BAR) != want:
            raise NavError(f"bar not open: state {e.u8(STATE)} bar {e.u8(BAR)}")

    def close_bar(self):
        if self.e.u8(STATE) == 2:
            self.e.press("B", 8)
            self.e.wait(30)

    def to_entry(self, kind):
        """Scroll the terrain bar until the highlighted entry is `kind` (word & 0x11F)."""
        self.open_bar()
        for _ in range(self.entries() + 2):
            if self.highlighted()[0] & 0x11F == kind:
                return
            self.e.press("LEFT", 6)
            self.e.wait(12)
        raise NavError(f"no entry {kind:#x} in the terrain bar {[hex(w) for w, _ in self.list()]}")

    def step_army(self, key="UP"):
        """UP (or DOWN, or SELECT) on the highlighted entry: the bar's army steps."""
        before = self.e.u8(TERRAIN_ARMY)
        self.e.press(key, 6)
        self.e.wait(30)
        return before != self.e.u8(TERRAIN_ARMY)

    def set_army(self, owner, key="UP"):
        for _ in range(8):
            if self.e.u8(TERRAIN_ARMY) == owner:
                return
            if not self.step_army(key):
                raise NavError(f"{key} did not change the bar's army (highlighted {self.highlighted()})")
        raise NavError(f"bar army {self.e.u8(TERRAIN_ARMY)}, wanted {owner}")

    def pick(self):
        """A on the open bar: its highlighted entry becomes the tool."""
        self.e.press("A", 8)
        if not self.e.wait_until(lambda: self.e.u8(STATE) == 1, 120, step=6):
            raise NavError("the bar did not close on A")
        self.e.wait(10)

    def goto(self, x, y):
        for _ in range(100):
            c = self.cursor()
            if c == (x, y):
                return
            key = "RIGHT" if c[0] < x else "LEFT" if c[0] > x else "DOWN" if c[1] < y else "UP"
            self.e.hold(key, 6)
            self.e.wait(10)
        raise NavError(f"editor cursor at {self.cursor()}, wanted {(x, y)}")

    def stamp(self, x, y):
        self.goto(x, y)
        self.e.press("A", 8)
        self.e.wait(20)

    def place(self, kind, owner, x, y):
        """Pick `kind` from the terrain bar for `owner` (None: not a
        property) and put it at (x, y)."""
        self.to_entry(kind)
        if owner is not None:
            self.set_army(owner)
        self.pick()
        self.stamp(x, y)

    def place_unit(self, army, unit_type, x, y):
        e = self.e
        self.open_bar(units=True)
        n = 27
        for _ in range(n + 2):
            # Seven units show; the fourth is the highlighted one.
            i = (e.u8(UNIT_WINDOW) + 3) % n
            if e.u16(LIST + 4 * i) & 0x3F == unit_type:
                break
            e.press("LEFT", 6)
            e.wait(12)
        for _ in range(8):
            if e.u8(UNIT_ARMY) == army:
                break
            e.press("UP", 6)
            e.wait(30)
        if e.u8(UNIT_ARMY) != army:
            raise NavError(f"unit bar army {e.u8(UNIT_ARMY)}, wanted {army}")
        self.pick()
        self.stamp(x, y)
        got = self.unit_at(x, y)
        if got != (army, unit_type):
            raise NavError(f"unit at {(x, y)}: {got}, wanted {(army, unit_type)}")

    def unit_at(self, x, y):
        """(army, type) of the unit on a cell, or None (five-army ids: 51 an army)."""
        row = self.e.u16(MAP + 0x417A + 2 * y)
        uid = self.e.u8(MAP + 0x12 + row + x)
        if not uid:
            return None
        t = self.e.u8(self.e.u32(0x08499594) + 12 * uid)
        return uid // 51 + 1, t

    def set_wasteland(self, on=True):
        if self.biome() == int(on):
            return
        self.to_entry(WASTELAND)
        self.pick()
        self.e.press("A", 8)
        self.e.wait(40)
        if self.biome() != int(on):
            raise NavError(f"biome {self.biome()}")

    # -- the File menu ---------------------------------------------------------
    def _file_menu(self):
        e = self.e
        self.close_bar()
        e.press("SELECT", 8)  # the menu: File Help Intel Fill End, on File
        if not e.wait_until(lambda: e.u8(STATE) == 3, 90, step=6):
            raise NavError("the editor's menu did not open")
        e.wait(30)
        e.press("A", 8)       # File: Load Save NameEntry, on Load
        e.wait(60)

    def save(self, slot=1):
        """File > Save > design slot `slot` > Yes."""
        e = self.e
        self._file_menu()
        e.press("DOWN", 6)
        e.wait(30)
        e.press("A", 8)
        e.wait(90)
        for _ in range(slot - 1):
            e.press("DOWN", 6)
            e.wait(20)
        e.press("A", 8)       # Save? Yes / No (on No)
        e.wait(90)
        e.press("LEFT", 6)
        e.wait(20)
        e.press("A", 8)
        if not e.wait_until(lambda: e.u8(STATE) == 1, 900, step=10):
            raise NavError("saving did not return to the map")
        e.wait(30)

    def load(self, slot=1):
        """File > Load > design slot `slot`."""
        e = self.e
        self._file_menu()
        e.press("A", 8)
        e.wait(90)
        for _ in range(slot - 1):
            e.press("DOWN", 6)
            e.wait(20)
        e.press("A", 8)
        if not e.wait_until(lambda: e.u8(STATE) == 1, 900, step=10):
            raise NavError("loading did not return to the map")
        e.wait(60)
