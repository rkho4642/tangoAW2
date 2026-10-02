"""Dual Strike's CO skills (crate::co_skills): each skill's effect in a Versus
battle, the game's numbers against the independent calculator (damage.py's
skill_attack / skill_defence, written from Dual Strike's code). The skills
each army has on are written to crate::co_skills::ACTIVE as a test aid (what
a mode's rules set when its battle starts); with none on, every battle plays
as it always did (the other suites)."""

import os

from aw2test import damage, paths
from aw2test import dscampaign as dc
from aw2test.emu import Emu
from aw2test.game import Game, NavError
from aw2test.harness import test
from aw2test import saves
from aw2test import survival as sv

ACTIVE = 0x0203F7E0  # crate::co_skills::ACTIVE: 6 bytes per army
FIRST = 0x20
FUNDS = 0


def set_skills(ctx, g, army, ids):
    b = bytearray(6)
    for i in ids:
        bit = i - FIRST
        b[bit // 8] |= 1 << (bit % 8)
    g.e.write(ACTIVE + 6 * (army - 1), bytes(b))
    ctx.skills[army] = set(ids)


def outcomes(ctx, g, src, dst, target):
    """The calculator's attack with the skills on, and without."""
    a = ctx.side(g, g.unit_at(*src), terrain=g.terrain_class(*dst))
    d = ctx.side(g, g.unit_at(*target))
    dist = abs(dst[0] - target[0]) + abs(dst[1] - target[1])
    with_ = damage.battle(ctx.rules, a, d, dist)[0]
    a.skills, d.skills = frozenset(), frozenset()
    plain = damage.battle(ctx.rules, a, d, dist)[0]
    return with_, plain


def attack_with(ctx, g, src, dst, target, label):
    """The attack, checked against the calculator with the skills (every
    number, ctx.attack); then the two numbers skills change, exactly: the
    attacker's damage before defence (the battle record's, the luck roll
    within the calculator's range) and the target's total defence, which
    must differ from what they would be without the skills."""
    sk, plain = outcomes(ctx, g, src, dst, target)
    ctx.attack(g, src, dst, target)
    ra, rd = ctx.battle_records(g)
    lo = damage.div(sk.acc * sk.base, 100)
    ctx.check(lo <= ra["damage"] < lo + sk.luck[0], f"{label}: the game's damage {ra['damage']} is acc {sk.acc} x base {sk.base} + luck")
    ctx.eq(rd["defence"], sk.defence, f"{label}: the target's defence")
    ctx.check((sk.acc, sk.defence) != (plain.acc, plain.defence), f"{label}: the skills change acc/defence ({plain.acc}/{plain.defence} -> {sk.acc}/{sk.defence})")


@test(modes=("ds",))
def skills_direct_attack(ctx):
    """Bruiser + Brawler: +13% for a direct unit."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "md tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x20, 0x21])
    attack_with(ctx, g, (10, 10), (10, 10), (11, 10), "Bruiser + Brawler")


@test(modes=("ds",))
def skills_indirect_attack(ctx):
    """Sharpshooter + Sniper: +13% for an indirect unit, and nothing for a
    direct one (its Bruiser-free tank fires as usual)."""
    m = ctx.map()
    m.unit(1, "artillery", 8, 10).unit(2, "md tank", 10, 10)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x22, 0x23])
    attack_with(ctx, g, (8, 10), (8, 10), (10, 10), "Sharpshooter + Sniper")


@test(modes=("ds",))
def skills_terrain_attack(ctx):
    """Road Rage on a road, Ranger in a wood (+10% each, only there)."""
    m = ctx.map()
    m.terrain(10, 10, "road").terrain(10, 12, "wood")
    m.unit(1, "tank", 10, 10).unit(2, "md tank", 11, 10)
    m.unit(1, "tank", 10, 12).unit(2, "md tank", 11, 12)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x2C, 0x2D])
    attack_with(ctx, g, (10, 10), (10, 10), (11, 10), "Road Rage on a road")
    attack_with(ctx, g, (10, 12), (10, 12), (11, 12), "Ranger in a wood")


@test(modes=("ds",))
def skills_defence(ctx):
    """Slam Guard + Slam Shield (+20 defence) against a direct attack."""
    m = ctx.map()
    m.unit(1, "md tank", 10, 10).unit(2, "md tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 2, [0x25, 0x26])
    attack_with(ctx, g, (10, 10), (10, 10), (11, 10), "Slam Guard + Slam Shield")


@test(modes=("ds",))
def skills_indirect_defence(ctx):
    """Snipe Guard + Snipe Shield (+20 defence) against an indirect attack."""
    m = ctx.map()
    m.unit(1, "rockets", 7, 10).unit(2, "tank", 10, 10)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 2, [0x27, 0x28])
    attack_with(ctx, g, (7, 10), (7, 10), (10, 10), "Snipe Guard + Snipe Shield")


@test(modes=("ds",))
def skills_weather_attack(ctx):
    """High and Dry: +20% in rain."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "md tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"], weather="rain")
    set_skills(ctx, g, 1, [0x32])
    attack_with(ctx, g, (10, 10), (10, 10), (11, 10), "High and Dry in rain")


@test(modes=("ds",))
def skills_off_change_nothing(ctx):
    """With no skill on the numbers are the calculator's without skills (the
    same battle as skills_direct_attack)."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "md tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    ctx.eq(g.e.read(ACTIVE, 30), bytes(30), "no skill on at the battle's start")
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


DATA = 0x0203E000      # crate::co_skills::DATA: "SKL1", then 32 bytes per CO
CO_LEN = 32
PLAYERS = 0x08499598
CO_SLOTS = list(range(19)) + list(range(72, 81))


def co_slot(co):
    return co if co <= 18 else 19 + co - 72


def co_exp(e, co):
    return e.u32(DATA + 4 + CO_LEN * co_slot(co))


def active(e, army):
    b = e.read(ACTIVE + 6 * (army - 1), 6)
    return sorted(FIRST + k for k in range(48) if b[k // 8] >> (k % 8) & 1)


def to_world_map(e, d):
    for _ in range(3000):
        if d.world_map_up() and e.u8(dc.WM_STATE + 0x10) and not d.scripts_running():
            break
        if d.scripts_running() or not d.in_battle():
            e.press("A", 4)
        e.wait(10)
    e.wait(60)


@test(modes=("ds",))
def skills_ds_campaign_exp_and_sets(ctx):
    """The DS Campaign: a won mission gives the player's CO EXP (the
    mission's score; x1 for Dual Strike's first eight missions), saved with
    the campaign's record and back after a reboot; in the next mission the
    player's CO has its Campaign set on (the skills open to it at its rank,
    as many as its slots: rank 2, two), the computer's armies none. (The
    mission is won by a test aid; the sets and EXP for rank 2 are written as
    test aids for the Set Skills panel.)"""
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    d = dc.DsCampaign(g)
    d.start(step=0)
    d.wait_map()
    co = e.u8(e.u32(PLAYERS) + 0x3C + 0x1D)
    ctx.eq(e.u32(DATA), 0x314C4B53, "the skill data in RAM")
    ctx.eq(co_exp(e, co), 0, "no EXP yet")
    ctx.eq(active(e, 1), [], "no skills on (no rank)")
    # Every CO: rank 2, a Campaign set of Bruiser (rank 1), Brawler (rank 8:
    # not open yet), Slam Guard (rank 1).
    for c in CO_SLOTS:
        a = DATA + 4 + CO_LEN * co_slot(c)
        e.w32(a, 2000)
        e.write(a + 4, bytes([0x20, 0x21, 0x25, 0]))
    ctx.require(d.force_win(), "Jake's Trial won (test aid)")
    to_world_map(e, d)
    score = e.u32(dc.RECORDS) >> 20
    ctx.check(score > 0, f"the mission's score {score}")
    ctx.eq(co_exp(e, co), 2000 + score, "EXP: + the score (x1 in the first eight missions)")
    save = e.save(os.path.join(ctx.out, "after"))
    e2 = Emu(save=save, ds=True)
    g2 = Game(e2, ctx.image)
    ctx.games.append(g2)
    d2 = dc.DsCampaign(g2)
    d2.start(new=False)
    d2.wait_map()
    ctx.eq(co_exp(e2, co), 2000 + score, "the EXP after a reboot")
    co2 = e2.u8(e2.u32(PLAYERS) + 0x3C + 0x1D)
    ctx.eq(active(e2, 1), [0x20, 0x25], f"the player's CO ({co2}): Bruiser and Slam Guard on (Brawler not open)")
    for a in (2, 3, 4):
        ctx.eq(active(e2, a), [], f"army {a}: none")


UNIT_TABLE = 0x08680000   # tangoAW2's unit table (crate::roster), 0x5C a unit
RANGE = 0x0201E450 + 0x2852
ROWS = 0x0201E450 + 0x417A


def reach(g, x, y):
    """The cells the unit at (x, y) can move to (its move range drawn)."""
    g.select(x, y)
    g.e.wait(8)
    cells = set()
    w, h = g.e.u16(0x0201E450), g.e.u16(0x0201E452)
    for yy in range(h):
        row = g.e.u16(ROWS + 2 * yy)
        r = g.e.read(RANGE + row, w)
        cells |= {(xx, yy) for xx in range(w) if r[xx] != 0xFF}
    g.e.press("B", 4)
    g.wait_for_input()
    return cells


@test(modes=("ds",))
def skills_apc_boost(ctx):
    """APC Boost: a transport moves one square more (an APC on plains:
    every cell within 7, not 6)."""
    m = ctx.map()
    m.unit(1, "apc", 10, 10).unit(2, "tank", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    before = reach(g, 10, 10)
    set_skills(ctx, g, 1, [0x24])
    after = reach(g, 10, 10)
    far = max(abs(x - 10) + abs(y - 10) for x, y in before)
    ctx.eq(far, 6, "an APC's reach without it")
    ctx.eq(max(abs(x - 10) + abs(y - 10) for x, y in after), 7, "with APC Boost: one more")


@test(modes=("ds",))
def skills_capture(ctx):
    """Invader + Conquerer: a capture takes 3 points more (a full-HP
    Infantry on a neutral city: 13 of 20, not 10)."""
    m = ctx.map()
    m.terrain(10, 10, "city")
    m.unit(1, "infantry", 10, 10).unit(2, "tank", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x3F, 0x40])
    u = g.unit_at(10, 10)
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Capt", g.ACTION_MENU)
    g.wait_for_input()
    got = g.e.u8(g.unit_addr(u["id"]) + 5) >> 3
    ctx.eq(got, 13, "the capture's points (10 + 1 + 2)")


@test(modes=("ds",))
def skills_build_cost(ctx):
    """Sale Price + Fire Sale: building costs 13% less (a Tank: 7000 -> 6090)."""
    m = ctx.map()
    m.terrain(10, 10, "base", 1)
    m.unit(2, "tank", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x43, 0x44])
    funds = lambda: g.e.u32(g.players_base + 0x3C)
    g.e.w32(g.players_base + 0x3C, 10000)  # (funds to build with: a test aid)
    before = funds()
    g.buy(10, 10, 5)
    ctx.eq(before - funds(), 7000 * 87 // 100, "a Tank's price less 13%")


@test(modes=("ds",))
def skills_gold_rush(ctx):
    """Gold Rush: 100 more a day per property that earns (two cities and
    the HQ: income 3300, not 3000)."""
    m = ctx.map()
    m.terrain(5, 5, "city", 1).terrain(6, 5, "city", 1)
    m.unit(1, "infantry", 2, 2).unit(2, "infantry", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x46])
    funds = lambda: g.e.u32(g.players_base + 0x3C)
    props = ctx.properties(g, 1)
    before = funds()
    g.end_turn()
    ctx.eq(funds() - before, 1100 * props, f"{props} properties: 1000 + 100 each")


@test(modes=("ds",))
def skills_repair(ctx):
    """Mechanic + Gear Head: a unit on its own city repairs 5 HP, not 2."""
    m = ctx.map()
    m.terrain(5, 5, "city", 1)
    m.unit(1, "tank", 5, 5).unit(2, "infantry", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x3D, 0x3E])
    u = g.unit_at(5, 5)
    ctx.set_hp(g, 5, 5, 40)
    g.end_turn()
    ctx.eq(g.unit(u["id"])["hp"], 90, "40 + 50 HP")


@test(modes=("ds",))
def skills_star_power(ctx):
    """Star Power: the power meter fills 10% faster (ctx.attack's meter check
    with the skill)."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x48])
    ctx.attack(g, (10, 10), (10, 10), (11, 10))


