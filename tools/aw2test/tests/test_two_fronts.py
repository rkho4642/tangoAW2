"""Battles on two fronts (crate::two_front, docs/AW2.md "Two fronts"): Dual
Strike's five two-front missions played on both fronts. Each front's state
is kept whole while the other is on the screen; the fronts take turns by
rounds (every army on one front, then every army on the other, then the
next day); the map menu's Front item shows the other front (display only);
Send moves a unit from the main front to the second; the computer directs
the player's army on the second front; each front ends by its own
conditions (Dual Strike's records for it), and the second front's end goes
back to the main front for good; a mission saved halfway keeps both fronts.
Per mission, each win and loss condition of each front is triggered
through the game's state and checked to end its front (or the mission)
as Dual Strike's does. No bot play: the units and structures are set up
directly."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, saves
from aw2test import twofront as tf
from aw2test.rom import DualStrike
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END = 8, 10, 14, 21, 24
TWO_FRONTS = (VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END)
DS_TABLE = 0x08E00000
REGIONS = {
    "units": (0x02022684, 12 * 256),
    "players": (0x02023284, 0x3C * 5),
    "inventions": (0x02028360, 0x80),
    "gPlaySt": (0x03003FC0, 0x48),
    "map tiles": (0x0201E450 + 0xA22, 0xA10),
    "map terrain": (0x0201E450 + 0x1432, 0x510),
    "map units": (0x0201E450 + 0x12, 0x510),
    "day and army": (0x03004080, 2),
    "weather": (0x03004490, 12),
    "battle flags": (0x030033F4, 16),
    "cursor": (0x030033E4, 4),
    "skills": (0x0203F7E0, 30),
}


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def start(ctx, mission):
    """The mission from the DS Campaign's world map, through its CO screens,
    to the player's first turn."""
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(mission))
    d.wait_map()
    ctx.require(d.mission() == mission and tf.state(e)["second"] == 1, f"mission {mission} on two fronts ({tf.state(e)})")
    return e, g, d


def snap(e):
    return {k: e.read(a, n) for k, (a, n) in REGIONS.items()}


def same(ctx, a, b, label):
    for k in REGIONS:
        diff = [i for i in range(len(a[k])) if a[k][i] != b[k][i]]
        ctx.check(not diff, f"{label}: {k} the same" + (f" ({len(diff)} bytes differ from +{diff[0]:#x})" if diff else ""))


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


def first_round(ctx, e, d, label="the first round"):
    seen = tf.end_round(e, d)
    ctx.check(e.u8(tf.STARTED) == 1, f"{label}: the second front played ({seen})")
    return seen


