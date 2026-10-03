//! The Set Skills screen (crate::co_skills): Dual Strike's SET SKILLS
//! screen on one GBA screen, over AW2's CO screens, with the Dual Strike
//! pack.
//!
//! - Where: the War Room's CO screen (also the DS Campaign's, Survival's
//!   and AW2's campaign's: `ProcScr_CoSelect` 0x08616638, at its input
//!   stage, its idle callback `CoSelect_IDLE` 0x0807CE5D), SELECT opens it
//!   for the CO highlighted (the carousel's `+0x52` of its group `+0x58`,
//!   the group's list offset `+0x5C`, the CO list 0x030058E0, group sizes
//!   0x03005948), editing the set of the mode: the DS Campaign's and AW2's
//!   campaign's (Campaign), Survival's, the War Room's. On Versus' Teams
//!   screen (on an army's CO stop: the record `0x02017C50`, cursor `+0x32`
//!   even, CO list `+0x18`, index `+0x1C + army`), SELECT opens it for that
//!   army's CO, editing its Versus set (R and L there change the army's
//!   colour, crate::pvp). The Versus rule Skills (crate::co_skills::VERSUS_RULE)
//!   is a row of the Rules screen (crate::versus_rules).
//! - The screen, as Dual Strike's (its bottom screen's SKILLS RANK board,
//!   its top screen's CO bar and set, its help bar), converted at run time
//!   from the .nds: the skill icons (`ohashi/res_skill`: a palette and
//!   44 16x16 icons, 0 none, then by id from 0x20), the board
//!   (`ohashi/res_skilledit`: its tilemap and palettes;
//!   `res_skilledit_lang_E`: the banner's and the spots' tiles). Columns are
//!   the ranks (1..9, X for 10) as Dual Strike's, 24 pixels apart (30 tiles:
//!   Dual Strike's 32 less its two filler tiles), six rows. Above them the
//!   CO's name, rank and set (its icons on its slots); below, the help bar:
//!   the skill under the cursor, its rank, its description. Text in AW2's
//!   own proportional font (`0x084C32E4` glyph pointers, `0x084C36E4`
//!   widths; 16 rows of 4bpp nibbles, `(width + 1) / 2` bytes a row).
//!   A skill the CO has not reached is drawn faded; a skill on the set has a
//!   red frame. The cursor is Dual Strike's corner brackets.
//! - Buttons: the D-pad moves the cursor over the board, A puts the skill on
//!   the set (as many as the CO's slots, min(rank, 4)) or takes it off, B
//!   (or SELECT or START) keeps the set and closes (Dual Strike's BACK).
//!   While it is up the game gets no button ([`tick`]).
//! - Drawn on BG0 (char block and screen as the screen has them: 0 and 14 on
//!   both screens), the other layers and the game's sprites off while it is
//!   up (gDispIo's DISPCNT shadow); BG palettes 0..2. What it covers (BG0's tiles and
//!   tilemap, the BG palettes) is copied aside when it opens, into the ROM
//!   image's free space (0x08EF0000..0x08EF4BFF), and put back when it
//!   closes, with the layers.
//!
//! Everything is in emulated memory: in netplay both seats' buttons reach
//! the Teams screen and the console boots from seat 0's save, so both peers
//! see the same sets and rule.

use mgba::core::Core;
use std::sync::OnceLock;

use crate::co_skills::{self, Set};

// --- State (EWRAM after the skill data) ------------------------------------------

const STATE: u32 = 0x0203_E3A0;
const OPEN: u32 = STATE;
/// The cursor's column (a rank, 0..9).
const COL: u32 = STATE + 1;
const CO: u32 = STATE + 2;
/// The set being edited: 0 Campaign, 1 Survival, 2 War Room, 3 Versus 0.
const WHICH: u32 = STATE + 3;
const IDS: u32 = STATE + 4;
/// 1 on the Teams screen (the rule's line).
const ON_TEAMS: u32 = STATE + 8;
/// The cursor's row.
const ROW: u32 = STATE + 9;
/// 1 while what the screen covers is copied aside; the layers' bits of
/// gDispIo's DISPCNT as they were (+12, 2 bytes).
const BACKED: u32 = STATE + 10;
const DISP_SAVED: u32 = STATE + 12;
#[cfg(test)]
const STATE_LEN: u32 = 14;

