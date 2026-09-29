//! What the new units do (stage 4c of 0.3.0), with the Dual Strike pack:
//!
//! - Stealth: Hide and Appear, the Sub's Dive and Rise (unit flag 0x20:
//!   seen only by adjacent units; hit only by Fighters and Stealths, see
//!   [`crate::roster::chart`]); 8 fuel a day while hidden.
//! - Carrier: its air cargo is resupplied at each turn start, as the
//!   Cruiser's copters are.
//! - Black Boat: Repair, the APC's Supply command: every adjacent unit of
//!   its army is resupplied and repaired by 1 HP, paid at a tenth of the
//!   unit's price per HP. It does not supply at turn start as the APC does.
//! - Black Bomb: Explode (in the silo's Launch slot): every unit within 3 spaces (either
//!   army's, but Oozium) loses 5 HP, never below 1, and the bomb is
//!   destroyed (the game's own destruction, with its explosion).
//! - Oozium: its attack destroys any ground or naval unit.
//! - Stealth's and Black Boat's commands carry their own names.

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;
use crate::roster::{BLACK_BOAT, CARRIER, OOZIUM, STEALTH};

/// The selected unit (a pointer to its 12-byte record).
const SELECTED: u32 = 0x0300_40D8;
const DIVED: u8 = 0x20;
const SUB: u8 = 0x18;
const CRUISER: u8 = 0x16;

fn selected_type(core: &Core) -> Option<u8> {
    let u = core.raw_read_32(SELECTED, -1);
    (u != 0).then(|| core.raw_read_8(u, -1))
}

// --- Stealth ----------------------------------------------------------------

/// `CanShowSubMenuItem` (Dive), testing the selected unit's type (r0)
/// against the Sub's: a Stealth passes too.
const DIVE_TYPE: u32 = 0x0802_CC98;
fn dive_type(core: &mut Core) {
    if is_on(core) && core.gba().cpu().gpr(0) as u8 == STEALTH {
        core.gba_mut().cpu_mut().set_gpr(0, SUB as i32);
    }
}

/// `sub_080253B0` (daily fuel, unit in r4) has just given a dived unit its
/// 5: a hidden Stealth burns 8.
const DIVED_FUEL: u32 = 0x0802_541A;
const HIDDEN_STEALTH_FUEL: i32 = 8;
fn dived_fuel(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let u = core.gba().cpu().gpr(4) as u32;
    if core.raw_read_8(u, -1) == STEALTH && core.raw_read_8(u + 1, -1) & DIVED != 0 {
        core.gba_mut().cpu_mut().set_gpr(5, HIDDEN_STEALTH_FUEL);
    }
}

// --- Carrier -------------------------------------------------------------

/// `sub_0802A3FC` (turn-start resupply) tests the unit's type (r2) against
/// the Cruiser's to resupply its cargo: the Carrier's too.
const CARGO_RESUPPLY: u32 = 0x0802_A456;
fn cargo_resupply(core: &mut Core) {
    if is_on(core) && core.gba().cpu().gpr(2) as u8 == CARRIER {
        core.gba_mut().cpu_mut().set_gpr(2, CRUISER as i32);
    }
}

// --- Black Boat ---------------------------------------------------------------

/// `sub_0802A1E4` (turn start: is a supplier next to this unit?) asks
/// `HasSupplyAbility` of the neighbour (r0): a Black Boat supplies only by
/// command.
const TURN_SUPPLY: u32 = 0x0802_A232;
const TURN_SUPPLY_NO: u32 = 0x0802_A250;
fn turn_supply(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let u = core.gba().cpu().gpr(0) as u32;
    if core.raw_read_8(u, -1) == BLACK_BOAT {
        core.gba_mut().cpu_mut().set_thumb_pc(TURN_SUPPLY_NO);
    }
}

