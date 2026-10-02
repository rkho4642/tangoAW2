"""Test context: build a map, start a Versus battle on it, act, and check
what the game did against what the independent calculator expects."""

import os
import struct
import subprocess
import traceback

from . import damage, paths, ram
from . import rom as romlib
from .emu import Emu
from .game import Game, power_star_cost
from .save import DesignMap


class TestFailure(AssertionError):
    pass


class Skip(Exception):
    pass


class Ctx:
    def __init__(self, name, ds=False, out=None):
        self.name = name
        self.ds = ds
        self.mode = "ds" if ds else "aw2"
        self.out = out or paths.out_dir(self.mode, name)
        self.image = romlib.Image.load()
        self.rules = damage.Rules.dual_strike() if ds else damage.Rules.aw2()
        self.log_lines = []
        self.failures = []
        self.checks = 0
        self.games = []
        # The CO skills each army has on (crate::co_skills; tests set them),
        # and a sandstorm blowing, for the calculator.
        self.skills = {}
        self.sandstorm = False

    # -- reporting ---------------------------------------------------------------
    def log(self, msg):
        self.log_lines.append(str(msg))

    def check(self, cond, msg):
        self.checks += 1
        if cond:
            self.log(f"ok    {msg}")
        else:
            self.log(f"FAIL  {msg}")
            self.failures.append(msg)
        return cond

    def eq(self, got, want, msg):
        return self.check(got == want, f"{msg}: got {got}, want {want}")

    def require(self, cond, msg):
        if not self.check(cond, msg):
            raise TestFailure(msg)

    # -- setup ----------------------------------------------------------------------
    def map(self, name=None, hq=((1, 0, 0), (2, 29, 19)), spare=True):
        """A 30x20 plains map with an HQ per army (army, x, y) and, with `spare`,
        an Infantry next to each HQ so no battle under test ends the game."""
        m = DesignMap(name=(name or self.name)[:16], image=self.image)
        for army, x, y in hq:
            m.terrain(x, y, "hq", army)
            if spare:
                m.unit(army, "infantry", x + (1 if x < 15 else -1), y + (1 if y < 10 else -1))
        return m

    def boot_teams(self, m):
        """Boot with map `m` and stop on the Teams screen."""
        save = os.path.join(self.out, "map.sav")
        m.write(paths.base_save(), save)
        e = Emu(save=save, ds=self.ds)
        g = Game(e, self.image)
        self.games.append(g)
        g.boot_to_teams()
        return g

    def start(self, m, cos, humans=(1,), fog=False, weather="clear", power=True, visuals="off", capt=None, trace=None):
        """`trace`: a file of ROM addresses whose `trap` lines the runner prints (AW2_TRACE)."""
        save = os.path.join(self.out, "map.sav")
        m.write(paths.base_save(), save)
        e = Emu(save=save, ds=self.ds, trace=trace)
        g = Game(e, self.image)
        self.games.append(g)
        g.setup(cos, humans=humans, fog=fog, weather=weather, power=power, visuals=visuals, capt=capt)
        st = g.playst()
        self.log(f"battle started at frame {e.frame}: {st}")
        self.eq(st["map"], 0xB4, "Versus map is design map 1")
        # With the Dual Strike pack rain brings fog (ds_weather.rs).
        want_fog = fog or (self.ds and weather == "rain")
        self.eq(st["fog"], 1 if want_fog else 0, "fog")
        self.eq(st["anim"], {"off": 0, "a": 1, "b": 2, "c": 3}[visuals], "battle animations option")
        if weather != "random":
            self.eq(st["weather"], {"clear": 0, "snow": 1, "rain": 2, "sandstorm": 0}[weather], "weather")
        for army, co in enumerate(cos or [], 1):
            self.eq(g.player(army)["co"], romlib.co_id(co), f"army {army} CO")
        return g

    def shot(self, g, name):
        return g.e.shot(os.path.join(self.out, name))

    # -- expectations ------------------------------------------------------------------
    def side(self, g, u, terrain=None):
        p = g.player(u["army"])
        st = g.playst()
        # With the Dual Strike pack, a Versus Lab is a Com Tower: the CO's tower
        # firepower (10%; Javier more, and defence too) per tower the army owns,
        # added with tempFirepower / tempDefence.
        towers = self.towers(g, u["army"]) if self.ds else 0
        tower_fp = self.rules.tower(p["co"], p["co_mode"], 0) * towers
        if self.ds and p["co"] == 74 and p["co_mode"] == 2 and st["co_abilities"]:
            # Kindle's High Society: +3% per property she owns (co_powers).
            tower_fp += 3 * self.properties(g, u["army"])
        tower_def = self.rules.tower(p["co"], p["co_mode"], 1) * towers
        return damage.Side(
            type=u["type"], hp=u["hp"], ammo=u["ammo"],
            terrain=(g.terrain_class(u["x"], u["y"]) if terrain is None else terrain) & 0x1F,
            co=p["co"], co_mode=p["co_mode"],
            temp_firepower=p["temp_firepower"] + tower_fp, temp_defence=p["temp_defence"] + tower_def,
            dived=bool(u["flags"] & 0x20), co_abilities=bool(st["co_abilities"]),
            skills=frozenset(self.skills.get(u["army"], ())),
            weather=3 if self.sandstorm else st["weather"],
            tag_firepower=self.tag_firepower(g, u["army"], p["co"]),
        )

    def tag_firepower(self, g, army, co):
        """A Tag Power under way (crate::tag): the pair's compatibility - 100,
        read from the .nds (CO record +0x84)."""
        if not self.ds:
            return 0
        from . import tag
        t = tag.partner(g.e, army)
        if not t or t["phase"] == 0:
            return 0
        if not hasattr(self, "_ds"):
            self._ds = romlib.DualStrike()
        return tag.compatibility(self._ds, co, t["co"]) - 100

    def properties(self, g, army):
        """Properties (city, HQ, airport, port, base, Lab/tower) army owns."""
        kinds = (6, 8, 10, 11, 14, 20)
        return sum(1 for y in range(20) for x in range(30)
                   if g.terrain_class(x, y) >> 5 == army and g.terrain_class(x, y) & 0x1F in kinds)

    def towers(self, g, army):
        """Com Towers (Labs) army owns, counted on the map."""
        return sum(1 for y in range(20) for x in range(30) if g.terrain_class(x, y) == (army << 5) | 0x14)

    def battle_records(self, g):
        def rec(addr):
            b = g.e.read(addr, 0x1A)
            f = struct.unpack_from("<Ihhhhhhhhh", b, 0)
            return {"terrain": b[4], "terrain_def": f[2], "remaining": f[3], "damage": f[5],
                    "defence": f[6], "base": f[7], "hp_loss": f[8], "display": f[9],
                    "attack_type": struct.unpack_from("<h", b, 0x18)[0]}
        return rec(ram.BATTLE_ATTACKER), rec(ram.BATTLE_DEFENDER)

    def attack(self, g, src, dst, target, expect_base=None, expect_weapon=None):
        """Move src -> dst, fire at target, and check every number the game produced."""
        att0 = g.unit_at(*src)
        dfd0 = g.unit_at(*target)
        self.require(att0 is not None and dfd0 is not None, f"units at {src} and {target}")
        dist = abs(dst[0] - target[0]) + abs(dst[1] - target[1])
        a_side = self.side(g, att0, terrain=g.terrain_class(*dst))
        d_side = self.side(g, dfd0)
        pa0 = g.player(att0["army"])
        pd0 = g.player(dfd0["army"])
        from . import tag
        ta0, td0 = (tag.partner(g.e, att0["army"]), tag.partner(g.e, dfd0["army"])) if self.ds else (None, None)
        (_, att1), (_, dfd1) = g.attack(src, dst, target)
        ra, rd = self.battle_records(g)
        first, counter = damage.battle(self.rules, a_side, d_side, dist, dfd_hp_after=dfd1["hp"])
        an, dn = romlib.UNIT_NAMES[att0["type"]], romlib.UNIT_NAMES[dfd0["type"]]
        label = f"{an} ({romlib.co_name(a_side.co)}, mode {a_side.co_mode}) -> {dn} ({romlib.co_name(d_side.co)})"
        self.log(f"{label} at distance {dist}; attacker {att0['hp']}->{att1['hp']}, defender {dfd0['hp']}->{dfd1['hp']}")
        self.log(f"  expected attack: {first.describe()}")
        self.log(f"  game's records: attacker {ra}")
        self.log(f"                  defender {rd}")
        if expect_base is not None:
            self.eq(first.base, expect_base, f"{label}: calculator base damage (from the {self.rules.name} chart)")
        if expect_weapon is not None:
            self.eq(first.weapon, expect_weapon, f"{label}: weapon")
        self.eq(ra["base"], first.base, f"{label}: game's base damage")
        self.eq(ra["attack_type"], {"primary": 1, "secondary": 5}[first.weapon], f"{label}: game's weapon")
        self.eq(att1["x"], dst[0], f"{label}: attacker moved to x")
        self.eq(att1["y"], dst[1], f"{label}: attacker moved to y")
        loss = dfd0["hp"] - dfd1["hp"]
        possible = sorted({min(l, dfd0["hp"]) for l in first.losses})
        self.check(loss in possible, f"{label}: defender lost {loss} HP, calculator allows {possible[0]}..{possible[-1]} {possible if len(possible) < 12 else ''}")
        if first.weapon == "primary":
            self.eq(att1["ammo"], att0["ammo"] - 1, f"{label}: attacker ammo")
        else:
            self.eq(att1["ammo"], att0["ammo"], f"{label}: attacker ammo (secondary weapon)")
        if counter is not None and dfd1["hp"] > 0:
            self.log(f"  expected counter: {counter.describe()}")
            self.eq(rd["base"], counter.base, f"{label}: game's counter base damage")
            closs = att0["hp"] - att1["hp"]
            cp = sorted({min(l, att0["hp"]) for l in counter.losses})
            self.check(closs in cp, f"{label}: counter took {closs} HP, calculator allows {cp[0]}..{cp[-1]}")
        else:
            self.eq(att1["hp"], att0["hp"], f"{label}: no counter")
        # Power meter (sub_08041978): each side gains its own HP bars lost x its
        # unit's price (GetCoPriceMultiplier: cost x (100 + CO cost %) / 100)
        # plus half of the other side's; clamped to the SCOP cost, and not
        # charged while a power is on (sub_080440E0).
        pa1, pd1 = g.player(att0["army"]), g.player(dfd0["army"])

        def price(p, t):
            cost = self.image.u16(romlib.UNIT_TABLE + romlib.UNIT_RECORD * t + 6)
            pct = g.co_cost_percent(p["co"], p["co_mode"])
            return damage.div(cost * (100 + pct), 100)

        x1 = (damage.hp_bars(att0["hp"]) - damage.hp_bars(att1["hp"])) * price(pa0, att0["type"])
        x2 = (damage.hp_bars(dfd0["hp"]) - damage.hp_bars(dfd1["hp"])) * price(pd0, dfd0["type"])
        for p0, p1, gain, who, army in ((pa0, pa1, x1 + damage.div(x2, 2), "attacker", att0["army"]),
                                        (pd0, pd1, x2 + damage.div(x1, 2), "defender", dfd0["army"])):
            if p0["co_mode"] != 0 or p1["co_mode"] != 0:
                continue
            if 0x48 in self.skills.get(army, ()):
                gain = damage.div(gain * 110, 100)  # Star Power (crate::co_skills)
            cap = power_star_cost(p0["powers_used"]) * g.co_stars(p0["co"])[1]
            self.eq(p1["charge"], min(cap, p0["charge"] + gain), f"{label}: {who}'s power meter")
            # A tag pair's partner: half the active CO's (before Star Power),
            # its own Star Power, up to its Super Power's cost (crate::tag).
            t0 = ta0 if army == att0["army"] else td0
            if t0 is not None:
                t1 = tag.partner(g.e, army)
                half = damage.div(x1 + damage.div(x2, 2) if who == "attacker" else x2 + damage.div(x1, 2), 2)
                pcap = power_star_cost(t0["uses"]) * g.co_stars(t0["co"])[1]
                self.eq(t1["charge"], min(pcap, t0["charge"] + half), f"{label}: {who}'s partner's power meter (half)")
        return {"first": first, "counter": counter, "before": (att0, dfd0), "after": (att1, dfd1),
                "records": (ra, rd)}

    def power(self, g, army, which):
        """Fill the meter, fire the power from the map menu; return (before, after) units."""
        cost = g.charge_power(army, which)
        self.log(f"army {army}: power meter set to {cost} for {'Super CO Power' if which.startswith('s') else 'CO Power'}")
        names = g.map_menu_names()
        cop_stars = g.co_stars(g.player(army)["co"])[0]
        want_power = which.startswith("p") or cop_stars > 0
        self.check(("Power" in names) == want_power and ("Super" in names or which.startswith("p")),
                   f"map menu offers the powers: {names}")
        before = {u["id"]: u for u in g.units()}
        p0 = g.player(army)
        g.power(which)
        p1 = g.player(army)
        after = {u["id"]: u for u in g.units()}
        mode = 2 if which.startswith("s") else 1
        self.eq(p1["co_mode"], mode, f"army {army} power mode")
        self.eq(p1["charge"], 0, f"army {army} meter spent")
        self.eq(p1["powers_used"], p0["powers_used"] + 1, f"army {army} powers used")
        return before, after

    def expect_hp_change(self, before, after, army_delta, label, repair=False):
        """army_delta: {army: +/-internal HP} applied to every unit of that army,
        clamped to 1..100 (the game never kills with a power: sub_08044F24).
        repair=True: heals go through RepairUnit (Andy): +10 per HP step until
        the unit shows 10 HP, then rounded up to a whole display HP."""
        for uid, u0 in before.items():
            u1 = after.get(uid)
            if u1 is None or u0["army"] not in army_delta:
                continue
            d = army_delta[u0["army"]]
            if d > 0 and repair:
                hp = u0["hp"]
                for _ in range(d // 10):
                    if damage.hp_bars(hp) == 10:
                        break
                    hp = min(100, hp + 10)
                want = damage.hp_bars(hp) * 10
            elif d > 0:
                want = min(100, u0["hp"] + d)
            elif d < 0:
                want = max(1, u0["hp"] + d)
            else:
                want = u0["hp"]
            self.eq(u1["hp"], want, f"{label}: {romlib.UNIT_NAMES[u0['type']]} of army {u0['army']} at {(u0['x'], u0['y'])} HP")

    def set_hp(self, g, x, y, hp):
        """Test setup: write a unit's HP (7 bits of the halfword at +4)."""
        u = g.unit_at(x, y)
        a = g.unit_addr(u["id"]) + 4
        v = g.e.u16(a)
        g.e.w16(a, (v & ~0x7F) | hp)

    def script(self, g, name, tail=()):
        """The run so far as an aw2_script file script, in commands every build
        of aw2_script has (wait, press, hold, poke8, poke16)."""
        lines = []
        for l in list(g.e.timeline) + list(tail):
            p = l.split()
            if p[0] == "poke32":
                a, v = int(p[1], 16), int(p[2], 16)
                lines += [f"poke16 {a:08x} {v & 0xFFFF:x}", f"poke16 {a + 2:08x} {v >> 16:x}"]
            elif p[0] == "pokebytes":
                a = int(p[1], 16)
                lines += [f"poke8 {a + i:08x} {b:x}" for i, b in enumerate(bytes.fromhex(p[2]))]
            else:
                lines.append(l)
        path = os.path.join(self.out, name)
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
        return path

    def run_script(self, runner, script, save, ds=False):
        """Run a file script with an aw2_script binary; returns its output."""
        env = dict(os.environ)
        if ds:
            env["TANGOAW2_DS_ROM"] = paths.ds_rom()
        else:
            env.pop("TANGOAW2_DS_ROM", None)
        out = subprocess.run([runner, paths.aw2_rom(), script, "--save", save], capture_output=True, text=True,
                             env=env, timeout=1800, cwd=self.out)
        return out.stdout + out.stderr

    def netplay_replay(self, g, peeks):
        """Replay this run's inputs on two rollback peers (aw2_netplay_script, seat 0
        pressing) and return (all_identical, {addr: bytes on peer 0}, output)."""
        # Pokes (a filled power meter) are replayed on both peers on the same
        # tick, with idle frames after them (aw2_netplay_script).
        lines = ["seat 0"] + list(g.e.timeline) + ["wait 60"] + [f"peek {a:08x} {n}" for a, n in peeks]
        path = os.path.join(self.out, "netplay.txt")
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
        env = dict(os.environ)
        if self.ds:
            env["TANGOAW2_DS_ROM"] = paths.ds_rom()
            env["AW2_SHARED_ART"] = "1"
        else:
            env.pop("TANGOAW2_DS_ROM", None)
            env.pop("AW2_SHARED_ART", None)
        out = subprocess.run([paths.runner("aw2_netplay_script"), g.e.rom, os.path.join(self.out, "map.sav"), path,
                              os.path.join(self.out, "netplay")], capture_output=True, text=True, env=env, timeout=1800)
        text = out.stdout + out.stderr
        with open(os.path.join(self.out, "netplay.log"), "w") as f:
            f.write(text)
        values = {}
        differs = False
        for line in text.splitlines():
            if line.startswith("peek ") and " peer0: " in line:
                addr = int(line.split()[1], 16)
                values[addr] = bytes.fromhex(line.split(" peer0: ")[1].replace(" ", ""))
            if "(differs)" in line:
                differs = True
        identical = "all identical: true" in text and not differs
        return identical, values, text

    def finish(self):
        for g in self.games:
            try:
                self.shot(g, "final")
            except Exception:
                pass
            g.e.close()
        with open(os.path.join(self.out, "log.txt"), "w") as f:
            f.write("\n".join(self.log_lines) + "\n")


TESTS = []


def test(name=None, modes=("aw2", "ds"), netplay=False):
    def deco(fn):
        TESTS.append({"name": name or fn.__name__, "fn": fn, "modes": modes, "netplay": netplay})
        return fn
    return deco


def run_one(t, mode):
    ctx = Ctx(t["name"], ds=(mode == "ds"))
    ok = False
    err = None
    skipped = None
    try:
        t["fn"](ctx)
        ok = not ctx.failures
    except Skip as ex:
        ok = True
        skipped = str(ex)
        ctx.log("SKIP " + skipped)
    except Exception as ex:  # noqa: BLE001 - report everything
        err = "".join(traceback.format_exception(ex))
        ctx.log("ERROR " + err)
    finally:
        ctx.finish()
    return {"name": t["name"], "mode": mode, "ok": ok, "checks": ctx.checks, "skipped": skipped,
            "failures": ctx.failures, "error": err, "out": ctx.out}
