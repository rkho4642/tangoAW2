//! Dual Strike's tag pairs on screen beyond the battle panel ([`crate::tag`],
//! with the Dual Strike pack), its words read from the .nds at run time:
//!
//! - **The Tag Power's screen.** Choosing Tag (a human army's or the
//!   computer's), after the first CO's quote and before AW2's Super Power
//!   screen: a band across the map as AW2's own SUPER POWER band (white
//!   between red stripes), the two COs' portraits at its ends, the pair's
//!   Tag Power name between them (a special pair's own, "Power Wrench";
//!   Dual Strike's "Dual Strike" for any other) and "POWER 110%", the
//!   pair's compatibility, as Dual Strike's tag screen shows them. It holds
//!   the power's script where AW2's waits for the quote to close
//!   (`sub_08039914`'s test), [`INTRO_FRAMES`] frames.
//! - **Change.** The map menu's Change runs a script of its own (in ROM,
//!   [`SCRIPT_CHANGE`]): the menu closes, the incoming CO says its tag-in
//!   line (Dual Strike's CO record +0x34 or +0x38, in AW2's quote box,
//!   `sub_08019818`), the band shows "CO SWAP" with the outgoing CO left
//!   and the incoming right ([`SWAP_FRAMES`] frames), the COs swap and the
//!   turn ends (`MapMenu_End`).
//! - **Victory.** A special pair's army winning: the results screen's
//!   quote (`GetVictoryQuoteTextId`, `0x0807A3AC`) is the pair's exchange,
//!   two of its four victory lines (its CO record's list, the active CO's
//!   entry), the active CO's line then the partner's.
//! - **The CO page's TAG box.** The CO page (the map menu's CO) gets a page
//!   after the Super Power's (DOWN from it, UP back): "TAG" and the CO's
//!   special partners, each with its star rating (Dual Strike's 1..3, in
//!   AW2's small star tiles), as Dual Strike's CO page's TAG box.
//!
//! The band is drawn on BG0 (the dialogue layer, free between the quote
//! and the power's screen): 128 tiles at its character base + `0x5600`
//! (the space crate::power_anim uses during a strike), BG palettes 8..10
//! (saved and put back), its map's rows 6..13 (cleared after). RAM:
//! [`STATE`] (`0x0203F300..0x0203F3FF`). Text ids 0x7305..0x7307, their
//! strings in ROM after crate::tag's (`0x08781000..`).

use mgba::core::Core;

use crate::tag;

// --- RAM ---------------------------------------------------------------------------

pub const STATE: u32 = 0x0203_F300;
/// The band shown: 0 none, 1 the Tag Power's, 2 Change's.
const KIND: u32 = STATE;
const ARMY: u32 = STATE + 1;
const FRAME: u32 = STATE + 2;
/// An army whose Tag Power's band is still to come (0 none).
const PENDING: u32 = STATE + 4;
/// The CO page shows the TAG page (1).
const TAG_PAGE: u32 = STATE + 5;
const SAVED_HOFS: u32 = STATE + 6;
const SAVED_VOFS: u32 = STATE + 8;
/// The CO page's star tiles borrowed (1), and them as they were.
const STARS_BORROWED: u32 = STATE + 0x0A;
/// The band's outgoing and incoming COs (Change).
const SWAP_FROM: u32 = STATE + 0x0B;
const SWAP_TO: u32 = STATE + 0x0C;
/// The TAG page's CO; whether the sprites were on before the band.
const PAGE_CO: u32 = STATE + 0x0D;
const SAVED_OBJ: u32 = STATE + 0x0E;
const SAVED_PALETTES: u32 = STATE + 0x10; // 3 x 32
const SAVED_STARS: u32 = STATE + 0x70; // 2 x 32
#[cfg(test)]
const STATE_END: u32 = STATE + 0xD0;

pub const INTRO_FRAMES: u16 = 150;
pub const SWAP_FRAMES: u16 = 100;

// --- ROM ---------------------------------------------------------------------------

const TEXT_TABLE: u32 = 0x0861_0A38;
pub const TEXT_TAGIN: u16 = 0x7305;
pub const TEXT_VICTORY: u16 = 0x7306;
pub const TEXT_TAGBOX: u16 = 0x7307;
const STRINGS: u32 = tag::ROM + 0x1000;
const TAGIN_AT: u32 = STRINGS;
const VICTORY_AT: u32 = STRINGS + 0x100;
const TAGBOX_AT: u32 = STRINGS + 0x300;
/// The TAG page's header, "TAG", in place of the Super Power's name (the
/// header keeps its Super Power icon).
const TAGHEAD_AT: u32 = STRINGS + 0x3F0;
const TAG_HEADER: &[u8] = b"TAG\0\0\0";
const STRING_MAX: usize = 0xF8;
/// Change's script (AW2's proc commands, 8 bytes each).
pub const SCRIPT_CHANGE: u32 = tag::ROM + 0x1400;

