"""Intel > General (crate::ally_posture, docs/AW2.md "Two fronts"): Dual
Strike's ally posture. Found in melonDS and its arm9: in every two-front
mission the main front's Intel menu has, after Unit, one of four items
(Strike, Assault, General, Defense: the army's posture, the AI icon), General
at the start; A turns it to the next (General, Defense, Strike, Assault) with
the menu up, its help line Dual Strike's ("Set ally to aggressive / offensive
/ general, all-purpose / defensive posture."); not on the second front's
Intel, not once the second front is over. The computer directs the army's
second-front turns by it: here, AW2's CPU moves the army's units by the
posture's role (Strike 4, toward enemy units; Assault 3, onto enemy
properties; Defense 0, holding; General their own). No bot play."""

import os

from aw2test import dscampaign as dc
from aw2test import paths, saves
from aw2test import twofront as tf
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END = 8, 10, 14, 21, 24
TWO_FRONTS = (VICTORY_OR_DEATH, LIGHTNING_STRIKES, OMENS_AND_SIGNS, RING_OF_FIRE, MEANS_TO_AN_END)
AUTO_CO = (LIGHTNING_STRIKES, RING_OF_FIRE)
PANEL = tf.STATE + 0x18   # the panel drawn: 7 General's help line
HELP_PANEL = 7
ICON_TILES = 0x06000000 + 32 * (0x1B4 + 4 * (0xE6 - 0x80))   # the icon's BG0 tiles (code 0xE6)
FOOT_AND_TRANSPORT = (1, 2, 7)   # Infantry, Mech, APC: their moves are not the roles'


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g, dc.DsCampaign(g)


def start(ctx, mission, poke=()):
    e, g, d = boot(ctx)
    d.start(step=dc.ORDER.index(mission), poke=poke)
    d.wait_map()
    ctx.require(d.mission() == mission and tf.state(e)["second"] == 1, f"mission {mission} on two fronts ({tf.state(e)})")
    return e, g, d


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


def continue_saved(ctx, path):
    e, g, d = boot(ctx, path)
    d.open_campaign_box()
    d.chooser_row(1)
    e.press("A", 8)
    e.wait(30)
    d.box_row(0)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1200, step=10), "Continue: the battle")
    g._units_base = g._players_base = None
    g.wait_for_input()
    return e, g, d


@test(modes=("ds",))
def two_front_general_where_ds_has_it(ctx):
    """Intel > General in each of the five two-front missions (Dual Strike's
    test asks only for a second front), after Unit, before Auto CO, reading
    General at the start for both armies; kept with Auto CO off; not in the
    second front's Intel (the player's turn there, Auto CO off); gone once
    the second front is over (with Auto CO's item); a one-front mission and
    Versus have the game's Intel."""
    for m in TWO_FRONTS:
        e, g, d = start(ctx, m)
        names = tf.intel(g)["names"]
        want = ["Status", "Terms", "Unit", "General"] + ([tf.AUTO_ON] if m in AUTO_CO else [])
        ctx.eq(names, want, f"mission {m}: Intel")
        ctx.eq((tf.posture(e, 1), tf.posture(e, 2)), (tf.GENERAL, tf.GENERAL), f"mission {m}: General at the start")
        if m == LIGHTNING_STRIKES:
            tf.set_auto_co(g, False)
            ctx.eq(tf.intel(g)["names"], ["Status", "Terms", "Unit", "General", tf.AUTO_OFF], "Auto CO off: General kept")
            seen = []
            d.end_turn()
            e.wait(30)
            ok = tf.until(e, d, lambda: e.u8(tf.LIVE) == 1 and e.u8(tf.BUSY) == 0 and e.u16(tf.CURRENT_ARMY) == 1
                          and e.u16(tf.MAP_STATE) == tf.CURSOR_STATE)
            ctx.require(ok, f"the player's turn on the second front ({tf.state(e)})")
            d.wait_control()
            g._units_base = g._players_base = None
            names = tf.intel(g)["names"]
            ctx.check(names and "General" not in names and not any(n in tf.POSTURE_NAMES for n in names),
                      f"no General on the second front's Intel ({names})")
        if m == RING_OF_FIRE:
            # The second front over (its result kept): no General, no Auto CO.
            e.w8(tf.SECOND, tf.SECOND_WON)
            names = tf.intel(g)["names"]
            ctx.eq(names, ["Status", "Terms", "Unit"], "the second front over: neither General nor Auto CO")
        e.close()
    e, g, d = boot(ctx)
    d.start(step=0)
    d.wait_map()
    m = tf.intel(g)
    ctx.eq((m["table"], m["names"]), (tf.GAME_INTEL_MENU, ["Status", "Terms", "Unit"]), "Jake's Trial: AW2's Intel")
    e.close()
    mp = ctx.map()
    mp.unit(1, "tank", 5, 5).unit(2, "tank", 20, 10)
    g = ctx.start(mp, ["andy", "drake"])
    ctx.eq(g.e.u32(tf.INTEL_MENU_POOL), tf.GAME_INTEL_MENU, "Versus: the game's Intel menu")


