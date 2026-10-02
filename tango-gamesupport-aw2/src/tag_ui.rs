//! CO tag pairs on screen ([`crate::tag`]), with the Dual Strike pack:
//!
//! - **Versus' Teams screen**: as Dual Strike's CO screen, every army has a
//!   partner slot: under its box the partner's box (32x28: the CO's Teams
//!   portrait drawn at 30 pixels in a dark line), or "None" (the default:
//!   the army plays single); the columns (frames, faces, emblems, labels,
//!   arrows) go up 16 pixels to make room.
//!   START on an army's CO stop switches the D-pad between its CO (the
//!   game's own) and its partner: the game's arrows move over and under the
//!   partner box, UP and DOWN go through the Teams list (and None), the
//!   army's own CO left out; the help line reads "Choose a partner CO."
//!   (text 0x7304, through crate::versus_rules's help trap). An army with a
//!   partner, a human's or the computer's, plays as a pair (no rule, as in
//!   Dual Strike, whose partner slot has a blank). In netplay
//!   both seats' buttons reach the Teams screen, as for the rest of it. The
//!   boxes' tiles and colours are borrowed (OBJ tiles 0x100..0x13F, palettes
//!   4..8, which the Teams screen leaves unused) and put back when it goes.
//! - **The CO panel** on the battle map, as Dual Strike's shows the pair (a
//!   face and a meter a CO): under AW2's panel and its stars, the partner's
//!   strip: the panel's own plate rows (its tiles, the army's colours; the
//!   lower half mirrored over the upper), the partner's HUD face in it and
//!   its meter under it as AW2 draws the active CO's (small stars for the
//!   CO Power, big ones for the rest of the Super Power; half and full).
//!   The face's tiles are OBJ tiles 0x309..0x310 and its colours OBJ
//!   palette 5 (no map sprite uses either while the panel is up;
//!   crate::heal_effect borrows those tiles while it plays: the face is left
//!   out then).

use mgba::core::Core;

use crate::tag;

const OBJ_VRAM: u32 = 0x0601_0000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
const NONE: u8 = 0xFF;

/// The HUD faces the game reads now (AW2's sheet, or tangoAW2's with the
/// new COs): a pool word of `DrawArmyCoPanel`; 8 tiles a CO.
const HUD_POOL: u32 = 0x0804_37EC;
/// The CO presentation table (a pool word of `LoadCoPalette`'s caller):
/// row +0x08, the CO's palettes (scheme 0 first).
const PRESENTATION_POOL: u32 = 0x0803_9B7C;
const PRESENTATION_ROW: u32 = 0x44;

fn hud_face(core: &Core, co: u8) -> [u8; 256] {
    let mut b = [0u8; 256];
    core.raw_read_range(core.raw_read_32(HUD_POOL, -1) + 0x100 * co as u32, -1, &mut b);
    b
}

fn co_palette(core: &Core, co: u8) -> [u8; 32] {
    let row = core.raw_read_32(PRESENTATION_POOL, -1) + PRESENTATION_ROW * co as u32;
    let mut b = [0u8; 32];
    core.raw_read_range(core.raw_read_32(row + 0x08, -1), -1, &mut b);
    b
}

fn write_palette(core: &mut Core, obj_pal: u32, p: &[u8; 32]) {
    for base in [PAL_BUFFER, PAL_RAM] {
        let at = base + 0x200 + 32 * obj_pal;
        let mut now = [0u8; 32];
        core.raw_read_range(at, -1, &mut now);
        if &now != p {
            core.raw_write_range(at, -1, p);
        }
    }
}