fn set_code(s: Set) -> u8 {
    match s {
        Set::Campaign => 0,
        Set::Survival => 1,
        Set::WarRoom => 2,
        Set::Versus(_) => 3,
    }
}

fn code_set(c: u8) -> Set {
    match c {
        0 => Set::Campaign,
        1 => Set::Survival,
        2 => Set::WarRoom,
        _ => Set::Versus(0),
    }
}

pub fn is_open(core: &Core) -> bool {
    core.raw_read_8(OPEN, -1) == 1
}

// --- The screens -----------------------------------------------------------------

const PROC_POOL: (u32, u32) = (0x0200_D610, 0x0200_E418);
const PROC_SIZE: u32 = 0x6C;
const CO_SELECT: u32 = 0x0861_6638;
const CO_SELECT_IDLE: u32 = 0x0807_CE5D;
const CO_LIST: u32 = 0x0300_58E0;
const GROUP_SIZES: u32 = 0x0300_5948;
const GAME_MODE: u32 = 0x0300_3FC1;

/// The CO screen at its input stage: the CO highlighted, and the set the mode
/// uses.
fn co_select(core: &Core) -> Option<(u8, Set)> {
    let proc = (PROC_POOL.0..PROC_POOL.1)
        .step_by(PROC_SIZE as usize)
        .find(|&p| core.raw_read_32(p, -1) == CO_SELECT && core.raw_read_32(p + 0x10, -1) == CO_SELECT_IDLE)?;
    if core.raw_read_16(proc + 0x4E, -1) != 0 || core.raw_read_16(proc + 0x60, -1) != 0 {
        return None;
    }
    let group = core.raw_read_32(proc + 0x58, -1);
    let size = core.raw_read_8(GROUP_SIZES + group.min(15), -1).max(1) as u32;
    let k = core.raw_read_16(proc + 0x52, -1) as u32 % size + core.raw_read_32(proc + 0x5C, -1);
    let co = core.raw_read_8(CO_LIST + k.min(63), -1);
    co_skills::co_slot(co)?;
    let set = if crate::ds_campaign::active(core) {
        Set::Campaign
    } else if crate::survival::on(core) {
        Set::Survival
    } else if core.raw_read_8(GAME_MODE, -1) == 1 {
        Set::Campaign
    } else {
        Set::WarRoom
    };
    Some((co, set))
}

/// The Teams record (AW2's, or crate::five's copy in a five-army game,
/// whose per-army CO indices are at +0xA8 instead of +0x1C).
const TEAMS: u32 = 0x0201_7C50;
const TEAMS_FIVE: u32 = 0x0203_0300;

/// Versus' Teams screen on an army's CO stop: the army's CO.
fn teams(core: &Core) -> Option<u8> {
    if !crate::pvp::on_teams_screen(core) {
        return None;
    }
    let five = crate::five::active(core);
    let rec = if five { TEAMS_FIVE } else { TEAMS };
    if core.raw_read_8(rec + 0x30, -1) != 1
        || core.raw_read_8(rec + 0x26, -1) != 0
        || core.raw_read_8(rec + 0x2D, -1) != 0
        || core.raw_read_8(rec + 0x24, -1) != 0
    {
        return None;
    }
    let cursor = core.raw_read_8(rec + 0x32, -1) as u32;
    if cursor % 2 != 0 {
        return None;
    }
    let army = cursor / 2;
    let list = core.raw_read_32(rec + 0x18, -1);
    if !(0x0200_0000..0x0400_0000).contains(&list) {
        return None;
    }
    let index_at = if five { rec + 0xA8 } else { rec + 0x1C };
    let co = core.raw_read_8(list + core.raw_read_8(index_at + army, -1) as u32, -1);
    co_skills::co_slot(co).map(|_| co)
}

// --- Input -----------------------------------------------------------------------

const KEY_A: u32 = 1 << 0;
const KEY_B: u32 = 1 << 1;
const KEY_SELECT: u32 = 1 << 2;
const KEY_START: u32 = 1 << 3;
const KEY_RIGHT: u32 = 1 << 4;
const KEY_LEFT: u32 = 1 << 5;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;
const ALL_KEYS: u32 = 0x3FF;