@test(modes=("ds",))
def two_front_general_menu(ctx):
    """Victory or Death!: the item reads General; A turns it to Defense,
    Strike, Assault and General again, the menu staying up, Dual Strike's
    help line under it; only army 1's posture changes, and the DS Campaign's
    record keeps the last choice. Its icon is Dual Strike's AI icon (the
    .nds's icon/res_icon0_cg_E, icon 0x5F) in BG tiles 0x34C..0x34F while
    the menu is up, its colours the .nds's icon palette's (AW2's menu
    palette, BG 10), the tiles empty again after."""
    from aw2test import rom as romlib
    e, g, d = start(ctx, VICTORY_OR_DEATH)
    panels = []

    def menu_shot(name):
        e.wait(10)
        shot(ctx, e, f"general_{name}")
        panels.append(e.u8(PANEL))
        if name == "General" and len(panels) == 1:
            ds = romlib.DualStrike()
            icon = ds.file("icon/res_icon0_cg_E")[128 * 0x5F:128 * 0x60]
            ctx.eq(e.read(ICON_TILES, 128), icon, "the icon's tiles: Dual Strike's AI icon")
            pal = ds.file("icon/res_icon_cl")[:32]
            used = sorted({v for b in icon for v in (b & 15, b >> 4) if v})
            ctx.eq([e.u16(0x05000000 + 32 * 10 + 2 * v) for v in used],
                   [int.from_bytes(pal[2 * v:2 * v + 2], "little") for v in used], "its colours: BG palette 10 = the .nds's")
            # Its tile map cells: the icon code's tiles in palette 10 (BG0).
            sb = 0x06000000 + ((e.u16(0x04000008) >> 8) & 31) * 0x800
            cells = [e.u16(sb + 2 * k) for k in range(0x400)]
            ctx.check((0xA000 | 0x34C) in cells and (0xA000 | 0x34F) in cells, "the icon on the screen (BG0's map)")

    seen = tf.set_posture(g, tf.ASSAULT, shot=menu_shot)
    ctx.eq(seen, ["General", "Defense", "Strike", "Assault"], "General, chosen: Defense, Strike, Assault")
    ctx.eq(panels, [HELP_PANEL] * 4, "Dual Strike's help line under each")
    ctx.eq((tf.posture(e, 1), tf.posture(e, 2)), (tf.ASSAULT, tf.GENERAL), "army 1's posture: Assault (Black Hole's General)")
    ctx.eq(e.u8(tf.P_POSTURE) ^ tf.GENERAL, tf.ASSAULT, "the record keeps the last choice")
    ctx.eq(e.read(ICON_TILES, 128), bytes(128), "the icon's tiles empty with the menu gone")
    ctx.eq(tf.set_posture(g, tf.GENERAL), ["Assault", "General"], "Assault, chosen: General")
    ctx.eq(tf.posture(e, 1), tf.GENERAL, "General again")
    e.close()
    # A new two-front battle starts the player's army at the last choice
    # (Dual Strike's save data keeps it), Black Hole's at General.
    e, g, d = start(ctx, OMENS_AND_SIGNS, poke=[(tf.P_POSTURE, tf.DEFENSE ^ tf.GENERAL)])
    ctx.eq((tf.posture(e, 1), tf.posture(e, 2)), (tf.DEFENSE, tf.GENERAL), "the last choice: Defense, Black Hole General")
    ctx.eq(tf.intel(g)["names"][3], "Defense", "Intel reads Defense")


def second_front_round(ctx, e, g, d, posture, roles):
    """From the checkpoint: army 1's posture set through Intel, its units on
    the second front given role `roles` (what the CPU gives units it
    buys), a round played; army 1's units there after it, by slot."""
    if posture != tf.GENERAL:
        tf.set_posture(g, posture)
    ctx.eq(tf.posture(e, 1), posture, f"posture {tf.POSTURE_NAMES[posture]}")
    for a in tf.store_unit_addrs(e, 1):
        e.w8(a + 0xB, roles)
    tf.end_round(e, d)
    d.wait_control()
    out = {}
    for j in range(51):
        a = tf.STORE + tf.B_UNITS + 12 * j
        if e.u8(a):
            out[j] = (e.u8(a), e.u8(a + 2), e.u8(a + 3), e.u16(a + 4) & 0x7F)
    return out


def enemy_distance(e, units):
    them = [(x, y) for a, _, x, y, _ in tf.store_units(e) if a == 2]
    mine = [u for u in units.values() if u[0] not in FOOT_AND_TRANSPORT]
    return sum(min(abs(x - ex) + abs(y - ey) for ex, ey in them) for _, x, y, _ in mine) / max(1, len(mine))


