"""Driving the DS Campaign (crate::ds_campaign): starting it from the menu,
reading its state, getting through dialogue, and forcing a mission's end."""

import struct

from .game import Game, NavError

# tangoAW2's DS Campaign RAM (crate::ds_campaign).
ACTIVE = 0x0203FD10
REQUEST = 0x0203FD11
MISSION = 0x0203FD12
PROGRESS = 0x0203FD30
P_MAGIC = PROGRESS
P_NEXT = PROGRESS + 4
P_WON = PROGRESS + 8
PROGRESS_MAGIC = 0x43445741
DS_MAP_ID = 0xF0  # every DS mission is played on this map id
# The menu's state (crate::campaign_menu).
LAST_RESULT = 0x0203FD1C      # 1 won / 2 lost, mission, day (u16)
LAST_CONDITION = 0x0203FD50   # the last Dual Strike condition that held, day
MENU_LEVEL = 0x0203FD13
MENU_CHOICE = 0x0203FD14

MAP = 0x0201E450         # gMap: size, units layer +0x12, classes +0x1432, rows +0x417A
# Direct-combat ground units able to fire (Infantry, Mech, Md Tank, Tank,
# Recon, Neotank, Megatank) and the land classes a unit may stand on.
DIRECT = (1, 2, 3, 4, 5, 6, 8)
LAND = (1, 3, 4, 5, 6, 8, 0x0A, 0x0B, 0x0C, 0x0E, 0x14)
WHEELS = (0x08616A08, 0x08616A40)  # the Select Mode carousel's proc scripts (entered, come back)
CO_SELECT = 0x086165C0   # the CO select screen's proc script
SELECT_MODE_CURSOR = 0x0300591C
CAMPAIGN = 4
GAME_MODE = 0x03003FC1
MAP_ID = 0x03003FC2
# Event script slots (gUnknown_0200C528: 10 x 0x18, script pointer first).
EVENT_SLOTS = 0x0200C528
TEXT_SKIP = 0x03002514
TEXT_BOX = 0x08014401         # the dialogue box's proc function
DAY = 0x03004080


