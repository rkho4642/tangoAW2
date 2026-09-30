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

/// Where the bar's list lives now: room for 31 entries of (word, tile).
pub const LIST: u32 = 0x0203_FF00;
/// The bar's length: the game's 17 and the ten inventions, and the Black
/// Crystal and Black Obelisk when their art is there
/// ([`crate::ds_art::features`]), and the Wasteland switch with the whole
/// Dual Strike pack ([`crate::ds_pack::features`]) the Wasteland switch
/// and the Com Tower. 31 entries end at 0x0203FF7C.
const ENTRIES_BASE: u32 = 27;
const ENTRIES_ALL: u32 = 29;
const ENTRIES_DS: u32 = 31;
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

/// The unit bar's: 20 units (with the eraser), 27 with Dual Strike's
/// (template [`crate::roster::UNIT_BAR`], read through one pool word).
const UNIT_SITES: [(u32, u32); 10] = [
    (0x0800_0D42, 0), // SetSelectedTile: += 20
    (0x0800_1D78, 1), // list search: i <= 19
    (0x0800_6282, 1), // RIGHT: slot > 19
    (0x0800_6286, 0), //        slot -= 20
    (0x0800_62FA, 0), // LEFT:  slot += 20
    (0x0800_648C, 1), // index > 19
    (0x0800_6494, 0), // index -= 20
    (0x0800_658A, 0), // index += 20
    (0x0800_77A2, 1), // ring rebuild: list > 19
    (0x0800_77A8, 0), //   list -= 20
];
const UNIT_BYTES_SITE: u32 = 0x0800_77A6; // pointer -= 20 * 4
const UNITS_BASE: u32 = 20;
const UNITS_DS: u32 = 27;
const UNIT_TEMPLATE_POINTER: u32 = 0x0800_7998;
const UNIT_TEMPLATE: u32 = 0x0848_8856;

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
        n @ (ENTRIES_BASE | ENTRIES_ALL | ENTRIES_DS) => n,
        _ => ENTRIES_BASE,
    }
}

/// The inventions' bar entries: (terrain type, tile placed), in
/// `design::INVENTIONS` order. The Black Crystal and Black Obelisk are a
/// minicannon and a Black Cannon to the game (`crate::obelisk`); their words
/// carry bit 8 ([`OURS`]) so the bar can tell them from the real ones. The
/// Wasteland entry, [`WASTE_WORD`], is not placed: it switches the map
/// between Normal and Wasteland ([`crate::wasteland`]); it shows the
/// mountain's icon. The last is the Com Tower ([`TOWER_WORD`]).
pub const ENTRIES_ADDED: [(u16, u16); 14] = [
    (0x15, 0x182),        // minicannon facing down
    (0x16, 0x183),        // up
    (0x17, 0x184),        // left
    (0x18, 0x185),        // right
    (0x19, 0x181),        // laser
    (0x1A, 0x187),        // Black Cannon facing down
    (0x1B, 0x18A),        // up
    (0x1D, 0x18D),        // Black Factory
    (0x1C, 0x1A7),        // Volcano
    (0x1E, 0x190),        // Deathray
    (OURS | 0x15, 0x192), // Black Crystal
    (OURS | 0x1A, 0x193), // Black Obelisk
    (WASTE_WORD, 0x020),  // Wasteland (shown as a mountain)
    (TOWER_WORD, 0x1D9),  // Com Tower (a Lab; its owner follows the bar's army)
];
/// The Com Tower's entry: a Lab ([`crate::com_tower`]), neutral here; the
/// editor's army choice is put in each frame ([`tower_owner`]).
pub const TOWER_WORD: u16 = crate::com_tower::LAB as u16;
pub const OURS: u16 = 0x100;
pub const WASTE_WORD: u16 = OURS | 0x03;

pub fn is_invention_type(class: u8) -> bool {
    (0x15..=0x1E).contains(&class)
}

/// The invention a bar word stands for, in `design::INVENTIONS` order.
pub fn invention_of(word: u16) -> Option<usize> {
    let word = word & (OURS | 0x1F);
    if word == WASTE_WORD || word == TOWER_WORD {
        return None;
    }
    ENTRIES_ADDED.iter().position(|&(w, _)| w == word)
}