@test(modes=("ds",))
def two_front_general_changes_cpu(ctx):
    """Lightning Strikes, day 2 (the second front set up by day 1's
    round, Auto CO on): army 1's units there given role 7 (AW2's guard the
    HQ), then the round played four times from the same state, once per
    posture. General: the units' own role; Defense: they hold where they
    stand; Strike and Assault: they advance on Black Hole (Strike toward its
    units, Assault onto its properties), each its own way. Black Hole's units
    there and the main front are alike in all four."""
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    tf.end_round(e, d)
    d.wait_control()
    g._units_base = g._players_base = None
    cp = tf.checkpoint(e, ctx, "day2")
    before = {j: (e.u8(a), e.u8(a + 2), e.u8(a + 3), e.u16(a + 4) & 0x7F)
              for j in range(51) if e.u8(a := tf.STORE + tf.B_UNITS + 12 * j)}
    start_dist = enemy_distance(e, before)
    after = {}
    for p in (tf.GENERAL, tf.DEFENSE, tf.STRIKE, tf.ASSAULT):
        tf.back_to(e, g, cp)
        after[p] = second_front_round(ctx, e, g, d, p, roles=7)
        after[p, "dist"] = enemy_distance(e, after[p])
        after[p, "bh"] = sorted(u for u in tf.store_units(e) if u[0] == 2)
        ctx.log(f"{tf.POSTURE_NAMES[p]}: {after[p]} distance {after[p, 'dist']:.1f} (from {start_dist:.1f})")
    moved = lambda p: [j for j, u in before.items() if u[0] not in FOOT_AND_TRANSPORT and after[p].get(j, u)[1:3] != u[1:3]]
    ctx.eq(moved(tf.DEFENSE), [], "Defense: every unit holds where it stood")
    ctx.check(moved(tf.GENERAL), "General: the units' own role (7) moves some")
    for p in (tf.STRIKE, tf.ASSAULT):
        ctx.check(after[p, "dist"] < after[tf.GENERAL, "dist"] - 1, f"{tf.POSTURE_NAMES[p]}: closer to Black Hole "
                  f"({after[p, 'dist']:.1f} against General's {after[tf.GENERAL, 'dist']:.1f})")
    ctx.check(after[tf.STRIKE] != after[tf.ASSAULT], "Strike and Assault move the units differently")
    ctx.check(after[tf.GENERAL] != after[tf.DEFENSE], "General and Defense differ")


@test(modes=("ds",))
def two_front_general_only_there(ctx):
    """The posture has no effect outside a two-front battle: a Versus battle
    whose CPU army's units would advance, its CPU turn played twice from the
    same state, once with the two-front state's posture bits all set (each
    army Assault there): the same moves, the same units after."""
    mp = ctx.map()
    mp.unit(1, "tank", 3, 3).unit(2, "tank", 20, 10).unit(2, "md_tank", 21, 11).unit(2, "artillery", 22, 12)
    g = ctx.start(mp, ["andy", "drake"])
    e = g.e
    cp = tf.checkpoint(e, ctx, "versus")
    results = []
    for bits in (0x00, 0xFF):
        tf.back_to(e, g, cp)
        e.w8(tf.POSTURES, bits)
        g.end_turn()
        results.append(sorted((u["army"], u["type"], u["x"], u["y"]) for u in g.units()))
    ctx.eq(results[1], results[0], "Versus: the CPU's turn the same whatever the posture bits")
    ctx.check(any(u[0] == 2 and (u[2], u[3]) not in ((20, 10), (21, 11), (22, 12)) for u in results[0]), f"the CPU moved ({results[0]})")


@test(modes=("ds",))
def two_front_general_saved(ctx):
    """Lightning Strikes, Strike chosen, saved halfway (map menu > Save), a
    reboot, Continue: army 1's posture is Strike (Intel reads Strike) and
    Black Hole's General."""
    e, g, d = start(ctx, LIGHTNING_STRIKES)
    tf.set_posture(g, tf.STRIKE)
    ctx.eq(tf.posture(e, 1), tf.STRIKE, "Strike chosen")
    saves.suspend(g)
    img = saves.flash(e, os.path.join(ctx.out, "saved"))
    ctx.check(not img.problems(), f"every slot passes AW2's check {img.problems()}")
    e.close()
    e, g, d = continue_saved(ctx, img.path)
    ctx.eq((tf.posture(e, 1), tf.posture(e, 2)), (tf.STRIKE, tf.GENERAL), "after a reboot: Strike, Black Hole General")
    ctx.eq(tf.intel(g)["names"][3], "Strike", "Intel reads Strike")
    shot(ctx, e, "continued")
