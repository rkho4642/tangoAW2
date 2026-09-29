//! Dual Strike's seven new units in AW2, with the Dual Strike pack: the
//! unit data (stage 4a of 0.3.0).
//!
//! Unit ids: AW2 left unused rows at exactly Dual Strike's ids for five of
//! them: 4 Megatank, 9 Piperunner, 12 Stealth, 13 Black Bomb, 18 Black
//! Boat. Carrier and Oozium (Dual Strike's 25 and 26) become 26 and 27:
//! 25 is AW2's damage column for a dived Sub and the Design Room's eraser.
//!
//! With the pack on, the game reads tangoAW2's copies of its per-unit data
//! in the ROM image's free space ([`DATA`]); with it off, its own, which are
//! never written. The copies:
//! - The unit table (0x085D5ABC, 25 records of 0x5C), grown to 28 records:
//!   every unit takes Dual Strike's cost, move, ammo, vision, range and fuel
//!   (as [`crate::ds_units`] describes), and the new units get whole
//!   records. 120 ROM words point at the table ([`TABLE_POINTERS`]).
//! - The damage chart: every read of it goes through two small functions
//!   (`sub_08043070`, CO-scaled, and `sub_080433F8`, raw), which return
//!   Dual Strike's chart here ([`chart`]), with its own columns for a
//!   submerged Sub and a hidden Stealth.
//! - The movement charts (7 variants of 7 rows at 0x085D511C, reached
//!   through the CO table), with two more rows each: pipes (Piperunner) and
//!   Oozium, as Dual Strike has them. Weather does not change movement in
//!   Dual Strike, so snow's and rain's pointers take clear's.
//! - The small per-unit tables (resupply, supply, home facility, AI), grown
//!   to 32 entries, and the build list.

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;

// --- Ids ---------------------------------------------------------------

pub const MEGATANK: u8 = 4;
pub const PIPERUNNER: u8 = 9;
pub const STEALTH: u8 = 12;
pub const BLACK_BOMB: u8 = 13;
pub const BLACK_BOAT: u8 = 18;
pub const CARRIER: u8 = 26;
pub const OOZIUM: u8 = 27;
pub const NEW_UNITS: [u8; 7] = [MEGATANK, PIPERUNNER, STEALTH, BLACK_BOMB, BLACK_BOAT, CARRIER, OOZIUM];
/// Records in use in the grown table (0..27).
pub const TYPES: u32 = 28;
/// Room in tangoAW2's copies, for units added later: records (unit ids are
/// one byte, and saved design maps keep 6 bits of them), movement rows.
pub const ROOM_TYPES: u32 = 64;
const ROOM_MOVEMENT_TYPES: u32 = 16;

/// A unit id's Dual Strike id (the same, but for Carrier and Oozium).
pub fn ds_id(t: u8) -> Option<u8> {
    match t {
        1..=24 => Some(t),
        CARRIER => Some(25),
        OOZIUM => Some(26),
        _ => None,
    }
}

// --- Where things are ------------------------------------------------------

/// AW2's unit table.
pub const AW2_TABLE: u32 = 0x085D_5ABC;
const RECORD: u32 = 0x5C;
const AW2_TYPES: u32 = 25;

/// tangoAW2's copies, each with room to grow ([`ROOM_TYPES`]).
const DATA: u32 = 0x0868_0000;
pub const TABLE: u32 = DATA; // 64 records: to +0x1700
const CHARTS: u32 = DATA + 0x1800; // 7 charts of 16 rows: to +0x2600
const SMALL_TABLES: u32 = DATA + 0x2800; // 64 bytes each
const TRANSPORTS: u32 = DATA + 0x2C00;
const BUILD_LIST: u32 = DATA + 0x2E00;
const CLONES: u32 = DATA + 0x3000; // see [`CLONE`]
const INFO_TEXT: u32 = DATA + 0x6000; // 64 rows of 16 text ids
const SLOTS: u32 = DATA + 0x6800; // [5][64] u16
const DESCRIPTIONS: u32 = DATA + 0x6C00; // 0x80 per string
const DATA_SENTINEL: u32 = DATA + 0x7FFC;
const DATA_MAGIC: u32 = 0x3355_5344; // "DSU3"

/// The unit table's address as the game reads it now.
pub fn table(core: &Core) -> u32 {
    if core.raw_read_32(TABLE_POINTERS[0], -1) == TABLE {
        TABLE
    } else {
        AW2_TABLE
    }
}

