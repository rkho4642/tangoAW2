"""Driving the DS Campaign (crate::ds_campaign): starting it from the menu,
reading its state, getting through dialogue, and forcing a mission's end."""

import struct

from .game import Game, NavError

# tangoAW2's DS Campaign RAM (crate::ds_campaign).
ACTIVE = 0x0203FA00
REQUEST = 0x0203FA01
MISSION = 0x0203FA02
PROGRESS = 0x0203FA40
P_MAGIC = PROGRESS
P_NEXT = PROGRESS + 4
P_WON = PROGRESS + 8
PROGRESS_MAGIC = 0x43445741
MAP_ID_BASE = 0xD8
# The menu's state (crate::campaign_menu).
MENU_LEVEL = 0x0203FA30
MENU_CHOICE = 0x0203FA31

MAP = 0x0201E450         # gMap: size, units layer +0x12, classes +0x1432, rows +0x417A
# Direct-combat ground units able to fire (Infantry, Mech, Md Tank, Tank,
# Recon, Neotank, Megatank) and the land classes a unit may stand on.
DIRECT = (1, 2, 3, 4, 5, 6, 8)
LAND = (1, 3, 4, 5, 6, 8, 0x0A, 0x0B, 0x0C, 0x0E, 0x14)
WHEEL = 0x08616A08       # the Select Mode carousel's proc script
CO_SELECT = 0x086165C0   # the CO select screen's proc script
SELECT_MODE_CURSOR = 0x0300591C
CAMPAIGN = 4
GAME_MODE = 0x03003FC1
MAP_ID = 0x03003FC2
# Event script slots (gUnknown_0200C528: 10 x 0x18, script pointer first).
EVENT_SLOTS = 0x0200C528
TEXT_SKIP = 0x03002514
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

    def scripts_running(self):
        b = self.e.read(EVENT_SLOTS, 0x18 * 10)
        return any(struct.unpack_from("<I", b, 0x18 * i)[0] for i in range(10))

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
        if not e.wait_until(self.active, 900, step=10):
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
            if self.e.u32(p) == WHEEL:
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
            # A menu or CO screen the presses above opened: back out.
            e.press("B", 4)
            e.wait(20)
            n += 46
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

    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)