fn ids(core: &Core) -> [u8; 4] {
    let mut b = [0u8; 4];
    core.raw_read_range(IDS, -1, &mut b);
    b
}

/// The board: per rank column (0..9: ranks 1..9, X for 10), the skills of
/// that rank a player can take, by id.
fn board() -> &'static [Vec<u8>; 10] {
    static BOARD: OnceLock<[Vec<u8>; 10]> = OnceLock::new();
    BOARD.get_or_init(|| {
        let mut b: [Vec<u8>; 10] = Default::default();
        for id in co_skills::ids() {
            if let Some((rank, _, _)) = co_skills::info(id) {
                if (1..=10).contains(&rank) {
                    b[rank as usize - 1].push(id);
                }
            }
        }
        b
    })
}

/// The skill under the cursor.
fn under_cursor(core: &Core) -> Option<u8> {
    let col = core.raw_read_8(COL, -1) as usize;
    let row = core.raw_read_8(ROW, -1) as usize;
    board().get(col)?.get(row).copied()
}

/// Every frame (before the game reads the pad): open, run or close the
/// screen; while it is up the game gets no button. `keys` the frame's,
/// `prev` the last frame's. Returns the keys the game gets.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    if !ds {
        if is_open(core) {
            close(core);
        }
        return keys;
    }
    let pressed = keys & !prev;
    if !is_open(core) {
        let target = co_select(core)
            .filter(|_| pressed & KEY_SELECT != 0)
            .map(|(co, s)| (co, s, false))
            .or_else(|| teams(core).filter(|_| pressed & KEY_SELECT != 0).map(|co| (co, Set::Versus(0), true)));
        if let Some((co, set, on_teams)) = target {
            crate::ds_campaign::skills_loaded(core);
            core.raw_write_8(OPEN, -1, 1);
            core.raw_write_8(COL, -1, 0);
            core.raw_write_8(ROW, -1, 0);
            core.raw_write_8(CO, -1, co);
            core.raw_write_8(WHICH, -1, set_code(set));
            core.raw_write_range(IDS, -1, &co_skills::set_of(core, co, set));
            core.raw_write_8(ON_TEAMS, -1, on_teams as u8);
            draw(core);
            return keys & !ALL_KEYS;
        }
        return keys;
    }
    // The screen went away under the panel: it closes as it was.
    let on_teams = core.raw_read_8(ON_TEAMS, -1) == 1;
    if (on_teams && teams(core).is_none()) || (!on_teams && co_select(core).is_none()) {
        close(core);
        return keys & !ALL_KEYS;
    }
    let co = core.raw_read_8(CO, -1);
    let n = co_skills::slots(core, co);
    if pressed & (KEY_B | KEY_SELECT | KEY_START) != 0 {
        let set = code_set(core.raw_read_8(WHICH, -1));
        let mut kept = [0u8; 4];
        for (k, &id) in ids(core).iter().take(n).filter(|&&id| id != 0).enumerate() {
            kept[k] = id;
        }
        co_skills::store_set(core, co, set, kept);
        close(core);
        return keys & !ALL_KEYS;
    }
    let b = board();
    let mut col = core.raw_read_8(COL, -1) as usize % 10;
    let mut row = core.raw_read_8(ROW, -1) as usize;
    if pressed & KEY_RIGHT != 0 {
        col = (col + 1) % 10;
    }
    if pressed & KEY_LEFT != 0 {
        col = (col + 9) % 10;
    }
    let len = b[col].len().max(1);
    if pressed & KEY_DOWN != 0 {
        row = (row + 1) % len;
    }
    if pressed & KEY_UP != 0 {
        row = (row + len - 1) % len;
    }
    row = row.min(len - 1);
    core.raw_write_8(COL, -1, col as u8);
    core.raw_write_8(ROW, -1, row as u8);
    if pressed & KEY_A != 0 {
        if let Some(id) = b[col].get(row).copied() {
            let mut cur: Vec<u8> = ids(core).iter().copied().filter(|&x| x != 0).collect();
            if let Some(at) = cur.iter().position(|&x| x == id) {
                cur.remove(at);
            } else if co_skills::unlocked(core, co, id) && cur.len() < n {
                cur.push(id);
            }
            let mut out = [0u8; 4];
            for (k, &x) in cur.iter().take(4).enumerate() {
                out[k] = x;
            }
            core.raw_write_range(IDS, -1, &out);
        }
    }
    draw(core);
    keys & !ALL_KEYS
}

