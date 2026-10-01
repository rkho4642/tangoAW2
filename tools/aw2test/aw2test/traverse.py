"""Can every army get everywhere it has to? A map checked cell by cell with the
game's own movement costs (the movement chart: a row per movement type, a
cost per terrain class, 0xFF impassable).

Terrain classes: the low 5 bits of a map cell's class (the owner in the top
3). Movement types: 0 foot, 1 mech, 2 treads, 3 tires, 4 air, 5 sea (ships),
6 lander (and the Black Boat), 7 pipe (Piperunner).

What is checked (`check` returns the failures, in words):
(a) foot, treads and tires reach every enemy HQ and every Com Tower from
    their HQ and bases, overland or by Lander: a unit walks onto a beach or a
    port, a Lander carries it through water it can sail to any other beach
    or port, and it steps off onto a cell next to that one; foot soldiers
    reach every property the same way;
(b) ships reach every enemy port from their own ports;
(c) a Piperunner built at a base on a pipe (or pre-deployed on a pipe) can
    travel at least three cells and has a property, tower or HQ it does not
    own, or its own HQ to guard, within range (2-5) of one of them;
(d) nothing is boxed in: every base lets foot and treads out, every port
    lets ships out;
(e) a base that touches the pipes touches them only through intact pipe
    seams, so breaking the seam cuts its Piperunner production.
"""

FOOT, MECH, TREADS, TIRES, AIR, SEA, LANDER, PIPE = range(8)
CITY, HQ, AIRPORT, PORT, SHOAL, BASE, PIPE_T, SEAM, LAB = 6, 8, 10, 11, 13, 14, 15, 16, 20
PROPERTIES = (CITY, HQ, AIRPORT, PORT, BASE, LAB)
NEXT = ((0, -1), (1, 0), (0, 1), (-1, 0))

# The Piperunner's row (Dual Strike's, as tangoAW2 adds it): base, pipe, seam.
PIPE_ROW = [0xFF] * 32
for _c in (BASE, PIPE_T, SEAM):
    PIPE_ROW[_c] = 1


def aw2_chart(rom, clear=0x085D511C):
    """The seven AW2 movement rows from the ROM file, and the pipe row."""
    o = clear - 0x08000000
    rows = [list(rom[o + 32 * r:o + 32 * r + 32]) for r in range(7)]
    return rows + [PIPE_ROW]


class Grid:
    def __init__(self, w, h, classes):
        self.w, self.h = w, h
        self.c = classes  # row by row

    def at(self, x, y):
        return self.c[y * self.w + x]

    def kind(self, x, y):
        return self.at(x, y) & 0x1F

    def owner(self, x, y):
        return self.at(x, y) >> 5

    def cells(self):
        for y in range(self.h):
            for x in range(self.w):
                yield x, y

    def near(self, x, y):
        for dx, dy in NEXT:
            if 0 <= x + dx < self.w and 0 <= y + dy < self.h:
                yield x + dx, y + dy


def flood(g, row, starts):
    """Cells a unit with movement row `row` can reach from `starts`."""
    seen = {s for s in starts if row[g.kind(*s)] != 0xFF}
    todo = list(seen)
    while todo:
        c = todo.pop()
        for n in g.near(*c):
            if n not in seen and row[g.kind(*n)] != 0xFF:
                seen.add(n)
                todo.append(n)
    return seen


def ground_reach(g, chart, mtype, starts):
    """Overland, then by Lander from beach or port to beach or port."""
    row, lander = chart[mtype], chart[LANDER]
    landing = {c for c in g.cells() if g.kind(*c) in (SHOAL, PORT) and row[g.kind(*c)] != 0xFF}
    reach = flood(g, row, starts)
    sailed = set()
    while True:
        boards = (landing & reach) - sailed
        if not boards:
            return reach
        new = set()
        for b in boards:
            if b in sailed:
                continue
            ends = flood(g, lander, [b]) & landing
            sailed |= ends | {b}
            for l in ends:
                new |= {l} | {n for n in g.near(*l) if row[g.kind(*n)] != 0xFF}
        reach = flood(g, row, reach | new)


