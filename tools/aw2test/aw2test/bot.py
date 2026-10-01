"""A player for the tests: plays a human army's turn through the game's own
inputs only (select, move, fire, capture, wait, build, power, end turn),
reading the game's state from RAM to decide. Nothing it does is anything a
player could not do with the pad.

The movement range the game shows when a unit is picked up is gMap's layer
at +0x2852 (a byte per cell through the row offsets: the move points spent
to get there, 0xFF out of reach).

How it plays: every unit, best attack first, picks the attack worth most for
what it costs (the counter-attack and what the enemy can do to it where it
stands, Dual Strike's damage chart and terrain); units without a good attack
capture, go back to a property to be repaired when worn down, or move up on
the goal (the mission's cells or structures, else the enemy) by the safest
cells, holding out of the enemy's reach while the enemy is the stronger."""

import os

from .game import NavError
from .dscampaign import LAST_RESULT, DAY

MAP = 0x0201E450
RANGE = MAP + 0x2852
ROWS = MAP + 0x417A
CLASSES = MAP + 0x1432
LAYER = MAP + 0x12            # unit ids by cell
UNIT_TABLE = 0x08680000       # tangoAW2's unit table with the pack (crate::roster)
CO_TABLE_POOL = 0x08042DDC    # the CO table the game reads (0x104 bytes a CO)
UNIT_RECORD = 0x5C
PLAYERS_PTR = 0x08499598
CURRENT_ARMY = 0x030033EC
# The structures ("inventions": minicannons, Black Crystals, Obelisks...):
# 16 entries of 8 bytes (x, y, word: kind in bits 6..9, HP at +4).
INVENTIONS = 0x02028360
# Terrain kinds (class & 0x1F) of properties, and those that build units.
HQ, CITY, BASE, AIRPORT, PORT, LAB = 8, 6, 0xE, 0xA, 0xB, 0x14
PROPERTIES = {HQ, CITY, BASE, AIRPORT, PORT, LAB}
CAPTURERS = {1, 2}            # Infantry, Mech
AIR = {12, 13, 16, 17, 19, 20}
NAVAL = {18, 21, 22, 23, 24, 25, 26}
ANTI_AIR = {14, 15, 16}
# Transports that carry infantry and mech (APC, T Copter, Lander).
FOOT_TRANSPORTS = {7, 20, 23}
# Where a worn unit is repaired.
REPAIRS = {"ground": {HQ, CITY, BASE}, "air": {AIRPORT}, "naval": {PORT}}
# What a factory builds, best first (tangoAW2 / Dual Strike ids).
BUILD = {BASE: [8, 3, 5, 10, 11, 14, 2, 1], AIRPORT: [19, 16, 17], PORT: [22, 24]}
# Structures are hit as hard as a Md Tank.
STRUCTURE_AS = 3


def dist(a, b):
    return abs(a[0] - b[0]) + abs(a[1] - b[1])


def domain(t):
    return "air" if t in AIR else "naval" if t in NAVAL else "ground"