const MAP: u32 = 0x0201_E450;
const UNIT_PLANE: u32 = MAP + 0x12;
const ROWS: u32 = MAP + 0x417A;
const UNITS_POINTER: u32 = 0x0849_9594;
const UNIT_SIZE: u32 = 12;
const PLAYERS_POINTER: u32 = 0x0849_9598;
const PLAYER_SIZE: u32 = 0x3C;
const HP_BITS: u16 = 0x7F;
const FULL_HP: u16 = 100;
const REPAIR_HP: u16 = 10;

/// The unit at a map cell (its record), if any.
pub(crate) fn unit_at(core: &Core, x: i32, y: i32) -> Option<u32> {
    let (w, h) = (core.raw_read_16(MAP, -1) as i32, core.raw_read_16(MAP + 2, -1) as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    let row = core.raw_read_16(ROWS + 2 * y as u32, -1) as u32;
    let id = core.raw_read_8(UNIT_PLANE + row + x as u32, -1) as u32;
    (id != 0).then(|| core.raw_read_32(UNITS_POINTER, -1) + UNIT_SIZE * id)
}

pub(crate) fn army_of(core: &Core, unit: u32) -> u32 {
    crate::five::army_of_index(core, (unit - core.raw_read_32(UNITS_POINTER, -1)) / UNIT_SIZE)
}

/// The Supply command's on-select (`sub_0802D158`): a Black Boat also
/// repairs its army's adjacent units by 1 HP each, while funds last.
const SUPPLY_SELECTED: u32 = 0x0802_D158;
fn supply_selected(core: &mut Core) {
    if !is_on(core) || selected_type(core) != Some(BLACK_BOAT) {
        return;
    }
    let boat = core.raw_read_32(SELECTED, -1);
    repair_around(core, boat);
}

/// A Black Boat's Repair where it stands: its army's adjacent units get
/// 1 HP each (and are resupplied by the Supply command itself), while
/// funds last.
pub(crate) fn repair_around(core: &mut Core, boat: u32) {
    let (x, y) = (core.raw_read_8(boat + 2, -1) as i32, core.raw_read_8(boat + 3, -1) as i32);
    let army = army_of(core, boat);
    let funds_at = core.raw_read_32(PLAYERS_POINTER, -1) + PLAYER_SIZE * army;
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let Some(u) = unit_at(core, x + dx, y + dy) else { continue };
        if army_of(core, u) != army {
            continue;
        }
        let w = core.raw_read_16(u + 4, -1);
        let hp = w & HP_BITS;
        if hp >= FULL_HP {
            continue;
        }
        let t = core.raw_read_8(u, -1) as u32;
        let price = core.raw_read_16(crate::roster::table(core) + 0x5C * t + 6, -1) as u32 * 10;
        let add = REPAIR_HP.min(FULL_HP - hp);
        let cost = price * add as u32 / 100;
        let funds = core.raw_read_32(funds_at, -1);
        if funds < cost {
            continue;
        }
        core.raw_write_32(funds_at, -1, funds - cost);
        core.raw_write_16(u + 4, -1, (w & !HP_BITS) | (hp + add));
    }
}

// --- Black Bomb ---------------------------------------------------------------

/// The command menu (`0x0849AE28`) has room for 13 commands, all used
/// (the menu keeps a flag per command in a fixed 13-byte array): Explode
/// takes the silo's Launch, which only foot soldiers ever get. While a
/// Black Bomb is selected, Launch shows when Wait does, reads "Explode",
/// and explodes.
const LAUNCH_HIDE: u32 = 0x0802_CA2C;
const WAIT_HIDE: u32 = 0x0802_C8BC;
const LAUNCH_SELECTED: u32 = 0x0802_D0A4;
/// Where the bomb's Wait returns to (see [`explode_selected`]), trapped:
/// the alignment padding after `sub_0802D0A4` (`movs r0, r0`), which the
/// game never runs.
const EXPLODE_DONE: u32 = 0x0802_D0B2;

