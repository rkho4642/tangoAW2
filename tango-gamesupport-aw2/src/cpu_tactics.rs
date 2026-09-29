//! The CPU uses the new units' abilities, with the Dual Strike pack.
//!
//! AW2's CPU moves units by role and knows Attack, Capture, Load, Supply
//! (the APC's, at turn start) and Launch at a silo; it has no idea of
//! Hide, Explode or a Black Boat's Repair. These are decided here, from the
//! game state in RAM, at the edges of a CPU army's turn (the same on both
//! netplay peers and in replays):
//!
//! - Black Bomb, when the CPU starts moving its units: it flies (within its
//!   move and fuel, to an empty square) to where exploding does the most
//!   harm and explodes if that harm, in funds, is at least
//!   [`BOMB_WORTH`] and at least twice its own side's; otherwise it flies
//!   towards the nearest enemy. The explosion is the player's Explode
//!   ([`crate::unit_actions::blast`]) and the game's own destruction.
//! - Stealth, as the CPU's turn ends: it hides when enemies that could hit
//!   it are near and none of them can hit a hidden Stealth (Fighters,
//!   Stealths), while it has fuel for it; a hidden Stealth appears when its
//!   fuel runs low (a hidden one burns 8 a day).
//! - Black Boat, as the CPU's turn ends: it repairs its army's adjacent
//!   units, as the player's Repair does.

use mgba::core::Core;

use crate::ds_weather::is_on;
use crate::roster::{BLACK_BOAT, BLACK_BOMB, STEALTH};
use crate::unit_actions::{army_of, blast_loss, blast_targets, repair_around, unit_at};

const CURRENT_ARMY: u32 = 0x0300_33EC;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT: u32 = 12;
const MAP: u32 = 0x0201_E450;
const ROWS: u32 = MAP + 0x417A;
/// The unit planes (both hold the unit id at each cell).
const PLANES: [u32; 2] = [MAP + 0x12, MAP + 0x51A];
const MOVED: u8 = 0x01;
const HIDDEN: u8 = 0x20;
const CARRIED: u8 = 0x08;
const FIGHTER: u8 = 16;

/// A CPU army: player +0x1B is 2 (1 is human).
fn is_cpu(core: &Core, army: u32) -> bool {
    (1..=5).contains(&army) && core.raw_read_8(crate::five::players(core) + 0x3C * army + 0x1B, -1) == 2
}

fn units(core: &Core) -> u32 {
    core.raw_read_32(UNITS_POINTER, -1)
}

/// Every unit of `army` on the map (not carried): (record, id).
fn army_units(core: &Core, army: u32) -> Vec<(u32, u32)> {
    let base = units(core);
    (1..255u32)
        .map(|i| (base + UNIT * i, i))
        .filter(|&(u, i)| {
            core.raw_read_8(u, -1) != 0
                && core.raw_read_8(u + 1, -1) & CARRIED == 0
                && crate::five::army_of_index(core, i) == army
        })
        .collect()
}

fn pos(core: &Core, u: u32) -> (i32, i32) {
    (core.raw_read_8(u + 2, -1) as i32, core.raw_read_8(u + 3, -1) as i32)
}

fn hp(core: &Core, u: u32) -> u16 {
    core.raw_read_16(u + 4, -1) & 0x7F
}

fn price(core: &Core, t: u8) -> u32 {
    core.raw_read_16(crate::roster::table(core) + 0x5C * t as u32 + 6, -1) as u32 * 10
}

fn bars(hp: u16) -> u32 {
    if hp == 0 { 0 } else { (hp as u32 - 1) / 10 + 1 }
}

fn teams(core: &Core, a: u32, b: u32) -> bool {
    let p = crate::five::players(core);
    core.raw_read_8(p + 0x3C * a + 0x2A, -1) == core.raw_read_8(p + 0x3C * b + 0x2A, -1)
}

// --- Black Bomb ---------------------------------------------------------------

/// Least harm, in funds, worth a Black Bomb (it costs 25000).
const BOMB_WORTH: u32 = 12000;

/// (harm to enemies, harm to its own side) in funds of a blast at (x, y).
fn blast_value(core: &Core, bomb: u32, army: u32, x: i32, y: i32) -> (u32, u32) {
    let (mut foe, mut own) = (0, 0);
    for u in blast_targets(core, bomb, x, y) {
        let h = hp(core, u);
        let lost = bars(h) - bars(h - blast_loss(h));
        let value = lost * price(core, core.raw_read_8(u, -1)) / 10;
        if teams(core, army_of(core, u), army) {
            own += value;
        } else {
            foe += value;
        }
    }
    (foe, own)
}

fn map_size(core: &Core) -> (i32, i32) {
    (core.raw_read_16(MAP, -1) as i32, core.raw_read_16(MAP + 2, -1) as i32)
}