const CLOSE_TOP_MENU: u32 = 0x0801_A168;
const LOCK_MAP: u32 = 0x0803_4F7C;
const UNLOCK_MAP: u32 = 0x0803_4F8C;
const WAIT_QUOTE: u32 = 0x0803_9914;
const MAP_MENU_END: u32 = 0x0802_CF6C;
const SHOW_QUOTE: u32 = 0x0801_9818;
const PROC_BREAK: u32 = 0x0801_CB20;

const OP_END: u16 = 0x00;
const OP_CALL: u16 = 0x02;
const OP_REPEAT: u16 = 0x03;
const OP_SLEEP: u16 = 0x0E;

fn cmd(op: u16, arg: u16, ptr: u32) -> [u8; 8] {
    let mut c = [0u8; 8];
    c[0..2].copy_from_slice(&op.to_le_bytes());
    c[2..4].copy_from_slice(&arg.to_le_bytes());
    c[4..8].copy_from_slice(&ptr.to_le_bytes());
    c
}

/// Change's script, its stubs crate::tag's ([`tag::stub_addr`]).
pub fn install(core: &mut Core) {
    let script = [
        cmd(OP_CALL, 0, CLOSE_TOP_MENU | 1),
        cmd(OP_CALL, 0, LOCK_MAP | 1),
        cmd(OP_SLEEP, 2, 0),
        cmd(OP_CALL, 0, tag::stub_addr(tag::S_QUOTE) | 1),
        cmd(OP_REPEAT, 0, WAIT_QUOTE | 1),
        cmd(OP_SLEEP, 1, 0),
        cmd(OP_REPEAT, 0, tag::stub_addr(tag::S_SWAP_FRAME) | 1),
        cmd(OP_CALL, 0, UNLOCK_MAP | 1),
        cmd(OP_CALL, 0, tag::stub_addr(tag::S_SWAP) | 1),
        cmd(OP_CALL, 0, MAP_MENU_END | 1),
        cmd(OP_END, 0, 0),
    ];
    let bytes: Vec<u8> = script.iter().flatten().copied().collect();
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(SCRIPT_CHANGE, -1, &mut now);
    if now != bytes {
        core.raw_write_range(SCRIPT_CHANGE, -1, &bytes);
    }
    let mut head = [0u8; 6];
    core.raw_read_range(TAGHEAD_AT, -1, &mut head);
    if head[..] != TAG_HEADER[..] {
        core.raw_write_range(TAGHEAD_AT, -1, TAG_HEADER);
    }
    for (id, at) in [(TEXT_TAGIN, TAGIN_AT), (TEXT_VICTORY, VICTORY_AT), (TEXT_TAGBOX, TAGBOX_AT)] {
        let entry = TEXT_TABLE + 4 * id as u32;
        if core.raw_read_32(entry, -1) != at {
            core.raw_write_32(entry, -1, at);
        }
    }
}

fn set_string(core: &mut Core, at: u32, s: &[u8]) {
    let mut b: Vec<u8> = s.iter().copied().take(STRING_MAX).collect();
    b.push(0);
    core.raw_write_range(at, -1, &b);
}

// --- Dual Strike's words -----------------------------------------------------------------

fn ds_text(r: u32) -> Option<Vec<u8>> {
    let pack = crate::ds_pack::pack()?;
    let ds = crate::ds_campaign_data::Ds::from_pack(pack)?;
    ds.text(r)
}

const DS_RECORDS: u32 = 0x0215_360C;
const DS_RECORD: u32 = 0x220;
const DS_TAG_IN: [u32; 2] = [0x34, 0x38];

