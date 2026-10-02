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
P_FLAGS = PROGRESS + 0x10
PROGRESS_MAGIC = 0x43445741
WM_STATE = 0x0202FDFC          # the world map's state (camera, cursor, mission +0x0C, flags +0x12)
WM_CURSOR_LOOP = 0x0807703D    # WorldMapCursor_Loop
WM_INFO_LOOP = 0x08077791      # WorldMapMissionInfo_InputLoop
LAB_FLAGS = {25: 0x90, 26: 0x91, 27: 0x92}
DS_MAP_ID = 0xF0  # every DS mission is played on this map id
# The menu's state (crate::campaign_menu).
LAST_RESULT = 0x0203FD1C      # 1 won / 2 lost, mission, day (u16)
LAST_CONDITION = 0x0203FD50   # the last Dual Strike condition that held, day
WIN_CAUSE = 0x0203FD58        # the condition an event script ended the battle on, day
# Dual Strike's mission conditions (ds_campaign_rules::predicate), for reports.
CONDITIONS = {
    0x0235066C: "every unit has moved", 0x02350824: "every unit has moved", 0x02350940: "every unit has moved",
    0x02350A1C: "every unit has moved", 0x02350B28: "every unit has moved",
    0x023507A8: "army 1 has no Infantry left", 0x02350708: "every unit out of fuel", 0x02350638: "a unit right of column 7",
    0x02350BE4: "army 2 has no Piperunner left", 0x02350D60: "army 1 has no Lander left", 0x02350FF0: "army 3 has no Megatank left",
    0x02350C60: "an airport taken", 0x02350F28: "a Com Tower taken",
    0x02350DDC: "the city at (9, 1) taken", 0x02350E6C: "the city at (7, 6) taken", 0x02350EC4: "the lab at (17, 1) taken",
    0x0235106C: "the lab at (23, 14) taken", 0x02351640: "the lab at (14, 1) taken",
    0x02351744: "the cities at (8, 2) and (8, 15) taken", 0x02351804: "the four Com Towers taken",
    0x02351B88: "no missile silo base left to Black Hole",
    0x02350CD4: "a minicannon destroyed", 0x023510FC: "a minicannon damaged", 0x02351C58: "a Black Crystal destroyed",
    0x023505C0: "the Black Obelisk destroyed", 0x02351708: "the Black Obelisk destroyed", 0x023505E8: "every Black Crystal destroyed",
    0x02350610: "every minicannon destroyed", 0x02350560: "the Grand Bolt's three weak points destroyed",
    0x02351CC8: "the Grand Bolt's charge (every sixth day)",
}
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
        AW2's ShowText, or 0x1A, a narration box without a speaker), or
        None."""
        e = self.e
        for i in range(10):
            cur = e.u32(EVENT_SLOTS + 0x18 * i + 4)
            if not e.u32(EVENT_SLOTS + 0x18 * i) or not 0x08000010 <= cur < 0x09000000:
                continue
            # the box being shown (its command done) or about to be
            for c in (cur - 16, cur):
                if e.u32(c) in (0x19, 0x1A):
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

    def start(self, new=True, step=None, pick=True):
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
            # Every mission before `step` won (a lab mission's own flag set
            # when `step` is one), the cursor's place at `step`.
            e.w32(P_MAGIC, PROGRESS_MAGIC)
            e.w8(P_NEXT, step)
            won = 0
            for m in ORDER[:step]:
                won |= 1 << m
            e.w32(P_WON, won)
            if ORDER[step] in LAB_FLAGS:
                f = LAB_FLAGS[ORDER[step]] - 0x20
                e.w8(P_FLAGS + f // 8, e.u8(P_FLAGS + f // 8) | (1 << (f % 8)))
            new = False
        self.box_row(1 if new else 0)
        e.press("A", 8)
        # New over a saved DS Campaign: the game's own notice ("If you save
        # a new game, your previous data will be overwritten.") first.
        for _ in range(40):
            if e.wait_until(self.active, 30, step=5):
                break
            e.press("A", 8)
        else:
            raise NavError("the DS Campaign did not start")
        if pick:
            self.pick_mission()

    # -- the world map --------------------------------------------------------------
    def proc_fn_running(self, fn):
        """A proc of the pool (0x0200D610, 0x6C each) repeats `fn` (+0x10)."""
        b = self.e.read(0x0200D610, 0x6C * 30)
        return any(struct.unpack_from("<I", b, 0x6C * k + 0x10)[0] == fn and struct.unpack_from("<I", b, 0x6C * k)[0]
                   for k in range(30))

    def world_map_up(self):
        """The world map's cursor answers the pad (WorldMapCursor_Loop)."""
        return self.proc_fn_running(WM_CURSOR_LOOP)

    def map_mission(self):
        """The mission the world map's cursor is on (its state's +0x0C)."""
        return self.e.u32(WM_STATE + 0x0C)

    def map_flags(self):
        """Per mission: 1 shown (selectable), 2 cleared."""
        return list(self.e.read(WM_STATE + 0x12, 28))

    def wait_world_map(self, max_frames=6000):
        e = self.e
        n = 0
        while n < max_frames:
            if self.world_map_up() and e.u8(WM_STATE + 0x10):
                return True
            if self.scripts_running() or not self.world_map_up() and not self.in_battle():
                e.press("A", 4)
            e.wait(10)
            n += 14
        raise NavError("the world map did not come up")

    def cleared_flags(self):
        """The starred flags drawn on won missions' points (OBJ tile 40,
        16x16): their (x, y) on screen."""
        oam = self.e.read(0x07000000, 0x400)
        out = []
        for k in range(128):
            a0, a1, a2 = struct.unpack_from("<3H", oam, 8 * k)
            if a0 & 0x300 != 0x200 and a2 & 0x3FF == 40 and (a0 & 0xFF) < 160:
                out.append((a1 & 0x1FF, a0 & 0xFF))
        return out

    def pick_mission(self):
        """On the world map, A on the mission under the cursor, A on its
        panel: the CO screen or the mission comes next."""
        e = self.e
        self.wait_world_map()
        e.press("A", 6)
        if not e.wait_until(lambda: self.proc_fn_running(WM_INFO_LOOP), 300, step=5):
            raise NavError("the mission panel did not open")
        gone = lambda: not self.proc_fn_running(WM_INFO_LOOP) and not self.proc_fn_running(WM_CURSOR_LOOP)
        # (A while the panel's text is still being written finishes it.)
        for _ in range(12):
            e.wait(30)
            e.press("A", 6)
            if e.wait_until(lambda: self.on_co_select() or gone() or self.in_battle(), 90, step=10):
                return
        raise NavError("the mission was not picked")

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

    def co_cursor(self):
        """On the CO screen: the CO under the cursor and the screen's
        lists, or None. The screen's proc (script 0x08616638) keeps the
        country tab at +0x58 and the place in it at +0x52; the lists are the
        DS Campaign's (CO ids by country tab, each tab's count)."""
        e = self.e
        p = next((p for p in range(0x0200D610, 0x0200E418, 0x6C) if e.u32(p) == CO_SCREEN), None)
        if p is None:
            return None
        groups = e.u32(CO_GROUPS)
        if not 1 <= groups <= 5:
            return None
        counts = list(e.read(CO_GROUP_COUNTS, groups))
        cos = list(e.read(CO_LIST, sum(counts)))
        tab, at = e.u8(p + 0x58), e.u8(p + 0x52)
        if tab >= groups or at >= counts[tab]:
            return None
        return {"co": cos[sum(counts[:tab]) + at], "tab": tab, "at": at, "counts": counts, "cos": cos}

    def choose_co(self, prefs=None):
        """On the CO screen: the first CO of `prefs` (tangoAW2 CO ids) the
        screen offers, moved to with the pad (DOWN: the next country tab,
        RIGHT: the next CO in it), then A. Returns the CO picked."""
        e = self.e
        prefs = CO_PREFS if prefs is None else prefs
        c = None
        for _ in range(60):
            c = self.co_cursor()
            if c:
                break
            e.wait(10)
        if not c:
            raise NavError("the CO screen's cursor not found")
        want = next((co for co in prefs if co in c["cos"]), c["co"])
        k = c["cos"].index(want)
        tab = next(t for t in range(len(c["counts"])) if k < sum(c["counts"][:t + 1]))
        at = k - sum(c["counts"][:tab])
        for _ in range(12):
            c = self.co_cursor()
            if c["tab"] == tab:
                break
            e.press("DOWN" if c["tab"] < tab else "UP", 6)
            e.wait(30)
        c = self.co_cursor()
        if c["tab"] != tab:
            # (a tab the screen keeps shut, e.g. the first pick's country
            # for the second army): the best CO of the tab it is on
            first = sum(c["counts"][:c["tab"]])
            here = c["cos"][first:first + c["counts"][c["tab"]]]
            want = next((co for co in prefs if co in here), c["co"])
            tab, at = c["tab"], here.index(want)
        for _ in range(12):
            c = self.co_cursor()
            if c["at"] == at:
                break
            e.press("RIGHT" if c["at"] < at else "LEFT", 6)
            e.wait(30)
        if self.co_cursor()["co"] != want:
            raise NavError(f"CO {want} not reached ({self.co_cursor()})")
        e.press("A", 6)
        return want

    def choose_cos(self, count, prefs=None, max_frames=6000):
        """The CO screens until the battle loads: `count` picks (the armies
        the player picks for), each the best of `prefs` not picked yet.
        Returns the COs picked."""
        e = self.e
        prefs = CO_PREFS if prefs is None else prefs
        picks = []
        n = 0
        while not self.in_battle() and n < max_frames:
            if len(picks) < count and self.on_co_select() and self.co_cursor():
                picks.append(self.choose_co([c for c in prefs if c not in picks]))
                e.wait(60)
                n += 60
            elif self.on_co_select() or self.scripts_running():
                e.press("A", 6)
            e.wait(10)
            n += 16
        return picks

    def cursor(self):
        return (self.e.u16(0x030033E4), self.e.u16(0x030033E6))

    def wait_control(self, max_frames=20000):
        """Presses A through dialogue until the map cursor answers the pad.
        Once the mission is over (the results, the world map) it stops: its
        LEFT/RIGHT probes would move the world map's cursor."""
        e = self.e
        n = 0
        while n < max_frames:
            if e.u8(LAST_RESULT) and not self.in_battle():
                raise NavError("the mission is over")
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
                "condition": e.u32(LAST_CONDITION), "condition_day": e.u16(LAST_CONDITION + 4),
                "cause": e.u32(WIN_CAUSE), "cause_day": e.u16(WIN_CAUSE + 4)}

    def end_reason(self):
        """Why the battle just ended: the Dual Strike condition an event
        script ended it on, else AW2's own rules (the other team has no
        units left, or its HQ was taken)."""
        e = self.e
        cause = e.u32(WIN_CAUSE)
        if cause:
            return f"Dual Strike's condition {cause:#x}: {CONDITIONS.get(cause, '?')} (day {e.u16(WIN_CAUSE + 4)})"
        p = self.players()
        team = e.u8(p + 0x3C + 0x2A)
        foes = [u for u in self.g.units() if e.u8(p + 0x3C * u["army"] + 0x2A) != team]
        return "every enemy unit destroyed" if not foes else f"the enemy HQ taken ({len(foes)} enemy units left)"

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
        out["reason"] = self.end_reason() if out["result"] else None
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
                    if e.u8(LAST_RESULT):
                        break
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
        out["reason"] = self.end_reason() if out["result"] else None
        out["days"] = last_day - start + 1
        out["was_mission"] = mission
        return out

    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)

    def win_mission(self, step, max_days=40, log=None, seed=None, cos=None, player=None):
        """A player's mission: from the title, DS CAMPAIGN with the record at
        `step` (every mission before it won), the mission picked on the world
        map, the test player's COs, the battle played through the pad
        (aw2test.bot, PLANS) for up to `max_days`, then the results and back
        to the world map. Returns play()'s outcome with the COs picked, the
        world map's flags and the record's progress afterwards."""
        e = self.e
        data = DsData()
        index = ORDER[step]
        opts = plan(data, index)
        if seed is not None:
            opts["seed"] = seed
        prefs = cos if cos is not None else opts.pop("cos", None)
        opts.pop("cos", None)
        player = player or opts.pop("player", "bot")
        opts.pop("player", None)
        self.start(step=step)
        picks = self.choose_cos(co_picks(data, index), prefs)
        # The player's side played by the test player (the pad), or by the
        # game's own CPU (its armies handed to the computer).
        r = self.autoplay(max_days, log=log) if player == "cpu" else self.play(max_days, log=log, **opts)
        r["player"] = player
        r["cos"] = picks
        # The results, then the world map (and the story Dual Strike plays
        # there after this mission): every text shown on the way.
        texts = []
        for _ in range(3000):
            t = self.text_shown()
            if t and (not texts or texts[-1] != t):
                texts.append(t)
            if self.world_map_up() and e.u8(WM_STATE + 0x10):
                break
            if self.scripts_running() or not self.in_battle():
                e.press("A", 4)
            e.wait(10)
        r["texts_after"] = texts
        e.wait(30)
        r["map"] = self.world_map_up()
        r["flags"] = self.map_flags()
        r["progress"] = self.progress()
        return r


