//! Five-army design maps: Black Hole as a fifth army in the Design Room,
//! beside Orange Star, Blue Moon, Green Earth and Yellow Comet, saved with
//! the map and played as a five-army Versus battle ([`crate::five`]).
//!
//! A design map is saved as a 0x724-byte record (`sub_0803CFA4`): tiles as
//! they are (Black Hole's property tiles 0x1B4..0x1B9 survive), and one byte
//! per cell for units, `type | (army - 1) << 6`. Unit types stop at 0x18, so
//! bit 5 is never set by the game: tangoAW2 saves a Black Hole unit as
//! `0xE0 | type` and loads any byte with bit 5 set as army 5. A five-army
//! map is marked by the first of the record's five colour bytes (the colour
//! of "slot 0", saved and restored but never used by the game) being
//! [`crate::five::DESIGN_FIVE`].

use mgba::core::Core;

/// The design-map loader (`sub_0803D238`), per cell: `r3` is the saved
/// byte, and `0x0803D28C lsrs r1, r1, #30` makes the army index from its
/// top two bits.
pub const LOAD_ARMY: u32 = 0x0803_D28C;
const LOAD_ARMY_NEXT: u32 = 0x0803_D28E;
/// `movs r2, #0x3F` (the type mask) -> `#0x1F`: bit 5 is the army-5 mark.
const LOAD_TYPE_MASK: (u32, u16, u16) = (0x0803_D29E, 0x223F, 0x221F);
pub const ARMY5_BIT: u8 = 0x20;

/// Trap at [`LOAD_ARMY`].
pub fn load_army(core: &mut Core) {
    let cpu = core.gba().cpu();
    let byte = cpu.gpr(3) as u8;
    let index = if byte & ARMY5_BIT != 0 { 4 } else { byte >> 6 };
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(1, index as i32);
    cpu.set_thumb_pc(LOAD_ARMY_NEXT);
}

/// Every frame: the ROM image edits (idempotent).
pub fn patch_rom(core: &mut Core) {
    let (at, old, new) = LOAD_TYPE_MASK;
    if core.raw_read_16(at, -1) == old {
        core.raw_write_16(at, -1, new);
    }
}

// ---------- the editor ----------

/// The Design Room's map editor is open: Black Hole is army 5 there, with
/// five's unit ids and player table ([`crate::five::patches_on`]).
pub fn editor_active(core: &Core) -> bool {
    crate::design::in_map_editor(core)
}

/// The terrain and unit bars' army slot runs 0..4 (terrain; 0 neutral) and
/// 1..4 (units): `cmp r4, #4` / `movs r4, #4` in the editor's input handler
/// (`sub_08005F4C`), raised to 5. And the (property kind, owner) -> bar entry
/// table, `0x084887AC`, 5 kinds x 5 owners with the terrain template right
/// after it: a copy with 8 owners per kind ([`OWNER_TABLE`]), and its two
/// readers' row index `k * 5 + owner` -> `k * 8 + owner`.
const EDITOR_HALVES: [(u32, u16, u16); 8] = [
    (0x0800_6C10, 0x2C04, 0x2C05),
    (0x0800_6C32, 0x2404, 0x2405),
    (0x0800_6CC0, 0x2C04, 0x2C05),
    (0x0800_6CE2, 0x2404, 0x2405),
    (0x0800_795C, 0x00A0, 0x00E0), // lsls r0, r4, #2 -> #3
    (0x0800_795E, 0x1900, 0x1C00), // adds r0, r0, r4 -> adds r0, r0, #0
    (0x0800_7808, 0x0081, 0x00C1), // lsls r1, r0, #2 -> #3
    (0x0800_780A, 0x1809, 0x1C09), // adds r1, r1, r0 -> adds r1, r1, #0
];
const GAME_OWNER_TABLE: u32 = 0x0848_87AC;
const OWNER_TABLE: u32 = 0x0866_0000;
const EDITOR_WORDS: [u32; 2] = [0x0800_7978, 0x0800_7848];

