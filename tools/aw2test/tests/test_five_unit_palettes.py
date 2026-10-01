"""Map unit colours in five-army games: every army's units, unmoved and moved
(greyed), drawn in exactly the colours the same army colour has in an
ordinary game.

AW2 draws map units on BG1 with BG palette 11 + army (12..15) and moved units
with BG palette 11, which holds the current army's grey (loaded at each turn
start). Five armies put Black Hole's units on 11 as well, so they took the
current army's grey (darker; red-brown at Orange Star's turn start) and moved
units took Black Hole's colours. tangoAW2 now moves the grey to BG 9 in
five-army games (five/patches.txt).

The references are read from ordinary games, for each colour: a four-army
game (Orange Star, Blue Moon, Green Earth, Yellow Comet; army 1 human, the
rest CPU) and a two-army game with Black Hole as army 1. Each army's unit
palette row, and the moved-unit row whenever moved units are on screen, are
recorded (and checked against the ROM's palette table).

The five-army runs sample every frame from the first turn through End turn
and the CPU turns back to the human's turn: every BG palette the unit layer
uses must hold its army's reference colours (12..15 armies 1..4, 11 Black
Hole, 9 the current army's grey). Human army 1, human Black Hole (army 5),
fog, and a battle scene that returns to the map. Screenshots: a strip of
frames across the end of the turn (Black Hole's units) and the map on each
army's turn."""

import os
import struct
import threading

from aw2test.harness import test

PAL_RAM = 0x05000000
DISPCNT = 0x04000000
BG1CNT = 0x0400000A
MAIN_CALLBACK = 0x03000000
BATTLE_MAIN = 0x08022049
UNIT_PALETTES = 0x0810E6E0  # 0x20 per colour 1..5, then their greys (colour + 5)
COLOURS = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}
GMAP = 0x0201E450
OUTLINE_PULSE = 0x0809139C  # colour 15 of a powered army's units, a step every 4 frames

_refs = {}
_refs_lock = threading.Lock()


def unit_layer(g):
    """The BG palettes BG1 (the map's unit layer) uses now, or None when the
    battle map is not up or BG1 shows something else (the battle scene, a
    power's portrait: palettes outside 9..15)."""
    if g.e.u32(MAIN_CALLBACK) != BATTLE_MAIN:
        return None
    io = g.e.read(DISPCNT, 0x0C)
    if not struct.unpack_from("<H", io, 0)[0] & 0x200:
        return None
    cnt = struct.unpack_from("<H", io, 0x0A)[0]
    ents = struct.unpack("<1024H", g.e.read(0x06000000 + ((cnt >> 8) & 0x1F) * 0x800, 0x800))
    pals = {e >> 12 for e in ents if e & 0x3FF}
    if pals - {9, 11, 12, 13, 14, 15}:
        return None
    return pals


def rows(g):
    b = g.e.read(PAL_RAM, 0x200)
    return [b[i * 32:(i + 1) * 32] for i in range(16)]


def wait_frames(g, n, fn):
    for _ in range(n):
        g.e.wait(1)
        fn()


def until_back(g, human, fn, max_frames=30000, settle=90):
    """Step frame by frame (calling fn) until it is army `human`'s turn again
    and `settle` more frames have passed."""
    n, back = 0, 0
    while n < max_frames:
        g.e.wait(1)
        n += 1
        fn()
        if g.current_army() == human:
            back += 1
            if back >= settle:
                return True
        else:
            back = 0
    return False


def end_turn(g):
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)


# -- references from ordinary games ------------------------------------------------

class TurnClock:
    """The current army and how many frames ago it last changed."""

    def __init__(self, g):
        self.g, self.army, self.prev, self.since = g, g.current_army(), None, 1000

    def tick(self):
        a = self.g.current_army()
        if a != self.army:
            self.prev, self.army, self.since = self.army, a, 0
        else:
            self.since += 1
        return a


# For a frame or two after the current army changes, the last army's moved
# units still show in the moved-unit palette while the next army's grey is
# loaded (ordinary games too); a moved row is taken as the reference only
# once the turn has settled, and the five-army check allows the last army's
# grey for as long after a change.
SETTLE = 8


