"""Driving Advance Wars 2 through tangoAW2: menus, Teams, Rules, the battle map.

All navigation is closed-loop: every press is followed by a RAM check, so a
missed press or a slow transition is retried or reported, never silently
mis-navigated.
"""

import struct

from . import ram
from . import rom as romlib
from .emu import Emu


class NavError(RuntimeError):
    pass


RULE_ITEMS = ("fog", "weather", "funds", "turn", "capt", "power", "visuals")
# Rules-screen item values (probed): fog 0 on / 1 off; weather 0 random, 1 clear,
# 2 rain, 3 snow, 4 sandstorm (Dual Strike pack only); power 0 on / 1 off;
# visuals 0 off, 1 A, 2 B, 3 C.
FOG_VALUES = {True: 0, False: 1}
WEATHER_VALUES = {"random": 0, "clear": 1, "rain": 2, "snow": 3, "sandstorm": 4}
POWER_VALUES = {True: 0, False: 1}
VISUALS_VALUES = {"off": 0, "a": 1, "b": 2, "c": 3}

SELECT_MODE_CURSOR = 0x0300591C   # Select Mode menu item (2 Design Room, 3 Versus, 4 Campaign)
SELECT_MODE_VERSUS = 3
MAP_TAB = 0x0300596C              # Select Map tab (8 = Design Maps)
MAP_TAB_DESIGN = 8
BATTLE_MAIN = 0x08022049
# Procs that keep running while the map waits for input (probed: 0x08034F8D
# runs whenever an air unit is on the map).
# 0x08003891 is the Design Room's cursor panel: its record stays in IWRAM
# after the editor is left (AW2's own, with or without tangoAW2), not run.
BACKGROUND_PROCS = (0x08034F8D, 0x08003891)
# Where the battle map's procs live (probed): IWRAM and an EWRAM pool.
PROC_AREAS = ((0x03000C00, 0x1300), (0x0200C000, 0x2000))          # main-loop callback while the battle map runs
# The event-script slots (gUnknown_0200C528: 10 x 0x18), in the EWRAM area.
EVENT_SLOTS = (0x0200C510, 0x0200C528 + 10 * 0x18)