// --- Dual Strike's art -------------------------------------------------------------

struct Art {
    /// res_skill's palette and 43 icons (16x16, palette indices).
    icon_pal: [u16; 16],
    icons: Vec<[u8; 256]>,
    /// res_skilledit's first palette.
    ui_pal: [u16; 16],
    /// The banner (30 x 2 tiles of pixels: 240x16) and an empty spot (16x16).
    banner: Vec<u8>,
    spot: [u8; 256],
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}

/// One 8x8 cell of a tile sheet, as a tilemap entry draws it (flips).
fn cell(tiles: &[u8], e: u16) -> [u8; 64] {
    let (k, hf, vf) = ((e & 0x3FF) as usize, e & 0x400 != 0, e & 0x800 != 0);
    let mut px = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            let (sx, sy) = (if hf { 7 - x } else { x }, if vf { 7 - y } else { y });
            let b = tiles.get(32 * k + 4 * sy + sx / 2).copied().unwrap_or(0);
            px[8 * y + x] = (b >> (4 * (sx & 1))) & 15;
        }
    }
    px
}

fn build_art() -> Option<Art> {
    let pack = crate::ds_pack::pack()?;
    let skill = pack.file("ohashi/res_skill")?;
    let mut icon_pal = [0u16; 16];
    for (k, c) in icon_pal.iter_mut().enumerate() {
        *c = u16_at(skill, 2 * k)?;
    }
    let tiles = skill.get(32..)?;
    let mut icons = Vec::new();
    for i in 0..ICONS {
        let mut px = [0u8; 256];
        for q in 0..4 {
            let c = cell(tiles, (4 * i + q) as u16);
            for y in 0..8 {
                for x in 0..8 {
                    px[16 * (8 * (q / 2) + y) + 8 * (q % 2) + x] = c[8 * y + x];
                }
            }
        }
        icons.push(px);
    }
    let edit = pack.file("ohashi/res_skilledit")?;
    let map = crate::ds_art::lz10(edit)?;
    let pal_at = edit.len().checked_sub(64)?;
    let mut ui_pal = [0u16; 16];
    for (k, c) in ui_pal.iter_mut().enumerate() {
        *c = u16_at(edit, pal_at + 2 * k)?;
    }
    let lang = crate::ds_art::lz10(pack.file("ohashi/res_skilledit_lang_E")?)?;
    let ent = |x: usize, y: usize| u16_at(&map, 2 * (32 * y + x)).unwrap_or(0);
    // The banner: rows 4..5, less the filler tiles at 2 and 29.
    let cols: Vec<usize> = (0..32).filter(|&c| c != 2 && c != 29).collect();
    let mut banner = vec![0u8; 240 * 16];
    for (k, &c) in cols.iter().enumerate() {
        for r in 0..2 {
            let px = cell(&lang, ent(c, 4 + r));
            for y in 0..8 {
                for x in 0..8 {
                    banner[240 * (8 * r + y) + 8 * k + x] = px[8 * y + x];
                }
            }
        }
    }
    // An empty spot: the board's first circle (rows 7..8, columns 0..1).
    let mut spot = [0u8; 256];
    for r in 0..2 {
        for c in 0..2 {
            let px = cell(&lang, ent(c, 7 + r));
            for y in 0..8 {
                for x in 0..8 {
                    spot[16 * (8 * r + y) + 8 * c + x] = px[8 * y + x];
                }
            }
        }
    }
    Some(Art { icon_pal, icons, ui_pal, banner, spot })
}

fn art() -> Option<&'static Art> {
    static ART: OnceLock<Option<Art>> = OnceLock::new();
    ART.get_or_init(build_art).as_ref()
}

