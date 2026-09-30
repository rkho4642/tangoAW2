"""Dual Strike's own map animations for the new units and COs (map_anim.rs,
co_new.rs, sandstorm.rs; with the pack): a Black Bomb's explosion, a Stealth
hiding and appearing, a Black Boat's REPAIR label, Oozium's death in its
army's colours, the new COs' power effect on their units, and the sandstorm's
sand. Each is played by a human and by the CPU (army 2, and Black Hole as army
5 of a five-army game), in all five army colours; a test passes when Dual
Strike's pictures were in the effect's VRAM while it played (compared with the
.nds's own files), the game went on, and the frames are kept in its output
folder."""

import struct

from aw2test import rom as romlib
from aw2test.harness import test

DS = ("ds",)
EFFECT_VRAM = 0x06013940       # AW2's map effects: OBJ tiles 0x1CA..
OBJ_PAL = 0x05000200
COLOURS = ["os", "bm", "ge", "yc", "bh"]
PRESENTATION = 0x08740000       # co_new: the grown CO presentation table
PRES_ROW = 0x44

_ds = None


def ds():
    global _ds
    if _ds is None:
        _ds = romlib.DualStrike()
    return _ds


def colours_for(colour):
    """Design-map colours: [0] the five-army mark, then each army's country."""
    others = [c for c in range(1, 6) if c != colour]
    return [0, colour] + others[:3]


def watch(ctx, g, frames, prefix, checks, step=2, shots_every=4, until=None):
    """Sample VRAM/palette for `frames` frames: checks {name: (addr, bytes)};
    returns the names seen. Screenshots while any matches."""
    seen = set()
    for k in range(0, frames, step):
        g.e.wait(step)
        hit = [n for n, (a, want) in checks.items() if g.e.read(a, len(want)) == want]
        seen.update(hit)
        if hit and k % shots_every == 0:
            ctx.shot(g, f"{prefix}_{k:04d}")
        if until and until(g):
            break
    return seen


def explosion_check():
    return {"explosion": (EFFECT_VRAM, ds().file("bmap/05f")[:256])}


def stealth_check():
    return {"stealth": (EFFECT_VRAM, ds().file("bmap/076")[:256])}


def repair_check():
    return {"repair": (EFFECT_VRAM + 2048, ds().file("national/003")[4096:4096 + 256])}


def ooze_check(colour):
    return {
        "ooze": (EFFECT_VRAM, ds().file("bmap/045")[:256]),
        "ooze_palette": (OBJ_PAL + 0xA0 + 2, ds().file("bmap/%03x" % (0x45 + colour))[2:32]),
    }


def power_effect_names(co, power):
    """Dual Strike's power effect for a CO's COP (0) or SCOP (1): its OBJ
    pictures' file (the effect table at arm9 0x02168850)."""
    d = ds()
    anim = d.a9(0x0215360C + 0x220 * co + 0x120 + 0x80 * power + 0x30, 1)[0]
    name_at = struct.unpack_from("<I", d.a9(0x02168850 + 12 * anim + 4, 4))[0]
    return "bmap/" + d.a9(name_at, 3).decode()


DS_IDS = {"jugger": 12, "koal": 14, "vonbolt": 11, "jake": 20, "rachel": 21, "sasha": 22,
          "javier": 23, "grimm": 24, "kindle": 25}


def power_check(co, power):
    return {"power": (EFFECT_VRAM, ds().file(power_effect_names(DS_IDS[co], power))[:256])}


# --- Human ----------------------------------------------------------------------------

def human_explode(ctx, colour):
    m = ctx.map()
    m.colours = colours_for(colour)
    m.unit(1, 13, 5, 10)
    m.unit(2, "tank", 10, 10).unit(2, "infantry", 11, 11)
    g = ctx.start(m, ["andy", "andy"])
    ctx.eq(g.player(1)["colour"], colour, "army 1's colour")
    g.select(5, 10)
    g.move_to(9, 10)
    g.choose("Explode", g.ACTION_MENU)
    seen = watch(ctx, g, 240, "boom", explosion_check())
    g.wait_for_input()
    ctx.check("explosion" in seen, "Dual Strike's explosion played")
    ctx.eq((g.unit_at(10, 10) or {}).get("hp"), 50, "the blast still takes 5 HP")
    ctx.check(g.unit_at(9, 10) is None, "the bomb is gone")


