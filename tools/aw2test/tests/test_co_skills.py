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
from aw2test.game import Game
from aw2test.harness import test

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
