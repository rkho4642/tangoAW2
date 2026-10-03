"""Intel > Auto CO (crate::two_front, docs/AW2.md "Two fronts"): in Dual
Strike's Lightning Strikes and Ring of Fire the player may command the
second front. Found in melonDS: the Intel menu's last item, "Auto CO On"
(help "Allow CPU to direct the secondary front.") at the start, chosen
"Auto CO Off" ("Direct the secondary front manually."), on the main front
only, any time; off, the player's army's turn on the second front waits for
the player, with that front's CO, funds and units, and its end goes on to
Black Hole's turn there and back to the main front. No bot play."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, saves
from aw2test import twofront as tf
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END = 8, 10, 14, 21, 24
TWO_FRONTS = (VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END)


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def start(ctx, mission):
    """The mission from the DS Campaign's world map to the player's first
    turn (the Setup phase's Deploy chosen)."""
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(mission))
    d.wait_map()
    ctx.require(d.mission() == mission and tf.state(e)["second"] == 1, f"mission {mission} on two fronts ({tf.state(e)})")
    return e, g, d


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


ALLOWED = (LIGHTNING_STRIKES, RING_OF_FIRE)
HELP = {tf.AUTO_ON: "Allow CPU to direct the secondary front.", tf.AUTO_OFF: "Direct the secondary front manually."}
PANEL = tf.STATE + 0x18   # the panel drawn: 5 Auto CO's help line


def human_turn(e):
    """The second front, army 1's turn, the cursor waiting for the pad."""
    return (e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0 and e.u16(tf.CURRENT_ARMY) == 1
            and e.u16(tf.MAP_STATE) == tf.CURSOR_STATE)


def to_human_turn(ctx, e, d, g, label):
    """The player ends the main front's turn; Black Hole plays; the second
    front's round starts with the player's army waiting for the player."""
    seen = []
    d.end_turn()
    e.wait(30)
    ok = tf.until(e, d, lambda: human_turn(e), each=lambda: seen.append((e.u8(tf.LIVE), e.u16(tf.CURRENT_ARMY))))
    ctx.require(ok, f"{label}: the player's turn on the second front ({tf.state(e)})")
    d.wait_control()
    g._units_base = g._players_base = None
    return seen


@test(modes=("ds",))
def two_front_auto_co_only_where_ds_has_it(ctx):
    """Intel's Auto CO item exists where Dual Strike's does: Lightning Strikes
    and Ring of Fire (its test reads their map records, 0xEA and 0xF5), "Auto
    CO On" at the start; Victory or Death!, Omens and Signs and Means to an
    End keep AW2's campaign Intel (Status, Terms, Unit); a one-front mission and
    Versus have the game's Intel menu itself."""
    for m in TWO_FRONTS:
        e, g, d = start(ctx, m)
        names = tf.intel(g)["names"]
        if m in ALLOWED:
            ctx.eq(names, ["Status", "Terms", "Unit", tf.AUTO_ON], f"mission {m}: Intel with Auto CO, on")
            ctx.eq(e.u8(tf.MANUAL), 0, f"mission {m}: Auto CO on at the start")
        else:
            ctx.eq(names, ["Status", "Terms", "Unit"], f"mission {m}: AW2's Intel, no Auto CO")
        e.close()
    e, g, d = boot(ctx)
    d.start(step=0)
    d.wait_map()
    ctx.eq(e.u32(tf.INTEL_MENU_POOL), tf.GAME_INTEL_MENU, "Jake's Trial: the game's Intel menu")
    m = tf.intel(g)
    ctx.eq((m["table"], m["names"]), (tf.GAME_INTEL_MENU, ["Status", "Terms", "Unit"]), "Jake's Trial: AW2's Intel")
    e.close()
    mp = ctx.map()
    mp.unit(1, "tank", 5, 5).unit(2, "tank", 20, 10)
    g = ctx.start(mp, ["andy", "drake"])
    ctx.eq(g.e.u32(tf.INTEL_MENU_POOL), tf.GAME_INTEL_MENU, "Versus: the game's Intel menu")


