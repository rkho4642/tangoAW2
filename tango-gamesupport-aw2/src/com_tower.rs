//! Dual Strike's Com Tower, with the Dual Strike pack, in Versus.
//!
//! Every unit of the army that owns a Com Tower fires 10% harder per tower
//! it owns; a neutral tower does nothing until it is captured. Towers earn
//! no funds, and capturing one never ends the battle (a Lab's would).
//!
//! AW2 has no free terrain code, but it has the Lab (0x14): a property that
//! is captured, counted per army (`gPlayers[a].labs`, +0x10) and has owner
//! tiles, and which only the campaign uses (capturing one does not end a
//! Versus battle). In Versus with the pack a Lab is a Com Tower: it gets
//! Dual Strike's tower picture (from the pack, `bmap/015` cell 8, drawn in
//! AW2's building colours), the name "Tower", and the firepower. In the
//! campaign, or without the pack, a Lab is a Lab.
//!
//! The picture goes in the building sheet's Lab tiles, except in a
//! five-army game, where those hold Black Hole's HQ ([`crate::five`]): there
//! the tower is drawn from the Crystal's tiles (or, with a Crystal on the
//! map, the Obelisk's; with both, it keeps the Lab's picture).

use mgba::core::Core;
use std::sync::OnceLock;

use crate::ds_weather::is_on;

pub const LAB: u8 = 0x14;
const GAME_MODE: u32 = 0x0300_3FC1;
const VERSUS: u8 = 3;
/// Firepower per tower, in %.
const PER_TOWER: i32 = 10;

/// Whether Labs are Com Towers now: in Versus, and in the Design Room's
/// map editor, with the pack.
pub fn active(core: &Core) -> bool {
    is_on(core) && (core.raw_read_8(GAME_MODE, -1) == VERSUS || crate::design::in_map_editor(core))
}

/// The Lab tile of an owner (0 neutral .. 4; 5 is Black Hole's, tangoAW2's
/// own, see `crate::five`).
pub fn tile_for(owner: u8) -> u16 {
    match owner {
        0..=4 => 0x1D9 + owner as u16,
        _ => 0x1B9,
    }
}

/// The towers each army owns, counted on the map every frame (the game's
/// own Lab count, +0x10 of each player, is not reset when it recounts, so
/// it runs high after a capture).
const TOWER_COUNTS: u32 = 0x0203_FFB0;

pub fn towers(core: &Core, army: u32) -> u32 {
    if !(1..=5).contains(&army) {
        return 0;
    }
    core.raw_read_8(TOWER_COUNTS + army - 1, -1) as u32
}

fn count_towers(core: &mut Core) {
    let mut n = [0u8; 5];
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    for y in 0..h.min(30) {
        let row = core.raw_read_16(ROWS + 2 * y, -1) as u32;
        for x in 0..w.min(30) {
            let c = core.raw_read_8(CLASSES + row + x, -1);
            let owner = (c >> 5) as usize;
            if c & 0x1F == LAB && (1..=5).contains(&owner) {
                n[owner - 1] += 1;
            }
        }
    }
    for (i, v) in n.iter().enumerate() {
        if core.raw_read_8(TOWER_COUNTS + i as u32, -1) != *v {
            core.raw_write_8(TOWER_COUNTS + i as u32, -1, *v);
        }
    }
}

// --- Firepower ---------------------------------------------------------

/// `sub_0804334C(battle unit)`, the firepower % added to a side in the
/// damage formula: the army in r0 just before its abilities are looked up,
/// and the result in r0 at its return.
const FIREPOWER_ARMY: u32 = 0x0804_336E;
const FIREPOWER_DONE: u32 = 0x0804_3380;
/// The army of the side being worked out (between the two traps).
const SIDE_ARMY: u32 = 0x0203_FFAA;

fn firepower_army(core: &mut Core) {
    if active(core) {
        let army = core.gba().cpu().gpr(0) as u8;
        core.raw_write_8(SIDE_ARMY, -1, army);
    }
}

fn firepower_done(core: &mut Core) {
    if !active(core) {
        return;
    }
    let army = core.raw_read_8(SIDE_ARMY, -1) as u32;
    let n = towers(core, army) as i32;
    if n > 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r0 = cpu.gpr(0);
        cpu.set_gpr(0, r0 + PER_TOWER * n);
    }
}

