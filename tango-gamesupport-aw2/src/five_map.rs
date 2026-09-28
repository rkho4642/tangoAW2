//! The 5-army Versus map ("Five Seas"): its tiles, units and name in the ROM
//! image's free space, listed on a new Versus tab, "5P Maps", through
//! map-table entry 0 (a dummy the game never lists). Also Black Hole's own property tiles
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

/// The map list's tabs run 2..8 (Classic .. Design Maps); these raise the
/// last one to 9, the 5P tab: (address, original, patched).
const TAB_BOUNDS: [(u32, u16, u16); 6] = [
    (0x0808_5AFA, 0x3008, 0x3009), // init: clear the cursor memory of tabs 0..9
    (0x0808_5C52, 0x2008, 0x2009), // setup: skipping empty tabs wraps to 9
    (0x0808_64A4, 0x2008, 0x2009), // L/Left wraps 2 -> 9
    (0x0808_64D6, 0x2008, 0x2009),
    (0x0808_65A4, 0x2808, 0x2809), // R/Right reaches 9, then wraps to 2
    (0x0808_65D6, 0x2808, 0x2809),
];
pub const CATEGORY: u16 = 9;
/// Tab titles, a text id per tab.
const TAB_TITLES: u32 = 0x0849_9CE4;
/// The text table has no room to grow, but ids are 16 bits and the table is
/// read without a bound: ids 0x3D72 and 0x3D73 read their string pointers
/// from 0x08620000 and 0x08620004, in the ROM image's free space.
const TEXT_TABLE: u32 = 0x0861_0A38;
const TAB_TEXT: u16 = 0x3D72;
const NAME_TEXT_ID: u16 = 0x3D73;
const TAB_NAME: u32 = 0x0862_0010;
pub const TAB_NAME_TEXT: &str = "5P Maps";

pub fn install(core: &mut Core) {
    assert!(TILES as usize + TILES_LZ77.len() <= UNIT_LIST as usize);
    core.raw_write_range(TILES, -1, TILES_LZ77);
    core.raw_write_range(UNIT_LIST, -1, UNITS);
    for (id, at, text) in [(TAB_TEXT, TAB_NAME, TAB_NAME_TEXT), (NAME_TEXT_ID, NAME, NAME_TEXT)] {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        core.raw_write_range(at, -1, &bytes);
        core.raw_write_32(TEXT_TABLE + 4 * id as u32, -1, at);
    }
    core.raw_write_16(TAB_TITLES + 2 * CATEGORY as u32, -1, TAB_TEXT);
    for (addr, old, new) in TAB_BOUNDS {
        if core.raw_read_16(addr, -1) == old {
            core.raw_write_16(addr, -1, new);
        }
    }

    let mut h = [0u8; 0x5C];
    let w32 = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    let w16 = |h: &mut [u8], at: usize, v: u16| h[at..at + 2].copy_from_slice(&v.to_le_bytes());
    w32(&mut h, 0x00, TILES);
    w16(&mut h, 0x14, NAME_TEXT_ID);
    h[0x16] = 2; // pre-deployed art
    h[0x18] = 4; // armies on the Teams screen (army 5 is tangoAW2's)
    w16(&mut h, 0x1A, CATEGORY); // the 5P tab
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
