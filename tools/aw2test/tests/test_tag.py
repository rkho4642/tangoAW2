"""CO tag pairs (tangoAW2's crate::tag, crate::tag_ui; docs/AW2.md "CO tag
pairs"): Dual Strike's Change and Tag Power in AW2's battles.

Dual Strike's numbers are read from the .nds (the pair's compatibility,
CO record +0x84); the damage is the calculator's (aw2test/damage.py, which
adds the Tag Power's firepower); the power meters are AW2's formula, the
partner's half of the active CO's."""

import os

from aw2test.harness import Skip, test
from aw2test import dscampaign as dc
from aw2test import paths, ram, saves, tag
from aw2test import rom as romlib
from aw2test.emu import Emu
from aw2test.game import Game

TEAMS = ram.TEAMS
TEAMS_CURSOR = TEAMS + 0x32


def tag_battle(ctx, cos, partners, humans=(1,), units=(), fog=False):
    """A Versus battle on the harness's plains with the partners set in RAM
    (None: single; the Teams screen's picks are tag_teams_screen's)."""
    m = ctx.map()
    for u in units:
        m.unit(*u)
    g = ctx.boot_teams(m)
    for a, p in enumerate(partners, 1):
        tag.set_teams_partner(g.e, a, p)
    g.set_teams(cos, set(humans))
    g.teams_to_rules()
    g.set_rules(fog=fog)
    g.start_battle()
    g.wait_for_input()
    return g


def teams_pick(g, cos, downs, humans=(1,)):
    """On the Teams screen through the pad only (as netplay plays it): the
    COs, then each army's partner `downs[a]` DOWNs from None (START,
    DOWN.., START): an army with a partner is a pair."""
    e = g.e
    g.set_teams(cos, set(humans))
    for _ in range(2 * (len(cos) - 1)):
        e.press("LEFT", 6)
        e.wait(14)
    for a, n in enumerate(downs):
        if a:
            e.press("RIGHT", 6); e.wait(14); e.press("RIGHT", 6); e.wait(14)
        if n:
            e.press("START", 4)
            e.wait(10)
            for _ in range(n):
                e.press("DOWN", 4)
                e.wait(10)
            e.press("START", 4)
            e.wait(10)


def oam(e):
    """The OAM's shown sprites: (attr0, attr1, attr2)."""
    import struct
    b = e.read(0x07000000, 0x400)
    out = []
    for i in range(128):
        a0, a1, a2 = struct.unpack_from("<HHH", b, 8 * i)
        if (a0 >> 8) & 3 != 2 and (a0 & 0xFF) < 160:
            out.append((a0, a1, a2))
    return out


def fill(g, army, active=True, partner=True):
    """Meters to their Super Powers' costs (AW2's units)."""
    e = g.e
    if active:
        p = g.player(army)
        e.w32(p["addr"] + ram.P_CHARGE, tag.star_cost(p["powers_used"]) * g.co_stars(p["co"])[1])
    if partner:
        t = tag.partner(e, army)
        e.w32(tag.rec(army) + tag.P_CHARGE, tag.star_cost(t["uses"]) * g.co_stars(t["co"])[1])


@test(modes=("ds",))
def tag_versus_single_by_default(ctx):
    """As Dual Strike's Versus: no rule; an army is single unless a partner
    is picked for it on Teams (the default None): no pair, the map menu
    AW2's (no Change, no Tag); every army's partner slot shows None."""
    g = tag_battle(ctx, ["andy", "olaf"], [None, None])
    e = g.e
    ctx.eq(tag.partner(e, 1), None, "army 1 has no partner")
    ctx.eq(tag.partner(e, 2), None, "army 2 has no partner")
    ctx.eq(e.u32(tag.MENU_POOL), 0x0849AAC0, "the map menu is AW2's own table")
    names = g.map_menu_names()
    ctx.check("Change" not in names and "Tag" not in names, f"no Change or Tag on the map menu ({names})")
    m = ctx.map()
    g2 = ctx.boot_teams(m)
    g2.set_teams(["andy", "olaf"], {1})
    g2.e.wait(10)
    ctx.eq([g2.e.u8(tag.TEAMS_PARTNER + a) for a in range(2)], [tag.NONE, tag.NONE], "None on a fresh Teams screen")
    boxes = sorted(s[2] & 0x3FF for s in oam(g2.e) if (s[2] & 0x3FF) in (0x100, 0x110))
    ctx.eq(boxes, [0x100, 0x110], "each army's partner slot shown (None)")
    ctx.shot(g2, "teams_none")
    ctx.eq(g2.e.u8(g2.SKILLS_RULE), 0, "Skills off on a fresh Teams screen")


@test(modes=("ds",))
def tag_rules_rows(ctx):
    """The Rules screen's Skills row (crate::versus_rules): RIGHT from
    Visuals reaches it, then wraps to Fog; LEFT from Fog comes back to it;
    UP turns it ON and DOWN OFF; it starts OFF; the game's own rules are
    unchanged by it. No CO Tag row (tag pairs need no rule)."""
    m = ctx.map()
    g = ctx.boot_teams(m)
    e = g.e
    g.set_teams(["andy", "olaf"], {1})
    g.teams_to_rules()
    e.wait(30)
    cursor = g.teams_addr() + ram.RULES_CURSOR - ram.TEAMS
    rules = g.teams_addr() + 0x84
    game_rules = e.read(rules, 8)
    for _ in range(6):
        e.press("RIGHT", 6)
        e.wait(14)
    ctx.eq(e.u8(cursor), 6, "Visuals")
    e.press("RIGHT", 6)
    e.wait(20)
    ctx.eq((e.u8(g.VRULE_CURSOR), e.u8(cursor)), (1, 6), "RIGHT on Visuals: Skills")
    ctx.eq(e.u8(g.SKILLS_RULE), 0, "Skills OFF by default")
    e.press("UP", 6)
    e.wait(20)
    ctx.eq(e.u8(g.SKILLS_RULE), 1, "UP: Skills ON")
    ctx.shot(g, "rules_skills_on")
    e.press("DOWN", 6)
    e.wait(20)
    ctx.eq(e.u8(g.SKILLS_RULE), 0, "DOWN: Skills OFF")
    e.press("UP", 6)
    e.wait(20)
    e.press("RIGHT", 6)
    e.wait(20)
    ctx.eq((e.u8(g.VRULE_CURSOR), e.u8(cursor)), (0, 0), "RIGHT on Skills: Fog")
    e.press("LEFT", 6)
    e.wait(20)
    ctx.eq(e.u8(g.VRULE_CURSOR), 1, "LEFT on Fog: Skills")
    e.press("LEFT", 6)
    e.wait(20)
    ctx.eq((e.u8(g.VRULE_CURSOR), e.u8(cursor)), (0, 6), "LEFT: Visuals")
    ctx.eq(e.read(rules, 8), game_rules, "the game's own rules unchanged")
    ctx.eq(e.u8(g.SKILLS_RULE), 1, "Skills ON")