/// res_skill's icons: 0 none, then one per skill by id from 0x20.
const ICONS: usize = 44;

/// A skill's icon (Dual Strike's: by id from 0x20, icon 1 the first).
fn icon_of(id: u8) -> usize {
    (id as usize).saturating_sub(0x1F).min(ICONS - 1)
}

// --- AW2's font --------------------------------------------------------------------

const GLYPHS: u32 = 0x084C_32E4;
const WIDTHS: u32 = 0x084C_36E4;
/// The glyph rows a line shows (of 16).
const TEXT_TOP: usize = 3;
const LINE: usize = 12;

/// A Dual Strike text as one line of the font's characters.
fn plain(t: &[u8]) -> String {
    let s: String = t
        .iter()
        .map(|&c| match c {
            b'\r' | b'\n' | 0x0E | 0x0F => ' ',
            32..=126 => c as char,
            _ => ' ',
        })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn width(core: &Core, s: &str) -> usize {
    s.bytes().map(|c| core.raw_read_8(WIDTHS + c as u32, -1) as usize + 1).sum::<usize>().saturating_sub(1)
}

// --- The canvas ----------------------------------------------------------------------

/// BG palettes: 0 the board's (Dual Strike's, colour 0 white: the backdrop),
/// 1 the icons', 2 the icons' faded.
const UI: u8 = 0;
const ICON: u8 = 1;
const FADED: u8 = 2;
/// The board palette's colours used by the text and the lines.
const INK: u8 = 10;
const SHADE: u8 = 15;
const RED: u8 = 4;
/// A colour the board's tiles leave unused, made Dual Strike's line blue.
const BLUE: u8 = 11;
const LINE_BLUE: u16 = 29 << 10 | 8 << 5 | 5;
const ICON_RED: u8 = 7;
const ICON_DARK: u8 = 4;
/// The faded icons' colour least used by the icons (9 pixels), made dark
/// for the cursor over a faded icon.
const FADED_DARK: u8 = 12;
const WHITE: u16 = 0x7FFF;
/// White in all three palettes (Dual Strike's colour 1).
const OPAQUE_WHITE: u8 = 1;

struct Canvas {
    px: Vec<u8>,
    pal: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        Canvas { px: vec![0; 240 * 160], pal: vec![UI; 30 * 20] }
    }
    fn put(&mut self, x: i32, y: i32, pal: u8, v: u8) {
        if (0..240).contains(&x) && (0..160).contains(&y) {
            self.px[240 * y as usize + x as usize] = v;
            self.pal[30 * (y as usize / 8) + x as usize / 8] = pal;
        }
    }
    /// A dark pixel in whatever palette its cell has (the board's ink, or
    /// the icons' darkest).
    fn dark(&mut self, x: i32, y: i32) {
        if (0..240).contains(&x) && (0..160).contains(&y) {
            let p = self.pal[30 * (y as usize / 8) + x as usize / 8];
            self.px[240 * y as usize + x as usize] = match p {
                UI => INK,
                FADED => FADED_DARK,
                _ => ICON_DARK,
            };
        }
    }
    /// A red line pixel in whatever palette its cell has.
    fn red(&mut self, x: i32, y: i32) {
        if (0..240).contains(&x) && (0..160).contains(&y) {
            let p = self.pal[30 * (y as usize / 8) + x as usize / 8];
            self.px[240 * y as usize + x as usize] = if p == UI { RED } else { ICON_RED };
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, pal: u8, v: u8) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, pal, v);
            }
        }
    }
    /// A 16x16 picture (colour 0 left as it is).
    fn picture(&mut self, x: i32, y: i32, px: &[u8; 256], pal: u8) {
        for yy in 0..16 {
            for xx in 0..16 {
                let v = px[16 * yy + xx];
                if v != 0 {
                    self.put(x + xx as i32, y + yy as i32, pal, v);
                } else {
                    self.put(x + xx as i32, y + yy as i32, pal, 0);
                }
            }
        }
    }
    /// Text in AW2's font from (x, y), `rows` of each glyph from TEXT_TOP;
    /// returns the x after it.
    fn text(&mut self, core: &Core, x: i32, y: i32, s: &str, rows: usize) -> i32 {
        let mut x = x;
        for c in s.bytes() {
            let w = core.raw_read_8(WIDTHS + c as u32, -1) as usize;
            let at = core.raw_read_32(GLYPHS + 4 * c as u32, -1);
            if (0x0800_0000..0x0A00_0000).contains(&at) {
                let stride = w.div_ceil(2);
                for r in 0..rows {
                    for cx in 0..w {
                        let b = core.raw_read_8(at + (stride * (TEXT_TOP + r) + cx / 2) as u32, -1);
                        let v = (b >> (4 * (cx & 1))) & 15;
                        if v != 0 {
                            self.put(x + cx as i32, y + r as i32, UI, if v == 0xA { INK } else { SHADE });
                        }
                    }
                }
            }
            x += w as i32 + 1;
        }
        x
    }
}

