"""An independent re-implementation of Advance Wars 2's damage formula.

Written from the aw2bhr decompilation (src/battle.c: CalcDamage, sub_08024C58,
sub_08024DDC, sub_08024E60, sub_08024ED8; src/unit.c: sub_08043070,
GetCoAttackBonus/GetCoDefenceBonus; src/map.c: GetTerrainDefense), with every
table read from a ROM image or the Dual Strike .nds, never from the running game.

HP is the game's internal 0..100 scale (display HP = ceil(hp / 10)).

    base      = chart[att][def] (weapon chosen as the game does), scaled by the
                attacker CO's firepower: max(1, v * (100 + coAtk) / 100), 0 stays 0
    acc       = 100 + tempFirepower (+ counter bonus when countering)
                (+ terrain defence when the CO has ability 0x40)
    damage    = acc * base / 100 + rand % luck - rand % negLuck   (clamped at 0)
    damage    = damage * hpBars(attacker) / 10
    defence   = hpBars(defender) * terrainDefence / 10 + 100 + coDef + tempDefence
                (terrainDefence = stars * 10, 0 for air units, x2 with ability 0x20)
    hpLoss    = (200 - defence) * damage / 100
All divisions truncate toward zero.
"""

from dataclasses import dataclass, field

from . import rom as romlib


OOZIUM = 27


def div(a, b):
    """C integer division (truncates toward zero)."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def hp_bars(hp):
    return div(hp - 1, 10) + 1 if hp > 0 else 0


class Rules:
    """Where the numbers come from: a unit chart and CO/terrain tables.

    `image` supplies unit stats, CO data and terrain stars (AW2 ROM file by
    default); `chart`, when given, overrides the damage rows (e.g. Dual Strike's
    chart read from the .nds)."""

    def __init__(self, image=None, chart=None, name="aw2"):
        self.image = image or romlib.Image.load()
        self.chart = chart
        self.name = name

    @classmethod
    def aw2(cls):
        return cls(name="aw2")

    @classmethod
    def dual_strike(cls):
        ds = romlib.DualStrike()
        return cls(chart=ds, name="dual strike")

    def raw(self, att, dfd, weapon):
        src = self.chart or self.image
        return src.damage_row(att, weapon)[dfd]

    def unit(self, t):
        return self.image.unit(t)

    def co_bonus(self, co, mode, unit_type, which):
        if self.chart is not None and unit_type == OOZIUM:
            return 0  # no CO changes an Oozium (Dual Strike: its class, 6, gets 0)
        if self.chart is not None:
            v = self.chart.co_stat(co, mode, unit_type, which)
            if v is not None:
                return v
        return romlib.co_stat(self.image, co, mode, unit_type, which)

    def co_mode(self, co, mode):
        """AW2's CoModeData; with the pack, Dual Strike's luck and counter."""
        # A new CO (72..) has Andy's row but for what Dual Strike gives it.
        m = dict(self.image.co_mode(co if co < 19 else 1, mode))
        if co >= 19 and self.chart is not None:
            b = self.chart.co_block(co, mode)
            skills = [(0, 0x01, 0x01), (0, 0x02, 0x02), (0, 0x04, 0x04), (0, 0x08, 0x08), (0, 0x10, 0x20),
                      (0, 0x40, 0x80), (1, 0x80, 0x40)]
            m["abilities"] = sum(aw2 for byte, bit, aw2 in skills if b[0x08 + byte] & bit)
        if self.chart is not None and self.chart.co_block(co, mode) is not None:
            m["luck"] = self.chart.co_field(co, mode, 0x1C)
            m["neg_luck"] = self.chart.co_field(co, mode, 0x1E)
            m["counter"] = self.chart.co_field(co, mode, 0x20)
        return m

    def terrain_firepower(self, co, mode, terrain):
        return self.chart.terrain_firepower(co, mode, terrain) if self.chart is not None else 0

    def enemy_terrain_cut(self, co, mode):
        """Terrain stars the CO's enemies lose (Sonja, Dual Strike)."""
        v = self.chart.co_field(co, mode, 0x2A) if self.chart is not None else None
        return max(0, v or 0)

    def indirect_defence(self, co, mode):
        """Javier's defence against indirect attacks (tangoAW2's co_powers;
        Dual Strike's 0x020E61D8)."""
        return (20, 40, 80)[min(mode, 2)] if self.chart is not None and co == 77 else 0

    def tower(self, co, mode, which):
        """Com Tower % per tower: attack (0) or defence (1)."""
        v = self.chart.co_field(co, mode, 0x26 + 2 * which) if self.chart is not None else None
        return (10, 0)[which] if v is None else v

    def stars(self, terrain_class):
        return self.image.terrain_stars(terrain_class)

    def scaled(self, co, mode, att, dfd, weapon):
        """sub_08043070: the chart value scaled by the CO's firepower."""
        v = self.raw(att, dfd, weapon)
        if v <= 0:
            return 0
        r = div(v * (100 + self.co_bonus(co, mode, att, 0)), 100)
        return r if r else 1