P_YIELD = 0x31
P_SCORE = 0x38
GAME_MODE = 0x03003FC1


def war_room_battle(e, g):
    """(on Select Mode) War Room -> its first map, the CO screen, the battle."""
    saves.wheel_to(e, saves.WAR_ROOM)
    e.press("A", 8)
    e.wait(60)
    saves.box_row(e, 1)
    e.press("A", 8)
    if not e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10):
        raise NavError("the War Room's SELECT MAP did not open")
    e.wait(90)
    e.press("A", 8)
    for _ in range(80):
        if e.u32(0x03000000) == 0x08022049:
            break
        if sv.running(e, sv.CO_SCREEN_PROC):
            e.wait(60)
            e.press("A", 8)
            e.wait(100)
            e.press("A", 8)
        e.wait(20)
    if not e.wait_until(lambda: e.u32(0x03000000) == 0x08022049, 1500, step=20):
        raise NavError("the battle did not start")
    g._units_base = g._players_base = None
    g.wait_for_input()


@test(modes=("ds",))
def skills_war_room_exp(ctx):
    """The War Room: the player's CO has its War Room set on (rank 3: three
    of its four open skills), the computer none; a won map gives the CO EXP:
    its score x2 with skills on (x2.5 without, as Dual Strike's War Room);
    saved with the DS Campaign's slot after the War Room's save. (The set,
    the EXP for the rank and the computer's yield are test aids.)"""
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    saves.to_select_mode(e)
    war_room_battle(e, g)
    ctx.log(f"game mode {e.u8(GAME_MODE)}")
    p1 = e.u32(PLAYERS) + 0x3C
    co = e.u8(p1 + 0x1D)
    ctx.eq(e.u32(DATA), 0x314C4B53, "the skill data in RAM")
    ctx.eq(active(e, 1), [], "rank 0: no skills")
    e.close()
    # Again, with rank 3 and a War Room set for every CO.
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    saves.to_select_mode(e)
    e.w32(DATA, 0x314C4B53)
    for c in CO_SLOTS:
        a = DATA + 4 + CO_LEN * co_slot(c)
        e.w32(a, 3000)
        e.write(a + 12, bytes([0x20, 0x25, 0x27, 0x47]))  # the War Room set
    war_room_battle(e, g)
    co = e.u8(e.u32(PLAYERS) + 0x3C + 0x1D)
    ctx.eq(active(e, 1), [0x20, 0x25, 0x27], "rank 3: Bruiser, Slam Guard, Snipe Guard on (three slots)")
    ctx.eq(active(e, 2), [], "the computer: none")
    for a in range(2, 5):
        if e.u8(e.u32(PLAYERS) + 0x3C * a + 0x1B):
            e.w8(e.u32(PLAYERS) + 0x3C * a + P_YIELD, 1)
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)
    if not sv.to_select_map(e):
        raise NavError("SELECT MAP did not come back")
    score = e.u16(e.u32(PLAYERS) + 0x3C + P_SCORE)
    ctx.check(score > 0, f"the map's score {score}")
    ctx.eq(co_exp(e, co), 3000 + score * 2, "EXP: + the score x2 (skills on)")
    img = saves.flash(e, os.path.join(ctx.out, "after"))
    rec = img.slot(15)
    ctx.require(rec is not None, "the DS Campaign's slot written")
    off = 0x20 + 8 * 32 + 4 + CO_LEN * co_slot(co)
    ctx.eq(int.from_bytes(rec[off:off + 4], "little"), 3000 + score * 2, "the EXP in Flash")


