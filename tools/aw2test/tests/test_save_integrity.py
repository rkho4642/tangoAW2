"""Save integrity: every mode's data is saved, survives a reboot (a fresh
console booted from the written save) and leaves every other mode's data
alone. Each step exports the cartridge's Flash before and after and checks,
with AW2's own rules (`aw2test.saveimg`: its sector check, its directory),
that only the slots and profile bytes the step is meant to write changed.

Slots: 0 the profile (AW2's unlocks and campaign flags, War Room and
campaign scores, options and suspend flags, tangoAW2's Survival records,
AW2's world map), 2 / 3 / 4 the Campaign / War Room / Versus suspend, 5..7
the design maps, 8 the design map a suspended Versus game is on, 14 a DS
Campaign mission saved halfway, 15 the DS Campaign's record (docs/AW2.md
"Saves"). The other files: test_save_integrity_design.py (the Design Room),
_campaign.py (AW2's campaign and the DS Campaign), _modes.py (the War Room,
Survival, every mode in one boot, every slot at once)."""

import os

from aw2test import saveimg, saves
from aw2test import paths
from aw2test.emu import Emu
from aw2test.game import Game
from aw2test.harness import test

VERSUS_SUSPEND, SUSPENDED_DESIGN = 4, 8


def boot(ctx, save, ds=None):
    e = Emu(save=save, ds=ctx.ds if ds is None else ds)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    return e, g


def out(ctx, name):
    return os.path.join(ctx.out, name)


def suspend_and_resume(ctx, g, label, design=True, act=None, after_resume=None, play_on=True):
    """Plays a little (`act`), saves from the map menu, checks the image,
    reboots and continues: the same battle, which then goes on (the turn
    ended, the computer's turns played, the player's turn back). Returns
    the image after and the resumed game."""
    e = g.e
    before = saves.flash(e, out(ctx, f"{label}_before"))
    if act:
        act(g)
    saves.suspend(g)
    snap = saves.snapshot(g)
    after = saves.flash(e, out(ctx, f"{label}_after"))
    want = [VERSUS_SUSPEND] + ([SUSPENDED_DESIGN] if design else [])
    saves.expect_slots(ctx, before, after, want, f"{label}: Save",
                       profile_allow=[saves.c420(saveimg.C420_SUSPEND[4])] + list(saves.OPTIONS) + [saves.MODE_BYTE])
    ctx.eq(after.slot(0)[saveimg.P_C420 + saveimg.C420_SUSPEND[4]], 1, f"{label}: the profile marks a Versus game saved")
    if design:
        played = saves.DESIGN_SLOT.get(snap["map"])
        rec, src = after.slot(SUSPENDED_DESIGN), after.slot(played) if played else None
        ctx.log(f"{label}: the suspended map's copy differs from its design at {saveimg.ranges(src or b'', rec or b'')[:12]}")
    e.close()
    e2, g2 = boot(ctx, after.path)
    saves.to_select_mode(e2)
    saves.versus_continue(g2)
    snap2 = saves.snapshot(g2)
    saves.compare_snapshots(ctx, snap, snap2, f"{label}: after a reboot, Continue")
    ctx.shot(g2, f"{label}_resumed")
    if after_resume:
        after_resume(g2)
    if play_on:
        g2.end_turn(human=snap["army"])
        ctx.check(not g2.battle_over(), f"{label}: the resumed battle goes on (a day played)")
    return after, g2


def tank_map(ctx, armies=2):
    hq = ((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19))[:armies]
    m = ctx.map(hq=hq)
    m.unit(1, "tank", 10, 10).unit(2, "tank", 12, 10)
    if armies > 2:
        m.unit(3, "recon", 20, 5)
    if armies > 3:
        m.unit(4, "artillery", 5, 15)
    return m


def move_tank(g):
    """Army 1's Tank moves next to army 2's and fires (some HP lost both
    ways, a power meter charged)."""
    g.attack((10, 10), (11, 10), (12, 10))


@test()
def save_versus_suspend_design_2p(ctx):
    """A 2P design map: Save on the map menu writes the Versus suspend and
    the map's copy (slots 4 and 8) and the profile's Versus flag, nothing
    else; after a reboot Continue brings back the same battle."""
    m = tank_map(ctx)
    g = ctx.start(m, ["andy", "drake"])
    suspend_and_resume(ctx, g, "2p", act=move_tank)


@test()
def save_versus_suspend_design_4p(ctx):
    m = tank_map(ctx, 4)
    g = ctx.start(m, ["andy", "drake", "olaf", "kanbei"])
    suspend_and_resume(ctx, g, "4p", act=move_tank)