/// Every ROM word holding the unit table's address (literal pools and
/// agbcc's `-fforce-addr` pool words). The first is `sub_08043070`'s, which
/// [`table`] reads.
const TABLE_POINTERS: [u32; 115] = [
    0x0804_30A4, 0x0800_0A48, 0x0800_8A6C, 0x0800_8C30, 0x0801_9814, 0x0801_F8F4, 0x0802_0268, 0x0802_03BC,
    0x0802_0DB8, 0x0802_4A20, 0x0802_4B98, 0x0802_5C48, 0x0802_5F64, 0x0802_5FB4, 0x0802_70B0, 0x0802_9AEC,
    0x0802_A0F0, 0x0802_A384, 0x0802_AF58, 0x0802_CB80, 0x0802_D274, 0x0802_D708, 0x0802_D908, 0x0803_87A4,
    0x0803_8958, 0x0803_8F00, 0x0803_A06C, 0x0803_A168, 0x0803_A334, 0x0803_A610, 0x0803_A658, 0x0803_E9D0,
    0x0804_01F8, 0x0804_1F2C, 0x0804_202C, 0x0804_2148, 0x0804_21B8, 0x0804_2224, 0x0804_2290, 0x0804_2354,
    0x0804_2C90, 0x0804_30F8, 0x0804_3168, 0x0804_31D8, 0x0804_3248, 0x0804_32F4, 0x0804_33C4, 0x0804_33D4,
    0x0804_33E4, 0x0804_33F4, 0x0804_3414, 0x0804_4480, 0x0804_44AC, 0x0804_44E4, 0x0804_7348, 0x0804_74EC,
    0x0804_7A34, 0x0805_7F50, 0x0805_7FA4, 0x0805_8314, 0x0805_83D8, 0x0805_8B7C, 0x0805_8C38, 0x0805_8CC8,
    0x0805_8DD8, 0x0805_8E64, 0x0805_8F24, 0x0805_8F80, 0x0805_928C, 0x0805_944C, 0x0805_9640, 0x0805_9B48,
    0x0805_B4C8, 0x0805_BAD8, 0x0805_BB88, 0x0805_BC70, 0x0805_BDD0, 0x0805_C8EC, 0x0805_CA54, 0x0805_CB08,
    0x0805_CBBC, 0x0805_CC74, 0x0805_CDDC, 0x0805_CEF8, 0x0805_CFAC, 0x0805_D064, 0x0805_D120, 0x0805_D1DC,
    0x0805_D290, 0x0805_ED38, 0x0805_EDC4, 0x0805_EEA4, 0x0805_EF38, 0x0805_F24C, 0x0805_F414, 0x0805_F888,
    0x0805_FAF0, 0x0806_0D10, 0x0806_12C0, 0x0806_1E7C, 0x0806_2554, 0x0806_2650, 0x0806_2724, 0x0806_28AC,
    0x0806_2A20, 0x0809_09AC, 0x0809_0A48, 0x0809_0B6C, 0x0809_0BA4, 0x0809_0BDC, 0x0809_1368, 0x0812_A138,
    0x0816_DAB4, 0x0816_DADC, 0x0816_DB0C,
];
/// Words holding `&table[0].transportTable` and `&table[0].repairTable`.
const FIELD_POINTERS: [(u32, u32); 5] = [
    (0x0805_AC6C, 0x14),
    (0x0805_ECBC, 0x14),
    (0x0805_FCD0, 0x14),
    (0x0804_6A5C, 0x54),
    (0x0804_6B8C, 0x54),
];

// --- Records -----------------------------------------------------------------

const DS_OVERLAY_BASE: u32 = 0x022A_D560;
const DS_UNITS: u32 = DS_OVERLAY_BASE + 0x47A58;
const DS_RECORD: usize = 0x6C;
const DS_MOVEMENT: u32 = DS_OVERLAY_BASE + 0x471B8;

fn ds_record(t: u8) -> Option<&'static [u8]> {
    let id = ds_id(t)? as u32;
    crate::ds_pack::pack()?.overlay_at(0, DS_OVERLAY_BASE, DS_UNITS + DS_RECORD as u32 * id, DS_RECORD)
}

/// What Dual Strike's table does not say about a new unit, in AW2's terms:
/// (AW2 unit whose record it starts from, name, primary and secondary
/// weapon names (text), unit class, movement type, domain bits, AI role,
/// AI target mask, own target class).
struct New {
    id: u8,
    like: u8,
    name: usize,
    weapons: (Text, Text),
    class: u8,
    movement: u8,
    domain: u8,
    role: u8,
    threatens: u8,
    target: u8,
}

#[derive(Clone, Copy)]
enum Text {
    Game(u16),
    Ours(usize),
}
const NONE: Text = Text::Game(2266);
const M_GUN: Text = Text::Game(2253);
const MISSILES: Text = Text::Game(2260);