PANEL = 0x0203E3A0          # crate::skills_panel: open, slot, CO, set, ids[4], on Teams
VERSUS_RULE = DATA + 4 + CO_LEN * 28 + 1
CO_SELECT = 0x08616638
CO_SELECT_IDLE = 0x0807CE5D


def co_select_idle(e):
    for p in range(0x0200D610, 0x0200E418, 0x6C):
        if e.u32(p) == CO_SELECT and e.u32(p + 0x10) == CO_SELECT_IDLE:
            return True
    return False


def panel_sprites(e):
    """The panel's four sprites at the head of the game's sprites in OAM (its
    shadow list is copied to OAM 16..): 64x32, OBJ palette 14."""
    out = []
    for k in range(4):
        a0, a1, a2 = e.u16(0x07000080 + 8 * k), e.u16(0x07000082 + 8 * k), e.u16(0x07000084 + 8 * k)
        out.append((a0 & 0xC000, a1 & 0xC000, a2 >> 12))
    return out


@test(modes=("ds",))
def skills_panel_co_screen(ctx):
    """The War Room's CO screen: SELECT opens the Set Skills panel for the CO
    highlighted (rank 2: two slots); RIGHT puts the first open skill on the
    first slot, DOWN and RIGHT RIGHT the second open skill on the second;
    while it is up the game's CO screen takes no button; A keeps the set
    (the CO's War Room set) and closes it; B would close it as it was.
    (The EXP for rank 2 is a test aid.)"""
    e = Emu(save=paths.base_save(), ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    saves.to_select_mode(e)
    saves.wheel_to(e, saves.WAR_ROOM)
    e.press("A", 8)
    e.wait(60)
    saves.box_row(e, 1)
    e.press("A", 8)
    if not e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10):
        raise NavError("the War Room's SELECT MAP did not open")
    e.wait(90)
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: co_select_idle(e), 900, step=10), "the CO screen at its input stage")
    e.wait(30)
    e.w32(DATA, 0x314C4B53)
    for c in CO_SLOTS:
        e.w32(DATA + 4 + CO_LEN * co_slot(c), 2000)
    e.press("SELECT", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 1, "SELECT: the panel is up")
    co = e.u8(PANEL + 2)
    ctx.eq(e.u8(PANEL + 3), 2, "the War Room's set")
    e.wait(4)
    ctx.eq(panel_sprites(e), [(0x4000, 0xC000, 14)] * 4, "its four 64x32 sprites first in OAM (16..19)")
    shot(ctx, e, "panel_open")
    e.press("RIGHT", 4)
    e.wait(6)
    e.press("DOWN", 4)
    e.wait(6)
    e.press("RIGHT", 4)
    e.wait(6)
    ctx.eq(list(e.read(PANEL + 4, 4)), [0x20, 0x22, 0, 0], "Bruiser, then Sharpshooter (Bruiser is on slot 1)")
    shot(ctx, e, "panel_two")
    ctx.check(co_select_idle(e), "the CO screen did not move under the panel")
    e.press("A", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 0, "A: closed")
    ctx.check(co_select_idle(e), "the A that kept the set did not reach the CO screen")
    a = DATA + 4 + CO_LEN * co_slot(co)
    ctx.eq(list(e.read(a + 12, 4)), [0x20, 0x22, 0, 0], "the CO's War Room set")
    e.press("SELECT", 4)
    e.wait(10)
    e.press("LEFT", 4)
    e.wait(6)
    e.press("B", 4)
    e.wait(10)
    ctx.eq(list(e.read(a + 12, 4)), [0x20, 0x22, 0, 0], "B: closed as it was")


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