/// Where a board cell's icon goes.
fn board_xy(col: usize, row: usize) -> (i32, i32) {
    (24 * col as i32 + 4, BOARD_Y + 16 * row as i32)
}

const BOARD_Y: i32 = 40;
const HELP_Y: i32 = 136;

fn compose(core: &mut Core, a: &Art) -> Canvas {
    let mut cv = Canvas::new();
    let co = core.raw_read_8(CO, -1);
    let n = co_skills::slots(core, co);
    let cur = ids(core);
    // The CO bar: name, rank, set; the set's slots at the right.
    let name = crate::co_new::ds_name(co).map(|t| plain(&t)).unwrap_or_default();
    let set = match code_set(core.raw_read_8(WHICH, -1)) {
        Set::Campaign => "Campaign",
        Set::Survival => "Survival",
        Set::WarRoom => "War Room",
        Set::Versus(_) => "Versus",
    };
    let x = cv.text(core, 4, 2, &name, LINE);
    let x = cv.text(core, x + 6, 2, &format!("Rank {}", co_skills::rank(core, co)), LINE);
    cv.text(core, x + 6, 2, set, LINE);
    cv.rect(0, 15, 240, 1, UI, BLUE);
    for k in 0..4 {
        let sx = 176 + 16 * k as i32;
        if k >= n {
            continue;
        }
        match cur[k] {
            0 => cv.picture(sx, -1, &a.spot, UI),
            id => cv.picture(sx, -1, &a.icons[icon_of(id)], ICON),
        }
    }
    // The banner and the ranks.
    for y in 0..16 {
        for x in 0..240 {
            let v = a.banner[240 * y + x];
            cv.put(x as i32, 16 + y as i32, UI, v);
        }
    }
    for col in 0..10 {
        let label = if col == 9 { "X".to_string() } else { format!("{}", col + 1) };
        let w = width(core, &label) as i32;
        let (bx, _) = board_xy(col, 0);
        cv.text(core, bx + 8 - w / 2, 32 - 4 + 1, &label, 11);
    }
    // The board.
    for (col, list) in board().iter().enumerate() {
        for row in 0..6 {
            let (x, y) = board_xy(col, row);
            match list.get(row) {
                None => cv.picture(x, y, &a.spot, UI),
                Some(&id) => {
                    let open = co_skills::unlocked(core, co, id);
                    cv.picture(x, y, &a.icons[icon_of(id)], if open { ICON } else { FADED });
                    if cur.contains(&id) {
                        for k in 0..16 {
                            cv.put(x + k, y, ICON, ICON_RED);
                            cv.put(x + k, y + 15, ICON, ICON_RED);
                            cv.put(x, y + k, ICON, ICON_RED);
                            cv.put(x + 15, y + k, ICON, ICON_RED);
                        }
                    }
                }
            }
        }
    }
    // The cursor: Dual Strike's corner brackets, two pixels thick and five
    // long, four pixels out from the cell.
    let (cx, cy) = board_xy(core.raw_read_8(COL, -1) as usize, core.raw_read_8(ROW, -1) as usize);
    let (lo_x, lo_y, hi_x, hi_y) = (cx - 4, cy - 4, cx + 19, cy + 19);
    for k in 0..5 {
        for t in 0..2 {
            for (x, y) in [
                (lo_x + k, lo_y + t),
                (lo_x + t, lo_y + k),
                (hi_x - k, lo_y + t),
                (hi_x - t, lo_y + k),
                (lo_x + k, hi_y - t),
                (lo_x + t, hi_y - k),
                (hi_x - k, hi_y - t),
                (hi_x - t, hi_y - k),
            ] {
                if y < HELP_Y {
                    cv.dark(x, y);
                }
            }
        }
    }
    // The help bar: the skill's icon, name and rank, its description.
    cv.rect(0, 158, 240, 2, UI, BLUE);
    if let Some(id) = under_cursor(core) {
        if let Some((rank, sname, desc)) = co_skills::info(id) {
            cv.picture(4, HELP_Y + 4, &a.icons[icon_of(id)], ICON);
            let x = cv.text(core, 24, HELP_Y + 1, &plain(&sname), LINE);
            let r = if rank == 10 { "Rank X".to_string() } else { format!("Rank {rank}") };
            cv.text(core, x + 6, HELP_Y + 1, &r, LINE);
            cv.text(core, 24, HELP_Y + 11, &plain(&desc), 158 - (HELP_Y as usize + 11));
        }
    }
    let hint = format!("{}/{}", cur.iter().take(n).filter(|&&x| x != 0).count(), n);
    let w = width(core, &hint) as i32;
    cv.text(core, 236 - w, HELP_Y + 1, &hint, LINE);
    for x in 0..240 {
        cv.red(x, HELP_Y);
        cv.red(x, HELP_Y + 1);
    }
    cv
}