/// Army 5's property tiles (tangoAW2's, see `five_map`): (class, tile) for
/// the HQ, city, base, airport and port, in the owner table's kind order, and
/// the lab.
const ARMY5_PROPERTIES: [(u16, u16); 5] = [
    (0xA8, 0x1B4),
    (0xA6, 0x1B6),
    (0xAE, 0x1B5),
    (0xAA, 0x1B7),
    (0xAB, 0x1B8),
];
const ARMY5_LAB: (u16, u16) = (0xB4, 0x1B9);
pub const HQ5_TILE: u16 = 0x1B4;
const HQ5_CLASS: u8 = 0xA8;

/// Written once: the owner table with Black Hole's column.
fn install_tables(core: &mut Core) {
    if core.raw_read_16(OWNER_TABLE + 4 * 5, -1) == HQ5_CLASS as u16 {
        return;
    }
    for k in 0..5u32 {
        for owner in 0..5u32 {
            let from = GAME_OWNER_TABLE + 4 * (k * 5 + owner);
            let to = OWNER_TABLE + 4 * (k * 8 + owner);
            core.raw_write_32(to, -1, core.raw_read_32(from, -1));
        }
        let (class, tile) = ARMY5_PROPERTIES[k as usize];
        core.raw_write_16(OWNER_TABLE + 4 * (k * 8 + 5), -1, class);
        core.raw_write_16(OWNER_TABLE + 4 * (k * 8 + 5) + 2, -1, tile);
        for owner in 6..8u32 {
            core.raw_write_32(OWNER_TABLE + 4 * (k * 8 + owner), -1, 0);
        }
    }
}

/// Set while the editor is open, once its players have been moved.
const EDITOR_READY: u32 = 0x0203_FFAC;
const OLD_PLAYERS: u32 = 0x0202_3284;

/// Every frame: the editor's patches on while it is open, off otherwise.
pub fn sync(core: &mut Core) {
    install_tables(core);
    let on = editor_active(core);
    // The editor sets its armies up in the game's player table before its
    // patches are on: move them to five's (with Black Hole as player 5).
    if !on {
        core.raw_write_8(EDITOR_READY, -1, 0);
        crate::invention_art::forget_saved(core);
    } else if core.raw_read_8(EDITOR_READY, -1) == 0 {
        let mut buf = vec![0u8; 5 * PLAYER_SIZE as usize];
        core.raw_read_range(OLD_PLAYERS, -1, &mut buf);
        core.raw_write_range(crate::five::PLAYERS, -1, &buf);
        let p5 = crate::five::PLAYERS + PLAYER_SIZE * 5;
        core.raw_write_range(p5, -1, &buf[4 * PLAYER_SIZE as usize..]);
        core.raw_write_8(p5 + COLOUR, -1, 5);
        core.raw_write_8(p5 + CO, -1, FLAK);
        // The editor opens on a new map: Normal until one is loaded or
        // switched (a battle's biome must not carry over).
        crate::wasteland::set_biome(core, crate::wasteland::NORMAL);
        core.raw_write_8(EDITOR_READY, -1, 1);
    }
    for (at, old, new) in EDITOR_HALVES {
        let want = if on { new } else { old };
        if core.raw_read_16(at, -1) != want {
            core.raw_write_16(at, -1, want);
        }
    }
    for at in EDITOR_WORDS {
        let want = if on { OWNER_TABLE } else { GAME_OWNER_TABLE };
        if core.raw_read_32(at, -1) != want {
            core.raw_write_32(at, -1, want);
        }
    }
}

/// Unit ids: the editor gives a new unit the block `(army - 1) << 6`
/// (`0x0800894E lsls r0, r0, #6`); with five's ids, `(army - 1) * 51`.
pub const CREATE_BASE: u32 = 0x0800_894E;
pub fn create_base(core: &mut Core) {
    if !editor_active(core) {
        return;
    }
    let r0 = core.gba().cpu().gpr(0);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, r0 * 51);
    cpu.set_thumb_pc(CREATE_BASE + 2);
}