class DsCampaign:
    def __init__(self, game: Game):
        self.g = game
        self.e = game.e

    # -- state ------------------------------------------------------------------
    def active(self):
        return self.e.u8(ACTIVE) != 0

    def mission(self):
        return self.e.u8(MISSION)

    def map_id(self):
        return self.e.u8(MAP_ID)

    def progress(self):
        b = self.e.read(PROGRESS, 0x10)
        magic, nxt, won = struct.unpack_from("<IBxxxI", b, 0)
        return {"valid": magic == PROGRESS_MAGIC, "next": nxt, "won": won}

    def text_shown(self):
        """The text of the dialogue box an event script shows now (op 0x19,
        AW2's ShowText), or None."""
        e = self.e
        for i in range(10):
            cur = e.u32(EVENT_SLOTS + 0x18 * i + 4)
            if not e.u32(EVENT_SLOTS + 0x18 * i) or not 0x08000010 <= cur < 0x09000000:
                continue
            # the box being shown (its command done) or about to be
            for c in (cur - 16, cur):
                if e.u32(c) == 0x19:
                    break
            else:
                continue
            tid = e.u16(c + 8)
            p = e.u32(0x08610A38 + 4 * tid)
            raw = e.read(p, 400)
            return raw[:raw.index(b"\0")].decode("latin-1") if b"\0" in raw else None
        return None

    def scripts_running(self):
        """An event script runs (the slots from 0x0200C510: the unit-selected
        list's script uses the one before gUnknown_0200C528), or a dialogue
        box is up."""
        b = self.e.read(EVENT_SLOTS - 0x18, 0x18 * 11)
        if any(struct.unpack_from("<I", b, 0x18 * i)[0] for i in range(11)):
            return True
        return any(f == TEXT_BOX for _, _, f in self.g.procs())

    # -- the menu ---------------------------------------------------------------
    def to_select_mode(self):
        e = self.e
        e.wait(700)
        e.press("START", 8)
        e.wait(300)
        e.press("A", 8)
        e.wait(150)
        for _ in range(8):
            cur = e.u8(SELECT_MODE_CURSOR)
            if cur == CAMPAIGN:
                return
            e.press("UP" if cur < CAMPAIGN else "DOWN", 8)
            e.wait(50)
        raise NavError(f"Select Mode cursor stuck at {e.u8(SELECT_MODE_CURSOR)}")

    def open_campaign_box(self):
        """From the title to Select Mode's Campaign box (the chooser with the pack)."""
        self.to_select_mode()
        self.e.press("A", 8)
        if not self.e.wait_until(lambda: self.box_open(), 120, step=4):
            raise NavError("the Campaign box did not open")
        self.e.wait(20)

    def start(self, new=True, step=None):
        """From the title: Campaign -> DS Campaign -> New (or Continue). With
        `step`, the progress record is set to start at that place of the
        campaign's order (a test aid)."""
        e = self.e
        self.open_campaign_box()
        self.chooser_row(1)  # DS Campaign
        e.press("A", 8)
        e.wait(30)
        if e.u8(MENU_LEVEL) != 2:
            raise NavError(f"not in the DS box (level {e.u8(MENU_LEVEL)})")
        if step is not None:
            e.w32(P_MAGIC, PROGRESS_MAGIC)
            e.w8(P_NEXT, step)
            new = False
        self.box_row(1 if new else 0)
        e.press("A", 8)
        # New over a saved DS Campaign: the game's own notice ("If you save
        # a new game, your previous data will be overwritten.") first.
        for _ in range(40):
            if e.wait_until(self.active, 30, step=5):
                return
            e.press("A", 8)
        raise NavError("the DS Campaign did not start")

    def chooser_row(self, row):
        e = self.e
        for _ in range(4):
            if e.u8(MENU_CHOICE) == row:
                return
            e.press("DOWN", 6)
            e.wait(12)
        raise NavError("chooser row not reached")

    def wheel(self):
        for p in range(0x0200D610, 0x0200E418, 0x6C):
            if self.e.u32(p) in WHEELS:
                return p
        return None

    def box_open(self):
        p = self.wheel()
        return p is not None and self.e.s16(p + 0x64) > 0

    def box_cursor(self):
        p = self.wheel()
        return None if p is None else self.e.u16(p + 0x66) % 2

    def box_row(self, row):
        """Moves the box's own cursor (AW2's Continue / New) to row 0 or 1."""
        e = self.e
        for _ in range(4):
            if self.box_cursor() == row:
                return
            e.press("DOWN" if row else "UP", 6)
            e.wait(12)
        if self.box_cursor() != row:
            raise NavError(f"box row {row} not reached")

    # -- dialogue and battle ----------------------------------------------------
    def through_dialogue(self, max_frames=6000):
        """Presses A while event scripts run (dialogue boxes)."""
        n = 0
        while n < max_frames:
            if not self.scripts_running():
                self.e.wait(20)
                if not self.scripts_running():
                    return True
            self.e.press("A", 4)
            self.e.wait(8)
            n += 14
        return False

    def in_battle(self):
        return self.e.u32(0x03000004) != 0

    def wait_map(self, max_frames=20000):
        """Through the CO select, the mission title and the opening dialogue
        to the player's control. Returns the frames it took."""
        start = self.e.frame
        n = 0
        while n < max_frames and not self.in_battle():
            if self.on_co_select():
                self.e.press("A", 6)
            self.e.wait(20)
            n += 20
        if not self.in_battle():
            raise NavError("the battle did not load")
        self.wait_control(max_frames)
        return self.e.frame - start

    def on_co_select(self):
        return any(self.e.u32(p) == CO_SELECT for p in range(0x0200D610, 0x0200E418, 0x6C))

    def cursor(self):
        return (self.e.u16(0x030033E4), self.e.u16(0x030033E6))

    def wait_control(self, max_frames=20000):
        """Presses A through dialogue until the map cursor answers the pad."""
        e = self.e
        n = 0
        while n < max_frames:
            if self.scripts_running():
                e.press("A", 4)
                e.wait(10)
                n += 16
                continue
            x, y = self.cursor()
            key, back = ("RIGHT", "LEFT") if x == 0 else ("LEFT", "RIGHT")
            e.hold(key, 6)
            e.wait(10)
            if self.cursor() != (x, y):
                e.hold(back, 6)
                e.wait(10)
                return
            # A dialogue box left waiting (A), or a menu or CO screen the
            # presses opened (B backs out).
            e.press("A", 4)
            e.wait(20)
            e.press("B", 4)
            e.wait(20)
            n += 70
        raise NavError(f"no control in {max_frames} frames")

    def end_turn(self):
        """Opens the map menu (START) and chooses End."""
        self.g.open_map_menu()
        self.g.choose("End", self.g.MAP_MENU)

    # -- forcing a mission's end (test aids) ------------------------------------
    def layer_cell(self, x, y):
        row = self.e.u16(MAP + 0x417A + 2 * y)
        return MAP + 0x12 + row + x

    def remove_unit(self, u):
        self.e.w8(self.g.unit_addr(u["id"]), 0)
        if self.e.u8(self.layer_cell(u["x"], u["y"])) == u["id"]:
            self.e.w8(self.layer_cell(u["x"], u["y"]), 0)

    def place_unit(self, u, x, y):
        """Moves a unit (record and map layer) to an empty cell."""
        if self.e.u8(self.layer_cell(u["x"], u["y"])) == u["id"]:
            self.e.w8(self.layer_cell(u["x"], u["y"]), 0)
        a = self.g.unit_addr(u["id"])
        self.e.w8(a + 2, x)
        self.e.w8(a + 3, y)
        self.e.w8(a + 1, 0)
        self.e.w8(self.layer_cell(x, y), u["id"])

    def force_win(self, player_team=(1,)):
        """Leaves the enemy one unit on 1 HP next to a player unit, and has
        that unit destroy it (the game's own rout). Returns True if a
        battle was fought."""
        g, e = self.g, self.e
        units = g.units()
        mine = [u for u in units if u["army"] in player_team and u["type"] in DIRECT]
        enemy = [u for u in units if u["army"] not in player_team]
        if not mine:
            # No unit able to fire (Tag Battle's air force, Lightning
            # Strikes' artillery): one of the player's units becomes a Tank.
            mine = [u for u in units if u["army"] in player_team]
            if mine:
                a = g.unit_addr(mine[0]["id"])
                e.w8(a, 4)
                e.w16(a + 4, (e.u16(a + 4) & 0x7F) | (9 << 7))
                mine[0]["type"] = 4
        if not mine or not enemy:
            return False
        w, h = self.size()
        victim = spot = None
        # Ground targets first: a Tank can't hit planes or submarines.
        for v in sorted(enemy, key=lambda u: u["type"] not in DIRECT):
            for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                x, y = v["x"] + dx, v["y"] + dy
                if 0 <= x < w and 0 <= y < h and e.u8(self.layer_cell(x, y)) == 0 and g.terrain_class(x, y) & 0x1F in LAND:
                    victim, spot = v, (x, y)
                    break
            if spot:
                break
        if spot is None:
            return False
        for u in enemy:
            if u is not victim:
                self.remove_unit(u)
        hp_ammo = e.u16(g.unit_addr(victim["id"]) + 4)
        e.w16(g.unit_addr(victim["id"]) + 4, (hp_ammo & ~0x7F) | 1)
        att = mine[0]
        self.place_unit(att, *spot)
        e.wait(4)
        self.fire(spot, (victim["x"], victim["y"]))
        return True

    def dialogue(self, max_frames=3000):
        """Presses A while an event script runs."""
        n = 0
        while self.scripts_running() and n < max_frames:
            self.e.press("A", 4)
            self.e.wait(10)
            n += 16

    def fire(self, at, target):
        """The unit at `at` fires (without moving) at `target`, getting
        through any event dialogue on the way."""
        g, e = self.g, self.e
        for tries in range(5):
            self.dialogue()
            try:
                g.goto(*at)
                break
            except NavError:
                if tries == 4:
                    raise
                e.wait(30)
        e.press("A", 4)
        e.wait(20)
        self.dialogue()
        e.wait(10)
        e.press("A", 4)  # move in place: the action menu
        e.wait(20)
        self.dialogue()
        g.wait_menu(g.ACTION_MENU, 600)
        g.choose("Fire", g.ACTION_MENU)
        g.pick_target(*target)
        e.wait(60)
        self.dialogue()

    # -- playing a mission out ---------------------------------------------------
    def players(self):
        return self.e.u32(0x08499598)

    def controllers(self):
        """Player +0x1B per army 1..4: 0 none, 1 the player, 2 the computer."""
        p = self.players()
        return [self.e.u8(p + 0x3C * a + 0x1B) for a in range(1, 5)]

    def last_result(self):
        e = self.e
        return {"result": e.u8(LAST_RESULT), "mission": e.u8(LAST_RESULT + 1), "day": e.u16(LAST_RESULT + 2),
                "condition": e.u32(LAST_CONDITION), "condition_day": e.u16(LAST_CONDITION + 4)}

    def state(self):
        """Units per army and the day (for logs)."""
        counts = {}
        for u in self.g.units():
            counts[u["army"]] = counts.get(u["army"], 0) + 1
        return {"day": self.e.u16(DAY), "units": counts, "army": self.e.u8(0x030033EC)}

    def autoplay(self, max_days=40, log=None, max_frames=600000):
        """The computer plays every army (the player's too) until the
        mission ends or `max_days` pass; answers dialogue with A. Returns
        the outcome (`last_result`, result 0 if none) and the day reached."""
        e = self.e
        mission = self.mission()
        e.w8(LAST_RESULT, 0)
        p = self.players()
        start = e.u16(DAY)
        last_day = start
        frames = 0
        # The computer plays the player's armies too, from the first turn
        # (called as the battle loads, before army 1's turn begins).
        if not e.wait_until(lambda: self.in_battle() and self.players() != 0, 20000, step=2):
            raise NavError("the battle did not load")
        p = self.players()
        for a in range(1, 5):
            if e.u8(p + 0x3C * a + 0x1B) == 1:
                e.w8(p + 0x3C * a + 0x1B, 2)
        while frames < max_frames:
            e.wait(30)
            frames += 30
            if e.u8(LAST_RESULT):
                break
            if self.scripts_running() or not self.in_battle() or self.on_co_select():
                e.press("A", 4)
            d = e.u16(DAY)
            if d != last_day and self.in_battle():
                last_day = d
                if log:
                    log(f"day {d}: {self.state()}")
                if d >= start + max_days:
                    break
        out = self.last_result()
        out["days"] = last_day - start + 1
        out["frames"] = frames
        out["was_mission"] = mission
        return out

    def play(self, max_days=40, log=None, max_frames=1500000, **bot):
        """Plays the mission as a player would: the player's armies (+0x1B
        = 1) through the pad by `aw2test.bot.Bot`, the computer's by the
        game. Stops when the mission ends or `max_days` pass."""
        from .bot import Bot
        e = self.e
        b = Bot(self, log=log, **bot)
        mission = self.mission()
        e.w8(LAST_RESULT, 0)
        if not e.wait_until(lambda: self.in_battle() and self.players() != 0, 20000, step=2):
            raise NavError("the battle did not load")
        start = e.u16(DAY)
        last_day = start
        t0 = e.frame
        while e.frame - t0 < max_frames:
            if e.u8(LAST_RESULT):
                break
            army = e.u8(0x030033EC)
            p = self.players()
            human = 1 <= army <= 4 and e.u8(p + 0x3C * army + 0x1B) == 1
            if self.in_battle() and human and not self.scripts_running():
                try:
                    b.play_turn(army)
                except NavError as ex:
                    if log:
                        log(f"turn of army {army}: {ex}")
                    b.cancel()
                continue
            if self.scripts_running() or not self.in_battle() or self.on_co_select():
                e.press("A", 4)
            e.wait(30)
            d = e.u16(DAY)
            if d != last_day and self.in_battle():
                last_day = d
                if log:
                    log(f"day {d}: {self.state()}")
                if d >= start + max_days:
                    break
        out = self.last_result()
        out["days"] = last_day - start + 1
        out["was_mission"] = mission
        return out

    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)