@test(modes=("ds",))
def tag_teams_screen(ctx):
    """The partners on the Teams screen (START, then UP/DOWN), the battle
    starting with them as pairs; human and computer armies alike."""
    m = ctx.map()
    g = ctx.boot_teams(m)
    e = g.e
    g.set_teams(["andy", "olaf"], {1})
    for _ in range(2):
        e.press("LEFT", 6)
        e.wait(14)
    ctx.eq(e.u8(TEAMS_CURSOR), 0, "army 1's CO stop")
    e.press("START", 4)
    e.wait(10)
    ctx.eq(e.u8(tag.STATE + 0xC0), 0, "START: the D-pad edits army 1's partner")
    ctx.shot(g, "teams_edit_none")
    lst = g.teams()["co_list"]
    for _ in range(3):
        e.press("DOWN", 4)
        e.wait(10)
    main1 = lst[e.u8(TEAMS + 0x1C)]
    others = [c for c in lst if c != main1]
    ctx.eq(e.u8(tag.TEAMS_PARTNER), others[2], "three DOWNs: the third CO of the list but army 1's own")
    ctx.eq(lst[e.u8(TEAMS + 0x1C)], main1, "army 1's CO is unchanged")
    ctx.shot(g, "teams_edit_partner")
    e.press("START", 4)
    e.wait(10)
    # Army 2 (the computer's): its partner the CO before the end of the list.
    e.press("RIGHT", 6); e.wait(14); e.press("RIGHT", 6); e.wait(14)
    e.press("START", 4)
    e.wait(10)
    e.press("UP", 4)
    e.wait(10)
    main2 = lst[e.u8(TEAMS + 0x1D)]
    want2 = [c for c in lst if c != main2][-1]
    ctx.eq(e.u8(tag.TEAMS_PARTNER + 1), want2, "army 2: UP from None is the list's last CO")
    e.press("START", 4)
    e.wait(10)
    ctx.shot(g, "teams_pairs")
    # The partner boxes (32x32 sprites, OBJ tiles 0x100 + 16 an army) under
    # the columns, which go up 16 pixels to make room.
    boxes = {s[2] & 0x3FF: (s[1] & 0x1FF, s[0] & 0xFF) for s in oam(e) if (s[2] & 0x3FF) in (0x100, 0x110)}
    ctx.eq(sorted(boxes), [0x100, 0x110], "a partner box for each army")
    faces = [s for s in oam(e) if s[2] & 0x3FF == 400]
    ctx.eq([f[0] & 0xFF for f in faces], [36], "army 1's face 16 pixels up")
    ctx.eq(boxes.get(0x100, (0, 0))[1], 36 + 57, "army 1's partner box under it")
    g.teams_to_rules()
    g.set_rules()
    g.start_battle()
    g.wait_for_input()
    ctx.eq(g.player(1)["co"], main1, "army 1's CO")
    ctx.eq((tag.partner(e, 1) or {}).get("co"), others[2], "army 1's partner")
    ctx.eq((tag.partner(e, 2) or {}).get("co"), want2, "army 2's (the computer's) partner")
    # The CO panel: the partner's HUD face in its tiles.
    face = e.read(e.u32(0x080437EC) + 0x100 * others[2], 0x100)
    ctx.eq(e.read(0x06010000 + 32 * 0x309, 0x100), face, "the CO panel shows the partner's face (OBJ tiles 0x309..)")
    # The partner's strip under the panel: its face 37 pixels under the
    # panel's top, the strip's plate (the header's tiles 16..31, palette 7).
    sprites = oam(e)
    header = next(((s[1] & 0x1FF, s[0] & 0xFF) for s in sprites if s[2] & 0x3FF == 0 and s[0] >> 14 == 1), None)
    pface = next(((s[1] & 0x1FF, s[0] & 0xFF) for s in sprites if s[2] & 0x3FF == 0x309), None)
    ctx.require(header is not None and pface is not None, "the panel and the partner's face are drawn")
    ctx.eq((pface[0] - header[0], pface[1] - header[1]), (2, 37), "the partner's face in its strip under the panel")
    plate = [s for s in sprites if (s[2] & 0x3FF) in (16, 20, 24, 28) and s[0] >> 14 == 1]
    ctx.eq(len(plate), 8, "the strip's plate: eight 32x8 pieces of the panel's own tiles")
    ctx.shot(g, "battle_panel")