fn write_tiles(core: &mut Core, tile: u32, b: &[u8]) {
    let at = OBJ_VRAM + 32 * tile;
    let mut now = vec![0u8; b.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != b {
        core.raw_write_range(at, -1, b);
    }
}

/// One sprite entry of the frame's list (attr0, attr1, attr2).
fn put(core: &mut Core, at: u32, a0: u16, a1: u16, a2: u16) {
    core.raw_write_16(at, -1, a0);
    core.raw_write_16(at + 2, -1, a1);
    core.raw_write_16(at + 4, -1, a2);
}

const WIDE: u16 = 1 << 14;
fn size(s: u16) -> u16 {
    s << 14
}

// --- Versus' Teams screen ---------------------------------------------------------------------

/// The Teams/Rules record (AW2's, or crate::five's copy with room for five
/// armies: the same fields, the per-army CO indices moved to +0xA8).
const TEAMS: u32 = 0x0201_7C50;
const TEAMS_FIVE: u32 = 0x0203_0300;
const ARMIES_AT: u32 = 0x08;
const CO_COUNT_AT: u32 = 0x17;
const CO_LIST_AT: u32 = 0x18;
const CURSOR_AT: u32 = 0x32;

fn record(core: &Core) -> u32 {
    if crate::five::active(core) {
        TEAMS_FIVE
    } else {
        TEAMS
    }
}

fn co_index_at(core: &Core) -> u32 {
    if crate::five::active(core) {
        TEAMS_FIVE + 0xA8
    } else {
        TEAMS + 0x1C
    }
}

fn armies(core: &Core) -> u32 {
    let max = if crate::five::active(core) { 5 } else { 4 };
    (core.raw_read_8(record(core) + ARMIES_AT, -1) as u32).clamp(1, max)
}

/// UI state ([`tag::UI`]): the army whose partner the D-pad edits (0xFF
/// none), the Teams screen's borrowed tiles and palettes saved (1), a frame
/// count for the blink.
const EDITING: u32 = tag::UI;
const BORROWED: u32 = tag::UI + 1;
const BLINK: u32 = tag::UI + 2;
/// What the Teams screen's partner boxes borrow, as it was: 80 tiles (16 an
/// army, five armies) and six OBJ palettes the Teams screen leaves unused
/// (a partner's for each army, the empty slot's), in EWRAM tangoAW2 keeps
/// free (0x0203E800..0x0203F2BF).
const SAVED: u32 = 0x0203_E800;
const TEAMS_TILE: u32 = 0x100;
const TEAMS_TILES: u32 = 82;
/// The rating's stars (8x8, AW2's own small star tiles from the ROM: empty
/// at 0x08102C24, full at 0x08102C64), after the five boxes.
const STAR_TILE: u32 = 0x150;
const STAR_SOURCES: [u32; 2] = [0x0810_2C24, 0x0810_2C64];
/// The stars' colours (their tiles' indices 9..15) in the empty slot's
/// palette: AW2's panel colours for them.
const STAR_COLOURS: [(usize, u16); 7] = [(9, 0x0000), (10, 0x5FFF), (11, 0x027F), (12, 0x77DC), (13, 0x6B39), (14, 0x35F1), (15, 0x0000)];
const BOX_TILES: u32 = 16;
const PARTNER_PALS: [u32; 5] = [5, 6, 7, 8, 12];
const NONE_PAL: u32 = 13;
const SAVED_PALS: [u32; 6] = [5, 6, 7, 8, 12, 13];

const KEY_START: u32 = 1 << 3;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;

/// The Teams stage of Versus' Teams/Rules screen (record +0x30: 1 Teams,
/// 0 Rules).
fn teams_on(core: &Core, ds: bool) -> bool {
    ds && crate::pvp::in_versus(core) && crate::pvp::on_teams_screen(core) && core.raw_read_8(record(core) + 0x30, -1) == 1
}

/// On an army's CO stop (the record's state as crate::skills_panel reads it).
fn co_stop(core: &Core) -> Option<u32> {
    let rec = record(core);
    if core.raw_read_8(rec + 0x30, -1) != 1
        || core.raw_read_8(rec + 0x26, -1) != 0
        || core.raw_read_8(rec + 0x2D, -1) != 0
        || core.raw_read_8(rec + 0x24, -1) != 0
    {
        return None;
    }
    let c = core.raw_read_8(rec + CURSOR_AT, -1) as u32;
    (c % 2 == 0).then_some(c / 2)
}

fn teams_list(core: &Core) -> Vec<u8> {
    let n = core.raw_read_8(record(core) + CO_COUNT_AT, -1) as u32;
    let at = core.raw_read_32(record(core) + CO_LIST_AT, -1);
    if !(0x0200_0000..0x0400_0000).contains(&at) {
        return Vec::new();
    }
    (0..n.min(64)).map(|k| core.raw_read_8(at + k, -1)).collect()
}

fn army_main(core: &Core, army: u32) -> Option<u8> {
    let list = teams_list(core);
    list.get(core.raw_read_8(co_index_at(core) + army, -1) as usize).copied()
}

/// AW2's "Choose a CO." (text 0x9DC), the Teams screen's help line on a
/// CO stop: on Versus' Teams screen with the pack it says START picks a
/// partner (crate::tag's string); everywhere else it is AW2's.
const TEXT_TABLE: u32 = 0x0861_0A38;
const CHOOSE_CO: u32 = 0x9DC;
static CHOOSE_CO_AW2: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

fn help_line(core: &mut Core, ds: bool) {
    let entry = TEXT_TABLE + 4 * CHOOSE_CO;
    let aw2 = *CHOOSE_CO_AW2.get_or_init(|| core.raw_read_32(entry, -1));
    // Versus only uses it on the Teams screen (set before the screen draws it).
    let want = if ds && crate::pvp::in_versus(core) { tag::CHOOSE_CO_AT } else { aw2 };
    if core.raw_read_32(entry, -1) != want {
        core.raw_write_32(entry, -1, want);
    }
}

/// The Teams screen's help line while a partner is picked
/// ("Choose a partner CO."), for crate::versus_rules's help trap.
pub fn help_override(core: &Core) -> Option<u16> {
    let on = crate::ds_weather::is_on(core) && teams_on(core, true);
    (on && core.raw_read_8(EDITING, -1) != NONE).then_some(tag::TEXT_PARTNER)
}

/// Every frame, before the game reads the pad: the partner picks.
pub fn teams_tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    if !ds {
        return keys;
    }
    help_line(core, ds);
    if !teams_on(core, ds) || crate::skills_panel::is_open(core) {
        if !teams_on(core, ds) {
            stop_editing(core);
            restore(core);
        }
        return keys;
    }
    let pressed = keys & !prev;
    let armies = armies(core);
    let stop = co_stop(core).filter(|&a| a < armies);
    let mut editing = core.raw_read_8(EDITING, -1);
    if editing != NONE && stop != Some(editing as u32) {
        editing = NONE;
    }
    if let Some(a) = stop {
        if pressed & KEY_START != 0 {
            editing = if editing == NONE { a as u8 } else { NONE };
        }
    }
    let mut keys = keys & !KEY_START;
    if editing != NONE {
        let a = editing as u32;
        let step: i32 = if pressed & KEY_DOWN != 0 {
            1
        } else if pressed & KEY_UP != 0 {
            -1
        } else {
            0
        };
        if step != 0 {
            let main = army_main(core, a);
            let mut options: Vec<u8> = vec![NONE];
            options.extend(teams_list(core).into_iter().filter(|&c| Some(c) != main));
            let cur = core.raw_read_8(tag::TEAMS_PARTNER + a, -1);
            let i = options.iter().position(|&c| c == cur).unwrap_or(0) as i32;
            let n = options.len() as i32;
            let next = options[(i + step).rem_euclid(n) as usize];
            core.raw_write_8(tag::TEAMS_PARTNER + a, -1, next);
        }
        keys &= !(KEY_UP | KEY_DOWN);
    }
    core.raw_write_8(EDITING, -1, editing);
    let b = core.raw_read_8(BLINK, -1);
    core.raw_write_8(BLINK, -1, b.wrapping_add(1));
    keys
}

