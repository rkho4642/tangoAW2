"""Missile silos work as AW2's everywhere but Crystal Calamity's map: a
Launch, the player's or the computer's, fires a missile at the map (3 HP off
every unit within 2 squares of the target, never below 1) and the silo is
spent (tile 0x180 to 0x1A0). Checked on a Versus map, on AW2's campaign
(Mission 1 with a silo put in), and on a Dual Strike mission with silos of
its own (Healing Touch); each test logs a `signature:` line (who launched,
the silo, the hits) so that runs of two builds can be compared
(`AW2TEST_RUNNER_DIR`: the v0.5.0 build gives the same lines).

Crystal Calamity is Dual Strike's exception (crate::onyx): the player's
Launch hits the Black Onyx (tests/test_ds_onyx.py), and the computer's
fires nothing (Dual Strike's `0x020B8FA8`: its CPU's Launch on map 0xF2 in
the campaign runs the mission's list with 0x32 instead, Black Hole's "We've
captured one of the anti-satellite missile bases. ..." once, and the unit
waits)."""

import os

from aw2test import dscampaign as dc
from aw2test import paths
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

SILO, SPENT = 0x180, 0x1A0
TILES = dc.MAP + 0xA22
ROWS = dc.MAP + 0x417A
CLASSES = dc.MAP + 0x1432
CURRENT_ARMY = 0x030033EC
LOCAL_FLAGS = 0x030033F4
MAP_BUSY = 0x030030F0
HEALING_TOUCH = 17
CRYSTAL_CALAMITY = 18


def tile(e, x, y):
    return e.u16(TILES + 2 * (e.u16(ROWS + 2 * y) + x))


def put_silo(g, x, y):
    """A missile silo at (x, y) of the battle's map (tile and class)."""
    e = g.e
    row = e.u16(ROWS + 2 * y)
    e.w16(TILES + 2 * (row + x), SILO)
    e.w8(CLASSES + row + x, g.image.tile_class(SILO))


def hp_by_unit(g):
    return {u["id"]: (u["army"], u["x"], u["y"], u["hp"]) for u in g.units()}


def blast(before, after):
    """The units hit (id: (army, x, y, hp before, hp after)), and the square
    the hits are centred on: every unit within 2 squares of it hit, none
    further (None if no square fits)."""
    hit = {i: (*b[:3], b[3], after[i][3]) for i, b in before.items() if i in after and after[i][3] != b[3]}
    if not hit:
        return hit, None
    xs = [b[1] for b in before.values()] + [b[2] for b in before.values()]
    top = max(xs) + 3
    fits = []
    for cx in range(-2, top):
        for cy in range(-2, top):
            near = {i for i, b in before.items() if abs(b[1] - cx) + abs(b[2] - cy) <= 2}
            if near == set(hit):
                fits.append((cx, cy))
    return hit, (fits[0] if len(fits) == 1 else (fits or None))


def check_missile(ctx, hit, centre, who):
    ctx.check(bool(hit), f"{who}'s missile hit units: {hit}")
    ctx.check(centre is not None, f"{who}'s hits are every unit within 2 squares of a square ({centre})")
    for i, (army, x, y, b, a) in hit.items():
        ctx.eq(a, max(1, b - 30), f"{who}'s missile: army {army}'s unit at {(x, y)} takes 3 HP (never below 1)")


def aim(g, at, target, d=None):
    """The target cursor (on the silo when it opens) moved to `target`, and
    the missile fired."""
    e = g.e
    e.wait(60)
    # (AW2's Mission 1 teaches first: its lesson answered)
    for _ in range(20):
        if d is None or not d.scripts_running():
            break
        d.dialogue()
        e.wait(30)
    # "Select missile target now." (where AW2 shows it): answered until the
    # cursor moves
    key = "RIGHT" if target[0] > at[0] else "LEFT" if target[0] < at[0] else "DOWN" if target[1] > at[1] else "UP"
    for _ in range(4):
        c0 = g.cursor()
        e.hold(key, 6)
        e.wait(12)
        if g.cursor() != c0:
            break
        e.press("A", 4)
        e.wait(40)
    # (the target cursor is the map's: moved until it is on the target)
    for _ in range(60):
        x, y = g.cursor()
        if (x, y) == target:
            break
        key = "RIGHT" if x < target[0] else "LEFT" if x > target[0] else "DOWN" if y < target[1] else "UP"
        e.hold(key, 6)
        e.wait(12)
    e.press("A", 4)
    e.wait(60)