@test(modes=("ds",))
def tag_versus_five_armies(ctx):
    """Five armies (Black Hole the fifth): each army's partner slot on the
    Teams screen; Orange Star, Green Earth, Yellow Comet and Black Hole
    with two COs play as pairs, Blue Moon with one plays single. Black
    Hole's partner picked with the pad."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)
    m.unit(5, "infantry", 16, 10)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.boot_teams(m)
    e = g.e
    e.wait(30)
    ctx.eq(e.u8(ram.FIVE_ON), 1, "a five-army game")
    boxes = sorted(s[2] & 0x3FF for s in oam(e) if (s[2] & 0x3FF) in (0x100, 0x110, 0x120, 0x130, 0x140))
    ctx.eq(boxes, [0x100, 0x110, 0x120, 0x130, 0x140], "five partner slots, empty")
    for _ in range(12):
        if e.u8(g.teams_addr() + 0x32) == 8:
            break
        e.press("RIGHT", 6)
        e.wait(14)
    e.press("START", 4)
    e.wait(10)
    for _ in range(3):
        e.press("DOWN", 4)
        e.wait(10)
    e.press("START", 4)
    e.wait(10)
    bh = e.u8(tag.TEAMS_PARTNER + 4)
    ctx.check(bh != tag.NONE, "Black Hole's partner picked with the pad")
    for a, p in ((1, "max"), (3, "eagle"), (4, "drake")):
        tag.set_teams_partner(e, a, p)
    e.wait(20)
    ctx.shot(g, "teams_five")
    g.teams_to_rules()
    g.set_rules()
    g.start_battle()
    g.wait_for_input()
    got = [(tag.partner(e, a) or {}).get("co") for a in range(1, 6)]
    want = [romlib.co_id("max"), None, romlib.co_id("eagle"), romlib.co_id("drake"), bh]
    ctx.eq(got, want, "pairs for the armies with two COs, Blue Moon single")


@test(modes=("ds",))
def tag_change(ctx):
    """Change: the active CO's power ends, the COs swap (CO, meter, power
    count), the turn ends; the new CO's day-to-day is the army's (an
    attack against the calculator); the partner keeps its meter."""
    g = tag_battle(ctx, ["max", "olaf"], ["andy", None], units=[(1, "tank", 10, 10), (2, "tank", 12, 10)])
    e = g.e
    p = g.player(1)
    e.w32(p["addr"] + ram.P_CHARGE, 5000)
    e.w32(tag.rec(1) + tag.P_CHARGE, 7000)
    e.w8(tag.rec(1) + tag.P_USES, 2)
    names = g.open_map_menu()["names"]
    ctx.eq(names, ["CO", "Intel", "Options", "Save", "Change", "End"], "the map menu with a partner")
    g.choose("Change", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.current_army() == 2, 900, step=8), "Change ends the turn")
    p = g.player(1)
    t = tag.partner(e, 1)
    ctx.eq((p["co"], p["charge"], p["powers_used"]), (romlib.co_id("andy"), 7000, 2), "Andy is the active CO, with his meter and power count")
    ctx.eq((t["co"], t["charge"], t["uses"]), (romlib.co_id("max"), 5000, 0), "Max is the partner, with his")
    ctx.require(e.wait_until(lambda: g.current_army() == 1, 20000, step=30), "the turn comes back")
    g.wait_for_input()
    ctx.eq(g.player(1)["co"], romlib.co_id("andy"), "Andy plays the next turn")
    # Max's day-to-day (+20% direct) is gone: Andy's numbers in the damage.
    u = [x for x in g.units(1) if x["type"] == romlib.unit_id("tank")][0]
    enemy = [x for x in g.units(2) if x["type"] == romlib.unit_id("tank")][0]
    ex, ey = enemy["x"], enemy["y"]
    spot = next(((x, y) for x, y in ((ex - 1, ey), (ex + 1, ey), (ex, ey - 1), (ex, ey + 1)) if g.unit_at(x, y) is None and 0 <= x < 30 and 0 <= y < 20), None)
    if spot and abs(spot[0] - u["x"]) + abs(spot[1] - u["y"]) <= 6:
        r = ctx.attack(g, (u["x"], u["y"]), spot, (ex, ey))
        ctx.eq(r["first"].acc >= 100, True, "an attack with Andy's numbers")


@test(modes=("ds",))
def tag_meters(ctx):
    """The partner's meter: half the active CO's charge from a battle (both
    armies' pairs), up to its Super Power's cost; nothing while a power is
    on (ctx.attack checks every meter)."""
    g = tag_battle(ctx, ["andy", "olaf"], ["max", "sami"], units=[(1, "tank", 10, 10), (2, "tank", 11, 10), (1, "artillery", 5, 10), (2, "tank", 7, 10)])
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    e = g.e
    t = tag.partner(e, 1)
    cap = tag.star_cost(0) * g.co_stars(t["co"])[1]
    e.w32(tag.rec(1) + tag.P_CHARGE, cap - 1000)
    ctx.attack(g, (5, 10), (5, 10), (7, 10))
    ctx.eq(tag.partner(e, 1)["charge"], cap, "the partner's meter stops at its Super Power's cost")


@test(modes=("ds",))
def tag_power(ctx):
    """Tag Power: offered with both meters full; the active CO's Super Power
    (first half: End hidden, Change goes on), then the partner's (the COs
    swap, every unit moves again); both meters spent; the firepower of the
    pair's compatibility (Max and Andy: 110) in both halves, against the
    calculator; both halves end at the army's next turn."""
    units = [(1, "tank", 10, 10), (2, "tank", 11, 10), (1, "tank", 10, 12), (2, "infantry", 11, 12), (1, "infantry", 3, 3)]
    g = tag_battle(ctx, ["max", "olaf"], ["andy", None], units=units)
    e = g.e
    for active, partner, label in ((True, False, "only the active CO's meter full"), (False, True, "only the partner's")):
        fill(g, 1, active=active, partner=partner)
        if not active:
            e.w32(g.player(1)["addr"] + ram.P_CHARGE, 0)
        else:
            e.w32(tag.rec(1) + tag.P_CHARGE, 0)
        names = g.open_map_menu()["names"]
        ctx.check("Tag" not in names, f"no Tag with {label} ({names})")
        e.press("B", 4)
        e.wait(30)
        g.wait_for_input()
    fill(g, 1)
    names = g.open_map_menu()["names"]
    ctx.eq(names, ["CO", "Intel", "Power", "Super", "Tag", "Options", "Save", "Change", "End"], "Tag Power offered")
    ctx.shot(g, "menu_tag")
    g.choose("Tag", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.player(1)["co_mode"] == 2, 3000, step=10), "the first Super Power starts")
    g.wait_for_input()
    p, t = g.player(1), tag.partner(e, 1)
    ctx.eq((p["co"], p["co_mode"], p["charge"], t["phase"]), (romlib.co_id("max"), 2, 0, 1), "Max's Super Power, the first half")
    compat = tag.compatibility(romlib.DualStrike(), romlib.co_id("max"), romlib.co_id("andy"))
    ctx.eq(compat, 110, "Max and Andy's compatibility in Dual Strike")
    ctx.eq(ctx.tag_firepower(g, 1, p["co"]), 10, "the Tag Power's firepower: +10")
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    g.wait_unit(3, 3)
    m = g.open_map_menu()
    ctx.eq(m["names"], ["CO", "Intel", "Options", "Save", "Change"], "first half: Change goes on, End is hidden")
    ctx.shot(g, "menu_first_half")
    g.choose("Change", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.player(1)["co"] == romlib.co_id("andy") and g.player(1)["co_mode"] == 2, 3000, step=10),
                "the second half: Andy's Super Power")
    g.wait_for_input()
    p, t = g.player(1), tag.partner(e, 1)
    ctx.eq((p["co_mode"], p["charge"], p["powers_used"], t["phase"], t["co"], t["uses"]), (2, 0, 1, 2, romlib.co_id("max"), 1),
           "Andy's Super Power, Max the partner with one power used")
    moved = [u for u in g.units(1) if u["flags"] & 1]
    ctx.eq(moved, [], "every unit may move again")
    ctx.attack(g, (10, 12), (10, 12), (11, 12))
    m = g.open_map_menu()
    ctx.eq(m["names"], ["CO", "Intel", "Options", "Save", "End"], "second half: End")
    ctx.shot(g, "menu_second_half")
    g.choose("End", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.current_army() == 2, 900, step=8), "the turn ends")
    ctx.eq(tag.partner(e, 1)["phase"], 2, "the Tag Power holds through the other army's turn")
    ctx.require(e.wait_until(lambda: g.current_army() == 1, 20000, step=30), "the turn comes back")
    g.wait_for_input()
    ctx.eq((tag.partner(e, 1)["phase"], g.player(1)["co_mode"]), (0, 0), "over at the army's next turn")


