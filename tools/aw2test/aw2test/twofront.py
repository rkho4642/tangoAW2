"""Driving a battle on two fronts (crate::two_front): its state, rounds, the
Front view, Send, the second front's units and structures, and the mission's
end through its own conditions."""

import os
import struct

from . import dscampaign as dc
from .game import NavError

# crate::two_front's RAM.
STATE = 0x0203E400
LIVE = STATE              # the front on the screen: 0 main, 1 second
SECOND = STATE + 1        # 0 not started, 1 fought, 2 won, 3 lost
STORE_KIND = STATE + 2    # 1: the store holds the front not on the screen
BUSY = STATE + 3          # a swap running (its kind)
VIEW = STATE + 5          # 1 while the other front is looked at
BANNER = STATE + 7        # the second front's result shown (frames left)
SENDING = STATE + 0x08
QUEUED = STATE + 0x14     # units sent before the second front started
WINNER = STATE + 0x15
STARTED = STATE + 0x16    # the second front has had its first round
STATE_LEN = 0xA0
SECOND_COS = STATE + STATE_LEN
STORE = 0x0203E500        # the front not on the screen (AW2's suspend block)
BLOCK_LEN = 0xE28
B_DAY, B_ARMY, B_PLAYERS, B_UNITS, B_INVENTIONS = 0x00, 0x02, 0x14, 0x188, 0xD28
SECOND_WON, SECOND_LOST = 2, 3
VIEW_IN = 2               # BUSY while looking at the other front

MAP_STATE = 0x030032D8
CURSOR_STATE = 0xD
CURRENT_ARMY = 0x030033EC
DAY = 0x03004080
MAP_MENU_POOL = 0x0802D49C
UNIT_MENU_POOL = 0x0802D59C
GAME_MAP_MENU = 0x0849AAC0
GAME_UNIT_MENU = 0x0849AE28
INVENTIONS = 0x02028360
MAP_LOCK = 0x030040E8


def state(e):
    return {"live": e.u8(LIVE), "second": e.u8(SECOND), "busy": e.u8(BUSY), "view": e.u8(VIEW),
            "day": e.u16(DAY), "army": e.u16(CURRENT_ARMY), "map_state": e.u16(MAP_STATE)}


def checkpoint(e, ctx, name):
    p = os.path.join(ctx.out, name + ".state")
    e.cmd(f"statefile {p}")
    return p


def back_to(e, g, path):
    e.cmd(f"loadfile {path}")
    e.wait(2)
    g._units_base = g._players_base = None


def player_turn(e):
    """The main front, the player's army, the cursor waiting."""
    return (e.u8(LIVE) == 0 and e.u8(BUSY) == 0 and e.u16(CURRENT_ARMY) == 1
            and e.u16(MAP_STATE) == CURSOR_STATE)


def until(e, d, pred, frames=40000, each=None):
    """Runs (A through dialogue) until `pred`; True if it held."""
    n = 0
    while n < frames:
        if pred():
            return True
        if d.scripts_running():
            e.press("A", 4)
        e.wait(16)
        n += 20
        if each:
            each()
    return pred()


def end_round(e, d, ctx=None, label=None, each=None):
    """The player ends the turn: Black Hole's turn on the main front, the
    second front's round, back to the player's next turn. Returns the
    states seen (live front, day, army) in order."""
    seen = []

    def watch():
        s = (e.u8(LIVE), e.u16(DAY), e.u16(CURRENT_ARMY))
        # (not at the handover: a front just brought back shows its last
        # round's army for that frame)
        if e.u8(BUSY) == 0 and e.u16(MAP_STATE) not in (0, 3) and (not seen or seen[-1] != s):
            seen.append(s)
        if each:
            each()

    d.end_turn()
    e.wait(30)
    if not until(e, d, lambda: player_turn(e) or e.u8(dc.LAST_RESULT) != 0, each=watch):
        raise NavError(f"the round did not come back to the player: {state(e)}")
    if e.u8(dc.LAST_RESULT) == 0:
        d.wait_control()
    return seen


def store_units(e):
    """The stored front's units: (army, type, x, y, hp)."""
    b = e.read(STORE + B_UNITS, 4 * 51 * 12)
    out = []
    for a in range(4):
        for j in range(51):
            r = b[12 * (a * 51 + j):12 * (a * 51 + j) + 12]
            if r[0]:
                out.append((a + 1, r[0], r[2], r[3], r[4] & 0x7F))
    return out


def store_unit_addrs(e, army):
    return [STORE + B_UNITS + 12 * ((army - 1) * 51 + j) for j in range(51)
            if e.u8(STORE + B_UNITS + 12 * ((army - 1) * 51 + j))]


