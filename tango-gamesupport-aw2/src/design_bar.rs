//! Black Hole's inventions as entries of the Design Room's terrain bar,
//! each with its own icon and name, picked like Sea or City.
//!
//! The terrain bar is built from a template into a 17-entry list at
//! `0x0200B224` (`sub_080078E4`), and the editor's code wraps its indices at
//! 17. tangoAW2 moves the list to unused EWRAM with room for 27 entries,
//! patches the editor's 17s (and 16s) to 27 (and 26) in the ROM image in
//! memory (the .gba file is untouched), and inserts the inventions after
//! the Silo each time the list is built. Their icons are the game's own
//! terrain-panel pictures for those terrain types, loaded when the bar asks
//! for them; the game already has their names (Mini, Laser, Cannon, Volcano,
//! Factory, Deathray). The patches and traps run inside the emulated frame,
//! the same on both netplay peers.

use mgba::core::Core;

/// Where the bar's list lives now: room for 29 entries of (word, tile).
pub const LIST: u32 = 0x0203_FF00;
/// The bar's length: the game's 17 and the ten inventions, and the Black
/// Crystal and Black Obelisk when their art is there
/// ([`crate::ds_art::features`]).
const ENTRIES_BASE: u32 = 27;
const ENTRIES_ALL: u32 = 29;
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
/// index (16) or its size in bytes (0x44); the immediate is the low byte:
/// (address, kind) with kind 0 length, 1 last index, 4 size in bytes.
const LENGTH_SITES: [(u32, u32); 11] = [
    (0x0800_0CEA, 0), // SetSelectedTile: += 17
    (0x0800_1D4E, 1), // list search: i <= 16
    (0x0800_626E, 1), // RIGHT: slot > 16
    (0x0800_6272, 0), //        slot -= 17
    (0x0800_62E6, 0), // LEFT:  slot += 17
    (0x0800_6460, 1), // index > 16
    (0x0800_6468, 0), // index -= 17
    (0x0800_6562, 0), // index += 17
    (0x0800_7798, 1), // ring rebuild: list > 16
    (0x0800_779C, 4), //   pointer -= 17 * 4 (29 * 4 = 0x74 still fits)
    (0x0800_779E, 0), //   list -= 17
];

fn site_value(kind: u32, entries: u32) -> u8 {
    match kind {
        0 => entries as u8,
        1 => entries as u8 - 1,
        _ => (entries * 4) as u8,
    }
}

/// The bar's length as the editor's code has it now (27 or 29).
pub fn entries(core: &Core) -> u32 {
    match core.raw_read_16(LENGTH_SITES[0].0, -1) as u8 as u32 {
        n @ (ENTRIES_BASE | ENTRIES_ALL) => n,
        _ => ENTRIES_BASE,
    }
}

/// The inventions' bar entries: (terrain type, tile placed), in
/// `design::INVENTIONS` order. The Black Crystal and Black Obelisk are a
/// minicannon and a Black Cannon to the game (`crate::obelisk`); their words
/// carry bit 8 ([`OURS`]) so the bar can tell them from the real ones.
pub const ENTRIES_ADDED: [(u16, u16); 12] = [
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
    (OURS | 0x15, 0x192), // Black Crystal
    (OURS | 0x1A, 0x193), // Black Obelisk
];
pub const OURS: u16 = 0x100;

pub fn is_invention_type(class: u8) -> bool {
    (0x15..=0x1E).contains(&class)
}

/// The invention a bar word stands for, in `design::INVENTIONS` order.
pub fn invention_of(word: u16) -> Option<usize> {
    ENTRIES_ADDED.iter().position(|&(w, _)| w == word & (OURS | 0x1F))
}

/// Every frame: keep the ROM image patched (idempotent; applied from the
/// first frame, so both netplay peers run the same code), with the Crystal
/// and Obelisk in the bar when `with_obelisk`.
pub fn patch_rom(core: &mut Core, with_obelisk: bool) {
    for p in LIST_POINTERS {
        if core.raw_read_32(p, -1) == OLD_LIST {
            core.raw_write_32(p, -1, LIST);
        }
    }
    let n = if with_obelisk { ENTRIES_ALL } else { ENTRIES_BASE };
    for (at, kind) in LENGTH_SITES {
        let op = core.raw_read_16(at, -1);
        let known = [site_value(kind, OLD_ENTRIES), site_value(kind, ENTRIES_BASE), site_value(kind, ENTRIES_ALL)];
        let want = site_value(kind, n);
        if op as u8 != want && known.contains(&(op as u8)) {
            core.raw_write_16(at, -1, (op & 0xFF00) | want as u16);
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
    let added = entries(core) - OLD_ENTRIES;
    for (k, e) in ENTRIES_ADDED.iter().take(added as usize).enumerate() {
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
    // The bar's call (0x080027A6) still has the entry's whole word in r6.
    let word = if cpu.gpr(14) as u32 == BAR_ICON_RETURN {
        cpu.gpr(6) as u16 & (OURS | 0x1F)
    } else {
        kind as u16
    };
    let pending = match word {
        CRYSTAL_WORD => PENDING_CRYSTAL,
        OBELISK_WORD => PENDING_OBELISK,
        _ => (is_invention_type(kind as u8) && load != 0) as u8,
    };
    core.raw_write_8(ICON_PENDING, -1, pending);
}

const BAR_ICON_RETURN: u32 = 0x0800_27AD;
const CRYSTAL_WORD: u16 = OURS | 0x15;
const OBELISK_WORD: u16 = OURS | 0x1A;
const PENDING_CRYSTAL: u8 = 2;
const PENDING_OBELISK: u8 = 3;

/// Trap at [`ICON_LOADED`]: map the loaded icon's colours.
pub fn icon_loaded(core: &mut Core) {
    let pending = core.raw_read_8(ICON_PENDING, -1);
    if pending == 0 {
        return;
    }
    core.raw_write_8(ICON_PENDING, -1, 0);
    let cpu = core.gba().cpu();
    let (kind, dest) = (cpu.gpr(4) as u32, cpu.gpr(5) as u32);
    // The Crystal and Obelisk: tangoAW2's own pictures, drawn in Black
    // Hole's invention palette, which is what the bar shows them with.
    let art = crate::ds_art::art();
    let own: Option<&[u8]> = match pending {
        PENDING_CRYSTAL => Some(art.map_or(&[0u8; 256][..], |a| &a.crystal)),
        PENDING_OBELISK => Some(art.map_or(&[0u8; 256][..], |a| &a.obelisk_small)),
        _ => None,
    };
    if let Some(picture) = own {
        core.raw_write_range(dest, -1, picture);
        return;
    }
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
    // The Crystal's and Obelisk's words carry OURS.
    let item = core.gba().cpu().gpr(3) as u32 & !(OURS as u32);
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

/// Where the bar draws an entry's name (r0 = the name picture, 32x16, the
/// terrain panel's; r3 = the entry's word).
pub const BAR_NAME: u32 = 0x0800_2998;

/// Trap at [`BAR_NAME`]: the Crystal's and Obelisk's own names.
pub fn bar_name(core: &mut Core) {
    if !crate::design::in_map_editor(core) {
        return;
    }
    let word = core.gba().cpu().gpr(3) as u16 & (OURS | 0x1F);
    let name = match word {
        CRYSTAL_WORD => crate::obelisk::CRYSTAL_NAME_AT,
        OBELISK_WORD => crate::obelisk::OBELISK_NAME_AT,
        _ => return,
    };
    core.gba_mut().cpu_mut().set_gpr(0, name as i32);
}