# Measured in Dual Strike (melonDS, computer against computer, the tag term
# its firepower function adds, 0x020E5D48): Andy+Max +10, Andy+Eagle +15,
# Sami+Eagle +20, Andy+Von Bolt -10, Koal+Rachel -35, a 100 pair 0; only in
# a Tag Power's halves; never on defence.
BOOST_PAIRS = [("andy", "max", 10), ("andy", "eagle", 15), ("sami", "eagle", 20),
               ("andy", "vonbolt", -10), ("koal", "rachel", -35), ("kanbei", "sonja", 30), ("jess", "colin", 0)]


@test(modes=("ds",))
def tag_boost_pairs(ctx):
    """Every pair's Tag Power firepower is Dual Strike's compatibility - 100
    (CO record +0x84 by partner): the whole 28x28 table read by the game
    (tangoAW2's) and by this harness from the .nds agree with the measured
    pairs; in battle, damage with a Tag Power under way against the damage
    calculator for a representative set (stars do nothing: Hachi and Sensei
    are 2 stars at 100), and none when the pair is not in a Tag Power; the
    defender's pair gives no defence."""
    ds = romlib.DualStrike()
    for a, b, want in BOOST_PAIRS:
        ctx.eq(tag.compatibility(ds, romlib.co_id(a), romlib.co_id(b)) - 100, want, f"{a}+{b}: the .nds table")
    cos = list(range(0, 19)) + list(range(72, 81))
    seen = {}
    for a in cos:
        for b in cos:
            v = tag.compatibility(ds, a, b)
            seen[v] = seen.get(v, 0) + 1
    ctx.log(f"compatibility values over the 28x28 table: {sorted(seen.items())}")
    ctx.check(all(60 <= v <= 130 for v in seen), "every compatibility in Dual Strike's range")
    for a, b, want in BOOST_PAIRS:
        units = [(1, "tank", 10, 10), (2, "tank", 11, 10), (1, "tank", 10, 12), (2, "tank", 11, 12)]
        g = tag_battle(ctx, [a, "olaf"], [b, "max"], units=units)
        e = g.e
        # A Tag Power's first half for both armies (the phase byte; no power
        # on, so only the pair's term moves the numbers).
        e.w8(tag.rec(1) + 1, 1)
        e.w8(tag.rec(2) + 1, 1)
        ctx.eq(ctx.tag_firepower(g, 1, g.player(1)["co"]), want, f"{a}+{b}: the calculator's tag firepower")
        ctx.attack(g, (10, 10), (10, 10), (11, 10))
        e.w8(tag.rec(1) + 1, 0)
        e.w8(tag.rec(2) + 1, 0)
        ctx.attack(g, (10, 12), (10, 12), (11, 12))