/// `RecountArmyProperties` lists each army's key properties in `gProperty`
/// (0x03003150): its HQs, and its Labs, which count as HQs: capturing one
/// defeats its army (the campaign's Lab rule; in Versus too). A Com Tower
/// is no such thing, so with the towers on a Lab is left out of the list
/// (r0 = its class, about to be stored; 0x08021C5A moves on to the next
/// cell without adding it).
const KEY_PROPERTY: u32 = 0x0802_1C4C;
const NEXT_CELL: u32 = 0x0802_1C5A;
fn key_property(core: &mut Core) {
    if active(core) && core.gba().cpu().gpr(0) as u8 & 0x1F == LAB {
        core.gba_mut().cpu_mut().set_thumb_pc(NEXT_CELL);
    }
}

/// A capture completes (`sub_08042650`): for an HQ or a Lab it marks the
/// previous owner to be defeated at the end of the turn (`killOnEndTurn`,
/// player +0x32). Here r0 is the captured terrain, just tested against the
/// HQ: a tower skips the mark (0x08042834 is past it).
const CAPTURED: u32 = 0x0804_281E;
const CAPTURED_DONE: u32 = 0x0804_2834;
fn captured(core: &mut Core) {
    if active(core) && core.gba().cpu().gpr(0) as u8 == LAB {
        core.gba_mut().cpu_mut().set_thumb_pc(CAPTURED_DONE);
    }
}

/// The CO screen's unit bars (`GetFirepowerIcon(army, unit)`): the army at
/// its entry, and the CO's firepower bonus in r0 after it is looked up, from
/// which the bar is chosen. With towers the bars show their bonus too.
const BAR_ARMY: u32 = 0x0808_5410;
const BAR_BONUS: u32 = 0x0808_5428;
/// The army whose bars are being drawn.
const BAR_SIDE: u32 = 0x0203_FFAB;

fn bar_army(core: &mut Core) {
    if active(core) {
        let army = core.gba().cpu().gpr(0) as u8;
        core.raw_write_8(BAR_SIDE, -1, army);
    }
}

fn bar_bonus(core: &mut Core) {
    if !active(core) {
        return;
    }
    let n = towers(core, core.raw_read_8(BAR_SIDE, -1) as u32) as i32;
    if n > 0 {
        let cpu = core.gba_mut().cpu_mut();
        let r0 = cpu.gpr(0);
        cpu.set_gpr(0, r0 + PER_TOWER * n);
    }
}

/// `sub_08026C6C(kind)`, a property kind's daily income: a Com Tower earns
/// nothing (as in Dual Strike).
const INCOME: u32 = 0x0802_6C6C;
const NO_INCOME: u32 = 0x0802_6CCC;
fn income(core: &mut Core) {
    if active(core) && core.gba().cpu().gpr(0) as u8 & 0x1F == LAB {
        core.gba_mut().cpu_mut().set_thumb_pc(NO_INCOME);
    }
}

// --- Picture -------------------------------------------------------------

/// Dual Strike's property sprites (16x32 cells, 4bpp linear).
const DS_SPRITES: &str = "bmap/015";
const DS_TOWER_CELL: usize = 8;
/// Dual Strike's property colours -> AW2's building colours, shade for
/// shade, so the game's owner palettes colour the tower like its other
/// buildings: Dual Strike's 1-3 are the grass under it (clear here), 4
/// white, 5-6 light, 7-8 mid, 9-10 dark and 11-12 darkest owner colour, 13
/// yellow, 14 cream; AW2's building sprites (OBJ palette 8 + owner) and the
/// terrain panel (its palette 8 + owner) both have white at 1, the owner
/// shades at 2-5 (light to darkest), yellow at 6 and cream at 7.
const SHADES: [u8; 16] = [0, 0, 0, 0, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 0];

static PICTURE: OnceLock<Option<Vec<u8>>> = OnceLock::new();

/// The tower as 8 GBA tiles (16x32, 1D order), in AW2's building colours.
pub fn picture() -> Option<&'static [u8]> {
    PICTURE
        .get_or_init(|| {
            let pack = crate::ds_pack::pack()?;
            let sprites = crate::ds_art::lz10(pack.file(DS_SPRITES)?)?;
            let cell = sprites.get(0x100 * DS_TOWER_CELL..0x100 * (DS_TOWER_CELL + 1))?;
            let bmp: Vec<u8> = cell
                .iter()
                .map(|&b| SHADES[(b & 15) as usize] | SHADES[(b >> 4) as usize] << 4)
                .collect();
            Some(crate::ds_art::tiles(&bmp, 16, &[(0, 0, 16, 32)]))
        })
        .as_deref()
}

/// The building sheet's Lab tiles (OBJ tiles 0x90..0x97).
const LAB_OBJ_TILE: u32 = 0x90;
const OBJ_VRAM: u32 = 0x0601_0000;

/// The building sheets (clear, snow) the map loads to OBJ tile 0x48: the
/// Lab's picture is at tile 0x90.
const SHEETS: [u32; 2] = [0x080C_FFC4, 0x080D_0B44];
const SHEET_TILE: u32 = 0x48;

