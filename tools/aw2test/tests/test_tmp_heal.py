from aw2test.harness import test


def rd(g, a, n):
    return b"".join(g.e.read(a + i, min(16, n - i)) for i in range(0, n, 16))


@test(modes=("ds",))
def tmp_heal_vram(ctx):
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)  # Black Hole's HQ
    m.terrain(12, 8, 0x192)   # Crystal
    m.terrain(18, 8, 0x193)   # Obelisk
    m.unit(5, "infantry", 16, 10).unit(5, "tank", 12, 9).unit(5, "mech", 19, 11)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    for army in range(1, 6):
        u = [x for x in g.units() if x["army"] == army]
        ctx.log(f"army {army}: {[(x['type'], x['hp']) for x in u]}")
    dumps = []
    def snap(name):
        ctx.shot(g, name)
        dumps.append((name, rd(g, 0x06010000, 0x8000), rd(g, 0x05000200, 0x200)))
    snap("idle")
    g.select(16, 10) if False else None
    g.open_map_menu(); snap("menu"); g.e.press("B", 4); g.e.wait(30)
    g.goto(12, 9); g.e.wait(30); snap("cursor_unit")
    ctx.set_hp(g, 12, 9, 50)
    frames = []
    def obs(gg):
        for k in range(12):
            gg.e.wait(20)
            ctx.shot(gg, f"turn{k}")
    g.end_turn(observe=None)
    snap("back")
    ctx.log(f"tank hp after BH turn: {g.unit_at(12, 9)}")
    tiles = 0x8000 // 32
    used = [any(d[1][32 * t:32 * t + 32]) for t in range(tiles) for d in [None]] if False else None
    free = [t for t in range(tiles) if not any(any(d[1][32 * t:32 * t + 32]) for d in dumps)]
    runs = []
    s = None
    for t in range(tiles + 1):
        if t < tiles and t in set(free):
            s = t if s is None else s
        elif s is not None:
            runs.append((s, t - s)); s = None
    ctx.log(f"free OBJ tile runs (start, len) >= 4: {[(hex(a), n) for a, n in runs if n >= 4][:30]}")
    for k in range(16):
        free_c = [c for c in range(1, 16) if all(d[2][32 * k + 2 * c:32 * k + 2 * c + 2] == b"\0\0" for d in dumps)]
        ctx.log(f"OBJ palette {k}: zero colours {free_c}")
