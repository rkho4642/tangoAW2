"""Every army builds an Oozium at a base (Dual Strike pack), in its own colour:
army 1 plays each of the five colours (Orange Star, Blue Moon, Green Earth,
Yellow Comet, Black Hole); the Oozium it buys shows that army's palette."""

from aw2test.harness import test

NAMES = {1: "Orange Star", 2: "Blue Moon", 3: "Green Earth", 4: "Yellow Comet", 5: "Black Hole"}
PAL_BUFFER = 0x030020C0


def make(colour):
    def fn(ctx):
        m = ctx.map()
        m.terrain(5, 5, "base", 1)
        m.unit(1, "tank", 8, 5)
        m.colours = [0, colour, 2 if colour != 2 else 1, 3, 4]
        g = ctx.start(m, ["andy", "max"])
        g.e.w32(g.player(1)["addr"], 50000)
        g.buy(5, 5, 27)
        u = g.unit_at(5, 5)
        ctx.check(u is not None and u["type"] == 27, f"{NAMES[colour]} built an Oozium: {u}")
        ctx.eq(g.player(1)["raw"][0x1A], colour, "army 1's colour")
        # A map unit is drawn with its army's BG palette; the tank next to it
        # (an AW2 unit of the same army) uses the same one.
        g.goto(6, 7)
        g.e.wait(30)
        ctx.shot(g, f"oozium_{colour}")
    fn.__name__ = f"oozium_built_by_{NAMES[colour].lower().replace(' ', '_')}"
    test(modes=("ds",))(fn)


for c in range(1, 6):
    make(c)