/// Unit names and weapons, tangoAW2's text entries ([`TEXT_BASE`] + k):
/// short, as AW2's own ("Md. Tank", "B Cptr"), to fit the build menu.
const STRINGS: [&str; 10] = [
    "Megatank",
    "Piperunr",
    "Stealth",
    "B Bomb",
    "B Boat",
    "Carrier",
    "Oozium",
    "Megacannon",
    "Pipe Cannon",
    "Omni-missile",
];
/// The new units' descriptions in the unit information screen (tangoAW2's
/// own words), text [`TEXT_BASE`] + 16 + k.
const DESCRIPTIONS_TEXT: [&[u8]; 7] = [
    b"\x82Megatank\x80: the strongest\rground unit, but short\ron fuel and ammo.",
    b"\x82Piperunner\x80: moves only\ron pipes and bases.\rFires at range 2-5.",
    b"\x82Stealth\x80: can hide. Hidden,\ronly Fighters and\rStealths can hit it.",
    b"\x82Black Bomb\x80: explodes,\rdamaging all units\rwithin 3 spaces.",
    b"\x82Black Boat\x80: carries two\rfoot soldiers and\rrepairs units nearby.",
    b"\x82Carrier\x80: carries two\rair units. Attacks air\runits at range 3-8.",
    b"\x82Oozium\x80: moves 1 space\rand destroys any unit\rit attacks.",
];
const DESCRIPTION_TEXT: u16 = 16;
const DESCRIPTION_SIZE: u32 = 0x80;

/// Text ids from 0x3D72 read their pointers from the ROM image's free space
/// (see `crate::five_map`, which uses 0x3D72..0x3D7C); these start at +32.
const TEXT_TABLE: u32 = 0x0861_0A38;
const TEXT_BASE: u16 = 0x3D72 + 32;
const TEXT_STRINGS: u32 = 0x0862_0100 + 0x20 * 32;

/// Movement types 7 and 8 (Dual Strike's pipe and Oozium rows).
const PIPE: u8 = 7;
const OOZE: u8 = 8;

const NEW: [New; 7] = [
    New { id: MEGATANK, like: 3, name: 0, weapons: (Text::Ours(7), M_GUN), class: 1, movement: 2, domain: 4, role: 5, threatens: 4, target: 4 },
    New { id: PIPERUNNER, like: 11, name: 1, weapons: (Text::Ours(8), NONE), class: 1, movement: PIPE, domain: 8, role: 4, threatens: 7, target: 4 },
    New { id: STEALTH, like: 16, name: 2, weapons: (Text::Ours(9), NONE), class: 2, movement: 4, domain: 0x10, role: 5, threatens: 7, target: 1 },
    New { id: BLACK_BOMB, like: 17, name: 3, weapons: (NONE, NONE), class: 2, movement: 4, domain: 0x10, role: 5, threatens: 0, target: 1 },
    New { id: BLACK_BOAT, like: 23, name: 4, weapons: (NONE, NONE), class: 4, movement: 6, domain: 0x20, role: 2, threatens: 0, target: 2 },
    New { id: CARRIER, like: 21, name: 5, weapons: (MISSILES, NONE), class: 4, movement: 5, domain: 0x20, role: 4, threatens: 1, target: 2 },
    New { id: OOZIUM, like: 3, name: 6, weapons: (NONE, NONE), class: 1, movement: OOZE, domain: 4, role: 5, threatens: 4, target: 4 },
];

fn text_id(t: Text) -> u16 {
    match t {
        Text::Game(id) => id,
        Text::Ours(k) => TEXT_BASE + k as u16,
    }
}

/// Transporters and their tables in tangoAW2's copy (cargo from Dual
/// Strike; where cargo may be dropped from the AW2 table named, or the
/// Lander's / Cruiser's for the new ones).
const TRANSPORTERS: [(u8, u32); 6] = [
    (7, 0x085D_63B8),
    (20, 0x085D_63F4),
    (23, 0x085D_6430),
    (22, 0x085D_646C),
    (BLACK_BOAT, 0x085D_6430),
    (CARRIER, 0x085D_646C),
];
const TRANSPORT_SIZE: u32 = 0x3C;
/// Capacity, then a flag per cargo type 0..24, then drop terrains.
const CARGO_TYPES: usize = 25;
const DROP_TERRAIN: usize = 0x1A;

fn transport_table(core: &Core, t: u8, drops_from: u32) -> Option<Vec<u8>> {
    let ds = ds_record(t)?;
    let at = u32::from_le_bytes(ds[0x18..0x1C].try_into().ok()?);
    let list = crate::ds_pack::pack()?.overlay_at(0, DS_OVERLAY_BASE, at, 28)?;
    let mut out = vec![0u8; TRANSPORT_SIZE as usize];
    core.raw_read_range(drops_from, -1, &mut out);
    out[0] = list[0];
    for c in 0..CARGO_TYPES {
        out[1 + c] = list[1 + c];
    }
    let _ = DROP_TERRAIN;
    Some(out)
}