fn ds_record_word(co: u8, off: u32) -> Option<u32> {
    let d = tag::ds_id(co)?;
    let b = crate::ds_pack::pack()?.arm9_at(DS_RECORDS + DS_RECORD * d as u32 + off, 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// A special pair's texts: its Tag Power's name and its four victory lines
/// (the entry of `a`'s record for partner `b`).
pub fn pair_texts(a: u8, b: u8) -> Option<(Vec<u8>, [Vec<u8>; 4])> {
    let (_, ptr) = tag::special_pair(a, b)?;
    let pack = crate::ds_pack::pack()?;
    let w = pack.arm9_at(ptr, 20)?;
    let id = |k: usize| u32::from_le_bytes([w[4 * k], w[4 * k + 1], w[4 * k + 2], w[4 * k + 3]]);
    let name = ds_text(id(4))?;
    let lines = [ds_text(id(0))?, ds_text(id(1))?, ds_text(id(2))?, ds_text(id(3))?];
    Some((name, lines))
}

/// Dual Strike's text, its pauses kept, for AW2's boxes (its own codes:
/// `\r` a line break, 0x0E a pause, as AW2's quotes).
fn clean(t: &[u8]) -> Vec<u8> {
    t.iter().copied().filter(|&c| c == b'\r' || c == 0x0E || (0x20..0x7F).contains(&c)).collect()
}

/// A line of text as one line (its breaks and pauses spaces / gone).
fn flat(t: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for &c in t {
        match c {
            b'\r' => {
                if out.last() != Some(&b' ') {
                    out.push(b' ');
                }
            }
            0x20..=0x7E => out.push(c),
            _ => {}
        }
    }
    while out.last() == Some(&b' ') {
        out.pop();
    }
    out
}

/// The results screen's quote box: its line's room in pixels.
const QUOTE_PIXELS: u32 = 104;

/// A line broken (`\r`) at the last space that keeps the first part in
/// the box's line.
fn wrap2(core: &Core, t: &[u8]) -> Vec<u8> {
    if text_width(core, t) <= QUOTE_PIXELS {
        return t.to_vec();
    }
    let cut = (0..t.len()).rev().filter(|&i| t[i] == b' ').find(|&i| text_width(core, &t[..i]) <= QUOTE_PIXELS);
    match cut {
        Some(i) => {
            let mut out = t[..i].to_vec();
            out.push(b'\r');
            out.extend_from_slice(&t[i + 1..]);
            out
        }
        None => t.to_vec(),
    }
}

fn text_width(core: &Core, s: &[u8]) -> u32 {
    s.iter().map(|&c| core.raw_read_8(WIDTHS + c as u32, -1) as u32 + 1).sum::<u32>().saturating_sub(1)
}

/// One of two by the day (Dual Strike picks one of a CO's two tag-in lines
/// and one of a pair's two exchanges; here the day decides, the same on
/// every peer).
fn pick(core: &Core) -> usize {
    core.raw_read_16(DAY, -1) as usize % 2
}
const DAY: u32 = 0x0300_4080;

/// An AW2 CO's name, from the game's own text (its CO table row +0x00).
fn co_name(core: &Core, co: u8) -> Vec<u8> {
    let table = core.raw_read_32(tag::CO_TABLE_POOL, -1);
    let id = core.raw_read_32(table + 0x104 * co as u32, -1);
    let at = core.raw_read_32(TEXT_TABLE + 4 * id, -1);
    let mut out = Vec::new();
    if (0x0800_0000..0x0A00_0000).contains(&at) {
        for k in 0..16 {
            let c = core.raw_read_8(at + k, -1);
            if c == 0 {
                break;
            }
            out.push(c);
        }
    }
    out
}

// --- The band ------------------------------------------------------------------------

const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
const BG0CNT: u32 = 0x0300_2B6C;
const DISPCNT: u32 = 0x0300_30CC;
const DISPCNT_IO: u32 = 0x0400_0000;
const BG0HOFS: u32 = 0x0300_1FF8;
const BG0VOFS: u32 = 0x0300_1418;
const VRAM: u32 = 0x0600_0000;
const TILE_OFFSET: u32 = 0x5600;
const BAND_PALETTES: [u32; 3] = [8, 9, 10];
const ROW_TOP: u32 = 6;
const ROW_BOTTOM: u32 = 13;
/// The band's own colours (palette 10): the white, AW2's banner red, the
/// dark line, the name's yellow.
const BAND_COLOURS: [(usize, u16); 5] = [(1, 0x7FFF), (2, 0x0C5F), (3, 0x0842), (4, 0x03FF), (5, 0x6F7B)];
const C_WHITE: u8 = 1;
const C_RED: u8 = 2;
const C_DARK: u8 = 3;
const C_YELLOW: u8 = 4;

fn bg0(core: &Core) -> (u32, u32) {
    let cnt = core.raw_read_16(BG0CNT, -1) as u32;
    (VRAM + ((cnt >> 2) & 3) * 0x4000, VRAM + ((cnt >> 8) & 0x1F) * 0x800)
}

fn write_palette(core: &mut Core, pal: u32, p: &[u8; 32]) {
    for base in [PAL_BUFFER, PAL_RAM] {
        core.raw_write_range(base + 32 * pal, -1, p);
    }
}

/// Pixels (w x h, a byte each) as 4bpp tiles, row by row.
fn tiles_of(px: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for ty in 0..h / 8 {
        for tx in 0..w / 8 {
            for y in 0..8 {
                for x in (0..8).step_by(2) {
                    let a = px[(8 * ty + y) * w + 8 * tx + x] & 15;
                    let b = px[(8 * ty + y) * w + 8 * tx + x + 1] & 15;
                    out.push(a | b << 4);
                }
            }
        }
    }
    out
}

const GLYPHS: u32 = 0x084C_32E4;
const WIDTHS: u32 = 0x084C_36E4;

/// Text in AW2's font, centred in `w` x 16 pixels on `ground`, its ink
/// and (if any) an outline.
fn text_px(core: &Core, s: &[u8], w: usize, ground: u8, ink: u8, outline: Option<u8>) -> Vec<u8> {
    let mut px = vec![ground; w * 16];
    let adv = |c: u8| core.raw_read_8(WIDTHS + c as u32, -1) as i32 + 1;
    let total: i32 = s.iter().map(|&c| if c == b' ' { 4 } else { adv(c) }).sum();
    let mut x = ((w as i32 - total) / 2).max(1);
    let mut mask = vec![false; w * 16];
    for &c in s {
        if c == b' ' {
            x += 4;
            continue;
        }
        let cw = core.raw_read_8(WIDTHS + c as u32, -1) as usize;
        let at = core.raw_read_32(GLYPHS + 4 * c as u32, -1);
        if (0x0800_0000..0x0A00_0000).contains(&at) {
            let stride = cw.div_ceil(2);
            for r in 0..12usize {
                for cx in 0..cw {
                    let b = core.raw_read_8(at + (stride * (3 + r) + cx / 2) as u32, -1);
                    let v = (b >> (4 * (cx & 1))) & 15;
                    let (xx, yy) = (x + cx as i32, 2 + r as i32);
                    if v == 0xA && (0..w as i32).contains(&xx) && (0..16).contains(&yy) {
                        mask[yy as usize * w + xx as usize] = true;
                    }
                }
            }
        }
        x += cw as i32 + 1;
    }
    for y in 0..16i32 {
        for x in 0..w as i32 {
            let i = y as usize * w + x as usize;
            if mask[i] {
                px[i] = ink;
            } else if let Some(o) = outline {
                let near = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (xx, yy) = (x + dx, y + dy);
                        (0..w as i32).contains(&xx) && (0..16).contains(&yy) && mask[yy as usize * w + xx as usize]
                    })
                });
                if near {
                    px[i] = o;
                }
            }
        }
    }
    px
}