/// The canvas as tiles (exact, the same ones shared), a 32x32 tilemap and
/// the three palettes.
fn tiles_of(cv: &Canvas) -> (Vec<u8>, Vec<u8>) {
    let mut tiles: Vec<[u8; 32]> = vec![[0u8; 32]];
    let mut index = std::collections::HashMap::new();
    index.insert([0u8; 32], 0u16);
    let mut map = vec![0u8; 0x800];
    for ty in 0..20 {
        for tx in 0..30 {
            let mut t = [0u8; 32];
            for y in 0..8 {
                for x in 0..8 {
                    // Colour 0 (Dual Strike's clear) as the palette's
                    // white: the screen is opaque (the game sets the
                    // backdrop colour itself).
                    let v = match cv.px[240 * (8 * ty + y) + 8 * tx + x] & 15 {
                        0 => OPAQUE_WHITE,
                        v => v,
                    };
                    t[4 * y + x / 2] |= v << (4 * (x & 1));
                }
            }
            let k = match index.get(&t) {
                Some(&k) => k,
                None if tiles.len() < 512 => {
                    let k = tiles.len() as u16;
                    tiles.push(t);
                    index.insert(t, k);
                    k
                }
                None => 0,
            };
            let e = k | (cv.pal[30 * ty + tx] as u16) << 12;
            map[2 * (32 * ty + tx)..2 * (32 * ty + tx) + 2].copy_from_slice(&e.to_le_bytes());
        }
    }
    (tiles.concat(), map)
}

fn palettes(a: &Art) -> Vec<u8> {
    let mut ui = a.ui_pal;
    ui[0] = WHITE;
    ui[BLUE as usize] = LINE_BLUE;
    let mut icon = a.icon_pal;
    icon[0] = WHITE;
    let mut faded = icon;
    for c in faded.iter_mut() {
        let f = |s: u16| (s + (31 - s) * 3 / 5) & 31;
        *c = f(*c & 31) | f((*c >> 5) & 31) << 5 | f((*c >> 10) & 31) << 10;
    }
    faded[FADED_DARK as usize] = 0x0421;
    [ui, icon, faded].iter().flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes())).collect()
}

// --- On the screen -------------------------------------------------------------------

const DISP_CT: u32 = 0x0300_30CC;
const BG0CNT: u32 = 0x0400_0008;
const LAYERS: u16 = 0x1F00;
const BG0_ONLY: u16 = 0x0100;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;
/// Where what the screen covers is kept while it is up (ROM image free space).
const BACKUP: u32 = 0x08EF_0000;
const CHARS_LEN: u32 = 0x4000;
const MAP_LEN: u32 = 0x800;

