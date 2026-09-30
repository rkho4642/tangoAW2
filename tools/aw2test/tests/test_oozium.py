"""The Oozium eats (Dual Strike pack, crate::oozium): it has no weapon (no
Fire, no counter-attack); moving onto a square next to it that holds a unit
of another team (any unit, air and naval included, on a square the Oozium can
enter) destroys that unit with the game's own destruction, no battle scene,
and the Oozium ends there. The CPU eats too."""

import struct

from aw2test import ram
from aw2test.harness import test

DS = ("ds",)
OOZIUM = 27
FUNDS_DESTROYED = 0x16  # player: units destroyed this turn (u16), +0x18 the most in a turn
CHARGE = 0x20


def charge(g, army):
    return struct.unpack_from("<I", g.player(army)["raw"], CHARGE)[0]


def destroyed(g, army):
    return struct.unpack_from("<H", g.player(army)["raw"], FUNDS_DESTROYED)[0]


def eat(ctx, g, src, dst, shots=None):
    """The player's Oozium at src moves onto dst (a unit it eats) and Waits;
    returns the command menu it got there."""
    g.select(*src)
    m = g.move_to(*dst)
    if shots:
        ctx.shot(g, f"{shots}_menu")
    g.choose("Wait", g.ACTION_MENU)
    if shots:
        g.e.wait(24)
        ctx.shot(g, f"{shots}_during")
    g.wait_for_input()
    if shots:
        ctx.shot(g, f"{shots}_after")
    return m["names"]


def others(g, army):
    """Army's units but the spare Infantry by its HQ."""
    return [u for u in g.units(army) if (u["x"], u["y"]) not in ((1, 1), (28, 18))]


@test(modes=DS)
def oozium_eats(ctx):
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    names = g.action_menu_at(10, 10)
    ctx.check("Fire" not in names, f"no Fire for the Oozium: {names}")
    ctx.shot(g, "before")
    c1, c2, d0 = charge(g, 1), charge(g, 2), destroyed(g, 1)
    lost0 = g.player(2)["raw"][0x3B]
    menu = eat(ctx, g, (10, 10), (11, 10), shots="eat")
    ctx.eq(menu, ["Wait"], "only Wait on the Tank's square")
    u = g.unit_at(11, 10)
    ctx.check(u is not None and u["type"] == OOZIUM, f"the Oozium is on the Tank's square: {u}")
    ctx.check(u is not None and u["flags"] & 1, "and has moved")
    ctx.check(g.unit_at(10, 10) is None, "it left its own square")
    ctx.eq(others(g, 2), [], "the Tank is gone")
    ctx.eq(u["hp"] if u else None, 100, "the Oozium is unharmed")
    # A Tank is 7000: the eater charges half its value, the victim all of it
    # (as a battle in which the Tank lost 10 HP).
    ctx.eq(charge(g, 1) - c1, 3500, "the eater's power meter charges as for a kill")
    ctx.eq(charge(g, 2) - c2, 7000, "the victim's power meter charges as for its loss")
    ctx.eq(destroyed(g, 1) - d0, 1, "army 1 counts a unit destroyed")
    ctx.eq(g.player(2)["raw"][0x3B] - lost0, 1, "army 2 counts a unit lost")
    g.end_turn()
    ctx.check(not g.battle_over(), "play goes on")


@test(modes=DS)
def oozium_eats_without_battle_scene(ctx):
    """With battle animations on, the eat plays on the map."""
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(2, "mdtank", 10, 11)
    g = ctx.start(m, ["andy", "olaf"], visuals="a")
    g.select(10, 10)
    g.move_to(10, 11)
    g.choose("Wait", g.ACTION_MENU)
    scene = 0
    for _ in range(150):
        g.e.wait(4)
        scene += g.e.u32(ram.MAIN_CALLBACK) == 0
    g.wait_for_input()
    ctx.eq(scene, 0, "no battle scene")
    ctx.eq(others(g, 2), [], "the Md Tank is gone")
    ctx.check(g.unit_at(10, 11) and g.unit_at(10, 11)["type"] == OOZIUM, "the Oozium is on its square")


@test(modes=DS)
def oozium_cancel_keeps_the_unit(ctx):
    """Backing out of the menu on the victim's square eats nothing."""
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 10)
    g.move_to(11, 10)
    g.e.press("B", 4)
    g.e.wait(20)
    g.e.press("B", 4)
    g.wait_for_input()
    ctx.check(g.unit_at(11, 10) and g.unit_at(11, 10)["type"] == 5, "the Tank is still there")
    ctx.check(g.unit_at(10, 10) and g.unit_at(10, 10)["type"] == OOZIUM, "the Oozium is back on its square")
    eat(ctx, g, (10, 10), (11, 10))
    ctx.eq(others(g, 2), [], "then it eats")