fn borrow(core: &mut Core) {
    if core.raw_read_8(BORROWED, -1) == 1 {
        return;
    }
    let mut t = vec![0u8; (32 * TEAMS_TILES) as usize];
    core.raw_read_range(OBJ_VRAM + 32 * TEAMS_TILE, -1, &mut t);
    core.raw_write_range(SAVED, -1, &t);
    for (k, pal) in SAVED_PALS.iter().enumerate() {
        let mut p = [0u8; 32];
        core.raw_read_range(PAL_BUFFER + 0x200 + 32 * pal, -1, &mut p);
        core.raw_write_range(SAVED + 32 * TEAMS_TILES + 32 * k as u32, -1, &p);
    }
    core.raw_write_8(BORROWED, -1, 1);
}

fn restore(core: &mut Core) {
    if core.raw_read_8(BORROWED, -1) != 1 {
        return;
    }
    let mut t = vec![0u8; (32 * TEAMS_TILES) as usize];
    core.raw_read_range(SAVED, -1, &mut t);
    core.raw_write_range(OBJ_VRAM + 32 * TEAMS_TILE, -1, &t);
    for (k, pal) in SAVED_PALS.iter().enumerate() {
        let mut p = [0u8; 32];
        core.raw_read_range(SAVED + 32 * TEAMS_TILES + 32 * k as u32, -1, &mut p);
        for base in [PAL_BUFFER, PAL_RAM] {
            core.raw_write_range(base + 0x200 + 32 * pal, -1, &p);
        }
    }
    core.raw_write_8(BORROWED, -1, 0);
}

