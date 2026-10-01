//! An extra entry on the Select Mode wheel: Survival ([`crate::survival`]),
//! with the Dual Strike pack, offline.
//!
//! The wheel (`ProcScr_MainMenu`, the `MainMenuCarousel*` procs, names from
//! the aw2bhr decompilation) turns six tiles: a table of six item ids
//! (`0x0861696C`: War Room 5, Battle Maps 2, Link 4, Design Room 3, Versus 1,
//! Campaign 0, read at `(position + 2) % 6`), each item with a big label
//! (the centre tile), a small one (the sides), a palette and a highlight
//! palette, and a help line per position. With the pack, offline, it turns
//! seven:
//!
//! - **Positions.** Every `DivRem(x, 6)` of the wheel's code (`movs r1, #6`
//!   before `bl DivRem` (`0x0808AAB0`) in `0x08080F00..0x08084C00`, found
//!   by scanning the ROM) becomes `DivRem(x, 7)`, the position's wrap at 5
//!   becomes 6 (the input loop `0x08081DF0`/`0x08081E2C`, the rotation
//!   `0x0808280C`/`0x08082830`, `SetMainMenuCarouselPosition` `0x08080F7E`),
//!   and the 34 words pointing at the item table point at a seven-entry
//!   copy (`[5, 8, 2, 4, 3, 1, 0]`: every other item keeps its position,
//!   Survival is position 6, between War Room and Battle Maps).
//! - **Item 8.** Its labels and palettes are tangoAW2's: traps at
//!   `LoadMainMenuCentreTileGraphic` (`0x080845A8`, the big label into OBJ
//!   VRAM `0x06013300`), `GetMainMenuTilePalette` (`0x08084864`),
//!   `GetMainMenuTileHighlightPalette` (`0x0808488C`) and
//!   `IsMainMenuTileComplete` (`0x08084858`, never complete). The small
//!   label sits in OBJ tiles 0x320.. (unused on this screen) with OBJ
//!   palette 13; the game draws an item's side tile at `0x1D8 + 32 * item`
//!   with palette `item + 2`, so at the sprite flush ([`remap`]) those of
//!   item 8 are pointed at ours.
//! - **Picking it.** Item 8 behaves like Battle Maps and Link (A starts the
//!   mode, no Continue/New): traps at the three `cmp r0, #4` that send an
//!   item there (`0x08081498` in `MainMenuCarouselWheel_Init`, `0x08081E94`
//!   and `0x08081EEE` in its input loop) answer 4 for item 8, and the
//!   mode store (`0x08081EF6`) stores the entry's mode (5, War Room New)
//!   and tells the entry it was picked.
//! - **Help line.** `MainMenuCarousel_DrawDescriptionText` (`0x08084600`)
//!   indexes a table by position with 6 and 7 for the Sound Room and Hard
//!   Campaign lines: the table (`0x08616FA4`) is copied with the entry's
//!   line at 6, those two become 7 and 8, and its Hard Campaign test (table
//!   index 0, Campaign) becomes index 6; the open item's choices' lines
//!   (`0x08616FB4`, two per line index) are copied likewise.
//!
//! Everything is switched every frame from the pack's state (the ROM image
//! is put back without it), so the wheel is AW2's own with the pack off.
//! One extra entry fits the free OBJ palette and tiles found on this screen;
//! a second would need its own (and the table and wraps for eight).

use mgba::core::Core;
use std::sync::OnceLock;

/// The new item's id (0..5 are the game's, 6 and 7 are the Sound Room's
/// and Hard Campaign's label graphics).
pub const ITEM: u8 = 8;
/// War Room, Battle Maps, Link, Design Room, Versus, Campaign, then ours.
const TABLE: [u8; 7] = [5, ITEM, 2, 4, 3, 1, 0];

const GAME_TABLE: u32 = 0x0861_696C;
const HELP_TABLE: u32 = 0x0861_6FA4;
const HELP_POOL: u32 = 0x0808_4680;
/// The open item's two choices' lines, two per position (the same 6 and 7
/// for the Sound Room and Hard Campaign).
const OPTIONS_TABLE: u32 = 0x0861_6FB4;
const OPTIONS_POOL: u32 = 0x0808_46C0;
const DIV_REM: u32 = 0x0808_AAB0;
const WHEEL_CODE: (u32, u32) = (0x0808_0F00, 0x0808_4C00);

