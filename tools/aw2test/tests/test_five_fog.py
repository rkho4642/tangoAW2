"""Fog of war on the 5P maps, with the Dual Strike pack: Sonja as Yellow
Comet (army 4) and Von Bolt as Black Hole (army 5), Orange Star human, the
rest computer, battle animations on. Black Monolith (AW2's look), Black
Wastes (Wasteland: Dual Strike's fog colours) and Coral Crown (AW2's look,
sea). Three whole days are played (army 1 by aw2test.bot; Sonja's and Von
Bolt's meters are filled so the computer fires their powers, Ex Machina
under fog included), and every few frames while the battle map is up the
screen is checked against the displayed vision plane (gMap +0x234A, a
viewer count per cell):

- the fog flag stays on;
- the terrain layer (BG3): every quadrant of a seen cell in BG palettes 0-3,
  of a fogged cell in 4-7 (their fogged copies), and palettes 4-7 are the
  look's fog colours (AW2's own, as an ordinary two-army fog game has
  them; Wasteland's from its colour set, crate::ds_look); on the Wasteland
  map the view is compared cell by cell with Dual Strike's own drawing,
  fog included (aw2test.looks), at each turn start;
- units (BG1): no unit of another army is drawn on a fogged cell;
- buildings (OBJ, 16x32 at priority 3): a fogged property in the neutral
  palette 8, a seen one in 8 + its owner's colour (Black Hole 13).

A problem counts once it lasts three samples in a row (the layers follow a
move or a scroll a frame or two later). Screenshots: every army's turn
start, power activations, battle scenes, mid-turn views, a stitched map of
the fog on day 2, and at the end Sonja's units next to Orange Star's,
damaged (her HP hidden from her enemies) beside a Blue Moon unit."""

import importlib.util
import os
import struct

from aw2test import harness, looks
from aw2test import rom as romlib
from aw2test.bot import Bot
from aw2test.dscampaign import DsCampaign
from aw2test.game import NavError
from aw2test.harness import Skip, test
from aw2test.stitch import stitch

# test_ds_maps' helpers (select a map from its tab like a player), without
# registering its tests a second time.
_n = len(harness.TESTS)
_spec = importlib.util.spec_from_file_location("ds_maps_helpers", os.path.join(os.path.dirname(__file__), "test_ds_maps.py"))
dm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(dm)
del harness.TESTS[_n:]

GMAP = 0x0201E450
SEEN = GMAP + 0x234A          # the vision plane drawn (viewers per cell)
SEEN_ARMY = GMAP + 0x1E42
ROWS = GMAP + 0x417A
CLASSES = GMAP + 0x1432
MAIN_CALLBACK = 0x03000000
BATTLE_MAIN = 0x08022049
FOG = 0x03003FCD
PAL_BUFFER = 0x030020C0
DISPCNT = 0x04000000
BG1CNT, BG3CNT = 0x0400000A, 0x0400000E
OAM = 0x07000000
BGOFS = 0x04000010             # BG0..3 HOFS, VOFS (mGBA reads them back)
DAY = 0x03004080
INVENTIONS = 0x02028360
# Property kinds (class & 0x1F): HQ, city, airport, port, base, Com Tower.
PROPERTIES = {8, 6, 0xA, 0xB, 0xE, 0x14}
COS = ["andy", "grit", "drake", "sonja", "von bolt"]
NAMES = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}
STREAK = 3
FOG_MAX_ERROR = 6.5
# AW2TEST_FOG_STATES=1: save the emulator's state at the first problems (to look into them).
STATES = bool(os.environ.get("AW2TEST_FOG_STATES"))


def reference_fog_rows(ctx):
    """BG palettes 0-7 of an ordinary two-army fog game (AW2's look)."""
    m = ctx.map()
    m.unit(1, "tank", 3, 3).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["andy", "andy"], fog=True)
    rows = g.e.read(PAL_BUFFER, 0x100)
    g.e.close()
    return rows


def invention_cells(e):
    """The cells of every invention's footprint (the list AW2's vision
    refresh walks, 0x02028360: x, y, and its size in the halfword at +2)."""
    raw = e.read(INVENTIONS, 0x80)
    out = set()
    for i in range(16):
        x0, y0, word = raw[8 * i], raw[8 * i + 1], struct.unpack_from("<H", raw, 8 * i + 2)[0]
        if not word & 0x3C0:
            break
        out |= {(x0 + dx, y0 + dy) for dx in range(word & 7) for dy in range((word >> 3) & 7)}
    return out