fn launch_hide(core: &mut Core) {
    if is_on(core) && selected_type(core) == Some(crate::roster::BLACK_BOMB) {
        core.gba_mut().cpu_mut().set_thumb_pc(WAIT_HIDE);
    }
}

fn launch_selected(core: &mut Core) {
    if is_on(core) && selected_type(core) == Some(crate::roster::BLACK_BOMB) {
        explode_selected(core);
    }
}

const WAIT_SELECTED: u32 = 0x0802_CFFC;
pub(crate) const DESTROY: u32 = 0x0804_018C;
/// Where the moving unit ends up (x, y u16).
const DESTINATION: u32 = 0x0300_3100;
const RADIUS: i32 = 3;
const BLAST_HP: u16 = 50;
/// The bomb, and where its command returns to, between the two traps.
const BOMB: u32 = 0x0203_FFC0;
const BOMB_RETURN: u32 = 0x0203_FFC4;

fn explode_selected(core: &mut Core) {
    let bomb = core.raw_read_32(SELECTED, -1);
    let (bx, by) = (core.raw_read_16(DESTINATION, -1) as i32, core.raw_read_16(DESTINATION + 2, -1) as i32);
    blast(core, bomb, bx, by);
    // Then the game's Wait (the move ends as usual), then its destruction.
    let lr = core.gba().cpu().gpr(14) as u32;
    core.raw_write_32(BOMB, -1, bomb);
    core.raw_write_32(BOMB_RETURN, -1, lr);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(14, (EXPLODE_DONE | 1) as i32);
    cpu.set_thumb_pc(WAIT_SELECTED);
}

/// The units a Black Bomb at (bx, by) would hit: every unit within
/// [`RADIUS`] (either army's, but Oozium and the bomb).
pub(crate) fn blast_targets(core: &Core, bomb: u32, bx: i32, by: i32) -> Vec<u32> {
    let (w, h) = (core.raw_read_16(MAP, -1) as i32, core.raw_read_16(MAP + 2, -1) as i32);
    let mut out = Vec::new();
    for y in (by - RADIUS).max(0)..=(by + RADIUS).min(h - 1) {
        for x in (bx - RADIUS).max(0)..=(bx + RADIUS).min(w - 1) {
            if (x - bx).abs() + (y - by).abs() > RADIUS {
                continue;
            }
            let Some(u) = unit_at(core, x, y) else { continue };
            if u != bomb && core.raw_read_8(u, -1) != OOZIUM {
                out.push(u);
            }
        }
    }
    out
}

/// HP (tenths) a blast takes off a unit with `hp`: 5, never below 1.
pub(crate) fn blast_loss(hp: u16) -> u16 {
    hp - hp.saturating_sub(BLAST_HP).max(1)
}

/// A Black Bomb at (bx, by) explodes: every unit it hits loses 5 HP.
pub(crate) fn blast(core: &mut Core, bomb: u32, bx: i32, by: i32) {
    for u in blast_targets(core, bomb, bx, by) {
        let v = core.raw_read_16(u + 4, -1);
        let hp = v & HP_BITS;
        core.raw_write_16(u + 4, -1, (v & !HP_BITS) | (hp - blast_loss(hp)));
    }
}

fn explode_done(core: &mut Core) {
    let (bomb, lr) = (core.raw_read_32(BOMB, -1), core.raw_read_32(BOMB_RETURN, -1));
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, bomb as i32);
    cpu.set_gpr(14, lr as i32);
    cpu.set_thumb_pc(DESTROY);
}

// --- The CPU -------------------------------------------------------------------