@test(modes=("ds",))
def tag_cpu(ctx):
    """The computer: Tag Power with both meters full (both halves, its units
    moving in each), and Change at its turn's end as Dual Strike's AI does
    (both meters empty: to the CO it rates higher, Sami over Max)."""
    units = [(1, "tank", 10, 10), (2, "tank", 13, 10), (2, "infantry", 20, 15), (2, "mech", 21, 15)]
    g = tag_battle(ctx, ["andy", "max"], [None, "sami"], units=units)
    e = g.e
    fill(g, 2)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.current_army() == 2, 900, step=8), "the computer's turn")
    seen = []
    n = 0
    while g.current_army() == 2 and n < 30000:
        p, t = g.player(2), tag.partner(e, 2)
        st = (p["co"], p["co_mode"], t["phase"])
        if not seen or seen[-1] != st:
            seen.append(st)
        e.wait(10)
        n += 10
    max_, sami = romlib.co_id("max"), romlib.co_id("sami")
    ctx.log(f"states {seen}")
    ctx.check((max_, 2, 1) in seen, "the computer's Tag Power: Max's Super Power first")
    ctx.check((sami, 2, 2) in seen, "then Sami's, in the same turn")
    t = tag.partner(e, 2)
    ctx.eq((t["co"], t["uses"], g.player(2)["powers_used"]), (max_, 1, 1), "both Super Powers used")
    # Change: a new battle, both meters empty.
    g2 = tag_battle(ctx, ["andy", "max"], [None, "sami"], units=units)
    g2.open_map_menu()
    g2.choose("End", g2.MAP_MENU)
    e2 = g2.e
    ctx.require(e2.wait_until(lambda: g2.current_army() == 2, 900, step=8), "the computer's turn")
    ctx.require(e2.wait_until(lambda: g2.current_army() == 1, 30000, step=30), "and back")
    ctx.eq((g2.player(2)["co"], tag.partner(e2, 2)["co"]), (sami, max_), "the computer Changed to Sami")


def watch_cpu(g, army, frames=30000, talk=False):
    """The computer army's turn, from its start until it ends: the
    (active CO, power mode, phase) states it went through (`talk`: A now
    and then, for a mission's dialogue)."""
    e = g.e
    seen = []
    n = 0
    while g.current_army() == army and n < frames:
        p, t = g.player(army), tag.partner(e, army)
        st = (p["co"], p["co_mode"], (t or {}).get("phase"))
        if not seen or seen[-1] != st:
            seen.append(st)
        if talk and n % 60 == 0:
            e.press("A", 2)
        e.wait(10)
        n += 10
    return seen


def end_and_watch(ctx, g, army=2):
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(g.e.wait_until(lambda: g.current_army() == army, 900, step=8), "the computer's turn")
    seen = watch_cpu(g, army)
    ctx.log(f"computer states {seen}")
    ctx.require(g.e.wait_until(lambda: g.current_army() == 1, 30000, step=30), "the turn comes back")
    g.wait_for_input()
    return seen


@test(modes=("ds",))
def tag_cpu_versus_partner(ctx):
    """Versus, as Dual Strike's: the player gives the computer's army a
    partner on Teams (START on its CO stop, DOWN): it plays as a pair; left
    at None it plays single."""
    m = ctx.map()
    g = ctx.boot_teams(m)
    e = g.e
    teams_pick(g, ["andy", "max"], [0, 2])
    want = e.u8(tag.TEAMS_PARTNER + 1)
    ctx.check(want != tag.NONE, "the computer's partner picked on Teams")
    ctx.eq(e.u8(tag.TEAMS_PARTNER), tag.NONE, "the human's left at None")
    ctx.shot(g, "teams_cpu_partner")
    g.teams_to_rules()
    g.set_rules()
    g.start_battle()
    g.wait_for_input()
    ctx.eq(tag.partner(e, 1), None, "the human army: single")
    ctx.eq((tag.partner(e, 2) or {}).get("co"), want, "the computer's army: the pair picked for it")
    g2 = tag_battle(ctx, ["andy", "max"], [None, None])
    ctx.eq(tag.partner(g2.e, 2), None, "None: the computer plays single")
    ctx.eq(g2.e.u32(tag.MENU_POOL), 0x0849AAC0, "no pairs: AW2's map menu")


@test(modes=("ds",))
def tag_cpu_change_and_powers(ctx):
    """The computer's Change as Dual Strike's AI (0x02099BA0): with the
    active CO nearer its Super Power than the partner it Changes at its
    turn's end; then the partner, now active, uses its own CO Power when
    its meter allows (AW2's AI deciding, the pair's rules on top)."""
    units = [(1, "tank", 10, 10), (2, "tank", 13, 10), (2, "infantry", 20, 15), (2, "mech", 21, 15)]
    g = tag_battle(ctx, ["andy", "max"], [None, "sami"], units=units)
    e = g.e
    max_, sami = romlib.co_id("max"), romlib.co_id("sami")
    p = g.player(2)
    mcop, mscop = g.co_stars(max_)
    scop_s = g.co_stars(sami)
    # Max: 40% of his Super Power (under his CO Power); Sami: past her CO
    # Power, far from her Super Power.
    e.w32(p["addr"] + ram.P_CHARGE, tag.star_cost(0) * mscop * 2 // 5)
    e.w32(tag.rec(2) + tag.P_CHARGE, tag.star_cost(0) * scop_s[0] + 1000)
    seen = end_and_watch(ctx, g)
    ctx.check(all(st[1] == 0 for st in seen), "no power this turn (Max short of his, Sami not active)")
    ctx.eq((g.player(2)["co"], tag.partner(e, 2)["co"]), (sami, max_), "the computer Changed to Sami (Max nearer his Super Power)")
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.current_army() == 2, 900, step=8), "the computer's turn")
    e.wait(60)
    ctx.shot(g, "cpu_after_change")
    seen = watch_cpu(g, 2)
    ctx.log(f"computer states {seen}")
    ctx.check((sami, 1, 0) in seen, "Sami's CO Power on the computer's next turn")


@test(modes=("ds",))
def tag_cpu_ds_mission(ctx):
    """A DS Campaign tag mission (Tag Battle: the computer's Jugger and
    Lash): with both its meters full the computer fires the Tag Power, both
    Super Powers in one turn."""
    data = dc.DsData()
    step = 7
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=step)
    d.choose_cos(1, dc.CO_PREFS)
    d.wait_control()
    army = next((a for a in range(2, 5) if tag.partner(e, a) is not None), None)
    ctx.require(army is not None, "the computer has a pair")
    a_co, b_co = g.player(army)["co"], tag.partner(e, army)["co"]
    fill(g, army)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.current_army() == army, 3000, step=8), "the computer's turn")
    seen = watch_cpu(g, army, 40000, talk=True)
    ctx.log(f"states {seen}")
    ctx.check((a_co, 2, 1) in seen, "the Tag Power: the active CO's Super Power first")
    ctx.check((b_co, 2, 2) in seen, "then the partner's, in the same turn")
    ctx.shot(g, "after_cpu_tag")