def check(g, chart, armies, piperunners=(), need_piperunners=True):
    """`armies`: the army numbers; `piperunners`: (army, x, y) pre-deployed;
    `need_piperunners`: every army must have a Piperunner base (the Dual
    Strike maps). Returns a list of failures."""
    bad = []
    cells = list(g.cells())

    def of(kind, army=None):
        return [c for c in cells if g.kind(*c) == kind and (army is None or g.owner(*c) == army)]

    towers = of(LAB)
    for a in armies:
        home = of(HQ, a) + of(BASE, a)
        if not of(HQ, a):
            bad.append(f"army {a} has no HQ")
            continue
        enemies_hq = [c for c in of(HQ) if g.owner(*c) != a]
        for mtype, name in ((FOOT, "foot"), (TREADS, "treads"), (TIRES, "tires")):
            reach = ground_reach(g, chart, mtype, home)
            for t in enemies_hq + towers:
                if t not in reach:
                    bad.append(f"army {a}: {name} cannot reach {'tower' if t in towers else 'HQ'} {t}")
            if mtype == FOOT:
                for t in cells:
                    if g.kind(*t) in PROPERTIES and t not in reach:
                        bad.append(f"army {a}: foot cannot reach property {t}")
        ports = of(PORT, a)
        for p in ports:
            sea = flood(g, chart[SEA], [p])
            for q in of(PORT):
                if g.owner(*q) not in (a, 0) and q not in sea:
                    bad.append(f"army {a}: ships from port {p} cannot reach port {q}")
            if not any(chart[SEA][g.kind(*n)] != 0xFF for n in g.near(*p)):
                bad.append(f"army {a}: port {p} is boxed in")
        for b in of(BASE, a):
            for mtype, name in ((FOOT, "foot"), (TREADS, "treads")):
                if not any(chart[mtype][g.kind(*n)] != 0xFF for n in g.near(*b)):
                    bad.append(f"army {a}: base {b} boxes {name} in")
            pipes = [n for n in g.near(*b) if g.kind(*n) in (PIPE_T, SEAM)]
            if any(g.kind(*n) != SEAM for n in pipes):
                bad.append(f"army {a}: base {b} touches a pipe not through a seam")
        starts = [b for b in of(BASE, a) if any(g.kind(*n) in (PIPE_T, SEAM) for n in g.near(*b))]
        starts += [(x, y) for (pa, x, y) in piperunners if pa == a]
        if not starts and need_piperunners:
            bad.append(f"army {a}: no Piperunner base")
        # Something to shell (a property, tower or HQ not its own) or its
        # own HQ's doorstep to guard.
        targets = [c for c in cells if (g.kind(*c) in PROPERTIES and g.owner(*c) != a)] + of(HQ, a)
        for s in starts:
            run = flood(g, chart[PIPE], [s])
            ok = any(2 <= abs(t[0] - r[0]) + abs(t[1] - r[1]) <= 5 for r in run for t in targets)
            if len(run) < 4 or not ok:
                bad.append(f"army {a}: a Piperunner from {s} has nothing to do ({len(run)} cells)")
    return bad


def beach_balance(g):
    """Per army: (its beaches, the distance from its HQ to the nearest beach
    of another army). A beach is an army's when that army's HQ is strictly
    the nearest to it (Manhattan); beaches as near to two HQs are shared."""
    hqs = {g.owner(*c): c for c in g.cells() if g.kind(*c) == HQ}
    dist = lambda a, b: abs(a[0] - b[0]) + abs(a[1] - b[1])
    mine = {a: [] for a in hqs}
    for c in g.cells():
        if g.kind(*c) != SHOAL:
            continue
        d = sorted((dist(c, h), a) for a, h in hqs.items())
        if len(d) == 1 or d[0][0] < d[1][0]:
            mine[d[0][1]].append(c)
    out = {}
    for a, h in hqs.items():
        theirs = [c for b, cs in mine.items() if b != a for c in cs]
        out[a] = (len(mine[a]), min((dist(h, c) for c in theirs), default=None))
    return out


def symmetric_armies(armies):
    """The armies whose beaches must match: every army but Black Hole in the
    middle of a 5P map (a 3P map's army 3, on the mirror line, is matched by
    hand to the other two)."""
    return {2: [1, 2], 3: [1, 2, 3], 4: [1, 2, 3, 4], 5: [1, 2, 3, 4]}[armies]