# -- Dual Strike's campaign, read from the .nds (independent of the Rust) -------
DS_OV0 = 0x022AD560
DS_RECORDS = 0x022DBD28 + 0xA0 * 0xE0   # the campaign's map records, 0xA0 bytes
DS_TEXT_GROUPS = DS_OV0 + 0x49690
# The campaign's order (ds_campaign::ORDER): 25 story missions with the three
# research-lab side missions (records 25..27) after the missions that open them.
ORDER = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 25, 10, 11, 26, 12, 13, 27, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]


# The CO screen (crate::ds_campaign's lists) and the test player's choice
# of CO, best first: Kanbei, Hawke, Max, Grimm, Jess, Andy, Jake, Rachel,
# Sensei, Nell, Javier, Sami, Sasha, Koal, Kindle, Jugger (tangoAW2 ids).
CO_SCREEN = 0x08616638
CO_LIST = 0x030058E0
CO_GROUPS = 0x03005944
CO_GROUP_COUNTS = 0x03005948
CO_PREFS = [6, 14, 2, 76, 17, 1, 79, 80, 18, 0, 77, 4, 78, 73, 74, 72]


# How the test player plays a mission (aw2test.bot.Bot's options), by
# mission index: units the mission is lost without (The New Black's
# Infantry, Black Boats Ahoy!'s Lander), missions won on their structures
# (a Black Crystal, minicannons, Black Obelisks, the Grand Bolt's weak
# points), and those against the clock (always on the attack).
PLANS = {1: {"protect": [1], "stance": "attack"}, 9: {"protect": [23]}, 12: {"rush": True}, 16: {"rush": True},
         # Means to an End: bombers and B copters from its airports fly over
         # the Oozium to the Grand Bolt's weak points.
         24: {"build": {0xA: [17, 19, 16]}}}