@dataclass
class Side:
    """One unit in a battle, as the game sees it when the attack is confirmed."""
    type: int
    hp: int
    terrain: int            # terrain class & 0x1F at the unit's (new) position
    co: int
    co_mode: int = 0
    ammo: int = 9
    temp_firepower: int = 0
    temp_defence: int = 0
    dived: bool = False
    co_abilities: bool = True
    # Dual Strike's CO skills on for the unit's army (ids 0x20..0x4A), and the
    # weather (0 clear, 1 snow, 2 rain, 3 sandstorm) for the weather skills.
    skills: frozenset = frozenset()
    weather: int = 0


# Dual Strike's skill effects (its code: attack 0x020E646C / 0x020E68D0 /
# 0x020E6AE4, defence 0x020E61D8 / 0x020E63B0), by unit class: indirect
# (Artillery, Rockets, Missiles, Piperunner, Battleship, Carrier), no weapon
# (APC, T Copter, Lander, Black Boat), neither (Black Bomb, Oozium), direct
# (the rest, a dived Sub and a hidden Stealth too).
SKILL_INDIRECT = {10, 11, 15, 9, 21, 26}
SKILL_TRANSPORT = {7, 20, 23, 18}
SKILL_NEITHER = {13, 27}


def skill_attack(s):
    """Attack (percentage points) side `s`'s skills give it."""
    k = s.skills
    if not k:
        return 0
    v = 0
    direct = s.type not in SKILL_INDIRECT | SKILL_TRANSPORT | SKILL_NEITHER
    if direct:
        v += 5 * (0x20 in k) + 8 * (0x21 in k)
    if s.type in SKILL_INDIRECT:
        v += 5 * (0x22 in k) + 8 * (0x23 in k)
    # road, wood, city, mountain, sea (AW2's classes 5, 4, 6, 3, 7)
    for skill, terrain in ((0x2C, 5), (0x2D, 4), (0x2E, 6), (0x2F, 3), (0x30, 7)):
        if skill in k and s.terrain == terrain:
            v += 10
    if 0x31 in k and s.dived:
        v += 15
    for skill, weather in ((0x32, 2), (0x33, 1), (0x34, 3)):
        if skill in k and s.weather == weather:
            v += 20
    return v


def skill_defence(s, dist):
    """Defence side `s`'s skills give it against an attack from `dist`."""
    k = s.skills
    if not k:
        return 0
    v = 8 * (0x25 in k) + 12 * (0x26 in k) if dist <= 1 else 8 * (0x27 in k) + 12 * (0x28 in k)
    if 0x29 in k and s.type in SKILL_TRANSPORT:
        v += 10
    return v


@dataclass
class Outcome:
    weapon: str             # 'primary', 'secondary' or None
    base: int               # base damage after the CO's firepower (BattleUnit +0x10)
    acc: int
    luck: tuple             # (luckPositive, luckNegative)
    defence: int            # the target's total defence
    losses: set = field(default_factory=set)   # possible HP losses of the target

    def describe(self):
        lo, hi = (min(self.losses), max(self.losses)) if self.losses else (0, 0)
        return (f"{self.weapon or 'no weapon'} base {self.base} acc {self.acc} luck {self.luck} "
                f"target defence {self.defence} -> loss {lo}..{hi}")