@test(modes=("ds",))
def two_front_rounds(ctx):
    """Victory or Death!: the fronts take turns as Dual Strike's do (checked
    in melonDS: day 1 the main front, the player then Black Hole; day 1 the
    second front, its armies in order; day 2 the main front ...): every army
    of the live front plays, the screen fades to black, the other front's
    armies play their round, and back; the second front starts on its own map
    and deployment, with its own COs and colours, its armies the computer's."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    shot(ctx, e, "main_day1")
    shots = []

    def each():
        if e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0 and not shots:
            shots.append(e.frame)
            shot(ctx, e, "second_day1")

    seen = tf.end_round(e, d, each=each)
    ctx.eq(seen, [(0, 1, 2), (1, 1, 1), (1, 1, 2), (0, 2, 1)], "main front day 1 (Black Hole), second front day 1 (army 1, army 2), main front day 2")
    ctx.eq(e.u16(DS_TABLE + 0x5C * dc.DS_MAP_ID + 0x24), 0, "the main front's header back in the map table (Victory or Death! has no day limit)")
    units = tf.store_units(e)
    ctx.check(units and all(t == 16 for _, t, _, _, _ in units), f"the second front's own deployment: Fighters ({units})")
    p = tf.STORE + tf.B_PLAYERS
    ctl = [e.u8(p + 0x3C * a + 0x1B) for a in (1, 2)]
    ctx.eq(ctl, [2, 2], "the second front's armies are the computer's (Dual Strike: 'In Campaign mode, the second front is controlled automatically')")
    cols = [e.u8(p + 0x3C * a + 0x1A) for a in (1, 2)]
    ctx.eq(cols, [1, 5], "its colours: Orange Star and Black Hole")
    cos = [e.u8(p + 0x3C * a + 0x1D) for a in (1, 2)]
    ctx.eq(cos[0], e.u8(tf.SECOND_COS), "the player's second CO (picked on the CO screen) leads it")
    ctx.check(cos[0] != e.u8(g.players_base + 0x3C + 0x1D), f"a CO of its own ({cos[0]}), not the main front's")
    ctx.eq(e.u16(tf.STORE + tf.B_DAY), 1, "its day: the main front's day 1 (it played day 1)")
    seen = tf.end_round(e, d)
    ctx.eq(seen, [(0, 2, 2), (1, 2, 1), (1, 2, 2), (0, 3, 1)], "day 2 on both, then day 3")


@test(modes=("ds",))
def two_front_view_round_trip(ctx):
    """The map menu's Front item (CO / Intel / Options / Front / Save / End,
    with its help line) shows the other front, display only: the cursor
    moves, every other button is kept from the game; B comes back. The front
    left is brought back byte for byte (units, players, inventions, gPlaySt,
    the map's planes, day and army, weather, the battle's flags, the cursor,
    the skills), before and after the second front has started, and the
    front looked at is unchanged too."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    m = g.open_map_menu()
    ctx.eq(m["names"], ["CO", "Intel", "Options", "Front", "Save", "End"], "the map menu with Front")
    for _ in range(3):
        e.press("DOWN", 4)
        e.wait(6)
    e.wait(10)
    shot(ctx, e, "front_highlighted")
    e.press("B", 4)
    g.wait_for_input()
    for label in ("before the second front's first round", "after it"):
        before = snap(e)
        store = e.read(tf.STORE, tf.BLOCK_LEN)
        ctx.require(tf.look_at_other_front(e, g), f"{label}: the other front shows")
        ctx.eq(e.u8(tf.LIVE), 1, f"{label}: the second front on the screen")
        shot(ctx, e, f"view_{label[:5]}")
        looked = snap(e)
        for k in ["RIGHT"] * 6 + ["DOWN"] * 3:
            e.press(k, 4)
            e.wait(4)
        for k in ("A", "START", "SELECT", "R", "L"):
            e.press(k, 4)
            e.wait(20)
        ctx.check(e.u8(tf.VIEW) == 1 and g.menu() is None, f"{label}: A, START, SELECT, L and R do nothing there")
        after_look = snap(e)
        for k in ("units", "players", "inventions", "day and army"):
            ctx.check(after_look[k] == looked[k], f"{label}: the front looked at unchanged ({k})")
        ctx.require(tf.come_back(e, g), f"{label}: B comes back")
        same(ctx, before, snap(e), f"{label}, back")
        if e.u8(tf.STARTED):
            ctx.check(e.read(tf.STORE, tf.BLOCK_LEN) == store, f"{label}: the stored second front unchanged")
        if label.startswith("before"):
            first_round(ctx, e, d)


@test(modes=("ds",))
def two_front_menus_only_there(ctx):
    """Front and Send exist only in the two-front missions: a one-front DS
    mission, a Versus battle and AW2's own menus keep the game's tables (and
    their items) byte for byte."""
    e, g, d = boot(ctx)
    d.start(step=0)
    d.wait_map()
    ctx.eq(e.u32(tf.MAP_MENU_POOL), tf.GAME_MAP_MENU, "Jake's Trial: the game's map menu")
    names = tf.map_menu_names(g)
    ctx.check("Front" not in names, f"no Front on a one-front mission's map menu ({names})")
    u = g.units(1)[0]
    names = tf.command(e, g, d, u["x"], u["y"], "Wait")
    ctx.check("Send" not in names, f"no Send there ({names})")
    ctx.check(e.u32(tf.UNIT_MENU_POOL) != tf.STORE, "the command menu is not ours")
    e.close()
    m = ctx.map()
    m.unit(1, "fighter", 5, 5).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["andy", "drake"])
    ctx.eq(g.e.u32(tf.MAP_MENU_POOL), tf.GAME_MAP_MENU, "Versus: the game's map menu")
    names = tf.map_menu_names(g)
    ctx.check("Front" not in names, f"no Front in Versus ({names})")
    names = g.action_menu_at(5, 5)
    ctx.check("Send" not in names, f"no Send in Versus ({names})")
    ctx.eq(g.e.read(tf.STATE, tf.STATE_LEN), bytes(tf.STATE_LEN), "nothing of the two fronts' state written")


