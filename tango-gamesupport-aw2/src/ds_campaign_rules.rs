//! Dual Strike's campaign conditions and actions, in Rust: what Dual
//! Strike's scripts call (its own code, in overlay 1 and the ARM9 image) is
//! reached through magic stubs ([`crate::ds_campaign_data::Magic`]) and runs
//! here, on AW2's state.
//!
//! Dual Strike's battle state is AW2's grown: its units are AW2's records
//! (army base + 1..50, type at +0, flags at +1 with bit 0 "has moved",
//! fuel in 7 bits), its map keeps AW2's terrain classes (kind | owner << 5,
//! through a row-offset table), its structures AW2's inventions list
//! (8 bytes: x, y, kind in bits 6..9 of the halfword at +2, HP at +4). So
//! each of Dual Strike's 40 or so mission conditions reads the same thing
//! on AW2's state here ([`predicate`], one entry per Dual Strike function,
//! as read from its code).

use mgba::core::Core;

use crate::ds_campaign_data::{Magic, GRAND_BOLT_WEAK_POINTS, MEANS_TO_AN_END, MTE_CRYSTALS};

/// AW2's player table pointer (tangoAW2 moves the table in five-army games).
const PLAYERS_PTR: u32 = 0x0849_9598;
const UNITS_PTR: u32 = 0x0849_9594;
const PLAYER: u32 = 0x3C;
const CO: u32 = 0x1D;
const UNIT: u32 = 12;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const DAY: u32 = 0x0300_4080;
/// The unit acting (selected), and the map cursor.
const ACTING_UNIT: u32 = 0x0300_40D8;
const CURSOR_X: u32 = 0x0300_33E4;
const CURSOR_Y: u32 = 0x0300_33E6;
/// gMap: tiles (u16) at +0xA22, terrain classes at +0x1432, row offsets
/// (u16 per row) at +0x417A.
const MAP: u32 = 0x0201_E450;
/// The inventions list: 16 entries of 8 bytes.
const INVENTIONS: u32 = 0x0202_8360;
/// AW2's mission-local flags 0..0x1F (bank 0).
const LOCAL_FLAGS: u32 = 0x0300_33F4;
/// gPlaySt weather: now, next.
const WEATHER: u32 = 0x0300_3FEC;
const NEXT_WEATHER: u32 = 0x0300_3FEE;
/// The unit table (fuel at +0x10).
const UNIT_TABLE_PTR: u32 = 0x085D_5ABC;

fn players(core: &Core) -> u32 {
    core.raw_read_32(PLAYERS_PTR, -1)
}

/// A Dual Strike CO's country, as the CO select screen's tabs number them
/// (0 Orange Star, 1 Blue Moon, 2 Green Earth, 3 Yellow Comet, 4 Black Hole).
pub fn country(ds_co: u8) -> u8 {
    match ds_co {
        1 | 2 | 3 | 5 | 20 | 21 => 0,
        4 | 6 | 17 | 22 => 1,
        9 | 10 | 18 | 23 => 2,
        7 | 8 | 19 | 24 => 3,
        _ => 4,
    }
}

/// An army's live units: (address, type, flags, x, y, fuel).
fn units(core: &Core, army: u32) -> Vec<(u32, u8, u8, u8, u8, u8)> {
    let base = core.raw_read_32(UNITS_PTR, -1);
    (1..=50)
        .filter_map(|slot| {
            let a = base + UNIT * ((army - 1) * 64 + slot);
            let t = core.raw_read_8(a, -1);
            (t != 0).then(|| {
                (a, t, core.raw_read_8(a + 1, -1), core.raw_read_8(a + 2, -1), core.raw_read_8(a + 3, -1), core.raw_read_8(a + 6, -1) & 0x7F)
            })
        })
        .collect()
}

fn class_at(core: &Core, x: u32, y: u32) -> u8 {
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_read_8(MAP + 0x1432 + row + x, -1)
}