/// The unit on a cell's army block (`GetUnitTypeAt`: `movs r3, #0xC0; ands
/// r3, r2` with r2 the id): `(id / 51) << 6` with five's ids.
pub const UNIT_BLOCK: u32 = 0x0800_8B8A;
pub fn unit_block(core: &mut Core) {
    if !editor_active(core) {
        return;
    }
    let id = core.gba().cpu().gpr(2) as u32 & 0xFF;
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(3, ((id / 51) << 6) as i32);
    cpu.set_thumb_pc(UNIT_BLOCK + 4);
}

/// Saving a design map (`sub_0803CFA4`), per unit: r0 is its army (1..5;
/// five's hook at 0x0803D098 makes it from the id), r1 its unit, r2 where
/// the byte goes. Army 5 is saved as `0xE0 | type` (see the module doc).
pub const SAVE_UNIT: u32 = 0x0803_D09C;
const SAVE_UNIT_DONE: u32 = 0x0803_D0C0;
pub fn save_unit(core: &mut Core) {
    let cpu = core.gba().cpu();
    let (army, unit, to) = (cpu.gpr(0), cpu.gpr(1) as u32, cpu.gpr(2) as u32);
    if !editor_active(core) || army != 5 {
        return;
    }
    let kind = core.raw_read_8(unit, -1) & 0x1F;
    core.raw_write_8(to, -1, 0xE0 | ARMY5_BIT | kind);
    core.gba_mut().cpu_mut().set_thumb_pc(SAVE_UNIT_DONE);
}

/// The unit bar's count panel (`0x080024FC ldrsb r3, [r2, #0x12]`, the
/// army's count from the editor's four counters): army 5's own count.
pub const UNIT_COUNT: u32 = 0x0800_24FC;
const E_UNIT_SLOT: u32 = 0x0200_B02F;
pub fn unit_count(core: &mut Core) {
    if !editor_active(core) || core.raw_read_8(E_UNIT_SLOT, -1) != 5 {
        return;
    }
    let n = army5_units(core);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(3, n as i32);
    cpu.set_thumb_pc(UNIT_COUNT + 2);
}

const UNITS: u32 = 0x0202_2684;
fn army5_units(core: &Core) -> u32 {
    (205..255)
        .filter(|id| core.raw_read_8(UNITS + 12 * id, -1) != 0)
        .count() as u32
}

/// The terrain bar's and Feature panel's property picture
/// (`sub_0800272C`): HQs load their army's art (`sub_0803F6BC(8, army)`)
/// for classes 0x08..0x88; Black Hole's, 0xA8, too.
pub const HQ_ICON: u32 = 0x0800_2756;
const HQ_ICON_DONE: u32 = 0x0800_27AF;
const LOAD_ICON: u32 = 0x0803_F6BC;
pub fn hq_icon(core: &mut Core) {
    if core.gba().cpu().gpr(6) as u8 != HQ5_CLASS {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, 8);
    cpu.set_gpr(1, 5);
    cpu.set_gpr(14, HQ_ICON_DONE as i32);
    cpu.set_thumb_pc(LOAD_ICON);
}