@test(modes=DS)
def oozium_no_counter(ctx):
    """A Tank attacks an Oozium: the Oozium takes Dual Strike's damage and hits back with nothing."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, OOZIUM, 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    (a0, a1), (d0, d1) = g.attack((10, 10), (10, 10), (11, 10))
    ctx.eq(a1["hp"], 100, "the Tank takes no counter-attack")
    ctx.check(d1["hp"] < 100, f"the Oozium took damage: {d1['hp']}")


@test(modes=DS)
def oozium_eats_air_on_land(ctx):
    """A Fighter on an airport and a B Copter on plains are eaten too."""
    m = ctx.map()
    m.terrain(11, 10, "airport", 2)
    m.unit(1, OOZIUM, 10, 10).unit(2, "fighter", 11, 10)
    m.unit(1, OOZIUM, 10, 14).unit(2, "bcopter", 10, 15)
    g = ctx.start(m, ["andy", "andy"])
    eat(ctx, g, (10, 10), (11, 10), shots="fighter")
    eat(ctx, g, (10, 14), (10, 15))
    ctx.eq(others(g, 2), [], "the Fighter and the B Copter are gone")
    ctx.check(g.unit_at(11, 10)["type"] == OOZIUM and g.unit_at(10, 15)["type"] == OOZIUM, "both Ooziums on their meals")


@test(modes=DS)
def oozium_cannot_enter_sea(ctx):
    """A B Copter over the sea: the Oozium can't go there, so can't eat it."""
    m = ctx.map()
    for x in range(11, 14):
        m.terrain(x, 10, "sea")
    m.unit(1, OOZIUM, 10, 10).unit(2, "bcopter", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 10)
    g.e.wait(20)
    row = g.e.u32(0x03003340 + 4 * 10)
    ctx.eq(g.e.u8(row + 11), 0xFF, "the copter's square is out of the Oozium's range")
    g.goto(11, 10)
    g.e.press("A", 4)
    g.e.wait(40)
    ctx.check(g.menu() is None or g.menu()["table"] != g.ACTION_MENU, "no move there")
    g.e.press("B", 4)
    g.wait_for_input()
    ctx.check(g.unit_at(11, 10) and g.unit_at(11, 10)["type"] == 19, "the copter is still there")


@test(modes=DS)
def oozium_eats_ship_in_port(ctx):
    m = ctx.map()
    m.terrain(11, 10, "port", 2).terrain(12, 10, "sea")
    m.unit(1, OOZIUM, 10, 10).unit(2, "cruiser", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    eat(ctx, g, (10, 10), (11, 10))
    ctx.eq(others(g, 2), [], "the Cruiser in port is gone")


@test(modes=DS)
def oozium_own_units_block(ctx):
    """Its own army's units are passed over as usual, never eaten; an ally's neither."""
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(1, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 10)
    g.goto(11, 10)
    g.e.press("A", 4)
    g.e.wait(40)
    ctx.check(g.menu() is None or g.menu()["table"] != g.ACTION_MENU, "no move onto its own Tank")
    g.e.press("B", 4)
    g.wait_for_input()
    ctx.eq(len(g.units(1)), 3, "every unit of army 1 still there")


@test(modes=DS)
def oozium_last_unit_ends_game(ctx):
    m = ctx.map(spare=False)
    m.unit(1, OOZIUM, 10, 10).unit(2, "tank", 11, 10).unit(1, "infantry", 1, 1)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 10)
    g.move_to(11, 10)
    g.choose("Wait", g.ACTION_MENU)
    players = g.players_base
    ok = g.e.wait_until(lambda: g.e.u16(players + 0x3C * 2 + 0x14) != 0, 1500, step=10)
    g.e.wait(120)
    ctx.shot(g, "rout")
    ctx.check(ok, "eating army 2's last unit defeats it (the battle ends)")


@test(modes=DS)
def oozium_eats_hidden_in_fog(ctx):
    """In fog, a unit the player doesn't see on the square an Oozium moves
    onto is eaten (instead of the usual Trap!). (A unit next to one of ours is
    always seen, so the test hides it from the player's view by hand.)"""
    m = ctx.map()
    for x in range(8, 16):
        m.terrain(x, 11, "wood")
    m.unit(1, OOZIUM, 10, 10).unit(2, "infantry", 10, 11)
    g = ctx.start(m, ["andy", "andy"], fog=True)
    seen_at = 0x0201E450 + 0x12 + g.e.u16(0x0201E450 + 0x417A + 2 * 11) + 10
    g.e.w8(seen_at, 0)
    g.select(10, 10)
    g.goto(10, 11)
    g.e.press("A", 4)
    g.wait_menu(g.ACTION_MENU, 600)
    ctx.shot(g, "fog_menu")
    g.choose("Wait", g.ACTION_MENU)
    g.wait_for_input()
    ctx.shot(g, "fog_after")
    ctx.eq(others(g, 2), [], "the unseen Infantry was eaten")
    u = g.unit_at(10, 11)
    ctx.check(u is not None and u["type"] == OOZIUM, f"the Oozium is on its square: {u}")


@test(modes=DS)
def oozium_black_bomb_spares_it(ctx):
    m = ctx.map()
    m.unit(1, "blackbomb", 5, 10).unit(2, OOZIUM, 10, 10).unit(2, "tank", 10, 12)
    g = ctx.start(m, ["andy", "andy"])
    g.select(5, 10)
    g.move_to(9, 10)
    g.choose("Explode", g.ACTION_MENU)
    g.wait_for_input()
    ctx.eq(g.unit_at(10, 10)["hp"], 100, "the Oozium takes nothing from the bomb")
    ctx.eq(g.unit_at(10, 12)["hp"], 50, "the Tank does")


# --- the CPU ------------------------------------------------------------------------


@test(modes=DS)
def cpu_oozium_eats(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, OOZIUM, 11, 10)
    g = ctx.start(m, ["andy", "andy"], visuals="a")
    c1, c2 = charge(g, 1), charge(g, 2)

    def watch(gg):
        for k in range(8):
            gg.e.wait(12)
            ctx.shot(gg, f"cpu_eat_{k}")

    g.end_turn(observe=watch)
    ctx.eq(others(g, 1), [], "the CPU's Oozium ate the Tank")
    u = g.unit_at(10, 10)
    ctx.check(u is not None and u["type"] == OOZIUM, f"the Oozium is on the Tank's square: {u}")
    ctx.eq(charge(g, 2) - c2, 3500, "the CPU's meter charges as for a kill")
    ctx.eq(charge(g, 1) - c1, 7000, "the player's as for the loss")


@test(modes=DS)
def cpu_oozium_eats_air(ctx):
    m = ctx.map()
    m.terrain(10, 10, "airport", 1)
    m.unit(1, "bomber", 10, 10).unit(2, OOZIUM, 10, 9)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    ctx.eq(others(g, 1), [], "the CPU's Oozium ate the parked Bomber")
    ctx.check(g.unit_at(10, 10) and g.unit_at(10, 10)["type"] == OOZIUM, "and stands on the airport")


@test(modes=DS)
def cpu_oozium_picks_pricier(ctx):
    m = ctx.map()
    m.unit(1, "infantry", 10, 9).unit(1, "neotank", 11, 10).unit(1, "recon", 10, 11)
    m.unit(2, OOZIUM, 10, 10)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    left = sorted(u["type"] for u in others(g, 1))
    ctx.eq(left, [1, 6], "the Neotank (the priciest) was eaten, the Infantry and Recon left")
    ctx.check(g.unit_at(11, 10) and g.unit_at(11, 10)["type"] == OOZIUM, "the Oozium is on the Neotank's square")


@test(modes=DS)
def cpu_oozium_advances(ctx):
    m = ctx.map()
    m.unit(1, "tank", 5, 10).unit(2, OOZIUM, 12, 10)
    g = ctx.start(m, ["andy", "andy"])
    for _ in range(3):
        g.end_turn()
    oz = [u for u in g.units(2) if u["type"] == OOZIUM][0]
    ctx.check(abs(oz["x"] - 5) + abs(oz["y"] - 10) < 7, f"the Oozium came towards the Tank: {oz['x'], oz['y']}")


@test(modes=DS)
def cpu_oozium_five_armies(ctx):
    """Black Hole (army 5 of a five-army game) eats with its Oozium."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 15, 0x1B4)
    m.colours = [5, 1, 2, 3, 4]
    m.unit(1, "tank", 10, 10).unit(5, OOZIUM, 11, 10).unit(5, "infantry", 16, 15)
    g = ctx.start(m, None)
    g.end_turn()
    ctx.eq(others(g, 1), [], "Black Hole's Oozium ate the Tank")
    u = g.unit_at(10, 10)
    ctx.check(u is not None and u["type"] == OOZIUM, f"on its square: {u}")


@test(modes=DS, netplay=True)
def netplay_oozium_eats(ctx):
    """A human eat and a CPU eat, replayed on two rollback peers."""
    m = ctx.map()
    m.unit(1, OOZIUM, 10, 10).unit(2, "tank", 11, 10)
    m.unit(1, "tank", 15, 10).unit(2, OOZIUM, 16, 10)
    g = ctx.start(m, ["andy", "andy"])
    eat(ctx, g, (10, 10), (11, 10))
    g.end_turn(human=1)
    ctx.check(g.unit_at(15, 10) and g.unit_at(15, 10)["type"] == OOZIUM, "the CPU ate too")
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    players = (g.players_base + ram.PLAYER_SIZE, ram.PLAYER_SIZE * 2)
    offline = {a: g.e.read(a, n) for a, n in (units, players)}
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, players])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")