/// The CPU's turn, per unit (`sub_0805D438`): its behaviour row, 12 bytes
/// per unit type in the CPU's record (`*0x085766E0` + 4 + 12 * type), has
/// just been stored at `0x03004784` (the unit in r4). The new units have
/// no rows of their own: they take their template unit's
/// ([`crate::roster::template`]), so the CPU moves and fights with them as
/// it does with those.
const BEHAVIOUR_ROW: u32 = 0x0805_D496;
const BEHAVIOUR: u32 = 0x0300_4784;
const CPU_RECORD_POINTER: u32 = 0x0857_66E0;
fn behaviour_row(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    crate::cpu_tactics::cpu_unit(core);
    let u = core.gba().cpu().gpr(4) as u32;
    if let Some(like) = crate::roster::template(core.raw_read_8(u, -1)) {
        let base = core.raw_read_32(CPU_RECORD_POINTER, -1);
        core.raw_write_32(BEHAVIOUR, -1, base + 4 + 12 * like as u32);
    }
}

/// The CPU's record has just been copied for its turn (`sub_08061788`):
/// the units it may build take their template's row (its build rate, byte
/// 11, included; the Black Bomb flies as a Bomber and explodes by
/// [`crate::cpu_tactics`]); the Piperunner (it would need pipes by its
/// base) keeps none, and the Carrier and Oozium are past the 24 types the
/// CPU's build code counts.
const CPU_RECORD_COPIED: u32 = 0x0806_184E;
const CPU_BUILDS: [u8; 4] = [crate::roster::MEGATANK, STEALTH, BLACK_BOAT, crate::roster::BLACK_BOMB];
fn cpu_record_copied(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let base = core.raw_read_32(CPU_RECORD_POINTER, -1);
    for t in CPU_BUILDS {
        let Some(like) = crate::roster::template(t) else { continue };
        let mut row = [0u8; 12];
        core.raw_read_range(base + 4 + 12 * like as u32, -1, &mut row);
        core.raw_write_range(base + 4 + 12 * t as u32, -1, &row);
    }
}

// --- Command names ------------------------------------------------------------

const TEXT_TABLE: u32 = 0x0861_0A38;
/// (text id, unit, tangoAW2's label: the game's icon glyph, then the name).
const LABELS: [(u32, u8, &[u8]); 4] = [
    (0x927, STEALTH, b"\x09\x8dHide\0"),
    (0x928, STEALTH, b"\x09\x8eAppear\0"),
    (0x925, BLACK_BOAT, b"\x09\x85Repair\0"),
    (0x929, crate::roster::BLACK_BOMB, b"\x09\x81Explode\0"),
];
const LABELS_AT: u32 = 0x0868_7200;
static GAME_LABELS: OnceLock<Vec<u32>> = OnceLock::new();

/// Every frame: while a Stealth, a Black Boat or a Black Bomb is selected,
/// its commands carry its own names.
pub fn tick(core: &mut Core, on: bool) {
    let game = GAME_LABELS.get_or_init(|| LABELS.iter().map(|l| core.raw_read_32(TEXT_TABLE + 4 * l.0, -1)).collect());
    let t = if on { selected_type(core) } else { None };
    for (k, &(id, unit, text)) in LABELS.iter().enumerate() {
        let ours = LABELS_AT + 0x10 * k as u32;
        let want = if t == Some(unit) { ours } else { game[k] };
        if want == ours && core.raw_read_8(ours, -1) != text[0] {
            core.raw_write_range(ours, -1, text);
        }
        let entry = TEXT_TABLE + 4 * id;
        if core.raw_read_32(entry, -1) != want {
            core.raw_write_32(entry, -1, want);
        }
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (DIVE_TYPE, Box::new(dive_type)),
        (DIVED_FUEL, Box::new(dived_fuel)),
        (CARGO_RESUPPLY, Box::new(cargo_resupply)),
        (TURN_SUPPLY, Box::new(turn_supply)),
        (SUPPLY_SELECTED, Box::new(supply_selected)),
        (LAUNCH_HIDE, Box::new(launch_hide)),
        (LAUNCH_SELECTED, Box::new(launch_selected)),
        (EXPLODE_DONE, Box::new(explode_done)),
        (BEHAVIOUR_ROW, Box::new(behaviour_row)),
        (CPU_RECORD_COPIED, Box::new(cpu_record_copied)),
    ]
}