/// tangoAW2's wheel data in the ROM image's free space.
const DATA: u32 = 0x08E4_0000;
const DATA_TABLE: u32 = DATA;
const DATA_HELP: u32 = DATA + 0x10;
const DATA_PALETTE: u32 = DATA + 0x40;
const DATA_HIGHLIGHT: u32 = DATA + 0x60;
const DATA_OPTIONS: u32 = DATA + 0x80;
const DATA_BIG: u32 = DATA + 0x100;
const DATA_SMALL: u32 = DATA + 0x900;
const SENTINEL: u32 = DATA + 0xFFC;
const MAGIC: u32 = 0x554E_454D; // "MENU"

/// The wheel's proc (`ProcScr_MainMenuC1`), running while Select Mode is up.
const MAIN_MENU: u32 = 0x0861_6990;
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;

const OBJ_VRAM: u32 = 0x0601_0000;
const CENTRE_VRAM: u32 = 0x0601_3300;
const SIDE_TILE_GAME: u32 = 0x1D8 + 32 * ITEM as u32;
const SIDE_TILE: u32 = 0x320;
const CENTRE_TILES: (u32, u32) = (0x198, 0x1D8);
const PALETTE: u32 = 13;
const GAME_PALETTE: u32 = ITEM as u32 + 2;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

/// (address, original, patched) halfwords besides the DivRem sites.
const HALVES: [(u32, u16, u16); 9] = [
    (0x0808_0F68, 0x1D08, 0x1D48), // SetMainMenuCarouselPosition: (i + 4) % 6 -> (i + 5) % 7
    (0x0808_1DF0, 0x2805, 0x2806), // UP: position 5 -> 0 becomes 6 -> 0
    (0x0808_1E2C, 0x2005, 0x2006), // DOWN: 0 -> 5 becomes 0 -> 6
    (0x0808_280C, 0x2805, 0x2806), // the rotation's own wraps
    (0x0808_2830, 0x2005, 0x2006),
    (0x0808_0F7E, 0x2905, 0x2906), // SetMainMenuCarouselPosition: i <= 6
    (0x0808_4630, 0x2506, 0x2507), // help: Sound Room line 6 -> 7
    (0x0808_464E, 0x2507, 0x2508), // help: Hard Campaign line 7 -> 8
    (0x0808_464A, 0x2800, 0x2806), // help: Campaign is table index 6
];

struct Built {
    divrem_sites: Vec<u32>,
    table_pools: Vec<u32>,
}

static BUILT: OnceLock<Built> = OnceLock::new();

fn scan(core: &Core) -> Built {
    let (lo, hi) = WHEEL_CODE;
    let mut code = vec![0u8; (hi - lo) as usize];
    core.raw_read_range(lo, -1, &mut code);
    let h = |o: usize| u16::from_le_bytes([code[o], code[o + 1]]);
    let mut divrem_sites = Vec::new();
    for o in (0..code.len() - 6).step_by(2) {
        if h(o) != 0x2106 {
            continue;
        }
        let (hi16, lo16) = (h(o + 2), h(o + 4));
        if hi16 >> 11 != 0x1E || lo16 >> 11 != 0x1F {
            continue;
        }
        let mut off = (((hi16 & 0x7FF) as i32) << 12) | (((lo16 & 0x7FF) as i32) << 1);
        if off & 0x40_0000 != 0 {
            off -= 0x80_0000;
        }
        let pc = lo + o as u32 + 2 + 4;
        if pc.wrapping_add(off as u32) == DIV_REM {
            divrem_sites.push(lo + o as u32);
        }
    }
    // Every word pointing at the item table, in the wheel's code and its
    // read-only data (0x081D938C..).
    let mut table_pools = Vec::new();
    for (from, to) in [(lo, hi), (0x081D_9300, 0x081D_9400)] {
        let mut b = vec![0u8; (to - from) as usize];
        core.raw_read_range(from, -1, &mut b);
        for o in (0..b.len()).step_by(4) {
            if u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) == GAME_TABLE {
                table_pools.push(from + o as u32);
            }
        }
    }
    Built { divrem_sites, table_pools }
}

