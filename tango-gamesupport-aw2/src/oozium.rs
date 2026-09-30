//! The Oozium eats, with the Dual Strike pack: it has no weapon (no Fire,
//! no counter-attack); its move takes in the squares next to it that hold
//! a unit of another team (any unit, air and naval included, on a square
//! the Oozium can enter), and moving onto one destroys that unit with the
//! game's own destruction (its explosion on the map, no battle scene).
//!
//! - The move range: AW2's flood fill (`sub_0801F4B4`, one step each in
//!   `sub_0801F6F0`) stops at a square holding a unit of an army the mover's
//!   blocks. For a human army's Oozium ([`flood_start`]) such a square is
//!   let in ([`block_test`]) and nothing is reached through it; the move
//!   may end there ([`may_end`]).
//! - The command menu there shows Wait (the game hides it on an occupied
//!   square: [`wait_hide`]); Wait ends the move as usual, then the eat
//!   ([`wait_selected`]).
//! - The eat ([`eat_next`]): both power meters charge as for a battle in
//!   which the victim lost all its HP (`sub_080440E0`, the value from
//!   `GetCoPriceMultiplier`), the eater's army counts a unit destroyed
//!   (`sub_08026588`'s counters), then the game's destruction
//!   (`sub_0804018C`: camera, explosion, removal, units lost, and the
//!   unit planes rebuilt from the records, which leaves the Oozium alone
//!   on its square). Rout follows as after any destruction.
//! - A hidden unit (fog, a hidden Stealth) on the square the Oozium moves
//!   onto is eaten too: the move's walk (`sub_0802E7C8`, which stops at a unit) lets
//!   an Oozium onto its last square ([`trap_check`]).
//! - The CPU ([`cpu_eats`], when its turn starts, from
//!   [`crate::cpu_tactics`]): each of its Ooziums next to units it can eat
//!   eats the most valuable one (bars x price); the move and the eat are
//!   played from the frame's effects pass ([`effects_pass`]), before the
//!   CPU's other units act. One with nothing to eat moves a square towards
//!   the nearest enemy ([`advance`]: AW2's CPU, finding no weapon, would
//!   leave it standing).
//! - Immunity ([`immune`]), as in Dual Strike (its CO stat functions return
//!   0 for the Oozium's class, 6, and its CO blocks' unit filters leave it
//!   out): no CO stats ([`crate::co_roster`], not even a power's +10
//!   defence), no power's damage, stun or fuel loss, repair, move again or
//!   resupply, no Missile Silo or Black Bomb damage ([`SKIPS`],
//!   [`LEAVE_OUT`], [`crate::co_powers`]); the CPU's silo and strike
//!   scoring leave it out.
//!
//! Everything is decided from emulated RAM, the same on both netplay peers
//! and in replays; without the pack no trap does anything.

use mgba::core::Core;

use crate::ds_weather::is_on;
use crate::roster::OOZIUM;
use crate::unit_actions::DESTROY;

const SELECTED: u32 = 0x0300_40D8;
const CURRENT_ARMY: u32 = 0x0300_33EC;
/// Where the moving unit ends up (x, y u16).
const DESTINATION: u32 = 0x0300_3100;
const MAP: u32 = 0x0201_E450;
const ROWS: u32 = MAP + 0x417A;
/// Units the player sees, and every unit (both hold unit ids).
const SEEN_PLANE: u32 = MAP + 0x12;
const ALL_PLANE: u32 = MAP + 0x51A;
const TERRAIN_PLANE: u32 = MAP + 0x1432;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
const MOVED: u8 = 0x01;
const CARRIED: u8 = 0x08;