@test(modes=("ds",))
def two_front_send(ctx):
    """Send (Dual Strike's command): on a front in the sky (Victory or
    Death!) a Fighter, Bomber, Stealth or Black Bomb may go, from anywhere,
    not a copter or a ground unit; it leaves the main front once its move
    ends and arrives by its army's units on the second front (before the
    second front's first round: when it starts; after: at once, in its
    stored state). On a ground front (Lightning Strikes) a unit on its army's
    HQ or base may go, and arrives by the army's HQ there."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    fighters = [u for u in g.units(1) if u["type"] == 16]
    others = [u for u in g.units(1) if u["type"] not in (16, 17, 12, 13)]
    names = tf.command(e, g, d, others[0]["x"], others[0]["y"], "Send")
    ctx.check("Send" not in names, f"no Send for a {others[0]['type']} ({names})")
    f = fighters[0]
    count = len(g.units(1))
    names = tf.command(e, g, d, f["x"], f["y"], "Send")
    ctx.check("Send" in names, f"Send for a Fighter ({names})")
    shot(ctx, e, "sent")
    ctx.eq(len(g.units(1)), count - 1, "the Fighter has left the main front")
    ctx.eq(g.unit_at(f["x"], f["y"]), None, "its cell empty")
    ctx.eq(e.u8(tf.QUEUED), 1, "on its way to the second front (not started yet)")
    first_round(ctx, e, d)
    fl = [u for u in tf.store_units(e) if u[0] == 1]
    ctx.eq(len(fl), 4, f"three Fighters of its own and the one sent on the second front ({fl})")
    # After the second front's start: straight into its state.
    f = [u for u in g.units(1) if u["type"] in (16, 17)][0]
    tf.command(e, g, d, f["x"], f["y"], "Send")
    fl2 = [u for u in tf.store_units(e) if u[0] == 1]
    ctx.eq(len(fl2), 5, "a second unit sent, now in the stored second front")
    new = [u for u in fl2 if u not in fl]
    ctx.check(new and new[0][1] == f["type"] and new[0][2] <= 2, f"by the army's units at the map's left edge ({new})")
    e.close()
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    hq = next((x, y) for y in range(d.size()[1]) for x in range(d.size()[0]) if g.terrain_class(x, y) == 0x08 | 1 << 5)
    u = next(u for u in g.units(1) if u["type"] in (1, 2))
    if g.unit_at(*hq) is None:
        d.place_unit(u, *hq)
    u = g.unit_at(*hq)
    names = tf.command(e, g, d, hq[0], hq[1], "Send")
    ctx.check("Send" in names, f"Send from the HQ ({names})")
    other = next(v for v in g.units(1) if g.terrain_class(v["x"], v["y"]) & 0x1F not in (0x08, 0x0E, 0x0A, 0x0B))
    names = tf.command(e, g, d, other["x"], other["y"], "Wait")
    ctx.check("Send" not in names, f"no Send off the HQ and bases ({names})")
    first_round(ctx, e, d)
    mine = [v for v in tf.store_units(e) if v[0] == 1]
    ctx.check(any(v[1] == u["type"] for v in mine), f"the unit sent on the second front ({mine})")


@test(modes=("ds",))
def two_front_cpu_directs(ctx):
    """The computer directs the player's army on the second front: an enemy
    Fighter set next to its Fighters is attacked in its round."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    first_round(ctx, e, d)
    bh = tf.store_unit_addrs(e, 2)[0]
    mine = tf.store_unit_addrs(e, 1)[0]
    e.w8(bh + 2, e.u8(mine + 2) + 1)
    e.w8(bh + 3, e.u8(mine + 3))
    hp = e.u8(bh + 4) & 0x7F
    tf.end_round(e, d)
    after = [u for u in tf.store_units(e) if u[0] == 2]
    ctx.check(len(after) < 6 or any(u[4] < hp for u in after), f"the enemy Fighter attacked by the computer ({after})")


# Each front's look (crate::wasteland's biome, in the weather block): the
# main front's its mission's (Means to an End's its own palette, 4), every
# second front Normal (0), as Dual Strike's top screen draws them; and each
# second front's deployment, its units per army as Dual Strike's record.
LOOK = 0x03004493
LOOKS = {VICTORY_OR_DEATH: 2, LIGHTNING_STRIKES: 0, OMENS_AND_SIGNS: 0, RING_OF_FIRE: 0, MEANS_TO_AN_END: 4}
DEPLOYED = {
    VICTORY_OR_DEATH: {1: 3, 2: 6},
    LIGHTNING_STRIKES: {1: 8, 2: 10},
    OMENS_AND_SIGNS: {1: 4, 2: 8},
    RING_OF_FIRE: {1: 9, 2: 16},
    MEANS_TO_AN_END: {1: 10, 2: 22},
}