// --- The label art -----------------------------------------------------------

/// The game's labels: (big, small) LZ77 per item.
const LABELS: u32 = 0x0861_6AC0;
/// Item palettes: 16 colours per item, the highlight ones from item 6.
const PALETTES: u32 = 0x0823_DC38;

fn label(core: &Core, item: u32, which: u32) -> Option<Vec<u8>> {
    let at = core.raw_read_32(LABELS + 8 * item + 4 * which, -1);
    let mut b = vec![0u8; 0x1000];
    core.raw_read_range(at, -1, &mut b);
    crate::ds_art::lz10(&b)
}

/// A label as pixels (128 wide), from tiles laid out as sprites `sw` tiles
/// wide, left to right.
fn to_pixels(data: &[u8], h_tiles: usize, sw: usize) -> Vec<Vec<u8>> {
    let mut px = vec![vec![0u8; 128]; 8 * h_tiles];
    let mut t = 0;
    for s in 0..16 / sw {
        for ty in 0..h_tiles {
            for tx in 0..sw {
                for y in 0..8 {
                    for x in 0..8 {
                        let b = data[32 * t + 4 * y + x / 2];
                        px[8 * ty + y][8 * (s * sw + tx) + x] = (b >> (4 * (x & 1))) & 15;
                    }
                }
                t += 1;
            }
        }
    }
    px
}

fn to_tiles(px: &[Vec<u8>], h_tiles: usize, sw: usize) -> Vec<u8> {
    let mut out = vec![0u8; 16 * h_tiles * 32];
    let mut t = 0;
    for s in 0..16 / sw {
        for ty in 0..h_tiles {
            for tx in 0..sw {
                for y in 0..8 {
                    for x in 0..8 {
                        let v = px[8 * ty + y][8 * (s * sw + tx) + x] & 15;
                        out[32 * t + 4 * y + x / 2] |= v << (4 * (x & 1));
                    }
                }
                t += 1;
            }
        }
    }
    out
}

/// The big "SURVIVAL": the letters of the game's own labels (item, first
/// and last column), on Campaign's plate. Letter colours (5..12) follow the
/// 16-pixel column bands every big label has.
const BIG_LETTERS: [(u32, usize, usize); 8] = [
    (3, 27, 37), // S of DESIGN ROOM
    (1, 84, 101), // U of VERSUS
    (3, 75, 85), // R of DESIGN ROOM
    (1, 6, 24), // V of VERSUS
    (3, 39, 45), // I of DESIGN ROOM
    (1, 6, 24), // V
    (2, 87, 99), // A of BATTLEMAPS
    (2, 51, 60), // L of BATTLEMAPS
];

fn big(core: &Core) -> Option<Vec<u8>> {
    let src: Vec<Vec<Vec<u8>>> = (0..4).map(|i| label(core, i, 0).map(|d| to_pixels(&d, 4, 8))).collect::<Option<_>>()?;
    let mut out = src[0].clone();
    for row in out.iter_mut().take(22).skip(2) {
        for v in row.iter_mut().take(124).skip(4) {
            *v = 1;
        }
    }
    let total: usize = BIG_LETTERS.iter().map(|&(_, a, b)| b - a + 1).sum::<usize>() + BIG_LETTERS.len() - 1;
    let mut x = 4 + (120 - total) / 2;
    for &(item, a, b) in &BIG_LETTERS {
        for sx in a..=b {
            for y in 2..22 {
                let mut v = src[item as usize][y][sx];
                if (5..=12).contains(&v) {
                    v = (5 + x / 16).min(12) as u8;
                }
                out[y][x] = v;
            }
            x += 1;
        }
        x += 1;
    }
    Some(to_tiles(&out, 4, 8))
}