/// Squares a flying unit at (x, y) can reach with `reach`: empty ones
/// and its own.
fn reachable(core: &Core, x: i32, y: i32, reach: i32) -> Vec<(i32, i32)> {
    let (w, h) = map_size(core);
    let mut out = Vec::new();
    for ty in (y - reach).max(0)..=(y + reach).min(h - 1) {
        for tx in (x - reach).max(0)..=(x + reach).min(w - 1) {
            if (tx - x).abs() + (ty - y).abs() > reach {
                continue;
            }
            if (tx, ty) == (x, y) || unit_at(core, tx, ty).is_none() {
                out.push((tx, ty));
            }
        }
    }
    out
}

fn move_unit(core: &mut Core, u: u32, id: u32, to: (i32, i32)) {
    let from = pos(core, u);
    for plane in PLANES {
        let row = |y: i32| core.raw_read_16(ROWS + 2 * y as u32, -1) as u32;
        let (r0, r1) = (row(from.1), row(to.1));
        core.raw_write_8(plane + r0 + from.0 as u32, -1, 0);
        core.raw_write_8(plane + r1 + to.0 as u32, -1, id as u8);
    }
    core.raw_write_8(u + 2, -1, to.0 as u8);
    core.raw_write_8(u + 3, -1, to.1 as u8);
    let fuel = core.raw_read_8(u + 6, -1);
    let used = ((to.0 - from.0).abs() + (to.1 - from.1).abs()) as u8;
    core.raw_write_8(u + 6, -1, (fuel & 0x80) | (fuel & 0x7F).saturating_sub(used));
}

/// The bombs to destroy (the game's destruction, started from the frame's
/// effects pass: [`effects_pass`]), RAM tangoAW2 keeps.
const PENDING: u32 = 0x0203_FD60; // up to 4 unit records
const PENDING_SLOTS: u32 = 4;
const CALLED: u32 = 0x0203_FD70;
const SAVED_R0: u32 = 0x0203_FD74;
/// The army whose turn the CPU is playing, once its bombs are decided.
const BOMBS_DONE: u32 = 0x0203_FD78;
const PREV_ARMY: u32 = 0x0203_FD79;

fn bombs(core: &mut Core, army: u32) {
    for (u, id) in army_units(core, army) {
        if core.raw_read_8(u, -1) != BLACK_BOMB || core.raw_read_8(u + 1, -1) & MOVED != 0 {
            continue;
        }
        let (x, y) = pos(core, u);
        let move_ = core.raw_read_8(crate::roster::table(core) + 0x5C * BLACK_BOMB as u32 + 0x0A, -1) as i32;
        let fuel = (core.raw_read_8(u + 6, -1) & 0x7F) as i32;
        let options = reachable(core, x, y, move_.min(fuel));
        let best = options
            .iter()
            .map(|&(tx, ty)| (blast_value(core, u, army, tx, ty), (tx, ty)))
            .filter(|&((foe, own), _)| foe >= BOMB_WORTH && foe >= 2 * own)
            .max_by_key(|&((foe, own), (tx, ty))| (foe - own, -(ty * 64 + tx)));
        if let Some((_, to)) = best {
            move_unit(core, u, id, to);
            crate::unit_actions::blast(core, u, to.0, to.1);
            let f = core.raw_read_8(u + 1, -1);
            core.raw_write_8(u + 1, -1, f | MOVED);
            for k in 0..PENDING_SLOTS {
                if core.raw_read_32(PENDING + 4 * k, -1) == 0 {
                    core.raw_write_32(PENDING + 4 * k, -1, u);
                    break;
                }
            }
            continue;
        }
        // Nothing worth it: towards the nearest enemy.
        let foes: Vec<(i32, i32)> = (1..=5u32)
            .filter(|&a| a != army && !teams(core, a, army))
            .flat_map(|a| army_units(core, a).into_iter().map(|(v, _)| pos(core, v)).collect::<Vec<_>>())
            .collect();
        let dist = |p: (i32, i32)| foes.iter().map(|f| (f.0 - p.0).abs() + (f.1 - p.1).abs()).min().unwrap_or(0);
        if let Some(&to) = options.iter().min_by_key(|&&p| (dist(p), p.1 * 64 + p.0)) {
            if to != (x, y) && dist(to) >= 2 {
                move_unit(core, u, id, to);
            }
        }
    }
}

/// `sub_0805D438`, the CPU's turn per unit (its behaviour row just stored):
/// the first unit of an army's turn is where its bombs are decided.
pub fn cpu_unit(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    if !is_cpu(core, army) || core.raw_read_8(BOMBS_DONE, -1) as u32 == army {
        return;
    }
    core.raw_write_8(BOMBS_DONE, -1, army as u8);
    bombs(core, army);
}

