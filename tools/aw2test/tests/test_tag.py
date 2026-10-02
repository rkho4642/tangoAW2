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


def tag_battle(ctx, cos, partners, humans=(1,), units=(), fog=False, rule=True):
    """A Versus battle on the harness's plains with the CO Tag rule and the
    partners set in RAM (the Teams screen's picks are tag_teams_screen's)."""
    m = ctx.map()
    for u in units:
        m.unit(*u)
    g = ctx.boot_teams(m)
    tag.set_rule(g.e, rule)
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
    DOWN.., START). The rule is the Rules screen's (Game.set_extra_rules)."""
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
def tag_versus_rule_off_by_default(ctx):
    """Off at boot: partners picked on the Teams screen make no pair, the map
    menu is AW2's (no Change, no Tag); on a fresh boot the Rules screen's
    Skills and CO Tag rows read OFF."""
    g = tag_battle(ctx, ["andy", "olaf"], ["max", "sami"], rule=False)
    e = g.e
    ctx.eq(e.u8(tag.RULE), 0, "the CO Tag rule is off")
    ctx.eq(tag.partner(e, 1), None, "army 1 has no partner")
    ctx.eq(tag.partner(e, 2), None, "army 2 has no partner")
    ctx.eq(e.u32(tag.MENU_POOL), 0x0849AAC0, "the map menu is AW2's own table")
    names = g.map_menu_names()
    ctx.check("Change" not in names and "Tag" not in names, f"no Change or Tag on the map menu ({names})")
    # A fresh boot: the rule is off on the Teams screen too.
    m = ctx.map()
    g2 = ctx.boot_teams(m)
    ctx.eq(g2.e.u8(tag.RULE), 0, "off on a fresh Teams screen")
    ctx.eq(g2.e.u8(g2.SKILLS_RULE), 0, "Skills off on a fresh Teams screen")


@test(modes=("ds",))
def tag_rules_rows(ctx):
    """The Rules screen's two rows (crate::versus_rules): RIGHT from Visuals
    reaches Skills, then CO Tag, then wraps to Fog; LEFT from Fog comes back
    to CO Tag; UP turns a row ON and DOWN OFF; both start OFF; the game's
    own rules are unchanged by it."""
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
    e.press("RIGHT", 6)
    e.wait(20)
    ctx.eq(e.u8(g.VRULE_CURSOR), 2, "RIGHT: CO Tag")
    ctx.eq(e.u8(tag.RULE), 0, "CO Tag OFF by default")
    e.press("UP", 6)
    e.wait(20)
    ctx.eq(e.u8(tag.RULE), 1, "UP: CO Tag ON")
    ctx.shot(g, "rules_rows_on")
    e.press("DOWN", 6)
    e.wait(20)
    ctx.eq(e.u8(tag.RULE), 0, "DOWN: CO Tag OFF")
    e.press("UP", 6)
    e.wait(20)
    e.press("RIGHT", 6)
    e.wait(20)
    ctx.eq((e.u8(g.VRULE_CURSOR), e.u8(cursor)), (0, 0), "RIGHT on CO Tag: Fog")
    e.press("LEFT", 6)
    e.wait(20)
    ctx.eq(e.u8(g.VRULE_CURSOR), 2, "LEFT on Fog: CO Tag")
    e.press("LEFT", 6)
    e.wait(20)
    ctx.eq(e.u8(g.VRULE_CURSOR), 1, "LEFT: Skills")
    e.press("LEFT", 6)
    e.wait(20)
    ctx.eq((e.u8(g.VRULE_CURSOR), e.u8(cursor)), (0, 6), "LEFT: Visuals")
    ctx.eq(e.read(rules, 8), game_rules, "the game's own rules unchanged")
    ctx.eq((e.u8(g.SKILLS_RULE), e.u8(tag.RULE)), (1, 1), "both rules ON")


@test(modes=("ds",))
def tag_teams_screen(ctx):
    """The partners on the Teams screen (START, then UP/DOWN), the CO Tag
    row on the Rules screen, the battle starting with them; human and
    computer armies alike."""
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
    g.teams_to_rules()
    g.set_rules()
    g.set_extra_rules(tag=True)
    ctx.eq(e.u8(tag.RULE), 1, "the Rules screen's CO Tag row: ON")
    g.start_battle()
    g.wait_for_input()
    ctx.eq(g.player(1)["co"], main1, "army 1's CO")
    ctx.eq((tag.partner(e, 1) or {}).get("co"), others[2], "army 1's partner")
    ctx.eq((tag.partner(e, 2) or {}).get("co"), want2, "army 2's (the computer's) partner")
    # The CO panel: the partner's HUD face in its tiles.
    face = e.read(e.u32(0x080437EC) + 0x100 * others[2], 0x100)
    ctx.eq(e.read(0x06010000 + 32 * 0x309, 0x100), face, "the CO panel shows the partner's face (OBJ tiles 0x309..)")
    ctx.shot(g, "battle_panel")


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
    ctx.eq((e2.u8(tag.RULE), e2.u8(g2.SKILLS_RULE)), (e.u8(tag.RULE), e.u8(g.SKILLS_RULE)), "the Rules screen's rows kept")
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
    g.set_extra_rules(skills=True, tag=True)
    ctx.check(e.u8(tag.RULE) == 1 and e.u8(g.SKILLS_RULE) == 1 and e.u8(tag.TEAMS_PARTNER) != tag.NONE,
              "rules and partners picked with the pad")
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
    rules = (e.u8(g.SKILLS_RULE), e.u8(tag.RULE))
    identical, values, text = ctx.netplay_replay(g, [(tag.STATE, 0x40), (g.players_base + 0x3C, 0x3C * 2),
                                                     (g.SKILLS_RULE, 1), (tag.RULE, 1)])
    ctx.check(identical, "both peers identical")
    ctx.eq((values.get(g.SKILLS_RULE), values.get(tag.RULE)), (bytes([rules[0]]), bytes([rules[1]])),
           "both rules ON on the peers")
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