/// A small label's letter cells: column runs between all-dark columns
/// inside its dark box (rows 4..10).
fn cells(px: &[Vec<u8>]) -> Vec<(usize, usize)> {
    let rows = 4..11;
    let dark = |x: usize| rows.clone().all(|y| px[y][x] == 1);
    let light = |x: usize| rows.clone().all(|y| px[y][x] == 5);
    let boxed: Vec<usize> = (4..124).filter(|&x| !light(x)).collect();
    let (Some(&lo), Some(&hi)) = (boxed.first(), boxed.last()) else { return Vec::new() };
    let mut out = Vec::new();
    let mut start = None;
    for x in lo..=hi + 1 {
        let d = x > hi || dark(x);
        match (d, start) {
            (false, None) => start = Some(x),
            (true, Some(s)) => {
                out.push((s, x - 1));
                start = None;
            }
            _ => {}
        }
    }
    out
}

fn small(core: &Core) -> Option<Vec<u8>> {
    let campaign = to_pixels(&label(core, 0, 1)?, 2, 4);
    let versus = to_pixels(&label(core, 1, 1)?, 2, 4);
    let link = to_pixels(&label(core, 4, 1)?, 2, 4);
    let (c, v, l) = (cells(&campaign), cells(&versus), cells(&link));
    if c.len() != 8 || v.len() != 6 || l.len() != 4 {
        return None;
    }
    // S U R V I V A L
    let letters: [(&Vec<Vec<u8>>, (usize, usize)); 8] =
        [(&versus, v[3]), (&versus, v[4]), (&versus, v[2]), (&versus, v[0]), (&campaign, c[5]), (&versus, v[0]), (&campaign, c[1]), (&link, l[0])];
    let mut out = campaign.clone();
    for row in out.iter_mut().take(11).skip(4) {
        for v in row.iter_mut().take(124).skip(4) {
            *v = 5;
        }
    }
    let total: usize = letters.iter().map(|(_, (a, b))| b - a + 1).sum::<usize>() + letters.len() + 1;
    let mut x = 4 + (120 - total) / 2;
    let dark_col = |out: &mut Vec<Vec<u8>>, x: usize| {
        for row in out.iter_mut().take(11).skip(4) {
            row[x] = 1;
        }
    };
    dark_col(&mut out, x);
    x += 1;
    for (src, (a, b)) in letters {
        for sx in a..=b {
            for y in 4..11 {
                out[y][x] = src[y][sx];
            }
            x += 1;
        }
        dark_col(&mut out, x);
        x += 1;
    }
    Some(to_tiles(&out, 2, 4))
}

/// Teal, from Campaign's blue and Link's green, colour by colour: the
/// lesser red, the greater green and blue.
fn palette(core: &Core, a: u32, b: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..16 {
        let ca = core.raw_read_16(PALETTES + 32 * a + 2 * i, -1);
        let cb = core.raw_read_16(PALETTES + 32 * b + 2 * i, -1);
        let mut c = 0u16;
        for ch in 0..3 {
            let va = (ca >> (5 * ch)) & 31;
            let vb = (cb >> (5 * ch)) & 31;
            let v = if ch == 0 { va.min(vb) } else { va.max(vb) };
            c |= v << (5 * ch);
        }
        out[2 * i as usize..2 * i as usize + 2].copy_from_slice(&c.to_le_bytes());
    }
    out
}

fn installed(core: &Core) -> bool {
    core.raw_read_32(SENTINEL, -1) == MAGIC
}