def settle(g, d):
    """Until the map waits for orders, answering any event's dialogue (AW2's
    Mission 1 teaches as it goes)."""
    from aw2test.game import NavError
    for _ in range(300):
        if d is not None and d.scripts_running():
            d.dialogue()
            continue
        try:
            g.wait_for_input(max_frames=120)
            if d is None or not d.scripts_running():
                return
        except NavError:
            g.e.press("A", 4)
            g.e.wait(10)


def player_launch(ctx, g, at, target, who="the player", d=None, exact=True):
    names = g.action_menu_at(*at)
    ctx.require("Launch" in names, f"{who}'s Infantry on the silo can Launch ({names})")
    settle(g, d)
    before = hp_by_unit(g)
    g.select(*at)
    g.move_to(*at)
    g.choose("Launch", g.ACTION_MENU)
    aim(g, at, target, d)
    settle(g, d)
    hit, centre = blast(before, hp_by_unit(g))
    check_missile(ctx, hit, centre, who)
    if exact:
        ctx.check(centre == target or (isinstance(centre, list) and target in centre), f"{who}'s missile lands on the square aimed at {target} ({centre})")
    ctx.eq(tile(g.e, *at), SPENT, f"{who}'s silo is spent")
    return hit, centre


def signature(ctx, who, at, tile_after, hit, centre):
    hits = sorted((a, x, y, b, c) for (a, x, y, b, c) in hit.values())
    ctx.log(f"signature: {who} silo {at} tile {tile_after:#x} centre {centre} hits {hits}")


# --- A Versus map ---------------------------------------------------------------------

@test()
def silo_versus_player(ctx):
    """Versus: the player's Infantry on a silo launches at (14, 10): the
    three Tanks and the Infantry within 2 squares take 3 HP, the Tank 3
    squares away nothing; the silo is spent."""
    m = ctx.map()
    m.terrain(10, 10, SILO)
    m.unit(1, "infantry", 10, 10)
    m.unit(2, "tank", 14, 10).unit(2, "tank", 15, 10).unit(2, "infantry", 14, 12).unit(2, "tank", 13, 9)
    m.unit(2, "tank", 17, 10)
    g = ctx.start(m, ["andy", "andy"])
    hit, centre = player_launch(ctx, g, (10, 10), (14, 10))
    ctx.eq(len(hit), 4, "four units within 2 squares")
    ctx.eq(g.unit_at(17, 10)["hp"], 100, "the Tank 3 squares away is untouched")
    ctx.shot(g, "after")
    signature(ctx, "player", (10, 10), tile(g.e, 10, 10), hit, centre)


@test()
def silo_versus_cpu(ctx):
    """Versus: the computer's Infantry on a silo launches at the player's
    group of units: those within 2 squares of its target take 3 HP; the
    silo is spent."""
    m = ctx.map()
    m.terrain(15, 10, SILO)
    m.unit(2, "infantry", 15, 10)
    for x, y in ((5, 5), (6, 5), (5, 6), (6, 6), (7, 5)):
        m.unit(1, "tank", x, y)
    g = ctx.start(m, ["andy", "andy"])
    before = hp_by_unit(g)
    g.end_turn()
    hit, centre = blast(before, hp_by_unit(g))
    check_missile(ctx, hit, centre, "the computer")
    ctx.eq(tile(g.e, 15, 10), SPENT, "the computer's silo is spent")
    ctx.shot(g, "after")
    signature(ctx, "cpu", (15, 10), tile(g.e, 15, 10), hit, centre)


# --- AW2's campaign ---------------------------------------------------------------------

def aw2_mission_1(ctx):
    """AW2 CAMPAIGN, New: Mission 1's battle, the player to move."""
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.open_campaign_box()
    if ctx.ds:
        d.chooser_row(0)  # AW2 CAMPAIGN
        e.press("A", 8)
        e.wait(30)
    d.box_row(1)  # New
    e.wait(30)
    e.press("A", 8)
    for _ in range(4000):
        if d.in_battle() and e.u8(CURRENT_ARMY) == 1 and not d.scripts_running():
            break
        e.press("A", 4)
        e.wait(10)
    ctx.require(d.in_battle() and not d.active(), "AW2's Mission 1 (not the DS Campaign)")
    d.wait_control()
    # (its lessons are once-only events: their flags, the battle's 0..0x1F,
    # set as if taught, so that none stops the play)
    e.write(LOCAL_FLAGS, b"\xff" * 4)
    e.wait(10)
    return e, g, d