fn tile_at(core: &Core, x: u32, y: u32) -> u16 {
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_read_16(MAP + 0xA22 + 2 * (row + x), -1)
}

fn owner_at(core: &Core, x: u32, y: u32) -> u8 {
    class_at(core, x, y) >> 5
}

/// The cell of the unit acting (else the cursor's).
fn action_cell(core: &Core) -> (u32, u32) {
    let u = core.raw_read_32(ACTING_UNIT, -1);
    if (0x0200_0000..0x0204_0000).contains(&u) {
        (core.raw_read_8(u + 2, -1) as u32, core.raw_read_8(u + 3, -1) as u32)
    } else {
        (core.raw_read_16(CURSOR_X, -1) as u32, core.raw_read_16(CURSOR_Y, -1) as u32)
    }
}

/// Dual Strike's structure kinds as AW2's inventions: 4 minicannons, 9 the
/// Black Crystals (tangoAW2's Crystal: a minicannon on tile 0x192), 0xA
/// the Black Obelisks (tangoAW2's Obelisk: a Black Cannon with tile 0x193
/// in its middle; also what the Grand Bolt's weak points become,
/// crate::ds_campaign_data::convert_map).
fn inventions(core: &Core, ds_kind: u8) -> Vec<(u8, u8)> {
    let mut out = Vec::new();
    for k in 0..16 {
        let a = INVENTIONS + 8 * k;
        let kind = (core.raw_read_16(a + 2, -1) >> 6) & 0xF;
        if kind == 0 {
            break;
        }
        let (x, y) = (core.raw_read_8(a, -1) as u32, core.raw_read_8(a + 1, -1) as u32);
        let tile = tile_at(core, x, y);
        let is = match ds_kind {
            4 => kind == 4 && tile != 0x192,
            9 => kind == 4 && tile == 0x192,
            // (an Obelisk is 3x3: the list keeps its top-left cell)
            0xA => kind == 3 && tile_at(core, x + 1, y + 1) == 0x193,
            _ => false,
        };
        if is {
            out.push((core.raw_read_8(a + 4, -1), kind as u8));
        }
    }
    out
}

fn alive_inventions(core: &Core, ds_kind: u8) -> usize {
    inventions(core, ds_kind).iter().filter(|(hp, _)| *hp > 0).count()
}

/// The player's team still in the battle (AW2's own test, `sub_0803861C`).
pub fn player_won(core: &Core) -> bool {
    let p = players(core);
    let team = core.raw_read_8(p + PLAYER + 0x2A, -1);
    let alive = |a: u32| core.raw_read_8(p + PLAYER * a + 0x1B, -1) != 0 && core.raw_read_16(p + PLAYER * a + 0x14, -1) == 0;
    (1..=4u32).any(|a| alive(a) && core.raw_read_8(p + PLAYER * a + 0x2A, -1) == team)
        && !(1..=4u32).any(|a| alive(a) && core.raw_read_8(p + PLAYER * a + 0x2A, -1) != team)
}

fn max_fuel(core: &Core, t: u8) -> u8 {
    let _ = UNIT_TABLE_PTR;
    core.raw_read_8(crate::roster::table(core) + 0x5C * t as u32 + 0x10, -1)
}

/// [`run`]'s answer when a game function was tail-called (the landing
/// must not return).
pub const TAIL_CALLED: u32 = 0xFFFF_FFFF;