class Bot:
    def __init__(self, d, log=None, protect=(), hold=(), goals=(), structures=False, stance="auto"):
        """`protect`: unit types to keep out of harm (they wait where they
        are, or step away from enemies); `hold`: types that never move;
        `goals`: cells the mission is won on (capturers head there first);
        `structures`: the mission is won on its structures (they come first
        among targets); `stance`: "attack", "defend" or "auto" (attack when
        the stronger)."""
        self.d, self.g, self.e = d, d.g, d.e
        self.log = log or (lambda s: None)
        self.protect = set(protect)
        self.hold = set(hold)
        self.goals = list(goals)
        self.structures_goal = structures
        self.stance = stance
        from .rom import DualStrike, Image
        self.chart = DualStrike()
        self.image = Image.load()
        self.rows = {}
        self.infos = {}
        # Turns in a row with no HP lost on either side (a stand-off: then
        # it attacks).
        self.quiet = {}
        self.last_hp = {}
        self.debug = bool(os.environ.get("AW2TEST_BOT_DEBUG"))
        self.reach = {}
        self.charts = {}
        self.classes = {}
        self.gdist = {}
        self.occ_now = {}

    # -- state ------------------------------------------------------------------
    def size(self):
        return self.e.u16(MAP), self.e.u16(MAP + 2)

    def row(self, y):
        return self.e.u16(ROWS + 2 * y)

    def cls(self, x, y):
        return self.e.u8(CLASSES + self.row(y) + x)

    def unit_info(self, t):
        if t not in self.infos:
            a = UNIT_TABLE + UNIT_RECORD * t
            self.infos[t] = {"cost": self.e.u16(a + 6) * 10, "move": self.e.u8(a + 0x0A),
                             "min": self.e.u8(a + 0x0E), "max": self.e.u8(a + 0x0F)}
        return self.infos[t]

    def team(self, army):
        return self.e.u8(self.e.u32(PLAYERS_PTR) + 0x3C * army + 0x2A)

    def funds(self, army):
        return self.e.u32(self.e.u32(PLAYERS_PTR) + 0x3C * army)

    def enemies(self, army):
        t = self.team(army)
        return [u for u in self.g.units() if self.team(u["army"]) != t and not (u["flags"] & 0x08)]

    def friends(self, army):
        t = self.team(army)
        return [u for u in self.g.units() if self.team(u["army"]) == t and not (u["flags"] & 0x08)]

    def structures(self):
        """Standing structures: dict(x, y, hp, kind) (every one is Black Hole's)."""
        out = []
        for k in range(16):
            b = self.e.read(INVENTIONS + 8 * k, 8)
            kind = (b[2] | b[3] << 8) >> 6 & 0xF
            if kind == 0:
                break
            if b[4] > 0:
                out.append({"x": b[0], "y": b[1], "hp": b[4], "kind": kind})
        return out

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

    def properties(self):
        w, h = self.size()
        out = []
        for y in range(h):
            for x in range(w):
                c = self.cls(x, y)
                if c & 0x1F in PROPERTIES:
                    out.append(((x, y), c & 0x1F, c >> 5))
        return out

    def targets_for_capture(self, army, props=None):
        """Properties not ours (the enemy HQ first)."""
        t = self.team(army)
        out = [(p, k) for p, k, o in (props or self.properties()) if o == 0 or self.team(o) != t]
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
        stars = self.stars(*cell) if dfd not in AIR else 0
        d = b * shown / 10 * (100 - stars * ((dfd_hp + 9) // 10)) / 100
        return min(dfd_hp, int(d))

    def armed(self, t):
        return any(self.base(t, d) > 0 for d in range(1, 26))

    def cost(self, t):
        return max(self.unit_info(t)["cost"], 1000)

    def value(self, u):
        return self.cost(u["type"]) * u["hp"] / 100

    def threat(self, u, cell, foes, hp=None):
        """What the enemy can do to `u` standing on `cell` next turn (HP points)."""
        hp = u["hp"] if hp is None else hp
        t = 0
        for f in foes:
            if not self.can_hit(f, cell):
                continue
            t += self.damage(f["type"], f["hp"], u["type"], hp, cell)
        return min(t, hp)

    def can_hit(self, f, cell):
        """Foe `f` can fire on `cell` next turn: from where it stands
        (indirect), or from a cell it can move to (worked out as the game
        works out a unit's range)."""
        info = self.unit_info(f["type"])
        d = dist(cell, (f["x"], f["y"]))
        if info["min"] > 1:
            return info["min"] <= d <= info["max"]
        if d > info["move"] + 1:
            return False
        key = (f.get("id"), f["x"], f["y"])
        if key not in self.reach:
            cells = set(self.moves(f))
            hits = set(cells)
            for (x, y) in cells:
                hits.update({(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)})
            self.reach[key] = hits
        return cell in self.reach[key]

    # -- movement (AW2's own: the CO's movement chart, a row of 32 terrain
    # costs per movement type, negative where the type cannot go) ----------
    def move_chart(self, army):
        co = self.e.u8(self.e.u32(PLAYERS_PTR) + 0x3C * army + 0x1D)
        if co not in self.charts:
            p = self.e.u32(self.e.u32(CO_TABLE_POOL) + 0x104 * co + 0x38 + 0x18)
            self.charts[co] = [x - 256 if x > 127 else x for x in self.e.read(p, 32 * 16)]
        return self.charts[co]

    def move_type(self, t):
        return self.e.u8(UNIT_TABLE + UNIT_RECORD * t + 0x19)

    def step_cost(self, chart, mt, cell):
        k = chart[mt * 32 + (self.cls_at(cell) & 0x1F)]
        return k if k > 0 else None

    def cls_at(self, cell):
        if cell not in self.classes:
            self.classes[cell] = self.cls(*cell)
        return self.classes[cell]

    def moves(self, u):
        """Where `u` can move this turn (enemy units block it)."""
        import heapq
        w, h = self.size()
        chart, mt = self.move_chart(u["army"]), self.move_type(u["type"])
        move = self.unit_info(u["type"])["move"]
        team = self.team(u["army"])
        occ = self.occ_now
        start = (u["x"], u["y"])
        best = {start: 0}
        q = [(0, start)]
        while q:
            c, (x, y) = heapq.heappop(q)
            if c > best[(x, y)]:
                continue
            for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if not (0 <= n[0] < w and 0 <= n[1] < h):
                    continue
                k = self.step_cost(chart, mt, n)
                o = occ.get(n)
                if k is None or (o and self.team(o["army"]) != team):
                    continue
                if c + k <= move and c + k < best.get(n, 999):
                    best[n] = c + k
                    heapq.heappush(q, (c + k, n))
        return best

    def goal_distance(self, u, goals):
        """Move points from each cell to the nearest goal for `u`'s kind of
        movement (units ignored); cells with no way there are left out."""
        import heapq
        key = (u["army"], self.move_type(u["type"]), tuple(sorted(goals)))
        if key in self.gdist:
            return self.gdist[key]
        w, h = self.size()
        chart, mt = self.move_chart(u["army"]), self.move_type(u["type"])
        best = {g: 0 for g in goals}
        q = [(0, g) for g in goals]
        heapq.heapify(q)
        while q:
            c, (x, y) = heapq.heappop(q)
            if c > best[(x, y)]:
                continue
            # (the cost of a step is the cell stepped onto: from n to here)
            k = self.step_cost(chart, mt, (x, y)) or 0
            for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if not (0 <= n[0] < w and 0 <= n[1] < h) or self.step_cost(chart, mt, n) is None:
                    continue
                if c + k < best.get(n, 1 << 30):
                    best[n] = c + k
                    heapq.heappush(q, (c + k, n))
        self.gdist[key] = best
        return best

    def scout(self, army):
        """A on each armed enemy unit (direct fire) within reach of ours:
        the game shows where it can move; the cells it can hit from there
        are kept for the turn. B puts the range away."""
        g, e = self.g, self.e
        self.reach = {}
        mine = [(m["x"], m["y"]) for m in self.friends(army)]
        for f in self.enemies(army):
            info = self.unit_info(f["type"])
            if info["min"] > 1 or not self.armed(f["type"]):
                continue
            if not mine or min(dist((f["x"], f["y"]), m) for m in mine) > 2 * info["move"] + 10:
                continue
            try:
                g.goto(f["x"], f["y"])
            except NavError:
                continue
            e.press("A", 4)
            e.wait(14)
            if g.has_proc(0x080228D9):
                cells = set(self.range_cells())
                cells.add((f["x"], f["y"]))
                hits = set(cells)
                for (x, y) in cells:
                    hits.update({(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)})
                self.reach[(f["id"], f["x"], f["y"])] = hits
            e.press("B", 4)
            e.wait(10)
            if not g.has_proc(0x080228D9) and g.menu():
                e.press("B", 4)
                e.wait(10)

    def attack_score(self, u, cell, f, indirect, foes):
        """Worth of `u` firing at `f` from `cell` (funds' worth; None: no damage)."""
        if f.get("structure"):
            b = self.base(u["type"], STRUCTURE_AS)
            if b <= 0:
                return None
            dealt = min(f["hp"], int(b * ((u["hp"] + 9) // 10) / 10))
            value = dealt / 100 * (8000 if self.structures_goal else 2500) + (6000 if dealt >= f["hp"] and self.structures_goal else 0)
            after = self.threat(u, cell, foes) if not indirect else 0
            return value - 0.6 * after / 100 * self.cost(u["type"])
        dealt = self.damage(u["type"], u["hp"], f["type"], f["hp"], (f["x"], f["y"]))
        if dealt <= 0:
            return None
        kill = dealt >= f["hp"]
        # Its worth, what it would do next turn (a unit taking one of our
        # properties most of all), and a kill ends both.
        worth = self.cost(f["type"]) + (4000 if self.capturing(f) else 0)
        value = dealt / 100 * worth + 15 * dealt + (0.5 * worth + 1500 if kill else 0)
        loss = 0
        hp = u["hp"]
        if not indirect and not kill and self.unit_info(f["type"])["min"] <= 1:
            back = self.damage(f["type"], f["hp"] - dealt, u["type"], u["hp"], cell)
            loss = back / 100 * self.cost(u["type"])
            hp -= back
        # Where it is left standing: what the others can do to it.
        others = [o for o in foes if not kill or o.get("id") != f.get("id")]
        after = self.threat(u, cell, others, max(hp, 1)) if not indirect else 0
        return value - 0.7 * loss - 0.4 * after / 100 * self.cost(u["type"])

    def capturing(self, f):
        """`f` stands on a property of another army (taking it)."""
        if f["type"] not in CAPTURERS:
            return False
        c = self.cls(f["x"], f["y"])
        return c & 0x1F in PROPERTIES and c >> 5 not in (0, f["army"])

    # -- planning ---------------------------------------------------------------
    def strength(self, army):
        mine = sum(self.value(u) for u in self.friends(army) if self.armed(u["type"]))
        theirs = sum(self.value(u) for u in self.enemies(army) if self.armed(u["type"]))
        return mine, theirs

    def aggressive(self, army):
        if self.stance != "auto":
            return self.stance == "attack"
        mine, theirs = self.strength(army)
        day = self.e.u16(DAY)
        return mine >= 1.25 * theirs or day >= 18 or theirs == 0 or self.quiet.get(army, 0) >= 2

    def rough_attack(self, u, foes):
        """A quick guess of `u`'s best attack this turn (no range read)."""
        info = self.unit_info(u["type"])
        here = (u["x"], u["y"])
        best = 0
        for f in foes:
            d = dist(here, (f["x"], f["y"]))
            if info["min"] > 1:
                if not info["min"] <= d <= info["max"]:
                    continue
            elif d > info["move"] + 1:
                continue
            if f.get("structure"):
                dealt = self.base(u["type"], STRUCTURE_AS)
            else:
                dealt = self.damage(u["type"], u["hp"], f["type"], f["hp"], (f["x"], f["y"]))
            best = max(best, dealt)
        return best

    def targets(self, army):
        out = list(self.enemies(army))
        for s in self.structures():
            out.append({"x": s["x"], "y": s["y"], "hp": s["hp"], "type": STRUCTURE_AS, "structure": True})
        return out

    # -- one action -------------------------------------------------------------
    def cancel(self):
        for _ in range(3):
            self.e.press("B", 4)
            self.e.wait(10)
        self.d.dialogue()

    def plan(self, u, army, cells):
        """Where `u` goes and what it does there: (kind, cell, target)."""
        info = self.unit_info(u["type"])
        here = (u["x"], u["y"])
        occ = self.occupied()
        free = [c for c in cells if c == here or c not in occ]
        foes = [f for f in self.enemies(army)]
        armed_foes = [f for f in foes if self.armed(f["type"])]
        targets = self.targets(army)
        indirect = info["min"] > 1
        if u["type"] in self.hold:
            return ("stay", here, None)
        if u["type"] in self.protect and u["type"] in CAPTURERS and not any(self.threat(u, c, armed_foes) == 0 for c in free):
            # No cell out of the enemy's reach: aboard an empty transport.
            boats = [(m["x"], m["y"]) for m in self.friends(army)
                     if m["type"] in FOOT_TRANSPORTS and m["cargo"] == (0, 0) and (m["x"], m["y"]) in cells]
            if boats:
                return ("load", min(boats, key=lambda c: cells[c]), None)
        if u["type"] in self.protect or not self.armed(u["type"]):
            if armed_foes:
                # Out of the enemy's reach, else as little in it as can be,
                # and among our armed units (they keep the enemy off).
                guards = [(m["x"], m["y"]) for m in self.friends(army) if m["id"] != u["id"] and self.armed(m["type"])]
                near = lambda c: min((dist(c, m) for m in guards), default=0)
                best = min(free, key=lambda c: (self.threat(u, c, armed_foes), near(c), cells[c]))
                return ("wait", best, None)
            return ("stay", here, None)
        # Fire: from here (indirect) or from a free cell next to the target.
        if indirect:
            cands = [(here, f) for f in targets if info["min"] <= dist(here, (f["x"], f["y"])) <= info["max"]]
        else:
            cands = [(c, f) for f in targets for c in free if dist(c, (f["x"], f["y"])) == 1]
        scored = [(self.attack_score(u, c, f, indirect, armed_foes), c, f) for c, f in cands]
        if self.debug:
            top = sorted((s for s in scored if s[0] is not None), key=lambda s: -s[0])[:4]
            self.log(f"    {len(cands)} attacks: " + ", ".join(f"{s[0]:.0f} from {s[1]} at {(s[2]['x'], s[2]['y'])}" for s in top))
        scored = [s for s in scored if s[0] is not None and s[0] > 0]
        if scored:
            scored.sort(key=lambda s: (-s[0], -self.stars(*s[1]), cells[s[1]]))
            _, c, f = scored[0]
            return ("fire", c, (f["x"], f["y"]))
        props = self.properties()
        # Capture: infantry and mech on (or onto) a property not ours.
        if u["type"] in CAPTURERS:
            capt = self.goals + [p for p, _ in self.targets_for_capture(army, props)]
            if here in capt:
                return ("capt", here, None)
            near = [c for c in free if c in capt]
            if near:
                return ("capt", min(near, key=lambda c: (capt.index(c), self.threat(u, c, armed_foes))), None)
        # Worn down: back to a property of ours that repairs it.
        if u["hp"] <= 40:
            t = self.team(army)
            mend = [p for p, k, o in props if k in REPAIRS[domain(u["type"])] and o and self.team(o) == t]
            near = [c for c in free if c in mend]
            if near:
                return ("wait", min(near, key=lambda c: self.threat(u, c, armed_foes)), None)
            if mend and armed_foes:
                best = min(free, key=lambda c: (min(dist(c, m) for m in mend) + 3 * self.threat(u, c, armed_foes) / max(u["hp"], 10), cells[c]))
                return ("wait", best, None)
        # Move up on the goal.
        if u["type"] in CAPTURERS:
            goals = self.goals + [p for p, _ in self.targets_for_capture(army, props)][:4]
        else:
            goals = [(s["x"], s["y"]) for s in targets if s.get("structure")] if self.structures_goal else []
            goals = goals or self.goals or [(f["x"], f["y"]) for f in foes] or [p for p, _ in self.targets_for_capture(army, props)][:4]
        if not goals:
            return ("stay", here, None)
        attack = self.aggressive(army)
        hp = max(u["hp"], 10)
        gd = self.goal_distance(u, goals)
        if not any(c in gd for c in free):
            gd = None

        def key(c):
            danger = self.threat(u, c, armed_foes) / hp
            far = gd.get(c, 999) if gd is not None else min(dist(c, gl) for gl in goals)
            if indirect:
                w = 12
            elif attack:
                w = 1.5
            else:
                w = 8
            return (far + w * danger - 0.3 * self.stars(*c), cells[c])
        best = min(free, key=key)
        return ("wait", best, None)

    def act(self, u, army):
        """Pick up `u`, choose where to go and what to do there."""
        g, e = self.g, self.e
        here = (u["x"], u["y"])
        g.select(*here)
        e.wait(8)
        self.d.dialogue()
        cells = self.range_cells()
        kind, cell, target = self.plan(u, army, cells)
        self.log(f"  {u['type']} {here} hp {u['hp']} -> {kind} {cell}" + (f" at {target}" if target else ""))
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
                self.pick_target(*target)
            except NavError:
                e.press("A", 4)  # the target the game offers
        elif kind == "capt" and any(n.startswith("capt") for n in names):
            g.choose("Capt", g.ACTION_MENU)
        elif kind == "load" and any(n.startswith("load") for n in names):
            g.choose("Load", g.ACTION_MENU)
        elif any(n.startswith("wait") for n in names):
            g.choose("Wait", g.ACTION_MENU)
        else:
            self.cancel()
            return
        e.wait(30)
        self.d.dialogue()

    def pick_target(self, x, y):
        """In target selection: cycle the targets until the cursor is on (x, y), then A."""
        e, g = self.e, self.g
        e.wait(10)
        for key in ["RIGHT"] * 16 + ["DOWN"] * 8:
            if g.cursor() == (x, y):
                e.press("A", 4)
                return
            e.press(key, 4)
            e.wait(8)
        raise NavError(f"target cursor at {g.cursor()}, wanted {(x, y)}")

    def build(self, army):
        """Every free factory of ours builds the best unit the funds allow
        (an Anti-Air first while the enemy has more aircraft than we have
        anti-air units)."""
        w, h = self.size()
        occ = self.occupied()
        foes = self.enemies(army)
        mine = self.friends(army)
        air = sum(1 for f in foes if f["type"] in AIR)
        aa = sum(1 for m in mine if m["type"] in ANTI_AIR)
        for (x, y), kind, owner in self.properties():
            if owner != army or kind not in BUILD or (x, y) in occ:
                continue
            funds = self.funds(army)
            order = list(BUILD[kind])
            if kind == BASE and air > aa:
                order = [14] + order
                aa += 1
            for t in order:
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
        """Every unit of `army` acts once (best attacks first), the factories
        build, the turn ends."""
        d, e = self.d, self.e
        d.wait_control()
        hp = (sum(u["hp"] for u in self.friends(army)), sum(u["hp"] for u in self.enemies(army)))
        self.quiet[army] = self.quiet.get(army, 0) + 1 if hp == self.last_hp.get(army) else 0
        self.last_hp[army] = hp
        self.use_power()
        d.wait_control()
        self.classes = {}
        self.gdist = {}
        done = set()
        for _ in range(80):
            if e.u8(LAST_RESULT) or e.u8(CURRENT_ARMY) != army:
                return
            ready = [u for u in self.g.units(army=army) if not (u["flags"] & 1) and not (u["flags"] & 0x08) and u["id"] not in done]
            if not ready:
                break
            # (the enemy's reach, again after each action: units moved, died)
            self.reach = {}
            self.occ_now = self.occupied()
            targets = self.targets(army)
            # Units with an attack first (the biggest hit first; indirect
            # ones before direct ones that could block them), then the rest,
            # the protected last.
            def order(u):
                hit = self.rough_attack(u, targets) if u["type"] not in self.protect else 0
                return (u["type"] in self.protect, hit == 0, self.unit_info(u["type"])["min"] <= 1, -hit)
            ready.sort(key=order)
            u = ready[0]
            done.add(u["id"])
            try:
                self.act(u, army)
            except NavError as ex:
                self.log(f"  {u['type']} at {(u['x'], u['y'])}: {ex}")
                if e.u8(LAST_RESULT):
                    return
                self.cancel()
            d.wait_control()
        try:
            self.build(army)
        except NavError as ex:
            self.log(f"  build: {ex}")
            self.cancel()
        d.wait_control()
        d.end_turn()