class Watch:
    def __init__(self, ctx, g, name, wasteland, ref_rows):
        self.ctx, self.g, self.e, self.name = ctx, g, g.e, name
        self.wasteland = wasteland
        self.ref_rows = ref_rows
        e = self.e
        self.w, self.h = e.u16(GMAP), e.u16(GMAP + 2)
        self.rows = [e.u16(ROWS + 2 * y) for y in range(self.h)]
        self.span = self.rows[-1] + self.w
        self.colours = {a: g.player(a)["colour"] for a in range(1, 6)}
        self.streak = {}
        self.fails = {}
        self.samples = 0
        self.stats = {}
        self.plane_diff = 0
        self.fog_off = 0
        self.saved, self.last_saved = 0, -1000

    def stat(self, k):
        self.stats[k] = self.stats.get(k, 0) + 1

    def sample(self):
        """One check of the screen; returns why it was skipped, or None."""
        e, g = self.e, self.g
        if e.u8(FOG) != 1:
            self.fog_off += 1
        if e.u32(MAIN_CALLBACK) != BATTLE_MAIN:
            return "scene"
        io = e.read(DISPCNT, 0x10)
        dispcnt = struct.unpack_from("<H", io, 0)[0]
        if dispcnt & 0x0A00 != 0x0A00:
            return "layers"
        camx, camy = e.s16(GMAP + 4), e.s16(GMAP + 6)
        if camx % 16 or camy % 16:
            return "scroll"
        cx, cy = e.s16(GMAP + 0xC), e.s16(GMAP + 0xE)
        # BG1's and BG3's scroll as the camera has it (not mid-scroll, not
        # shaking: a hit, a unit destroyed, a power's blast).
        sx, sy = camx // 16, camy // 16
        scroll = struct.unpack("<8H", e.read(BGOFS, 16))
        want = ((16 * ((sx - cx) & 15)) & 0xFF, (16 * ((sy - cy) & 15)) & 0xFF)
        # (BG1, the units, sits 3 pixels higher.)
        if (scroll[6] & 0xFF, scroll[7] & 0xFF) != want or (scroll[2] & 0xFF, (scroll[3] - 3) & 0xFF) != want:
            self.stat(("skipped", "shake"))
            return "shake"
        bg1 = struct.unpack("<1024H", e.read(0x06000000 + ((struct.unpack_from("<H", io, 0x0A)[0] >> 8) & 0x1F) * 0x800, 0x800))
        if {v >> 12 for v in bg1 if v & 0x3FF} - {9, 11, 12, 13, 14, 15}:
            return "bg1"  # the banner or a portrait over the map
        bg3 = struct.unpack("<1024H", e.read(0x06000000 + ((struct.unpack_from("<H", io, 0x0E)[0] >> 8) & 0x1F) * 0x800, 0x800))
        raw = e.read(SEEN, self.span)
        raw_army = e.read(SEEN_ARMY, self.span)
        if raw != raw_army:
            self.plane_diff += 1
        classes = e.read(CLASSES, self.span)
        seen = lambda x, y: raw[self.rows[y] + x]
        view = [(x, y) for y in range(max(0, sy), min(sy + 10, self.h)) for x in range(max(0, sx), min(sx + 15, self.w))]
        quads = lambda tm, x, y: [tm[(((y - cy) & 15) * 2 + dy) * 32 + ((x - cx) & 15) * 2 + dx] for dy in (0, 1) for dx in (0, 1)]
        self.samples += 1
        problems = {}
        footprints = invention_cells(e)
        # Terrain: seen cells in palettes 0-3, fogged cells in 4-7.
        for x, y in view:
            pals = {q >> 12 for q in quads(bg3, x, y)}
            fog = not seen(x, y)
            if (fog and min(pals) < 4) or (not fog and max(pals) >= 4):
                if (x, y) in footprints:
                    # AW2's own: the full vision refresh marks every
                    # invention's footprint seen, the computer's refresh
                    # does not, and a row the camera scrolls in is drawn
                    # with the plane of the moment (vanilla AW2 too).
                    self.stat(("invention footprint drawn", "seen" if max(pals) < 4 else "fogged", "plane", int(not fog)))
                    continue
                problems[("terrain", x, y)] = f"cell {(x, y)} {'fogged' if fog else 'seen'}, BG3 palettes {sorted(pals)}"
        # The fog colours.
        rows = e.read(PAL_BUFFER, 0x100)
        if rows[0x80:] != self.ref_rows[0x80:]:
            bad = [p for p in range(4, 8) if rows[32 * p:32 * p + 32] != self.ref_rows[32 * p:32 * p + 32]]
            problems[("fog_colours",)] = f"BG palettes {bad} not the look's fog colours"
        # Units: none of another army drawn on a fogged cell.
        inview = set(view)
        for u in g.units():
            c = (u["x"], u["y"])
            if c not in inview or u["army"] == 1:
                continue
            drawn = any(q & 0x3FF for q in quads(bg1, *c))
            if not seen(*c):
                self.stat(("unit in fog", "drawn" if drawn else "hidden"))
                if drawn:
                    problems[("unit", u["id"], c)] = f"{NAMES[u['army']]} unit type {u['type']} at {c} drawn in fog"
            else:
                self.stat(("unit seen", "drawn" if drawn else "not drawn"))
        # Buildings.
        oam = e.read(OAM, 0x400)
        for i in range(128):
            a0, a1, a2 = struct.unpack_from("<HHH", oam, 8 * i)
            if (a0 >> 8) & 3 == 2 or a0 >> 14 != 2 or a1 >> 14 != 2 or (a2 >> 10) & 3 != 3:
                continue
            pal = a2 >> 12
            if not 8 <= pal <= 13:
                continue
            ox, oy = a1 & 0x1FF, a0 & 0xFF
            if ox >= 240 or oy >= 160:
                continue
            x, y = (ox + camx) // 16, (oy + 16 + camy) // 16
            if not (0 <= x < self.w and 0 <= y < self.h):
                continue
            cls = classes[self.rows[y] + x]
            if cls & 0x1F not in PROPERTIES:
                self.stat(("sprite on", hex(cls & 0x1F), pal))
                continue
            owner = cls >> 5
            fog = not seen(x, y)
            want = 8 if fog or owner == 0 else 8 + self.colours[owner]
            self.stat(("building", "fog" if fog else "seen", owner, pal))
            if pal != want:
                problems[("building", x, y)] = f"property at {(x, y)} owner {owner} {'fogged' if fog else 'seen'}: OBJ palette {pal}, want {want}"
        for k, why in problems.items():
            self.streak[k] = self.streak.get(k, 0) + 1
            if self.streak[k] >= STREAK and k not in self.fails:
                self.fails[k] = (e.frame, g.current_army(), why)
                self.ctx.log(f"frame {e.frame} army {g.current_army()}: {why}")
                self.ctx.shot(g, f"problem_{len(self.fails):02d}")
                self.save_state("problem")
        for k in list(self.streak):
            if k not in problems:
                del self.streak[k]
        return None

    def save_state(self, what):
        if STATES and self.saved < 12 and self.e.frame - self.last_saved > 60:
            self.saved += 1
            self.last_saved = self.e.frame
            self.e.cmd(f"statefile {os.path.join(self.ctx.out, f'{what}_state_{self.saved:02d}_{self.e.frame}.state')}")

    def terrain_vs_ds(self, label):
        if self.wasteland:
            # (a fogged composite, a treetop over a river cell, comes out at
            # 6.1 against the reduction's 6.0: the fog colours of folded
            # colours)
            r = looks.terrain_cells(self.e, looks.WASTELAND, max_err=FOG_MAX_ERROR)
            if r is None:
                self.ctx.log(f"{label}: colours not the look's (not compared)")
                return
            checked, bad, fogged, _ = r
            footprints = invention_cells(self.e)
            skipped = [b for b in bad if b[0] in footprints]
            if skipped:
                self.ctx.log(f"{self.name} {label}: invention footprint cells drawn in the other fog state (AW2's own): {skipped}")
            bad = [b for b in bad if b[0] not in footprints]
            if bad:
                self.save_state("vs_ds")
            self.ctx.check(checked and not bad, f"{self.name} {label}: {len(checked)} cells ({fogged} fogged) drawn as Dual "
                                                f"Strike draws them, fog included ({len(bad)} differ: {bad[:6]})")

    def report(self):
        name = self.name
        self.ctx.log(f"{name}: {self.samples} samples; vision planes differed on {self.plane_diff}; stats "
                     f"{sorted(self.stats.items(), key=str)}")
        self.ctx.check(self.samples > 200, f"{name}: the screen was checked ({self.samples} samples)")
        self.ctx.eq(self.fog_off, 0, f"{name}: fog stayed on (samples with it off)")
        kinds = {}
        for k, (frame, army, why) in self.fails.items():
            kinds.setdefault(k[0], []).append(f"frame {frame} army {army}: {why}")
        for kind in ("terrain", "fog_colours", "unit", "building"):
            got = kinds.get(kind, [])
            self.ctx.check(not got, f"{name}: {kind} right on every sample ({len(got)} problems: {got[:4]})")
        self.ctx.check(self.stats.get(("building", "fog", 5, 8), 0) + self.stats.get(("building", "seen", 5, 13), 0) > 0,
                       f"{name}: Black Hole's properties were on screen")