/// Runs a magic function; returns r0.
pub fn run(core: &mut Core, m: &Magic) -> u32 {
    match *m {
        Magic::Predicate(f) => {
            let held = predicate(core, f);
            // (the match-end lists' "player won" is not a mission condition)
            if held && f != 0x020D_5D2C {
                core.raw_write_32(crate::ds_campaign::LAST_CONDITION, -1, f);
                let day = core.raw_read_16(DAY, -1);
                core.raw_write_16(crate::ds_campaign::LAST_CONDITION + 4, -1, day);
            }
            held as u32
        }
        Magic::CoPair { army, a, b } => {
            let army = if army == 5 { core.raw_read_16(CURRENT_ARMY, -1) as u32 } else { army as u32 };
            let co = core.raw_read_8(players(core) + PLAYER * army + CO, -1);
            // AW2 armies have one CO: the tag partner (b) is not checked.
            let _ = b;
            (a == 0xFF || co == a) as u32
        }
        Magic::Call(f, arg) => {
            if call(core, f, arg) {
                return TAIL_CALLED;
            }
            0
        }
        Magic::Countdown(_) | Magic::ArmyFlag { .. } | Magic::Unhandled(_) | Magic::Flow(_) => 0,
    }
}

/// Dual Strike's predicates by address (overlay 1, and 0x020D5D2C in the
/// ARM9 image). Unknown ones are false.
pub fn predicate(core: &mut Core, f: u32) -> bool {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let all_owned = |core: &Core, cells: &[(u32, u32)], owner: u8| cells.iter().all(|&(x, y)| owner_at(core, x, y) == owner);
    match f {
        // The match is won by the player's team (the match-end lists).
        0x020D_5D2C => player_won(core),
        // Every unit of the army moving now has moved (the tutorials' "units
        // awaiting orders"; Dual Strike's versions name the mission's types).
        0x0235_066C | 0x0235_0824 | 0x0235_0940 | 0x0235_0A1C | 0x0235_0B28 => {
            units(core, army.clamp(1, 4)).iter().all(|u| u.2 & 1 != 0)
        }
        // Army 1 has no Infantry left (The New Black: the player loses with
        // the last one; Dual Strike reads army 1's unit range, whoever moves).
        0x0235_07A8 => !units(core, 1).iter().any(|u| u.1 == 1),
        // Means to an End's choice: converted to AW2's own two-option answer
        // (ds_campaign_data, `IsTwoOptionChoiceFirst`); never reached here.
        0x0201_99A4 => true,
        // Every unit of the army out of fuel.
        0x0235_0708 => units(core, army.clamp(1, 4)).iter().all(|u| u.5 == 0),
        // The action's cell is right of column 7.
        0x0235_0638 => action_cell(core).0 > 7,
        // An army has no unit of a type left.
        0x0235_0BE4 => !units(core, 2).iter().any(|u| u.1 == 9),
        0x0235_0D60 => !units(core, 1).iter().any(|u| u.1 == 23),
        0x0235_0FF0 => !units(core, 3).iter().any(|u| u.1 == crate::roster::MEGATANK),
        // The action's cell is an airport / a Com Tower of the player's side.
        0x0235_0C60 => {
            let (x, y) = action_cell(core);
            let c = class_at(core, x, y);
            c & 0x1F == 0x0A && c >> 5 == 1
        }
        0x0235_0F28 => {
            let (x, y) = action_cell(core);
            let c = class_at(core, x, y);
            c & 0x1F == crate::com_tower::LAB && matches!(c >> 5, 1 | 2)
        }
        // A property of the player's side at a cell (a city hiding a map, a
        // factory, a lab).
        0x0235_0DDC => owner_at(core, 9, 1) == 1,
        0x0235_0E6C => owner_at(core, 7, 6) == 1,
        0x0235_0EC4 => matches!(owner_at(core, 17, 1), 1 | 2),
        0x0235_106C => owner_at(core, 23, 14) == 1,
        0x0235_1640 => matches!(owner_at(core, 14, 1), 1 | 2),
        0x0235_1744 => all_owned(core, &[(8, 2), (8, 15)], 1),
        0x0235_1804 => all_owned(core, &[(7, 7), (7, 12), (12, 7), (12, 12)], 1),
        // None of the four silo bases is Black Hole's (army 3) any more.
        0x0235_1B88 => ![(1, 4), (16, 1), (4, 16), (19, 13)].iter().any(|&(x, y)| owner_at(core, x, y) == 3),
        // Structures: one gone (count changed), all gone, one damaged.
        0x0235_0CD4 => alive_inventions(core, 4) != 4,
        0x0235_10FC => inventions(core, 4).iter().filter(|(hp, _)| *hp >= 99).count() != 4,
        // (Means to an End: once per crystal shattered, its dialogue each
        // time, as Dual Strike's second front)
        0x0235_1C58 if crate::ds_campaign::mission(core) == MEANS_TO_AN_END as u8 => {
            let gone = (0..3).filter(|&k| !crystal_alive(core, k)).count() as u8;
            let told = core.raw_read_8(MTE_TOLD, -1);
            if gone > told {
                core.raw_write_8(MTE_TOLD, -1, told + 1);
                true
            } else {
                false
            }
        }
        0x0235_1C58 => alive_inventions(core, 9) != 3,
        0x0235_05C0 | 0x0235_1708 => alive_inventions(core, 0xA) == 0,
        0x0235_05E8 => alive_inventions(core, 9) == 0,
        0x0235_0610 => alive_inventions(core, 4) == 0,
        // A stealth of the army moving now (or the one acting) on half its
        // fuel or less; the acting unit a carrier that has moved.
        0x0235_1174 => units(core, army.clamp(1, 4)).iter().any(|u| u.1 == 12 && u.5 <= max_fuel(core, 12) / 2),
        0x0235_1268 => {
            let u = core.raw_read_32(ACTING_UNIT, -1);
            (0x0200_0000..0x0204_0000).contains(&u)
                && core.raw_read_8(u, -1) == 12
                && core.raw_read_8(u + 6, -1) & 0x7F <= max_fuel(core, 12) / 2
        }
        0x0235_12EC => {
            let u = core.raw_read_32(ACTING_UNIT, -1);
            (0x0200_0000..0x0204_0000).contains(&u) && core.raw_read_8(u, -1) == crate::roster::CARRIER && core.raw_read_8(u + 1, -1) & 1 != 0
        }
        // An oozium in sight of the player (within 4 cells of a unit of army 1).
        0x0235_1444 => {
            let mine = units(core, 1);
            (2..=4).any(|a| {
                units(core, a).iter().any(|o| {
                    o.1 == crate::roster::OOZIUM
                        && mine.iter().any(|m| (m.3 as i32 - o.3 as i32).abs() + (m.4 as i32 - o.4 as i32).abs() <= 4)
                })
            })
        }
        // Means to an End: the Grand Bolt's three weak points all destroyed
        // (Dual Strike's kinds 0xB..0xD; tangoAW2's Obelisks on them).
        0x0235_0560 => alive_inventions(core, 0xA) == 0,
        // A weak point still standing, and the cell below it (where it
        // spawns an Oozium) not held by the moving army's own unit.
        0x0235_21A4 => weak_point_spawns(core, 0, army),
        0x0235_20F8 => weak_point_spawns(core, 1, army),
        0x0235_204C => weak_point_spawns(core, 2, army),
        // The DS Campaign is Hard (the triggers of one difficulty).
        crate::ds_campaign_data::HARD_CAMPAIGN => crate::ds_campaign::hard(core),
        // Every 6th day (the Grand Bolt's charge).
        0x0235_1CC8 => {
            let d = core.raw_read_16(DAY, -1);
            d != 0 && d % 6 == 0
        }
        // Crystal Calamity: the Black Onyx's laser charged to 90% or more,
        // a real-time charge on Dual Strike's top screen (its 50 minutes,
        // op 0x5A, are left out): never (docs/AW2.md).
        0x0235_172C => false,
        _ => false,
    }
}