/// Every frame: keep the ROM image patched (idempotent; applied from the
/// first frame, so both netplay peers run the same code), with the Crystal
/// and Obelisk in the bar when `with_obelisk`, and the Wasteland switch too
/// when `with_wasteland`.
pub fn patch_rom(core: &mut Core, with_obelisk: bool, with_wasteland: bool) {
    let units = if with_wasteland { UNITS_DS } else { UNITS_BASE };
    for (at, kind) in UNIT_SITES.iter().copied().chain([(UNIT_BYTES_SITE, 4)]) {
        let op = core.raw_read_16(at, -1);
        let known = [site_value(kind, UNITS_BASE), site_value(kind, UNITS_DS)];
        let want = site_value(kind, units);
        if op as u8 != want && known.contains(&(op as u8)) {
            core.raw_write_16(at, -1, (op & 0xFF00) | want as u16);
        }
    }
    let template = if with_wasteland { crate::roster::UNIT_BAR } else { UNIT_TEMPLATE };
    if core.raw_read_32(UNIT_TEMPLATE_POINTER, -1) != template {
        core.raw_write_32(UNIT_TEMPLATE_POINTER, -1, template);
    }
    for p in LIST_POINTERS {
        if core.raw_read_32(p, -1) == OLD_LIST {
            core.raw_write_32(p, -1, LIST);
        }
    }
    let n = match (with_obelisk, with_wasteland) {
        (true, true) => ENTRIES_DS,
        (true, false) => ENTRIES_ALL,
        _ => ENTRIES_BASE,
    };
    for (at, kind) in LENGTH_SITES {
        let op = core.raw_read_16(at, -1);
        let known = [
            site_value(kind, OLD_ENTRIES),
            site_value(kind, ENTRIES_BASE),
            site_value(kind, ENTRIES_ALL),
            site_value(kind, ENTRIES_DS),
        ];
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
    // The Com Tower is built for the bar's army (the builder's owner
    // argument, still in r5), like the editor's own properties.
    let owner = (core.gba().cpu().gpr(5) as u16).min(5);
    for e in list.iter_mut() {
        if e.0 == TOWER_WORD {
            *e = tower_entry(owner);
        }
    }
    for (i, (w, t)) in list.iter().enumerate() {
        core.raw_write_16(LIST + 4 * i as u32, -1, *w);
        core.raw_write_16(LIST + 4 * i as u32 + 2, -1, *t);
    }
}

/// The Com Tower's entry for `owner`.
fn tower_entry(owner: u16) -> (u16, u16) {
    (TOWER_WORD | owner << 5, crate::com_tower::tile_for(owner as u8))
}

/// `sub_0800C7E8(class)`, the editor's "is this a property" (1, 2 for an
/// HQ, 0 not), which the terrain bar asks of each shown entry (UP/DOWN and
/// SELECT change the army only on a property, and only property entries
/// are redrawn in the new army's colours) and the Feature panel of the
/// picked tool. A Lab is none, so the tower's entry kept the army the bar
/// was opened with, and its own army stepping (0.3.1) left the bar's
/// entries, the picked tool and the placed tile disagreeing. With the
/// towers on, a Lab is a property for those callers (the return
/// addresses below; the editor's map-cell checks, `sub_0800C840` and
/// `sub_0800C608`, are left alone).
pub const IS_PROPERTY: u32 = 0x0800_C7E8;
const IS_PROPERTY_BAR_CALLS: [u32; 12] = [
    0x0800_22D8, // Feature panel (the picked tool)
    0x0800_2348,
    0x0800_23D0,
    0x0800_2440,
    0x0800_6BDA, // terrain bar: UP/DOWN changes the army
    0x0800_6C50, //   and marks the property entries for redrawing
    0x0800_6DEE, // terrain bar: scrolling
    0x0800_6E4A,
    0x0800_6F44,
    0x0800_705C,
    0x0800_70D8,
    0x0800_71A8,
];

/// `sub_080077EC(word, owner)` commits a change of the bar's army (UP/DOWN
/// or SELECT on a property entry, `0x0800701A`): it stores the owner
/// (`+0x2E`), rewrites the five property entries of the list (9..13) from
/// the owner table, and then puts them into the shown entries around the
/// highlighted one, placed by the highlighted word's kind. Here, after the
/// list is rewritten, r6 is the highlighted word. The tower's list entry
/// takes the new army too; with the tower highlighted (a kind the game's
/// placement has no case for: it would write the five over the wrong shown
/// entries), only the shown tower entry changes and the function returns.
pub const OWNER_CHANGED: u32 = 0x0800_782A;
const OWNER_CHANGED_RETURN: u32 = 0x0800_78C2;
const SHOWN: u32 = 0x0200_B0D0;
const SHOWN_SIZE: u32 = 0x1C;
const SHOWN_FIRST: u32 = 0x0200_B03A;
const BAR_OWNER: u32 = 0x0200_B02E;

/// Trap at [`OWNER_CHANGED`].
pub fn owner_changed(core: &mut Core) {
    if !crate::design::in_map_editor(core) || !crate::com_tower::active(core) {
        return;
    }
    let owner = (core.raw_read_8(BAR_OWNER, -1) as u16).min(5);
    let (word, tile) = tower_entry(owner);
    for i in 0..entries(core) {
        if core.raw_read_16(LIST + 4 * i, -1) & 0x11F & !0xE0 == TOWER_WORD {
            core.raw_write_16(LIST + 4 * i, -1, word);
            core.raw_write_16(LIST + 4 * i + 2, -1, tile);
        }
    }
    let highlighted = core.gba().cpu().gpr(6) as u16;
    if highlighted & 0x11F & !0xE0 != TOWER_WORD {
        return;
    }
    let mut shown = core.raw_read_16(SHOWN_FIRST, -1) as i16 as i32 + 4;
    if shown > 9 {
        shown -= 10;
    }
    core.raw_write_16(SHOWN + SHOWN_SIZE * shown as u32 + 4, -1, word);
    core.gba_mut().cpu_mut().set_thumb_pc(OWNER_CHANGED_RETURN);
}

/// Trap at [`IS_PROPERTY`].
pub fn is_property(core: &mut Core) {
    let cpu = core.gba().cpu();
    let (class, lr) = (cpu.gpr(0) as u32, cpu.gpr(14) as u32);
    if class & 0x1F != TOWER_WORD as u32
        || !IS_PROPERTY_BAR_CALLS.contains(&(lr.wrapping_sub(5)))
        || !crate::design::in_map_editor(core)
        || !crate::com_tower::active(core)
    {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, 1);
    cpu.set_thumb_pc(lr & !1);
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
        TOWER_WORD if crate::com_tower::active(core) => PENDING_TOWER,
        _ => (is_invention_type(kind as u8) && load != 0) as u8,
    };
    core.raw_write_8(ICON_PENDING, -1, pending);
}

