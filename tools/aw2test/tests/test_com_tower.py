"""Com Towers (Dual Strike pack, Versus): each tower an army owns gives its units
+10% firepower; neutral towers give nothing. Without the pack they are Labs,
which give nothing. The calculator adds the bonus in ds mode (harness.side)."""

import struct

from aw2test.harness import test

LAB = {0: 0x1D9, 1: 0x1DA, 2: 0x1DB}


@test()
def com_tower_firepower(ctx):
    m = ctx.map()
    m.terrain(2, 2, LAB[1]).terrain(4, 2, LAB[1]).terrain(6, 2, LAB[2]).terrain(8, 2, LAB[0])
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["andy", "andy"])
    ctx.eq(ctx.towers(g, 1), 2, "Orange Star owns two towers")
    ctx.eq(ctx.towers(g, 2), 1, "Blue Moon owns one tower")
    g.goto(2, 2)
    g.e.wait(30)
    ctx.shot(g, "tower_panel")
    # Attacker +20%, the counter +10% (with the pack).
    ctx.attack(g, (10, 6), (10, 6), (11, 6), expect_base=55)
    ctx.shot(g, "after")


@test()
def com_tower_earns_nothing(ctx):
    m = ctx.map()
    m.terrain(2, 2, LAB[1]).terrain(4, 2, LAB[1]).terrain(6, 2, "city", 1)
    m.unit(1, "infantry", 10, 6).unit(2, "infantry", 20, 6)
    g = ctx.start(m, ["andy", "andy"])
    funds = lambda: struct.unpack_from("<I", g.player(1)["raw"], 0)[0]
    f0 = funds()
    g.end_turn()
    got = funds() - f0
    # Army 1's turn start pays for its HQ and city (1000 each), and in AW2
    # for its two Labs too.
    want = {"aw2": 4000, "ds": 2000}[ctx.mode]
    ctx.eq(got, want, "income at the next turn start")


# Tower tiles per owner: neutral, Orange Star, Blue Moon, Green Earth, Yellow
# Comet, and Black Hole (tangoAW2's five-army owner 5, crate::five).
TOWER_TILES = [0x1D9, 0x1DA, 0x1DB, 0x1DC, 0x1DD, 0x1B9]


@test(modes=("ds",))
def com_tower_colours_five_armies(ctx):
    """Every army's captured tower shows in its colours, with five armies."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)  # Black Hole's HQ
    m.unit(5, "infantry", 16, 10)
    m.colours = [5, 1, 2, 3, 4]  # the five-army mark
    for owner, t in enumerate(TOWER_TILES):
        m.terrain(2 + 2 * owner, 3, t)
    g = ctx.start(m, None)
    for owner in range(1, 6):
        ctx.eq(g.terrain_class(2 + 2 * owner, 3), (owner << 5) | 0x14, f"owner {owner}'s tower")
    g.goto(2, 3)
    g.e.wait(30)
    ctx.shot(g, "five_towers")
    g.goto(15, 10)
    g.e.wait(30)
    ctx.shot(g, "black_hole_hq")


@test()
def com_tower_forecast_shows_boost(ctx):
    """The damage forecast shown while picking a target includes the towers."""
    m = ctx.map()
    m.terrain(2, 2, LAB[1]).terrain(4, 2, LAB[1])
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.select(10, 6)
    g.move_to(10, 6)
    g.choose("Fire", g.ACTION_MENU)
    g.e.wait(40)
    ctx.shot(g, "forecast")


@test(modes=("ds",))
def com_tower_intel(ctx):
    m = ctx.map()
    m.terrain(2, 2, LAB[1]).terrain(4, 2, LAB[1]).terrain(6, 2, LAB[2])
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.open_map_menu()
    ctx.shot(g, "menu")
    g.choose("Intel", g.MAP_MENU)
    g.e.wait(60)
    ctx.shot(g, "intel")
    g.e.press("A", 4)
    g.e.wait(90)
    ctx.shot(g, "intel_a")
    g.e.press("RIGHT", 4)
    g.e.wait(60)
    ctx.shot(g, "intel_right")


@test(modes=("ds",))
def com_tower_co_page(ctx):
    m = ctx.map()
    m.terrain(2, 2, LAB[1]).terrain(4, 2, LAB[1])
    m.unit(1, "tank", 10, 6).unit(2, "tank", 11, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.open_map_menu()
    g.choose("CO", g.MAP_MENU)
    g.e.wait(90)
    for i, k in enumerate(["DOWN", "DOWN", "DOWN", "DOWN", "RIGHT", "R", "L"]):
        g.e.press(k, 4)
        g.e.wait(60)
        ctx.shot(g, f"co{i}")


def bars(ctx, g, name):
    """Screenshot the CO screen's unit bars page (map menu > CO, 4 x DOWN)."""
    g.open_map_menu()
    g.choose("CO", g.MAP_MENU)
    g.e.wait(90)
    for _ in range(4):
        g.e.press("DOWN", 4)
        g.e.wait(40)
    ctx.shot(g, name)
    for _ in range(4):
        g.e.press("B", 4)
        g.e.wait(40)
    g.wait_for_input()