/// The column's emblem (the country's, 16x16, OBJ palette 9, at the
/// face's lower left): its attr2.
fn emblem(core: &Core, start: u32, at: u32, fx: i32, fy: i32) -> Option<u16> {
    let mut e = start;
    while e + 8 <= at {
        let (a0, a1, a2) = (core.raw_read_16(e, -1), core.raw_read_16(e + 2, -1), core.raw_read_16(e + 4, -1));
        if (a0 >> 14) == 0 && (a1 >> 14) == 1 && a2 >> 12 == 9 && (a1 & 0x1FF) as i32 == fx + 2 && (a0 & 0xFF) as i32 == fy + 32 {
            return Some(a2);
        }
        e += 8;
    }
    None
}

/// A sprite of the frame's list: (index, x, y, tile).
fn find(core: &Core, start: u32, at: u32, tile: u16, wide: bool) -> Option<(u32, i32, i32)> {
    let mut e = start;
    while e + 8 <= at {
        let a0 = core.raw_read_16(e, -1);
        let a1 = core.raw_read_16(e + 2, -1);
        let a2 = core.raw_read_16(e + 4, -1);
        if a2 & 0x3FF == tile && ((a0 >> 14) == 1) == wide && (a0 >> 8) & 3 != 2 {
            let y = (a0 & 0xFF) as i32;
            let x = (a1 & 0x1FF) as i32;
            return Some((e, if x >= 256 { x - 512 } else { x }, y));
        }
        e += 8;
    }
    None
}

fn stop_editing(core: &mut Core) {
    if core.raw_read_8(EDITING, -1) != NONE {
        core.raw_write_8(EDITING, -1, NONE);
    }
}

/// The partner's box: 32x28 (in a 32x32 sprite), a dark line round the
/// CO's Teams-screen portrait (48x48, presentation row +0x0C, LZ77) drawn
/// at 30 pixels, its bottom rows (the shoulders) left out; behind the face
/// the portrait's lightest colour, as the big box's light ground. Pixels
/// as palette indices of the CO's own palette.
const BOX_W: usize = 32;
const BOX_H: usize = 28;
const FACE_ROW: u32 = 0x0C;

fn partner_box(core: &Core, co: u8) -> (Vec<u8>, [u8; 32]) {
    let row = core.raw_read_32(PRESENTATION_POOL, -1) + PRESENTATION_ROW * co as u32;
    let pal = co_palette(core, co);
    let colour = |i: usize| u16::from_le_bytes([pal[2 * i], pal[2 * i + 1]]);
    let lum = |c: u16| (c & 31) as u32 * 3 + ((c >> 5) & 31) as u32 * 6 + ((c >> 10) & 31) as u32;
    let dark = (1..16).min_by_key(|&i| lum(colour(i))).unwrap_or(15) as u8;
    let light = (1..16).max_by_key(|&i| lum(colour(i))).unwrap_or(1) as u8;
    let src = crate::invention_art::lz77(core, core.raw_read_32(row + FACE_ROW, -1));
    // The portrait: 6 rows of 6 tiles (a 32x8 and a 16x8 sprite a row).
    let face = |x: usize, y: usize| -> u8 {
        let t = 6 * (y / 8) + x / 8;
        let b = src.get(32 * t + 4 * (y % 8) + (x % 8) / 2).copied().unwrap_or(0);
        (b >> (4 * (x & 1))) & 15
    };
    let mut px = vec![0u8; BOX_W * 32];
    for y in 0..BOX_H {
        for x in 0..BOX_W {
            let edge = y == 0 || y == BOX_H - 1 || x == 0 || x == BOX_W - 1;
            px[y * BOX_W + x] = if edge {
                dark
            } else {
                let v = face((x - 1) * 48 / 30, (y - 1) * 48 / 30);
                if v == 0 { light } else { v }
            };
        }
    }
    (tiles_of(&px, BOX_W, 32), pal)
}