/// The band's tiles and map: (tile number in the 128, pixels) per cell.
struct Band {
    tiles: Vec<u8>,
    /// (column, row, tile index within ours, palette, flipped) per cell drawn.
    cells: Vec<(u32, u32, u32, u32, bool)>,
    palettes: [[u8; 32]; 3],
}

fn portrait(core: &Core, co: u8) -> (Vec<u8>, [u8; 32]) {
    crate::tag_ui::portrait48(core, co)
}

fn build_band(core: &Core, left: u8, right: u8, title: &[u8], info: &[u8]) -> Band {
    let mut tiles = Vec::new();
    let mut cells = Vec::new();
    // Tile 0: white; 1: the red stripe (a dark line at its band edge).
    let white = vec![C_WHITE; 64];
    tiles.extend(tiles_of(&white, 8, 8));
    let mut red = vec![C_RED; 64];
    for x in 0..8 {
        red[7 * 8 + x] = C_DARK;
    }
    tiles.extend(tiles_of(&red, 8, 8));
    let mut red_bottom = vec![C_RED; 64];
    for x in 0..8 {
        red_bottom[x] = C_DARK;
    }
    tiles.extend(tiles_of(&red_bottom, 8, 8));
    for col in 0..30u32 {
        cells.push((col, ROW_TOP, 1, 10, false));
        cells.push((col, ROW_BOTTOM, 2, 10, false));
        for row in ROW_TOP + 1..ROW_BOTTOM {
            cells.push((col, row, 0, 10, false));
        }
    }
    let mut palettes = [[0u8; 32]; 3];
    // The portraits: 6x6 tiles, rows 7..12, at columns 1 and 23, the
    // right one mirrored (the two face each other, as on Dual Strike's).
    for (k, (co, col0, flip)) in [(left, 1u32, false), (right, 23u32, true)].into_iter().enumerate() {
        let (t, p) = portrait(core, co);
        palettes[k] = p;
        let base = (tiles.len() / 32) as u32;
        tiles.extend_from_slice(&t);
        for r in 0..6u32 {
            for c in 0..6u32 {
                let src = if flip { 5 - c } else { c };
                cells.push((col0 + c, ROW_TOP + 1 + r, base + 6 * r + src, 8 + k as u32, flip));
            }
        }
    }
    // The title (16 x 2 tiles, columns 7..22, rows 8..9) and the line under
    // it (10 x 2 tiles, columns 10..19, rows 10..11).
    for (s, w, col0, row0, ink, outline) in [(title, 128usize, 7u32, 8u32, C_YELLOW, Some(C_DARK)), (info, 80, 10, 10, C_DARK, None)] {
        let px = text_px(core, s, w, C_WHITE, ink, outline);
        let base = (tiles.len() / 32) as u32;
        tiles.extend(tiles_of(&px, w, 16));
        let cols = (w / 8) as u32;
        for r in 0..2u32 {
            for c in 0..cols {
                cells.push((col0 + c, row0 + r, base + cols * r + c, 10, false));
            }
        }
    }
    let mut own = [0u8; 32];
    for (i, c) in BAND_COLOURS {
        own[2 * i..2 * i + 2].copy_from_slice(&c.to_le_bytes());
    }
    palettes[2] = own;
    Band { tiles, cells, palettes }
}