def look(e):
    return e.u8(LOOK) >> 4 & 7


@test(modes=("ds",))
def two_front_looks_and_deployments(ctx):
    """Each front is drawn in its own look, set at every swap and the view
    and put back; each second front starts with Dual Strike's deployment."""
    for m in (VICTORY_OR_DEATH, MEANS_TO_AN_END, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE):
        e, g, d = start(ctx, m)
        ctx.eq(look(e), LOOKS[m], f"mission {m}: the main front's look")
        # The structures' picture and colours (OBJ tile 0x130, palette 10:
        # crate::sky_front borrows them on a front in the sky).
        structures = lambda: (e.read(0x06010000 + 32 * 0x130, 0x800), e.read(0x05000200 + 32 * 10, 32))
        before = structures()
        g.wait_for_input()
        tf.look_at_other_front(e, g)
        ctx.eq(look(e), 0, f"mission {m}: the second front looked at: Normal")
        tf.come_back(e, g)
        ctx.eq(look(e), LOOKS[m], f"mission {m}: back: the main front's look")
        d.end_turn()
        ctx.check(tf.until(e, d, lambda: e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0), f"mission {m}: the second front's round")
        ctx.eq(look(e), 0, f"mission {m}: the second front's round: Normal")
        sky = m in (VICTORY_OR_DEATH, OMENS_AND_SIGNS)
        e.wait(10)
        if sky:
            # (crate::sky_front: the Black Arc's picture and colours, the
            # pack's bmap/0a7 and 0aa)
            ds = DualStrike()
            ctx.check(structures() == (ds.file("bmap/0a7")[:0x800], ds.file("bmap/0aa")[:32]), f"mission {m}: the Black Arc's picture and colours in the sky")
        ctx.eq({a: n for a in (1, 2, 3, 4) if (n := len(g.units(a)))}, DEPLOYED[m], f"mission {m}: the second front's deployment")
        if m == MEANS_TO_AN_END:
            shot(ctx, e, "means_to_an_end_second_front")
        ctx.check(tf.until(e, d, lambda: tf.player_turn(e) and e.u8(tf.LIVE) == 0), f"mission {m}: back to the main front")
        ctx.eq(look(e), LOOKS[m], f"mission {m}: the main front's look again")
        # (after a front in the sky: its colours, and the picture where the
        # main front draws one: Omens and Signs' fortress)
        now = structures()
        ctx.check(not sky or (now[1] == before[1] and (m != OMENS_AND_SIGNS or now[0] == before[0])), f"mission {m}: the main front's structures as they were")
        e.close()