/// tangoAW2's RAM for this (0x34 bytes, see docs/AW2.md).
const RAM: u32 = 0x0203_FDC8;
/// The unit being eaten (its record), 0 when none.
const VICTIM: u32 = RAM;
/// Where the eat returns to when done (a Thumb address, bit 0 set).
const RETURN: u32 = RAM + 4;
/// The eaten unit's value (bars x price) for the power meters.
const VALUE: u32 = RAM + 8;
/// The eating Oozium's army, and the eat's next step.
const KILLER: u32 = RAM + 0x0C;
const STEP: u32 = RAM + 0x0D;
/// The flood fill running now is a human Oozium's: 1 + its square.
const FLOOD: u32 = RAM + 0x30;
/// A CPU Oozium's eat waiting for the effects pass: its record (u32 at
/// [`CPU_EATER`]) and its victim's ([`CPU_VICTIM`]).
const CPU_EATER: u32 = RAM + 0x10;
const CPU_VICTIM: u32 = RAM + 0x14;
const CPU_SLOTS: u32 = 4;
const CPU_SLOT: u32 = 8;
/// End of this module's RAM.
#[cfg(test)]
const RAM_END: u32 = FLOOD + 3;

fn units(core: &Core) -> u32 {
    core.raw_read_32(UNITS_POINTER, -1)
}

fn players(core: &Core) -> u32 {
    crate::five::players(core)
}

fn army_of(core: &Core, u: u32) -> u32 {
    crate::five::army_of_index(core, (u - units(core)) / UNIT)
}

fn same_team(core: &Core, a: u32, b: u32) -> bool {
    let p = players(core);
    a == b || core.raw_read_8(p + 0x3C * a + 0x2A, -1) == core.raw_read_8(p + 0x3C * b + 0x2A, -1)
}

fn is_cpu(core: &Core, army: u32) -> bool {
    (1..=5).contains(&army) && core.raw_read_8(players(core) + 0x3C * army + 0x1B, -1) == 2
}

fn map_size(core: &Core) -> (i32, i32) {
    (core.raw_read_16(MAP, -1) as i32, core.raw_read_16(MAP + 2, -1) as i32)
}

fn cell(core: &Core, x: i32, y: i32) -> Option<u32> {
    let (w, h) = map_size(core);
    if x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    Some(core.raw_read_16(ROWS + 2 * y as u32, -1) as u32 + x as u32)
}

/// The unit on a plane at (x, y) (its record).
fn unit_on(core: &Core, plane: u32, x: i32, y: i32) -> Option<u32> {
    let c = cell(core, x, y)?;
    let id = core.raw_read_8(plane + c, -1) as u32;
    (id != 0).then(|| units(core) + UNIT * id)
}

fn pos(core: &Core, u: u32) -> (i32, i32) {
    (core.raw_read_8(u + 2, -1) as i32, core.raw_read_8(u + 3, -1) as i32)
}

fn bars(hp: u16) -> u32 {
    if hp == 0 { 0 } else { (hp as u32 - 1) / 10 + 1 }
}

/// A unit an Oozium of `army` would eat: alive, on the map, another team's.
fn edible(core: &Core, army: u32, u: u32) -> bool {
    let f = core.raw_read_8(u + 1, -1);
    core.raw_read_8(u, -1) != 0 && f & CARRIED == 0 && !same_team(core, army, army_of(core, u))
}

// --- The move range ---------------------------------------------------------

/// `sub_0801F4B4(x, y, type, move, [blocking])`, the flood fill's start:
/// noted when it is a human army's Oozium's (the one on (x, y)).
const FLOOD_START: u32 = 0x0801_F4B4;
fn flood_start(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (x, y, t) = (cpu.gpr(0) as u8, cpu.gpr(1) as u8, cpu.gpr(2) as u8);
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let own = t == OOZIUM
        && unit_on(core, ALL_PLANE, x as i32, y as i32)
            .is_some_and(|u| core.raw_read_8(u, -1) == OOZIUM && army_of(core, u) == army)
        && !is_cpu(core, army);
    let v = if own { [1, x, y] } else { [0, 0, 0] };
    if core.raw_read_8(FLOOD, -1) != v[0] || own {
        core.raw_write_range(FLOOD, -1, &v);
    }
}