fn draw_band(core: &mut Core, band: &Band) {
    let (chars, screen) = bg0(core);
    let first = TILE_OFFSET / 32;
    let tiles = &band.tiles[..band.tiles.len().min(128 * 32)];
    let mut now = vec![0u8; tiles.len()];
    core.raw_read_range(chars + TILE_OFFSET, -1, &mut now);
    if now != tiles {
        core.raw_write_range(chars + TILE_OFFSET, -1, tiles);
    }
    for &(col, row, t, pal, flip) in &band.cells {
        core.raw_write_16(screen + 2 * (32 * row + col), -1, ((first + t) | (flip as u32) << 10 | pal << 12) as u16);
    }
    core.raw_write_16(BG0HOFS, -1, 0);
    core.raw_write_16(BG0VOFS, -1, 0);
    for reg in [DISPCNT, DISPCNT_IO] {
        let d = core.raw_read_16(reg, -1);
        if d & 1 << 12 != 0 {
            core.raw_write_16(reg, -1, d & !(1 << 12));
        }
    }
}

fn show_band(core: &mut Core, band: &Band) {
    let d = core.raw_read_16(DISPCNT, -1);
    core.raw_write_8(SAVED_OBJ, -1, ((d >> 12) & 1) as u8);
    for (k, pal) in BAND_PALETTES.iter().enumerate() {
        let mut old = [0u8; 32];
        core.raw_read_range(PAL_BUFFER + 32 * pal, -1, &mut old);
        core.raw_write_range(SAVED_PALETTES + 32 * k as u32, -1, &old);
        write_palette(core, *pal, &band.palettes[k]);
    }
    core.raw_write_16(SAVED_HOFS, -1, core.raw_read_16(BG0HOFS, -1));
    core.raw_write_16(SAVED_VOFS, -1, core.raw_read_16(BG0VOFS, -1));
    // (The map's sprites, units and cursor, off while the band shows.)
    draw_band(core, band);
}

fn hide_band(core: &mut Core) {
    let (_, screen) = bg0(core);
    for row in ROW_TOP..=ROW_BOTTOM {
        core.raw_write_range(screen + 2 * 32 * row, -1, &[0u8; 64]);
    }
    for (k, pal) in BAND_PALETTES.iter().enumerate() {
        let mut old = [0u8; 32];
        core.raw_read_range(SAVED_PALETTES + 32 * k as u32, -1, &mut old);
        write_palette(core, *pal, &old);
    }
    core.raw_write_16(BG0HOFS, -1, core.raw_read_16(SAVED_HOFS, -1));
    core.raw_write_16(BG0VOFS, -1, core.raw_read_16(SAVED_VOFS, -1));
    if core.raw_read_8(SAVED_OBJ, -1) == 1 {
        for reg in [DISPCNT, DISPCNT_IO] {
            let d = core.raw_read_16(reg, -1);
            core.raw_write_16(reg, -1, d | 1 << 12);
        }
    }
    core.raw_write_8(KIND, -1, 0);
}

/// The pair's band: its Tag Power's name (a special pair's, else "Dual
/// Strike") and "POWER 1xx%".
fn intro_band(core: &Core, a: u8, b: u8) -> Band {
    let name = pair_texts(a, b).map(|(n, _)| clean(&n)).unwrap_or_else(|| b"Dual Strike".to_vec());
    let info = format!("POWER {}%", tag::compatibility(a, b));
    build_band(core, a, b, &name, info.as_bytes())
}

fn swap_band(core: &Core, from: u8, to: u8) -> Band {
    build_band(core, from, to, b"CO SWAP", &co_name(core, to))
}

/// One frame of the band of `kind`; true once it is over (and taken away).
fn band_frame(core: &mut Core, kind: u8, frames: u16) -> bool {
    if core.raw_read_8(KIND, -1) != kind {
        let army = core.raw_read_8(ARMY, -1) as u32;
        let band = if kind == 1 {
            let a = tag::army_co_of(core, army);
            let Some(b) = tag::partner(core, army) else { return true };
            intro_band(core, a, b)
        } else {
            let (from, to) = (core.raw_read_8(SWAP_FROM, -1), core.raw_read_8(SWAP_TO, -1));
            swap_band(core, from, to)
        };
        core.raw_write_8(KIND, -1, kind);
        core.raw_write_16(FRAME, -1, 0);
        show_band(core, &band);
        return false;
    }
    let f = core.raw_read_16(FRAME, -1) + 1;
    core.raw_write_16(FRAME, -1, f);
    if f >= frames {
        hide_band(core);
        return true;
    }
    // Kept in place: the tiles (other text may borrow the space), the
    // scroll and the sprites off.
    let army = core.raw_read_8(ARMY, -1) as u32;
    let band = if kind == 1 {
        let a = tag::army_co_of(core, army);
        let Some(b) = tag::partner(core, army) else { return false };
        intro_band(core, a, b)
    } else {
        swap_band(core, core.raw_read_8(SWAP_FROM, -1), core.raw_read_8(SWAP_TO, -1))
    };
    draw_band(core, &band);
    false
}