PLAIN, ROAD = 1, 3


def tutorial(g, d, at):
    """Mission 1 teaches: picking up a unit type the first time shows its
    lesson. Picked up and put down (the lesson answered) until it no
    longer shows."""
    e = g.e
    for _ in range(3):
        g.wait_idle()
        g.goto(*at)
        e.press("A", 4)
        e.wait(40)
        if not d.scripts_running():
            e.press("B", 4)
            e.wait(20)
            g.wait_for_input()
            return
        d.dialogue()
        e.wait(20)
        e.press("B", 4)
        e.wait(20)
        e.press("B", 4)
        e.wait(20)


def free_cell(g, near, avoid=(), dist=(0, 6)):
    """An empty plain or road nearest to `near`, between `dist` squares
    from it."""
    w, h = g.e.u16(dc.MAP), g.e.u16(dc.MAP + 2)
    cells = [(x, y) for y in range(h) for x in range(w)
             if dist[0] <= abs(x - near[0]) + abs(y - near[1]) <= dist[1]
             and (x, y) not in avoid and not g.unit_at(x, y) and g.terrain_class(x, y) & 0x1F in (PLAIN, ROAD)]
    return min(cells, key=lambda c: (abs(c[0] - near[0]) + abs(c[1] - near[1]), c[1], c[0])) if cells else None


@test()
def silo_aw2_campaign(ctx):
    """AW2's campaign (Mission 1, a silo put in): the player's Infantry
    launches (3 HP within 2 squares, the silo spent); then an enemy
    Infantry on another silo: the computer launches the same way."""
    e, g, d = aw2_mission_1(ctx)
    mine = g.units(1)
    foes = [u for u in g.units() if u["army"] != 1]
    ctx.require(mine and foes, "both sides have units")
    # The player: a unit made an Infantry on a silo, aimed at an enemy.
    target = (foes[0]["x"], foes[0]["y"])
    at = free_cell(g, target, avoid=[target], dist=(4, 8))
    ctx.require(at, "a free square for the player's silo")
    put_silo(g, *at)
    u = mine[0]
    e.w8(g.unit_addr(u["id"]), 1)
    d.place_unit(u, *at)
    e.wait(5)
    tutorial(g, d, at)
    hit, centre = player_launch(ctx, g, at, target, d=d, exact=False)
    signature(ctx, "player", at, tile(e, *at), hit, centre)
    # The computer: one of its units made an Infantry on a silo near the
    # player's units.
    mine = g.units(1)
    foe = [u for u in g.units() if u["army"] != 1][0]
    at2 = free_cell(g, (mine[0]["x"], mine[0]["y"]), avoid=[at], dist=(4, 8))
    ctx.require(at2, "a free square for the computer's silo")
    put_silo(g, *at2)
    e.w8(g.unit_addr(foe["id"]), 1)
    d.place_unit(foe, *at2)
    # The player's units in a group: a target worth a missile.
    group = [(mine[0]["x"], mine[0]["y"])]
    for v in mine[1:]:
        c = free_cell(g, group[0], avoid=[at, at2] + group, dist=(1, 2))
        if c:
            d.place_unit(v, *c)
            group.append(c)
    e.wait(5)
    m = Missile(g, at2)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    for _ in range(60):
        if e.u8(CURRENT_ARMY) != 1:
            break
        if d.scripts_running():
            d.dialogue()
        elif g.menu():
            e.press("A", 4)
        e.wait(10)
    for _ in range(5000):
        if d.scripts_running():
            d.dialogue()
        elif e.u8(CURRENT_ARMY) == 1:
            settle(g, d)
            break
        e.wait(4)
        m()
    hit2, centre2 = m.result()
    used = tile(e, *at2) == SPENT
    ctx.log(f"the computer's silo at {at2}: {'spent' if used else 'not used'}; hits {hit2}")
    ctx.check(used, "the computer launched from its silo")
    if used:
        check_missile(ctx, hit2, centre2, "the computer")
    ctx.shot(g, "after")
    signature(ctx, "cpu", at2, tile(e, *at2), hit2, centre2)


# --- Dual Strike's missions ----------------------------------------------------------------

def ds_mission(ctx, index):
    e = Emu(save=paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=dc.ORDER.index(index))
    d.wait_map()
    d.wait_control()
    return e, g, d


def silos_of(e, w, h):
    return [(x, y) for y in range(h) for x in range(w) if tile(e, x, y) == SILO]