@test(modes=("ds",))
def skills_panel_teams_and_rule(ctx):
    """Versus' Teams screen: SELECT on an army's CO stop opens the panel for
    its CO (its Versus set), L in it turns the Versus rule Skills on; with the
    rule on, every army (the computer's too) has its CO's Versus set on in
    the battle; off (the default), none. (The EXP for rank 1 is a test aid.)"""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 20, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    e.wait(30)
    e.w32(DATA, 0x314C4B53)
    for c in CO_SLOTS:
        e.w32(DATA + 4 + CO_LEN * co_slot(c), 1000)
        e.write(DATA + 4 + CO_LEN * co_slot(c) + 16, bytes([0x25, 0, 0, 0]))  # Versus set 0: Slam Guard
    ctx.eq(e.u8(VERSUS_RULE), 0, "the rule: off by default")
    e.press("SELECT", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 1, "SELECT: the panel is up")
    ctx.eq((e.u8(PANEL + 3), e.u8(PANEL + 8)), (3, 1), "the Versus set, on the Teams screen")
    shot(ctx, e, "teams_panel")
    e.press("L", 4)
    e.wait(6)
    ctx.eq(e.u8(VERSUS_RULE), 1, "L: the rule on")
    edited = e.u8(PANEL + 2)
    e.press("RIGHT", 4)
    e.wait(6)
    e.press("A", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 0, "A: closed")
    a = DATA + 4 + CO_LEN * co_slot(edited)
    ctx.eq(list(e.read(a + 16, 4)), [0x27, 0, 0, 0], "its CO's Versus set: Snipe Guard (the next one open)")
    g.set_teams(["andy", "max"], {1})
    g.teams_to_rules()
    g.set_rules(fog=False, weather="clear", power=True, visuals="off", capt=None)
    g.start_battle()
    g.wait_for_input()
    want1 = [0x27] if e.u8(e.u32(PLAYERS) + 0x3C + 0x1D) == edited else [0x25]
    ctx.eq(active(e, 1), want1, "army 1: its CO's Versus set on")
    ctx.eq(active(e, 2), [0x25], "army 2 (the computer, Max): Max's Versus set, Slam Guard")