/// The empty slot (no partner: the army plays single): the partner box's
/// frame on a light ground, the army's emblem drawn on it (a sprite).
fn none_box(_core: &Core) -> Vec<u8> {
    let mut px = vec![0u8; BOX_W * 32];
    for y in 0..BOX_H {
        for x in 0..BOX_W {
            let edge = y == 0 || y == BOX_H - 1 || x == 0 || x == BOX_W - 1;
            let inner = y == 1 || y == BOX_H - 2 || x == 1 || x == BOX_W - 2;
            px[y * BOX_W + x] = if edge { 3 } else if inner { 2 } else { 1 };
        }
    }
    tiles_of(&px, BOX_W, 32)
}

/// Pixels (w x h, one byte each) as 4bpp OBJ tiles in rows.
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

/// The empty slot's colours: a light ground, a lighter rim, the dark line.
const NONE_COLOURS: [u16; 4] = [0, 0x6F7B, 0x7FFF, 0x1064];

/// The Teams screen's face sprites: the first column's (32x8) tile 400 + 36k.
const FACE_TILE: u16 = 400;
const FACE_STRIDE: u16 = 36;
/// The game's arrows over and under the column being edited (16x8).
const ARROW_UP: u16 = 700;
const ARROW_DOWN: u16 = 702;

/// With a partner shown the columns (frames, faces, emblems, labels,
/// arrows: every sprite between these lines) go up, the partner boxes
/// under them.
const SHIFT: i32 = 16;
const COLUMN_TOP: i32 = 36;
const COLUMN_BOTTOM: i32 = 110;
/// The partner box under the big one (its face's top + 57), the arrows
/// over and under it while it is picked.
const BOX_DY: i32 = 57;