/// The predicates and calls [`predicate`] and [`call`] know (the rest are
/// false / do nothing); a test lists the campaign's others.
#[cfg(test)]
pub const KNOWN: &[u32] = &[crate::ds_campaign_data::HARD_CAMPAIGN, 0x0200_0000, 0x0204_0000, 0x020D_5D2C, 0x0235_05C0, 0x0235_05E8, 0x0235_0610, 0x0235_0638, 0x0235_066C, 0x0235_0708, 0x0235_0824, 0x0235_0940, 0x0235_0A1C, 0x0235_0B28, 0x0235_0BE4, 0x0235_0C60, 0x0235_0CD4, 0x0235_0D60, 0x0235_0DDC, 0x0235_0E6C, 0x0235_0EC4, 0x0235_0F28, 0x0235_0FF0, 0x0235_106C, 0x0235_10FC, 0x0235_1174, 0x0235_1268, 0x0235_12EC, 0x0235_1444, 0x0235_1640, 0x0235_1708, 0x0235_1744, 0x0235_1804, 0x0235_1B88, 0x0235_1C58, 0x0235_1CC8, 0x0235_07A8, 0x0201_99A4, 0x0235_0560, 0x0235_21A4, 0x0235_20F8, 0x0235_204C, 0x0235_1F34, 0x0235_1EB8, 0x0235_1E3C, 0x0235_2018, 0x0235_1FE4, 0x0235_1FB0, 0x0235_0E34, 0x0235_0FA8, 0x0235_0FB8, 0x0235_10C4, 0x0235_16B8, 0x0235_17C4, 0x0235_17F4, 0x0235_172C, 0x0235_0D44, 0x0235_1988, 0x0235_18CC, 0x0235_1D28, 0x0235_1D3C, 0x0235_16A4, 0x0235_1334, 0x0235_1538, 0x0235_17D4, 0x0235_17E4, 0x0235_1A4C, 0x0235_1AF0, 0x0235_1D50, 0x0235_1D74, 0x0235_1D9C, 0x0235_1DC8, 0x0235_1DD8, 0x0235_1DE8, 0x0235_1E04, 0x0235_1E20, 0x0200_3F8C, 0x0201_993C, 0x0201_9950];