@test(modes=("ds",))
def skills_netplay_versus_rule(ctx):
    """Netplay: the Versus rule turned on in the Teams panel and the sets of
    the console's save (seat 0's) play the same on both peers: this run's
    inputs replayed on two rollback peers (aw2_netplay_script) give the same
    skills on and the same battle. (The EXP for rank 1 is a test aid, replayed
    on both peers.)"""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 11, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    e.wait(30)
    e.w32(DATA, 0x314C4B53)
    for c in CO_SLOTS:
        a = DATA + 4 + CO_LEN * co_slot(c)
        e.w32(a, 1000)
        e.w32(a + 16, 0x20)  # Versus set 0: Bruiser
    e.press("SELECT", 4)
    e.wait(10)
    e.press("L", 4)
    e.wait(6)
    e.press("A", 4)
    e.wait(10)
    g.set_teams(["andy", "andy"], {1, 2})
    g.teams_to_rules()
    g.set_rules(fog=False, weather="clear", power=True, visuals="off", capt=None)
    g.start_battle()
    g.wait_for_input()
    ctx.eq((active(e, 1), active(e, 2)), ([0x20], [0x20]), "both armies: Bruiser (the rule on)")
    ctx.skills = {1: {0x20}, 2: {0x20}}
    ctx.attack(g, (10, 10), (10, 10), (11, 10))
    units = (g.units_base, 12 * 140)
    identical, values, text = ctx.netplay_replay(g, [(ACTIVE, 30), units, (VERSUS_RULE, 1)])
    ctx.check(identical, f"both peers the same ({text.splitlines()[-3:] if text else ''})")
    ctx.eq(values.get(VERSUS_RULE), bytes([1]), "the rule on, on the peers")


