//! Black Hole's Black Crystal and Black Obelisk, after Advance Wars: Dual
//! Strike: structures that heal and resupply Black Hole's units at the start
//! of its turn (Crystal: units within 2 spaces, +2 HP; Obelisk: within 4
//! spaces of it, +4 HP), and that can be attacked and destroyed.
//!
//! They ride on two of AW2's own inventions, so the game registers, targets
//! and destroys them: the Crystal is a minicannon (1 tile) and the Obelisk a
//! Black Cannon (3x3), each on its own map tile (0x192, 0x193). tangoAW2
//! tells them apart by that tile, stops them firing, draws them with their
//! own art (five/obelisk_art.py), names them in the terrain panel, and heals.
//! Real minicannons and Black Cannons keep their own tiles and are untouched.

use mgba::core::Core;

use crate::obelisk_art::{CRYSTAL_NAME, OBELISK_NAME};

pub const CRYSTAL_TILE: u16 = 0x192;
pub const OBELISK_TILE: u16 = 0x193;
/// Their terrain classes: minicannon facing down, Black Cannon facing down.
const CRYSTAL_CLASS: u8 = 0x15;
const OBELISK_CLASS: u8 = 0x1A;
/// Invention kinds in the list: minicannon 4, Black Cannon 3.
const KIND_MINICANNON: u16 = 4;
const KIND_CANNON: u16 = 3;

const TERRAIN_TABLE: u32 = 0x080C_1BC4;
const TERRAIN_RAM: u32 = 0x0202_33B0;
const METATILES: u32 = 0x080B_FBC4;
const PLAIN_QUAD: [u16; 4] = [0x2002, 0x2003, 0x2022, 0x2023];

const MAP: u32 = 0x0201_E450;
const INVENTIONS: u32 = 0x0202_8360;
const INVENTION_COUNT: u32 = 16;
const CURRENT_ARMY: u32 = 0x0300_33EC;
const UNITS: u32 = 0x0202_2684;
const UNIT_TYPES: u32 = 0x085D_5ABC;

/// tangoAW2's data in the ROM image's free space.
const DATA: u32 = 0x0864_0000;
const OBELISK_DEF: u32 = DATA;
const CRYSTAL_DEF: u32 = DATA + 0x20;
pub const CRYSTAL_NAME_AT: u32 = DATA + 0x100;
pub const OBELISK_NAME_AT: u32 = DATA + 0x200;
const CRYSTAL_PICTURE_AT: u32 = DATA + 0x300;
const OBELISK_PICTURE_AT: u32 = DATA + 0x400;
const DATA_SENTINEL: u32 = DATA + 0xFFC;
const DATA_MAGIC: u32 = 0x334B_4C42; // "BLK3" (bump when the data changes)

/// OBJ tiles for the sprites in battle (no screen of the battle map writes
/// 0x176..0x1A5): the Obelisk's 36 tiles, then the Crystal's 8.
const OBELISK_OBJ_TILE: u32 = 0x176;
const CRYSTAL_OBJ_TILE: u32 = 0x19A;
/// Which structure's name the terrain panel is showing (1 Crystal, 2 Obelisk).
const PANEL: u32 = 0x0203_0207;

pub fn install(core: &mut Core) {
    // Every frame (cheap): the tiles' classes, in ROM and in the RAM copy the
    // game makes of the table.
    for (tile, class) in [(CRYSTAL_TILE, CRYSTAL_CLASS), (OBELISK_TILE, OBELISK_CLASS)] {
        core.raw_write_8(TERRAIN_TABLE + tile as u32, -1, class);
        core.raw_write_8(TERRAIN_RAM + tile as u32, -1, class);
    }
    if core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC {
        return;
    }
    for tile in [CRYSTAL_TILE, OBELISK_TILE] {
        for (i, q) in PLAIN_QUAD.iter().enumerate() {
            core.raw_write_16(METATILES + tile as u32 * 8 + 2 * i as u32, -1, *q);
        }
    }
    // Sprite definitions: count, then attr0/attr1/attr2 (tile relative to
    // 0x48, priority 3), drawn from the structure's top-left corner. The
    // Obelisk covers its 3x3 rect with four sprites (32x32, 16x32, 32x16,
    // 16x16, as the Black Cannon), the Crystal is one 16x32 a tile above
    // its cell. The same whichever art is loaded (`crate::ds_art`).
    let tile = |t: u32| 0x0C00 | (t - 0x48) as u16;
    let obelisk_def: &[u16] = &[
        0x0004,
        0x0000,
        0x8000,
        tile(OBELISK_OBJ_TILE),
        0x8000,
        0x8020,
        tile(OBELISK_OBJ_TILE + 16),
        0x4020,
        0x8000,
        tile(OBELISK_OBJ_TILE + 24),
        0x0020,
        0x4020,
        tile(OBELISK_OBJ_TILE + 32),
    ];
    let crystal_def: &[u16] = &[0x0001, 0x80F0, 0x8000, tile(CRYSTAL_OBJ_TILE)];
    for (at, def) in [(OBELISK_DEF, obelisk_def), (CRYSTAL_DEF, crystal_def)] {
        for (i, h) in def.iter().enumerate() {
            core.raw_write_16(at + 2 * i as u32, -1, *h);
        }
    }
    core.raw_write_range(CRYSTAL_NAME_AT, -1, &CRYSTAL_NAME);
    core.raw_write_range(OBELISK_NAME_AT, -1, &OBELISK_NAME);
    // Their pictures: Dual Strike's, imported; without it they are shown
    // only in someone else's replay, and then as nothing.
    let blank = [0u8; 256];
    let art = crate::ds_art::art();
    core.raw_write_range(CRYSTAL_PICTURE_AT, -1, art.map_or(&blank[..], |a| &a.crystal));
    core.raw_write_range(OBELISK_PICTURE_AT, -1, art.map_or(&blank[..], |a| &a.obelisk_small));
    core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
}

