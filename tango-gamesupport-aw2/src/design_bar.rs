//! Black Hole's inventions as entries of the Design Room's terrain bar,
//! each with its own icon and name, picked like Sea or City.
//!
//! The terrain bar is built from a template into a 17-entry list at
//! `0x0200B224` (`sub_080078E4`), and the editor's code wraps its indices at
//! 17. tangoAW2 moves the list to unused EWRAM with room for 27 entries,
//! patches the editor's 17s (and 16s) to 27 (and 26) in the ROM image in
//! memory (the .gba file is untouched), and inserts the ten inventions after
//! the Silo each time the list is built. Their icons are the game's own
//! terrain-panel pictures for those terrain types, loaded when the bar asks
//! for them; the game already has their names (Mini, Laser, Cannon, Volcano,
//! Factory, Deathray). The patches and traps run inside the emulated frame,
//! the same on both netplay peers.

use mgba::core::Core;

/// Where the bar's list lives now: 27 entries of (word, tile).
pub const LIST: u32 = 0x0203_FF80;
pub const ENTRIES: u32 = 27;
const OLD_LIST: u32 = 0x0200_B224;
const OLD_ENTRIES: u32 = 17;

/// Every literal-pool word in the editor pointing at the old list.
const LIST_POINTERS: [u32; 9] = [
    0x0800_1CFC,
    0x0800_1D58,
    0x0800_1D88,
    0x0800_62B8,
    0x0800_6340,
    0x0800_7750,
    0x0800_7844,
    0x0800_78D0,
    0x0800_7918,
];

/// Thumb instructions carrying the terrain list's length (17), its last
/// index (16) or its size in bytes (0x44); the immediate is the low byte.
const LENGTH_SITES: [(u32, u8, u8); 11] = [
    (0x0800_0CEA, 0x11, ENTRIES as u8),       // SetSelectedTile: += 17
    (0x0800_1D4E, 0x10, ENTRIES as u8 - 1),   // list search: i <= 16
    (0x0800_626E, 0x10, ENTRIES as u8 - 1),   // RIGHT: slot > 16
    (0x0800_6272, 0x11, ENTRIES as u8),       //        slot -= 17
    (0x0800_62E6, 0x11, ENTRIES as u8),       // LEFT:  slot += 17
    (0x0800_6460, 0x10, ENTRIES as u8 - 1),   // index > 16
    (0x0800_6468, 0x11, ENTRIES as u8),       // index -= 17
    (0x0800_6562, 0x11, ENTRIES as u8),       // index += 17
    (0x0800_7798, 0x10, ENTRIES as u8 - 1),   // ring rebuild: list > 16
    (0x0800_779C, 0x44, (ENTRIES * 4) as u8), //   pointer -= 17 * 4
    (0x0800_779E, 0x11, ENTRIES as u8),       //   list -= 17
];

/// The ten inventions' bar entries: (terrain type, tile placed), in
/// `design::INVENTIONS` order.
pub const ENTRIES_ADDED: [(u16, u16); 10] = [
    (0x15, 0x182), // minicannon facing down
    (0x16, 0x183), // up
    (0x17, 0x184), // left
    (0x18, 0x185), // right
    (0x19, 0x181), // laser
    (0x1A, 0x187), // Black Cannon facing down
    (0x1B, 0x18A), // up
    (0x1D, 0x18D), // Black Factory
    (0x1C, 0x1A7), // Volcano
    (0x1E, 0x190), // Deathray
];

pub fn is_invention_type(class: u8) -> bool {
    (0x15..=0x1E).contains(&class)
}

/// The invention a terrain type stands for, in `design::INVENTIONS` order.
pub fn invention_of(class: u8) -> Option<usize> {
    ENTRIES_ADDED.iter().position(|&(c, _)| c as u8 == class)
}

/// Every frame: keep the ROM image patched (idempotent; applied from the
/// first frame, so both netplay peers run the same code).
pub fn patch_rom(core: &mut Core) {
    for p in LIST_POINTERS {
        if core.raw_read_32(p, -1) == OLD_LIST {
            core.raw_write_32(p, -1, LIST);
        }
    }
    for (at, from, to) in LENGTH_SITES {
        let op = core.raw_read_16(at, -1);
        if op as u8 == from {
            core.raw_write_16(at, -1, (op & 0xFF00) | to as u16);
        }
    }
}

/// The end of the list builder (`sub_080078E4`), before it returns.
pub const LIST_BUILT: u32 = 0x0800_79B2;
const EDITOR_BAR: u32 = 0x0200_B007;
const SILO: u16 = 0x11;

/// Trap at [`LIST_BUILT`]: the terrain list is 17 entries at [`LIST`];
/// insert the inventions after the Silo.
pub fn list_built(core: &mut Core) {
    if !crate::design::in_map_editor(core) || core.raw_read_8(EDITOR_BAR, -1) != 0 {
        return;
    }
    let mut list: Vec<(u16, u16)> = (0..OLD_ENTRIES)
        .map(|i| {
            (
                core.raw_read_16(LIST + 4 * i, -1),
                core.raw_read_16(LIST + 4 * i + 2, -1),
            )
        })
        .collect();
    // Built already (the trap sees each build once, but be safe).
    if list.iter().any(|&(w, _)| is_invention_type(w as u8 & 0x1F)) {
        return;
    }
    let at = list
        .iter()
        .position(|&(w, _)| w & 0x1F == SILO)
        .map_or(list.len(), |i| i + 1);
    for (k, e) in ENTRIES_ADDED.iter().enumerate() {
        list.insert(at + k, *e);
    }
    for (i, (w, t)) in list.iter().enumerate() {
        core.raw_write_16(LIST + 4 * i as u32, -1, *w);
        core.raw_write_16(LIST + 4 * i as u32 + 2, -1, *t);
    }
}