def fuel(g, x, y):
    return g.unit_at(x, y)["fuel"]


@test(modes=("ds",))
def skills_hidden_fuel(ctx):
    """Sneaky + Stealthy: a hidden Stealth burns 3 fuel less a day (8 -> 5)."""
    m = ctx.map()
    m.unit(1, 12, 10, 10).unit(2, "tank", 20, 10)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    set_skills(ctx, g, 1, [0x41, 0x42])
    f0 = fuel(g, 10, 10)
    g.select(10, 10)
    g.move_to(10, 10)
    g.choose("Hide", g.ACTION_MENU)
    g.wait_idle()
    g.end_turn(human=2)
    g.end_turn(human=1)
    ctx.eq(f0 - fuel(g, 10, 10), 5, "8 less 1 and 2")


@test(modes=("ds",))
def skills_move_costs(ctx):
    """Prairie Dog: a Recon (tires) pays 1 on plains (its reach on an open
    plain: 8, not 4). Pathfinder: a Tank (treads) pays 1 in woods (6, not 3,
    in a wood)."""
    m = ctx.map()
    for x in range(3, 15):
        for y in range(13, 20):
            m.terrain(x, y, "wood")
    m.unit(1, "recon", 20, 6).unit(1, "tank", 8, 16).unit(2, "tank", 28, 1)
    g = ctx.start(m, ["andy", "andy"])
    far = lambda cells, x, y: max(abs(a - x) + abs(b - y) for a, b in cells)
    recon0, tank0 = far(reach(g, 20, 6), 20, 6), far(reach(g, 8, 16), 8, 16)
    set_skills(ctx, g, 1, [0x38, 0x39])
    recon1, tank1 = far(reach(g, 20, 6), 20, 6), far(reach(g, 8, 16), 8, 16)
    ctx.log(f"recon {recon0} -> {recon1}, tank in woods {tank0} -> {tank1}")
    ctx.check(recon1 > recon0, f"Prairie Dog: the Recon goes further on plains ({recon0} -> {recon1})")
    ctx.check(tank1 > tank0, f"Pathfinder: the Tank goes further in woods ({tank0} -> {tank1})")


SEEN = 0x0201E450 + 0x12


