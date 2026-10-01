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

    def start(self, new=True, mission=None):
        """From the title: Campaign -> DS Campaign -> New (or Continue). With
        `mission`, the progress record is set to start there (a test aid)."""
        e = self.e
        self.to_select_mode()
        e.press("A", 8)  # open the Campaign box
        e.wait(60)
        self.choose_box(1)  # DS Campaign
        e.press("A", 8)
        e.wait(40)
        if mission is not None:
            e.w32(P_MAGIC, PROGRESS_MAGIC)
            e.w8(P_NEXT, mission)
            new = False
        self.choose_box(1 if new else 0)
        e.press("A", 8)
        if not e.wait_until(self.active, 600, step=10):
            raise NavError("the DS Campaign did not start")

    def choose_box(self, row):
        """Moves the Select Mode sub-box cursor to row 0 (top) or 1."""
        e = self.e
        for _ in range(4):
            if self.box_row() == row:
                return
            e.press("DOWN" if row > self.box_row() else "UP", 6)
            e.wait(20)

    def box_row(self):
        # The carousel proc's sub-box cursor (unk66: 6, 7, ...; parity = row).
        for p in range(0x0200D610, 0x0200E418, 0x6C):
            if self.e.u32(p) == 0x08616990:
                pass
        return self.e.u8(MENU_CHOICE)

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

    def wait_map(self, max_frames=6000):
        """Through the mission title and opening dialogue to the player's control."""
        n = 0
        while n < max_frames:
            if self.e.u32(0x03000004) != 0:
                break
            self.e.wait(20)
            n += 20
        self.through_dialogue(max_frames)
        self.g.wait_for_input(max_frames)

    def rout(self, enemy_armies):
        """Removes every unit of these armies (a forced win at the next check)."""
        for u in self.g.units():
            if u["army"] in enemy_armies:
                self.e.w8(self.g.unit_addr(u["id"]), 0)

    def kill_army(self, army):
        self.rout([army])