/// One unit's record in tangoAW2's table.
fn record(core: &Core, t: u8) -> Option<[u8; RECORD as usize]> {
    let mut r = [0u8; RECORD as usize];
    let new = NEW.iter().find(|n| n.id == t);
    let src = new.map_or(t as u32, |n| n.like as u32);
    core.raw_read_range(AW2_TABLE + RECORD * src.min(AW2_TYPES - 1), -1, &mut r);
    let Some(ds) = ds_record(t) else {
        return Some(r);
    };
    for (off, bytes) in crate::ds_units::ds_fields(ds) {
        r[off as usize..off as usize + bytes.len()].copy_from_slice(&bytes);
    }
    if let Some(n) = new {
        r[0x00..0x02].copy_from_slice(&text_id(Text::Ours(n.name)).to_le_bytes());
        r[0x02..0x04].copy_from_slice(&text_id(n.weapons.0).to_le_bytes());
        r[0x04..0x06].copy_from_slice(&text_id(n.weapons.1).to_le_bytes());
        r[0x08] = if ds[0x11] > 0 { 10 } else { 0 };
        r[0x0D] = 4;
        r[0x11] = ds[0x17];
        r[0x18] = n.class;
        r[0x19] = n.movement;
        r[0x1A] = n.domain;
        r[0x1B] = n.role;
        r[0x1C] = n.threatens;
        r[0x1D] = n.target;
        r[0x14..0x18].copy_from_slice(&0u32.to_le_bytes());
    }
    if let Some(k) = TRANSPORTERS.iter().position(|&(u, _)| u == t) {
        r[0x14..0x18].copy_from_slice(&(TRANSPORTS + TRANSPORT_SIZE * k as u32).to_le_bytes());
    }
    Some(r)
}

// --- Damage --------------------------------------------------------------

/// Damage columns beyond the unit ids: AW2's for a dived Sub, and
/// tangoAW2's for a hidden Stealth.
pub const DIVED_SUB: u8 = 0x19;
pub const HIDDEN_STEALTH: u8 = 0x1C;
const DS_SUBMERGED: usize = 27;
const DS_HIDDEN_STEALTH: usize = 28;

/// Dual Strike's base damage: attacker `att`, defender column `def` (a unit
/// id, [`DIVED_SUB`] or [`HIDDEN_STEALTH`]), weapon 0 primary / 1
/// secondary.
pub fn chart(att: u8, def: u8, weapon: u32) -> u8 {
    let Some(ds) = ds_record(att) else { return 0 };
    let slot = match def {
        DIVED_SUB => DS_SUBMERGED,
        HIDDEN_STEALTH => DS_HIDDEN_STEALTH,
        d => match ds_id(d) {
            Some(i) => i as usize,
            None => return 0,
        },
    };
    let row = if weapon == 0 { 0x24 } else { 0x44 };
    ds[row + slot - 1]
}

/// `sub_080433F8(att, def, weapon)`, the raw chart value: Dual Strike's.
const RAW: u32 = 0x0804_33F8;
fn raw(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (att, def, w, lr) = (cpu.gpr(0) as u8, cpu.gpr(1) as u8, cpu.gpr(2) as u32, cpu.gpr(14) as u32);
    let v = chart(att, def, w);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, v as i32);
    cpu.set_thumb_pc(lr & !1);
}

/// `sub_08043070(co, mode, att, def)` (weapon on the stack) scales the chart
/// value by the CO: Dual Strike's value is worked out at its entry and put
/// in place of the one it loads (r4, before its CO lookup).
const SCALED: u32 = 0x0804_3070;
const SCALED_LOADED: u32 = 0x0804_3088;
const SCALED_VALUE: u32 = 0x0203_FFB8;
fn scaled(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (att, def, sp) = (cpu.gpr(2) as u8, cpu.gpr(3) as u8, cpu.gpr(13) as u32);
    let w = core.raw_read_32(sp, -1);
    let v = chart(att, def, w);
    core.raw_write_8(SCALED_VALUE, -1, v);
}
fn scaled_loaded(core: &mut Core) {
    if is_on(core) {
        let v = core.raw_read_8(SCALED_VALUE, -1);
        core.gba_mut().cpu_mut().set_gpr(4, v as i32);
    }
}

/// `CalcDamage` puts AW2's dived-Sub column (`movs r3, #0x19`) where the
/// defender is dived (the defender unit still in r3): a hidden Stealth has
/// its own.
const DIVED_COLUMN: [u32; 2] = [0x0802_4B90, 0x0802_4BF6];
fn dived_column(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (unit, pc) = (cpu.gpr(3) as u32, cpu.thumb_pc());
    if core.raw_read_8(unit, -1) == STEALTH {
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(3, HIDDEN_STEALTH as i32);
        cpu.set_thumb_pc(pc + 2);
    }
}