/// `sub_0801F6F0`, one step of the flood fill, about to test for a
/// blocking unit (the fill's record in r5, the square's index in r9):
/// for a human Oozium, a square holding a unit it eats is let in, and
/// nothing goes on from one.
const BLOCK_TEST: u32 = 0x0801_F77C;
const STEP_PASS: u32 = 0x0801_F7A6;
const STEP_REJECT: u32 = 0x0801_F808;
const FLOOD_FROM: u32 = 0x0300_409C;
fn block_test(core: &mut Core) {
    if !is_on(core) || core.raw_read_8(FLOOD, -1) == 0 {
        return;
    }
    let fill = core.gba().cpu().gpr(5) as u32;
    if core.raw_read_16(fill + 0x22, -1) == 0 {
        return;
    }
    let (sx, sy) = (core.raw_read_8(FLOOD + 1, -1) as i32, core.raw_read_8(FLOOD + 2, -1) as i32);
    let Some(me) = unit_on(core, ALL_PLANE, sx, sy) else { return };
    let army = army_of(core, me);
    let from = core.raw_read_32(FLOOD_FROM, -1);
    let (fx, fy) = (core.raw_read_8(from, -1) as i32, core.raw_read_8(from + 1, -1) as i32);
    if (fx, fy) != (sx, sy) && unit_on(core, SEEN_PLANE, fx, fy).is_some_and(|u| edible(core, army, u)) {
        core.gba_mut().cpu_mut().set_thumb_pc(STEP_REJECT);
        return;
    }
    let idx = core.gba().cpu().gpr(9) as u32;
    let id = core.raw_read_8(SEEN_PLANE + idx, -1) as u32;
    if id != 0 && edible(core, army, units(core) + UNIT * id) {
        core.gba_mut().cpu_mut().set_thumb_pc(STEP_PASS);
    }
}

/// The unit the selected Oozium would eat at its destination.
fn victim_at_destination(core: &Core) -> Option<u32> {
    let me = core.raw_read_32(SELECTED, -1);
    if me == 0 || core.raw_read_8(me, -1) != OOZIUM {
        return None;
    }
    let (x, y) = (core.raw_read_16(DESTINATION, -1) as i32, core.raw_read_16(DESTINATION + 2, -1) as i32);
    let u = unit_on(core, ALL_PLANE, x, y)?;
    (u != me && edible(core, army_of(core, me), u)).then_some(u)
}

/// `sub_0802E724(x, y)`, may the selected unit end its move on (x, y)
/// (within its range, empty or a unit it joins or boards): a human
/// Oozium may on a square in its range holding a unit it eats.
const MAY_END: u32 = 0x0802_E724;
const MOVE_MAP_ROWS: u32 = 0x0300_3340;
const IN_RANGE: u8 = 0x78;
fn may_end(core: &mut Core) {
    if !is_on(core) || core.raw_read_8(FLOOD, -1) == 0 {
        return;
    }
    let me = core.raw_read_32(SELECTED, -1);
    if me == 0 || core.raw_read_8(me, -1) != OOZIUM {
        return;
    }
    let army = army_of(core, me);
    if army != core.raw_read_16(CURRENT_ARMY, -1) as u32 || is_cpu(core, army) {
        return;
    }
    let cpu = core.gba().cpu();
    let (x, y, lr) = (cpu.gpr(0) as i16 as i32, cpu.gpr(1) as i16 as i32, cpu.gpr(14) as u32);
    if cell(core, x, y).is_none() {
        return;
    }
    let reach = core.raw_read_8(core.raw_read_32(MOVE_MAP_ROWS + 4 * y as u32, -1) + x as u32, -1);
    if reach > IN_RANGE || !unit_on(core, SEEN_PLANE, x, y).is_some_and(|u| u != me && edible(core, army, u)) {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, 1);
    cpu.set_thumb_pc(lr & !1);
}