def store_inventions(e):
    """The stored front's inventions: (address, x, y, kind, hp)."""
    out = []
    for k in range(16):
        a = STORE + B_INVENTIONS + 8 * k
        kind = (e.u16(a + 2) >> 6) & 15
        if kind == 0:
            break
        out.append((a, e.u8(a), e.u8(a + 1), kind, e.u8(a + 4)))
    return out


def inventions(e):
    out = []
    for k in range(16):
        a = INVENTIONS + 8 * k
        kind = (e.u16(a + 2) >> 6) & 15
        if kind == 0:
            break
        out.append((a, e.u8(a), e.u8(a + 1), kind, e.u8(a + 4)))
    return out


def map_menu_names(g):
    m = g.open_map_menu()
    names = m["names"]
    g.e.press("B", 4)
    g.wait_for_input()
    return names


def look_at_other_front(e, g):
    """Map menu > Front: the other front shows (display only)."""
    g.open_map_menu()
    g.choose("Front", g.MAP_MENU)
    ok = e.wait_until(lambda: e.u8(VIEW) == 1 and e.u8(BUSY) == VIEW_IN and e.u8(MAP_LOCK) == 0, 1200, step=10)
    e.wait(20)
    return ok


def come_back(e, g):
    e.press("B", 4)
    ok = e.wait_until(lambda: e.u8(BUSY) == 0 and e.u16(MAP_STATE) == CURSOR_STATE, 1500, step=10)
    e.wait(20)
    g.wait_for_input()
    return ok


def select(e, g, d, x, y):
    """Picks up the unit at (x, y), through any dialogue its selection
    starts."""
    g.wait_idle()
    g.goto(x, y)
    e.press("A", 4)
    for _ in range(300):
        e.wait(10)
        if d.scripts_running():
            e.press("A", 4)
            continue
        if g.has_proc(0x080228D9):
            e.wait(10)
            if not d.scripts_running():
                return
    raise NavError(f"unit at {(x, y)} not selected")


def command(e, g, d, x, y, name, to=None):
    """The unit at (x, y) moves to `to` (or stays) and takes command `name`;
    the dialogue after it is pressed through. Returns the command menu's
    names."""
    select(e, g, d, x, y)
    m = g.move_to(*(to or (x, y)))
    names = m["names"]
    if name not in names:
        e.press("B", 4)
        e.wait(20)
        e.press("B", 4)
        g.wait_for_input()
        return names
    k = names.index(name)
    for _ in range(k):
        e.press("DOWN", 4)
        e.wait(6)
    e.press("A", 4)
    for _ in range(300):
        e.wait(10)
        if d.scripts_running():
            e.press("A", 4)
        elif g.idle():
            break
    g.wait_for_input()
    return names


def act(e, g, d):
    """A unit of the player's waits (the after-action events run); one is
    made when the army has none."""
    units = [u for u in g.units(1) if not u["flags"] & 1] or [player_unit(d)]
    for u in units:
        try:
            names = command(e, g, d, u["x"], u["y"], "Wait")
        except NavError:
            # (the battle may have ended with the action)
            return True
        if "Wait" in names:
            return True
    return False


def force_lose(d, player=(1,)):
    """The player's army left one unit, at 1 HP, next to an enemy unit able
    to hit it; then the turn ends: the enemy destroys it (AW2's rout)."""
    g, e = d.g, d.e
    units = g.units()
    mine = [u for u in units if u["army"] in player]
    enemy = [u for u in units if u["army"] not in player and u["type"] in dc.DIRECT]
    if not mine or not enemy:
        return False
    w, h = d.size()
    for v in enemy:
        for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            x, y = v["x"] + dx, v["y"] + dy
            if 0 <= x < w and 0 <= y < h and e.u8(d.layer_cell(x, y)) == 0 and g.terrain_class(x, y) & 0x1F in dc.LAND:
                last = next((u for u in mine if u["type"] == 1), mine[0])
                for u in mine:
                    if u is not last:
                        d.remove_unit(u)
                d.place_unit(last, x, y)
                a = g.unit_addr(last["id"])
                e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 1)
                e.w8(a, 1)  # an Infantry
                return True
    return False


def mission_over(e, d, frames=30000):
    """Runs (A through the scenes and the results) until the mission's end is
    recorded: 1 won, 2 lost (0: it goes on)."""
    n = 0
    while n < frames and e.u8(dc.LAST_RESULT) == 0:
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(16)
        n += 20
        if n > 3000 and d.in_battle() and not d.scripts_running() and e.u8(BUSY) == 0 and e.u16(MAP_STATE) in (CURSOR_STATE, 0xE):
            break
    return e.u8(dc.LAST_RESULT)