@test(modes=("ds",))
def skills_vision(ctx):
    """Scout: vision +1 in fog. An Infantry (vision 2) moves to 3 squares
    from a Tank: it does not see it, with Scout it does (the fog worked out
    as it moves)."""
    for scout in (False, True):
        m = ctx.map()
        m.unit(1, "infantry", 10, 10).unit(2, "tank", 14, 10)
        g = ctx.start(m, ["andy", "andy"], fog=True)
        if scout:
            set_skills(ctx, g, 1, [0x3B])
        g.select(10, 10)
        g.move_to(11, 10)
        g.choose("Wait", g.ACTION_MENU)
        g.wait_idle()
        seen = g.e.u8(SEEN + g.e.u16(ROWS + 20) + 14) != 0
        ctx.eq(seen, scout, f"{'with' if scout else 'without'} Scout: the Tank 3 away {'seen' if scout else 'not seen'}")


@test(modes=("ds",))
def skills_mistwalker(ctx):
    """Mistwalker: in the CO's Super Power its units strike first when
    attacked (Sonja's Counter Break): a Tank attacked by an Infantry on 1 HP
    destroys it before it fires."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "infantry", 11, 10)
    g = ctx.start(m, ["andy", "andy"], humans=(1, 2))
    set_skills(ctx, g, 1, [0x49])
    g.charge_power(1, "super")
    g.power("super")
    ctx.set_hp(g, 11, 10, 10)
    g.end_turn(human=2)
    hp = g.unit_at(10, 10)["hp"]
    g.attack((11, 10), (11, 10), (10, 10))
    ctx.check(g.unit_at(11, 10) is None, "the Infantry destroyed")
    ctx.eq(g.unit_at(10, 10)["hp"], hp, "the Tank struck first: untouched")


@test(modes=("ds",))
def skills_soul_of_hachi(ctx):
    """Soul of Hachi: in the CO's Super Power its cities build ground units."""
    m = ctx.map()
    m.terrain(5, 5, "city", 1)
    m.unit(2, "tank", 27, 17)
    g = ctx.start(m, ["andy", "andy"])
    set_skills(ctx, g, 1, [0x4A])
    g.e.w32(g.players_base + 0x3C, 10000)
    g.charge_power(1, "super")
    g.power("super")
    ids = g.buy(5, 5, 1)
    ctx.check(1 in ids, f"the city's build menu ({ids})")
    ctx.check(g.unit_at(5, 5) is not None, "an Infantry built at the city")


TEAMS_COLOUR = 0x02017C5D   # the Teams record's colours (+0x0D + army)


@test(modes=("ds",))
def skills_teams_buttons(ctx):
    """Versus' Teams screen: R and L change the highlighted army's colour
    (every colour reachable, Black Hole included); SELECT opens the Set
    Skills panel and leaves the colour as it was."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 20, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    e.wait(30)
    seen = {e.u8(TEAMS_COLOUR)}
    for _ in range(6):
        e.press("R", 4)
        e.wait(10)
        seen.add(e.u8(TEAMS_COLOUR))
    ctx.check(5 in seen and len(seen) >= 4, f"R goes through the colours, Black Hole included ({sorted(seen)})")
    before = e.u8(TEAMS_COLOUR)
    e.press("L", 4)
    e.wait(10)
    ctx.check(e.u8(TEAMS_COLOUR) != before, "L changes it the other way")
    c = e.u8(TEAMS_COLOUR)
    e.press("SELECT", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 1, "SELECT: the Set Skills panel")
    ctx.eq(e.u8(TEAMS_COLOUR), c, "SELECT leaves the colour")
    e.press("B", 4)
    e.wait(10)
    ctx.eq(e.u8(PANEL), 0, "B: closed")


@test(modes=("aw2",))
def skills_none_without_pack(ctx):
    """Without the Dual Strike pack there are no skills: on Versus' Teams
    screen SELECT changes the highlighted army's colour as in 0.4.0 (as R
    does), and no Set Skills panel opens there or on the CO screen."""
    m = ctx.map()
    m.unit(1, "tank", 10, 10).unit(2, "tank", 20, 10)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=False)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    e.wait(30)
    seen = [e.u8(TEAMS_COLOUR)]
    for _ in range(5):
        e.press("SELECT", 4)
        e.wait(10)
        seen.append(e.u8(TEAMS_COLOUR))
        ctx.eq(e.u8(PANEL), 0, "SELECT: no panel")
    ctx.check(all(a != b for a, b in zip(seen, seen[1:])) and 5 in seen,
              f"SELECT changes the colour each press, Black Hole included ({seen})")
    before = e.u8(TEAMS_COLOUR)
    e.press("R", 4)
    e.wait(10)
    ctx.check(e.u8(TEAMS_COLOUR) != before, "R as well")