@test(modes=("ds",))
def tag_save_versus(ctx):
    """A Versus game saved in a Tag Power's first half and continued after a
    reboot: both COs, their meters and power counts, the phase; the second
    half then plays."""
    units = [(1, "tank", 10, 10), (2, "tank", 11, 10)]
    g = tag_battle(ctx, ["max", "olaf"], ["andy", "sami"], units=units)
    e = g.e
    fill(g, 1)
    e.w32(tag.rec(2) + tag.P_CHARGE, 12345)
    g.open_map_menu()
    g.choose("Tag", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: g.player(1)["co_mode"] == 2, 3000, step=10), "Tag Power")
    g.wait_for_input()
    want = (tag.partner(e, 1), tag.partner(e, 2), g.player(1)["co"], g.player(1)["co_mode"])
    snap = saves.snapshot(g)
    saves.suspend(g)
    path = e.save(os.path.join(ctx.out, "saved"))
    e2 = Emu(save=path, ds=True)
    g2 = Game(e2)
    ctx.games.append(g2)
    saves.to_select_mode(e2)
    saves.versus_continue(g2)
    saves.compare_snapshots(ctx, snap, saves.snapshot(g2), "continued")
    have = (tag.partner(e2, 1), tag.partner(e2, 2), g2.player(1)["co"], g2.player(1)["co_mode"])
    ctx.eq(have, want, "both pairs, meters, power counts and the phase kept")
    ctx.eq(e2.u8(g2.SKILLS_RULE), e.u8(g.SKILLS_RULE), "the Rules screen's Skills row kept")
    names = g2.open_map_menu()["names"]
    ctx.eq(names, ["CO", "Intel", "Options", "Save", "Change"], "still the first half")
    g2.choose("Change", g2.MAP_MENU)
    ctx.require(e2.wait_until(lambda: g2.player(1)["co"] == romlib.co_id("andy") and g2.player(1)["co_mode"] == 2, 3000, step=10),
                "the second half after Continue")


@test(modes=("ds",), netplay=True)
def tag_netplay(ctx):
    """A battle with pairs (the rule and partners picked with the pad, Tag
    Power both halves, End) replayed on two rollback peers: identical, and
    the pairs as played."""
    m = ctx.map()
    for u in [(1, "tank", 10, 10), (2, "tank", 11, 10), (1, "infantry", 3, 3)]:
        m.unit(*u)
    g = ctx.boot_teams(m)
    e = g.e
    teams_pick(g, ["max", "olaf"], [1, 3])
    g.teams_to_rules()
    g.set_rules()
    g.set_extra_rules(skills=True)
    ctx.check(e.u8(g.SKILLS_RULE) == 1 and e.u8(tag.TEAMS_PARTNER) != tag.NONE,
              "the rule and partners picked with the pad")
    g.start_battle()
    g.wait_for_input()
    ctx.require(tag.partner(e, 1) is not None, "army 1 has a partner")
    fill(g, 1)
    g.open_map_menu()
    g.choose("Tag", g.MAP_MENU)
    e.wait_until(lambda: g.player(1)["co_mode"] == 2, 3000, step=10)
    g.wait_for_input()
    g.wait_unit(3, 3)
    partner = tag.partner(e, 1)["co"]
    g.open_map_menu()
    g.choose("Change", g.MAP_MENU)
    e.wait_until(lambda: g.player(1)["co"] == partner and g.player(1)["co_mode"] == 2, 3000, step=10)
    g.wait_for_input()
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    e.wait_until(lambda: g.current_army() == 1, 20000, step=30)
    g.wait_for_input()
    want = e.read(tag.STATE, 0x40)
    players = e.read(g.players_base + 0x3C, 0x3C * 2)
    identical, values, text = ctx.netplay_replay(g, [(tag.STATE, 0x40), (g.players_base + 0x3C, 0x3C * 2),
                                                     (g.SKILLS_RULE, 1), (tag.TEAMS_PARTNER, 5)])
    ctx.check(identical, "both peers identical")
    ctx.eq(values.get(g.SKILLS_RULE), bytes([1]), "the Skills rule ON on the peers")
    ctx.eq(values.get(tag.STATE), want, "the pairs as played")
    ctx.eq(values.get(g.players_base + 0x3C), players, "the players as played")


@test(modes=("aw2",))
def tag_pack_off(ctx):
    """Without the pack nothing of it: START on the Teams screen does
    nothing, the Rules screen has no Skills or CO Tag rows (RIGHT on
    Visuals goes where AW2's does), the map menu is AW2's, and tangoAW2's
    tag RAM is never written."""
    m = ctx.map()
    g = ctx.boot_teams(m)
    e = g.e
    before = e.read(tag.STATE, 0x100)
    e.press("START", 6)
    e.wait(20)
    ctx.check(g.on_teams(), "START keeps the Teams screen")
    g.set_teams(["andy", "olaf"], {1})
    g.teams_to_rules()
    e.wait(30)
    cursor = g.teams_addr() + ram.RULES_CURSOR - ram.TEAMS
    for _ in range(7):
        e.press("RIGHT", 6)
        e.wait(14)
    ctx.check(e.u8(cursor) != 6, f"RIGHT on Visuals leaves it as in AW2 (cursor {e.u8(cursor)})")
    ctx.shot(g, "rules_pack_off")
    g.set_rules()
    g.start_battle()
    g.wait_for_input()
    ctx.eq(e.u32(tag.MENU_POOL), 0x0849AAC0, "the map menu is AW2's")
    ctx.eq(g.map_menu_names(), ["CO", "Intel", "Options", "Save", "End"], "AW2's map menu")
    ctx.eq(e.read(tag.STATE, 0x100), before, "the tag RAM untouched")