def choose_weapon(rules, a: Side, b: Side, dist, is_attacker):
    """CalcDamage: (weapon, base) for `a` hitting `b` at distance `dist`."""
    t = rules.unit(a.type)
    v1 = v2 = v3 = 0
    col = 25 if b.dived else b.type
    if dist == 1:
        v2 = rules.raw(a.type, b.type, 1)
        if v2:
            v1 = rules.scaled(a.co, a.co_mode, a.type, b.type, 1)
        if t["min_range"] == 1 and a.ammo:
            v3 = rules.scaled(a.co, a.co_mode, a.type, col, 0)
    else:
        rng = t["max_range"] + (range_bonus(rules, a) if a.co_abilities else 0)
        if t["min_range"] <= dist <= rng and a.ammo and is_attacker:
            v3 = rules.scaled(a.co, a.co_mode, a.type, col, 0)
    if v3 <= v1:
        if v2:
            return "secondary", v1
        return None, 0
    return "primary", v3


def range_bonus(rules, a: Side):
    return rules.co_bonus(a.co, a.co_mode, a.type, 3)


def terrain_defence(rules, s: Side):
    u = rules.unit(s.type)
    if u["domain"] == 0x10:
        return 0
    d = rules.stars(s.terrain) * 10
    if s.co_abilities and rules.co_mode(s.co, s.co_mode)["abilities"] & 0x20:
        d *= 2
    return d


def total_defence(rules, s: Side, cut=0, dist=1):
    """`cut`: terrain stars the other side's CO takes away (Dual Strike's Sonja);
    `dist` > 1: an indirect attack (Javier's defence)."""
    coDef = rules.co_bonus(s.co, s.co_mode, s.type, 1) if s.co_abilities else 0
    if s.co_abilities and dist > 1:
        coDef += rules.indirect_defence(s.co, s.co_mode)
    coDef += skill_defence(s, dist)
    temp = s.temp_defence
    if rules.chart is not None and s.type == OOZIUM:
        coDef, temp = 0, 0  # nothing from its CO: no power's +10, towers or Javier's
    terrain = max(0, terrain_defence(rules, s) - 10 * cut)
    total = div(hp_bars(s.hp) * terrain, 10) + 100 + coDef + temp
    if rules.chart is not None:
        total = min(total, 200)  # Dual Strike's cap (0x020C34C8), with the pack
    return total


def strike(rules, a: Side, b: Side, dist, is_attacker, a_hp_now):
    """Possible HP losses of `b` when `a` (now at `a_hp_now` HP) strikes it."""
    weapon, base = choose_weapon(rules, a, b, dist, is_attacker)
    m = rules.co_mode(a.co, a.co_mode)
    acc = 100 + a.temp_firepower
    if not is_attacker and a.co_abilities:
        acc += m["counter"]
    if a.co_abilities and m["abilities"] & 0x40:
        acc += terrain_defence(rules, a)
    if a.co_abilities:
        acc += rules.terrain_firepower(a.co, a.co_mode, a.terrain)
    acc += skill_attack(a)
    luck = (m["luck"], m["neg_luck"]) if a.co_abilities else (10, 0)
    dmg0 = div(acc * base, 100)
    cut = rules.enemy_terrain_cut(a.co, a.co_mode) if a.co_abilities else 0
    defence = total_defence(rules, b, cut, dist)
    losses = set()
    if dmg0 == 0:
        rolls = [0]
    else:
        rolls = sorted({max(0, dmg0 + r - q) for r in range(luck[0]) for q in range(max(luck[1], 1))})
    for d in rolls:
        d = div(d * hp_bars(a_hp_now), 10)
        losses.add(div((200 - defence) * d, 100))
    return Outcome(weapon, base, acc, luck, defence, losses)


def battle(rules, att: Side, dfd: Side, dist, dfd_hp_after=None):
    """The attack and the counter.

    Returns (attack Outcome, counter Outcome or None). The counter depends on the
    defender's HP after the attack; give the observed value as `dfd_hp_after` to
    get the counter's range for exactly that case (luck makes it otherwise a
    union over every attack roll)."""
    first = strike(rules, att, dfd, dist, True, att.hp)
    counter = None
    if dist == 1:
        after = [dfd_hp_after] if dfd_hp_after is not None else [dfd.hp - l for l in first.losses]
        losses = set()
        out = None
        for h in after:
            if h <= 0:
                continue
            out = strike(rules, dfd, att, dist, False, h)
            losses |= out.losses
        if out is not None and out.weapon:
            out.losses = losses
            counter = out
    return first, counter
