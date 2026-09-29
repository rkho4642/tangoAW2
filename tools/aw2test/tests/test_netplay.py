"""Netplay: a scripted battle replayed on two rollback peers.

The offline run's inputs (recorded by the driver) are replayed by
aw2_netplay_script with seat 0 pressing (army 1; army 2 is the CPU) over a
fake network with delay and jitter. Both peers and a straight replay of the
confirmed inputs must end byte-identical, and the units must end exactly as
they did offline.
"""

from aw2test import ram
from aw2test.harness import test


@test()
def netplay_tank_battle_and_cpu_turn(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10).unit(1, "artillery", 8, 10)
    g = ctx.start(m, ["andy", "hawke"])
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    ctx.attack(g, (8, 10), (8, 10), (11, 10))
    g.end_turn(human=1)
    units = (g.units_base + ram.UNIT_SIZE * 1, ram.UNIT_SIZE * 127)
    players = (g.players_base + ram.PLAYER_SIZE, ram.PLAYER_SIZE * 2)
    offline = {a: g.e.read(a, n) for a, n in (units, players)}
    g.e.wait(60)
    identical, values, text = ctx.netplay_replay(g, [units, players])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay)")
    for a, v in offline.items():
        ctx.eq(values.get(a, b"").hex(), v.hex(), f"netplay peer 0 RAM at {a:08x} equals the offline run")