// --- The Tag Power's band ------------------------------------------------------------

/// A Tag Power is chosen (crate::tag): its band comes after the quote.
pub fn tag_chosen(core: &mut Core, army: u32) {
    if crate::ds_pack::pack().is_some() {
        core.raw_write_8(PENDING, -1, army as u8);
    }
}

/// `sub_08039914` after its test of the quote box (r0: open): while the
/// band plays the script waits (no `Proc_Break`).
const WAIT_QUOTE_TEST: u32 = 0x0803_991C;
const WAIT_QUOTE_DONE: u32 = 0x0803_9928;
fn wait_quote(core: &mut Core) {
    if !crate::ds_weather::is_on(core) {
        return;
    }
    let pending = core.raw_read_8(PENDING, -1);
    if pending == 0 && core.raw_read_8(KIND, -1) != 1 {
        return;
    }
    if core.gba().cpu().gpr(0) & 0xFF != 0 {
        return;
    }
    if pending != 0 {
        core.raw_write_8(ARMY, -1, pending);
        core.raw_write_8(PENDING, -1, 0);
    }
    if !band_frame(core, 1, INTRO_FRAMES) {
        core.gba_mut().cpu_mut().set_thumb_pc(WAIT_QUOTE_DONE);
    }
}

// --- Change ------------------------------------------------------------------------------

/// Change chosen (crate::tag's action): its script starts (`Proc_Start`,
/// as `PayForCoPower` starts the power's).
const PROC_START: u32 = 0x0801_C8F4;
pub fn change_chosen(core: &mut Core, army: u32) {
    let from = tag::army_co_of(core, army);
    let to = tag::partner(core, army).unwrap_or(from);
    core.raw_write_8(ARMY, -1, army as u8);
    core.raw_write_8(SWAP_FROM, -1, from);
    core.raw_write_8(SWAP_TO, -1, to);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, SCRIPT_CHANGE as i32);
    cpu.set_gpr(1, 3);
    cpu.set_thumb_pc(PROC_START);
}

/// The script's quote: the incoming CO's tag-in line in AW2's quote box.
pub fn quote(core: &mut Core) {
    let to = core.raw_read_8(SWAP_TO, -1);
    let line = ds_record_word(to, DS_TAG_IN[pick(core)]).and_then(ds_text).map(|t| clean(&t));
    let line = line.unwrap_or_else(|| b"It's my turn now!".to_vec());
    set_string(core, TAGIN_AT, &line);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, TEXT_TAGIN as i32);
    cpu.set_gpr(1, to as i32);
    cpu.set_gpr(2, 0);
    cpu.set_thumb_pc(SHOW_QUOTE);
}

/// The script's band: a frame (a `PROC_REPEAT`; `Proc_Break` when over).
pub fn swap_frame(core: &mut Core) {
    if band_frame(core, 2, SWAP_FRAMES) {
        core.gba_mut().cpu_mut().set_thumb_pc(PROC_BREAK);
    } else {
        let cpu = core.gba_mut().cpu_mut();
        let lr = cpu.gpr(14) as u32;
        cpu.set_thumb_pc(lr & !1);
    }
}

// --- Victory ---------------------------------------------------------------------------

/// `GetVictoryQuoteTextId(co, mission)`: a special pair's army that won
/// says the pair's exchange.
const VICTORY_QUOTE: u32 = 0x0807_A3AC;
fn victory_quote(core: &mut Core) {
    if !crate::ds_weather::is_on(core) {
        return;
    }
    let co = core.gba().cpu().gpr(0) as u8;
    let Some(army) = (1..=5u32).find(|&a| tag::army_co_of(core, a) == co && tag::partner(core, a).is_some()) else { return };
    let Some(b) = tag::partner(core, army) else { return };
    let Some((_, lines)) = pair_texts(co, b) else { return };
    // One box, two lines: the active CO's line, then the partner's after
    // its name. Of Dual Strike's two exchanges the day's, or the other if
    // only that one fits AW2's box; neither: the active CO's line alone.
    let partner_name = co_name(core, b);
    let fits = |k: usize| {
        let mut second = partner_name.clone();
        second.extend_from_slice(b": ");
        second.extend_from_slice(&flat(&lines[k + 1]));
        text_width(core, &flat(&lines[k])) <= QUOTE_PIXELS && text_width(core, &second) <= QUOTE_PIXELS
    };
    let first = 2 * pick(core);
    let k = [first, 2 - first].into_iter().find(|&k| fits(k));
    let mut t = flat(&lines[k.unwrap_or(first)]);
    if k.is_none() {
        // The active CO's line alone, over the box's two lines.
        t = wrap2(core, &t);
    }
    if let Some(k) = k {
        t.push(b'\r');
        t.extend_from_slice(&partner_name);
        t.extend_from_slice(b": ");
        t.extend_from_slice(&flat(&lines[k + 1]));
    }
    set_string(core, VICTORY_AT, &t);
    let cpu = core.gba_mut().cpu_mut();
    cpu.set_gpr(0, TEXT_VICTORY as i32);
    let lr = cpu.gpr(14) as u32;
    cpu.set_thumb_pc(lr & !1);
}