fn tile_at(core: &Core, x: u32, y: u32) -> u16 {
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    if x >= w || y >= h {
        return 0;
    }
    let row = core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32;
    core.raw_read_16(MAP + 0xA22 + 2 * (row + x), -1) & 0x3FF
}

#[derive(Clone, Copy, PartialEq)]
enum Structure {
    Crystal,
    Obelisk,
}

/// What an invention-list entry is, if it is one of ours.
fn structure(core: &Core, entry: u32) -> Option<Structure> {
    let (x, y) = (core.raw_read_8(entry, -1) as u32, core.raw_read_8(entry + 1, -1) as u32);
    let kind = (core.raw_read_16(entry + 2, -1) >> 6) & 0xF;
    match kind {
        KIND_MINICANNON if tile_at(core, x, y) == CRYSTAL_TILE => Some(Structure::Crystal),
        KIND_CANNON if tile_at(core, x + 1, y + 1) == OBELISK_TILE => Some(Structure::Obelisk),
        _ => None,
    }
}

/// Our structure covering map cell (x, y), if any.
fn structure_at(core: &Core, x: u32, y: u32) -> Option<Structure> {
    match tile_at(core, x, y) {
        CRYSTAL_TILE => return Some(Structure::Crystal),
        OBELISK_TILE => return Some(Structure::Obelisk),
        _ => {}
    }
    for i in 0..INVENTION_COUNT {
        let e = INVENTIONS + 8 * i;
        if (core.raw_read_16(e + 2, -1) >> 6) & 0xF == 0 {
            break;
        }
        if structure(core, e) == Some(Structure::Obelisk) {
            let (ex, ey) = (core.raw_read_8(e, -1) as u32, core.raw_read_8(e + 1, -1) as u32);
            if (ex..ex + 3).contains(&x) && (ey..ey + 3).contains(&y) {
                return Some(Structure::Obelisk);
            }
        }
    }
    None
}

// ---------- traps ----------

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (0x0803_F6A0, Box::new(load_tiles)),
        (0x0803_F908, Box::new(sprite)),
        (0x0803_ED7A, Box::new(no_fire)),
        (0x0803_EA06, Box::new(no_range)),
        (0x0803_EAD0, Box::new(heal)),
        (0x0802_A914, Box::new(panel_name)),
        (0x0802_A982, Box::new(panel_picture)),
    ]
}

/// After the building sheet loads (0x0803F6A0): our sprites' tiles.
fn load_tiles(core: &mut Core) {
    let blank = [0u8; 36 * 32];
    let art = crate::ds_art::art();
    core.raw_write_range(
        0x0601_0000 + OBELISK_OBJ_TILE * 32,
        -1,
        art.map_or(&blank[..], |a| &a.obelisk),
    );
    core.raw_write_range(
        0x0601_0000 + CRYSTAL_OBJ_TILE * 32,
        -1,
        art.map_or(&blank[..256], |a| &a.crystal),
    );
}

/// sub_0803F908(x, y, def, army, fog) puts a building's or invention's
/// sprite: ours get their own definitions (a destroyed Obelisk keeps the
/// Black Cannon's rubble).
fn sprite(core: &mut Core) {
    let cpu = core.gba().cpu();
    let (x, y, def, lr) = (
        cpu.gpr(0) as u32,
        cpu.gpr(1) as u32,
        cpu.gpr(2) as u32,
        cpu.gpr(14) as u32,
    );
    let new = match lr {
        0x0803_FB93 if tile_at(core, x, y) == CRYSTAL_TILE => CRYSTAL_DEF,
        0x0803_FD09 if matches!(def, 0x0849_FA22 | 0x0849_FA08) && tile_at(core, x + 1, y + 1) == OBELISK_TILE => {
            OBELISK_DEF
        }
        _ => return,
    };
    core.gba_mut().cpu_mut().set_gpr(2, new as i32);
}

