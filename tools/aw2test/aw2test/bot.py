"""A player for the tests: plays a human army's turn through the game's own
inputs only (select, move, fire, capture, wait, build, end turn), reading
the game's state from RAM to decide. Nothing it does is anything a player
could not do with the pad.

The movement range the game shows when a unit is picked up is gMap's layer
at +0x2852 (a byte per cell through the row offsets: the move points spent
to get there, 0xFF out of reach)."""

from .game import NavError
from .dscampaign import LAST_RESULT

MAP = 0x0201E450
RANGE = MAP + 0x2852
ROWS = MAP + 0x417A
CLASSES = MAP + 0x1432
LAYER = MAP + 0x12            # unit ids by cell
UNIT_TABLE = 0x08680000       # tangoAW2's unit table with the pack (crate::roster)
UNIT_RECORD = 0x5C
PLAYERS_PTR = 0x08499598
CURRENT_ARMY = 0x030033EC
# Terrain kinds (class & 0x1F) of properties, and those that build units.
HQ, CITY, BASE, AIRPORT, PORT, LAB = 8, 6, 0xE, 0xA, 0xB, 0x14
PROPERTIES = {HQ, CITY, BASE, AIRPORT, PORT, LAB}
CAPTURERS = {1, 2}            # Infantry, Mech
# What a factory builds, best first (tangoAW2 / Dual Strike ids).
BUILD = {BASE: [8, 3, 5, 10, 11, 14, 2, 1], AIRPORT: [19, 16, 17], PORT: [22, 24]}


def dist(a, b):
    return abs(a[0] - b[0]) + abs(a[1] - b[1])