def human_stealth(ctx, colour):
    m = ctx.map()
    m.colours = colours_for(colour)
    m.unit(1, 12, 10, 10).unit(2, "tank", 14, 10)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Hide", g.ACTION_MENU)
    seen = watch(ctx, g, 90, "hide", stealth_check())
    g.wait_for_input()
    ctx.check("stealth" in seen, "Dual Strike's Stealth cloud played as it hid")
    ctx.check(g.unit_at(10, 10)["flags"] & 0x20, "the Stealth is hidden")
    g.end_turn(human=2)
    g.end_turn(human=1)
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Appear", g.ACTION_MENU)
    seen = watch(ctx, g, 90, "appear", stealth_check())
    g.wait_for_input()
    ctx.check("stealth" in seen, "and as it appeared")
    ctx.check(not g.unit_at(10, 10)["flags"] & 0x20, "the Stealth is visible")


def sea_map(ctx):
    m = ctx.map()
    for x in range(9, 13):
        for y in range(8, 13):
            m.terrain(x, y, "sea")
    m.terrain(12, 10, "plain")
    return m


def human_repair(ctx, colour):
    m = sea_map(ctx)
    m.colours = colours_for(colour)
    m.unit(1, 18, 11, 10).unit(1, "tank", 12, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.set_hp(g, 12, 10, 50)
    t = g.unit_at(12, 10)
    g.e.w16(g.unit_addr(t["id"]) + 4, (g.e.u16(g.unit_addr(t["id"]) + 4) & ~(0xF << 7)) | (3 << 7) | 50)
    g.select(11, 10)
    g.move_to(11, 10)
    g.choose("Repair", g.ACTION_MENU)
    seen = watch(ctx, g, 60, "repair", repair_check(), step=1, shots_every=2)
    g.wait_for_input()
    ctx.check("repair" in seen, "Dual Strike's REPAIR label showed")
    ctx.eq(g.unit_at(12, 10)["hp"], 60, "the Tank is repaired by 1 HP")


def human_ooze(ctx, colour):
    """Army 1's Oozium (in `colour`) dies to army 2's Megatank."""
    m = ctx.map()
    m.colours = colours_for(colour)
    m.unit(1, 27, 10, 10).unit(1, "infantry", 2, 2)
    m.unit(2, "megatank", 11, 10)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    ctx.set_hp(g, 10, 10, 10)
    g.end_turn(human=2)
    g.select(11, 10)
    g.move_to(11, 10)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(10, 10)
    seen = watch(ctx, g, 240, "ooze", ooze_check(colour))
    g.wait_for_input()
    ctx.check(g.unit_at(10, 10) is None, "the Oozium is destroyed")
    ctx.check("ooze" in seen, "Dual Strike's Oozium death played")
    ctx.check("ooze_palette" in seen, f"in its army's colours ({COLOURS[colour - 1]})")


def human_power(ctx, co, which, colour=1):
    m = ctx.map()
    m.colours = colours_for(colour)
    m.unit(1, "tank", 10, 10).unit(1, "infantry", 11, 10).unit(2, "tank", 20, 10)
    g = ctx.start(m, [co, "andy"])
    power = 1 if which == "super" else 0
    p = g.player(1)
    row = PRESENTATION + PRES_ROW * p["co"] + 0x1C + 0x14 * power
    ctx.log(f"presentation row bytes {g.e.read(row, 2).hex()}")
    g.charge_power(1, which)
    g.open_map_menu()
    g.choose("Super" if which == "super" else "Power", g.MAP_MENU)
    seen = watch(ctx, g, 900, "power", power_check(co, power), step=3, shots_every=6)
    g.wait_for_input()
    ctx.check("power" in seen, f"Dual Strike's power effect ({power_effect_names(DS_IDS[co], power)}) on the units")


GENERIC = [("jugger", "power"), ("jugger", "super"), ("koal", "power"), ("koal", "super"),
           ("grimm", "power"), ("grimm", "super"), ("jake", "power"), ("jake", "super"),
           ("javier", "power"), ("javier", "super"), ("kindle", "super"), ("rachel", "power"),
           ("sasha", "power"), ("sasha", "super")]


def make(fn, name, *args):
    def t(ctx):
        fn(ctx, *args)
    t.__name__ = name
    test(modes=DS)(t)


for c in range(1, 6):
    n = COLOURS[c - 1]
    make(human_explode, f"map_anim_explode_{n}", c)
    make(human_stealth, f"map_anim_stealth_{n}", c)
    make(human_repair, f"map_anim_repair_{n}", c)
    make(human_ooze, f"map_anim_ooze_{n}", c)
    make(human_power, f"map_anim_power_grimm_{n}", "grimm", "power", c)
for co, which in GENERIC:
    make(human_power, f"map_anim_power_{co}_{'cop' if which == 'power' else 'scop'}", co, which)


# --- The CPU ------------------------------------------------------------------------

def cpu_turn(ctx, g, checks, prefix, frames=2400):
    box = {}

    def observe(gg):
        box["seen"] = watch(ctx, gg, frames, prefix, checks, shots_every=4,
                            until=lambda x: x.current_army() == 1)
    g.end_turn(observe=observe)
    return box.get("seen", set())


def cpu_setup(ctx, colour, five):
    """A map for the CPU: army 2 in `colour` (two armies), or Black Hole as
    army 5 (five armies). Returns (map, cpu army)."""
    if five:
        m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
        m.terrain(15, 10, 0x1B4)
        m.colours = [5, 1, 2, 3, 4]
        return m, 5
    m = ctx.map()
    others = [c for c in range(1, 6) if c != colour]
    m.colours = [0, others[0], colour] + others[1:3]
    return m, 2


def cpu_start(ctx, m, five, co="andy"):
    return ctx.start(m, None) if five else ctx.start(m, ["andy", co])


def cpu_explode(ctx, colour, five=False):
    m, a = cpu_setup(ctx, colour, five)
    m.unit(1, "tank", 10, 6).unit(1, "mdtank", 11, 6).unit(1, "tank", 10, 7).unit(1, "artillery", 11, 7)
    m.unit(a, "blackbomb", 16, 6)
    if five:
        m.unit(a, "infantry", 16, 11)
    g = cpu_start(ctx, m, five)
    seen = cpu_turn(ctx, g, explosion_check(), "cpu_boom")
    ctx.check("explosion" in seen, "the CPU's bomb shows Dual Strike's explosion")
    ctx.eq([u for u in g.units(a) if u["type"] == 13], [], "the bomb is gone")


def cpu_stealth(ctx, colour, five=False):
    m, a = cpu_setup(ctx, colour, five)
    m.unit(1, "antiair", 10, 6).unit(1, "missiles", 9, 8)
    m.unit(a, "stealth", 14, 6)
    if five:
        m.unit(a, "infantry", 16, 11)
    g = cpu_start(ctx, m, five)
    seen = cpu_turn(ctx, g, stealth_check(), "cpu_hide")
    s = [u for u in g.units(a) if u["type"] == 12]
    ctx.check(s and s[0]["flags"] & 0x20, "the CPU's Stealth hid")
    ctx.check("stealth" in seen, "with Dual Strike's Stealth cloud")


def cpu_repair(ctx, colour, five=False):
    m, a = cpu_setup(ctx, colour, five)
    for x in range(8, 22):
        for y in range(12, 18):
            m.terrain(x, y, "sea")
    m.unit(1, "infantry", 2, 2)
    m.unit(a, "blackboat", 15, 14).unit(a, "cruiser", 16, 14).unit(a, "lander", 15, 15)
    if five:
        m.unit(a, "infantry", 16, 11)
    g = cpu_start(ctx, m, five)
    ctx.set_hp(g, 16, 14, 50)
    g.e.w32(g.player(a)["addr"], 20000)
    seen = cpu_turn(ctx, g, repair_check(), "cpu_repair")
    cruiser = [u for u in g.units(a) if u["type"] == 22][0]
    ctx.check(cruiser["hp"] >= 60, f"the Black Boat repaired the Cruiser: hp {cruiser['hp']}")
    ctx.check("repair" in seen, "with Dual Strike's REPAIR label")


def cpu_power(ctx, colour, five=False, co="grimm", which="power"):
    m, a = cpu_setup(ctx, colour, five)
    m.unit(1, "tank", 10, 6).unit(1, "infantry", 5, 5)
    m.unit(a, "tank", 14, 6).unit(a, "artillery", 18, 8).unit(a, "mech", 16, 10)
    g = cpu_start(ctx, m, five, co)
    if five:
        g.e.w8(g.player(a)["addr"] + 0x1D, romlib.co_id(co))
    seen = set()
    for turn in range(3):
        g.charge_power(a, which)
        seen |= cpu_turn(ctx, g, power_check(co, 1 if which == "super" else 0), f"cpu_power{turn}")
        if g.player(a)["powers_used"]:
            break
    ctx.eq(g.player(a)["powers_used"], 1, "the CPU fired its power")
    ctx.check("power" in seen, "with Dual Strike's power effect on its units")


for c in range(1, 6):
    n = COLOURS[c - 1]
    make(cpu_explode, f"cpu_map_anim_explode_{n}", c)
    make(cpu_stealth, f"cpu_map_anim_stealth_{n}", c)
    make(cpu_repair, f"cpu_map_anim_repair_{n}", c)
    make(cpu_power, f"cpu_map_anim_power_{n}", c)
make(cpu_explode, "cpu_map_anim_explode_five_black_hole", 5, True)
make(cpu_stealth, "cpu_map_anim_stealth_five_black_hole", 5, True)
make(cpu_repair, "cpu_map_anim_repair_five_black_hole", 5, True)
make(cpu_power, "cpu_map_anim_power_five_black_hole", 5, True)


# --- Sandstorm ------------------------------------------------------------------------

def sand_tiles():
    t = ds().file("bmap/0b2")
    dots = lambda k: sum((b & 15 != 0) + (b >> 4 != 0) for b in t[32 * k:32 * k + 32])
    order = sorted(range(len(t) // 32), key=lambda k: (-dots(k), k))
    return b"".join(t[32 * k:32 * k + 32] for k in order[:3])


@test(modes=DS)
def map_anim_sandstorm(ctx):
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["andy", "andy"], weather="sandstorm")
    checks = {"sand": (0x06010000 + 32 * 0x173, sand_tiles()),
              "colours": (OBJ_PAL + 7 * 32 + 10, struct.pack("<HH", 0x4BDF, 0x439F))}
    seen = watch(ctx, g, 120, "sand", checks)
    ctx.check(seen == {"sand", "colours"}, f"Dual Strike's sand blows: {sorted(seen)}")


@test(modes=DS)
def cpu_map_anim_sandstorm_five_black_hole(ctx):
    m, a = cpu_setup(ctx, 5, True)
    m.unit(1, "tank", 10, 6).unit(a, "tank", 14, 6)
    g = ctx.start(m, None, weather="sandstorm")
    checks = {"sand": (0x06010000 + 32 * 0x173, sand_tiles())}
    seen = cpu_turn(ctx, g, checks, "cpu_sand", frames=600)
    ctx.check("sand" in seen, "Dual Strike's sand blows on the CPU's turns too")


@test(modes=DS, netplay=True)
def netplay_map_anim(ctx):
    """The CPU's bomb, Stealth and Black Boat turn end play the same on both
    peers and in the replay."""
    m = ctx.map()
    for x in range(8, 22):
        for y in range(14, 19):
            m.terrain(x, y, "sea")
    m.unit(1, "tank", 10, 6).unit(1, "mdtank", 11, 6).unit(1, "tank", 10, 7).unit(1, "antiair", 11, 7)
    m.unit(2, "blackbomb", 16, 6).unit(2, "stealth", 16, 9)
    m.unit(2, "blackboat", 15, 15).unit(2, "cruiser", 16, 15)
    g = ctx.start(m, ["andy", "andy"])
    g.end_turn(human=1)
    g.end_turn(human=1)
    identical, _, text = ctx.netplay_replay(g, [])
    ctx.log("\n".join(l for l in text.splitlines() if not l.startswith("peek")))
    ctx.check(identical, "netplay: both peers and the straight replay identical")