/// `sub_0802E7C8(x, y, path, ...)`, the move's walk: a unit of another team
/// on the next square of the path (every unit, hidden ones too; its id in
/// r2, the path's step in r5) stops the move there ("Trap!"). An Oozium's
/// last square is its meal instead.
const TRAP_CHECK: u32 = 0x0802_E872;
const PATH_END: [u8; 2] = [4, 0xFF];
fn trap_check(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (id, step) = (cpu.gpr(2) as u32 & 0xFF, cpu.gpr(5) as u32);
    let me = core.raw_read_32(SELECTED, -1);
    if id == 0 || me == 0 || core.raw_read_8(me, -1) != OOZIUM || !PATH_END.contains(&core.raw_read_8(step + 1, -1)) {
        return;
    }
    let army = army_of(core, me);
    if army != core.raw_read_16(CURRENT_ARMY, -1) as u32 || is_cpu(core, army) {
        return;
    }
    if edible(core, army, units(core) + UNIT * id) {
        core.gba_mut().cpu_mut().set_gpr(2, 0);
    }
}

// --- The command menu ---------------------------------------------------------

/// Wait's show test (`sub_0802C8BC`: hidden when the destination holds a
/// unit): shown for an Oozium about to eat.
const WAIT_HIDE: u32 = 0x0802_C8BC;
fn wait_hide(core: &mut Core) {
    if !is_on(core) || victim_at_destination(core).is_none() {
        return;
    }
    let lr = core.gba().cpu().gpr(14) as u32;
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, 0);
    cpu.set_thumb_pc(lr & !1);
}

/// Wait's on-select (`sub_0802CFFC`): an Oozium about to eat ends its move,
/// then eats ([`eat_next`], where Wait returns to).
const WAIT_SELECTED: u32 = 0x0802_CFFC;
fn wait_selected(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let Some(v) = victim_at_destination(core) else { return };
    let me = core.raw_read_32(SELECTED, -1);
    let lr = core.gba().cpu().gpr(14) as u32;
    begin(core, v, army_of(core, me), lr);
    core.gba_mut().cpu_mut().set_gpr(14, (EAT_NEXT | 1) as i32);
}

fn begin(core: &mut Core, victim: u32, killer: u32, ret: u32) {
    core.raw_write_32(VICTIM, -1, victim);
    core.raw_write_32(RETURN, -1, ret);
    core.raw_write_8(KILLER, -1, killer as u8);
    core.raw_write_8(STEP, -1, 0);
}

// --- The eat ------------------------------------------------------------------

/// Where each of the eat's calls returns, trapped: the alignment padding in
/// `sub_0802C8BC` (after its `b`, before its literal pool), which the game
/// never runs.
const EAT_NEXT: u32 = 0x0802_C8E2;
const PRICE: u32 = 0x0804_2C9C;
const CHARGE: u32 = 0x0804_40E0;

fn call(core: &mut Core, f: u32, args: &[u32], ret: u32) {
    let cpu = core.gba_mut().cpu_mut();
    for (k, &a) in args.iter().enumerate() {
        cpu.set_gpr(k, a as i32);
    }
    cpu.set_gpr(14, ret as i32);
    cpu.set_thumb_pc(f);
}

/// The eat, one call at a time: the victim's value
/// (`GetCoPriceMultiplier(army, type)` x its bars), the eater's meter
/// (half of it, as for damage dealt), the victim's (all of it, as for
/// damage taken), the eater's army's destroyed count, then the game's
/// destruction, which returns where the eat began.
fn eat_next(core: &mut Core) {
    let victim = core.raw_read_32(VICTIM, -1);
    if victim == 0 {
        return;
    }
    let killer = core.raw_read_8(KILLER, -1) as u32;
    let step = core.raw_read_8(STEP, -1);
    core.raw_write_8(STEP, -1, step + 1);
    let next = EAT_NEXT | 1;
    let foe = army_of(core, victim);
    match step {
        0 => {
            let t = core.raw_read_8(victim, -1) as u32;
            call(core, PRICE, &[foe, t], next);
        }
        1 => {
            let price = core.gba().cpu().gpr(0) as u32;
            let value = price * bars(core.raw_read_16(victim + 4, -1) & 0x7F);
            core.raw_write_32(VALUE, -1, value);
            call(core, CHARGE, &[killer, value / 2], next);
        }
        2 => {
            let value = core.raw_read_32(VALUE, -1);
            call(core, CHARGE, &[foe, value], next);
        }
        _ => {
            let p = players(core) + 0x3C * killer;
            let now = core.raw_read_16(p + 0x16, -1).wrapping_add(1);
            core.raw_write_16(p + 0x16, -1, now);
            if now > core.raw_read_16(p + 0x18, -1) {
                core.raw_write_16(p + 0x18, -1, now);
            }
            let ret = core.raw_read_32(RETURN, -1);
            core.raw_write_32(VICTIM, -1, 0);
            call(core, DESTROY, &[victim], ret);
        }
    }
}