fn install(core: &mut Core, help_text: u16) -> bool {
    BUILT.get_or_init(|| scan(core));
    if installed(core) {
        return true;
    }
    let (Some(big), Some(small)) = (big(core), small(core)) else { return false };
    core.raw_write_range(DATA_TABLE, -1, &TABLE);
    // Help lines: positions 0..5, ours, then the Sound Room's and Hard
    // Campaign's.
    let mut help = Vec::new();
    for i in 0..6 {
        help.push(core.raw_read_16(HELP_TABLE + 2 * i, -1));
    }
    help.push(help_text);
    help.push(core.raw_read_16(HELP_TABLE + 12, -1));
    help.push(core.raw_read_16(HELP_TABLE + 14, -1));
    for (i, h) in help.iter().enumerate() {
        core.raw_write_16(DATA_HELP + 2 * i as u32, -1, *h);
    }
    // Choices' lines: positions 0..5, none for ours, then the two others'.
    let mut options = Vec::new();
    for i in 0..12 {
        options.push(core.raw_read_16(OPTIONS_TABLE + 2 * i, -1));
    }
    options.extend([0, 0]);
    for i in 12..16 {
        options.push(core.raw_read_16(OPTIONS_TABLE + 2 * i, -1));
    }
    for (i, h) in options.iter().enumerate() {
        core.raw_write_16(DATA_OPTIONS + 2 * i as u32, -1, *h);
    }
    core.raw_write_range(DATA_PALETTE, -1, &palette(core, 0, 4));
    core.raw_write_range(DATA_HIGHLIGHT, -1, &palette(core, 6, 10));
    core.raw_write_range(DATA_BIG, -1, &big);
    core.raw_write_range(DATA_SMALL, -1, &small);
    core.raw_write_32(SENTINEL, -1, MAGIC);
    true
}

/// The item at a wheel position (the game reads `table[(position + 2) %
/// n]`, n = 6, or 7 with Survival's entry).
pub fn item(core: &Core, position: u32) -> u8 {
    if active(core) {
        TABLE[(position as usize + 2) % 7]
    } else {
        core.raw_read_8(GAME_TABLE + (position + 2) % 6, -1)
    }
}

/// Whether the wheel has seven entries now (the patches are in).
pub fn active(core: &Core) -> bool {
    BUILT.get().is_some_and(|b| b.table_pools.first().is_some_and(|&p| core.raw_read_32(p, -1) == DATA_TABLE))
}

fn running(core: &Core) -> bool {
    (PROCS..PROCS_END).step_by(PROC_SIZE as usize).any(|p| core.raw_read_32(p, -1) == MAIN_MENU)
}

/// Every frame: the wheel gets the entry with the pack on (offline), and
/// is the game's own otherwise.
pub fn tick(core: &mut Core, on: bool, help_text: u16) {
    let on = on && install(core, help_text);
    let Some(built) = BUILT.get() else { return };
    if !on && !installed(core) {
        return;
    }
    let w16 = |core: &mut Core, at: u32, v: u16| {
        if core.raw_read_16(at, -1) != v {
            core.raw_write_16(at, -1, v);
        }
    };
    let w32 = |core: &mut Core, at: u32, v: u32| {
        if core.raw_read_32(at, -1) != v {
            core.raw_write_32(at, -1, v);
        }
    };
    for &at in &built.divrem_sites {
        w16(core, at, if on { 0x2107 } else { 0x2106 });
    }
    for &(at, old, new) in &HALVES {
        let now = core.raw_read_16(at, -1);
        if now == old || now == new {
            w16(core, at, if on { new } else { old });
        }
    }
    for &at in &built.table_pools {
        w32(core, at, if on { DATA_TABLE } else { GAME_TABLE });
    }
    w32(core, HELP_POOL, if on { DATA_HELP } else { HELP_TABLE });
    w32(core, OPTIONS_POOL, if on { DATA_OPTIONS } else { OPTIONS_TABLE });
    if on && running(core) {
        // The small label and its palette (the game never loads item 8's).
        let mut small = vec![0u8; 1024];
        core.raw_read_range(DATA_SMALL, -1, &mut small);
        let at = OBJ_VRAM + 32 * SIDE_TILE;
        let mut now = vec![0u8; 1024];
        core.raw_read_range(at, -1, &mut now);
        if now != small {
            core.raw_write_range(at, -1, &small);
        }
        let mut pal = [0u8; 32];
        core.raw_read_range(DATA_PALETTE, -1, &mut pal);
        for base in [PAL_BUFFER, PAL_RAM] {
            let mut now = [0u8; 32];
            core.raw_read_range(base + 0x200 + 32 * PALETTE, -1, &mut now);
            if now != pal {
                core.raw_write_range(base + 0x200 + 32 * PALETTE, -1, &pal);
            }
        }
    }
}