/// A building's sprite palette (`sub_0803F908`: owner + 8, 13 for army 5,
/// which in the editor is its panels' palette): OBJ 2 there, Black Hole's.
pub const BUILDING_PALETTE: u32 = 0x0803_F928;
pub fn building_palette(core: &mut Core) {
    if !editor_active(core) || core.gba().cpu().gpr(3) != 5 {
        return;
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(3, 0x2000);
    cpu.set_thumb_pc(BUILDING_PALETTE + 8);
}

/// The tile a property class is placed as (`GetDefaultTileForTerrain`,
/// whose jump table stops at owner 4): Black Hole's.
pub const DEFAULT_TILE: u32 = 0x0800_12DC;
pub fn default_tile(core: &mut Core) {
    let class = core.gba().cpu().gpr(0) as u16 & 0xFF;
    let tower = (class & 0x1F == crate::com_tower::LAB as u16 && crate::com_tower::active(core))
        .then(|| crate::com_tower::tile_for((class >> 5) as u8));
    let tile = tower.or_else(|| {
        ARMY5_PROPERTIES
            .iter()
            .chain(std::iter::once(&ARMY5_LAB))
            .find(|&&(c, _)| c == class)
            .map(|&(_, t)| t)
    });
    if let Some(tile) = tile {
        let lr = core.gba().cpu().gpr(14) as u32;
        let cpu = core.gba_mut().cpu_mut();
        cpu.set_gpr(0, tile as i32);
        cpu.set_thumb_pc(lr & !1);
    }
}

/// A plain's look from the cell to its left (`sub_08001704`: class - 3
/// through a jump table that stops at Yellow Comet's classes, 0x8E), so a
/// building's shadow: Black Hole's classes (0xA0..) as Yellow Comet's.
pub const PLAIN_LEFT: u32 = 0x0800_1750;
pub fn plain_left(core: &mut Core) {
    let class = core.gba().cpu().gpr(0) as u32;
    if editor_active(core) && (0xA0..=0xBF).contains(&class) {
        core.gba_mut().cpu_mut().set_gpr(0, (class - 0x20) as i32);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (LOAD_ARMY, Box::new(load_army)),
        (CREATE_BASE, Box::new(create_base)),
        (UNIT_BLOCK, Box::new(unit_block)),
        (SAVE_UNIT, Box::new(save_unit)),
        (UNIT_COUNT, Box::new(unit_count)),
        (HQ_ICON, Box::new(hq_icon)),
        (BUILDING_PALETTE, Box::new(building_palette)),
        (DEFAULT_TILE, Box::new(default_tile)),
        (HQ_LOOKUP, Box::new(hq_lookup)),
        (SAVE_RECORD, Box::new(save_record)),
        (PLAIN_LEFT, Box::new(plain_left)),
    ]
}

// ---------- the editor, every frame ----------

const MAP: u32 = 0x0201_E450;
const E_STATE: u32 = 0x0200_B004;
/// The map-wide colour list's first byte (`gPlaySt.armyColor[0]`), saved
/// with the map as the five-army mark (see the module doc).
const MARK: u32 = 0x0300_3FF3;
const PLAYER_SIZE: u32 = 0x3C;
const COLOUR: u32 = 0x1A;
const CO: u32 = 0x1D;
const FLAK: u8 = 11;
/// Black Hole's unit palette, and the BG palette army 5's units are drawn
/// with (five's `ICON_PALETTES[5]`: BG 11, the game's unused neutral-unit
/// palette).
const UNIT_PALETTES: u32 = 0x0810_E6E0;
/// Black Hole's HQ art (five's HQ sheet art, army 5), and where its sprite
/// takes it from (OBJ tile 0x90, the building sheet's lab slot).
const HQ5_ART: u32 = 0x080D_16C4 + 4 * 0x100;
const HQ5_VRAM: u32 = 0x0601_0000 + 0x90 * 32;
const UNIT_BANK: u32 = 11;

fn map_size(core: &Core) -> (u32, u32) {
    (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32)
}
fn cell(core: &Core, x: u32, y: u32) -> u32 {
    core.raw_read_16(MAP + 0x417A + 2 * y, -1) as u32 + x
}
fn tile_at(core: &Core, x: u32, y: u32) -> u16 {
    core.raw_read_16(MAP + 0xA22 + 2 * cell(core, x, y), -1) & 0x1FF
}
/// Where Black Hole's HQ is, if one is placed.
fn hq5(core: &Core) -> Option<(u32, u32)> {
    let (w, h) = map_size(core);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .find(|&(x, y)| tile_at(core, x, y) == HQ5_TILE)
}
/// Whether the map has anything of Black Hole's: a property or a unit.
fn has_army5(core: &Core) -> bool {
    let (w, h) = map_size(core);
    let props = (0..h).any(|y| (0..w).any(|x| (0x1B4..=0x1B9).contains(&tile_at(core, x, y))));
    props || army5_units(core) > 0
}

pub fn editor_tick(core: &mut Core) {
    // Black Hole's HQ picture in the building sheet's lab slot (five puts
    // it in the sheet, which the editor loaded before).
    let mut art = [0u8; 0x100];
    core.raw_read_range(HQ5_ART, -1, &mut art);
    let mut now = [0u8; 0x100];
    core.raw_read_range(HQ5_VRAM, -1, &mut now);
    if now != art {
        core.raw_write_range(HQ5_VRAM, -1, &art);
    }
    // Army 5's units' BG palette.
    let mut pal = [0u8; 0x20];
    core.raw_read_range(UNIT_PALETTES + 4 * 0x20, -1, &mut pal);
    for base in [0x0300_20C0, 0x0500_0000] {
        let mut now = [0u8; 0x20];
        core.raw_read_range(base + UNIT_BANK * 0x20, -1, &mut now);
        if now != pal {
            core.raw_write_range(base + UNIT_BANK * 0x20, -1, &pal);
        }
    }
    // The five-army mark goes with the map when it is saved.
    let mark = if has_army5(core) { crate::five::DESIGN_FIVE } else { 0 };
    if core.raw_read_8(MARK, -1) != mark {
        core.raw_write_8(MARK, -1, mark);
    }
    load_emblem(core);
}

/// The editor's "where is this army's HQ" (`sub_0800C6E8(hq, &x, &y)`,
/// which placing an HQ uses to remove the army's old one): the four armies'
/// come from its table, Black Hole's from the map.
pub const HQ_LOOKUP: u32 = 0x0800_C6E8;
pub fn hq_lookup(core: &mut Core) {
    let cpu = core.gba().cpu();
    let (hq, px, py, lr) = (
        cpu.gpr(0) as u32,
        cpu.gpr(1) as u32,
        cpu.gpr(2) as u32,
        cpu.gpr(14) as u32,
    );
    if hq != HQ5_CLASS as u32 || !editor_active(core) {
        return;
    }
    let found = hq5(core);
    if let Some((x, y)) = found {
        core.raw_write_32(px, -1, x);
        core.raw_write_32(py, -1, y);
    }
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, found.is_some() as i32);
    cpu.set_thumb_pc(lr & !1);
}