/// Every frame: where the map's sprites hold the Lab's picture, the tower's
/// (only an exact Lab picture is replaced, so any other screen's use of
/// those tiles is left alone). In a five-army game, the tower's own tiles
/// ([`five_army_tiles`]).
pub fn show(core: &mut Core) {
    if !active(core) {
        return;
    }
    let Some(p) = picture() else {
        return;
    };
    if crate::five::active(core) || crate::design::in_map_editor(core) {
        after_sheet(core);
        return;
    }
    let at = OBJ_VRAM + 32 * LAB_OBJ_TILE;
    let mut now = [0u8; 256];
    core.raw_read_range(at, -1, &mut now);
    if now[..] == p[..] {
        return;
    }
    for sheet in SHEETS {
        let mut lab = [0u8; 256];
        core.raw_read_range(sheet + 32 * (LAB_OBJ_TILE - SHEET_TILE), -1, &mut lab);
        if now == lab {
            core.raw_write_range(at, -1, p);
            return;
        }
    }
}

fn map_has(core: &Core, tile: u16) -> bool {
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    (0..h.min(30)).any(|y| (0..w.min(30)).any(|x| crate::obelisk::tile_at(core, x, y) == tile))
}

/// Sprite tiles the Design Room's editor never uses (the end of
/// `crate::invention_art`'s range, past what the inventions take).
const EDITOR_TILE: u32 = 524;

/// Where the tower is drawn from when the Lab's tiles are taken: in the
/// Design Room (they hold Black Hole's HQ there, `crate::design5`), tiles
/// the editor never uses; in a five-army game (they hold Black Hole's HQ,
/// `crate::five`), tangoAW2's own battle tiles: the Crystal's when no
/// Crystal is on the map, else the Obelisk's when no Obelisk is (with both,
/// the tower keeps the game's picture).
pub fn five_army_tiles(core: &Core) -> Option<u32> {
    if !active(core) {
        return None;
    }
    if crate::design::in_map_editor(core) {
        return Some(EDITOR_TILE);
    }
    if !crate::five::active(core) {
        return None;
    }
    if !map_has(core, crate::obelisk::CRYSTAL_TILE) {
        Some(crate::obelisk::CRYSTAL_OBJ_TILE)
    } else if !map_has(core, crate::obelisk::OBELISK_TILE) {
        Some(crate::obelisk::OBELISK_OBJ_TILE)
    } else {
        None
    }
}

/// After the building sheet (and tangoAW2's structures' tiles) load, and
/// every frame: in a five-army game, the tower's picture in its tiles.
pub fn after_sheet(core: &mut Core) {
    let (Some(t), Some(p)) = (five_army_tiles(core), picture()) else {
        return;
    };
    let at = OBJ_VRAM + 32 * t;
    let mut now = [0u8; 256];
    core.raw_read_range(at, -1, &mut now);
    if now[..] != p[..] {
        core.raw_write_range(at, -1, p);
    }
}

/// At the Design Room's VBlank sprite flush ([`crate::design::flush_sprites`]):
/// the map's towers, which the editor itself never draws, as 16x32 sprites
/// in their owner's building palette (8 + owner; Black Hole's, as its other
/// buildings there), standing a cell tall like the battle's.
pub fn append_editor(core: &mut Core, at: u32, end: u32, bottom: i32) -> u32 {
    if !active(core) || !crate::design::in_map_editor(core) {
        return at;
    }
    let (w, h) = (core.raw_read_16(MAP, -1) as u32, core.raw_read_16(MAP + 2, -1) as u32);
    let (cam_x, cam_y) = (core.raw_read_16(MAP + 4, -1) as i32, core.raw_read_16(MAP + 6, -1) as i32);
    let mut at = at;
    for y in 0..h.min(30) {
        let row = core.raw_read_16(ROWS + 2 * y, -1) as u32;
        for x in 0..w.min(30) {
            let c = core.raw_read_8(CLASSES + row + x, -1);
            if c & 0x1F != LAB {
                continue;
            }
            let (sx, sy) = (16 * x as i32 - cam_x, 16 * y as i32 - 16 - cam_y);
            if sx <= -16 || sx >= 240 || sy <= -32 || sy + 16 >= bottom || at + 8 > end {
                continue;
            }
            let owner = (c >> 5) as u16;
            let palette = if owner >= 5 {
                crate::invention_art::black_hole_palette() as u16
            } else {
                8 + owner
            };
            core.raw_write_16(at, -1, 0x8000 | (sy as u16 & 0xFF));
            core.raw_write_16(at + 2, -1, 0x8000 | (sx as u16 & 0x1FF));
            core.raw_write_16(at + 4, -1, palette << 12 | 3 << 10 | EDITOR_TILE as u16);
            at += 8;
        }
    }
    at
}