@test(modes=("ds",))
def two_front_saved_halfway(ctx):
    """A mission saved halfway (map menu > Save) keeps both fronts: after a
    reboot DS CAMPAIGN's Continue brings the main front back as saved and the
    second front's state (its whole block, its tangoAW2 state, the battle's
    two-front state) with it; the next round plays it on."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    first_round(ctx, e, d)
    state = e.read(tf.STATE, tf.STATE_LEN + 5)
    store = e.read(tf.STORE, tf.BLOCK_LEN + 0x6F)
    names = saves.suspend(g)
    ctx.check("Save" in names and "Front" in names, f"Save and Front on the main front ({names})")
    snap0 = saves.snapshot(g)
    img = saves.flash(e, os.path.join(ctx.out, "saved"))
    ctx.check(not img.problems(), f"every slot passes AW2's check {img.problems()}")
    ctx.check(len(img.slot(14) or b"") > 0xE28, f"slot 14 holds both fronts ({len(img.slot(14) or b'')} bytes)")
    e.close()
    e, g, d = boot(ctx, img.path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "Continue: the battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    saves.compare_snapshots(ctx, snap0, saves.snapshot(g), "the main front after a reboot")
    ctx.eq(e.read(tf.STATE, tf.STATE_LEN + 5), state, "the two fronts' state after a reboot")
    ctx.eq(e.read(tf.STORE, tf.BLOCK_LEN + 0x6F), store, "the second front after a reboot")
    seen = tf.end_round(e, d)
    ctx.eq([s[0] for s in seen][-3:], [1, 1, 0], f"the next round on the second front, as saved ({seen})")
    ctx.eq(e.u16(tf.STORE + tf.B_DAY), 2, "its day 2")


# -- each mission's conditions -----------------------------------------------------------
# Dual Strike's records per front (overlay 1, read in ds_campaign_data's
# conversion): a condition's op is 3 * AW2's op + its front (0 main, 1 second),
# a fire's 0x17 + its front. AW2's own rules (an army routed, an HQ taken) end
# a front as Dual Strike's do.

LAST_RESULT = dc.LAST_RESULT


def won_on_main(ctx, e, d, label):
    ctx.eq(tf.mission_over(e, d), 1, f"{label}: the mission is won")


def lost_on_main(ctx, e, d, label):
    ctx.eq(tf.mission_over(e, d), 2, f"{label}: the mission is lost")


def second_front_ends(ctx, e, d, g, result, label, setup, army=None):
    """The player ends the turn; when the second front's round begins (its
    first army's turn, on the screen) `setup` arranges its state; the round
    is played. Then: the second front is over (2 won, 3 lost), the main front
    back for good, the mission going on; its result shown; Front gone from
    the map menu; the main front's days go on alone."""
    seen = []
    # (set up as the army whose computer acts on it starts its turn: the
    # player's army to win it, the enemy's to lose it)
    army = army or (1 if result == tf.SECOND_WON else 2)
    d.end_turn()
    ok = tf.until(e, d, lambda: e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0 and e.u16(tf.CURRENT_ARMY) == army
                  and e.u16(tf.MAP_STATE) in (4, 5, 6, 7, 8, 9, 10, 11, 12))
    ctx.require(ok, f"{label}: the second front's round")
    setup()
    tf.until(e, d, lambda: tf.player_turn(e) or e.u8(LAST_RESULT) != 0,
             each=lambda: seen.append(e.u8(tf.BANNER)) if e.u8(tf.BANNER) else None)
    ctx.eq(e.u8(tf.SECOND), result, f"{label}: the second front {'won' if result == tf.SECOND_WON else 'lost'}")
    ctx.check(e.u8(tf.LIVE) == 0 and e.u8(LAST_RESULT) == 0 and d.in_battle(), f"{label}: the mission goes on, on the main front")
    ctx.check(bool(seen), f"{label}: its result shown")
    d.wait_control()
    names = tf.map_menu_names(g)
    ctx.check("Front" not in names, f"{label}: no Front once it is over ({names})")
    day = e.u16(tf.DAY)
    seen2 = tf.end_round(e, d)
    ctx.check(all(s[0] == 0 for s in seen2) and e.u16(tf.DAY) == day + 1, f"{label}: the main front's days go on alone ({seen2})")


DIRECT = (1, 2, 3, 4, 5, 6, 8)
AIR_DIRECT = (16, 19)


def kill_setup(d, victim_army, killer_army, keep_others=True):
    """On the front on the screen: a unit of `victim_army`, on 1 HP, next to
    one of `killer_army`'s able to hit it at once, which its computer does;
    with `keep_others` False the victim's army has no other unit (AW2's
    rout when it falls)."""
    g, e = d.g, d.e
    killers = [u for u in g.units(killer_army) if u["type"] in DIRECT + AIR_DIRECT]
    killer = killers[0] if killers else g.units(killer_army)[0]
    if killer["type"] not in DIRECT + AIR_DIRECT:
        e.w8(g.unit_addr(killer["id"]), 4)  # a Tank
        killer["type"] = 4
    air = killer["type"] in AIR_DIRECT
    w, h = d.size()
    spot = None
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        x, y = killer["x"] + dx, killer["y"] + dy
        if 0 <= x < w and 0 <= y < h and e.u8(d.layer_cell(x, y)) == 0 and (air or g.terrain_class(x, y) & 0x1F in dc.LAND):
            spot = (x, y)
            break
    victims = g.units(victim_army)
    victim = victims[0]
    if not keep_others:
        for u in victims[1:]:
            d.remove_unit(u)
    d.place_unit(victim, *spot)
    a = g.unit_addr(victim["id"])
    e.w8(a, 16 if air else 1)
    e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 1)


def structures_down(d, which, keep=0):
    """On the front on the screen: its inventions of kind 4 (minicannons,
    Black Crystals) chosen by `which` brought to `keep` HP; and, for its
    after-action records to be tested, an action there (an enemy unit to
    finish off)."""
    for a, x, y, kind, hp in tf.inventions(d.e):
        if kind == 4 and which(x, y):
            d.e.w8(a + 4, keep)
    kill_setup(d, 2, 1)