class Bot:
    def __init__(self, d, log=None, protect=(), hold=(), goals=()):
        """`protect`: unit types to keep out of harm (they wait where they
        are, or step away from enemies); `hold`: types that never move;
        `goals`: cells the mission is won on (capturers head there first)."""
        self.d, self.g, self.e = d, d.g, d.e
        self.log = log or (lambda s: None)
        self.protect = set(protect)
        self.hold = set(hold)
        self.goals = list(goals)
        from .rom import DualStrike, Image
        self.chart = DualStrike()
        self.image = Image.load()
        self.rows = {}

    # -- state ------------------------------------------------------------------
    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)

    def row(self, y):
        return self.e.u16(ROWS + 2 * y)

    def cls(self, x, y):
        return self.e.u8(CLASSES + self.row(y) + x)

    def unit_info(self, t):
        a = UNIT_TABLE + UNIT_RECORD * t
        return {"cost": self.e.u16(a + 6) * 10, "move": self.e.u8(a + 0x0A),
                "min": self.e.u8(a + 0x0E), "max": self.e.u8(a + 0x0F)}

    def team(self, army):
        return self.e.u8(self.e.u32(PLAYERS_PTR) + 0x3C * army + 0x2A)

    def funds(self, army):
        return self.e.u32(self.e.u32(PLAYERS_PTR) + 0x3C * army)

    def enemies(self, army):
        t = self.team(army)
        return [u for u in self.g.units() if self.team(u["army"]) != t and not (u["flags"] & 0x08)]

    def range_cells(self):
        w, h = self.size()
        out = {}
        for y in range(h):
            r = self.e.read(RANGE + self.row(y), w)
            for x in range(w):
                if r[x] != 0xFF:
                    out[(x, y)] = r[x]
        return out

    def occupied(self):
        return {(u["x"], u["y"]): u for u in self.g.units()}

    def targets_for_capture(self, army):
        """Properties not ours (the enemy HQ first)."""
        w, h = self.size()
        t = self.team(army)
        out = []
        for y in range(h):
            for x in range(w):
                c = self.cls(x, y)
                kind, owner = c & 0x1F, c >> 5
                if kind in PROPERTIES and (owner == 0 or self.team(owner) != t):
                    out.append(((x, y), kind))
        out.sort(key=lambda p: p[1] != HQ)
        return out

    # -- damage -----------------------------------------------------------------
    def base(self, att, dfd):
        """Dual Strike's base damage of `att` on `dfd` (the better weapon)."""
        if att not in self.rows:
            self.rows[att] = (self.chart.damage_row(att, 0), self.chart.damage_row(att, 1))
        p, s = self.rows[att]
        return max(p[dfd] if dfd < len(p) else 0, s[dfd] if dfd < len(s) else 0)

    def stars(self, x, y):
        try:
            return self.image.terrain_stars(self.cls(x, y))
        except Exception:
            return 0

    def damage(self, att, att_hp, dfd, dfd_hp, cell):
        """Expected damage (in HP points of 100) as AW2 works it out, COs aside."""
        b = self.base(att, dfd)
        if b <= 0:
            return 0
        shown = (att_hp + 9) // 10
        stars = self.stars(*cell) if dfd not in (16, 17, 19, 20) else 0
        d = b * shown / 10 * (100 - stars * ((dfd_hp + 9) // 10)) / 100
        return min(dfd_hp, int(d))

    def cost(self, t):
        return max(self.unit_info(t)["cost"], 1000)

    def attack_score(self, u, cell, f, indirect):
        dealt = self.damage(u["type"], u["hp"], f["type"], f["hp"], (f["x"], f["y"]))
        if dealt <= 0:
            return None
        value = dealt / 100 * self.cost(f["type"]) + (3000 if dealt >= f["hp"] else 0)
        loss = 0
        if not indirect and dealt < f["hp"] and self.unit_info(f["type"])["min"] <= 1:
            back = self.damage(f["type"], f["hp"] - dealt, u["type"], u["hp"], cell)
            loss = back / 100 * self.cost(u["type"])
        return value - 0.9 * loss

    def threat(self, cell, foes):
        """Rough danger of standing on `cell`: foes able to reach and hit it."""
        t = 0
        for f in foes:
            info = self.unit_info(f["type"])
            reach = info["max"] if info["min"] > 1 else info["move"] + 1
            if dist(cell, (f["x"], f["y"])) <= reach:
                t += f["hp"]
        return t

    # -- one action -------------------------------------------------------------
    def cancel(self):
        for _ in range(3):
            self.e.press("B", 4)
            self.e.wait(10)
        self.d.dialogue()

    def act(self, u, army):
        """Pick up `u`, choose where to go and what to do there."""
        g, e = self.g, self.e
        info = self.unit_info(u["type"])
        here = (u["x"], u["y"])
        g.select(*here)
        e.wait(8)
        self.d.dialogue()
        cells = self.range_cells()
        occ = self.occupied()
        free = [c for c in cells if c == here or c not in occ]
        foes = self.enemies(army)
        indirect = info["min"] > 1
        plan = None
        if u["type"] in self.hold:
            plan = ("stay", here)
        elif u["type"] in self.protect:
            # Out of reach of the enemy: the reachable cell farthest from them.
            if foes:
                best = max(free, key=lambda c: (min(dist(c, (f["x"], f["y"])) for f in foes), -cells[c]))
                plan = ("wait", best)
            else:
                plan = ("stay", here)
        else:
            # Fire: from here (indirect) or from a free cell next to a foe,
            # at the target worth most for what the counter costs.
            if indirect:
                cands = [(here, f) for f in foes if info["min"] <= dist(here, (f["x"], f["y"])) <= info["max"]]
            else:
                cands = [(c, f) for f in foes for c in free if dist(c, (f["x"], f["y"])) == 1]
            scored = [(self.attack_score(u, c, f, indirect), c, f) for c, f in cands]
            scored = [s for s in scored if s[0] is not None and s[0] > 0]
            if scored:
                scored.sort(key=lambda s: (-s[0], cells[s[1]]))
                _, c, f = scored[0]
                plan = ("fire", c)
                self.target = (f["x"], f["y"])
            # Capture: infantry and mech on (or onto) a property not ours.
            if plan is None and u["type"] in CAPTURERS:
                props = self.goals + [p for p, _ in self.targets_for_capture(army)]
                if here in props:
                    plan = ("capt", here)
                else:
                    near = [c for c in free if c in props]
                    if near:
                        plan = ("capt", min(near, key=lambda c: cells[c]))
            if plan is None:
                goals = [(f["x"], f["y"]) for f in foes]
                if u["type"] in CAPTURERS:
                    goals = self.goals + [p for p, _ in self.targets_for_capture(army)][:4] or goals
                elif not goals:
                    goals = self.goals + [p for p, _ in self.targets_for_capture(army)][:4]
                if goals:
                    # Closer to the goal, but not into more danger than the
                    # unit can take (indirect units keep out of reach).
                    hp = u["hp"]
                    def key(c):
                        danger = self.threat(c, foes)
                        far = min(dist(c, gl) for gl in goals)
                        penalty = danger * (3 if indirect else 0.5) / max(hp, 10)
                        return (far + penalty, -cells[c])
                    best = min(free, key=key)
                    plan = ("wait", best)
                else:
                    plan = ("stay", here)
        kind, cell = plan
        self.log(f"  {u['type']} {here} -> {kind} {cell}")
        try:
            m = g.move_to(*cell)
            self.d.dialogue()
            m = g.menu() or m
        except NavError:
            self.cancel()
            return
        names = [n.lower() for n in m["names"]]
        if kind == "fire" and any(n.startswith("fire") and "greyed" not in n for n in names):
            g.choose("Fire", g.ACTION_MENU)
            e.wait(12)
            try:
                g.pick_target(*self.target)
            except NavError:
                e.press("A", 4)  # the target the game offers
        elif kind == "capt" and any(n.startswith("capt") for n in names):
            g.choose("Capt", g.ACTION_MENU)
        elif any(n.startswith("wait") for n in names):
            g.choose("Wait", g.ACTION_MENU)
        else:
            self.cancel()
            return
        e.wait(30)
        self.d.dialogue()

    def build(self, army):
        """Every free factory of ours builds the best unit the funds allow."""
        w, h = self.size()
        occ = self.occupied()
        for y in range(h):
            for x in range(w):
                c = self.cls(x, y)
                kind, owner = c & 0x1F, c >> 5
                if owner != army or kind not in BUILD or (x, y) in occ:
                    continue
                funds = self.funds(army)
                for t in BUILD[kind]:
                    if self.unit_info(t)["cost"] <= funds:
                        try:
                            self.g.buy(x, y, t)
                            self.log(f"  built {t} at {(x, y)}")
                        except NavError:
                            self.cancel()
                        break
                self.d.dialogue()

    def use_power(self):
        """The CO's Super Power, else its Power, when the meter allows."""
        g = self.g
        try:
            m = g.open_map_menu()
        except NavError:
            self.cancel()
            return
        names = [n for n, fl in zip(m["names"], [m["flags"][i] for i in m["visible"]]) if fl == 0]
        for want in ("Super", "Power"):
            if any(n.startswith(want) for n in names):
                try:
                    g.choose(want, g.MAP_MENU)
                    self.e.wait(60)
                    self.d.dialogue()
                    self.d.wait_control()
                    self.log(f"  {want}")
                except NavError:
                    self.cancel()
                return
        self.cancel()

    def play_turn(self, army):
        """Every unit of `army` acts once, the factories build, the turn ends."""
        d, e = self.d, self.e
        d.wait_control()
        self.use_power()
        d.wait_control()
        done = set()
        for _ in range(60):
            if e.u8(LAST_RESULT) or e.u8(CURRENT_ARMY) != army:
                return
            ready = [u for u in self.g.units(army=army) if not (u["flags"] & 1) and u["id"] not in done]
            if not ready:
                break
            # Indirect units first (they fire before others block them), the
            # protected ones last.
            ready.sort(key=lambda u: (u["type"] in self.protect, -self.unit_info(u["type"])["min"]))
            u = ready[0]
            done.add(u["id"])
            try:
                self.act(u, army)
            except NavError as ex:
                self.log(f"  {u['type']} at {(u['x'], u['y'])}: {ex}")
                self.cancel()
            d.wait_control()
        try:
            self.build(army)
        except NavError as ex:
            self.log(f"  build: {ex}")
            self.cancel()
        d.wait_control()
        d.end_turn()