// --- The CPU ------------------------------------------------------------------

/// The cost for `army`'s unit of type `t` to enter (x, y), as the flood
/// fill charges it (`CacheUnitMovementCosts`: the CO's chart for its power
/// and the weather), 0xFF where it cannot.
fn move_cost(core: &Core, army: u32, t: u8, x: i32, y: i32) -> u8 {
    let Some(c) = cell(core, x, y) else { return 0xFF };
    let terrain = core.raw_read_8(TERRAIN_PLANE + c, -1) as u32 & 0x1F;
    let p = players(core) + 0x3C * army;
    let row = if core.raw_read_8(0x0300_3FC8, -1) != 0 { core.raw_read_8(p + 0x1D, -1) as u32 } else { 1 };
    let mode = core.raw_read_8(p + 0x1E, -1) as u32;
    let weather = core.raw_read_8(crate::ds_weather::WEATHER, -1) as u32;
    let cos = core.raw_read_32(0x0801_F8E4, -1);
    let chart = core.raw_read_32(cos + 0x50 + 0x104 * row + 4 * (17 * mode + weather), -1);
    let movement = core.raw_read_8(crate::roster::table(core) + 0x5C * t as u32 + 0x19, -1) as u32;
    core.raw_read_8(chart + 32 * movement + terrain, -1)
}

fn value(core: &Core, u: u32) -> u32 {
    let t = core.raw_read_8(u, -1) as u32;
    let price = core.raw_read_16(crate::roster::table(core) + 0x5C * t + 6, -1) as u32 * 10;
    price * bars(core.raw_read_16(u + 4, -1) & 0x7F)
}

/// What a CPU Oozium at its square would eat: the most valuable unit next
/// to it on a square it can enter (ties: the first of up, right, down,
/// left).
fn best_meal(core: &Core, oozium: u32, army: u32) -> Option<u32> {
    let (x, y) = pos(core, oozium);
    let reach = core.raw_read_8(crate::roster::table(core) + 0x5C * OOZIUM as u32 + 0x0A, -1).max(1);
    let fuel = core.raw_read_8(oozium + 6, -1) & 0x7F;
    let mut best: Option<(u32, u32)> = None;
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let Some(u) = unit_on(core, ALL_PLANE, x + dx, y + dy) else { continue };
        if !edible(core, army, u) {
            continue;
        }
        let cost = move_cost(core, army, OOZIUM, x + dx, y + dy);
        if cost == 0xFF || cost > reach || cost > fuel {
            continue;
        }
        let v = value(core, u);
        if best.is_none_or(|(bv, _)| v > bv) {
            best = Some((v, u));
        }
    }
    best.map(|(_, u)| u)
}