def round_with(ctx, e, d, setup):
    """A round in which `setup` arranges the second front once its round
    begins."""
    d.end_turn()
    ok = tf.until(e, d, lambda: e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0 and e.u16(tf.CURRENT_ARMY) == 1
                  and e.u16(tf.MAP_STATE) in (4, 5, 6, 7, 8, 9, 10, 11, 12))
    ctx.require(ok, "the second front's round")
    setup()
    tf.until(e, d, lambda: tf.player_turn(e) or e.u8(LAST_RESULT) != 0)
    d.wait_control()


def main_structures(e, which):
    return [i for i in tf.inventions(e) if i[3] == 4 and which(i[1], i[2])]


def tile(e, x, y):
    row = e.u16(dc.MAP + 0x417A + 2 * y)
    return e.u16(dc.MAP + 0xA22 + 2 * (row + x))


@test(modes=("ds",))
def two_front_victory_or_death(ctx):
    """Victory or Death! (Dual Strike's records): main front won when every
    Black Crystal there is destroyed (after an action, 0x023505E8: Black Hole
    loses), or by AW2's rules; lost by AW2's rules. Second front (the Black
    Arc, in the sky): won when its four minicannons are destroyed
    (0x02350610), lost when the player's units there are routed. Across
    fronts: Black Hole's Black Arc drops its bomb on the main front's (13, 5)
    each of its turns until the second front is won (the bomb's rule tests
    the battle's flag 2, which the second front's winning record sets: both
    fronts share the battle's flags); won, the survivors charge the main
    front's power meter."""
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    cp0 = tf.checkpoint(e, ctx, "start")
    # Main front won: its Black Crystal(s) destroyed.
    crystals = main_structures(e, lambda x, y: tile(e, x, y) == 0x192)
    ctx.check(len(crystals) >= 1, f"the main front's Black Crystal ({crystals})")
    for a, *_ in crystals:
        e.w8(a + 4, 0)
    tf.act(e, g, d)
    won_on_main(ctx, e, d, "every Black Crystal destroyed")
    tf.back_to(e, g, cp0)
    ctx.require(tf.rout_player(d), "the player's last unit set up")
    d.end_turn()
    lost_on_main(ctx, e, d, "the player's army routed")
    tf.back_to(e, g, cp0)
    first_round(ctx, e, d)
    cp1 = tf.checkpoint(e, ctx, "second_started")
    flag2 = lambda: e.u8(0x030033F4) >> 2 & 1
    ctx.eq(flag2(), 0, "the Black Arc's flag (2) clear while it flies")
    p = g.players_base + 0x3C
    charge = e.u32(p + 0x20)
    second_front_ends(ctx, e, d, g, tf.SECOND_WON, "its four minicannons destroyed", lambda: structures_down(d, lambda x, y: True))
    ctx.eq(flag2(), 1, "the battle's flag 2 set by the second front's record: the bombing stops")
    ctx.check(e.u32(p + 0x20) > charge, f"the survivors charge the power meter ({charge} -> {e.u32(p + 0x20)})")
    ctx.eq(e.u8(tf.WINNER), 1, "the second front's winner: the player's army (the tag-pair hook's)")
    tf.back_to(e, g, cp1)
    second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "the player's units there routed", lambda: kill_setup(d, 1, 2, keep_others=False))
    ctx.eq(flag2(), 0, "lost: the Black Arc flies on")


@test(modes=("ds",))
def two_front_lightning_strikes(ctx):
    """Lightning Strikes: Dual Strike's records have no condition of their
    own for its fronts: each is won and lost by AW2's rules (an army routed,
    an HQ taken). Main front won (the enemy routed) and lost (the player
    routed); second front won (the enemy routed there) and lost (the player
    routed there)."""
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    cp0 = tf.checkpoint(e, ctx, "start")
    ctx.require(tf.rout_enemy(d), "the enemy's last unit destroyed")
    won_on_main(ctx, e, d, "the enemy routed")
    tf.back_to(e, g, cp0)
    ctx.require(tf.rout_player(d), "the player's last unit set up")
    d.end_turn()
    lost_on_main(ctx, e, d, "the player routed")
    tf.back_to(e, g, cp0)
    first_round(ctx, e, d)
    cp1 = tf.checkpoint(e, ctx, "second_started")
    second_front_ends(ctx, e, d, g, tf.SECOND_WON, "the enemy routed on the second front", lambda: kill_setup(d, 2, 1, keep_others=False))
    tf.back_to(e, g, cp1)
    second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "the player routed on the second front", lambda: kill_setup(d, 1, 2, keep_others=False))