/// The Obelisk standing on weak point `k` ([`GRAND_BOLT_WEAK_POINTS`]: its
/// bottom row's middle; the inventions list keeps its top-left cell).
fn weak_point_alive(core: &Core, k: usize) -> bool {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    (0..16).map(|i| INVENTIONS + 8 * i).take_while(|&a| (core.raw_read_16(a + 2, -1) >> 6) & 0xF != 0).any(|a| {
        (core.raw_read_16(a + 2, -1) >> 6) & 0xF == 3
            && core.raw_read_8(a, -1) as u32 + 1 == x
            && core.raw_read_8(a + 1, -1) as u32 + 2 == y
            && core.raw_read_8(a + 4, -1) > 0
    })
}

/// The unit id on a cell (0: none).
fn unit_id_at(core: &Core, x: u32, y: u32) -> u8 {
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_read_8(MAP + 0x12 + row + x, -1)
}

/// Means to an End's state (EWRAM the game never writes, past the DS
/// Campaign's records): the crystals shattered so far told, and each weak
/// point's hit points while its crystal stands.
const MTE_TOLD: u32 = 0x0203_F700;
const MTE_HP: u32 = 0x0203_F701;
/// Ring of Fire: its Volcano stilled (Dual Strike's rule: Black Hole's
/// unit hiding in the city found).
const VOLCANO_STILL: u32 = 0x0203_F704;
/// The mission's state above, cleared when a mission starts (0x0203F710..
/// holds the Volcano's eruption cells: [`eruption`]).
const MISSION_STATE: (u32, u32) = (0x0203_F700, 0x10);

/// Crystal `k` of Means to an End still stands.
pub fn crystal_alive(core: &Core, k: usize) -> bool {
    let (x, y) = MTE_CRYSTALS[k];
    (0..16).map(|i| INVENTIONS + 8 * i).take_while(|&a| (core.raw_read_16(a + 2, -1) >> 6) & 0xF != 0).any(|a| {
        (core.raw_read_16(a + 2, -1) >> 6) & 0xF == 4
            && core.raw_read_8(a, -1) as u32 == x
            && core.raw_read_8(a + 1, -1) as u32 == y
            && core.raw_read_8(a + 4, -1) > 0
    })
}