class Missile:
    """Watches a computer's silo while its turn runs: the units' HP just
    before it is spent and once the strike is over (the map no longer
    busy), before any other unit acts."""

    def __init__(self, g, at):
        self.g, self.at = g, at
        self.before = hp_by_unit(g)
        self.spent = False
        self.after = None

    def __call__(self):
        g, e = self.g, self.g.e
        if self.after is not None:
            return
        if tile(e, *self.at) != SPENT:
            self.before = hp_by_unit(g)
        elif not self.spent:
            self.spent = True
        elif e.u8(MAP_BUSY) == 0:
            self.after = hp_by_unit(g)

    def result(self):
        return blast(self.before, self.after or hp_by_unit(self.g))


def cpu_turns(e, g, d, max_turns=8, watch=None):
    """Ends the player's turns until the computer has moved and the first
    army's turn is back (dialogue answered on the way); the texts shown.
    `watch()` is called every few frames of the computer's turn."""
    texts = []
    cpu_seen = False
    for _ in range(max_turns):
        army = e.u8(CURRENT_ARMY)
        if d.controllers()[army - 1] == 1:
            if army == 1 and cpu_seen:
                break
            d.wait_control()
            d.end_turn()
            e.wait(60)
            continue
        cpu_seen = True
        for _ in range(400):
            t = d.text_shown()
            if t and (not texts or texts[-1] != t):
                texts.append(t.replace("\r", " "))
            if d.scripts_running():
                e.press("A", 4)
            e.wait(4)
            if watch:
                watch()
            if d.controllers()[e.u8(CURRENT_ARMY) - 1] == 1 and not d.scripts_running():
                break
    d.wait_control()
    return texts


@test(modes=("ds",))
def silo_ds_healing_touch(ctx):
    """Healing Touch (Dual Strike's, with five silos): the player's Infantry
    on a silo launches as AW2's; then a computer's unit made an Infantry on
    another silo next to the player's units launches as AW2's too."""
    e, g, d = ds_mission(ctx, HEALING_TOUCH)
    w, h = d.size()
    silos = silos_of(e, w, h)
    ctx.eq(len(silos), 5, f"the mission's five silos ({silos})")
    p = d.players()
    team = lambda a: e.u8(p + 0x3C * a + 0x2A)
    cpu = [a for a, c in enumerate(d.controllers(), 1) if c == 2 and team(a) != team(1)]
    ctx.require(cpu, f"an enemy computer army ({d.controllers()})")
    foes = [u for u in g.units() if u["army"] in cpu]
    mine = g.units(1)
    # The player: from the first silo at the nearest enemy.
    at = silos[0]
    if g.unit_at(*at):
        d.remove_unit(g.unit_at(*at))
    u = mine[0]
    e.w8(g.unit_addr(u["id"]), 1)
    d.place_unit(u, *at)
    e.wait(5)
    foe = min(foes, key=lambda f: abs(f["x"] - at[0]) + abs(f["y"] - at[1]))
    target = (foe["x"], foe["y"])
    hit, centre = player_launch(ctx, g, at, target)
    signature(ctx, "player", at, tile(e, *at), hit, centre)
    # The computer: from the last silo, the player's units gathered nearby.
    at2 = silos[-1]
    if g.unit_at(*at2):
        d.remove_unit(g.unit_at(*at2))
    # (one with the ordinary AI byte, +0xB 0, made a full-HP Infantry:
    # Healing Touch's own are nearly destroyed and mostly hold)
    foe = [f for f in g.units() if f["army"] in cpu and f["raw"][0xB] == 0][0]
    a = g.unit_addr(foe["id"])
    e.w8(a, 1)
    e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 100)
    d.place_unit(foe, *at2)
    group = []
    for v in [v for v in g.units(1) if v["id"] != u["id"]][:5]:
        c = free_cell(g, (at2[0], at2[1] - 4), avoid=[at, at2] + group, dist=(0, 3))
        if c:
            e.w8(g.unit_addr(v["id"]), 4)  # (a Tank: a target worth a missile)
            d.place_unit(v, *c)
            group.append(c)
    ctx.log(f"controllers {d.controllers()}, teams {[team(a) for a in range(1, 5)]}, the computer's Infantry {foe['army']}, group {group}")
    e.wait(5)
    m = Missile(g, at2)
    texts = cpu_turns(e, g, d, watch=m)
    hit2, centre2 = m.result()
    used = tile(e, *at2) == SPENT
    if used:
        check_missile(ctx, hit2, centre2, "the computer")
    ctx.log(f"the computer's silo at {at2}: {'spent' if used else 'not used'}; texts {texts[:4]}")
    ctx.check(used, "the computer launched from its silo")
    ctx.check(not any("anti-satellite" in t for t in texts), "no Crystal Calamity dialogue")
    ctx.shot(g, "after")
    signature(ctx, "cpu", at2, tile(e, *at2), hit2, centre2)