/// A CPU army's turn starts ([`crate::cpu_tactics::cpu_unit`]): its
/// Ooziums next to units they eat are marked as moved and their eats
/// queued for the effects pass.
pub fn cpu_eats(core: &mut Core, army: u32) {
    let base = units(core);
    let mut slot = 0;
    for i in 1..255u32 {
        let u = base + UNIT * i;
        if core.raw_read_8(u, -1) != OOZIUM
            || core.raw_read_8(u + 1, -1) & (MOVED | CARRIED) != 0
            || crate::five::army_of_index(core, i) != army
        {
            continue;
        }
        while slot < CPU_SLOTS && core.raw_read_32(CPU_EATER + CPU_SLOT * slot, -1) != 0 {
            slot += 1;
        }
        if slot == CPU_SLOTS {
            return;
        }
        let Some(v) = best_meal(core, u, army) else {
            advance(core, u, army);
            continue;
        };
        let f = core.raw_read_8(u + 1, -1);
        core.raw_write_8(u + 1, -1, f | MOVED);
        core.raw_write_32(CPU_EATER + CPU_SLOT * slot, -1, u);
        core.raw_write_32(CPU_VICTIM + CPU_SLOT * slot, -1, v);
    }
}

/// A CPU Oozium with nothing to eat (AW2's CPU, finding no weapon, would
/// leave it standing) moves one square nearer the nearest unit of another
/// team, onto an empty square it can enter, and is done for the day.
fn advance(core: &mut Core, u: u32, army: u32) {
    let (x, y) = pos(core, u);
    let base = units(core);
    let foes: Vec<(i32, i32)> = (1..255u32)
        .map(|i| base + UNIT * i)
        .filter(|&v| edible(core, army, v))
        .map(|v| pos(core, v))
        .collect();
    let dist = |p: (i32, i32)| foes.iter().map(|f| (f.0 - p.0).abs() + (f.1 - p.1).abs()).min();
    let Some(now) = dist((x, y)) else { return };
    let reach = core.raw_read_8(crate::roster::table(core) + 0x5C * OOZIUM as u32 + 0x0A, -1).max(1);
    let fuel = core.raw_read_8(u + 6, -1) & 0x7F;
    let mut best: Option<(i32, (i32, i32), u8)> = None;
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let to = (x + dx, y + dy);
        if cell(core, to.0, to.1).is_none() || unit_on(core, ALL_PLANE, to.0, to.1).is_some() {
            continue;
        }
        let cost = move_cost(core, army, OOZIUM, to.0, to.1);
        if cost == 0xFF || cost > reach || cost > fuel {
            continue;
        }
        let d = dist(to).unwrap_or(now);
        if d < now && best.is_none_or(|(bd, _, _)| d < bd) {
            best = Some((d, to, cost));
        }
    }
    if let Some((_, to, cost)) = best {
        move_unit(core, u, to, cost);
        let f = core.raw_read_8(u + 1, -1);
        core.raw_write_8(u + 1, -1, f | MOVED);
    }
}

/// Moves a unit to (x, y) on the map (its record and both unit planes),
/// paying the fuel.
fn move_unit(core: &mut Core, u: u32, to: (i32, i32), cost: u8) {
    let from = pos(core, u);
    let id = ((u - units(core)) / UNIT) as u8;
    for plane in [SEEN_PLANE, ALL_PLANE] {
        if let Some(c) = cell(core, from.0, from.1) {
            if core.raw_read_8(plane + c, -1) == id {
                core.raw_write_8(plane + c, -1, 0);
            }
        }
        if let Some(c) = cell(core, to.0, to.1) {
            core.raw_write_8(plane + c, -1, id);
        }
    }
    core.raw_write_8(u + 2, -1, to.0 as u8);
    core.raw_write_8(u + 3, -1, to.1 as u8);
    let fuel = core.raw_read_8(u + 6, -1);
    core.raw_write_8(u + 6, -1, (fuel & 0x80) | (fuel & 0x7F).saturating_sub(cost));
}