/// The turn-start firing loop, per entry (r2): ours skip to the next.
fn no_fire(core: &mut Core) {
    let entry = core.gba().cpu().gpr(2) as u32;
    if structure(core, entry).is_some() {
        core.gba_mut().cpu_mut().set_thumb_pc(0x0803_EEAC);
    }
}

/// A on a structure shows its firing range (sub_0803E9F8, entry in r5):
/// ours have none.
fn no_range(core: &mut Core) {
    let entry = core.gba().cpu().gpr(5) as u32;
    if structure(core, entry).is_some() {
        core.gba_mut().cpu_mut().set_thumb_pc(0x0803_EAC4);
    }
}

/// Turn start (sub_0803EAD0, before the inventions act): if the army moving
/// now is Black Hole, heal and resupply its units near each structure.
fn heal(core: &mut Core) {
    let army = core.raw_read_16(CURRENT_ARMY, -1) as u32;
    let players = crate::five::players(core);
    if !(1..=5).contains(&army) || core.raw_read_8(players + 0x3C * army + 0x1A, -1) != 5 {
        return;
    }
    // (x0, y0, x1, y1, range, hp): the structure's cells and what it gives.
    let mut sources = Vec::new();
    for i in 0..INVENTION_COUNT {
        let e = INVENTIONS + 8 * i;
        if (core.raw_read_16(e + 2, -1) >> 6) & 0xF == 0 {
            break;
        }
        if core.raw_read_8(e + 4, -1) == 0 {
            continue;
        }
        let (x, y) = (core.raw_read_8(e, -1) as i32, core.raw_read_8(e + 1, -1) as i32);
        match structure(core, e) {
            Some(Structure::Crystal) => sources.push((x, y, x, y, 2, 20)),
            Some(Structure::Obelisk) => sources.push((x, y, x + 2, y + 2, 4, 40)),
            None => {}
        }
    }
    if sources.is_empty() {
        return;
    }
    let (first, per) = if crate::five::active(core) {
        ((army - 1) * 51, 51)
    } else {
        ((army - 1) * 64, 64)
    };
    for id in first + 1..first + per.min(51) {
        let u = UNITS + 12 * id;
        let kind = core.raw_read_8(u, -1) as u32;
        if kind == 0 {
            continue;
        }
        let (ux, uy) = (core.raw_read_8(u + 2, -1) as i32, core.raw_read_8(u + 3, -1) as i32);
        let heal = sources
            .iter()
            .filter(|&&(x0, y0, x1, y1, range, _)| {
                let dx = if ux < x0 {
                    x0 - ux
                } else if ux > x1 {
                    ux - x1
                } else {
                    0
                };
                let dy = if uy < y0 {
                    y0 - uy
                } else if uy > y1 {
                    uy - y1
                } else {
                    0
                };
                dx + dy <= range
            })
            .map(|s| s.5)
            .max();
        let Some(heal) = heal else { continue };
        let stats = UNIT_TYPES + kind * 0x5C;
        let max_ammo = core.raw_read_8(stats + 0x0B, -1) as u16 & 0xF;
        let max_fuel = core.raw_read_8(stats + 0x10, -1) & 0x7F;
        let w = core.raw_read_16(u + 4, -1);
        let hp = ((w & 0x7F) + heal).min(100);
        let w = (w & !0x7FF) | (max_ammo << 7) | hp;
        core.raw_write_16(u + 4, -1, w);
        let f = core.raw_read_8(u + 6, -1);
        core.raw_write_8(u + 6, -1, (f & 0x80) | max_fuel);
    }
}

/// The terrain panel (sub_0802A8DC; cell x = r8, y = r5): our structures'
/// names and pictures instead of "Cannon"'s.
fn panel_name(core: &mut Core) {
    let cpu = core.gba().cpu();
    let (x, y) = (cpu.gpr(8) as u32, cpu.gpr(5) as u32);
    let (which, name) = match structure_at(core, x, y) {
        Some(Structure::Crystal) => (1, CRYSTAL_NAME_AT),
        Some(Structure::Obelisk) => (2, OBELISK_NAME_AT),
        None => (0, 0),
    };
    core.raw_write_8(PANEL, -1, which);
    if which != 0 {
        core.gba_mut().cpu_mut().set_gpr(0, name as i32);
    }
}

fn panel_picture(core: &mut Core) {
    let picture = match core.raw_read_8(PANEL, -1) {
        1 => CRYSTAL_PICTURE_AT,
        2 => OBELISK_PICTURE_AT,
        _ => return,
    };
    core.gba_mut().cpu_mut().set_gpr(0, picture as i32);
}