// ---------- the fifth HQ emblem ----------

/// The editor shows the placed HQs' army emblems in a 2x2 grid beside its
/// side panel (`sub_08003088`, four lanes: 16x16 sprites, OBJ palette 4,
/// lane k's tiles at 796 + 4k, 18 pixels apart), on whichever side of the
/// screen the panel is. Black Hole's goes beside them, drawn here into the
/// last four of the editor's free OBJ tiles (its art is the Teams screen's
/// emblem 0x42, made for the same palette).
const EMBLEM_TILE: u32 = 532;
const LANE_TILE: u32 = 796;
const EMBLEM_STEP: i32 = 18;
/// From a column of the X to its middle: one emblem's width, so the middle
/// one fits between the columns.
const X_STEP: i32 = 16;
const EMBLEM_PALETTE: u16 = 4;
/// The flushed sprite area's descriptor (see `branding::flush`).
const FLUSH_AREA: u32 = 0x0300_0268;

fn load_emblem(core: &mut Core) {
    let id = crate::pvp::EMBLEM_BASE_ID + 5;
    let n = crate::pvp::sprite_tiles(core, id).min(4);
    let mut art = vec![0u8; (n * 32) as usize];
    core.raw_read_range(
        crate::pvp::sprite_source(core, id, crate::pvp::EMBLEM_GROUP),
        -1,
        &mut art,
    );
    let at = 0x0601_0000 + EMBLEM_TILE * 32;
    let mut now = vec![0u8; art.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != art {
        core.raw_write_range(at, -1, &art);
    }
}

/// At the VBlank sprite flush (see `design::flush_sprites`): with Black
/// Hole's HQ placed, the five emblems as an X (the dice five): the other
/// armies' four spread to the corners, Black Hole's in the middle.
pub fn append_emblem(core: &mut Core, at: u32, end: u32) -> u32 {
    if !editor_active(core) || core.raw_read_8(E_STATE, -1) != 1 || at + 8 > end || hq5(core).is_none() {
        return at;
    }
    // The four lanes' sprites this frame (lane k is column k % 2, row k / 2
    // of the game's 2 x 2 grid), and the grid's corner.
    let start = core.raw_read_32(FLUSH_AREA, -1);
    let mut lanes = Vec::new();
    let mut origin = None;
    // The lanes are double-size affine sprites, drawn 8 pixels right of and
    // below their coordinates; Black Hole's is a plain one.
    let mut offset = 0;
    let mut p = start;
    while p + 8 <= at {
        let (a0, a1, a2) = (
            core.raw_read_16(p, -1),
            core.raw_read_16(p + 2, -1),
            core.raw_read_16(p + 4, -1),
        );
        let tile = (a2 & 0x3FF) as u32;
        if a0 & 0x300 != 0x200 && (LANE_TILE..LANE_TILE + 16).contains(&tile) && (tile - LANE_TILE) % 4 == 0 {
            let lane = ((tile - LANE_TILE) / 4) as i32;
            let x = ((a1 & 0x1FF) as i32 ^ 0x100) - 0x100;
            let y = ((a0 & 0xFF) as i32 ^ 0x80) - 0x80;
            origin.get_or_insert((x - EMBLEM_STEP * (lane % 2), y - EMBLEM_STEP * (lane / 2)));
            if a0 & 0x300 == 0x300 {
                offset = 8;
            }
            lanes.push((p, lane));
        }
        p += 8;
    }
    let Some((x0, y0)) = origin else { return at };
    // The rows stay (the "Play OK!" label is above the grid and the
    // "Surplus" count under it); the columns move apart, away from the
    // screen's edge, to make room in the middle.
    let span = 2 * X_STEP;
    let left = if x0 < 120 { x0 } else { x0 + EMBLEM_STEP - span };
    for (p, lane) in lanes {
        let x = left + span * (lane % 2);
        let y = y0 + EMBLEM_STEP * (lane / 2);
        let a0 = core.raw_read_16(p, -1);
        let a1 = core.raw_read_16(p + 2, -1);
        core.raw_write_16(p, -1, (a0 & 0xFF00) | (y as u16 & 0xFF));
        core.raw_write_16(p + 2, -1, (a1 & !0x1FF) | (x as u16 & 0x1FF));
    }
    let (x, y) = (left + X_STEP + offset, y0 + EMBLEM_STEP / 2 + offset);
    core.raw_write_16(at, -1, (y as u16) & 0xFF);
    core.raw_write_16(at + 2, -1, (x as u16 & 0x1FF) | (1 << 14));
    core.raw_write_16(at + 4, -1, EMBLEM_TILE as u16 | (EMBLEM_PALETTE << 12));
    at + 8
}

// ---------- saving ----------

/// Saving a design map (`sub_0803CF54`): the record is built at
/// 0x02000000 and its army count (+0x4C3) set from the HQs; at
/// `0x0803CF7A` it is about to be written to flash. For a five-army map
/// (its mark, +0x4C4, is [`crate::five::DESIGN_FIVE`]): the army count
/// the game sees stays 4 (the Versus list only takes 2..4; tangoAW2 adds
/// army 5 itself), and the property totals, which the game's per-owner
/// counters get wrong for owner 5 (its count lands on the running total a
/// second time, +0x4C9, and is left out of the largest, +0x4CA), are put
/// right.
pub const SAVE_RECORD: u32 = 0x0803_CF7A;
const RECORD: u32 = 0x0200_0000;
pub fn save_record(core: &mut Core) {
    crate::wasteland::save_record(core, RECORD);
    if core.raw_read_8(RECORD + 0x4C4, -1) != crate::five::DESIGN_FIVE {
        return;
    }
    let (w, h) = (
        core.raw_read_8(RECORD, -1) as u32,
        core.raw_read_8(RECORD + 1, -1) as u32,
    );
    let army5 = (0..w * h)
        .filter(|k| (0x1B4..=0x1B9).contains(&(core.raw_read_16(RECORD + 2 + 2 * k, -1) & 0x1FF)))
        .count() as u8;
    if core.raw_read_8(RECORD + 0x4C3, -1) > 4 {
        core.raw_write_8(RECORD + 0x4C3, -1, 4);
    }
    let total = core.raw_read_8(RECORD + 0x4C9, -1);
    core.raw_write_8(RECORD + 0x4C9, -1, total.saturating_sub(army5));
    let most = core.raw_read_8(RECORD + 0x4CA, -1);
    core.raw_write_8(RECORD + 0x4CA, -1, most.max(army5 + 1));
}