// --- The CO page's TAG page ------------------------------------------------------------

/// The CO page (`0x080852A8` draws its page `[0x03005940]`; the input at
/// `0x08084C90`): page 3 (the Super Power) and DOWN shows the TAG page
/// first; DOWN again goes on (the unit charts), UP back.
const PAGE: u32 = 0x0300_5940;
const PAGE_OPENED: u32 = 0x0808_49BC;
const PAGE_DOWN: u32 = 0x0808_4D0E;
const PAGE_UP: u32 = 0x0808_4CC4;
const PAGE_REDRAW_UP: u32 = 0x0808_4CDA;
const PAGE_REDRAW_DOWN: u32 = 0x0808_4D24;
const PAGE_TEXT: u32 = 0x0808_52DC;

fn page_opened(core: &mut Core) {
    if core.raw_read_8(TAG_PAGE, -1) != 0 {
        core.raw_write_8(TAG_PAGE, -1, 0);
    }
}

fn page_down(core: &mut Core) {
    if !crate::ds_weather::is_on(core) {
        return;
    }
    if core.raw_read_8(TAG_PAGE, -1) == 1 {
        core.raw_write_8(TAG_PAGE, -1, 0);
        return;
    }
    if core.raw_read_32(PAGE, -1) == 3 {
        core.raw_write_8(TAG_PAGE, -1, 1);
        core.gba_mut().cpu_mut().set_thumb_pc(PAGE_REDRAW_DOWN);
    }
}

fn page_up(core: &mut Core) {
    if crate::ds_weather::is_on(core) && core.raw_read_8(TAG_PAGE, -1) == 1 {
        core.raw_write_8(TAG_PAGE, -1, 0);
        core.gba_mut().cpu_mut().set_thumb_pc(PAGE_REDRAW_UP);
    }
}

/// The CO the page shows (its army: the proc's +0x66).
fn page_co(core: &Core) -> Option<u8> {
    let proc = core.gba().cpu().gpr(6) as u32;
    if !(0x0200_0000..0x0400_0000).contains(&proc) {
        return None;
    }
    let army = core.raw_read_16(proc + 0x66, -1) as u32;
    (1..=5).contains(&army).then(|| tag::army_co_of(core, army))
}

/// The CO's special partners and their stars, Dual Strike's order.
pub fn partners_of(core: &Core, co: u8) -> Vec<(u8, u8)> {
    let _ = core;
    let Some(d) = tag::ds_id(co) else { return Vec::new() };
    let Some(pack) = crate::ds_pack::pack() else { return Vec::new() };
    let Some(list) = pack.arm9_at(DS_RECORDS + DS_RECORD * d as u32 + 0x6C, 0x18) else { return Vec::new() };
    let mut out = Vec::new();
    for e in list.chunks(8) {
        if e[0] == 0 {
            break;
        }
        if let Some(b) = tag::aw2_co(e[0]) {
            out.push((b, e[2]));
        }
    }
    // Dual Strike's TAG box lists them from the record's end (Sami's
    // record: Eagle, Sonja; its box: Sonja, Eagle).
    out.reverse();
    out
}

/// The page's header (page 3's: the Super Power's name, `0x080149C0` with
/// the text in r3): "TAG" on the TAG page.
const PAGE_HEADER: u32 = 0x0808_5378;
fn page_header(core: &mut Core) {
    if crate::ds_weather::is_on(core) && core.raw_read_8(TAG_PAGE, -1) == 1 {
        core.gba_mut().cpu_mut().set_gpr(3, TAGHEAD_AT as i32);
    }
}

fn page_text(core: &mut Core) {
    if !crate::ds_weather::is_on(core) || core.raw_read_8(TAG_PAGE, -1) != 1 {
        return;
    }
    let Some(co) = page_co(core) else { return };
    let partners = partners_of(core, co);
    let mut t = Vec::new();
    if partners.is_empty() {
        t.extend_from_slice(b"No special partners.");
    }
    for (k, (b, _)) in partners.iter().enumerate() {
        if k > 0 {
            t.push(b'\r');
        }
        t.extend_from_slice(&co_name(core, *b));
    }
    set_string(core, TAGBOX_AT, &t);
    core.raw_write_8(PAGE_CO, -1, co);
    core.gba_mut().cpu_mut().set_gpr(3, TEXT_TAGBOX as i32);
}