def record_refs(ctx, g, refs, armies):
    """Unit rows 12.. by army; the moved row (11) whenever moved units show."""
    colours = {a: g.player(a)["colour"] for a in armies}
    r = rows(g)
    for a in armies:
        refs["normal"].setdefault(colours[a], r[11 + a])
    clock = TurnClock(g)

    def sample():
        a = clock.tick()
        pals = unit_layer(g)
        if pals and 11 in pals and a in colours:
            v = rows(g)[11]
            if clock.since >= SETTLE:
                seen = refs["moved_seen"].setdefault(colours[a], {})
                seen[v] = seen.get(v, 0) + 1
            else:
                refs["transition"].append((clock.since, colours.get(clock.prev), colours[a], v))
    return colours, sample


def reference_games(ctx):
    refs = {"normal": {}, "moved": {}, "moved_seen": {}, "transition": []}
    # Four armies, colours 1..4; the CPUs fight next to each other.
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    place_armies(m, 4)
    g = ctx.start(m, None, humans=(1,))
    colours, sample = record_refs(ctx, g, refs, (1, 2, 3, 4))
    ctx.eq([colours[a] for a in (1, 2, 3, 4)], [1, 2, 3, 4], "four-army reference: colours")
    g.select(2, 3)
    g.move_to(2, 2)
    g.choose("Wait", g.ACTION_MENU)
    g.wait_for_input()
    sample()
    end_turn(g)
    ctx.check(until_back(g, 1, sample), "four-army reference: the turn comes back to army 1")
    g.e.close()
    # Black Hole as army 1 of a two-army game.
    m = ctx.map()
    m.colours = [0, 5, 2, 3, 4]
    m.unit(1, "tank", 2, 3).unit(1, "tank", 3, 3).unit(2, "tank", 4, 3)
    g = ctx.start(m, ["andy", "max"])
    colours, sample = record_refs(ctx, g, refs, (1,))
    ctx.eq(colours[1], 5, "two-army reference: army 1 is Black Hole")
    g.select(2, 3)
    g.move_to(2, 2)
    g.choose("Wait", g.ACTION_MENU)
    g.wait_for_input()
    sample()
    for c, seen in refs["moved_seen"].items():
        ctx.check(len(seen) == 1, f"reference {COLOURS[c]}: one moved row once the turn settles: "
                                  f"{[(v.hex()[-8:], n) for v, n in seen.items()]}")
        refs["moved"][c] = max(seen, key=seen.get)
    for since, prev, cur, v in refs["transition"]:
        what = [k for k, w in refs["moved"].items() if w == v]
        ctx.log(f"reference transition: {since} frames after {COLOURS.get(prev)} -> {COLOURS.get(cur)}, "
                f"the moved row is {[COLOURS[k] for k in what]}'s grey")
    # The rows are the ROM's palettes (colour c, and its grey at c + 5);
    # colour 15 (the outline) is black (sub_08024720 sets it every frame).
    for c in range(1, 6):
        ctx.check(c in refs["normal"] and c in refs["moved"], f"reference: {COLOURS[c]} normal and moved recorded")
        if c in refs["normal"] and c in refs["moved"]:
            rom = g.e.read(UNIT_PALETTES + (c - 1) * 32, 30) + bytes(2)
            ctx.eq(refs["normal"][c].hex(), rom.hex(), f"reference {COLOURS[c]}: the ROM's unit palette")
            ctx.eq(refs["moved"][c].hex(), g.e.read(UNIT_PALETTES + (c + 4) * 32, 32).hex(),
                   f"reference {COLOURS[c]}: the ROM's moved (grey) palette")
    g.e.close()
    return refs


def references(ctx):
    with _refs_lock:
        if ctx.mode not in _refs:
            _refs[ctx.mode] = reference_games(ctx)
        else:
            ctx.log(f"references for {ctx.mode} from an earlier test")
        return _refs[ctx.mode]


# -- five armies -------------------------------------------------------------------