fn teams_flush(core: &mut Core, start: u32, mut at: u32, end: u32) -> u32 {
    if !teams_on(core, true) || crate::skills_panel::is_open(core) {
        return at;
    }
    let armies = armies(core);
    let editing = core.raw_read_8(EDITING, -1);
    let blink = core.raw_read_8(BLINK, -1);
    let partner_of = |core: &Core, a: u32| {
        let p = core.raw_read_8(tag::TEAMS_PARTNER + a, -1);
        (p != NONE && Some(p) != army_main(core, a)).then_some(p)
    };
    // Every army has its partner slot, as Dual Strike's CO screen: a
    // partner's box, or None.
    borrow(core);
    // The columns up.
    let mut e = start;
    while e + 8 <= at {
        let a0 = core.raw_read_16(e, -1);
        let y = (a0 & 0xFF) as i32;
        if (a0 >> 8) & 3 != 2 && (COLUMN_TOP..COLUMN_BOTTOM).contains(&y) {
            core.raw_write_16(e, -1, (a0 & !0xFF) | ((y - SHIFT) as u16 & 0xFF));
        }
        e += 8;
    }
    // The empty slot's palette, with the stars' colours.
    let mut p = [0u8; 32];
    for (k, c) in NONE_COLOURS.iter().enumerate() {
        p[2 * k..2 * k + 2].copy_from_slice(&c.to_le_bytes());
    }
    for (k, c) in STAR_COLOURS {
        p[2 * k..2 * k + 2].copy_from_slice(&c.to_le_bytes());
    }
    write_palette(core, NONE_PAL, &p);
    for a in 0..armies {
        let Some((_, fx, fy)) = find(core, start, at, FACE_TILE + FACE_STRIDE * a as u16, true) else { continue };
        let tile = TEAMS_TILE + BOX_TILES * a;
        let pal = match partner_of(core, a) {
            Some(p) => {
                let (tiles, palette) = partner_box(core, p);
                write_tiles(core, tile, &tiles);
                write_palette(core, PARTNER_PALS[a as usize], &palette);
                PARTNER_PALS[a as usize]
            }
            None => {
                write_tiles(core, tile, &none_box(core));
                NONE_PAL
            }
        };
        let (x, y) = (fx + 8, fy + BOX_DY);
        if at + 8 > end {
            break;
        }
        // A special pair (Dual Strike's TAG box): its rating, 1..3 stars
        // of three, on the box's bottom edge.
        let stars = army_main(core, a)
            .zip(partner_of(core, a))
            .and_then(|(m, p)| tag::special_pair(m, p))
            .map_or(0, |(s, _)| s.min(3));
        if stars > 0 && at + 8 * 4 <= end {
            for (k, src) in STAR_SOURCES.iter().enumerate() {
                let mut t = [0u8; 32];
                core.raw_read_range(*src, -1, &mut t);
                write_tiles(core, STAR_TILE + k as u32, &t);
            }
            for k in 0..3u16 {
                let full = (k as u8) < stars;
                let t = STAR_TILE as u16 + full as u16;
                put(core, at, (y + 21) as u16 & 0xFF, (x + 2 + 7 * k as i32) as u16 & 0x1FF, t | (NONE_PAL as u16) << 12);
                core.raw_write_16(at + 6, -1, 0);
                at += 8;
            }
        }
        if pal == NONE_PAL {
            // The empty slot shows the army's emblem, as Dual Strike's blank
            // slot its army's: the game's own emblem sprite of the column.
            if let Some(em) = emblem(core, start, at, fx, fy) {
                if at + 16 > end {
                    break;
                }
                put(core, at, (y + 6) as u16 & 0xFF, ((x + 8) as u16 & 0x1FF) | size(1), em);
                core.raw_write_16(at + 6, -1, 0);
                at += 8;
            }
        }
        put(core, at, y as u16 & 0xFF, (x as u16 & 0x1FF) | size(2), tile as u16 | (pal as u16) << 12);
        core.raw_write_16(at + 6, -1, 0);
        at += 8;
        if editing as u32 == a {
            // The game's arrows go to the partner box, blinking as the game's do.
            for (tile, ay) in [(ARROW_UP, y - 9), (ARROW_DOWN, y + BOX_H as i32)] {
                if let Some((e, _, _)) = find(core, start, at, tile, true) {
                    let a2 = core.raw_read_16(e + 4, -1);
                    let show = blink % 32 < 24;
                    let yy = if show { ay as u16 & 0xFF } else { 160 };
                    put(core, e, yy | WIDE, ((x + 8) as u16 & 0x1FF) | size(0), a2);
                }
            }
        }
    }
    at
}

// --- The CO panel on the battle map ---------------------------------------------------------

const PANEL_FACE_TILE: u32 = 0x309;
const PANEL_PAL: u32 = 5;
/// The panel's header (64x32, OBJ tile 0, palette 7: the army's colours)
/// and its star tiles (small: 32 empty, 33 half, 34 full; big, 16x16: 35,
/// 39, 43).
const HEADER_TILE: u16 = 0;
const STAR_SMALL: [u16; 3] = [32, 33, 34];
const STAR_BIG: [u16; 3] = [35, 39, 43];
/// The partner's strip, under the panel's stars: its top (outline) at the
/// panel's y + 34, the face's 16 rows from y + 37, the stars at y + 53.
const STRIP_Y: i32 = 29;
const STRIP_FACE_Y: i32 = 37;
const STRIP_STARS_Y: i32 = 53;
const VFLIP: u16 = 1 << 13;