@test(modes=("ds",))
def ds_onyx_cpu_silo(ctx):
    """Crystal Calamity: Black Hole's computer on a silo fires nothing (Dual
    Strike's CPU Launch on map 0xF2, `0x020B8FA8`): the mission's list runs
    with 0x32, its script (once, flag 0x0E) has Black Hole say "We've
    captured one of the anti-satellite missile bases. The allied forces are
    no longer a threat. They cannot fire on Black Onyx." and "Aha ha ha!
    Bravo! Now the barrier field will be completed!", then gives Black Hole
    the win (op 0x41): "Dude. WEAK!", DEFEAT, the mission lost. No missile
    flies (the silo not spent, no unit hit by one, the satellite's hits
    kept). As forced in melonDS (onyx2/work navf*: the CPU's action set to
    0x16 on map 0xF2, the same lines, DEFEAT)."""
    e, g, d = ds_mission(ctx, CRYSTAL_CALAMITY)
    before = d.progress()
    silo = (6, 2)
    ctx.eq(tile(e, *silo), SILO, "the silo at (6, 2)")
    bh = [u for u in g.units() if u["army"] == 4]
    u = min(bh, key=lambda v: abs(v["x"] - silo[0]) + abs(v["y"] - silo[1]))
    e.w8(g.unit_addr(u["id"]), 1)
    if g.unit_at(*silo):
        d.remove_unit(g.unit_at(*silo))
    d.place_unit(u, *silo)
    # The player's units nearby, a target worth a missile.
    group = []
    for v in [v for v in g.units() if v["army"] in (1, 2, 3)][:4]:
        c = free_cell(g, (10, 4), avoid=[silo] + group, dist=(0, 3))
        if c:
            d.place_unit(v, *c)
            group.append(c)
    e.wait(5)
    hits0 = e.u8(0x0203FFC9)
    # The player's armies end their turns; Black Hole's turn until its line.
    for _ in range(6):
        if d.controllers()[e.u8(CURRENT_ARMY) - 1] != 1:
            break
        d.wait_control()
        d.end_turn()
        e.wait(60)
    hp0 = hp_by_unit(g)
    said = False
    for _ in range(3000):
        t = d.text_shown() or ""
        if "anti-satellite" in t:
            said = True
            break
        if d.scripts_running():
            e.press("A", 4)
        e.wait(4)
    ctx.require(said, "Black Hole's line")
    ctx.eq(tile(e, *silo), SILO, "the silo is not spent")
    now = hp_by_unit(g)
    missile = {i: (b, now[i]) for i, b in hp0.items() if i in now and now[i][3] == max(1, b[3] - 30) and b[3] > 1}
    ctx.eq(missile, {}, "no unit took a missile's 3 HP")
    ctx.eq(e.u8(0x0203FFC9), hits0, "the satellite keeps its hits")
    e.shot(os.path.join(ctx.out, "captured"))
    r = d.follow_defeat(ctx.out)
    ctx.log(f"texts: {r['texts']}")
    ctx.check(any("We've captured one of the anti-satellite missile bases" in x for x in r["texts"]), "\"We've captured one of the anti-satellite missile bases. ...\"")
    ctx.check(any("Now the barrier field will be completed" in x for x in r["texts"]), "\"Aha ha ha! Bravo! Now the barrier field will be completed!\"")
    ctx.check(any("Dude. WEAK!" in x for x in r["texts"]), "the defeat's \"Dude. WEAK!\"")
    ctx.eq(r["box_left"], [], "no terrain box window or darkening left over the dialogue")
    ctx.eq(r["panel_left"], [], "no CO panel (its partner strip) showing below a dialogue box at the top")
    ctx.check(r["banner"], "the DEFEAT banner")
    res = d.last_result()
    ctx.eq((res["result"], res["mission"]), (2, CRYSTAL_CALAMITY), "the mission lost")
    ctx.check(r["world_map"], "back on the world map")
    ctx.eq(d.map_flags()[CRYSTAL_CALAMITY] & 2, 0, "Crystal Calamity not cleared")
    ctx.eq(d.progress(), before, "the campaign's record as before")