@test(modes=("ds",))
def two_front_omens_and_signs(ctx):
    """Omens and Signs: main front won when its ocean fortress's four
    minicannons are destroyed (0x02350610 on the main front), lost by AW2's
    rules; the fortress is shielded while the Black Arc stands (Dual Strike:
    "Black Hole's utilizing a barrier field ... energy is flowing from the
    Black Arc"): a hit is undone and every minicannon down does not win.
    Second front won when the Black Arc's minicannons are destroyed ("Black
    Arc fatal error. Ocean fortress barrier collapsing."): the barrier falls,
    and the fortress can be destroyed; lost when the player is routed
    there."""
    e, g, d = start(ctx, OMENS_AND_SIGNS)
    cp0 = tf.checkpoint(e, ctx, "start")
    fort = main_structures(e, lambda x, y: tile(e, x, y) not in (0x192, 0x194))
    ctx.eq(len(fort), 4, f"the ocean fortress's four minicannons ({fort})")
    hp0 = [h for *_, h in fort]
    e.w8(fort[0][0] + 4, 30)
    e.wait(4)
    ctx.eq(e.u8(fort[0][0] + 4), hp0[0], "shielded: a hit undone")
    for a, *_ in fort:
        e.w8(a + 4, 0)
    e.wait(4)
    tf.act(e, g, d)
    e.wait(60)
    ctx.check(e.u8(LAST_RESULT) == 0 and [e.u8(a + 4) for a, *_ in fort] == hp0, "shielded: no win, every minicannon whole again")
    ctx.require(tf.rout_player(d), "the player's last unit set up")
    d.end_turn()
    lost_on_main(ctx, e, d, "the player routed")
    tf.back_to(e, g, cp0)
    first_round(ctx, e, d)
    cp1 = tf.checkpoint(e, ctx, "second_started")
    second_front_ends(ctx, e, d, g, tf.SECOND_WON, "the Black Arc's minicannons destroyed", lambda: structures_down(d, lambda x, y: True))
    fort = main_structures(e, lambda x, y: tile(e, x, y) not in (0x192, 0x194))
    e.w8(fort[0][0] + 4, 30)
    e.wait(4)
    ctx.eq(e.u8(fort[0][0] + 4), 30, "the barrier down: a hit stays")
    for a, *_ in fort:
        e.w8(a + 4, 0)
    tf.act(e, g, d)
    won_on_main(ctx, e, d, "the fortress destroyed")
    tf.back_to(e, g, cp1)
    second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "the player routed on the second front", lambda: kill_setup(d, 1, 2, keep_others=False))


RING_OF_FIRE_SECOND_CELLS = [(7, 2), (9, 2), (10, 2), (11, 3), (11, 4), (11, 6), (10, 7), (8, 7), (7, 7), (6, 6), (6, 5), (6, 3)]
VOLCANO_STILL = 0x0203F704
ERUPTION_CELLS = 0x0203F708


@test(modes=("ds",))
def two_front_ring_of_fire(ctx):
    """Ring of Fire: main front lost on day 18 (Dual Strike's turn-start
    record for Black Hole: it wins), won by AW2's rules. Second front won by
    AW2's rules: its record then stills the main front's Volcano (Dual
    Strike's 0x023518CC: "We've seized a volcano-controlling unit!"); lost
    when the player is routed there. Each front's Volcano erupts on its own
    cells (Dual Strike's eruption lists by front: list 1 the main front's,
    list 2 the second's)."""
    e, g, d = start(ctx, RING_OF_FIRE)
    cp0 = tf.checkpoint(e, ctx, "start")
    ctx.eq(e.u16(DS_TABLE + 0x5C * dc.DS_MAP_ID + 0x24), 18, "the day limit: 18")
    e.w16(tf.DAY, 18)
    d.end_turn()
    lost_on_main(ctx, e, d, "day 18")
    tf.back_to(e, g, cp0)
    ctx.require(tf.rout_enemy(d), "the enemy's last unit destroyed")
    won_on_main(ctx, e, d, "the enemy routed")
    tf.back_to(e, g, cp0)
    first_round(ctx, e, d)
    cp1 = tf.checkpoint(e, ctx, "second_started")
    # The second front's Volcano: its own cells (from its day 3).
    e.w16(tf.DAY, 3)
    e.w16(tf.STORE + tf.B_DAY, 2)
    cells = []

    def each():
        if e.u8(tf.LIVE) == 1 and e.u16(ERUPTION_CELLS) != 0:
            cells.append([(e.u16(ERUPTION_CELLS + 4 * k), e.u16(ERUPTION_CELLS + 4 * k + 2)) for k in range(12)])

    for _ in range(2):
        tf.end_round(e, d, each=each)
    ctx.check(any(c == RING_OF_FIRE_SECOND_CELLS for c in cells), f"the second front's eruption on Dual Strike's list 2 ({cells[-1:] if cells else 'none'})")
    tf.back_to(e, g, cp1)
    ctx.eq(e.u8(VOLCANO_STILL), 0, "the main front's Volcano active")
    second_front_ends(ctx, e, d, g, tf.SECOND_WON, "the enemy routed on the second front", lambda: kill_setup(d, 2, 1, keep_others=False))
    ctx.eq(e.u8(VOLCANO_STILL), 1, "its record stills the main front's Volcano")
    tf.back_to(e, g, cp1)
    second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "the player routed on the second front", lambda: kill_setup(d, 1, 2, keep_others=False))
    ctx.eq(e.u8(VOLCANO_STILL), 0, "lost: the Volcano erupts on")