/// The invention entry of weak point `k` (its Obelisk: top-left cell).
fn weak_point_entry(core: &Core, k: usize) -> Option<u32> {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    (0..16).map(|i| INVENTIONS + 8 * i).take_while(|&a| (core.raw_read_16(a + 2, -1) >> 6) & 0xF != 0).find(|&a| {
        (core.raw_read_16(a + 2, -1) >> 6) & 0xF == 3 && core.raw_read_8(a, -1) as u32 + 1 == x && core.raw_read_8(a + 1, -1) as u32 + 2 == y
    })
}

/// Means to an End in a session.
fn means_to_an_end(core: &Core) -> bool {
    crate::ds_campaign::active(core) && crate::ds_campaign::mission(core) == MEANS_TO_AN_END as u8 && crate::ds_campaign::in_battle(core)
}

/// Every frame in Means to an End: a weak point whose crystal stands
/// takes no damage (its force field: its hit points kept); once its
/// crystal is shattered it is open.
pub fn mte_tick(core: &mut Core) {
    if !means_to_an_end(core) {
        return;
    }
    for k in 0..3 {
        let Some(e) = weak_point_entry(core, k) else { continue };
        let hp = core.raw_read_8(e + 4, -1);
        let kept = core.raw_read_8(MTE_HP + k as u32, -1);
        if crystal_alive(core, k) {
            if kept == 0 || hp > kept {
                core.raw_write_8(MTE_HP + k as u32, -1, hp);
            } else if hp < kept {
                core.raw_write_8(e + 4, -1, kept);
            }
        }
    }
}

/// A mission's start: the mission's state (Means to an End's, Ring of
/// Fire's) cleared.
pub fn mte_start(core: &mut Core) {
    for k in 0..MISSION_STATE.1 {
        core.raw_write_8(MISSION_STATE.0 + k, -1, 0);
    }
}

/// AW2's invention kind of the Volcano (the inventions list).
const KIND_VOLCANO: u16 = 2;

/// The invention-list entry `entry` is a Volcano that Dual Strike's rule
/// stilled (crate::obelisk's turn-start trap then skips it): only in a DS
/// Campaign battle.
pub fn volcano_still(core: &Core, entry: u32) -> bool {
    crate::ds_campaign::active(core)
        && crate::ds_campaign::in_battle(core)
        && core.raw_read_8(VOLCANO_STILL, -1) != 0
        && (core.raw_read_16(entry + 2, -1) >> 6) & 0xF == KIND_VOLCANO
}

/// The Volcano's eruption (`sub_0803E764(targets, damage)`, called from
/// the turn-start loop with AW2's own mission's cells, `[0x0849F728 +
/// 4 * (day & 1)]`): in a DS Campaign battle its cells are Dual Strike's
/// for the Volcano (the ARM9 image's lists by the volcano's owner,
/// `0x02167E98`; the main map's is list 1, twelve cells round the map's
/// edge), written as AW2's list (x, y halfwords, 0xFFFF ends) to
/// [`ERUPTION_CELLS`].
pub const ERUPTION_CALL: u32 = 0x0803_EE3C;
const DS_ERUPTION_LISTS: u32 = 0x0216_7E98;
const ERUPTION_CELLS: u32 = 0x0203_F710;
pub fn eruption(core: &mut Core) {
    if !(crate::ds_campaign::active(core) && crate::ds_campaign::in_battle(core)) {
        return;
    }
    let Some(pack) = crate::ds_pack::pack() else { return };
    let Some(list) = pack.arm9_at(DS_ERUPTION_LISTS + 4, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) else { return };
    let mut at = ERUPTION_CELLS;
    for k in 0..12u32 {
        let Some(c) = pack.arm9_at(list + 4 * k, 4) else { break };
        let (x, y) = (u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]]));
        if x == 0xFFFF {
            break;
        }
        core.raw_write_16(at, -1, x);
        core.raw_write_16(at + 2, -1, y);
        at += 4;
    }
    core.raw_write_16(at, -1, 0xFFFF);
    core.raw_write_16(at + 2, -1, 0);
    core.gba_mut().cpu_mut().set_gpr(0, ERUPTION_CELLS as i32);
}