// --- Movement ---------------------------------------------------------------

const AW2_CHARTS: u32 = 0x085D_511C;
const CHART_COUNT: u32 = 7;
const AW2_CHART: u32 = 7 * 32;
const CHART: u32 = ROOM_MOVEMENT_TYPES * 32;
/// The "every terrain costs 1" chart (Sturm, Lash's powers).
const EVEN_CHART: u32 = 3;
const CO_TABLE: u32 = 0x085D_3DD0;
const CO_ROW: u32 = 0x104;
const CO_COUNT: u32 = 19;
/// Terrain codes from 21 are AW2's inventions: impassable.
const INVENTIONS: usize = 21;

fn moved_chart(p: u32) -> u32 {
    if (AW2_CHARTS..AW2_CHARTS + AW2_CHART * CHART_COUNT).contains(&p) {
        CHARTS + CHART * ((p - AW2_CHARTS) / AW2_CHART)
    } else {
        p
    }
}

fn charts(core: &Core) -> Option<Vec<u8>> {
    let ds = crate::ds_pack::pack()?.overlay_at(0, DS_OVERLAY_BASE, DS_MOVEMENT, 9 * 32)?;
    let mut out = Vec::new();
    for k in 0..CHART_COUNT {
        let mut c = vec![0u8; AW2_CHART as usize];
        core.raw_read_range(AW2_CHARTS + AW2_CHART * k, -1, &mut c);
        for row in [PIPE, OOZE] {
            let mut r = ds[32 * row as usize..32 * row as usize + 32].to_vec();
            for (i, v) in r.iter_mut().enumerate() {
                if i >= INVENTIONS {
                    *v = 0xFF;
                } else if k == EVEN_CHART && *v != 0xFF {
                    *v = 1;
                }
            }
            c.extend_from_slice(&r);
        }
        c.resize(CHART as usize, 0xFF);
        out.extend_from_slice(&c);
    }
    Some(out)
}

/// Every CO's movement chart pointers: (address, AW2's value, with the
/// pack). Dual Strike's weather does not change movement: snow's and rain's
/// take clear's.
static CHART_POINTERS: OnceLock<Vec<(u32, u32, u32)>> = OnceLock::new();
fn chart_pointers(core: &Core) -> &'static [(u32, u32, u32)] {
    CHART_POINTERS.get_or_init(|| {
        let mut out = Vec::new();
        for co in 0..CO_COUNT {
            for mode in 0..3 {
                let block = CO_TABLE + CO_ROW * co + 0x38 + 0x44 * mode + 0x18;
                let clear = core.raw_read_32(block, -1);
                for w in 0..3 {
                    let at = block + 4 * w;
                    out.push((at, core.raw_read_32(at, -1), moved_chart(clear)));
                }
            }
        }
        out
    })
}

// --- Small tables -------------------------------------------------------------

/// (AW2 table, entries, first type it describes, ROM words pointing at it,
/// values for the new units by id).
struct Small {
    at: u32,
    entries: u32,
    first: u8,
    refs: &'static [u32],
    values: &'static [(u8, u8)],
}