# -- Dual Strike's campaign, read from the .nds (independent of the Rust) -------
DS_OV0 = 0x022AD560
DS_RECORDS = 0x022DBD28 + 0xA0 * 0xE0   # the campaign's map records, 0xA0 bytes
DS_TEXT_GROUPS = DS_OV0 + 0x49690
# The campaign's order (ds_campaign::ORDER): 25 story missions with the three
# research-lab side missions (records 25..27) after the missions that open them.
ORDER = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 25, 10, 11, 26, 12, 13, 27, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]


class DsData:
    """Dual Strike's campaign missions as its ROM has them."""

    def __init__(self, ds=None):
        from . import rom as romlib
        self.ds = ds or romlib.DualStrike()
        self.lz10 = romlib.lz10

    def bytes(self, a, n):
        o = a - DS_OV0
        return self.ds.ov0[o:o + n]

    def u32(self, a):
        return struct.unpack("<I", self.bytes(a, 4))[0]

    def text(self, ref):
        """A text by reference (bank << 24 | index), as bytes."""
        p = DS_TEXT_GROUPS
        while True:
            key, group = struct.unpack("<II", self.bytes(p, 8))
            if key == 0xFFFFFFFF:
                return None
            if key >> 24 == ref >> 24:
                break
            p += 8
        at = self.u32(group + 4 * (ref & 0xFFFFFF))
        raw = self.bytes(at, 0x800)
        return raw[:raw.index(b"\0")]

    def mission(self, i):
        r = self.bytes(DS_RECORDS + 0xA0 * i, 0xA0)
        w = lambda o: struct.unpack_from("<I", r, o)[0]
        h = lambda o: struct.unpack_from("<H", r, o)[0]
        m = self.lz10(self.bytes(w(0x44), 0x2000))
        width, height = m[0], m[1]
        tiles = list(struct.unpack_from(f"<{width * height}H", m, 2))
        units, army, p = [], None, w(0x4C)
        while True:
            u = self.bytes(p, 13)
            p += 13
            if u[0] == 0xFF:
                break
            if u[0] == 0xFE:
                army = u[1]
                continue
            t = {25: 26, 26: 27}.get(u[2], u[2])  # Carrier, Oozium: tangoAW2's ids
            if 1 <= t <= 27:
                units.append((army, u[0], u[1], t))
        sp = w(0x0C)
        structure = None
        if sp:
            raw = self.bytes(sp, 8)
            structure = raw[:raw.index(b"\0")].decode()
        return {
            "name": self.text(0xC0000000 | h(0x14)).decode("latin-1"),
            "w": width, "h": height, "tiles": tiles, "units": units,
            "armies": h(0x24), "cos": [r[0x56 + 2 * k] for k in range(4)], "look": r[0x1A], "weather": r[0x1B], "fog": r[0x1C],
            "structure": structure,
        }