/// Victory or Death!'s Black Arc bomb (Dual Strike's `0x020EEE1C(13, 5,
/// 100, 0)`, each of Black Hole's turns while its rule holds): every unit
/// within 2 spaces of (13, 5), but those of army 2's team, Ooziums and
/// loaded units, is left with 1 HP (100 damage, never destroying).
const BLACK_ARC: (i32, i32) = (13, 5);
fn black_arc(core: &mut Core) {
    let p = players(core);
    let bh_team = core.raw_read_8(p + PLAYER * 2 + 0x2A, -1);
    for army in 1..=4u32 {
        if core.raw_read_8(p + PLAYER * army + 0x2A, -1) == bh_team {
            continue;
        }
        for u in units(core, army) {
            if u.1 == crate::roster::OOZIUM || u.2 & 0x08 != 0 {
                continue;
            }
            if (u.3 as i32 - BLACK_ARC.0).abs() + (u.4 as i32 - BLACK_ARC.1).abs() > 2 {
                continue;
            }
            let hp = core.raw_read_8(u.0 + 4, -1);
            let left = (hp & 0x7F).saturating_sub(100).max(1);
            core.raw_write_8(u.0 + 4, -1, (hp & 0x80) | left);
        }
    }
}

/// `GetInventionAt(x, y)` (0x0803DE94) asked where a unit picks a target
/// (the cursor's cell, `0x0802B3DC`; the targets around a unit,
/// `0x0802E2EA`): a weak point whose crystal stands is no target.
pub const GET_INVENTION_AT: u32 = 0x0803_DE94;
const TARGET_CALLERS: [u32; 2] = [0x0802_B3E1, 0x0802_E2EF];
pub fn invention_at(core: &mut Core) {
    if !means_to_an_end(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (x, y, lr) = (cpu.gpr(0) as u32 & 0xFFFF, cpu.gpr(1) as u32 & 0xFFFF, cpu.gpr(14) as u32);
    if !TARGET_CALLERS.contains(&lr) {
        return;
    }
    let closed = (0..3).any(|k| {
        let (wx, wy) = GRAND_BOLT_WEAK_POINTS[k];
        crystal_alive(core, k) && (wx - 1..=wx + 1).contains(&x) && (wy - 2..=wy).contains(&y)
    });
    if closed {
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, 0);
        cpu.set_thumb_pc(lr & !1);
    }
}

fn weak_point_spawns(core: &Core, k: usize, army: u32) -> bool {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    let id = unit_id_at(core, x, y + 1) as u32;
    weak_point_alive(core, k) && (id == 0 || id / 64 + 1 != army) && units(core, army.clamp(1, 4)).len() < 50
}

/// Destroys the unit on a cell (and what it carries), as Dual Strike's
/// `0x020EDB84` does before an Oozium spawns there.
fn destroy_unit_at(core: &mut Core, x: u32, y: u32) {
    let id = unit_id_at(core, x, y) as u32;
    if id == 0 {
        return;
    }
    let base = core.raw_read_32(UNITS_PTR, -1);
    let a = base + UNIT * id;
    for cargo in [core.raw_read_8(a + 7, -1), core.raw_read_8(a + 8, -1)] {
        if cargo != 0 {
            core.raw_write_8(base + UNIT * cargo as u32, -1, 0);
        }
    }
    core.raw_write_8(a, -1, 0);
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_write_8(MAP + 0x12 + row + x, -1, 0);
}