class Game:
    def __init__(self, emu: Emu, image=None):
        self.e = emu
        self.image = image or romlib.Image.load(emu.rom)
        self._units_base = None
        self._players_base = None

    # -- helpers --------------------------------------------------------------
    def proc_area(self):
        lo, hi = ram.PROCS
        b = self.e.read(lo, hi - lo)
        return b, lo

    def proc_fns(self):
        b, lo = self.proc_area()
        return {struct.unpack_from("<I", b, i)[0] for i in range(0, len(b), 4)}

    def on_teams(self):
        return ram.TEAMS_PROC_FN in self.proc_fns()

    def rules_items(self):
        """Rules-screen item procs (address list in screen order), or []."""
        b, lo = self.proc_area()
        items = []
        for off in range(0, len(b) - 0x60, 4):
            fn = struct.unpack_from("<I", b, off + ram.RULES_ITEM_FN)[0]
            if fn in ram.RULES_ITEM_FNS and b[off + ram.RULES_ITEM_COUNT] and off % 0x10 == 0:
                # the proc starts 0x4C before its function word; check its header too
                items.append(lo + off)
        # keep one proc per 0x60 block
        out = []
        for a in items:
            if not out or a - out[-1] >= 0x60:
                out.append(a)
        return out

    def on_rules(self):
        return len(self.rules_items()) >= 6

    def press_until(self, key, pred, tries=20, hold=6, settle=12):
        for _ in range(tries):
            if pred():
                return True
            self.e.press(key, hold)
            self.e.wait(settle)
        return pred()

    # -- boot to the Teams screen ---------------------------------------------
    def boot_to_teams(self):
        e = self.e
        e.wait(700)
        # Title -> START -> A opens Select Mode (its cursor byte becomes 1..8).
        e.press("START", 8)
        e.wait(300)
        e.press("A", 8)
        e.wait(150)
        cur = e.u8(SELECT_MODE_CURSOR)
        for _ in range(8):
            cur = e.u8(SELECT_MODE_CURSOR)
            if cur == SELECT_MODE_VERSUS:
                break
            e.press("UP" if cur < SELECT_MODE_VERSUS else "DOWN", 8)
            e.wait(50)
        if e.u8(SELECT_MODE_CURSOR) != SELECT_MODE_VERSUS:
            raise NavError(f"Select Mode cursor stuck at {cur}")
        e.press("A", 8)  # Versus -> Continue/New (on New)
        e.wait(150)
        e.press("A", 8)  # New -> Select Map
        e.wait(150)
        if not self.press_until("LEFT", lambda: e.u8(MAP_TAB) == MAP_TAB_DESIGN, tries=12, hold=8, settle=50):
            raise NavError(f"Design Maps tab not reached (tab {e.u8(MAP_TAB)})")
        e.wait(30)
        e.press("A", 8)  # Design Map 1 (the first entry)
        if not e.wait_until(self.on_teams, 400, step=10):
            raise NavError("Teams screen did not open")
        e.wait(40)

    # -- Teams screen ------------------------------------------------------------
    def teams_addr(self):
        """The Teams record: tangoAW2 moves it in a five-army game (crate::five)."""
        return ram.TEAMS_FIVE if self.e.u8(ram.FIVE_ON) == 1 else ram.TEAMS

    def teams(self):
        e = self.e
        rec = e.read(self.teams_addr(), 0x40)
        count = rec[ram.T_CO_COUNT]
        lst = struct.unpack_from("<I", rec, ram.T_CO_LIST)[0]
        co_list = list(e.read(lst, count))
        return {
            "armies": rec[ram.T_ARMY_COUNT],
            "controllers": list(rec[ram.T_CONTROLLERS:ram.T_CONTROLLERS + 4]),
            "co_index": list(rec[ram.T_CO_INDEX:ram.T_CO_INDEX + 4]),
            "co_list": co_list,
        }

    def set_teams(self, cos, humans):
        """cos: CO id (or name) per army in order; humans: set of 1-based armies played by 1P.

        COs are chosen with the pad (UP/DOWN on the army's CO stop; RIGHT moves
        two stops per army), checked against the Teams record after each press.
        Controllers are written into the Teams record (+0x09), which the battle's
        player blocks are built from.
        """
        e = self.e
        t = self.teams()
        n = t["armies"]
        base = self.teams_addr()
        if cos is None:
            # Keep the Teams screen's COs; only the controllers are set.
            for army in range(min(n, 4)):
                want = 1 if (army + 1) in humans else 2
                if e.u8(base + ram.T_CONTROLLERS + army) != want:
                    e.w8(base + ram.T_CONTROLLERS + army, want)
            if n >= 5 and self.e.u8(ram.FIVE_ON) == 1:
                # The five-army record keeps all five armies' controllers
                # at +0x90 (army 5, Black Hole, has its own stop there).
                for army in range(5):
                    e.w8(base + ram.T5_CONTROLLERS + army, 1 if (army + 1) in humans else 2)
            e.wait(4)
            return
        if len(cos) != n:
            raise NavError(f"map has {n} armies, {len(cos)} COs given")
        for army, co in enumerate(cos):
            co = romlib.co_id(co)
            want = t["co_list"].index(co)
            addr = base + ram.T_CO_INDEX + army
            for _ in range(len(t["co_list"]) + 2):
                cur = e.u8(addr)
                if cur == want:
                    break
                down = (want - cur) % len(t["co_list"])
                e.press("DOWN" if down <= len(t["co_list"]) // 2 else "UP", 6)
                e.wait(14)
            if e.u8(addr) != want:
                raise NavError(f"army {army + 1}: CO index {e.u8(addr)}, wanted {want}")
            if army + 1 < n:
                e.press("RIGHT", 6)
                e.wait(14)
                e.press("RIGHT", 6)
                e.wait(14)
        for army in range(n):
            want = 1 if (army + 1) in humans else 2
            if e.u8(base + ram.T_CONTROLLERS + army) != want:
                e.w8(base + ram.T_CONTROLLERS + army, want)
        e.wait(4)

    # -- Rules screen ---------------------------------------------------------------
    def teams_to_rules(self):
        e = self.e
        for _ in range(4):
            e.press("A", 8)
            if e.wait_until(self.on_rules, 120, step=10):
                e.wait(30)
                return
        raise NavError("Rules screen did not open")

    def weather_values(self):
        return WEATHER_VALUES

    def set_rules(self, fog=False, weather="clear", power=True, visuals="off", capt=None):
        e = self.e
        items = self.rules_items()
        if len(items) < 7:
            raise NavError(f"rules items found: {[hex(a) for a in items]}")
        wants = {0: FOG_VALUES[fog], 1: weather if isinstance(weather, int) else self.weather_values()[weather],
                 5: POWER_VALUES[power],
                 6: VISUALS_VALUES[visuals]}
        for idx in sorted(wants):
            # cursor to item idx
            for _ in range(10):
                cur = e.u8(self.teams_addr() + ram.RULES_CURSOR - ram.TEAMS)
                if cur == idx:
                    break
                e.press("RIGHT" if cur < idx else "LEFT", 6)
                e.wait(20)
            if e.u8(self.teams_addr() + ram.RULES_CURSOR - ram.TEAMS) != idx:
                raise NavError(f"rules cursor at {e.u8(self.teams_addr() + ram.RULES_CURSOR - ram.TEAMS)}, wanted {idx}")
            proc = self.rules_items()[idx]
            count = e.u8(proc + ram.RULES_ITEM_COUNT)
            for _ in range(count + 2):
                v = e.u8(proc + ram.RULES_ITEM_VALUE)
                if v == wants[idx]:
                    break
                e.press("DOWN", 6)
                e.wait(20)
            if e.u8(proc + ram.RULES_ITEM_VALUE) != wants[idx]:
                raise NavError(f"rules item {RULE_ITEMS[idx]}: value {e.u8(proc + ram.RULES_ITEM_VALUE)}, wanted {wants[idx]}")
        if capt is not None:
            # The capture limit's choice (0 off), set directly: stepping
            # through up to 100 values with the pad takes too long.
            e.w8(self.rules_items()[4] + ram.RULES_ITEM_VALUE, capt)
            e.wait(4)

    # The Rules screen's Skills row (tangoAW2's crate::versus_rules, with the
    # Dual Strike pack): the row the cursor is on (0 none, 1 Skills; the
    # game's own cursor stays on Visuals) and the rule's byte.
    VRULE_CURSOR = 0x0203F4C8
    SKILLS_RULE = 0x0203E385

    def set_extra_rules(self, skills=None):
        """On the Rules screen, with the pad: Skills ON (True) or OFF (False);
        None leaves it. Back on Visuals after."""
        e = self.e
        cursor = self.teams_addr() + ram.RULES_CURSOR - ram.TEAMS
        for _ in range(12):
            if e.u8(self.VRULE_CURSOR) == 0 and e.u8(cursor) == 6:
                break
            e.press("LEFT" if e.u8(self.VRULE_CURSOR) else "RIGHT", 6)
            e.wait(20)
        for want, addr, row in ((skills, self.SKILLS_RULE, 1),):
            for _ in range(4):
                if e.u8(self.VRULE_CURSOR) >= row:
                    break
                e.press("RIGHT", 6)
                e.wait(20)
            if e.u8(self.VRULE_CURSOR) != row:
                raise NavError(f"the Rules screen's row {row} not reached")
            if want is not None and (e.u8(addr) == 1) != want:
                e.press("UP" if want else "DOWN", 6)
                e.wait(20)
            if want is not None and (e.u8(addr) == 1) != want:
                raise NavError(f"the Rules screen's row {row} not set")
        for _ in range(4):
            if e.u8(self.VRULE_CURSOR) == 0:
                break
            e.press("LEFT", 6)
            e.wait(20)

    def start_battle(self):
        e = self.e
        e.press("A", 8)
        if not e.wait_until(lambda: e.u32(ram.MAIN_CALLBACK) == BATTLE_MAIN, 600, step=10):
            raise NavError("battle map did not start")
        self._units_base = e.u32(ram.UNITS_PTR)
        self._players_base = e.u32(ram.PLAYERS_PTR)
        # the Day 1 banner and turn start run as procs; wait for them to come and go
        if not e.wait_until(lambda: bool(self.procs()), 900, step=5):
            raise NavError("turn start never ran")

    def setup(self, cos, humans=(1,), fog=False, weather="clear", power=True, visuals="off", capt=None):
        """Boot, open Versus -> Design Maps -> map 1, pick COs, rules, start."""
        self.boot_to_teams()
        self.set_teams(cos, set(humans))
        self.teams_to_rules()
        self.set_rules(fog=fog, weather=weather, power=power, visuals=visuals, capt=capt)
        self.start_battle()
        self.wait_for_input()

    # -- battle state -------------------------------------------------------------------
    @property
    def units_base(self):
        if self._units_base is None:
            self._units_base = self.e.u32(ram.UNITS_PTR)
        return self._units_base

    @property
    def players_base(self):
        if self._players_base is None:
            self._players_base = self.e.u32(ram.PLAYERS_PTR)
        return self._players_base

    def unit_addr(self, uid):
        return self.units_base + ram.UNIT_SIZE * uid

    def units(self, army=None):
        """Every live unit: dict(id, army, type, x, y, hp, ammo, fuel, flags)."""
        e = self.e
        raw = e.read(self.units_base, ram.UNIT_SIZE * 256)
        # Five armies (tangoAW2's five.rs): 51 slots an army, else 64.
        per = 51 if e.u8(ram.FIVE_ON) == 1 else 64
        out = []
        for uid in range(256):
            r = raw[ram.UNIT_SIZE * uid: ram.UNIT_SIZE * (uid + 1)]
            if r[0] == 0 or uid % per == 0:
                continue
            a = uid // per + 1
            if army is not None and a != army:
                continue
            out.append(parse_unit(uid, r, per))
        return out

    def unit(self, uid):
        return parse_unit(uid, self.e.read(self.unit_addr(uid), ram.UNIT_SIZE))

    def unit_at(self, x, y):
        for u in self.units():
            if (u["x"], u["y"]) == (x, y) and not (u["flags"] & 0x08):
                return u
        return None

    def player(self, army):
        p = self.players_base + ram.PLAYER_SIZE * army
        b = self.e.read(p, ram.PLAYER_SIZE)
        return {
            "addr": p,
            "colour": b[ram.P_COLOUR],
            "co": b[ram.P_CO],
            "co_mode": b[ram.P_CO_MODE],
            "co_activation": b[ram.P_CO_ACTIVATION],
            "charge": struct.unpack_from("<I", b, ram.P_CHARGE)[0],
            "powers_used": b[ram.P_POWERS_USED],
            "temp_firepower": struct.unpack_from("<h", b, ram.P_TEMP_FIREPOWER)[0],
            "temp_defence": struct.unpack_from("<h", b, ram.P_TEMP_DEFENCE)[0],
            "raw": b,
        }

    def terrain_class(self, x, y):
        row = self.e.u16(ram.MAP_ROW_OFFSETS + 2 * y)
        return self.e.u8(ram.MAP_TERRAIN + row + x)

    def current_army(self):
        return self.e.u8(ram.CURRENT_ARMY)

    def cursor(self):
        return self.e.u8(ram.CURSOR_X), self.e.u8(ram.CURSOR_Y)

    def playst(self):
        e = self.e
        return {
            "fog": e.u8(ram.FOG),
            "weather": e.u8(ram.WEATHER),
            "anim": e.u8(ram.ANIM_OPTS),
            "co_powers": e.u8(ram.CO_POWERS_ENABLED),
            "co_abilities": e.u8(ram.CO_ABILITIES),
            "map": e.u8(ram.VS_MAP),
        }

    # -- battle input -------------------------------------------------------------------
    def procs(self):
        """Running procs on the battle map: (address, script, function) triples.

        The battle map runs its UI as procs in IWRAM (0x03000C00..0x03001F00) and
        EWRAM (0x0200C000..0x0200E000):
        two script pointers (0x084xxxxx) then a Thumb function. None are running
        while the map waits for the player (probed: selection 0x080228D9, menus
        0x08019D0D/0x08019DED, turn start 0x08027DD9/0x0802788D/..)."""
        out = []
        for base, n in PROC_AREAS:
            b = self.e.read(base, n)
            for i in range(0, len(b) - 12, 4):
                # The event-script slots (script, cursor, callback) are not
                # procs (a cursor script's would look like one).
                if EVENT_SLOTS[0] <= base + i < EVENT_SLOTS[1]:
                    continue
                s0, s1, fn = struct.unpack_from("<III", b, i)
                if (0x08400000 <= s0 < 0x08700000 and 0x08400000 <= s1 < 0x08700000
                        and 0x08000000 < fn < 0x08100000 and fn & 1):
                    out.append((base + i, s0, fn))
        return out

    def idle(self):
        if self.e.u32(ram.MAIN_CALLBACK) != BATTLE_MAIN:
            return False
        return not [p for p in self.procs() if p[2] not in BACKGROUND_PROCS]

    def battle_over(self):
        """True once the map has handed over to the result screens."""
        return self.e.u32(ram.MAIN_CALLBACK) not in (BATTLE_MAIN, 0)

    def wait_idle(self, max_frames=1200, stable=8, step=4):
        """Wait until no map proc runs for `stable` checks in a row."""
        n = 0
        run = 0
        while n <= max_frames:
            if self.idle():
                run += 1
                if run >= stable:
                    return True
            else:
                run = 0
            self.e.wait(step)
            n += step
        raise NavError(f"map not idle after {max_frames} frames: {[(hex(a), hex(f)) for a, _, f in self.procs()]}")

    def wait_for_input(self, max_frames=4000):
        """Wait until the free map cursor answers the pad.

        No proc running is not enough (power cutscenes, weather changes and the
        day banner run without one), so the cursor is nudged one cell and back:
        it only moves when the player has control."""
        n = 0
        while n <= max_frames:
            self.wait_idle(max_frames, stable=4)
            x, y = self.cursor()
            # (towards the map's middle: maps are up to 30 wide)
            width = self.e.u16(0x0201E450) or 30
            key, back = ("RIGHT", "LEFT") if x < min(29, width - 1) else ("LEFT", "RIGHT")
            self.e.hold(key, 6)
            self.e.wait(10)
            if self.cursor() != (x, y):
                self.e.hold(back, 6)
                self.e.wait(10)
                if self.cursor() != (x, y):
                    # The game moved the cursor itself (a turn start still
                    # panning): not in the player's hands yet.
                    n += 30
                    continue
                return
            self.e.wait(20)
            n += 36
        raise NavError(f"the map cursor never answered in {max_frames} frames")

    def goto(self, x, y):
        """Walk the map cursor to (x, y) with the arrows, reading it from RAM each step."""
        for _ in range(80):
            cx, cy = self.cursor()
            if (cx, cy) == (x, y):
                break
            key = "RIGHT" if cx < x else "LEFT" if cx > x else "DOWN" if cy < y else "UP"
            self.e.hold(key, 6)
            self.e.wait(10)
        if self.cursor() != (x, y):
            raise NavError(f"cursor at {self.cursor()}, wanted {(x, y)}")

    def has_proc(self, fn):
        return any(f == fn for _, _, f in self.procs())

    # menus (a builder proc: +0x08 = 0x08019D0D, +0x20 table, +0x31 visible
    # entry indexes (+0x41 of them), +0x44 the menu proc whose +0x20 is the cursor)
    MENU_BUILDER_FN = 0x08019D0D
    @property
    def ACTION_MENU(self):
        """The unit command menu's table, as the game reads it (tangoAW2's copy
        with the Dual Strike pack: pool word 0x0802D59C)."""
        return self.e.u32(0x0802D59C)

    @property
    def MAP_MENU(self):
        """The map menu's table, as the game reads it now (tangoAW2's copy
        with Front in a two-front battle, or with Tag and Change in a battle
        with CO tag pairs: pool word 0x0802D49C)."""
        return self.e.u32(0x0802D49C)

    def live_label(self, table, i):
        """A menu entry's label as the game shows it now (text table and
        strings read from the running ROM image, which tangoAW2 edits), without
        its icon code."""
        tid = self.e.u32(table + 0x20 * i + 0x1C)
        p = self.e.u32(0x08610A38 + 4 * tid)
        raw = self.e.read(p, 40)
        t = raw[:raw.index(b"\0")] if b"\0" in raw else raw
        if t[:1] in (b"\t", b"\n"):
            t = t[2:]
        return t.decode("latin-1")

    def menu(self):
        for addr, _, fn in self.procs():
            if fn == self.MENU_BUILDER_FN:
                b = self.e.read(addr, 0x48)
                table = struct.unpack_from("<I", b, 0x20)[0]
                vis = list(b[0x31:0x31 + b[0x41]])  # +0x40 entries in the table, +0x41 shown
                menu_proc = struct.unpack_from("<I", b, 0x44)[0]
                if not (0x08000000 <= table < 0x09000000):
                    return None
                try:
                    names = [self.live_label(table, i) for i in vis]
                    if table == self.ACTION_MENU:
                        # entry 1 is the second Fire: an indirect unit that has not
                        # moved and has no target gets it greyed out (sub_0802CB20)
                        names = [n + " (greyed)" if i == 1 else n for n, i in zip(names, vis)]
                except Exception:
                    return None
                return {"table": table, "visible": vis, "names": names, "cursor_addr": menu_proc + 0x20,
                        "flags": list(b[0x24:0x24 + b[0x40]]),  # per entry: 0 shown, 1 hidden, 2 greyed
                        "cursor": self.e.u8(menu_proc + 0x20)}
        return None

    def wait_menu(self, table=None, max_frames=240):
        n = 0
        while n <= max_frames:
            m = self.menu()
            if m and (table is None or m["table"] == table) and m["names"]:
                self.e.wait(6)
                return self.menu()
            self.e.wait(4)
            n += 4
        raise NavError(f"menu {hex(table or 0)} did not open; procs {[(hex(a), hex(f)) for a, _, f in self.procs()]}")

    def choose(self, name, table=None):
        """Pick a menu entry by label (case-insensitive prefix match on the text)."""
        m = self.wait_menu(table)
        want = [i for i, n in enumerate(m["names"]) if n.lower().startswith(name.lower())]
        if not want:
            raise NavError(f"no {name!r} in menu {m['names']}")
        idx = want[0]
        for _ in range(len(m["names"]) + 2):
            cur = self.e.u8(m["cursor_addr"])
            if cur == idx:
                break
            self.e.press("DOWN" if cur < idx else "UP", 4)
            self.e.wait(6)
        if self.e.u8(m["cursor_addr"]) != idx:
            raise NavError(f"menu cursor {self.e.u8(m['cursor_addr'])}, wanted {idx} ({name})")
        self.e.press("A", 4)
        return m["names"]

    def select(self, x, y):
        """Pick up the unit at (x, y) (its movement range opens)."""
        u = self.unit_at(x, y)
        if u is None:
            raise NavError(f"no unit at {(x, y)}")
        self.wait_idle()
        self.goto(x, y)
        self.e.press("A", 4)
        want = self.unit_addr(u["id"])
        if not self.e.wait_until(lambda: self.e.u32(ram.SELECTED_UNIT) == want and self.has_proc(0x080228D9), 120, step=4):
            raise NavError(f"unit at {(x, y)} not selected (selected {hex(self.e.u32(ram.SELECTED_UNIT))})")
        return u

    def move_to(self, x, y):
        """With a unit selected: move it to (x, y); the action menu opens."""
        self.goto(x, y)
        self.e.press("A", 4)
        return self.wait_menu(self.ACTION_MENU, 600)

    def pick_target(self, x, y):
        """In target selection: cycle the targets until the cursor is on (x, y), then A."""
        self.e.wait(10)
        for key in ["RIGHT"] * 8 + ["DOWN"] * 8:
            if self.cursor() == (x, y):
                break
            self.e.press(key, 4)
            self.e.wait(8)
        if self.cursor() != (x, y):
            raise NavError(f"target cursor at {self.cursor()}, wanted {(x, y)}")
        self.e.press("A", 4)

    def attack(self, src, dst, target):
        """Move the unit at src to dst and fire at target; returns (attacker, defender)
        unit records before and after, read from RAM."""
        att = self.select(*src)
        dfd = self.unit_at(*target)
        if dfd is None:
            raise NavError(f"no unit to attack at {target}")
        self.move_to(*dst)
        self.choose("Fire", self.ACTION_MENU)
        self.pick_target(*target)
        self.wait_for_input()
        return (att, self.unit(att["id"])), (dfd, self.unit(dfd["id"]))

    def wait_unit(self, x, y):
        """Move the unit at (x, y) nowhere and Wait."""
        self.select(x, y)
        self.move_to(x, y)
        self.choose("Wait", self.ACTION_MENU)
        self.wait_idle()

    def action_menu_at(self, x, y):
        """The action menu of the unit at (x, y) when it stays put; then cancel
        (B twice) so the unit can still act. Returns the entry labels."""
        self.select(x, y)
        m = self.move_to(x, y)
        self.e.press("B", 4)
        self.e.wait(20)
        self.e.press("B", 4)
        self.wait_for_input()
        if self.e.u32(ram.SELECTED_UNIT) and self.has_proc(0x080228D9):
            raise NavError("unit still selected after cancelling")
        return m["names"]

    def buy(self, x, y, unit_type):
        """Build `unit_type` at the property at (x, y) through the build menu."""
        self.wait_idle()
        self.goto(x, y)
        self.e.press("A", 4)
        self.e.wait(60)
        ids = []
        for k in range(30):
            t = self.e.u8(0x02023830 + 4 * k)
            if t == 0:
                break
            ids.append(t)
        if unit_type not in ids:
            raise NavError(f"{unit_type} not in the build menu {ids}")
        for _ in range(ids.index(unit_type)):
            self.e.press("DOWN", 4)
            self.e.wait(8)
        self.e.press("A", 4)
        self.wait_for_input()
        return ids

    def empty_cells(self):
        """The map's cells without a unit, nearest the cursor first (inside
        the map: gMap's size)."""
        occupied = {(u["x"], u["y"]) for u in self.units()}
        w, h = self.e.u16(0x0201E450), self.e.u16(0x0201E452)
        if not (0 < w <= 30 and 0 < h <= 30):
            w, h = 30, 20
        cx, cy = self.cursor()
        cells = [(abs(x - cx) + abs(y - cy), x, y) for y in range(h) for x in range(w) if (x, y) not in occupied]
        return [(x, y) for _, x, y in sorted(cells)]

    def empty_cell(self):
        return self.empty_cells()[0]

    def open_map_menu(self):
        """A on a cell without a unit opens the map menu, unless the cell is
        a factory of the army (its build menu) or a structure: then B and
        the next cell."""
        self.wait_idle()
        for x, y in self.empty_cells()[:8]:
            self.goto(x, y)
            self.e.press("A", 4)
            try:
                return self.wait_menu(self.MAP_MENU, max_frames=90)
            except NavError:
                self.e.press("B", 4)
                self.e.wait(20)
                self.wait_idle()
        return self.wait_menu(self.MAP_MENU)

    def map_menu_names(self):
        m = self.open_map_menu()
        self.e.press("B", 4)
        self.wait_for_input()
        return m["names"]

    # The CO table as the game reads it now (tangoAW2's copy with the Dual
    # Strike pack): a literal-pool word of GetCoPriceMultiplier's neighbour.
    CO_TABLE_POOL = 0x08042DDC

    def co_stars(self, co):
        a = self.e.u32(self.CO_TABLE_POOL) + romlib.CO_RECORD * co
        return self.e.u32(a + 0x0C), self.e.u32(a + 0x10)

    def co_cost_percent(self, co, mode):
        a = self.e.u32(self.CO_TABLE_POOL) + romlib.CO_RECORD * co + 0x38 + 0x44 * mode + 0x14
        v = self.e.read(a, 2)
        return int.from_bytes(v, "little", signed=True)

    def power(self, which="power"):
        """Activate the CO Power ('power') or Super CO Power ('super') from the map menu."""
        self.open_map_menu()
        army = self.current_army()
        mode = 2 if which.lower().startswith("s") else 1
        self.choose("Super" if mode == 2 else "Power", self.MAP_MENU)
        if not self.e.wait_until(lambda: self.player(army)["co_mode"] == mode, 3000, step=10):
            raise NavError(f"army {army}'s power mode did not become {mode}")
        self.wait_for_input()

    def end_turn(self, human=1, max_frames=20000, observe=None):
        """End the turn and wait until army `human` can move again (CPU turns run).

        observe(game), if given, is called once the next army's turn has begun."""
        army = self.current_army()
        self.open_map_menu()
        self.choose("End", self.MAP_MENU)
        # wait for the turn to leave this army, then come back to `human`
        if not self.e.wait_until(lambda: self.current_army() != army, 600, step=8):
            raise NavError("turn did not end")
        if observe:
            observe(self)
        if not self.e.wait_until(lambda: self.current_army() == human, max_frames, step=30):
            raise NavError(f"army {human}'s turn did not come back")
        self.wait_for_input()

    def charge_power(self, army, which="super"):
        """Fill army's power meter exactly to the COP or SCOP cost (charge +0x20)."""
        p = self.player(army)
        cop, scop = self.co_stars(p["co"])
        cost = power_star_cost(p["powers_used"]) * (scop if which.lower().startswith("s") else cop)
        self.e.w32(p["addr"] + ram.P_CHARGE, cost)
        return cost


def power_star_cost(uses):
    """GetCoPowerStarCost: 9000 per star, +20% per power already used (at most 10 uses)."""
    pct = 200 if uses > 9 else 100 + 20 * uses
    return 9000 * pct // 100


def parse_unit(uid, r, per=64):
    hp_ammo = struct.unpack_from("<H", r, 4)[0]
    return {
        "id": uid,
        "army": uid // per + 1,
        "type": r[0],
        "flags": r[1],
        "x": r[2],
        "y": r[3],
        "hp": hp_ammo & 0x7F,
        "ammo": (hp_ammo >> 7) & 0xF,
        "fuel": r[6] & 0x7F,
        "cargo": (r[7], r[8]),
        "raw": bytes(r),
    }