@test(modes=("ds",))
def two_front_auto_co_off_human_plays(ctx):
    """Lightning Strikes, Intel > Auto CO chosen: the item reads "Auto CO Off"
    (redrawn in place, Dual Strike's help line under it). The player ends the
    turn: Black Hole's main-front turn, then the second front with the
    player's army waiting for the player: its controller the player's, its
    CO the second front's (the player's pick), its own funds; its map menu
    Dual Strike's there (CO, Intel without Auto CO, Options, Save, End; no
    Power), with Front, which shows the main front and comes back to the
    turn as it was. A unit moves and
    waits; End: Black Hole's turn on the second front (the computer's), then
    day 2 on the main front, the moved unit kept where it went. Auto CO
    turned on again: the next second-front round is the computer's."""
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    shots = {}

    def menu_shot(name):
        e.wait(10)
        shot(ctx, e, f"intel_{name}")
        shots[name] = e.u8(PANEL)

    before, after = tf.set_auto_co(g, False, shot=menu_shot)
    ctx.eq((before, after), (tf.AUTO_ON, tf.AUTO_OFF), "Auto CO On, chosen: Auto CO Off")
    ctx.eq(shots, {"before": 5, "after": 5}, "Auto CO's help line under the menu")
    ctx.eq(e.u8(tf.MANUAL), 1, "army 1's Auto CO off")
    ctx.eq(tf.intel(g)["names"][-1], tf.AUTO_OFF, "the setting kept")
    main_co = e.u8(g.players_base + 0x3C + 0x1D)
    seen = to_human_turn(ctx, e, d, g, "Auto CO off")
    ctx.check((0, 2) in seen, f"Black Hole's main-front turn before ({seen})")
    shot(ctx, e, "human_second_front")
    p = g.players_base + 0x3C
    ctx.eq(e.u8(p + 0x1B), 1, "army 1's controller on the second front: the player's")
    ctx.eq(e.u8(g.players_base + 2 * 0x3C + 0x1B), 2, "Black Hole's there: the computer's")
    ctx.eq(e.u8(p + 0x1D), e.u8(tf.SECOND_COS), "its CO: the second front's (the player's pick)")
    ctx.check(e.u8(p + 0x1D) != main_co, f"not the main front's ({main_co})")
    funds, main_funds = e.u32(p), e.u32(tf.STORE + tf.B_PLAYERS + 0x3C)
    ctx.check(funds != main_funds, f"its own funds: {funds} (the main front's {main_funds})")
    names = tf.map_menu_names(g)
    ctx.eq(names, ["CO", "Intel", "Options", "Front", "Save", "End"], "the second front's map menu: Dual Strike's (no Power there), with Front")
    names = tf.intel(g)["names"]
    ctx.check(names and not any(n.startswith("Auto CO") for n in names), f"no Auto CO on the second front ({names})")
    # Front: the main front looked at, and back to the turn.
    before = saves.snapshot(g)
    ctx.require(tf.look_at_other_front(e, g), "Front: the main front shows")
    ctx.eq(e.u8(tf.LIVE), 0, "the main front on the screen")
    shot(ctx, e, "human_views_main_front")
    ctx.require(tf.come_back(e, g), "B: back")
    g._units_base = g._players_base = None
    saves.compare_snapshots(ctx, before, saves.snapshot(g), "back on the second front")
    ctx.check(human_turn(e), f"the player's turn there goes on ({tf.state(e)})")
    # A unit moves and waits.
    u, to = next(((u, c) for u in g.units(1) if u["type"] in (1, 2) and (c := tf.free_land_next_to(d, u["x"], u["y"]))), (None, None))
    ctx.require(to is not None, f"a foot soldier with a free cell next to it ({g.units(1)})")
    names = tf.command(e, g, d, u["x"], u["y"], "Wait", to=to)
    ctx.check("Wait" in names, f"Wait offered ({names})")
    ctx.check(g.unit_at(*to) is not None and g.unit_at(*to)["army"] == 1, f"the unit moved to {to}")
    shot(ctx, e, "human_moved")
    # End: Black Hole's turn there, then day 2 on the main front.
    seen = []
    day = e.u16(tf.DAY)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ok = tf.until(e, d, lambda: tf.player_turn(e), each=lambda: seen.append((e.u8(tf.LIVE), e.u16(tf.DAY), e.u16(tf.CURRENT_ARMY))))
    ctx.require(ok, f"back to the main front ({tf.state(e)})")
    d.wait_control()
    ctx.check((1, day, 2) in seen, f"Black Hole's turn on the second front ({sorted(set(seen))})")
    ctx.eq((e.u8(tf.LIVE), e.u16(tf.DAY), e.u16(tf.CURRENT_ARMY)), (0, day + 1, 1), "day 2 on the main front")
    ctx.check(any((a, x, y) == (1, to[0], to[1]) for a, _, x, y, _ in tf.store_units(e)), "the moved unit kept on the second front")
    # Auto CO on again: the computer's round.
    ctx.eq(tf.set_auto_co(g, True), (tf.AUTO_OFF, tf.AUTO_ON), "Auto CO Off, chosen: Auto CO On")
    ctx.eq(e.u8(tf.MANUAL), 0, "army 1's Auto CO on")
    seen = tf.end_round(e, d)
    ctx.eq(seen, [(0, 2, 2), (1, 2, 1), (1, 2, 2), (0, 3, 1)], "day 2: the computer plays the second front, then day 3")
    ctx.eq(e.u8(tf.STORE + tf.B_PLAYERS + 0x3C + 0x1B), 2, "army 1's controller there: the computer's")


