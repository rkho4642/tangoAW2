//! tangoAW2's Versus maps (five/maps.txt): their tiles, units and names in
//! the ROM image's free space, through map-table entries the game never
//! uses ([`IDS`]). The 5-army ones are listed on a new Versus tab, "5P
//! Maps"; the 2-, 3- and 4-army obelisk maps on the game's own tabs. Also
//! Black Hole's own property tiles
//! (0x1B4..0x1B9), which army 5 owns: they look like the other property
//! tiles (the buildings are sprites drawn over plain grass).

use mgba::core::Core;

use crate::five_map_data::MAPS;

const MAP_TABLE: u32 = 0x085C_77A0;
/// Each map's tiles and units: 4 KiB apiece from here.
const MAP_DATA: u32 = 0x0862_2000;
const MAP_DATA_SIZE: u32 = 0x1000;

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
/// Each map's id, in five/maps.txt's order: map-table entry 0 (a dummy the
/// game never lists) and design-map ids 0xB8..0xBF (the Design Room has three
/// slots, 0xB4..0xB6, and a suspend copy, 0xB7; the rest serve only
/// multi-cartridge link play).
pub const IDS: [u8; 9] = [0, 0xBC, 0xBD, 0xBE, 0xBF, 0xB8, 0xB9, 0xBA, 0xBB];

/// Map ids 0xB4..0xBF are design maps to the game. These make 0xB8..0xBF
/// ordinary maps (header blob, name, unit list, preview): (address,
/// original, patched).
const ORDINARY_IDS: &[(u32, u16, u16)] = &[
    // (u8)(id + 0x4C) <= 0xB, the design range 0xB4..0xBF -> 0xB4..0xB7
    (0x0801_6D6E, 0x280B, 0x2803), // suspend save
    (0x0801_6DDA, 0x280B, 0x2803), // suspend load
    (0x0801_701C, 0x280B, 0x2803), // suspend tile changes
    (0x0801_733C, 0x280B, 0x2803), // resume
    (0x0801_73E6, 0x280B, 0x2803), // resume: changed tiles
    (0x0802_164C, 0x280B, 0x2803), // map setup
    (0x0802_1828, 0x280B, 0x2803), // property census
    (0x0802_47B0, 0x280B, 0x2803), // LoadMapData
    (0x0802_4918, 0x280B, 0x2803), // army count
    (0x0802_4950, 0x280B, 0x2803), // name
    (0x0803_C158, 0x280B, 0x2803), // Teams colours
    // the list filter rejects 0xB7..0xBF; now 0xB7 only
    (0x0803_7424, 0x2808, 0x2800),
    // the map list's preview tests `id <= 0xB3`: a helper in dead code
    // (sub_0803CC3C, no callers) that answers as for an ordinary id for
    // 0xB8..0xBF. Entries per register the id is loaded from.
    (0x0803_CC3C, 0xB500, 0x7808), // entry_r1: ldrb r0,[r1]
    (0x0803_CC3E, 0x0400, 0xE004), //   b 0x0803CC4A
    (0x0803_CC40, 0x0C00, 0x7828), // entry_r5: ldrb r0,[r5]
    (0x0803_CC42, 0x1C01, 0xE002), //   b 0x0803CC4A
    (0x0803_CC44, 0x39B4, 0x7880), // entry_link: ldrb r0,[r0,#2]
    (0x0803_CC46, 0x0408, 0xE000), //   b 0x0803CC4A
    (0x0803_CC48, 0x0C00, 0x7820), // entry_r4: ldrb r0,[r4]
    (0x0803_CC4A, 0x280B, 0x28B7), // cmp r0,#0xb7
    (0x0803_CC4C, 0xD807, 0xD801), // bhi (a new id)
    (0x0803_CC4E, 0x1C08, 0x28B3), // cmp r0,#0xb3
    (0x0803_CC50, 0x2103, 0x4770), // bx lr
    (0x0803_CC52, 0xF04D, 0x4280), // new id: cmp r0,r0 (as for id <= 0xB3)
    (0x0803_CC54, 0xFF2B, 0x4770), // bx lr
    // callers: `ldrb r0,[rX]; cmp r0,#0xb3` -> `bl entry`
    (0x0808_6EBE, 0x7820, 0xF7B5),
    (0x0808_6EC0, 0x28B3, 0xFEC3),
    (0x0808_6EF8, 0x7820, 0xF7B5),
    (0x0808_6EFA, 0x28B3, 0xFEA6),
    (0x0808_6F4A, 0x7820, 0xF7B5),
    (0x0808_6F4C, 0x28B3, 0xFE7D),
    (0x0808_6FC0, 0x7820, 0xF7B5),
    (0x0808_6FC2, 0x28B3, 0xFE42),
    (0x0808_7110, 0x7808, 0xF7B5),
    (0x0808_7112, 0x28B3, 0xFD94),
];
/// Tab titles, a text id per tab.
const TAB_TITLES: u32 = 0x0849_9CE4;
/// The text table has no room to grow, but ids are 16 bits and the table is
/// read without a bound: id 0x3D72 + k reads its string pointer from
/// 0x08620000 + 4k, in the ROM image's free space. 0x3D72 is the tab's
/// title, 0x3D73.. the maps' names; the strings follow from 0x08620100.
const TEXT_TABLE: u32 = 0x0861_0A38;
const TAB_TEXT: u16 = 0x3D72;
const STRINGS: u32 = 0x0862_0100;
pub const TAB_NAME_TEXT: &str = "5P Maps";