/// At the sprite flush ([`crate::branding::flush`]): item 8's side tiles
/// and its tile on its way to the centre use our tiles and palette.
pub fn remap(core: &mut Core, start: u32, end: u32) {
    if !active(core) || !running(core) {
        return;
    }
    let mut at = start;
    while at + 8 <= end {
        let a2 = core.raw_read_16(at + 4, -1) as u32;
        let (tile, pal) = (a2 & 0x3FF, a2 >> 12);
        if pal == GAME_PALETTE {
            let new = if (SIDE_TILE_GAME..SIDE_TILE_GAME + 32).contains(&tile) {
                Some(tile - SIDE_TILE_GAME + SIDE_TILE)
            } else if (CENTRE_TILES.0..CENTRE_TILES.1).contains(&tile) {
                Some(tile)
            } else {
                None
            };
            if let Some(t) = new {
                core.raw_write_16(at + 4, -1, ((PALETTE << 12) | (a2 & 0x0C00) | t) as u16);
            }
        }
        at += 8;
    }
}

// --- Traps -----------------------------------------------------------------------

fn ret(core: &mut Core, r0: u32) {
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, r0 as i32);
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

fn r0(core: &Core) -> u32 {
    core.gba().cpu().gpr(0) as u32
}

const CENTRE_GRAPHIC: u32 = 0x0808_45A8;
fn centre_graphic(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        let mut big = vec![0u8; 2048];
        core.raw_read_range(DATA_BIG, -1, &mut big);
        core.raw_write_range(CENTRE_VRAM, -1, &big);
        ret(core, 0);
    }
}

const TILE_COMPLETE: u32 = 0x0808_4858;
fn tile_complete(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        ret(core, 0);
    }
}

const TILE_PALETTE: u32 = 0x0808_4864;
fn tile_palette(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        ret(core, DATA_PALETTE);
    }
}

const TILE_HIGHLIGHT: u32 = 0x0808_488C;
fn tile_highlight(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        ret(core, DATA_HIGHLIGHT);
    }
}

/// `cmp r0, #4` with the item in r0: item 8 answers as Link.
const LIKE_LINK: [u32; 2] = [0x0808_1498, 0x0808_1E94];
fn like_link(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        core.gba_mut().cpu_mut().set_gpr(0, 4);
    }
}

/// The last `cmp r0, #4` before the mode is stored: item 8 is picked.
const PICKED: u32 = 0x0808_1EEE;
fn picked(core: &mut Core) {
    if active(core) && r0(core) == ITEM as u32 {
        core.gba_mut().cpu_mut().set_gpr(0, 4);
        core.raw_write_8(crate::survival::MENU_PICKED, -1, 1);
    }
}

/// `str r0, [r1]`: the mode (`0x030033FC`) Link would start; ours is the
/// War Room's New (5), [`crate::survival`] taking over from there.
const MODE_STORE: u32 = 0x0808_1EF6;
const MODE: u32 = 5;
fn mode_store(core: &mut Core) {
    if active(core) && core.raw_read_8(crate::survival::MENU_PICKED, -1) != 0 {
        core.gba_mut().cpu_mut().set_gpr(0, MODE as i32);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (CENTRE_GRAPHIC, Box::new(centre_graphic)),
        (TILE_COMPLETE, Box::new(tile_complete)),
        (TILE_PALETTE, Box::new(tile_palette)),
        (TILE_HIGHLIGHT, Box::new(tile_highlight)),
        (LIKE_LINK[0], Box::new(like_link)),
        (LIKE_LINK[1], Box::new(like_link)),
        (PICKED, Box::new(picked)),
        (MODE_STORE, Box::new(mode_store)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_keeps_every_position() {
        // (position + 2) % 7 in the new table gives what (position + 2) % 6
        // gave in the game's, for positions 0..5.
        const GAME: [u8; 6] = [0, 5, 2, 4, 3, 1];
        for p in 0..6 {
            assert_eq!(TABLE[(p + 2) % 7], GAME[(p + 2) % 6]);
        }
        assert_eq!(TABLE[(6 + 2) % 7], ITEM);
        assert!(SIDE_TILE + 32 <= 0x3A0);
    }
}