const SMALL: [Small; 9] = [
    // Can be resupplied (by an APC, a Black Boat).
    Small { at: 0x0849_95A8, entries: 25, first: 0, refs: &[0x0802_A2D4, 0x0804_2144, 0x0805_A310, 0x0805_A478, 0x0805_A6C8],
        values: &[(MEGATANK, 1), (PIPERUNNER, 1), (STEALTH, 1), (BLACK_BOMB, 1), (BLACK_BOAT, 1), (CARRIER, 1), (OOZIUM, 1)] },
    // Supplies others: the APC and the Black Boat (not id 9's leftover).
    Small { at: 0x0849_95C1, entries: 25, first: 0, refs: &[0x0804_2094],
        values: &[(PIPERUNNER, 0), (BLACK_BOAT, 1)] },
    // Home facility (no daily fuel there): airport, port.
    Small { at: 0x0849_95DA, entries: 25, first: 0, refs: &[0x0802_5460],
        values: &[(STEALTH, 0x0A), (BLACK_BOMB, 0x0A), (BLACK_BOAT, 0x0B), (CARRIER, 0x0B)] },
    // AI: factory kind (2 base, 4 airport, 6 port).
    Small { at: 0x0857_680F, entries: 25, first: 0,
        refs: &[0x0805_8310, 0x0805_83D4, 0x0805_8484, 0x0805_873C, 0x0805_A118, 0x0806_12C4, 0x0806_13B8, 0x0806_16DC, 0x0816_D9A4, 0x0816_DAE4],
        values: &[(MEGATANK, 2), (PIPERUNNER, 2), (STEALTH, 4), (BLACK_BOMB, 4), (BLACK_BOAT, 6), (CARRIER, 6), (OOZIUM, 2)] },
    // AI: rank (planes 5, copters 2, ships 1).
    Small { at: 0x0857_6828, entries: 25, first: 0, refs: &[0x0805_A6D4],
        values: &[(STEALTH, 5), (BLACK_BOMB, 5), (BLACK_BOAT, 1), (CARRIER, 1)] },
    // AI: domain (1 ground, 2 air, 3 sea).
    Small { at: 0x0857_6841, entries: 25, first: 0, refs: &[0x0805_A3C8],
        values: &[(MEGATANK, 1), (PIPERUNNER, 1), (STEALTH, 2), (BLACK_BOMB, 2), (BLACK_BOAT, 3), (CARRIER, 3), (OOZIUM, 1)] },
    // AI: counted in its tallies.
    Small { at: 0x0857_6877, entries: 25, first: 0, refs: &[0x0806_07E0],
        values: &[(MEGATANK, 1), (PIPERUNNER, 1), (STEALTH, 1), (BLACK_BOMB, 1), (BLACK_BOAT, 1), (CARRIER, 1), (OOZIUM, 1)] },
    // AI: ground unit (from type 1).
    Small { at: 0x0857_67A0, entries: 24, first: 1, refs: &[0x0805_F668, 0x0806_1650],
        values: &[(MEGATANK, 1), (PIPERUNNER, 1), (STEALTH, 0), (BLACK_BOMB, 0), (BLACK_BOAT, 0), (CARRIER, 0), (OOZIUM, 1)] },
    // Battle scene per unit, for now another unit's (Neotank, Rockets,
    // Bomber, Bomber, Lander, Battleship, Mech).
    Small { at: 0x0809_131E, entries: 26, first: 0, refs: &[0x0804_1C04],
        values: &[(MEGATANK, 7), (PIPERUNNER, 10), (STEALTH, 16), (BLACK_BOMB, 16), (BLACK_BOAT, 22), (CARRIER, 20), (OOZIUM, 1)] },
];
const SMALL_SIZE: u32 = ROOM_TYPES;

// --- Tables the new units borrow a template's rows of -------------------------

/// Per-unit tables the new units take their template unit's ([`New::like`])
/// entries of, until they have their own (graphics, info screen): (AW2
/// table, bytes per unit, units in it, first unit, ROM words pointing at
/// it). Grown to 64 units.
struct Clone {
    at: u32,
    stride: u32,
    entries: u32,
    first: u8,
    refs: &'static [u32],
}

const CLONE: [Clone; 5] = [
    // Moving unit sprites and animation.
    Clone { at: 0x0849_CD88, stride: 36, entries: 25, first: 0,
        refs: &[0x0803_5B24, 0x0803_5B38, 0x0803_5B64, 0x0803_5B7C, 0x0803_5F94, 0x0803_60CC, 0x0809_0EB0, 0x0809_0EB4, 0x0809_0EB8] },
    // Unit information: sprites and pictures per country.
    Clone { at: 0x0849_DC18, stride: 60, entries: 25, first: 0, refs: &[0x0803_A15C, 0x0803_A2A8] },
    // Unit information: picture palettes (from unit 1).
    Clone { at: 0x0855_5D30, stride: 20, entries: 24, first: 1, refs: &[0x0803_A2B4, 0x0804_BD54] },
    // Unit information: class icon.
    Clone { at: 0x0849_E224, stride: 1, entries: 25, first: 0, refs: &[0x0803_A164] },
    // Unit information: its byte script.
    Clone { at: 0x0849_97C8, stride: 4, entries: 25, first: 0, refs: &[0x0803_AB6C] },
];
/// Where each clone goes in [`CLONES`] (64 units each).
fn clone_at(k: usize) -> u32 {
    CLONES + CLONE[..k].iter().map(|c| c.stride * ROOM_TYPES).sum::<u32>()
}

/// The information screen's text: a row per unit (`0x081BA068`, s8, 1-based
/// rows of `0x0849E398`, 16 text ids each; -1 none).
const INFO_ROWS: u32 = 0x081B_A068;
const INFO_ROW_REFS: [u32; 5] = [0x0803_A538, 0x0803_A6FC, 0x0803_A868, 0x0804_72B4, 0x0804_745C];
const INFO_TEXT_AW2: u32 = 0x0849_E398;
const INFO_TEXT_REFS: [u32; 3] = [0x0803_A534, 0x0803_A6F8, 0x0803_A864];
const INFO_TEXT_ROWS: u32 = 19;
const INFO_ROWS_AT: u32 = SMALL_TABLES + SMALL_SIZE * SMALL.len() as u32;

/// The map's unit sprites: slot per (country, unit), `u16 [5][25]` at
/// 0x08499608, read by `sub_080261A4` with a row stride of `movs r1, #0x32`.
const AW2_SLOTS: u32 = 0x0849_9608;
const SLOTS_REF: u32 = 0x0802_61C4;
const SLOTS_STRIDE: (u32, u16, u16) = (0x0802_61B2, 0x2132, 0x2180);