/// A 5-army map's id.
pub fn is_five_map(id: u8) -> bool {
    MAPS.iter().zip(IDS.iter()).any(|(m, &i)| i == id && m.armies == 5)
}

pub fn install(core: &mut Core) {
    let mut names = vec![TAB_NAME_TEXT];
    names.extend(MAPS.iter().map(|m| m.name));
    for (k, text) in names.iter().enumerate() {
        let at = STRINGS + 0x20 * k as u32;
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        assert!(bytes.len() <= 0x20);
        core.raw_write_range(at, -1, &bytes);
        core.raw_write_32(TEXT_TABLE + 4 * (TAB_TEXT as u32 + k as u32), -1, at);
    }
    core.raw_write_16(TAB_TITLES + 2 * CATEGORY as u32, -1, TAB_TEXT);
    for &(addr, old, new) in TAB_BOUNDS.iter().chain(ORDINARY_IDS) {
        if core.raw_read_16(addr, -1) == old {
            core.raw_write_16(addr, -1, new);
        }
    }
    assert_eq!(MAPS.len(), IDS.len());
    for (k, (map, &id)) in MAPS.iter().zip(IDS.iter()).enumerate() {
        let tiles = MAP_DATA + MAP_DATA_SIZE * k as u32;
        let units = tiles + map.tiles.len() as u32;
        assert!(map.tiles.len() + map.units.len() <= MAP_DATA_SIZE as usize);
        core.raw_write_range(tiles, -1, map.tiles);
        core.raw_write_range(units, -1, map.units);

        let mut h = [0u8; 0x5C];
        let w32 = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let w16 = |h: &mut [u8], at: usize, v: u16| h[at..at + 2].copy_from_slice(&v.to_le_bytes());
        w32(&mut h, 0x00, tiles);
        w16(&mut h, 0x14, TAB_TEXT + 1 + k as u16);
        h[0x16] = 2; // pre-deployed art
        // armies in the header (a 5-army map says 4: army 5 is tangoAW2's)
        h[0x18] = map.armies.min(4);
        w16(&mut h, 0x1A, map.tab);
        w16(&mut h, 0x1C, 1);
        w16(&mut h, 0x1E, 1);
        w16(&mut h, 0x20, 0x16);
        h[0x26] = 0xFF;
        w32(&mut h, 0x2C, tiles);
        w32(&mut h, 0x34, units);
        for (i, &c) in map.colours.iter().take(4).enumerate() {
            h[0x40 + i] = c;
        }
        h[0x44..0x48].copy_from_slice(&[1, 2, 3, 4]);
        for i in 0..4 {
            h[0x48 + 4 * i..0x4C + 4 * i].copy_from_slice(&[0xFF, 0xFF, 0, 0]);
        }
        core.raw_write_range(MAP_TABLE + 0x5C * id as u32, -1, &h);
    }

    let mut quad = [0u8; 8];
    core.raw_read_range(METATILES + NEUTRAL_HQ_TILE * 8, -1, &mut quad);
    for (tile, terrain) in ARMY5_TILES {
        core.raw_write_8(TERRAIN_TABLE + tile, -1, terrain);
        core.raw_write_range(METATILES + tile * 8, -1, &quad);
    }
}