# --- The DS Campaign ---------------------------------------------------------------------

def aw2_co(ds):
    """tangoAW2's CO for a Dual Strike CO id (a clone, 0x80 | id, is its CO)."""
    return next(c for c, d in romlib.DS_CO_IDS.items() if d == ds & 0x7F)


def _mission_pairs(step, picks, label):
    def fn(ctx):
        data = dc.DsData()
        index = dc.ORDER[step]
        m = data.mission(index)
        e = Emu(save=paths.base_save(), ds=True)
        g = Game(e)
        ctx.games.append(g)
        d = dc.DsCampaign(g)
        d.start(step=step)
        got = d.choose_cos(picks, dc.CO_PREFS)
        d.wait_control()
        ctx.log(f"{m['name']}: picks {got}, record COs {[hex(c) for c in m['cos'][:m['armies']]]}, tags {[hex(c) for c in m['tags'][:m['armies']]]}")
        n = sum(1 for k in range(m["armies"]) if m["cos"][k] == 0x1C)
        for a in range(1, m["armies"] + 1):
            co, partner = m["cos"][a - 1], m["tags"][a - 1]
            t = tag.partner(e, a)
            if co == 0x1C and partner == 0x1C and a - 1 < min(n, 4 - n):
                ctx.eq(g.player(a)["co"], got[a - 1], f"{label}: army {a}'s CO, the player's pick")
                ctx.eq((t or {}).get("co"), got[n + a - 1], f"{label}: army {a}'s partner, the player's pick")
            elif co not in (0, 0x1C) and partner not in (0, 0x1C):
                want = (aw2_co(co), aw2_co(partner))
                ctx.eq((g.player(a)["co"], (t or {}).get("co")), want, f"{label}: army {a}'s pair as Dual Strike's record")
            else:
                ctx.eq(t, None, f"{label}: army {a} has no partner")
        ctx.shot(g, "map")
    fn.__name__ = f"tag_ds_campaign_{label}"
    test(modes=("ds",))(fn)


_mission_pairs(7, 1, "tag_battle")          # the computer's Jugger and Lash
_mission_pairs(9, 2, "black_boats_ahoy")    # the player's pair
_mission_pairs(12, 4, "frozen_fortress")    # two player pairs, the computer's Kindle and Jugger


# --- Two fronts: the second front's CO comes back as a partner --------------------------

@test(modes=("ds",))
def tag_two_front_partner(ctx):
    """Victory or Death! on two fronts (crate::two_front): when the second
    front is won, the player's second-front CO reports back to the main
    front as the army's tag partner (Dual Strike: "The CO will now report
    back to the main front."); lost, Black Hole's second-front CO joins
    Black Hole's army there ("Return to the main front for tag battle.")."""
    import importlib.util, os as _os
    spec = importlib.util.spec_from_file_location("test_two_fronts_h", _os.path.join(_os.path.dirname(__file__), "test_two_fronts.py"))
    t2 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(t2)
    from aw2test import twofront as tf
    e, g, d = t2.start(ctx, t2.VICTORY_OR_DEATH)
    ctx.eq((tag.partner(e, 1), tag.partner(e, 2)), (None, None), "no pairs while both fronts are fought")
    t2.first_round(ctx, e, d)
    cp = tf.checkpoint(e, ctx, "second_started")
    t2.second_front_ends(ctx, e, d, g, tf.SECOND_WON, "won", lambda: t2.structures_down(d, lambda x, y: True))
    t = tag.partner(e, 1)
    ctx.check(t is not None and t["co"] != g.player(1)["co"], f"won: the player's army has a partner ({t})")
    ctx.eq(tag.partner(e, 2), None, "won: Black Hole's army stays single")
    names = tf.map_menu_names(g)
    ctx.check("Change" in names, f"won: Change on the map menu ({names})")
    ctx.shot(g, "won_pair_panel")
    tf.back_to(e, g, cp)
    t2.second_front_ends(ctx, e, d, g, tf.SECOND_LOST, "lost", lambda: t2.kill_setup(d, 1, 2, keep_others=False))
    ctx.eq(tag.partner(e, 1), None, "lost: the player's army stays single")
    t = tag.partner(e, 2)
    ctx.check(t is not None, f"lost: Black Hole's army has its second-front CO as partner ({t})")


# --- Dual Strike's tag screens (crate::tag_extras) --------------------------------------

EXTRAS = 0x0203F300          # crate::tag_extras::STATE: +0 the band (1 Tag, 2 Change)
STRINGS = 0x08781000         # its texts: tag-in +0, victory +0x100, TAG page +0x300


def rom_string(e, at):
    """A text crate::tag_extras wrote (free ROM reads 0xFF before)."""
    b = e.read(at, 0x100)
    if not b or b[0] == 0xFF:
        return b""
    return b[:b.index(0)] if 0 in b else b


def ds_text_of(ds_word):
    from aw2test import dscampaign as dc
    t = dc.DsData().text(ds_word)
    return bytes(c for c in t if c in (0x0D, 0x0E) or 0x20 <= c < 0x7F)


def ds_record(ds, co, off):
    import struct
    d = romlib.DS_CO_IDS[romlib.co_id(co)]
    return struct.unpack("<I", ds.a9(0x0215360C + 0x220 * d + off, 4))[0]


