"""Dual Strike's new units (pack only): build menus, stats, damage."""

import struct

from aw2test.harness import test

BUILD_BUFFER = 0x02023830
NEW = {4: "Megatank", 9: "Piperunner", 12: "Stealth", 13: "Black Bomb", 18: "Black Boat", 26: "Carrier", 27: "Oozium"}


def menu_at(ctx, g, x, y, name):
    g.wait_idle()
    g.goto(x, y)
    g.e.press("A", 4)
    g.e.wait(60)
    ids = []
    for k in range(30):
        t = g.e.u8(BUILD_BUFFER + 4 * k)
        if t == 0:
            break
        ids.append(t)
    ctx.shot(g, name)
    for _ in range(12):
        g.e.press("DOWN", 4)
        g.e.wait(6)
    g.e.wait(20)
    ctx.shot(g, name + "_end")
    g.e.press("B", 4)
    g.e.wait(30)
    g.wait_for_input()
    return ids


@test()
def build_menus(ctx):
    m = ctx.map()
    m.terrain(5, 5, "base", 1).terrain(7, 5, "airport", 1).terrain(9, 5, "port", 1)
    g = ctx.start(m, ["andy", "andy"])
    g.e.w32(g.player(1)["addr"], 90000)
    base = menu_at(ctx, g, 5, 5, "base")
    air = menu_at(ctx, g, 7, 5, "airport")
    port = menu_at(ctx, g, 9, 5, "port")
    ctx.log(f"base {base}\nairport {air}\nport {port}")
    if ctx.mode == "ds":
        ctx.eq(base, [1, 2, 6, 5, 3, 8, 4, 7, 10, 11, 14, 15, 9], "base: the game's list with Megatank and Piperunner")
        ctx.eq(air, [16, 17, 19, 20, 12, 13], "airport: with Stealth and Black Bomb")
        ctx.eq(port, [21, 22, 23, 24, 18, 26], "port: with Black Boat and Carrier")
    else:
        ctx.eq(base, [1, 2, 6, 5, 3, 8, 7, 10, 11, 14, 15], "base: AW2's list")
        ctx.eq(air, [16, 17, 19, 20], "airport: AW2's list")
        ctx.eq(port, [21, 22, 23, 24], "port: AW2's list")