/// The building sprite for a tower (`sub_0803F908`'s definition `def`, the
/// Lab's): in a five-army game, a copy drawn from [`five_army_tiles`].
pub fn sprite_def(core: &mut Core, def: u32) -> Option<u32> {
    let t = five_army_tiles(core)?;
    if core.raw_read_16(def, -1) != 1 {
        return None;
    }
    let (a0, a1, a2) = (
        core.raw_read_16(def + 2, -1),
        core.raw_read_16(def + 4, -1),
        core.raw_read_16(def + 6, -1),
    );
    let a2 = (a2 & 0xFC00) | (t - SHEET_TILE) as u16;
    for (i, h) in [1, a0, a1, a2].iter().enumerate() {
        core.raw_write_16(FIVE_ARMY_DEF + 2 * i as u32, -1, *h);
    }
    Some(FIVE_ARMY_DEF)
}

// --- Terrain panel ---------------------------------------------------------

/// tangoAW2's Com Tower data in the ROM image's free space.
const DATA: u32 = 0x0867_2000;
pub const NAME_AT: u32 = DATA;
pub const PICTURE_AT: u32 = DATA + 0x100;
/// The five-army tower sprite ([`sprite_def`]).
const FIVE_ARMY_DEF: u32 = DATA + 0x200;
const DATA_SENTINEL: u32 = DATA + 0xFFC;
const DATA_MAGIC: u32 = 0x3354_4344; // "DCT3"

/// Every frame with the pack on: the map's tower pictures ([`show`]), and
/// the panel's name and picture in the ROM image (once).
pub fn tick(core: &mut Core, on: bool) {
    if !on {
        return;
    }
    if active(core) {
        count_towers(core);
    }
    show(core);
    if core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC {
        return;
    }
    let Some(p) = picture() else {
        return;
    };
    core.raw_write_range(NAME_AT, -1, &TOWER_NAME);
    core.raw_write_range(PICTURE_AT, -1, p);
    core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
}

const MAP: u32 = 0x0201_E450;
const CLASSES: u32 = MAP + 0x1432;
const ROWS: u32 = MAP + 0x417A;

/// Whether the map cell (x, y) is a Com Tower now.
pub fn tower_at(core: &Core, x: u32, y: u32) -> bool {
    if !active(core) || x >= 30 || y >= 30 {
        return false;
    }
    let row = core.raw_read_16(ROWS + 2 * y, -1) as u32;
    core.raw_read_8(CLASSES + row + x, -1) & 0x1F == LAB
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (FIREPOWER_ARMY, Box::new(firepower_army)),
        (FIREPOWER_DONE, Box::new(firepower_done)),
        (INCOME, Box::new(income)),
        (KEY_PROPERTY, Box::new(key_property)),
        (CAPTURED, Box::new(captured)),
        (BAR_ARMY, Box::new(bar_army)),
        (BAR_BONUS, Box::new(bar_bonus)),
    ]
}

/// "Tower", the Com Tower's name in the terrain panel
/// (32x16, 4x2 tiles, the panel font colours: 1 fill, 15 outline).
pub const TOWER_NAME: [u8; 256] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x1F, 0x11, 0x11, 0x11,
    0x1F, 0x11, 0x11, 0x11, 0xFF, 0xFF, 0x11, 0xFF, 0x00, 0xF0, 0x11, 0x0F, 0x00, 0xF0, 0x11, 0x0F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xF1, 0x00, 0x00, 0x00,
    0xF1, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xFF, 0xF0,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xF0, 0xFF, 0xF0,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xF0, 0xFF, 0xFF,
    0x00, 0xF0, 0x11, 0x0F, 0x00, 0xF0, 0x11, 0x0F, 0x00, 0xF0, 0x11, 0x0F, 0x00, 0xF0, 0x11, 0x0F,
    0x00, 0xF0, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0xFF, 0xF1, 0xFF, 0x00, 0x1F, 0x1F, 0xFF, 0x00, 0x1F, 0x1F, 0xFF, 0x00, 0xFF, 0xF1, 0xFF,
    0x00, 0xF0, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xF1, 0xF0, 0xF1, 0xFF, 0xF1, 0xFF, 0xF1, 0x1F, 0xF1, 0xF1, 0xF1, 0x1F, 0x1F, 0x1F, 0xFF, 0xFF,
    0xFF, 0xFF, 0x0F, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xF1, 0xFF, 0xF1, 0xF1, 0x11, 0xFF, 0x11, 0xFF, 0xFF, 0xFF, 0xF1, 0x0F, 0x11, 0xFF, 0xF1, 0x00,
    0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