/// The TAG page's stars (at the sprite flush): a row of three after each
/// partner's name, full for its rating.
const STAR_TILE: u32 = 0x320;
const STAR_SOURCES: [u32; 2] = [0x0810_2C24, 0x0810_2C64];
const OBJ_VRAM: u32 = 0x0601_0000;
/// Where the page's lines are (its text box at tile (1, 7)): the first
/// line's top and the spacing, and the stars' x.
pub const LINE_Y: i32 = 61;
pub const LINE_STEP: i32 = 16;
pub const STARS_X: i32 = 64;
/// The stars' OBJ palette (unused on the CO page; saved and put back):
/// AW2's panel star colours at their tiles' indices 9..15.
const STAR_PALETTE: u16 = 12;
const STAR_COLOURS: [(usize, u16); 7] = [(9, 0x0000), (10, 0x5FFF), (11, 0x027F), (12, 0x77DC), (13, 0x6B39), (14, 0x35F1), (15, 0x0000)];
const SAVED_STAR_PALETTE: u32 = STATE + 0xB0;
const OBJ_PALETTES: u32 = 0x200;

pub fn flush(core: &mut Core, at: u32, end: u32) -> u32 {
    if !crate::ds_weather::is_on(core) {
        return at;
    }
    let on = core.raw_read_8(TAG_PAGE, -1) == 1;
    let borrowed = core.raw_read_8(STARS_BORROWED, -1) == 1;
    if !on {
        if borrowed {
            let mut t = [0u8; 64];
            core.raw_read_range(SAVED_STARS, -1, &mut t);
            core.raw_write_range(OBJ_VRAM + 32 * STAR_TILE, -1, &t);
            let mut p = [0u8; 32];
            core.raw_read_range(SAVED_STAR_PALETTE, -1, &mut p);
            write_palette(core, OBJ_PALETTES / 32 + STAR_PALETTE as u32, &p);
            core.raw_write_8(STARS_BORROWED, -1, 0);
        }
        return at;
    }
    if !borrowed {
        let mut t = [0u8; 64];
        core.raw_read_range(OBJ_VRAM + 32 * STAR_TILE, -1, &mut t);
        core.raw_write_range(SAVED_STARS, -1, &t);
        let mut p = [0u8; 32];
        core.raw_read_range(PAL_BUFFER + OBJ_PALETTES + 32 * STAR_PALETTE as u32, -1, &mut p);
        core.raw_write_range(SAVED_STAR_PALETTE, -1, &p);
        core.raw_write_8(STARS_BORROWED, -1, 1);
    }
    let mut p = [0u8; 32];
    for (i, c) in STAR_COLOURS {
        p[2 * i..2 * i + 2].copy_from_slice(&c.to_le_bytes());
    }
    write_palette(core, OBJ_PALETTES / 32 + STAR_PALETTE as u32, &p);
    for (k, src) in STAR_SOURCES.iter().enumerate() {
        let mut t = [0u8; 32];
        core.raw_read_range(*src, -1, &mut t);
        core.raw_write_range(OBJ_VRAM + 32 * (STAR_TILE + k as u32), -1, &t);
    }
    let co = core.raw_read_8(PAGE_CO, -1);
    let mut at = at;
    for (line, (_, stars)) in partners_of(core, co).iter().enumerate() {
        for s in 0..3u8 {
            if at + 8 > end {
                return at;
            }
            let x = STARS_X + 7 * s as i32;
            let y = LINE_Y + LINE_STEP * line as i32;
            let tile = STAR_TILE as u16 + (s < *stars) as u16;
            core.raw_write_16(at, -1, y as u16 & 0xFF);
            core.raw_write_16(at + 2, -1, x as u16 & 0x1FF);
            core.raw_write_16(at + 4, -1, tile | STAR_PALETTE << 12);
            core.raw_write_16(at + 6, -1, 0);
            at += 8;
        }
    }
    at
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![
        (WAIT_QUOTE_TEST, Box::new(wait_quote)),
        (VICTORY_QUOTE, Box::new(victory_quote)),
        (PAGE_OPENED, Box::new(page_opened)),
        (PAGE_DOWN, Box::new(page_down)),
        (PAGE_UP, Box::new(page_up)),
        (PAGE_TEXT, Box::new(page_text)),
        (PAGE_HEADER, Box::new(page_header)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        assert!(STATE_END <= tag::STATE);
        assert!(SAVED_OBJ < SAVED_PALETTES);
        assert!(SAVED_STARS + 64 <= STATE_END);
    }
}