def concealed(path):
    """The picture tango-backend-mgba's `concealed` shows a seat that may
    not watch (dark, a faint band): the shot's pixels nearly all its two
    colours."""
    with open(path, "rb") as f:
        px = f.read()[54:]
    step = 3 * 7
    dark = sum(1 for i in range(0, len(px) - 2, step) if px[i:i + 3] in (bytes((20, 8, 8)), bytes((52, 40, 40))))
    return dark > 0.95 * (len(px) // step)


class Shots:
    def __init__(self, ctx, g, limit=260):
        self.ctx, self.g, self.n, self.limit = ctx, g, 0, limit
        self.dark = []

    def __call__(self, what):
        if self.n >= self.limit:
            return
        self.n += 1
        e = self.g.e
        name = f"s{self.n:03d}_d{e.u16(DAY)}_a{self.g.current_army()}_{what}"
        if concealed(self.ctx.shot(self.g, name)):
            self.dark.append(name)


def cpu_turns(ctx, g, watch, shots, day):
    """From army 1's End to its next turn: every 2 frames a check, shots at
    each army's turn start, power and battle scene, and every 300 frames.
    The current army (0x030033EC) is the turn's once it holds for 24
    frames: at a computer's turn start the game runs it through the armies,
    and between two computers' turns it reads 1 for a few frames."""
    e = g.e
    army = g.current_army()
    cand, held = army, 0
    modes = {a: g.player(a)["co_mode"] for a in range(1, 6)}
    in_scene, scenes, n = 0, 0, 0
    pending = []
    while n < 80000:
        e.wait(2)
        n += 2
        a = g.current_army()
        if a == cand:
            held += 2
        else:
            cand, held = a, 0
        if held >= 24 and cand != army:
            army = cand
            if army == 1:
                break
            pending += [(n + 2, "turn_start"), (n + 40, "banner"), (n + 120, "turn")]
            if STATES:
                e.cmd(f"statefile {os.path.join(ctx.out, f'turn_d{day}_a{army}_{e.frame}.state')}")
            if army in (2, 4, 5) or day > 1:
                watch.terrain_vs_ds(f"day {day} army {army}'s turn")
        for a2 in range(1, 6):
            m = g.player(a2)["co_mode"]
            if m != modes[a2]:
                modes[a2] = m
                if m:
                    ctx.log(f"day {day}: {NAMES[a2]} ({COS[a2 - 1]}) power mode {m} at frame {e.frame}")
                    pending += [(n + k, f"power{m}_{COS[a2 - 1].replace(' ', '')}") for k in (2, 20, 50, 90, 140, 200, 280)]
        scene = e.u32(MAIN_CALLBACK) != BATTLE_MAIN
        if scene:
            in_scene += 2
            if in_scene in (20, 80, 140) and scenes < 12:
                shots(f"scene{scenes}")
        elif in_scene:
            in_scene, scenes = 0, scenes + 1
        if n % 300 == 0:
            pending.append((n, "mid"))
        for t, what in [p for p in pending if p[0] <= n]:
            pending.remove((t, what))
            shots(what)
        watch.sample()
    ctx.require(army == 1, f"day {day}: back to Orange Star's turn")
    g.wait_for_input()
    shots("army1_turn")


def play(ctx, name, wasteland):
    if not ctx.ds:
        raise Skip("the pack's 5P maps and COs")
    ref_rows = reference_fog_rows(ctx)
    g, found = dm.select_map(ctx, name)
    ctx.require(found, f"{name} is on the 5P tab")
    dm.to_teams(g)
    e = g.e
    g.set_teams(COS, {1})
    e.wait(60)
    ctx.shot(g, "teams")
    g.teams_to_rules()
    g.set_rules(fog=True, weather="clear", power=True, visuals="a")
    g.start_battle()
    g.wait_for_input()
    for a in range(1, 6):
        p = g.player(a)
        ctx.eq((p["colour"], p["co"]), (a, romlib.co_id(COS[a - 1])), f"army {a}: {NAMES[a]}, {COS[a - 1]}")
    st = g.playst()
    ctx.eq((st["fog"], st["anim"]), (1, 1), "fog on, battle animations on")
    if wasteland:
        ref = looks.Look(e, looks.WASTELAND)
        ref_rows = ref.clear[:256]
    else:
        ctx.check(e.read(PAL_BUFFER, 0x80) == ref_rows[:0x80], f"{name}: AW2's map colours")
    watch = Watch(ctx, g, name, wasteland, ref_rows)
    shots = Shots(ctx, g)
    d = DsCampaign(g)
    bot = Bot(d, stance="attack", nearest=True, log=ctx.log)
    watch.terrain_vs_ds("day 1")
    for day in (1, 2, 3):
        shots("army1_start")
        watch.sample()
        if day == 2:
            stitch(ctx, g, f"{name} day 2 (Orange Star's vision)", watch.w, watch.h, each=watch.sample)
        # The computer fires Sonja's and Von Bolt's powers as soon as it can.
        g.charge_power(4, "super" if day != 2 else "power")
        g.charge_power(5, "super")
        try:
            bot.play_turn(1)
        except NavError as ex:
            ctx.log(f"day {day}: the bot stopped: {ex}")
            d.wait_control()
            d.end_turn()
        cpu_turns(ctx, g, watch, shots, day)
    sonja_vignette(ctx, g, d, watch, shots)
    watch.report()
    # Solo play shows every army's turn: the concealed picture is for a
    # netplay seat waiting on the other's army. It covered every turn of
    # armies 2 and 4 (the computer's too) until tango-backend-mgba's solo
    # side stopped concealing.
    ctx.check(not shots.dark, f"{name}: no screenshot is the concealed picture ({len(shots.dark)}: {shots.dark[:6]})")
    ctx.log(f"{shots.n} screenshots")


def sonja_vignette(ctx, g, d, watch, shots):
    """Sonja's units hide their HP from her enemies: a damaged Yellow Comet
    unit and a damaged Blue Moon one next to Orange Star's units, the
    cursor on each (the terrain panel), and the map."""
    e = g.e
    mine = [u for u in g.units(1)]
    occupied = {(u["x"], u["y"]) for u in g.units()}
    spots = []
    for u in mine:
        for dx, dy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
            c = (u["x"] + dx, u["y"] + dy)
            if 0 <= c[0] < watch.w and 0 <= c[1] < watch.h and c not in occupied and c not in spots \
                    and g.terrain_class(*c) & 0x1F in (1, 2, 4, 5, 6, 8, 0xE):
                spots.append(c)
    picks = [(a, next((u for u in g.units(a) if u["type"] in (1, 2, 4, 5, 6)), None)) for a in (4, 2, 5)]
    for (a, u), c in zip(picks, spots):
        if u is None:
            continue
        d.place_unit(u, *c)
        addr = g.unit_addr(u["id"])
        e.w16(addr + 4, (e.u16(addr + 4) & ~0x7F) | 45)
        ctx.log(f"vignette: {NAMES[a]} unit type {u['type']} at {c}, HP 45")
    # A pick-up and cancel redraws the units and the vision.
    u = mine[0]
    try:
        g.select(u["x"], u["y"])
        e.press("B", 4)
        g.wait_for_input()
    except NavError as ex:
        ctx.log(f"vignette: {ex}")
    for (a, u), c in zip(picks, spots):
        if u is None:
            continue
        g.goto(*c)
        e.wait(30)
        shots(f"hp_{NAMES[a].replace(' ', '_')}")


@test(modes=("ds",))
def five_fog_black_monolith(ctx):
    play(ctx, "Black Monolith", False)


@test(modes=("ds",))
def five_fog_black_wastes(ctx):
    play(ctx, "Black Wastes", True)


@test(modes=("ds",))
def five_fog_coral_crown(ctx):
    play(ctx, "Coral Crown", False)