# -- tangoAW2's Versus maps ---------------------------------------------------------------
# name -> (map id, Versus tab) (tests/test_ds_maps.py)
DS_MAPS = {"Rust Basin": (0xC1, 3), "Coral Strait": (0xC5, 3), "Black Wastes": (0xC4, 9),
           "Obelisk Duel": (0xB8, 3), "Obelisk Plains": (0xBA, 6), "Black Rampart": (0xC0, 9), "Five Seas": (0x00, 9)}
BIOME = 0x03004493
LAB = 0x14


def versus_on(ctx, name, weather="clear", humans=(1,)):
    mid, tab = DS_MAPS[name]
    e, g = boot(ctx, paths.base_save())
    saves.to_select_mode(e)
    saves.versus_select_map(g, tab, mid)
    saves.versus_start(g, humans=humans, weather=weather)
    ctx.eq(e.u8(0x03003FC2), mid, f"{name}: map id")
    return e, g


def give_tower(g, army):
    """The first neutral Com Tower on the map becomes `army`'s (its tile and
    class, as a capture leaves them): a changed tile the suspend keeps."""
    e = g.e
    w, h = e.u16(saves.GMAP), e.u16(saves.GMAP + 2)
    for y in range(h):
        row = e.u16(0x0201E450 + 0x417A + 2 * y)
        for x in range(w):
            if e.u8(0x0201E450 + 0x1432 + row + x) == LAB:
                e.w16(saves.GMAP_TILES + 2 * (row + x), 0x1D9 + army)
                e.w8(0x0201E450 + 0x1432 + row + x, LAB | army << 5)
                p = g.player(army)["addr"]
                e.w8(p + 0x11, e.u8(p + 0x11) + 1)   # its property count, as a capture adds
                return (x, y)
    return None


def _ds_map(name, weather, tower):
    def fn(ctx):
        e, g = versus_on(ctx, name, weather=weather)
        biome = (e.u8(BIOME) >> 4) & 7
        cell = give_tower(g, 1) if tower else None
        ctx.log(f"{name}: biome {biome}, tower given at {cell}")
        _, g2 = suspend_and_resume(ctx, g, name.replace(" ", "_").lower(), design=False)
        ctx.eq((g2.e.u8(BIOME) >> 4) & 7, biome, f"{name}: the look after the resumed day")
        if cell:
            ctx.eq(g2.terrain_class(*cell), LAB | 1 << 5, f"{name}: the captured tower is still army 1's")
    fn.__name__ = "save_versus_suspend_" + name.replace(" ", "_").lower()
    fn.__doc__ = (f"{name} (tangoAW2's map) with {weather} weather: suspended, rebooted and continued, its look, "
                  "weather and a captured tower kept.")
    test(modes=("ds",))(fn)


for _n, _w, _t in (("Rust Basin", "sandstorm", True), ("Coral Strait", "random", True), ("Obelisk Duel", "snow", False),
                   ("Obelisk Plains", "rain", False)):
    _ds_map(_n, _w, _t)


@test(modes=("ds",))
def save_versus_suspend_wasteland_design(ctx):
    """A Wasteland design map with Com Towers, Dual Strike's units and new
    COs, in a sandstorm: suspended (its map's copy keeps the look byte),
    rebooted, continued: the same battle, still Wasteland."""
    m = ctx.map()
    m.biome = 1
    m.terrain(2, 2, 0x1DA).terrain(4, 2, 0x1DB).terrain(6, 2, 0x1D9)
    m.terrain(15, 15, "sea").terrain(16, 15, "sea")
    m.unit(1, "tank", 10, 10).unit(2, "tank", 12, 10).unit(1, "oozium", 8, 8).unit(2, "megatank", 20, 8)
    m.unit(1, "carrier", 15, 15)
    g = ctx.start(m, ["jake", "kindle"], weather="sandstorm")
    ctx.eq((g.e.u8(BIOME) >> 4) & 7, 1, "the battle is Wasteland")
    after, g2 = suspend_and_resume(ctx, g, "wasteland", act=move_tank)
    ctx.eq(after.slot(SUSPENDED_DESIGN)[0x723], 0xB1, "the suspended map's copy keeps the Wasteland byte")
    ctx.eq((g2.e.u8(BIOME) >> 4) & 7, 1, "after the resumed day: still Wasteland")
    ctx.eq(g2.player(1)["co"], 79, "army 1's CO (Jake)")


def no_save_on_map_menu(ctx, g, label):
    """A five-army battle: Save is not on the map menu; a day played writes
    nothing to Flash."""
    e = g.e
    before = saves.flash(e, out(ctx, f"{label}_before"))
    names = g.map_menu_names()
    ctx.check("Save" not in names, f"{label}: no Save on the map menu ({names})")
    g.end_turn(human=1)
    ctx.check(not g.battle_over(), f"{label}: a day played")
    after = saves.flash(e, out(ctx, f"{label}_after"))
    ctx.check(after.data == before.data, f"{label}: nothing written to Flash ({saveimg.diff(before, after)})")