fn panel_flush(core: &mut Core, start: u32, at: u32, end: u32) -> u32 {
    if core.raw_read_8(tag::PANEL + 5, -1) != 1 {
        return at;
    }
    core.raw_write_8(tag::PANEL + 5, -1, 0);
    let army = core.raw_read_8(tag::PANEL + 4, -1) as u32;
    let Some(b) = tag::partner(core, army) else { return at };
    let x = core.raw_read_16(tag::PANEL, -1) as i32;
    let y = core.raw_read_16(tag::PANEL + 2, -1) as i32;
    // The header must be in this frame's list (the panel drawn).
    let Some((header, _, _)) = find(core, start, at, HEADER_TILE, true) else { return at };
    let header_a2 = core.raw_read_16(header + 4, -1);
    let base = header_a2 & 0x3FF;
    let pal = header_a2 & 0xF000;
    let mut sprites: Vec<[u16; 3]> = Vec::new();
    // The partner's meter, as AW2 draws the active CO's: a small star a
    // star of its CO Power, a big one a star more of its Super Power;
    // half-filled and full as the meter is.
    let uses = tag::partner_uses(core, army);
    let (cop, scop) = stars_of(core, b);
    let per = tag::star_cost(uses).max(1);
    let charge = tag::partner_charge(core, army);
    for k in (0..scop.min(10)).rev() {
        let level = if charge >= per * (k + 1) {
            2
        } else if charge >= per * k + per / 2 {
            1
        } else {
            0
        };
        let sy = (y + STRIP_STARS_Y) as u16 & 0xFF;
        if k < cop {
            let sx = x + 6 * k as i32;
            sprites.push([sy, sx as u16 & 0x1FF, STAR_SMALL[level] | pal]);
        } else {
            let sx = x + 6 * k as i32 - 4;
            sprites.push([sy, (sx as u16 & 0x1FF) | size(1), STAR_BIG[level] | pal]);
        }
    }
    // The partner's face (crate::heal_effect borrows its tiles while it plays).
    if !crate::heal_effect::playing(core) {
        write_tiles(core, PANEL_FACE_TILE, &hud_face(core, b));
        write_palette(core, PANEL_PAL, &co_palette(core, b));
        sprites.push([((y + STRIP_FACE_Y) as u16 & 0xFF) | WIDE, ((x + 2) as u16 & 0x1FF) | size(2), PANEL_FACE_TILE as u16 | (PANEL_PAL as u16) << 12]);
    }
    // The strip: the header's own plate rows (its tiles, its colours), the
    // lower half mirrored over the upper: rounded at both right corners.
    for (row, dy, flip) in [(3u16, 0, VFLIP), (2, 8, VFLIP), (2, 16, 0), (3, 24, 0)] {
        for half in 0..2u16 {
            let sx = x + 32 * half as i32;
            sprites.push([
                ((y + STRIP_Y + dy) as u16 & 0xFF) | WIDE,
                (sx as u16 & 0x1FF) | flip | size(1),
                (base + 8 * row + 4 * half) | pal,
            ]);
        }
    }
    // In front of the header: inserted before it in the list.
    let n = sprites.len() as u32;
    if at + 8 * n > end {
        return at;
    }
    let mut buf = vec![0u8; (at - header) as usize];
    core.raw_read_range(header, -1, &mut buf);
    core.raw_write_range(header + 8 * n, -1, &buf);
    for (k, s) in sprites.iter().enumerate() {
        put(core, header + 8 * k as u32, s[0], s[1], s[2]);
        core.raw_write_16(header + 8 * k as u32 + 6, -1, 0);
    }
    at + 8 * n
}

fn stars_of(core: &Core, co: u8) -> (u32, u32) {
    let row = core.raw_read_32(0x0804_2DDC, -1) + 0x104 * co as u32;
    (core.raw_read_32(row + 0x0C, -1), core.raw_read_32(row + 0x10, -1))
}

/// At the sprite flush (crate::branding::flush).
pub fn flush(core: &mut Core, start: u32, at: u32, end: u32) -> u32 {
    if !crate::ds_weather::is_on(core) {
        return at;
    }
    let at = teams_flush(core, start, at, end);
    tag::co_screen_flush(core, start, at);
    panel_flush(core, start, at, end)
}
