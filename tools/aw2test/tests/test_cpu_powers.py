"""The computer uses the new COs' powers (crate::co_new: each takes the CPU
power settings of the AW2 CO it follows): with a full meter, the CPU fires
its Super CO Power on its turn, and its CO Power when only that is paid for."""

from aw2test import ram, rom as romlib
from aw2test.harness import test

NEW = ["jugger", "koal", "kindle", "vonbolt", "grimm", "javier", "sasha", "jake", "rachel"]


def cpu_fires(ctx, co, which):
    m = ctx.map()
    m.unit(1, "tank", 10, 6).unit(1, "infantry", 5, 5)
    m.unit(2, "tank", 14, 6).unit(2, "artillery", 18, 8).unit(2, "mech", 16, 10)
    g = ctx.start(m, ["andy", co])
    modes = set()
    def watch(gg):
        for _ in range(200):
            gg.e.wait(10)
            p = gg.player(2)
            modes.add(p["co_mode"])
            if gg.current_army() != 2:
                break
    # The CPU fires a CO Power 95% of the time it could (AW2's rule, the
    # same for the new COs): give it up to three turns.
    for turn in range(3):
        cost = g.charge_power(2, which)
        ctx.log(f"{co}: turn {turn}, meter set to {cost} for {which}")
        g.end_turn(observe=watch)
        if g.player(2)["powers_used"]:
            break
    p = g.player(2)
    ctx.log(f"{co}: modes seen on its turn {sorted(modes)}, powers used {p['powers_used']}")
    ctx.eq(p["powers_used"], 1, f"the CPU fired {co}'s {which}")
    want = 2 if which == "super" else 1
    ctx.check(want in modes, f"{co}'s power mode {want} seen during its turn")


def make(co, which):
    def fn(ctx):
        cpu_fires(ctx, co, which)
    fn.__name__ = f"cpu_power_{co}_{'scop' if which == 'super' else 'cop'}"
    test(modes=("ds",))(fn)


for co in NEW:
    make(co, "super")
    if co != "vonbolt":
        make(co, "power")