/// `CreateUnitAt(x, y, type)`: a unit of the army moving now, the map's
/// unit layers rebuilt.
const CREATE_UNIT_AT: u32 = 0x0802_5CC8;

fn local_flag(core: &Core, id: u32) -> bool {
    id < 0x20 && core.raw_read_8(LOCAL_FLAGS + id / 8, -1) & (1 << (id % 8)) != 0
}

fn set_weather(core: &mut Core, w: u8) {
    core.raw_write_8(WEATHER, -1, w);
    core.raw_write_8(NEXT_WEATHER, -1, w);
}

/// Dual Strike's script-called functions by address. Unknown ones do
/// nothing. True when control has passed to a game function (which
/// returns to the script engine itself).
fn call(core: &mut Core, f: u32, arg: u32) -> bool {
    let _ = arg;
    match f {
        // Means to an End: a weak point destroys the unit below it and
        // spawns an Oozium there (Dual Strike's 0x020EDB84, then its
        // 0x022AE4A8 animation and 0x020C7D30 unit).
        0x0235_1F34 => destroy_below(core, 0),
        0x0235_1EB8 => destroy_below(core, 1),
        0x0235_1E3C => destroy_below(core, 2),
        0x0235_2018 => return spawn_oozium(core, 0),
        0x0235_1FE4 => return spawn_oozium(core, 1),
        0x0235_1FB0 => return spawn_oozium(core, 2),
        // A research lab's map was found (the mission's flag): its side
        // mission opens (campaign flags 0x90..0x92: AW2 reads 0x60 as Hard
        // Campaign, crate::ds_campaign_data::ds_flag).
        0x0235_0E34 if local_flag(core, 0) => crate::ds_campaign::set_campaign_flag(core, 0x90),
        0x0235_0FB8 if local_flag(core, 1) => crate::ds_campaign::set_campaign_flag(core, 0x91),
        0x0235_10C4 if local_flag(core, 0) => crate::ds_campaign::set_campaign_flag(core, 0x92),
        // The weather: Dual Strike's 0x020D207C(0 clear, 1 snow, 2 rain).
        0x0235_0FA8 => set_weather(core, 1),
        0x0235_16B8 | 0x0235_17C4 => set_weather(core, 0),
        0x0235_17F4 => set_weather(core, 2),
        // Victory or Death!: the Black Arc's bomb.
        0x0235_0D44 => black_arc(core),
        // Ring of Fire: the volcano's controller found in the city (all four
        // of Black Hole's cities taken), or the second front's won: the
        // Volcano erupts no more.
        0x0235_1988 | 0x0235_18CC => core.raw_write_8(VOLCANO_STILL, -1, 1),
        // Means to an End's choice (Dual Strike's `0x021017F4(0x3C, v)`):
        // campaign flag 0x3C cleared or set.
        0x0235_1D28 => crate::ds_campaign::clear_campaign_flag(core, 0x3C),
        0x0235_1D3C => crate::ds_campaign::set_campaign_flag(core, 0x3C),
        // Presentation only (camera pans, sounds, flashes, fades, the
        // eruption's opening scene, the skip handler): nothing on the
        // map's state. Muck Amok!'s rout of army 3 on its HQ's capture
        // (0x023516A4) is AW2's own HQ capture.
        _ => {}
    }
    false
}

fn destroy_below(core: &mut Core, k: usize) {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    destroy_unit_at(core, x, y + 1);
}

fn spawn_oozium(core: &mut Core, k: usize) -> bool {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    if unit_id_at(core, x, y + 1) != 0 {
        return false;
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, x as i32);
    cpu.set_gpr(1, (y + 1) as i32);
    cpu.set_gpr(2, crate::roster::OOZIUM as i32);
    cpu.set_thumb_pc(CREATE_UNIT_AT);
    true
}