def capture(g, x, y, ctx=None, name=None):
    g.select(x, y)
    g.move_to(x, y)
    g.choose("Capt", g.ACTION_MENU)
    if name:
        for i in range(6):
            g.e.wait(120)
            ctx.shot(g, f"{name}{i}")
            ctx.log(f"{name}{i}: battle over {g.battle_over()} procs {[hex(f) for a, _, f in g.procs()]}")
    if ctx is not None and ctx.mode == "aw2" and name:
        return
    g.wait_for_input()


@test()
def com_tower_up_and_down(ctx):
    """Towers change hands: each army's tower count, CO-screen bars and
    firepower follow; a Super CO Power stacks with the towers."""
    m = ctx.map()
    m.terrain(5, 5, LAB[0]).terrain(8, 5, LAB[1])
    m.unit(1, "infantry", 5, 5).unit(2, "infantry", 8, 5)
    m.unit(1, "tank", 10, 8).unit(2, "tank", 11, 8)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    tw = lambda a: ctx.towers(g, a)
    ctx.eq((tw(1), tw(2)), (1, 0), "start: Orange Star owns one tower")
    bars(ctx, g, "bars_1_tower")
    capture(g, 5, 5)                  # Orange Star: the neutral tower, half
    g.end_turn(human=2)
    capture(g, 8, 5)                  # Blue Moon: Orange Star's tower, half
    g.end_turn(human=1)
    capture(g, 5, 5)                  # Orange Star takes the neutral tower
    ctx.eq((tw(1), tw(2)), (2, 0), "Orange Star captured the neutral tower")
    bars(ctx, g, "bars_2_towers")
    g.end_turn(human=2)
    capture(g, 8, 5, ctx, "steal")    # Blue Moon takes Orange Star's tower
    if ctx.mode == "aw2":
        # Without the pack it is a Lab: capturing an army's Lab defeats it
        # (AW2's own rule; steal5 shows the VICTORY screen).
        return
    # With the pack the battle goes on: Blue Moon's turn ends normally and
    # Orange Star plays again.
    ctx.eq((tw(1), tw(2)), (1, 1), "Blue Moon took one of Orange Star's towers")
    bars(ctx, g, "bars_blue_1_tower")
    g.end_turn(human=1)
    bars(ctx, g, "bars_orange_1_tower")
    # A Super CO Power on top of the towers (Andy's Hyper Upgrade: +HP and
    # firepower); the calculator adds the towers to the power's numbers.
    ctx.power(g, 1, "super")
    bars(ctx, g, "bars_super_1_tower")
    ctx.attack(g, (10, 8), (10, 8), (11, 8), expect_base=None)


@test()
def com_tower_counts_for_capture_limit(ctx):
    """A captured tower counts toward a 'capture N properties' win, like any
    property (the game's census counts Labs): Orange Star holds its HQ and a
    city, the limit is 3, and taking a tower wins."""
    m = ctx.map()
    m.terrain(3, 3, "city", 1).terrain(5, 5, LAB[0])
    m.unit(1, "infantry", 5, 5)
    g = ctx.start(m, ["andy", "andy"], capt=1)  # the Rules choice: limit 3
    players = g.e.u32(0x08499598)
    caps = lambda a: g.e.u8(players + 0x3C * a + 0x11)
    ctx.eq(g.e.u8(0x03003FF1), 3, "capture limit")
    ctx.eq(caps(1), 2, "Orange Star holds 2 properties")
    capture(g, 5, 5)
    g.end_turn()
    g.select(5, 5)
    g.move_to(5, 5)
    g.choose("Capt", g.ACTION_MENU)
    g.e.wait(600)
    ctx.shot(g, "won")
    ctx.eq(caps(1), 3, "the tower counts: Orange Star holds 3")
    ctx.check(g.e.u16(players + 0x3C * 2 + 0x14) != 0, "Blue Moon is defeated (capture limit reached)")
