"""Dual Strike's Survival (survival.rs, mode_menu.rs): a seventh Select Mode
entry with the Dual Strike pack, its three runs on Dual Strike's own maps
(converted at run time and checked here against the .nds directly), the
budget carried from map to map, losing when it runs out, the results, the
records in the save, and the CPU on Survival maps. Without the pack none of
it is there."""

import os
import struct

from aw2test import paths, ram
from aw2test import rom as romlib
from aw2test import survival as sv
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import Skip, test

PLAYERS = 0x020232C0  # army 1's block (four armies)
P_FUNDS, P_HUMAN, P_YIELD = 0x00, 0x1B, 0x31
OAM = 0x07000000
GLYPH_SLASH = 0x3FD


def player(army):
    return PLAYERS + 0x3C * (army - 1)


def boot(ctx, save=None):
    e = Emu(save=save or paths.base_save(), ds=ctx.ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g


def shot(ctx, e, name):
    e.shot(os.path.join(ctx.out, name))


def end_turn(g):
    g.wait_for_input()
    g.open_map_menu()
    g.choose("End", g.MAP_MENU)


def win(ctx, e, g, armies):
    """Every computer army yields; ending the turn wins the map."""
    for a in range(2, armies + 1):
        e.w8(player(a) + P_YIELD, 1)
    end_turn(g)
    return sv.to_select_map(e)


def check_map(ctx, e, g, data, m, label):
    """The battle map against Dual Strike's map: size, every tile's terrain
    (the game's own class table, tangoAW2's tiles included), the units, fog,
    weather and look."""
    w, h = e.u16(ram.MAP_PTR), e.u16(ram.MAP_PTR + 2)
    ctx.eq((w, h), (m["w"], m["h"]), f"{label}: size")
    classes = e.read(ram.MAP_TERRAIN, 0x600)
    live = e.read(romlib.TILE_CLASS, 0x400)
    rows = [e.u16(ram.MAP_ROW_OFFSETS + 2 * y) for y in range(h)]
    bad = [(x, y) for y in range(h) for x in range(w)
           if classes[rows[y] + x] != live[m["tiles"][y * w + x]]]
    ctx.check(not bad, f"{label}: every tile's terrain as Dual Strike's ({len(bad)} differ: {bad[:6]})")
    none = [(x, y) for y in range(h) for x in range(w) if classes[rows[y] + x] == 0]
    ctx.check(not none, f"{label}: every tile has a terrain ({len(none)} without: {none[:6]})")
    have = sorted((u["army"], u["x"], u["y"], u["type"]) for u in g.units())
    ctx.eq(have, sorted(m["units"]), f"{label}: the pre-deployed units")
    ctx.eq(e.u8(ram.FOG) != 0, m["fog"] != 0, f"{label}: fog")
    if m["weather"] == 3:
        ctx.check(e.u8(ram.WEATHER) == 0 and e.u8(sv.WEATHER_MODE) == 3 and e.u8(sv.WEATHER_DEFAULT) == 0,
                  f"{label}: sandstorm")
    else:
        ctx.eq(e.u8(ram.WEATHER), m["weather"], f"{label}: weather")
    ctx.eq((e.u8(sv.BIOME) >> 4) & 7, 1 if m["look"] == 3 else 0, f"{label}: Wasteland look")
    # The War Room gives the player's army its CO's colour and moves a
    # computer army that had it to another.
    mine = e.u8(player(1) + 0x1A)
    for k in range(2, m["armies"] + 1):
        got = e.u8(player(k) + 0x1A)
        if m["colours"][k - 1] != mine:
            ctx.eq(got, m["colours"][k - 1], f"{label}: army {k}'s colour")
        else:
            ctx.check(got != mine, f"{label}: army {k}'s colour moved off the player's ({got})")


def hud_shown(e):
    oam = e.read(OAM, 0x400)
    return any(struct.unpack_from("<H", oam, 8 * i + 4)[0] & 0x3FF == GLYPH_SLASH
               and (oam[8 * i + 1] & 3) != 2 for i in range(128))


@test()
def survival_on_select_mode(ctx):
    """Survival is a seventh entry on the wheel with the pack, offline; without
    it the wheel turns its six as ever."""
    e, g = boot(ctx)
    sv.to_select_mode(e)
    seen = set()
    for _ in range(9):
        seen.add(e.u8(sv.SELECT_MODE_CURSOR))
        if e.u8(sv.SELECT_MODE_CURSOR) == sv.SURVIVAL_POSITION:
            shot(ctx, e, "select_mode_survival")
        e.press("UP", 8)
        e.wait(60)
    if not ctx.ds:
        ctx.eq(sorted(seen), list(range(6)), "six wheel positions")
        ctx.eq(e.u32(0x080814F0), 0x0861696C, "the wheel reads the game's own item table")
        return
    ctx.eq(sorted(seen), list(range(7)), "seven wheel positions")
    ctx.require(sv.wheel_to(e, sv.SURVIVAL_POSITION), "Survival reached")
    e.wait(40)
    shot(ctx, e, "select_mode")
    e.press("A", 8)
    ctx.require(e.wait_until(lambda: sv.running(e, sv.SELECT_MAP_PROC), 600, step=10), "SELECT MAP opens")
    ctx.eq(e.u32(ram.TITLE_MODE), 5, "the War Room's mode")
    ctx.eq(sv.state(e)["on"], 1, "Survival is on")
    ctx.eq(sv.listed(e), [sv.ENTRY_IDS[k] for k in sv.LIST_ORDER], "Money, Turn and Time Survival listed")
    e.wait(30)
    shot(ctx, e, "select_map")
    e.press("B", 8)
    ctx.require(e.wait_until(lambda: e.u8(sv.ON) == 0, 600, step=10), "B closes Survival")


def start_run(ctx, kind, co_steps=0):
    if not ctx.ds:
        raise Skip("the pack's mode")
    data = sv.Survival()
    e, g = boot(ctx)
    ctx.require(sv.open_survival(e), "Survival's SELECT MAP")
    ctx.require(sv.pick(e, sv.LIST_ORDER.index(kind), co_steps), f"{sv.NAMES[kind]} starts")
    return data, e, g


def _first_map(kind):
    def fn(ctx):
        data, e, g = start_run(ctx, kind)
        m = data.map(data.run(kind)[0])
        st = sv.state(e)
        ctx.eq((st["kind"], st["stage"], st["phase"]), (kind, 0, sv.PLAYING), "the run is on")
        ctx.eq(st["left"], data.budget(kind), "the whole budget")
        ctx.eq(e.u8(ram.VS_MAP), sv.ENTRY_IDS[kind], "the run's entry map")
        g.wait_for_input()
        check_map(ctx, e, g, data, m, m["name"])
        if kind == sv.MONEY:
            ctx.eq(e.u32(player(1) + P_FUNDS), 500000, "Money: the funds are the budget")
            ctx.eq(e.u32(ram.FUNDS_PER_PROPERTY), 0, "Money: no income")
        e.wait(2)
        ctx.check(hud_shown(e), "the budget is shown on the map")
        shot(ctx, e, "first_map")
    fn.__name__ = "survival_first_map_" + sv.NAMES[kind].split()[0].lower()
    test(modes=("ds",))(fn)


for _k in (sv.MONEY, sv.TURN, sv.TIME):
    _first_map(_k)


def _carry(kind):
    def fn(ctx):
        """Win map 1 having used some of the budget; map 2 is the only one
        listed and starts with what was left, and with map 1's CO (picked
        once a run: map 2's CO screen offers only that CO)."""
        data, e, g = start_run(ctx, kind, co_steps=2)
        m1 = data.map(data.run(kind)[0])
        g.wait_for_input()
        co = e.u8(player(1) + ram.P_CO)
        ctx.eq(sv.state(e)["co"], co, "the run's CO is the one picked")
        if kind == sv.MONEY:
            e.w32(player(1) + P_FUNDS, 432100)  # 67,900 G spent
            want = 432100
        elif kind == sv.TURN:
            end_turn(g)                          # day 2: the map costs 1 day
            g.wait_for_input()
            want = 99 - (e.u16(sv.DAY) - 1)
        else:
            e.wait(600)                          # ten more seconds of the player's turn
        time_used = e.u32(sv.FRAMES)
        if kind == sv.TIME:
            want = 90000 - (time_used + 0) // 60 * 60
        ctx.require(win(ctx, e, g, m1["armies"]), "back on SELECT MAP after the win")
        st = sv.state(e)
        if kind == sv.TIME:
            # the frames to the end of the turn count too: a few seconds more
            ctx.check(want - 600 <= st["left"] <= want, f"Time: {st['left']} frames left (about {want})")
        else:
            ctx.eq(st["left"], want, "what is left")
        ctx.eq((st["stage"], st["phase"]), (1, sv.BETWEEN), "map 2 next")
        ctx.eq(sv.listed(e), [data.map_id(kind, 1)], "only map 2 is listed")
        shot(ctx, e, "between")
        ctx.require(sv.pick(e, 0), "map 2 starts")
        g.wait_for_input()
        ctx.eq(e.u8(ram.VS_MAP), data.map_id(kind, 1), "map 2")
        st2 = sv.state(e)
        ctx.eq(st2["left"], st["left"], "carried over")
        ctx.eq(e.u8(player(1) + ram.P_CO), co, "map 2: the run's CO")
        if kind == sv.MONEY:
            ctx.eq(e.u32(player(1) + P_FUNDS), want, "Money: the funds left are map 2's")
        m2 = data.map(data.run(kind)[1])
        check_map(ctx, e, g, data, m2, m2["name"])
        shot(ctx, e, "map2")
    fn.__name__ = "survival_carry_" + sv.NAMES[kind].split()[0].lower()
    test(modes=("ds",))(fn)


for _k in (sv.MONEY, sv.TURN, sv.TIME):
    _carry(_k)


def _lose(kind):
    def fn(ctx):
        """Running out loses the run: the player's army yields, the results
        say GAME OVER, A closes them."""
        data, e, g = start_run(ctx, kind)
        g.wait_for_input()
        if kind == sv.MONEY:
            e.w32(player(1) + P_FUNDS, 0)
        elif kind == sv.TURN:
            e.w32(sv.LEFT, 0)
        else:
            e.w32(sv.LEFT, e.u32(sv.FRAMES) + 60)
        e.wait(80)
        ctx.eq(e.u8(player(1) + P_YIELD), 1, "out of budget: the army yields")
        end_turn(g)
        ctx.require(sv.to_select_map(e), "back on SELECT MAP")
        st = sv.state(e)
        ctx.eq(st["phase"], sv.LOST, "the run is lost")
        shot(ctx, e, "results_lost")
        e.press("A", 8)
        e.wait(30)
        ctx.eq(sv.state(e)["phase"], sv.CHOOSING, "A closes the results")
        ctx.eq(sv.listed(e), [sv.ENTRY_IDS[k] for k in sv.LIST_ORDER], "the three kinds again")
    fn.__name__ = "survival_lose_" + sv.NAMES[kind].split()[0].lower()
    test(modes=("ds",))(fn)


for _k in (sv.MONEY, sv.TURN, sv.TIME):
    _lose(_k)


@test(modes=("ds",))
def survival_clear_record_after_reboot(ctx):
    """The last map cleared: rank and bonus as Dual Strike works them out, the
    record in the profile, saved, and shown again after a reboot."""
    data, e, g = start_run(ctx, sv.MONEY)
    g.wait_for_input()
    e.w8(sv.STAGE, 10)            # this is the run's last map
    e.w32(player(1) + P_FUNDS, 123400)
    ctx.require(win(ctx, e, g, data.map(data.run(sv.MONEY)[0])["armies"]), "back on SELECT MAP")
    st = sv.state(e)
    ctx.eq((st["phase"], st["stage"], st["left"]), (sv.CLEARED, 11, 123400), "cleared with 123,400 G left")
    ctx.eq(st["rank"], 5, "rank S (50,000 G or more left)")
    ctx.eq(st["bonus"], 123400 // 200, "a point per 200 G left")
    rec = sv.records(e)
    ctx.eq(rec.get(sv.MONEY), (5, st["co"], 123400), "the record")
    shot(ctx, e, "results_cleared")
    e.press("A", 8)
    e.wait(30)
    save = e.save(os.path.join(ctx.out, "after"))
    e2, g2 = boot(ctx, save)
    ctx.require(sv.open_survival(e2), "Survival's SELECT MAP after a reboot")
    ctx.eq(sv.records(e2).get(sv.MONEY), (5, st["co"], 123400), "the record survived the reboot")
    e2.wait(30)
    shot(ctx, e2, "record_after_reboot")


def _cpu(kind, stage, days=4):
    def fn(ctx):
        """Every army a CPU on a Survival map: days pass, nothing hangs."""
        data, e, g = start_run(ctx, kind)
        m = data.map(data.run(kind)[stage])
        if stage > 0:
            # Map 1 won as map `stage`: the run goes on at map `stage` + 1.
            g.wait_for_input()
            e.w8(sv.STAGE, stage - 1)
            ctx.require(win(ctx, e, g, data.map(data.run(kind)[0])["armies"]), "back on SELECT MAP")
            ctx.require(sv.pick(e, 0), f"{m['name']} starts")
        ctx.eq(e.u8(ram.VS_MAP), data.map_id(kind, stage), m["name"])
        e.w8(player(1) + P_HUMAN, 2)
        before = {u["id"]: (u["x"], u["y"], u["hp"]) for u in g.units()}
        start = e.u16(sv.DAY)
        for _ in range(days * m["armies"] * 12):
            e.wait(300)
            if e.u16(sv.DAY) >= start + days or e.u32(sv.MAIN_CALLBACK) not in (sv.MAP_CALLBACK, 0):
                break
        shot(ctx, e, "cpu")
        day = e.u16(sv.DAY)
        ctx.check(day >= start + days or e.u32(sv.MAIN_CALLBACK) != sv.MAP_CALLBACK,
                  f"{m['name']}: {day - start} CPU days passed ({m['armies']} armies)")
        if e.u32(sv.MAIN_CALLBACK) != sv.MAP_CALLBACK:
            ctx.log(f"{m['name']}: the battle ended on day {day}")
            return
        moved = {u["army"] for u in g.units() if before.get(u["id"]) != (u["x"], u["y"], u["hp"])}
        have = {a for a, _, _, _ in m["units"]}
        ctx.check(have <= moved | {u["army"] for u in g.units() if u["id"] not in before},
                  f"{m['name']}: every army with units acted ({sorted(moved)})")
    fn.__name__ = "survival_cpu_" + str(kind) + "_" + str(stage)
    test(modes=("ds",))(fn)


# Convoy Cape, Crystal Field (Black Crystals), Chokepoint (three armies),
# The Gooping (Oozium), Triple Threat (the largest), Cape Splinter (volcanoes).
for _k, _s in ((sv.TURN, 0), (sv.TURN, 8), (sv.MONEY, 2), (sv.MONEY, 7), (sv.MONEY, 6), (sv.TIME, 2)):
    _cpu(_k, _s)


@test(modes=("aw2",))
def survival_hidden_without_pack(ctx):
    """Without the pack: the War Room lists its own maps, no Survival map id
    is in the table the game reads, nothing of Survival's is in RAM."""
    e, g = boot(ctx)
    sv.to_select_mode(e)
    ctx.eq(e.u32(0x080196EC), 0x08650000, "the map table is five_map's")
    ctx.eq(e.u8(0x08090EF2), 7, "the War Room lists its own tab")
    ctx.eq(e.read(sv.STATE, 0x24), bytes(0x24), "no Survival state")