@test(modes=("ds",))
def two_front_auto_co_in_setup(ctx):
    """In the Setup phase before day 1 (Dual Strike's: Setup, CO, Intel,
    Options, Save, Deploy, Intel with Auto CO): the Setup menu's Intel has
    Auto CO too; set off there, it holds once Deploy starts day 1."""
    e, g, d = boot(ctx)
    d.auto_deploy = False
    d.start(step=dc.ORDER.index(LIGHTNING_STRIKES))
    d.wait_map()
    ctx.require(d.in_setup(), "the Setup phase")
    d.setup_menu()
    g.choose("Intel", dc.SETUP_MENU)
    m = g.wait_menu(e.u32(tf.INTEL_MENU_POOL))
    names = [n.rstrip("\x1c") for n in m["names"]]
    ctx.eq(names, ["Status", "Terms", "Unit", tf.AUTO_ON], "the Setup menu's Intel: Auto CO On")
    g.choose("Auto CO", m["table"])
    e.wait(20)
    e.wait(10)
    shot(ctx, e, "setup_intel_off")
    ctx.eq([n.rstrip("\x1c") for n in g.menu()["names"]][-1], tf.AUTO_OFF, "chosen: Auto CO Off")
    tf.close_menus(g)
    d.auto_deploy = True
    d.wait_control()
    ctx.check(not d.in_setup(), "Deploy: day 1")
    ctx.eq(e.u8(tf.MANUAL), 1, "army 1's Auto CO off on day 1")
    to_human_turn(ctx, e, d, g, "set in the Setup phase")


@test(modes=("ds",))
def two_front_auto_co_on_cpu_plays(ctx):
    """Auto CO on (the start): the second front's round is the computer's,
    in both missions that have the item; nobody waits for the player."""
    for m in ALLOWED:
        e, g, d = start(ctx, m)
        seen = tf.end_round(e, d)
        ctx.eq(seen, [(0, 1, 2), (1, 1, 1), (1, 1, 2), (0, 2, 1)], f"mission {m}: day 1 on both fronts, the second front's armies the computer's")
        p = tf.STORE + tf.B_PLAYERS
        ctx.eq([e.u8(p + 0x3C * a + 0x1B) for a in (1, 2)], [2, 2], f"mission {m}: the second front's controllers")
        e.close()


@test(modes=("ds",))
def two_front_auto_co_saved(ctx):
    """Auto CO off, saved halfway (map menu > Save on the main front), a
    reboot, Continue: the setting is back (Intel reads "Auto CO Off") and the
    next second-front round waits for the player. Saved again on that turn
    (Dual Strike offers Save there too), a reboot, Continue: the player's
    turn on the second front comes back as saved (its map, units, players,
    the main front stored), and End goes on to Black Hole's turn there and
    the next day on the main front."""
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    tf.set_auto_co(g, False)
    saves.suspend(g)
    img = saves.flash(e, os.path.join(ctx.out, "saved"))
    ctx.check(not img.problems(), f"every slot passes AW2's check {img.problems()}")
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
    ctx.eq(e.u8(tf.MANUAL), 1, "army 1's Auto CO off after a reboot")
    ctx.eq(tf.intel(g)["names"][-1], tf.AUTO_OFF, "Intel: Auto CO Off")
    to_human_turn(ctx, e, d, g, "after Continue")
    ctx.eq(e.u8(g.players_base + 0x3C + 0x1B), 1, "the player's turn on the second front")
    # Saved there (Dual Strike's menu has Save on that turn too): Continue
    # comes back to it, on the second front's map, and the round goes on.
    names = saves.suspend(g)
    ctx.check("Save" in names, f"Save on the player's second-front turn ({names})")
    snap0 = saves.snapshot(g)
    state = e.read(tf.STATE, tf.STATE_LEN + 5)
    store = e.read(tf.STORE, tf.BLOCK_LEN)
    img = saves.flash(e, os.path.join(ctx.out, "saved2"))
    ctx.check(not img.problems(), f"every slot passes AW2's check {img.problems()}")
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
    shot(ctx, e, "continued_on_second_front")
    ctx.check(human_turn(e), f"Continue: the player's turn on the second front ({tf.state(e)})")
    saves.compare_snapshots(ctx, snap0, saves.snapshot(g), "the second front after a reboot")
    ctx.eq(e.read(tf.STATE, tf.STATE_LEN + 5), state, "the two fronts' state after a reboot")
    ctx.eq(e.read(tf.STORE, tf.BLOCK_LEN), store, "the main front (stored) after a reboot")
    seen = []
    day = e.u16(tf.DAY)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ok = tf.until(e, d, lambda: tf.player_turn(e), each=lambda: seen.append((e.u8(tf.LIVE), e.u16(tf.DAY), e.u16(tf.CURRENT_ARMY))))
    ctx.require(ok, f"End: back to the main front ({tf.state(e)})")
    ctx.check((1, day, 2) in seen, f"Black Hole's turn on the second front ({sorted(set(seen))})")
    ctx.eq((e.u8(tf.LIVE), e.u16(tf.DAY)), (0, day + 1), "the next day on the main front")
