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
MTE_CRYSTALS = {(3, 3), (9, 5), (15, 3)}  # Means to an End's crystals (ds_campaign_data::MTE_CRYSTALS)
AIR = {12, 13, 16, 17, 19, 20}
NAVAL = {18, 21, 22, 23, 24, 25, 26}
ANTI_AIR = {14, 15, 16}
OOZIUM = 27
# Transports that carry infantry and mech (APC, T Copter, Lander), and
# those that take units over water (a Lander any ground unit, a T Copter
# infantry and mech); a Lander loads and unloads on a beach or in a port.
FOOT_TRANSPORTS = {7, 20, 23}
FERRIES = {20, 23}
SHOAL = 0x0D
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
    def __init__(self, d, log=None, protect=(), hold=(), goals=(), structures=False, stance="auto", rush=False, seed=None, build=None,
                 finish=None, garrison=False, ooze=False):
        """`protect`: unit types to keep out of harm (they wait where they
        are, or step away from enemies); `hold`: types that never move;
        `goals`: cells the mission is won on (capturers head there first);
        `structures`: the mission is won on its structures (they come first
        among targets); `stance`: "attack", "defend" or "auto" (attack when
        the stronger); `rush`: the units that capture go for the enemy HQ
        only (a mission against the clock); `finish`: from that day on, as
        `rush` and on the attack (a mission against the clock: win it by
        the HQ in the last days); `garrison`: a unit always stands on our HQ
        (none can capture it from under it); `ooze`: against Ooziums (an
        Oozium eats a unit next to it, one a day: it is worth hunting down
        while it can be finished off, and the unit on the enemy HQ comes
        first)."""
        self.d, self.g, self.e = d, d.g, d.e
        self.log = log or (lambda s: None)
        self.protect = set(protect)
        self.hold = set(hold)
        self.goals = list(goals)
        self.structures_goal = structures
        self.stance = stance
        self.rush = rush
        self.finish = finish
        self.garrison = garrison
        self.ooze = ooze
        # `build`: what a factory kind builds, best first (instead of BUILD).
        self.build_order = {int(k): v for k, v in (build or {}).items()}
        # `seed`: another player's style (how much danger each kind of unit
        # takes, when it goes on the attack, which unit moves first among
        # equals); the same seed plays the same game.
        import random
        rng = random.Random(seed)
        jit = (lambda: 1.0) if seed is None else (lambda: rng.uniform(0.6, 1.5))
        self.w_indirect, self.w_attack, self.w_defend = 12 * jit(), 1.5 * jit(), 8 * jit()
        self.stronger = 1.25 * jit()
        self.tiebreak = (lambda u: 0) if seed is None else (lambda u, r=random.Random(seed): r.random())
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
        self.props = None
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
                # A 3x3 structure (an Obelisk, a Black Cannon) is listed by
                # its top-left cell and fired on at its bottom middle.
                x, y = (b[0], b[1]) if kind == 4 else (b[0] + 1, b[1] + 2)
                out.append({"x": x, "y": y, "hp": b[4], "kind": kind})
        # Means to an End: a weak point whose crystal (in its column, north
        # of it) stands is no target yet.
        crystals = [s for s in out if s["kind"] == 4 and (s["x"], s["y"]) in MTE_CRYSTALS]
        if crystals:
            out = [s for s in out if not (s["kind"] == 3 and any(c["x"] == s["x"] for c in crystals))]
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
        """(cell, kind, owner) of every property (read once an action)."""
        if self.props is not None:
            return self.props
        w, h = self.size()
        row = [self.e.read(CLASSES + self.row(y), w) for y in range(h)]
        self.props = [((x, y), row[y][x] & 0x1F, row[y][x] >> 5) for y in range(h) for x in range(w) if row[y][x] & 0x1F in PROPERTIES]
        return self.props

    def targets_for_capture(self, army, props=None):
        """Properties not ours (the enemy HQ first; with `rush`, the enemy
        HQ alone)."""
        t = self.team(army)
        out = [(p, k) for p, k, o in (props or self.properties()) if o == 0 or self.team(o) != t]
        if self.rush or self.late():
            out = [(p, k) for p, k in out if k == HQ]
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
        """Expected damage (in HP points of 100) as AW2 works it out, COs aside.
        An Oozium has no weapon: moving onto a unit next to it destroys it."""
        if att == OOZIUM:
            return 0 if dfd in NAVAL else dfd_hp
        b = self.base(att, dfd)
        if b <= 0:
            return 0
        shown = (att_hp + 9) // 10
        stars = self.stars(*cell) if dfd not in AIR else 0
        d = b * shown / 10 * (100 - stars * ((dfd_hp + 9) // 10)) / 100
        return min(dfd_hp, int(d))

    def armed(self, t):
        return t == OOZIUM or any(self.base(t, d) > 0 for d in range(1, 26))

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
        if self.ooze and f["type"] == OOZIUM:
            return d <= max(info["move"], 1)
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
        start = dict(goals) if isinstance(goals, dict) else {g: 0 for g in goals}
        key = (u["army"], self.move_type(u["type"]), tuple(sorted(start.items())))
        if key in self.gdist:
            return self.gdist[key]
        w, h = self.size()
        chart, mt = self.move_chart(u["army"]), self.move_type(u["type"])
        best = dict(start)
        q = [(c, g) for g, c in start.items()]
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
        if self.ooze:
            if f["type"] == OOZIUM:
                worth = max(worth, 12000)
            if self.on_enemy_hq(f, u["army"]):
                worth += 15000
        value = dealt / 100 * worth + 15 * dealt + (0.5 * worth + 1500 if kill else 0)
        if self.late():
            # The last days of a mission against the clock: every hit counts.
            return value
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

    def on_enemy_hq(self, f, army):
        """`f` stands on an HQ of the enemy of `army` (in the way of its capture)."""
        c = self.cls(f["x"], f["y"])
        return c & 0x1F == HQ and c >> 5 and self.team(c >> 5) != self.team(army)

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

    def late(self):
        return self.finish is not None and self.e.u16(DAY) >= self.finish

    def aggressive(self, army):
        if self.late():
            return True
        if self.stance != "auto":
            return self.stance == "attack"
        mine, theirs = self.strength(army)
        day = self.e.u16(DAY)
        return mine >= self.stronger * theirs or day >= 18 or theirs == 0 or self.quiet.get(army, 0) >= 2

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
        # Ferrying: a transport takes units the enemy cannot be reached
        # from over the water; they board it where it waits.
        if u["type"] in FERRIES or u["type"] == 7:
            p = self.ferry_plan(u, army, cells, free, armed_foes)
            if p:
                return p
        elif domain(u["type"]) == "ground":
            p = self.board_plan(u, army, cells, free)
            if p:
                return p
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
        # A garrison: whoever stands on our HQ stays (firing from there if
        # it can); with nobody on it, the first unit that can get onto it.
        if self.garrison and u["type"] not in CAPTURERS | self.protect:
            t = self.team(army)
            hqs = [p for p, k, o in self.properties() if k == HQ and o and self.team(o) == t]
            mine = {(m["x"], m["y"]) for m in self.friends(army) if m["id"] != u["id"]}
            if here in hqs:
                free = [here]
            elif not any(h in mine for h in hqs):
                onto = [c for c in hqs if c in free]
                if onto:
                    return ("wait", onto[0], None)
        # Our HQ with an enemy that captures in reach of it: whoever stands
        # on it stays (firing from there if it can), else a unit that can
        # get onto it does (a unit on it cannot be captured from under it).
        guard = self.hq_in_danger(army)
        if guard and here in guard:
            free = [here]
        elif guard:
            onto = [c for c in guard if c in free]
            if onto and u["type"] not in self.protect:
                return ("wait", onto[0], None)
        # A unit that captures and can stand on the enemy HQ (or a goal)
        # captures it before anything else: that capture ends the army.
        if u["type"] in CAPTURERS:
            hq = self.goals + [p for p, k in self.targets_for_capture(army) if k == HQ]
            if here in hq:
                return ("capt", here, None)
            onto = [c for c in free if c in hq]
            if onto:
                return ("capt", min(onto, key=lambda c: (hq.index(c), self.threat(u, c, armed_foes))), None)
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
            # Nothing left to capture: on the enemy's units like the rest.
            goals = goals or [(f["x"], f["y"]) for f in foes]
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
                w = self.w_indirect
            elif attack:
                w = self.w_attack
            else:
                w = self.w_defend
            return (far + w * danger - 0.3 * self.stars(*c), cells[c])
        best = min(free, key=key)
        return ("wait", best, None)

    def hq_in_danger(self, army):
        """Our HQs an enemy that captures can get onto next turn (or stands on)."""
        t = self.team(army)
        hqs = [p for p, k, o in self.properties() if k == HQ and o and self.team(o) == t]
        if not hqs:
            return []
        capturers = [f for f in self.enemies(army) if f["type"] in CAPTURERS]
        out = []
        for hq in hqs:
            for f in capturers:
                if (f["x"], f["y"]) == hq or hq in self.moves(f):
                    out.append(hq)
                    break
        return out

    # -- ferrying ----------------------------------------------------------------
    def nbrs(self, c):
        w, h = self.size()
        return [n for n in ((c[0] + 1, c[1]), (c[0] - 1, c[1]), (c[0], c[1] + 1), (c[0], c[1] - 1)) if 0 <= n[0] < w and 0 <= n[1] < h]

    def goals_for(self, u, army):
        """The cells `u` makes for: what it captures, else the structures
        the mission is won on, else the enemy's units, else what is left to
        capture."""
        props = self.properties()
        capt = self.goals + [p for p, _ in self.targets_for_capture(army, props)][:4]
        if u["type"] in CAPTURERS:
            return capt
        structs = [(s["x"], s["y"]) for s in self.structures()] if self.structures_goal else []
        land = [(f["x"], f["y"]) for f in self.enemies(army) if domain(f["type"]) == "ground"]
        return structs or self.goals or land or capt

    def stranded(self, u, army):
        """No way over land from `u` to any of its goals."""
        goals = self.goals_for(u, army)
        return bool(goals) and (u["x"], u["y"]) not in self.goal_distance(u, goals)

    def fits(self, t, u):
        if t["type"] == 23:
            return domain(u["type"]) == "ground"
        return u["type"] in CAPTURERS

    def room(self, t):
        cargo = [i for i in t["cargo"] if i]
        return len(cargo) < (2 if t["type"] == 23 else 1)

    def can_unload_at(self, t, c):
        if t["type"] == 23:
            return self.cls_at(c) & 0x1F in (SHOAL, PORT)
        return True

    def turns(self, u, gd, cell):
        """Turns `u` needs from `cell` to its goal (gd: move points)."""
        v = gd.get(cell)
        return None if v is None else v / max(self.unit_info(u["type"])["move"], 1)

    def wants_ride(self, u, army):
        """`u` cannot get to its goal over land, or (a unit that captures)
        needs three turns or more to walk there."""
        goals = self.goals_for(u, army)
        if not goals:
            return False
        gd = self.goal_distance(u, goals)
        t = self.turns(u, gd, (u["x"], u["y"]))
        return t is None or (u["type"] in CAPTURERS and t >= 3)

    def board_plan(self, u, army, cells, free):
        ferries = [m for m in self.friends(army) if m["type"] in FERRIES | {7} and self.fits(m, u) and self.room(m)
                   and (m["type"] != 7 or u["type"] in CAPTURERS)]
        if not ferries or not self.wants_ride(u, army):
            return None
        on = [(m["x"], m["y"]) for m in ferries if (m["x"], m["y"]) in cells]
        if on:
            return ("load", on[0], None)
        if not self.stranded(u, army):
            return None
        gd = self.goal_distance(u, [(m["x"], m["y"]) for m in ferries])
        best = min(free, key=lambda c: (gd.get(c, 999), cells[c]))
        return ("wait", best, None)

    def ferry_plan(self, t, army, cells, free, foes):
        """A transport's turn: loaded, it carries its cargo to where the
        cargo gets to its goal soonest (its own way there, then the cargo's
        walk) and drops it when that is as soon as it gets; empty, it goes
        to wait where a unit that wants a ride can board it."""
        w, h = self.size()
        occ = self.occ_now or self.occupied()
        hp = max(t["hp"], 10)
        careful = 20 if t["type"] in self.protect else 4
        tmove = max(self.unit_info(t["type"])["move"], 1)
        here = (t["x"], t["y"])
        cargo = [self.g.unit(i) for i in t["cargo"] if i]
        if cargo:
            cu = dict(cargo[0], army=army)
            cmove = max(self.unit_info(cu["type"])["move"], 1)
            gd = self.goal_distance(cu, self.goals_for(cu, army))
            chart, mt = self.move_chart(army), self.move_type(cu["type"])
            # Turns to the goal when dropped next to cell c (best neighbour).
            def after_drop(c, free_only):
                best = None
                for n in self.nbrs(c):
                    if n in gd and self.step_cost(chart, mt, n) and (not free_only or n not in occ):
                        v = gd[n] / cmove + 1
                        if best is None or v < best[0]:
                            best = (v, n)
                return best
            sources = {}
            for y in range(h):
                for x in range(w):
                    if self.can_unload_at(t, (x, y)):
                        a = after_drop((x, y), False)
                        if a:
                            sources[(x, y)] = int(a[0] * tmove)
            if not sources:
                return None
            gdt = self.goal_distance(t, sources)
            drops = []
            for c in free:
                if self.can_unload_at(t, c):
                    a = after_drop(c, True)
                    if a:
                        drops.append((a[0] + careful * self.threat(t, c, foes) / hp / tmove, c, a[1]))
            moving = min(free, key=lambda c: (gdt.get(c, 1 << 20) / tmove + careful * self.threat(t, c, foes) / hp / tmove, cells[c]))
            move_turns = gdt.get(moving, 1 << 20) / tmove + 1
            if drops:
                v, c, n = min(drops)
                if v <= move_turns:
                    return ("drop", c, n)
            return ("wait", moving, None)
        riders = [m for m in self.friends(army) if m["id"] != t["id"] and self.fits(t, m) and self.wants_ride(m, army)
                  and (t["type"] != 7 or m["type"] in CAPTURERS)]
        if not riders:
            return None
        spots = set()
        for m in riders:
            mc = (m["x"], m["y"])
            land = self.moves(dict(m, army=army))
            spots |= {c for c in land if c != mc and c not in occ and self.can_unload_at(t, c)}
            spots |= {n for c in land for n in self.nbrs(c) if n not in occ and self.can_unload_at(t, n)}
        if here in spots:
            return ("wait", here, None)
        if not spots:
            return None
        gd = self.goal_distance(t, list(spots))
        best = min(free, key=lambda c: (gd.get(c, 999) + careful * self.threat(t, c, foes) / hp, cells[c]))
        return ("wait", best, None)

    def drop(self, target):
        """In the Drop: the first cargo, the cursor to `target` (else the
        cell the game offers), A; a second cargo goes too."""
        g, e = self.g, self.e
        for k in range(2):
            e.wait(20)
            if g.menu():
                e.press("A", 4)
                e.wait(20)
            for _ in range(8):
                c = g.cursor()
                if target is None or c == target:
                    break
                key = "RIGHT" if c[0] < target[0] else "LEFT" if c[0] > target[0] else "DOWN" if c[1] < target[1] else "UP"
                e.press(key, 4)
                e.wait(10)
            e.press("A", 4)
            e.wait(40)
            m = g.menu()
            if not m:
                return
            names = [n.lower() for n in m["names"]]
            if any(n.startswith("drop") for n in names):
                g.choose("Drop", g.ACTION_MENU)
                target = None
                continue
            if any(n.startswith("wait") for n in names):
                g.choose("Wait", g.ACTION_MENU)
            return

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
        elif kind == "drop" and any(n.startswith("drop") for n in names):
            g.choose("Drop", g.ACTION_MENU)
            self.drop(target)
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
        anti-air units; an Infantry while we have fewer than two units that
        capture and there is something to capture). No more than 20 units:
        more only block each other."""
        w, h = self.size()
        occ = self.occupied()
        foes = self.enemies(army)
        mine = [m for m in self.g.units(army=army)]
        air = sum(1 for f in foes if f["type"] in AIR)
        aa = sum(1 for m in mine if m["type"] in ANTI_AIR)
        props = self.properties()
        capt = sum(1 for m in mine if m["type"] in CAPTURERS)
        need_capt = bool(self.goals or self.targets_for_capture(army, props))
        count = len(mine)
        for (x, y), kind, owner in props:
            if owner != army or kind not in BUILD or (x, y) in occ:
                continue
            funds = self.funds(army)
            order = list(self.build_order.get(kind, BUILD[kind]))
            if kind == BASE and need_capt and capt < 2:
                order = [2, 1]
                capt += 1
            elif count >= 20:
                continue
            elif kind == BASE and air > aa:
                order = [14] + order
                aa += 1
            count += 1
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
        self.props = None
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
            self.props = None
            targets = self.targets(army)
            # Units with an attack first (the biggest hit first; indirect
            # ones before direct ones that could block them), then the rest,
            # the protected last.
            def order(u):
                hit = self.rough_attack(u, targets) if u["type"] not in self.protect else 0
                return (u["type"] in self.protect, hit == 0, self.unit_info(u["type"])["min"] <= 1, -hit, self.tiebreak(u))
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
        self.props = None
        try:
            self.build(army)
        except NavError as ex:
            self.log(f"  build: {ex}")
            self.cancel()
        d.wait_control()
        d.end_turn()
