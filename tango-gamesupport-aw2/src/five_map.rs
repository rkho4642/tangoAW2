//! The 5-army Versus map ("Five Seas"): its tiles, units and name in the ROM
//! image's free space, listed on the Versus 4P tab through map-table entry 0
//! (a dummy the game never lists). Also Black Hole's own property tiles
//! (0x1B4..0x1B9), which army 5 owns: they look like the other property
//! tiles (the buildings are sprites drawn over plain grass).

use mgba::core::Core;

use crate::five_map_data::{TILES_LZ77, UNITS};

const MAP_TABLE: u32 = 0x085C_77A0;
const HEADER: u32 = MAP_TABLE + 0x5C * crate::five::MAP_ID as u32;
const TILES: u32 = crate::five::ROM_DATA + 0x100;
const UNIT_LIST: u32 = crate::five::ROM_DATA + 0x1000;
const NAME: u32 = crate::five::ROM_DATA + 0x1400;
pub const NAME_TEXT: &str = "Five Seas";

/// Tile -> terrain table (copied to RAM when a map loads) and metatiles.
const TERRAIN_TABLE: u32 = 0x080C_1BC4;
const METATILES: u32 = 0x080B_FBC4;
const NEUTRAL_HQ_TILE: u32 = 0x1C0;
/// Army 5's HQ, base, city, airport, port and lab.
const ARMY5_TILES: [(u32, u8); 6] = [(0x1B4, 0xA8), (0x1B5, 0xAE), (0x1B6, 0xA6), (0x1B7, 0xAA), (0x1B8, 0xAB), (0x1B9, 0xB4)];

/// The map name lookup (sub_08024944(mapID) -> string).
pub const MAP_NAME: u32 = 0x0802_4944;

pub fn install(core: &mut Core) {
    assert!(TILES as usize + TILES_LZ77.len() <= UNIT_LIST as usize);
    core.raw_write_range(TILES, -1, TILES_LZ77);
    core.raw_write_range(UNIT_LIST, -1, UNITS);
    let mut name = NAME_TEXT.as_bytes().to_vec();
    name.push(0);
    core.raw_write_range(NAME, -1, &name);

    let mut h = [0u8; 0x5C];
    let w32 = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let w16 = |h: &mut [u8], at: usize, v: u16| h[at..at + 2].copy_from_slice(&v.to_le_bytes());
    w32(&mut h, 0x00, TILES);
    let name_index = core.raw_read_16(MAP_TABLE + 0x5C + 0x14, -1);
    w16(&mut h, 0x14, name_index); // the lookup is trapped for this map
    h[0x16] = 2; // pre-deployed art
    h[0x18] = 4; // armies on the Teams screen (army 5 is tangoAW2's)
    w16(&mut h, 0x1A, 6); // the 4P tab
    w16(&mut h, 0x1C, 1);
    w16(&mut h, 0x1E, 1);
    w16(&mut h, 0x20, 0x16);
    h[0x26] = 0xFF;
    w32(&mut h, 0x2C, TILES);
    w32(&mut h, 0x34, UNIT_LIST);
    h[0x40..0x44].copy_from_slice(&[1, 2, 3, 4]);
    h[0x44..0x48].copy_from_slice(&[1, 2, 3, 4]);
    for i in 0..4 {
        h[0x48 + 4 * i..0x4C + 4 * i].copy_from_slice(&[0xFF, 0xFF, 0, 0]);
    }
    core.raw_write_range(HEADER, -1, &h);

    let mut quad = [0u8; 8];
    core.raw_read_range(METATILES + NEUTRAL_HQ_TILE * 8, -1, &mut quad);
    for (tile, terrain) in ARMY5_TILES {
        core.raw_write_8(TERRAIN_TABLE + tile, -1, terrain);
        core.raw_write_range(METATILES + tile * 8, -1, &quad);
    }
}

/// Trap at [`MAP_NAME`]: this map's name.
pub fn map_name(core: &mut Core) {
    let cpu = core.gba().cpu();
    if cpu.gpr(0) as u32 & 0xFFFF != crate::five::MAP_ID as u32 {
        return;
    }
    let lr = cpu.gpr(14) as u32;
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, NAME as i32);
    cpu.set_thumb_pc(lr & !1);
}