@test(modes=("ds",))
def tag_power_screen(ctx):
    """Tag: after the first CO's quote, Dual Strike's tag screen as a band
    across the map (BG0 rows 6..13: both portraits, the pair's Tag Power
    name, POWER 110%) holds the power's script, then goes (its rows
    cleared) and AW2's Super Power screen follows."""
    units = [(1, "tank", 10, 4), (2, "tank", 20, 10)]
    g = tag_battle(ctx, ["max", "olaf"], ["andy", None], units=units)
    e = g.e
    fill(g, 1)
    g.open_map_menu()
    g.choose("Tag", g.MAP_MENU)
    seen = e.wait_until(lambda: e.u8(EXTRAS) == 1, 1200, step=4)
    ctx.require(seen, "the tag band shows")
    e.wait(20)
    cnt = e.u16(0x03002B6C)
    screen = 0x06000000 + ((cnt >> 8) & 0x1F) * 0x800
    row = [e.u16(screen + 2 * (32 * 9 + c)) & 0x3FF for c in range(30)]
    ctx.check(all(t >= 0x2B0 for t in row), f"row 9 of BG0 is the band's tiles ({row[:6]}...)")
    ctx.shot(g, "tag_band")
    ctx.require(e.wait_until(lambda: e.u8(EXTRAS) == 0, 600, step=4), "the band goes")
    row = [e.u16(screen + 2 * (32 * 9 + c)) for c in range(30)]
    ctx.eq(row, [0] * 30, "its rows cleared")
    ctx.require(e.wait_until(lambda: g.player(1)["co_mode"] == 2, 3000, step=10), "the Super Power follows")


@test(modes=("ds",))
def tag_change_line_and_band(ctx):
    """Change: the incoming CO says its Dual Strike tag-in line (its CO
    record +0x38 on day 1, as Dual Strike's capture), the CO SWAP band
    shows, then the COs swap and the turn ends."""
    g = tag_battle(ctx, ["andy", "olaf"], ["max", None], units=[(1, "tank", 10, 4), (2, "tank", 20, 10)])
    e = g.e
    ds = romlib.DualStrike()
    g.open_map_menu()
    g.choose("Change", g.MAP_MENU)
    ctx.require(e.wait_until(lambda: rom_string(e, STRINGS) != b"", 300, step=2), "the tag-in line")
    want = ds_text_of(ds_record(ds, "max", 0x38))
    ctx.eq(rom_string(e, STRINGS), want, "Max's tag-in line from the .nds")
    e.wait(40)
    ctx.shot(g, "change_quote")
    ctx.require(e.wait_until(lambda: e.u8(EXTRAS) == 2, 1200, step=4), "the CO SWAP band")
    ctx.shot(g, "change_band")
    ctx.require(e.wait_until(lambda: g.current_army() == 2, 1500, step=8), "the turn ends")
    ctx.eq((g.player(1)["co"], tag.partner(e, 1)["co"]), (romlib.co_id("max"), romlib.co_id("andy")), "the COs swapped")


@test(modes=("ds",))
def tag_victory_lines(ctx):
    """A special pair's army wins: the results screen's quote is the pair's
    exchange from the .nds (Andy and Max: Andy's line, then "Max: " and
    Max's)."""
    m = ctx.map(spare=False)
    m.unit(1, "tank", 10, 10).unit(2, "infantry", 11, 10)
    g = ctx.boot_teams(m)
    e = g.e
    tag.set_teams_partner(e, 1, "max")
    g.set_teams(["andy", "olaf"], {1})
    g.teams_to_rules()
    g.set_rules()
    g.start_battle()
    g.wait_for_input()
    inf = g.unit_at(11, 10)
    a = g.unit_addr(inf["id"])
    e.w16(a + 4, (e.u16(a + 4) & ~0x7F) | 1)
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Fire", g.ACTION_MENU)
    g.pick_target(11, 10)
    for _ in range(60):
        if rom_string(e, STRINGS + 0x100) != b"":
            break
        e.wait(40)
        e.press("A", 2)
    ctx.require(rom_string(e, STRINGS + 0x100) != b"", "the pair's quote")
    t = rom_string(e, STRINGS + 0x100)
    ctx.log(f"victory quote {t!r}")
    ctx.check(t.startswith(b"If it's a tag battle...") and b"\rMax: We're the best!" in t, f"Andy and Max's exchange ({t!r})")
    e.wait(60)
    ctx.shot(g, "victory_quote")


@test(modes=("ds",))
def tag_co_page(ctx):
    """The CO page (map menu, CO): DOWN from the Super Power's page shows
    the TAG page, the CO's special partners with their Dual Strike stars
    (Sami: Sonja 1, Eagle 3, Dual Strike's order); DOWN again goes on to the unit charts, UP
    back to the Super Power."""
    g = tag_battle(ctx, ["sami", "olaf"], [None, None], units=[(1, "tank", 10, 4)])
    e = g.e
    g.open_map_menu()
    g.choose("CO", g.MAP_MENU)
    e.wait(90)
    for _ in range(3):
        e.press("DOWN", 4)
        e.wait(40)
    ctx.eq(e.u32(0x03005940), 3, "the Super Power's page")
    e.press("DOWN", 4)
    e.wait(40)
    ctx.eq((e.u32(0x03005940), e.u8(EXTRAS + 5)), (3, 1), "DOWN: the TAG page")
    ctx.eq(rom_string(e, STRINGS + 0x300), b"Sonja\rEagle", "its partners, as Dual Strike's box lists them")
    stars = [s for s in oam(e) if (s[2] & 0x3FF) in (0x320, 0x321) and s[2] >> 12 == 12]
    full = [s for s in stars if s[2] & 0x3FF == 0x321]
    ctx.eq((len(stars), len(full)), (4, 4), "the ratings' full stars only: 1 + 3")
    ctx.shot(g, "co_page_tag")
    e.press("UP", 4)
    e.wait(40)
    ctx.eq((e.u32(0x03005940), e.u8(EXTRAS + 5)), (3, 0), "UP: back to the Super Power's page")
    e.press("DOWN", 4)
    e.wait(40)
    e.press("DOWN", 4)
    e.wait(40)
    ctx.eq((e.u32(0x03005940), e.u8(EXTRAS + 5)), (4, 0), "DOWN, DOWN: the unit charts")