MTE_CRYSTALS = [(1, 1), (8, 1), (14, 1)]   # ds_campaign_data::MTE_CRYSTALS (second front, west to east)
WEAK_POINTS = [(3, 9), (9, 11), (15, 9)]


@test(modes=("ds",))
def two_front_means_to_an_end(ctx):
    """Means to an End: main front lost on day 24 (Dual Strike's limit,
    restored: its counter, its record and its texts), won when the Grand
    Bolt's three weak points are destroyed (0x02350560). Its second front
    holds the three Black Crystals (none on the main map): each shattered
    opens one weak point (west to east: until then it is shielded, a hit
    undone); every crystal shattered wins the second front (0x023505E8);
    lost when the player is routed there."""
    e, g, d = start(ctx, MEANS_TO_AN_END)
    cp0 = tf.checkpoint(e, ctx, "start")
    ctx.eq(e.u16(DS_TABLE + 0x5C * dc.DS_MAP_ID + 0x24), 24, "the day limit: Dual Strike's 24")
    w, h = d.size()
    ctx.check(not any(tile(e, x, y) == 0x192 for y in range(h) for x in range(w)), "no Black Crystal on the main map")
    e.w16(tf.DAY, 24)
    d.end_turn()
    lost_on_main(ctx, e, d, "day 24")
    tf.back_to(e, g, cp0)
    first_round(ctx, e, d)
    cp1 = tf.checkpoint(e, ctx, "second_started")
    crystals = [i for i in tf.store_inventions(e) if i[3] == 4]
    ctx.eq(sorted((x, y) for _, x, y, _, _ in crystals), sorted(MTE_CRYSTALS), "the three Black Crystals on the second front")
    weak = main_structures(e, lambda x, y: (x, y) in WEAK_POINTS)
    ctx.eq(len(weak), 3, "the Grand Bolt's three weak points on the main front")
    hp = e.u8(weak[0][0] + 4)
    e.w8(weak[0][0] + 4, hp - 30)
    e.wait(4)
    ctx.eq(e.u8(weak[0][0] + 4), hp, "a weak point whose crystal stands: a hit undone")
    # The west crystal shattered on the second front.
    round_with(ctx, e, d, lambda: structures_down(d, lambda x, y: (x, y) == MTE_CRYSTALS[0]))
    ctx.eq(e.u8(tf.SECOND), 1, "one crystal down: the second front goes on")
    west = next(w for w in main_structures(e, lambda x, y: (x, y) == WEAK_POINTS[0]))
    centre = next(w for w in main_structures(e, lambda x, y: (x, y) == WEAK_POINTS[1]))
    e.w8(west[0] + 4, west[4] - 30)
    e.w8(centre[0] + 4, centre[4] - 30)
    e.wait(4)
    ctx.eq(e.u8(west[0] + 4), west[4] - 30, "the west weak point open (its crystal shattered)")
    ctx.eq(e.u8(centre[0] + 4), centre[4], "the centre one still shielded")
    cp2 = tf.checkpoint(e, ctx, "one_crystal")
    second_front_ends(ctx, e, d, g, tf.SECOND_WON, "every crystal shattered", lambda: structures_down(d, lambda x, y: True))
    for wp in main_structures(e, lambda x, y: (x, y) in WEAK_POINTS):
        e.w8(wp[0] + 4, 0)
    tf.act(e, g, d)
    won_on_main(ctx, e, d, "the Grand Bolt's weak points destroyed")
    tf.back_to(e, g, cp2)
    second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "the player routed on the second front", lambda: kill_setup(d, 1, 2, keep_others=False))