def place_armies(m, n):
    """Two Tanks and an Infantry an army in columns 2.., rows 3..5: neighbours,
    so the CPUs attack and every army's units move."""
    for a in range(1, n + 1):
        m.unit(a, "tank", 1 + a, 3).unit(a, "tank", 1 + a, 5).unit(a, "infantry", 1 + a, 4)


def five_map(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 15, 0x1B4)
    m.colours = [5, 1, 2, 3, 4]
    place_armies(m, 5)
    return m


class Checker:
    """Every frame: each BG palette the unit layer uses holds its reference."""

    def __init__(self, ctx, g, refs):
        self.ctx, self.g, self.refs = ctx, g, refs
        self.colours = {a: g.player(a)["colour"] for a in range(1, 6)}
        self.frames = 0
        self.clock = TurnClock(g)
        self.bad = {}
        self.seen = set()
        self.powered = set()
        self.power9 = 0
        self.pulse = set(struct.unpack("<16H", g.e.read(OUTLINE_PULSE, 32)))
        ctx.eq([self.colours[a] for a in range(1, 6)], [1, 2, 3, 4, 5], "five armies: colours")

    def want(self, p):
        if 12 <= p <= 15:
            return "normal", self.colours[p - 11]
        if p == 11:
            return "normal", self.colours[5]
        if p == 9:
            return "moved", self.colours.get(self.g.current_army())
        return None, None

    def __call__(self, label=""):
        g = self.g
        self.clock.tick()
        pals = unit_layer(g)
        if not pals:
            return
        self.frames += 1
        r = rows(g)
        players = g.e.read(g.players_base, 6 * 0x3C)
        for p in sorted(pals):
            kind, c = self.want(p)
            self.seen.add((p, kind, c))
            if kind is None or c is None:
                continue
            want = self.refs[kind].get(c)
            if kind == "normal":
                # Colour 15, the outline, pulses while the army's CO power
                # is on (sub_08024720, every frame); black otherwise.
                army = 5 if p == 11 else p - 11
                outline = struct.unpack_from("<H", r[p], 30)[0]
                if players[0x3C * army + 0x1E] or (self.clock.since < SETTLE and outline in self.pulse):
                    # (the power ends at the army's turn start; the pulse
                    # stops on the next frame, in ordinary games too)
                    self.powered.add(c)
                    ok = r[p][:30] == want[:30] and outline in self.pulse
                else:
                    ok = r[p] == want
            else:
                ok = r[p] == want
            if not ok and p == 9 and self.clock.since < SETTLE and self.clock.prev:
                ok = r[p] == self.refs["moved"].get(self.colours[self.clock.prev])
            if not ok and p == 9 and label == "power":
                # The CO power portrait's colours go in BG 9 from just before
                # it covers the map until the game puts the grey back after
                # it (sub_0801A57C): moved units show them under the wipe.
                self.power9 += 1
                continue
            if not ok:
                key = (p, kind, c)
                if key not in self.bad:
                    self.ctx.log(f"frame {g.e.frame} [{label}] army {g.current_army()}: BG palette {p} "
                                 f"({COLOURS.get(c)} {kind}) holds {r[p].hex()}, want {self.refs[kind].get(c, b'').hex()}")
                    self.ctx.shot(g, f"wrong_{p}_{kind}_{c}")
                self.bad[key] = self.bad.get(key, 0) + 1

    def report(self, what):
        self.ctx.log(f"{what}: {self.frames} frames checked; palettes seen {sorted(self.seen, key=str)}")
        self.ctx.check(self.frames > 0, f"{what}: the unit layer was checked")
        for (p, kind, c), n in sorted(self.bad.items(), key=str):
            self.ctx.check(False, f"{what}: BG palette {p} not {COLOURS.get(c)}'s {kind} colours on {n} frames")
        if not self.bad:
            self.ctx.check(True, f"{what}: every unit palette right on every frame")
        moved = {c for p, kind, c in self.seen if kind == "moved"}
        return moved