/// The frame's effects pass ([`crate::cpu_tactics::effects_pass`]): a CPU
/// Oozium's queued eat is played: it moves onto its victim's square and
/// eats it, returning to `ret`. `true` when it began one.
pub fn effects_pass(core: &mut Core, ret: u32) -> bool {
    if core.raw_read_32(VICTIM, -1) != 0 {
        return false;
    }
    for k in 0..CPU_SLOTS {
        let at = CPU_EATER + CPU_SLOT * k;
        let (me, v) = (core.raw_read_32(at, -1), core.raw_read_32(at + 4, -1));
        if me == 0 {
            continue;
        }
        core.raw_write_32(at, -1, 0);
        core.raw_write_32(at + 4, -1, 0);
        let army = army_of(core, me);
        // Still there, both of them (nothing else took the victim meanwhile).
        let (to, from) = (pos(core, v), pos(core, me));
        if core.raw_read_8(me, -1) != OOZIUM
            || !edible(core, army, v)
            || (to.0 - from.0).abs() + (to.1 - from.1).abs() != 1
        {
            continue;
        }
        let cost = move_cost(core, army, OOZIUM, to.0, to.1);
        move_unit(core, me, to, cost);
        begin(core, v, army, ret);
        eat_next(core);
        return true;
    }
    false
}

// --- Immunity -------------------------------------------------------------------

/// A unit record that is an Oozium, with the pack on: COs, powers, a Missile
/// Silo and a Black Bomb leave it alone.
pub fn immune(core: &Core, unit: u32) -> bool {
    is_on(core) && (0x0200_0000..0x0204_0000).contains(&unit) && core.raw_read_8(unit, -1) == OOZIUM
}

/// Where a unit (r4) is about to be hit or helped, and where to go instead:
/// a strike's square (`sub_08044854`: Sturm's Meteor Strike, Von Bolt's Ex
/// Machina, Rachel's Covering Fire), a Missile Silo's square
/// (`sub_08026100`), a power's heal of its own army (`sub_08044E10`:
/// Hawke's), each. (Mass damage, `sub_08044F24`, is [`crate::co_powers`]'s
/// trap.)
const SKIPS: [(u32, u32); 3] = [(0x0804_48A2, 0x0804_48DE), (0x0802_614E, 0x0802_618A), (0x0804_4EA0, 0x0804_4EEE)];
fn skip(core: &mut Core, to: u32) {
    if immune(core, core.gba().cpu().gpr(4) as u32) {
        core.gba_mut().cpu_mut().set_thumb_pc(to);
    }
}

/// Tests that a unit (r4) is one to take part, the answer in r0 (0 = no):
/// a power's effect on each unit of its army (`sub_08044610`: the
/// sparkle and its `onEachUnit`, Andy's repair, Eagle's move again, Jess's
/// resupply, ...), just returned; and the CPU's scoring of a strike or a
/// silo's target (`sub_0805C2DC`, `sub_0805C514`, `sub_0805C720`: a unit
/// with 1 HP or less is not counted), about to compare its HP.
const LEAVE_OUT: [u32; 4] = [0x0804_46AC, 0x0805_C432, 0x0805_C66A, 0x0805_C878];
fn leave_out(core: &mut Core) {
    if immune(core, core.gba().cpu().gpr(4) as u32) {
        core.gba_mut().cpu_mut().set_gpr(0, 0);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    let mut t: Vec<(u32, Box<dyn Fn(&mut Core)>)> = Vec::new();
    for (at, to) in SKIPS {
        t.push((at, Box::new(move |core: &mut Core| skip(core, to))));
    }
    for at in LEAVE_OUT {
        t.push((at, Box::new(leave_out)));
    }
    t.extend(eat_traps());
    t
}

fn eat_traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (FLOOD_START, Box::new(flood_start)),
        (BLOCK_TEST, Box::new(block_test)),
        (MAY_END, Box::new(may_end)),
        (TRAP_CHECK, Box::new(trap_check)),
        (WAIT_HIDE, Box::new(wait_hide)),
        (WAIT_SELECTED, Box::new(wait_selected)),
        (EAT_NEXT, Box::new(eat_next)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram() {
        assert!(STEP < CPU_EATER && CPU_EATER + CPU_SLOT * CPU_SLOTS <= FLOOD && RAM_END <= 0x0203_FE00);
    }
}