const BAR_ICON_RETURN: u32 = 0x0800_27AD;
const CRYSTAL_WORD: u16 = OURS | 0x15;
const OBELISK_WORD: u16 = OURS | 0x1A;
const PENDING_CRYSTAL: u8 = 2;
const PENDING_OBELISK: u8 = 3;
const PENDING_TOWER: u8 = 4;

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
        PENDING_TOWER => crate::com_tower::picture(),
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
    // Black Hole's properties (owner 5, classes 0xA6..0xAE): Black Hole's
    // palette, as its buildings (crate::design5).
    let raw = core.gba().cpu().gpr(3) as u32;
    if matches!(raw, 0xA6 | 0xA8 | 0xAA | 0xAB | 0xAE) {
        let palette = crate::invention_art::black_hole_palette() as i32;
        core.gba_mut().cpu_mut().set_gpr(0, palette);
        return;
    }
    // The Com Tower: its owner's building palette (8 + owner), Black Hole's
    // as its other buildings.
    if raw as u16 & 0x11F == TOWER_WORD && crate::com_tower::active(core) {
        let owner = (raw >> 5) & 7;
        let palette = if owner >= 5 {
            crate::invention_art::black_hole_palette() as i32
        } else {
            8 + owner as i32
        };
        core.gba_mut().cpu_mut().set_gpr(0, palette);
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
        crate::invention_art::black_hole_palette() as i32
    };
    core.gba_mut().cpu_mut().set_gpr(0, palette);
}

/// Where the bar draws an entry's name (r0 = the name picture, 32x16, the
/// terrain panel's; r3 = the entry's word).
pub const BAR_NAME: u32 = 0x0800_2998;

/// Trap at [`BAR_NAME`]: the Crystal's, Obelisk's and Wasteland's own names.
pub fn bar_name(core: &mut Core) {
    if !crate::design::in_map_editor(core) {
        return;
    }
    let word = core.gba().cpu().gpr(3) as u16 & (OURS | 0x1F);
    let name = match word {
        CRYSTAL_WORD => crate::obelisk::CRYSTAL_NAME_AT,
        OBELISK_WORD => crate::obelisk::OBELISK_NAME_AT,
        WASTE_WORD => crate::wasteland::NAME_AT,
        TOWER_WORD if crate::com_tower::active(core) => crate::com_tower::NAME_AT,
        _ => return,
    };
    core.gba_mut().cpu_mut().set_gpr(0, name as i32);
}