fn bg0(core: &Core) -> (u32, u32) {
    let c = core.raw_read_16(BG0CNT, -1) as u32;
    (0x0600_0000 + 0x4000 * ((c >> 2) & 3), 0x0600_0000 + 0x800 * ((c >> 8) & 31))
}

fn copy(core: &mut Core, from: u32, to: u32, len: u32) {
    let mut b = vec![0u8; len as usize];
    core.raw_read_range(from, -1, &mut b);
    core.raw_write_range(to, -1, &b);
}

fn write_if_changed(core: &mut Core, at: u32, b: &[u8]) {
    let mut now = vec![0u8; b.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != b {
        core.raw_write_range(at, -1, b);
    }
}

/// The screen drawn (what it covers copied aside first).
fn draw(core: &mut Core) {
    let Some(a) = art() else { return };
    let (chars, map) = bg0(core);
    if core.raw_read_8(BACKED, -1) != 1 {
        copy(core, chars, BACKUP, CHARS_LEN);
        copy(core, map, BACKUP + CHARS_LEN, MAP_LEN);
        copy(core, PAL_RAM, BACKUP + CHARS_LEN + MAP_LEN, 0x200);
        copy(core, PAL_BUFFER, BACKUP + CHARS_LEN + MAP_LEN + 0x200, 0x200);
        let d = core.raw_read_16(DISP_CT, -1);
        core.raw_write_16(DISP_SAVED, -1, d & LAYERS);
        core.raw_write_8(BACKED, -1, 1);
    }
    let cv = compose(core, a);
    let (tiles, tilemap) = tiles_of(&cv);
    write_if_changed(core, chars, &tiles);
    write_if_changed(core, map, &tilemap);
    let pal = palettes(a);
    write_if_changed(core, PAL_BUFFER, &pal);
    write_if_changed(core, PAL_RAM, &pal);
    let d = core.raw_read_16(DISP_CT, -1);
    core.raw_write_16(DISP_CT, -1, (d & !LAYERS) | BG0_ONLY);
}

/// The screen closed: what it covered put back, the layers as they were.
fn close(core: &mut Core) {
    core.raw_write_8(OPEN, -1, 0);
    if core.raw_read_8(BACKED, -1) == 1 {
        let (chars, map) = bg0(core);
        copy(core, BACKUP, chars, CHARS_LEN);
        copy(core, BACKUP + CHARS_LEN, map, MAP_LEN);
        copy(core, BACKUP + CHARS_LEN + MAP_LEN, PAL_RAM, 0x200);
        copy(core, BACKUP + CHARS_LEN + MAP_LEN + 0x200, PAL_BUFFER, 0x200);
        let d = core.raw_read_16(DISP_CT, -1);
        core.raw_write_16(DISP_CT, -1, (d & !LAYERS) | core.raw_read_16(DISP_SAVED, -1));
        core.raw_write_8(BACKED, -1, 0);
    }
}

/// At the VBlank sprite flush (crate::branding::flush, last): nothing of
/// the screen's (its cursor is in the picture; the game's sprites are off
/// while it is up). Returns the list's end.
pub fn flush(_core: &mut Core, _start: u32, at: u32, _end: u32) -> u32 {
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_fits() {
        // A halfword at an odd address is the one below it.
        assert_eq!(DISP_SAVED % 2, 0);
        assert!(STATE >= co_skills::VERSUS_RULE + 1 && STATE + STATE_LEN <= 0x0203_F600);
    }

    #[test]
    fn text() {
        assert_eq!(plain(b"Direct attack +5%"), "Direct attack +5%");
        assert_eq!(plain(b"Two\rlines"), "Two lines");
    }

    #[test]
    fn board_fits() {
        // Ten columns of 24 pixels on 240, six rows from 40 to the help bar.
        assert_eq!(board_xy(9, 0).0 + 16 + 4, 240);
        assert_eq!(board_xy(0, 5).1 + 16, HELP_Y);
        // The backup in the ROM image's free space (0x08EE0000..0x08EFFFFF).
        assert!(BACKUP >= 0x08EE_0000 && BACKUP + CHARS_LEN + MAP_LEN + 0x400 <= 0x08F0_0000);
    }
}