@test()
def save_hidden_five_army_design(ctx):
    """A five-army design map (Black Hole the fifth): Save hidden, nothing
    written in a day."""
    m = ctx.map(hq=((1, 0, 0), (2, 29, 19), (3, 29, 0), (4, 0, 19)))
    m.terrain(15, 10, 0x1B4)
    m.unit(5, "infantry", 16, 10)
    m.colours = [5, 1, 2, 3, 4]
    g = ctx.start(m, None)
    no_save_on_map_menu(ctx, g, "five-army design")


@test(modes=("ds",))
def save_hidden_five_army_maps(ctx):
    """tangoAW2's 5P maps (Five Seas, Black Rampart): Save hidden, nothing
    written to Flash in a day."""
    for name in ("Five Seas", "Black Rampart"):
        e, g = versus_on(ctx, name)
        no_save_on_map_menu(ctx, g, name)
        e.close()


# -- AW2's flags and unlocks ----------------------------------------------------------------
FLAGS = 0x02028030     # AW2's campaign flags 0x20.. (a bit each), then 0x60..


@test()
def save_keeps_aw2_completion_flags(ctx):
    """AW2's campaign flags beside Hard Campaign's (0x20) and the Sound
    Room's (0x28), which tangoAW2 keeps set: the campaign won (0x21) and the
    flags AW2's missions set (0x23..0x26), and those of 0x60.. (0x61..0x64,
    Hard Campaign's): set as the game sets them, they are kept in RAM, saved
    with the next save (a design saved here) and back after a reboot."""
    e, g = boot(ctx, paths.base_save())
    saves.to_select_mode(e)
    want = {0x21, 0x23, 0x26, 0x2B, 0x61, 0x64}
    for f in want:
        k = f - 0x20 if f < 0x60 else f - 0x60 + 0x40
        e.w8(FLAGS + k // 8, e.u8(FLAGS + k // 8) | 1 << (k % 8))
    e.wait(30)

    def flags_set(read):
        got = set()
        for f in list(range(0x20, 0x30)) + list(range(0x60, 0x68)):
            k = f - 0x20 if f < 0x60 else f - 0x60 + 0x40
            if read(k // 8) >> (k % 8) & 1:
                got.add(f)
        return got

    ram = flags_set(lambda i: e.u8(FLAGS + i))
    ctx.eq(sorted(ram & want), sorted(want), "the flags kept in RAM a moment later")
    ctx.check({0x20, 0x28} <= ram, "Hard Campaign and the Sound Room still unlocked")
    from aw2test.editor import Editor
    saves.wheel_to(e, saves.DESIGN_ROOM)
    e.press("A", 8)
    e.wait(150)
    e.press("A", 8)
    e.wait(150)
    for _ in range(30):
        if e.u8(0x0200B004) == 1:
            break
        e.press("A", 8)
        e.wait(40)
    Editor(e).save(2)
    img = saves.flash(e, out(ctx, "saved"))
    p = img.slot(0)
    saved = flags_set(lambda i: p[i])
    ctx.eq(sorted(saved & want), sorted(want), "the flags in the saved profile")
    e.close()
    e, g = boot(ctx, img.path)
    saves.to_select_mode(e)
    ctx.eq(sorted(flags_set(lambda i: e.u8(FLAGS + i)) & want), sorted(want), "after a reboot: the flags")


# -- netplay ------------------------------------------------------------------------------------
@test()
def save_netplay_suspend_identical_on_both_peers(ctx):
    """A Versus game saved from the map menu over netplay (the run replayed on
    two rollback peers, seat 0 pressing): both peers' console Flash ends the
    same, byte for byte, and the same as the offline run's; nothing is
    written to the save file the peers booted from. (A netplay session never
    writes its console's save back to either player's file: only
    single-player sessions do, tango/src/session/launch.rs.)"""
    m = tank_map(ctx)
    g = ctx.start(m, ["andy", "drake"])
    move_tank(g)
    saves.suspend(g)
    g.e.wait(60)
    offline = saves.flash(g.e, out(ctx, "offline"))
    ctx.check(VERSUS_SUSPEND in offline.tags(), "offline: the Versus suspend written")
    booted = open(out(ctx, "map.sav"), "rb").read()
    identical, values, text = ctx.netplay_replay(g, [(0x0E000000 + 0x1000 * s, 0x1000) for s in range(16)])
    ctx.check(identical, "netplay: all identical: true (both peers and the straight replay; Flash included)")
    flash = b"".join(values.get(0x0E000000 + 0x1000 * s, b"") for s in range(16))
    ctx.check(flash == offline.data, f"netplay peer 0's Flash equals the offline run's ({len(flash)} bytes)")
    ctx.check(open(out(ctx, "map.sav"), "rb").read() == booted, "the save file the peers booted from is unchanged")


# -- Dual Strike's COs: state that lasts past a turn --------------------------------------------
STUN_PENDING = 0x0203FE00      # crate::co_powers: Ex Machina's marked units


@test(modes=("ds",))
def save_versus_suspend_keeps_ex_machina_stun(ctx):
    """Von Bolt's Ex Machina marks the units it hits to be held through
    their next turn (tangoAW2's own state, crate::co_powers). Saved from the
    map menu right after it and continued after a reboot: the marked units
    are still held on their army's turn, and free the turn after."""
    m = ctx.map()
    m.unit(1, "tank", 2, 2)
    m.unit(2, "tank", 20, 10).unit(2, "mech", 21, 10).unit(2, "infantry", 20, 11)
    g = ctx.start(m, ["vonbolt", "andy"], humans=(1, 2))
    before, after = ctx.power(g, 1, "super")
    hit = sorted(i for i, u in before.items() if u["army"] == 2 and after[i]["hp"] < u["hp"])
    ctx.check(len(hit) >= 2, f"the strike hit enemy units: {hit}")
    marked = g.e.read(STUN_PENDING, 0x28)
    _, g2 = suspend_and_resume(ctx, g, "stun", play_on=False)
    ctx.check(g2.e.read(STUN_PENDING, 0x28) == marked, "after a reboot, Continue: the units still marked")
    g2.end_turn(human=2)
    for u in g2.units(2):
        if u["id"] in hit:
            ctx.check(u["flags"] & 1, f"unit {u['id']} is held on army 2's turn")
    g2.end_turn(human=1)
    g2.end_turn(human=2)
    for u in g2.units(2):
        if u["id"] in hit:
            ctx.check(not u["flags"] & 1, f"unit {u['id']} is free on the turn after")


FOG = 0x03003FCD
WEATHER_NOW = 0x03003FEC
NEXT_WEATHER = 0x03003FEE


@test(modes=("ds",))
def save_versus_suspend_in_rain_keeps_fog_rule(ctx):
    """With the pack rain brings fog of war, and takes it away when it stops
    (crate::ds_weather keeps the Rules' own fog choice meanwhile). A no-fog
    game with rain coming (random weather's decision at a turn end, set
    here) is saved from the map menu, rebooted and continued: once the rain
    is over, there is no fog again, as without the save."""
    m = ctx.map()
    m.unit(1, "tank", 2, 2).unit(2, "tank", 27, 17)
    g = ctx.start(m, ["andy", "andy"], fog=False)
    g.e.w8(NEXT_WEATHER, 2)
    g.e.wait(4)
    ctx.eq(g.e.u8(FOG), 1, "rain coming: fog on")
    _, g2 = suspend_and_resume(ctx, g, "rain", play_on=False)
    e = g2.e
    seen = []
    for day in range(3):
        g2.end_turn(human=1)
        seen.append((e.u8(WEATHER_NOW), e.u8(NEXT_WEATHER), e.u8(FOG)))
    ctx.log(f"(weather, next, fog) at army 1's next turns: {seen}")
    ctx.eq(seen[-1], (0, 0, 0), "the rain over: clear, and no fog (the Rules' choice)")


RULE_FOG = 0x0203FFA7  # ds_weather::RULE_FOG


@test(modes=("ds",))
def save_rain_fog_not_carried_to_the_next_battle(ctx):
    """The rain's fog rule (crate::ds_weather) is forgotten out of a
    battle: a battle left with rain coming and no fog of its own (gPlaySt
    keeps "next: rain" through the menus; set here as a test aid in the
    menus) no longer turns off the fog of the next battle set up in the
    same boot."""
    m = ctx.map()
    m.unit(1, "tank", 2, 2).unit(2, "tank", 27, 17)
    save = os.path.join(ctx.out, "map.sav")
    m.write(paths.base_save(), save)
    e = Emu(save=save, ds=True)
    g = Game(e, ctx.image)
    ctx.games.append(g)
    g.boot_to_teams()
    e.w8(FOG, 0)
    e.w8(NEXT_WEATHER, 2)
    e.wait(4)
    ctx.eq(e.u8(RULE_FOG), 0, "no fog rule kept in the menus")
    g.set_teams(["andy", "andy"], {1})
    g.teams_to_rules()
    g.set_rules(fog=True, weather="clear", power=True, visuals="off", capt=None)
    g.start_battle()
    g.wait_for_input()
    e.wait(30)
    ctx.eq(e.u8(FOG), 1, "the next battle's own fog")