def force_win(d):
    """The enemy left one unit, on 1 HP, next to a unit of the player's able
    to hit it, which does (the game's own rout). The DS Campaign's helper
    first; else the enemy's unit is brought next to the player's."""
    if d.force_win():
        return True
    g, e = d.g, d.e
    mine = [u for u in g.units(1) if u["type"] in dc.DIRECT]
    enemy = [u for u in g.units() if u["army"] != 1]
    if not mine or not enemy:
        return False
    w, h = d.size()
    for m in mine:
        for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            x, y = m["x"] + dx, m["y"] + dy
            if 0 <= x < w and 0 <= y < h and e.u8(d.layer_cell(x, y)) == 0 and g.terrain_class(x, y) & 0x1F in dc.LAND:
                victim = enemy[0]
                for u in enemy[1:]:
                    d.remove_unit(u)
                d.place_unit(victim, x, y)
                a = g.unit_addr(victim["id"])
                e.w8(a, 1)
                e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 1)
                e.wait(4)
                d.fire((m["x"], m["y"]), (x, y))
                return True
    return False


def make_unit(d, army, t, x, y, hp=100):
    """A unit of `army` (its first free slot) of type `t` at (x, y)."""
    g, e = d.g, d.e
    base = g.units_base
    for j in range(1, 51):
        uid = (army - 1) * 64 + j
        a = base + 12 * uid
        if e.u8(a) == 0:
            e.cmd(f"pokebytes {a:08x} " + "00" * 12)
            e.w8(a, t)
            e.w8(a + 2, x)
            e.w8(a + 3, y)
            e.w16(a + 4, hp | (9 << 7))
            e.w8(a + 6, 99)
            e.w8(d.layer_cell(x, y), uid)
            # (the army's unit count, which its rout reads)
            p = g.players_base + 0x3C * army + 0x3A
            e.w8(p, e.u8(p) + 1)
            return uid
    return None


def free_land_next_to(d, x0, y0):
    g, e = d.g, d.e
    w, h = d.size()
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        x, y = x0 + dx, y0 + dy
        if 0 <= x < w and 0 <= y < h and e.u8(d.layer_cell(x, y)) == 0 and g.terrain_class(x, y) & 0x1F in dc.LAND:
            return x, y
    return None


def free_land(d):
    g, e = d.g, d.e
    w, h = d.size()
    for y in range(h):
        for x in range(w):
            if e.u8(d.layer_cell(x, y)) == 0 and g.terrain_class(x, y) & 0x1F == 1 and free_land_next_to(d, x, y):
                return x, y
    return None


def player_unit(d):
    """A unit of the player's that can still act (one is made, an
    Infantry, when the army has none: Means to an End's starts without)."""
    g = d.g
    free = [u for u in g.units(1) if not u["flags"] & 1]
    if free:
        return free[0]
    x, y = free_land(d)
    make_unit(d, 1, 1, x, y)
    d.e.wait(4)
    return next(u for u in g.units(1) if (u["x"], u["y"]) == (x, y))


def rout_enemy(d):
    """The enemy's last unit (made when it has none) destroyed by a Tank of
    the player's (made when it has none): AW2's rout."""
    g, e = d.g, d.e
    if d.force_win():
        return True
    mine = [u for u in g.units(1) if u["type"] in dc.DIRECT]
    if mine:
        m = mine[0]
    else:
        x, y = free_land(d)
        make_unit(d, 1, 4, x, y)
        e.wait(4)
        m = next(u for u in g.units(1) if (u["x"], u["y"]) == (x, y))
    spot = free_land_next_to(d, m["x"], m["y"])
    if spot is None:
        return False
    for u in [u for u in g.units() if u["army"] != 1]:
        d.remove_unit(u)
    enemy_army = 2
    make_unit(d, enemy_army, 1, spot[0], spot[1], hp=1)
    e.wait(4)
    d.fire((m["x"], m["y"]), spot)
    return True


def rout_player(d):
    """The player's last unit, on 1 HP, next to an enemy Tank (made when
    the enemy has none able to hit it); then the player's turn ends."""
    g, e = d.g, d.e
    if force_lose(d):
        return True
    m = player_unit(d)
    for u in g.units(1):
        if u["id"] != m["id"]:
            d.remove_unit(u)
    a = g.unit_addr(m["id"])
    e.w8(a, 1)
    e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 1)
    spot = free_land_next_to(d, m["x"], m["y"])
    make_unit(d, 2, 4, spot[0], spot[1])
    e.wait(4)
    return True