STRUCTURE_MISSIONS = {8, 13, 14, 17, 18, 23, 24}
TIMED_MISSIONS = {12, 21, 22, 24}


def plan(data, index):
    """The bot's options for mission `index`: PLANS, and as goals Dual
    Strike's research labs (the lab missions are won on them) and, for
    Surrounded!, its Com Towers."""
    m = data.mission(index)
    w = m["w"]
    cells = [(i % w, i // w) for i, t in enumerate(m["tiles"]) if 0x1D9 <= t <= 0x1DD]
    if index == 22:
        cells += [(i % w, i // w) for i, t in enumerate(m["tiles"]) if 0x1B9 <= t <= 0x1BD]
    out = dict(PLANS.get(index, {}))
    if cells:
        out["goals"] = cells
    if index in STRUCTURE_MISSIONS:
        out["structures"] = True
    if index in TIMED_MISSIONS:
        out["stance"] = "attack"
    return out


# Dual Strike's story outside the battles (overlay 5; crate::ds_campaign_data
# Story): the scripts of the party after Crystal Calamity and of the ending
# after Means to an End, and the narration texts (bank 0x21).
STORY_SCRIPTS = {18: [0x023683C8, 0x023686E8, 0x023684E8],
                 24: [0x02369180, 0x02368CA0, 0x02368E40, 0x02368FE0, 0x02368B60]}
STORY_NARRATION = {8: [0x21000003]}
PROLOGUE = [0x21000000, 0x21000001, 0x21000002]


def story_texts(data, index):
    """The texts Dual Strike shows after mission `index`'s win, in order
    (none for most missions)."""
    if index in STORY_NARRATION:
        return [data.text(r) for r in STORY_NARRATION[index]]
    out = []
    for s in STORY_SCRIPTS.get(index, []):
        for k in range(400):
            c = data.ov5_bytes(s + 16 * k, 16)
            op, ref = c[0], struct.unpack_from("<I", c, 12)[0]
            if op in (0x19, 0x1A):
                out.append(data.text(ref))
            if op in (2, 3, 4):
                break
    return out


def same_text(shown, ds_text):
    """A shown (re-wrapped) text is Dual Strike's: the same words."""
    norm = lambda t: "".join(ch for ch in t if ch.isalnum())
    return norm(shown) == norm(ds_text.decode("latin-1") if isinstance(ds_text, bytes) else ds_text)


def co_picks(data, index):
    """How many armies of mission `index` the player picks a CO for (Dual
    Strike's 0x1C)."""
    m = data.mission(index)
    return sum(1 for k in range(min(m["armies"], 4)) if m["cos"][k] == 0x1C)


class DsData:
    """Dual Strike's campaign missions as its ROM has them."""

    def __init__(self, ds=None):
        from . import rom as romlib
        self.ds = ds or romlib.DualStrike()
        self.lz10 = romlib.lz10

    def bytes(self, a, n):
        o = a - DS_OV0
        return self.ds.ov0[o:o + n]

    def ov5_bytes(self, a, n):
        """Overlay 5 (the story), loaded at overlay 1's address."""
        if not hasattr(self, "_ov5"):
            from . import paths
            rom = open(paths.ds_rom(), "rb").read()
            fat = struct.unpack_from("<I", rom, 0x48)[0]
            ovt = struct.unpack_from("<I", rom, 0x50)[0]
            fid = struct.unpack_from("<I", rom, ovt + 32 * 5 + 0x18)[0]
            lo, hi = struct.unpack_from("<II", rom, fat + 8 * fid)
            self._ov5 = rom[lo:hi]
        o = a - 0x02350560
        return self._ov5[o:o + n]

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