/// The frame's effects pass (`sub_0803550C`, weather in r0, trapped by
/// [`crate::sandstorm`]): a bomb waiting for its destruction gets it here,
/// the game's `sub_0804018C(unit)` (its own proc: explosion, removal),
/// returning to this same instruction. `true` when it went there.
const EFFECTS: u32 = 0x0803_5514;
pub fn effects_pass(core: &mut Core) -> bool {
    if core.raw_read_8(CALLED, -1) != 0 {
        core.raw_write_8(CALLED, -1, 0);
        let r0 = core.raw_read_32(SAVED_R0, -1);
        core.gba_mut().cpu_mut().set_gpr(0, r0 as i32);
        return false;
    }
    for k in 0..PENDING_SLOTS {
        let bomb = core.raw_read_32(PENDING + 4 * k, -1);
        if bomb == 0 {
            continue;
        }
        core.raw_write_32(PENDING + 4 * k, -1, 0);
        let r0 = core.gba().cpu().gpr(0) as u32;
        core.raw_write_32(SAVED_R0, -1, r0);
        core.raw_write_8(CALLED, -1, 1);
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, bomb as i32);
        cpu.set_gpr(14, (EFFECTS | 1) as i32);
        cpu.set_thumb_pc(crate::unit_actions::DESTROY);
        return true;
    }
    false
}

// --- Stealth and Black Boat ---------------------------------------------------

/// The Supply half of a Black Boat's Repair: adjacent units of its army get
/// full ammo and fuel.
fn resupply_around(core: &mut Core, boat: u32, army: u32) {
    let (x, y) = pos(core, boat);
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let Some(u) = unit_at(core, x + dx, y + dy) else { continue };
        if army_of(core, u) != army {
            continue;
        }
        let stats = crate::roster::table(core) + 0x5C * core.raw_read_8(u, -1) as u32;
        let ammo = core.raw_read_8(stats + 0x0B, -1) as u16 & 0xF;
        let fuel = core.raw_read_8(stats + 0x10, -1) & 0x7F;
        let w = core.raw_read_16(u + 4, -1);
        core.raw_write_16(u + 4, -1, (w & !0x780) | (ammo << 7));
        let f = core.raw_read_8(u + 6, -1);
        core.raw_write_8(u + 6, -1, (f & 0x80) | fuel);
    }
}

/// A Stealth hides when an enemy that could hit it is this close...
const THREAT_RANGE: i32 = 7;
/// ...and no enemy Fighter or Stealth (they hit a hidden one) is this
/// close; with more fuel than this (it appears again at or under it).
const HUNTER_RANGE: i32 = 10;
const HIDE_FUEL: u8 = 16;

fn can_hit_air(t: u8) -> bool {
    // The unit's damage row against a (visible) Stealth.
    crate::roster::chart(t, STEALTH, 0) > 0 || crate::roster::chart(t, STEALTH, 1) > 0
}

fn end_of_turn(core: &mut Core, army: u32) {
    let foes: Vec<(u8, (i32, i32))> = (1..=5u32)
        .filter(|&a| a != army && !teams(core, a, army))
        .flat_map(|a| {
            army_units(core, a).into_iter().map(|(v, _)| (core.raw_read_8(v, -1), pos(core, v))).collect::<Vec<_>>()
        })
        .collect();
    for (u, _) in army_units(core, army) {
        match core.raw_read_8(u, -1) {
            STEALTH => {
                let p = pos(core, u);
                let near = |r: i32, q: (i32, i32)| (q.0 - p.0).abs() + (q.1 - p.1).abs() <= r;
                let threat = foes.iter().any(|&(t, q)| near(THREAT_RANGE, q) && can_hit_air(t));
                let hunter = foes.iter().any(|&(t, q)| near(HUNTER_RANGE, q) && (t == FIGHTER || t == STEALTH));
                let fuel = core.raw_read_8(u + 6, -1) & 0x7F;
                let f = core.raw_read_8(u + 1, -1);
                let hide = threat && !hunter && fuel > HIDE_FUEL;
                let want = if hide { f | HIDDEN } else if fuel <= HIDE_FUEL || hunter { f & !HIDDEN } else { f };
                if want != f {
                    core.raw_write_8(u + 1, -1, want);
                }
            }
            BLACK_BOAT => {
                repair_around(core, u);
                resupply_around(core, u, army);
            }
            _ => {}
        }
    }
}

/// Every frame: a CPU army's turn has just ended (the army moving now
/// changed): its Stealths and Black Boats act.
pub fn tick(core: &mut Core, on: bool) {
    if !on {
        return;
    }
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let prev = core.raw_read_8(PREV_ARMY, -1) as u32;
    if army == prev {
        return;
    }
    core.raw_write_8(PREV_ARMY, -1, army as u8);
    if is_cpu(core, prev) {
        end_of_turn(core, prev);
    }
    if core.raw_read_8(BOMBS_DONE, -1) as u32 != army {
        core.raw_write_8(BOMBS_DONE, -1, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram() {
        assert!(PENDING + 4 * PENDING_SLOTS <= CALLED && PREV_ARMY < 0x0203_FD80);
    }
}