// --- Build menu --------------------------------------------------------------

/// The build menu's order (the game's, 0x081BA054, with the new units),
/// ended by -1; reached through one pool word.
const AW2_BUILD_LIST: u32 = 0x081B_A054;
const BUILD_LIST_POINTER: u32 = 0x0809_0C08;
const BUILD: [u8; 25] = [
    1, 2, 6, 5, 3, 8, MEGATANK, 7, 10, 11, 14, 15, PIPERUNNER, 16, 17, 19, 20, STEALTH, BLACK_BOMB, 21, 22, 23, 24,
    BLACK_BOAT, CARRIER,
];
/// A base builds units whose domain has any of these bits (`sub_0802D5E8`,
/// `movs r4, #7`): with the Piperunner's pipe bit (8) too.
const BASE_MASK: (u32, u16, u16) = (0x0802_D654, 0x2407, 0x240F);

// --- Putting it together --------------------------------------------------------

fn install(core: &mut Core) -> bool {
    if core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC {
        return true;
    }
    let Some(c) = charts(core) else { return false };
    let mut records = Vec::new();
    for t in 0..TYPES as u8 {
        let Some(r) = record(core, t) else { return false };
        records.push(r);
    }
    let mut transports = Vec::new();
    for &(t, drops) in TRANSPORTERS.iter() {
        let Some(tt) = transport_table(core, t, drops) else { return false };
        transports.push(tt);
    }
    for (t, r) in records.iter().enumerate() {
        core.raw_write_range(TABLE + RECORD * t as u32, -1, r);
    }
    for (k, tt) in transports.iter().enumerate() {
        core.raw_write_range(TRANSPORTS + TRANSPORT_SIZE * k as u32, -1, tt);
    }
    core.raw_write_range(CHARTS, -1, &c);
    for (k, s) in SMALL.iter().enumerate() {
        let mut t = vec![0u8; SMALL_SIZE as usize];
        core.raw_read_range(s.at, -1, &mut t[s.first as usize..s.first as usize + s.entries as usize]);
        for &(id, v) in s.values {
            t[id as usize] = v;
        }
        // Stored from `first`, as the game indexes it.
        core.raw_write_range(SMALL_TABLES + SMALL_SIZE * k as u32, -1, &t[s.first as usize..]);
    }
    for (k, c) in CLONE.iter().enumerate() {
        let mut t = vec![0u8; (c.stride * ROOM_TYPES) as usize];
        let aw2 = (c.stride * c.entries) as usize;
        core.raw_read_range(c.at, -1, &mut t[..aw2]);
        for n in &NEW {
            let (src, dst) = (n.like as usize - c.first as usize, n.id as usize - c.first as usize);
            let row = t[src * c.stride as usize..(src + 1) * c.stride as usize].to_vec();
            t[dst * c.stride as usize..(dst + 1) * c.stride as usize].copy_from_slice(&row);
        }
        core.raw_write_range(clone_at(k), -1, &t);
    }
    // Information text: the game's rows, then one per new unit with its own
    // description and the template's lines about move, vision and fuel.
    let mut rows = vec![0u8; ROOM_TYPES as usize];
    core.raw_read_range(INFO_ROWS, -1, &mut rows[..AW2_TYPES as usize]);
    let mut text = vec![0u8; 32 * ROOM_TYPES as usize];
    core.raw_read_range(INFO_TEXT_AW2, -1, &mut text[..32 * INFO_TEXT_ROWS as usize]);
    for (k, n) in NEW.iter().enumerate() {
        let row = INFO_TEXT_ROWS as usize + k;
        rows[n.id as usize] = row as u8 + 1;
        let template = rows[n.like as usize] as usize - 1;
        let mut ids = [0u16; 16];
        ids[0] = TEXT_BASE + DESCRIPTION_TEXT + k as u16;
        for f in 1..4 {
            ids[f] = u16::from_le_bytes([text[32 * template + 2 * f], text[32 * template + 2 * f + 1]]);
        }
        for (f, id) in ids.iter().enumerate() {
            text[32 * row + 2 * f..32 * row + 2 * f + 2].copy_from_slice(&id.to_le_bytes());
        }
        let at = DESCRIPTIONS + DESCRIPTION_SIZE * k as u32;
        let mut b = DESCRIPTIONS_TEXT[k].to_vec();
        b.push(0);
        core.raw_write_range(at, -1, &b);
    }
    core.raw_write_range(INFO_ROWS_AT, -1, &rows);
    core.raw_write_range(INFO_TEXT, -1, &text);
    // Map sprite slots, 64 units a row.
    let mut slots = vec![0u8; 2 * 5 * ROOM_TYPES as usize];
    for country in 0..5 {
        let mut row = vec![0u8; 2 * AW2_TYPES as usize];
        core.raw_read_range(AW2_SLOTS + 0x32 * country, -1, &mut row);
        for n in &NEW {
            let l = 2 * n.like as usize;
            let (a, b) = (row[l], row[l + 1]);
            let d = 2 * n.id as usize;
            if d + 1 < row.len() {
                row[d] = a;
                row[d + 1] = b;
            } else {
                row.resize(d + 2, 0);
                row[d] = a;
                row[d + 1] = b;
            }
        }
        let o = 2 * ROOM_TYPES as usize * country as usize;
        slots[o..o + row.len()].copy_from_slice(&row);
    }
    core.raw_write_range(SLOTS, -1, &slots);
    let mut list = BUILD.to_vec();
    list.push(0xFF);
    core.raw_write_range(BUILD_LIST, -1, &list);
    for (k, text) in STRINGS.iter().enumerate() {
        let at = TEXT_STRINGS + 0x20 * k as u32;
        let mut b = text.as_bytes().to_vec();
        b.push(0);
        core.raw_write_range(at, -1, &b);
    }
    core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
    true
}