def strip(ctx, g, name, frames, every, chk):
    """A screenshot every `every` frames for `frames` frames (the map camera
    logged with each), checking every frame."""
    for k in range(frames):
        if k % every == 0:
            ctx.shot(g, f"{name}_{k:04}")
            ctx.log(f"{name}_{k:04}: frame {g.e.frame} army {g.current_army()} camera {g.e.u16(GMAP + 4)},{g.e.u16(GMAP + 6)}")
        g.e.wait(1)
        chk(name)


def run_five(ctx, human, fog=False, visuals="off", battle=False):
    refs = references(ctx)
    m = five_map(ctx)
    g = ctx.start(m, None, humans=(human,), fog=fog, visuals=visuals)
    chk = Checker(ctx, g, refs)
    wait_frames(g, 30, lambda: chk("start"))
    ctx.shot(g, "start")
    x = 1 + human
    if battle:
        # A Tank attacks its neighbour with the battle scene on.
        g.select(x, 3)
        g.move_to(x, 3)
        g.choose("Fire", g.ACTION_MENU)
        g.pick_target(x + 1 if human < 5 else x - 1, 3)
        wait_frames(g, 1200, lambda: chk("battle"))
        g.wait_for_input()
        ctx.shot(g, "after_battle")
    g.select(x, 5)
    g.move_to(x, 6)
    g.choose("Wait", g.ACTION_MENU)
    wait_frames(g, 30, lambda: chk("moved"))
    g.wait_for_input()
    ctx.shot(g, "moved")
    end_turn(g)
    strip(ctx, g, "end", 240, 6, chk)
    ctx.check(until_back(g, human, lambda: chk("cpu turns")), "the turn comes back to the human")
    g.wait_for_input()
    wait_frames(g, 30, lambda: chk("back"))
    ctx.shot(g, "back")
    moved = chk.report("five armies")
    ctx.check(len(moved) >= 3, f"moved units were on screen on several armies' turns: {sorted(moved)}")
    return g


@test(modes=("aw2", "ds"))
def five_unit_palettes_human_os(ctx):
    run_five(ctx, 1)


@test(modes=("aw2", "ds"))
def five_unit_palettes_human_bh(ctx):
    run_five(ctx, 5)


@test(modes=("aw2", "ds"))
def five_unit_palettes_fog(ctx):
    run_five(ctx, 1, fog=True)


@test(modes=("aw2", "ds"))
def five_unit_palettes_battle_scene(ctx):
    run_five(ctx, 1, visuals="a", battle=True)


@test(modes=("aw2", "ds"))
def five_unit_palettes_powers(ctx):
    """Orange Star (human) moves a unit, then fires its CO Power; Black Hole
    (CPU) has a full meter for its turn."""
    refs = references(ctx)
    g = ctx.start(five_map(ctx), None, humans=(1,))
    chk = Checker(ctx, g, refs)
    g.select(2, 5)
    g.move_to(2, 6)
    g.choose("Wait", g.ACTION_MENU)
    wait_frames(g, 30, lambda: chk("moved"))
    g.wait_for_input()
    g.charge_power(1, "power")
    g.charge_power(5, "super")
    g.open_map_menu()
    g.choose("Power", g.MAP_MENU)
    strip(ctx, g, "power", 900, 6, chk)
    g.wait_for_input()
    wait_frames(g, 30, lambda: chk("after power"))
    ctx.shot(g, "after_power")
    ctx.check(g.player(1)["co_mode"] == 1, "Orange Star's power is on")
    end_turn(g)
    bh_power = []

    def watch():
        chk("cpu turns")
        if g.current_army() == 5 and g.player(5)["co_mode"] and not bh_power:
            bh_power.append(g.e.frame)
            ctx.shot(g, "bh_power")
    ctx.check(until_back(g, 1, watch), "the turn comes back to army 1")
    ctx.check(bool(bh_power), f"Black Hole (CPU) fired its power: {bh_power}")
    chk.report("five armies with powers")
    ctx.log(f"the moved units' grey gave way to the power portrait's colours on {chk.power9} frames")
    ctx.check(chk.power9 < 300, f"only while the power portrait runs: {chk.power9} frames")
    ctx.check({1, 5} <= chk.powered, f"powered armies' outlines checked: {sorted(chk.powered)}")
