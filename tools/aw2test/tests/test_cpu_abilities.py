"""The CPU uses the new units' abilities (crate::cpu_tactics), with the pack:
a Black Bomb flies to a crowd of enemies and explodes; a Stealth hides when
threatened by units that can't hit a hidden Stealth, and stays visible near
Fighters; a Black Boat repairs and resupplies the units next to it."""

from aw2test.harness import test

DS = ("ds",)


@test(modes=DS)
def cpu_black_bomb_explodes(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(1, "mdtank", 11, 6).unit(1, "tank", 10, 7).unit(1, "artillery", 11, 7)
    m.unit(2, "blackbomb", 16, 6)
    g = ctx.start(m, ["andy", "andy"])
    before = {(u["x"], u["y"]): u["hp"] for u in g.units(1)}
    g.end_turn()
    g.e.wait(60)
    after = {(u["x"], u["y"]): u["hp"] for u in g.units(1)}
    ctx.log(f"army 1 HP before {before} after {after}")
    hit = [p for p in before if after.get(p, 0) == before[p] - 50]
    ctx.check(len(hit) >= 3, f"the bomb hit the group for 5 HP each: {hit}")
    ctx.eq([u for u in g.units(2) if u["type"] == 13], [], "the bomb is gone")
    ctx.shot(g, "after")


@test(modes=DS)
def cpu_black_bomb_holds_for_little(ctx):
    m = ctx.map()
    m.unit(1, "infantry", 5, 5)
    m.unit(2, "blackbomb", 16, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    ctx.eq(g.unit_at(5, 5)["hp"], 100, "one infantry isn't worth a bomb")
    bombs = [u for u in g.units(2) if u["type"] == 13]
    ctx.eq(len(bombs), 1, "the bomb is kept")
    ctx.check(abs(bombs[0]["x"] - 5) + abs(bombs[0]["y"] - 5) < 12, f"it flew towards the enemy: {bombs[0]['x'], bombs[0]['y']}")


@test(modes=DS)
def cpu_stealth_hides(ctx):
    m = ctx.map()
    m.unit(1, "antiair", 10, 6).unit(1, "missiles", 9, 8)
    m.unit(2, "stealth", 14, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    s = [u for u in g.units(2) if u["type"] == 12][0]
    ctx.check(s["flags"] & 0x20, f"the Stealth hid (flags {s['flags']:#x}) at {s['x'], s['y']}")


@test(modes=DS)
def cpu_stealth_stays_visible_near_fighters(ctx):
    m = ctx.map()
    m.unit(1, "antiair", 10, 6).unit(1, "fighter", 9, 4)
    m.unit(2, "stealth", 14, 6)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn()
    s = [u for u in g.units(2) if u["type"] == 12]
    ctx.check(not s or not s[0]["flags"] & 0x20, "near a Fighter the Stealth doesn't hide")


@test(modes=DS)
def cpu_black_boat_repairs(ctx):
    m = ctx.map()
    for x in range(8, 22):
        for y in range(12, 18):
            m.terrain(x, y, "sea")
    m.unit(1, "infantry", 2, 2)
    m.unit(2, "blackboat", 15, 14).unit(2, "cruiser", 16, 14).unit(2, "lander", 15, 15)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 16, 14, 50)
    # A repair costs a tenth of the unit's price per HP (1800 for a Cruiser).
    g.e.w32(g.player(2)["addr"], 20000)
    g.end_turn()
    boat = [u for u in g.units(2) if u["type"] == 18][0]
    cruiser = [u for u in g.units(2) if u["type"] == 22][0]
    ctx.log(f"boat at {boat['x'], boat['y']}, cruiser at {cruiser['x'], cruiser['y']} hp {cruiser['hp']}")
    near = abs(boat["x"] - cruiser["x"]) + abs(boat["y"] - cruiser["y"]) == 1
    ctx.check(near and cruiser["hp"] >= 60, f"the Black Boat repaired the Cruiser next to it: hp {cruiser['hp']}")


@test(modes=DS, netplay=True)
def netplay_cpu_abilities(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(1, "mdtank", 11, 6).unit(1, "tank", 10, 7).unit(1, "antiair", 11, 7)
    m.unit(2, "blackbomb", 16, 6).unit(2, "stealth", 16, 9)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn(human=1)
    g.end_turn(human=1)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")


@test(modes=DS)
def cpu_black_bomb_five_armies(ctx):
    """Army 5 (Black Hole, five-army game: 51 unit ids an army) bombs a group."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)
    m.unit(1, "tank", 10, 6).unit(1, "mdtank", 11, 6).unit(1, "tank", 10, 7).unit(1, "artillery", 11, 7)
    m.unit(5, "blackbomb", 16, 6).unit(5, "infantry", 16, 11)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    before = {(u["x"], u["y"]): u["hp"] for u in g.units(1)}
    g.end_turn()
    after = {(u["x"], u["y"]): u["hp"] for u in g.units(1)}
    hit = [p for p in before if after.get(p, 0) == before[p] - 50]
    ctx.check(len(hit) >= 3, f"Black Hole's bomb hit the group: {hit} ({after})")