/// The bar's sprite loader (`sub_0803F6BC(kind, variant, dest, load)`).
/// For a type it has no case for, it loads that type's terrain-panel picture
/// (rows 2..7, drawn for panel palette entry N) over a shared base (drawn
/// for the Plain's panel palette), so the inventions get their own pictures;
/// only the colours need mapping to the palette they are drawn with here.
pub const ICON_LOADER: u32 = 0x0803_F6BC;
/// Its exit, where r4 is still the kind and r5 the destination.
pub const ICON_LOADED: u32 = 0x0803_F7FE;
/// Set while an invention icon is being loaded (entry to exit).
const ICON_PENDING: u32 = 0x0203_FFFE;

/// Trap at [`ICON_LOADER`]: note whether this call loads an invention's icon.
pub fn icon_loader(core: &mut Core) {
    if !crate::design::in_map_editor(core) {
        core.raw_write_8(ICON_PENDING, -1, 0);
        return;
    }
    let cpu = core.gba().cpu();
    let (kind, load) = (cpu.gpr(0) as u32 & 0x1F, cpu.gpr(3));
    let pending = is_invention_type(kind as u8) && load != 0;
    core.raw_write_8(ICON_PENDING, -1, pending as u8);
}

/// Trap at [`ICON_LOADED`]: map the loaded icon's colours.
pub fn icon_loaded(core: &mut Core) {
    if core.raw_read_8(ICON_PENDING, -1) == 0 {
        return;
    }
    core.raw_write_8(ICON_PENDING, -1, 0);
    let cpu = core.gba().cpu();
    let (kind, dest) = (cpu.gpr(4) as u32, cpu.gpr(5) as u32);
    if !is_invention_type(kind as u8) {
        return;
    }
    let mut tiles = [0u8; 0x100];
    core.raw_read_range(dest, -1, &mut tiles);
    let target = if kind == VOLCANO_TYPE {
        PANEL_PALETTES + VOLCANO_PANEL_PALETTE * 32
    } else {
        BLACK_HOLE_PALETTE
    };
    let picture = if kind == VOLCANO_TYPE {
        VOLCANO_PANEL_PALETTE
    } else {
        INVENTION_PANEL_PALETTE
    };
    remap(core, &mut tiles[..0xC0], PANEL_PALETTES + picture * 32, target);
    remap(
        core,
        &mut tiles[0xC0..],
        PANEL_PALETTES + PLAIN_PANEL_PALETTE * 32,
        target,
    );
    core.raw_write_range(dest, -1, &tiles);
}

const VOLCANO_TYPE: u32 = 0x1C;
/// The panel palettes (`GetTerrainNamePalette`, entry per terrain type at
/// `0x0849A2C8`): inventions 13 (the Volcano 0, the mountains'), the shared
/// base under every icon was drawn for the Plain's, 2.
const PANEL_PALETTES: u32 = 0x0810_6864;
const INVENTION_PANEL_PALETTE: u32 = 13;
const VOLCANO_PANEL_PALETTE: u32 = 0;
const PLAIN_PANEL_PALETTE: u32 = 2;
const BLACK_HOLE_PALETTE: u32 = 0x080D_3E84;

/// Map each pixel's colour index from palette `from` to the index of the
/// nearest colour in palette `to` (index 0 stays transparent).
fn remap(core: &Core, tiles: &mut [u8], from: u32, to: u32) {
    let read = |addr: u32| -> [u16; 16] {
        let mut p = [0u16; 16];
        for (i, c) in p.iter_mut().enumerate() {
            *c = core.raw_read_16(addr + 2 * i as u32, -1);
        }
        p
    };
    let (from, to) = (read(from), read(to));
    let dist = |a: u16, b: u16| {
        let ch = |c: u16, s: u16| ((c >> s) & 31) as i32;
        (0..3).map(|k| (ch(a, k * 5) - ch(b, k * 5)).pow(2)).sum::<i32>()
    };
    let mut map = [0u8; 16];
    for (i, m) in map.iter_mut().enumerate().skip(1) {
        *m = (1..16).min_by_key(|&j| dist(from[i], to[j])).unwrap() as u8;
    }
    for b in tiles.iter_mut() {
        *b = map[(*b & 15) as usize] | (map[(*b >> 4) as usize] << 4);
    }
}

/// The end of the icon palette lookup (`sub_08001D04(item)`; r3 = item,
/// r0 = palette), at its return.
pub const ICON_PALETTE: u32 = 0x0800_1D20;
/// The editor's mountain palette (the Volcano picture's).
const MOUNTAIN_PALETTE: i32 = 7;

/// Trap at [`ICON_PALETTE`]: invention icons use the palette their
/// pictures were mapped to.
pub fn icon_palette(core: &mut Core) {
    if !crate::design::in_map_editor(core) {
        return;
    }
    let item = core.gba().cpu().gpr(3) as u32;
    if item > 0x1F || !is_invention_type(item as u8) {
        return;
    }
    let palette = if item == VOLCANO_TYPE {
        MOUNTAIN_PALETTE
    } else {
        crate::invention_art::black_hole_palette(core) as i32
    };
    core.gba_mut().cpu_mut().set_gpr(0, palette);
}