fn switch32(core: &mut Core, at: u32, want: u32) {
    if core.raw_read_32(at, -1) != want {
        core.raw_write_32(at, -1, want);
    }
}

/// Every frame: the game reads tangoAW2's copies with the pack on, its own
/// without (idempotent; the same on both netplay peers).
pub fn tick(core: &mut Core, on: bool) {
    let pointers = chart_pointers(core);
    let on = on && install(core);
    let installed = core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC;
    if !on && !installed {
        return;
    }
    let (table, fields) = if on { (TABLE, TABLE) } else { (AW2_TABLE, AW2_TABLE) };
    for at in TABLE_POINTERS {
        switch32(core, at, table);
    }
    for (at, field) in FIELD_POINTERS {
        switch32(core, at, fields + field);
    }
    for &(at, aw2, ds) in pointers {
        switch32(core, at, if on { ds } else { aw2 });
    }
    for (k, s) in SMALL.iter().enumerate() {
        let want = if on { SMALL_TABLES + SMALL_SIZE * k as u32 } else { s.at };
        for &at in s.refs {
            switch32(core, at, want);
        }
    }
    switch32(core, BUILD_LIST_POINTER, if on { BUILD_LIST } else { AW2_BUILD_LIST });
    for (k, c) in CLONE.iter().enumerate() {
        let want = if on { clone_at(k) } else { c.at };
        for &at in c.refs {
            switch32(core, at, want);
        }
    }
    for at in INFO_ROW_REFS {
        switch32(core, at, if on { INFO_ROWS_AT } else { INFO_ROWS });
    }
    for at in INFO_TEXT_REFS {
        switch32(core, at, if on { INFO_TEXT } else { INFO_TEXT_AW2 });
    }
    switch32(core, SLOTS_REF, if on { SLOTS } else { AW2_SLOTS });
    let (at, aw2, ds) = SLOTS_STRIDE;
    let want = if on { ds } else { aw2 };
    let now = core.raw_read_16(at, -1);
    if now != want && (now == aw2 || now == ds) {
        core.raw_write_16(at, -1, want);
    }
    let (at, aw2, ds) = BASE_MASK;
    let want = if on { ds } else { aw2 };
    let now = core.raw_read_16(at, -1);
    if now != want && (now == aw2 || now == ds) {
        core.raw_write_16(at, -1, want);
    }
    if on {
        for k in 0..STRINGS.len() as u32 {
            switch32(core, TEXT_TABLE + 4 * (TEXT_BASE as u32 + k), TEXT_STRINGS + 0x20 * k);
        }
        for k in 0..DESCRIPTIONS_TEXT.len() as u32 {
            let entry = TEXT_TABLE + 4 * (TEXT_BASE as u32 + DESCRIPTION_TEXT as u32 + k);
            switch32(core, entry, DESCRIPTIONS + DESCRIPTION_SIZE * k);
        }
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    let mut t: Vec<(u32, Box<dyn Fn(&mut Core)>)> = vec![
        (RAW, Box::new(raw)),
        (SCALED, Box::new(scaled)),
        (SCALED_LOADED, Box::new(scaled_loaded)),
    ];
    for at in DIVED_COLUMN {
        t.push((at, Box::new(dived_column)));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(ds_id(CARRIER), Some(25));
        assert_eq!(ds_id(OOZIUM), Some(26));
        assert_eq!(ds_id(25), None);
        assert_eq!(moved_chart(AW2_CHARTS + AW2_CHART * 2), CHARTS + CHART * 2);
        assert!(BUILD.len() < 24);
    }
}
