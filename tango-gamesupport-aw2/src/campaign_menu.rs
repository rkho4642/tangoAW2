//! The Select Mode menu's Campaign entry with the Dual Strike pack: a small
//! sub-menu first, "AW2 CAMPAIGN" (then AW2's own Continue / New, unchanged)
//! or "DS CAMPAIGN" (then Continue / New of the DS Campaign,
//! [`crate::ds_campaign`]). Without the pack the menu is AW2's own.
//!
//! The menu is AW2's carousel (`MainMenuCarouselWheel_InputLoop`, the proc
//! running one of [`WHEELS`]): Campaign opens a box of two rows (Continue, New),
//! its cursor at +0x66 (6 or 7: the row is its parity), open while +0x64 is
//! positive. tangoAW2 keeps a level ([`LEVEL`]): 0 the chooser (the box's
//! rows show "AW2 CAMPAIGN" and "DS CAMPAIGN"; UP/DOWN and A are taken
//! from the game), 1 AW2's own box, 2 the DS Campaign's box (B goes back to
//! the chooser), 3 its Normal / Hard choice: once a Normal campaign has
//! been cleared (as Dual Strike opens Hard), New in the DS box asks
//! for the difficulty in the chooser's style (A takes it and goes on with
//! New, B back to the box); before that New starts Normal. In the DS box the game's "is there a campaign to
//! continue" (`GetCampaignSaveFlag`, trapped) answers for the DS Campaign,
//! and the choice is passed on through [`crate::ds_campaign::REQUEST`].
//!
//! The chooser's labels are drawn like the game's own (its label sprites,
//! 48x16 at OBJ tiles 664 and 676, in its 5-row-high box style): two 80x16
//! labels in free OBJ tiles ([`TILES`]), put in place of the game's at the
//! VBlank sprite flush ([`flush`], called by [`crate::branding::flush`]).

use mgba::core::Core;

/// The game's process pool (`sProcArray`): 0x6C bytes each, script first.
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
/// `ProcScr_MainMenu`: the SELECT MODE menu.
const MAIN_MENU: u32 = 0x0849_E818;
/// The carousel wheel's proc scripts (their input loop is
/// `MainMenuCarouselWheel_InputLoop`, 0x08081D30): the one started on
/// entering Select Mode, and the one running when the menu comes back from
/// a mode (`sub_08081334` starts the first unless the second is up).
const WHEELS: [u32; 2] = [0x0861_6A08, 0x0861_6A40];
/// Campaign's item id on the carousel (its items by position:
/// [`crate::mode_menu::item`]).
const W_INDEX: u32 = 0x52;
const W_BOX: u32 = 0x64;
const W_CURSOR: u32 = 0x66;
const W_CHOSEN: u32 = 0x6A;
const CAMPAIGN: u8 = 0;
/// The box's first row's cursor value (6: top row, 7: bottom).
const ROW0: u16 = 6;

/// tangoAW2's menu state (EWRAM, crate::ds_campaign's block).
pub const LEVEL: u32 = crate::ds_campaign::MENU_LEVEL;
/// The chooser's row: 0 AW2, 1 DS.
pub const CHOICE: u32 = crate::ds_campaign::MENU_CHOICE;

/// `GetCampaignSaveFlag` (returns `gUnknown_03003F30[1]`).
pub const SAVE_FLAG: u32 = 0x0803_BC7C;

/// Free OBJ tiles on the Select Mode screen (the game uses up to 734 and
/// 768..771; Survival's small label 800..831, crate::mode_menu; tangoAW2's
/// badge 992..): two 80x16 labels, 36 tiles each.
const TILES: u32 = 832;
const OBJ_VRAM: u32 = 0x0601_0000;
/// The game's label tiles: the top row's and the bottom row's.
const GAME_LABEL_TILES: [u16; 2] = [664, 676];

const KEY_A: u32 = 1 << 0;
const KEY_B: u32 = 1 << 1;
const KEY_UP: u32 = 1 << 6;
const KEY_DOWN: u32 = 1 << 7;

fn proc_with(core: &Core, script: u32) -> Option<u32> {
    (PROCS..PROCS_END).step_by(PROC_SIZE as usize).find(|&p| core.raw_read_32(p, -1) == script)
}

fn wheel(core: &Core) -> Option<u32> {
    (PROCS..PROCS_END).step_by(PROC_SIZE as usize).find(|&p| WHEELS.contains(&core.raw_read_32(p, -1)))
}

/// The Select Mode menu is up (entered from the title, or come back to
/// from a mode: then only the wheel's proc may be running).
pub fn on_select_mode(core: &Core) -> bool {
    proc_with(core, MAIN_MENU).is_some() || wheel(core).is_some()
}

/// The carousel wheel proc, with Campaign's box open.
fn campaign_box(core: &Core) -> Option<u32> {
    let p = wheel(core)?;
    let index = core.raw_read_16(p + W_INDEX, -1) as u32;
    let kind = crate::mode_menu::item(core, index);
    let open = (core.raw_read_16(p + W_BOX, -1) as i16) > 0;
    (kind == CAMPAIGN && open).then_some(p)
}

/// Whether the chooser (AW2 / DS, or Normal / Hard) is showing.
pub fn chooser(core: &Core) -> bool {
    crate::ds_weather::is_on(core) && campaign_box(core).is_some() && matches!(core.raw_read_8(LEVEL, -1), 0 | 3)
}

/// The Normal / Hard choice's row (0 Normal, 1 Hard).
pub const DIFFICULTY: u32 = 0x0203_FD5F;

/// The box's help lines (text ids 0x9C2 "Continue a campaign in
/// progress." and 0x9C3 "Start a new campaign.": their text table words)
/// and AW2's texts; under the Normal / Hard choice they are the choice's.
const HELP_WORDS: [u32; 2] = [0x0861_3140, 0x0861_3144];
const AW2_HELP: [u32; 2] = [0x0860_7684, 0x0860_76A8];

fn help_lines(core: &mut Core) {
    let ds = (core.raw_read_8(LEVEL, -1) == 3).then(|| crate::ds_campaign::campaign(core).map(|c| c.help)).flatten();
    for (k, &at) in HELP_WORDS.iter().enumerate() {
        let want = ds.map_or(AW2_HELP[k], |h| h[k]);
        if core.raw_read_32(at, -1) != want {
            core.raw_write_32(at, -1, want);
        }
    }
}

/// The chooser's entries: AW2's own campaign first, then every campaign
/// whose source has it ([`crate::campaign_model::SOURCES`]: today Dual
/// Strike's, with its pack). The box shows two at a time ([`TOP`] the
/// first shown); UP and DOWN go through all of them (wrapping), the two
/// shown following the cursor. A on the first opens AW2's own box (level
/// 1), on another that campaign's box (level 2).
pub fn entries(core: &Core) -> Vec<&'static str> {
    let mut v = vec![LABELS[0]];
    v.extend(crate::campaign_model::SOURCES.iter().filter(|s| (s.available)(core)).map(|s| s.label));
    v
}

/// The chooser's first entry shown (EWRAM, crate::ds_campaign's block).
const TOP: u32 = 0x0203_FD56;

/// The chooser's window: the first entry shown, the cursor's row in it.
pub fn window(choice: usize, top: usize, n: usize) -> (usize, usize) {
    let last = n.saturating_sub(2);
    let mut top = top.min(last);
    if choice < top {
        top = choice;
    } else if choice > top + 1 {
        top = choice - 1;
    }
    (top, choice - top)
}

/// The labels showing and the highlighted row.
fn shown(core: &Core) -> ([&'static str; 2], usize) {
    if core.raw_read_8(LEVEL, -1) == 3 {
        return (DIFFICULTY_LABELS, core.raw_read_8(DIFFICULTY, -1) as usize & 1);
    }
    let all = entries(core);
    let n = all.len();
    let choice = (core.raw_read_8(CHOICE, -1) as usize).min(n.saturating_sub(1));
    let (top, row) = window(choice, core.raw_read_8(TOP, -1) as usize, n);
    let label = |k: usize| all.get(k).copied().unwrap_or("");
    ([label(top), label(top + 1)], row)
}

/// Every frame, before the game runs: the keys the game gets.
pub fn tick(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    let keys = tick_menu(core, ds, keys, prev);
    help_lines(core);
    keys
}

fn tick_menu(core: &mut Core, ds: bool, keys: u32, prev: u32) -> u32 {
    if !ds {
        return keys;
    }
    let Some(p) = campaign_box(core) else {
        // Box closed (or elsewhere): back to the chooser, nothing requested
        // unless a choice was just made.
        if core.raw_read_8(LEVEL, -1) != 0 && wheel(core).is_some_and(|w| core.raw_read_16(w + W_CHOSEN, -1) == 0) {
            core.raw_write_8(LEVEL, -1, 0);
            core.raw_write_8(crate::ds_campaign::REQUEST, -1, 0);
        }
        return keys;
    };
    if core.raw_read_16(p + W_CHOSEN, -1) != 0 {
        return keys;
    }
    let pressed = keys & !prev;
    let mut keys = keys;
    match core.raw_read_8(LEVEL, -1) {
        0 => {
            let n = entries(core).len().max(1);
            let mut choice = (core.raw_read_8(CHOICE, -1) as usize).min(n - 1);
            if pressed & KEY_UP != 0 {
                choice = (choice + n - 1) % n;
            } else if pressed & KEY_DOWN != 0 {
                choice = (choice + 1) % n;
            }
            let (top, row) = window(choice, core.raw_read_8(TOP, -1) as usize, n);
            core.raw_write_8(CHOICE, -1, choice as u8);
            core.raw_write_8(TOP, -1, top as u8);
            core.raw_write_16(p + W_CURSOR, -1, ROW0 + row as u16);
            if pressed & KEY_A != 0 {
                core.raw_write_8(LEVEL, -1, if choice == 0 { 1 } else { 2 });
                let has = if choice == 0 { core.raw_read_8(0x0300_3F31, -1) != 0 } else { crate::ds_campaign::has_save(core) };
                core.raw_write_16(p + W_CURSOR, -1, if has { ROW0 } else { ROW0 + 1 });
            }
            keys &= !(KEY_UP | KEY_DOWN | KEY_A);
        }
        3 => {
            // Normal / Hard after New in the DS box.
            let mut row = core.raw_read_8(DIFFICULTY, -1) & 1;
            if pressed & (KEY_UP | KEY_DOWN) != 0 {
                row ^= 1;
                core.raw_write_8(DIFFICULTY, -1, row);
            }
            core.raw_write_16(p + W_CURSOR, -1, ROW0 + row as u16);
            if pressed & KEY_B != 0 {
                core.raw_write_8(LEVEL, -1, 2);
                core.raw_write_16(p + W_CURSOR, -1, ROW0 + 1);
                keys &= !(KEY_UP | KEY_DOWN | KEY_B);
            } else if pressed & KEY_A != 0 {
                // On with New: the box's cursor back on New, A to the game.
                core.raw_write_8(crate::ds_campaign::HARD_REQUEST, -1, row);
                core.raw_write_8(LEVEL, -1, 2);
                core.raw_write_16(p + W_CURSOR, -1, ROW0 + 1);
                core.raw_write_8(crate::ds_campaign::REQUEST, -1, 1);
                keys &= !(KEY_UP | KEY_DOWN);
            } else {
                keys &= !(KEY_UP | KEY_DOWN | KEY_A);
            }
        }
        level => {
            if pressed & KEY_B != 0 {
                // Back to the chooser on this box's entry.
                let n = entries(core).len().max(1);
                let choice = (core.raw_read_8(CHOICE, -1) as usize).min(n - 1);
                let (top, row) = window(choice, core.raw_read_8(TOP, -1) as usize, n);
                core.raw_write_8(LEVEL, -1, 0);
                core.raw_write_8(TOP, -1, top as u8);
                core.raw_write_16(p + W_CURSOR, -1, ROW0 + row as u16);
                core.raw_write_8(crate::ds_campaign::REQUEST, -1, 0);
                keys &= !KEY_B;
            } else if level == 2 {
                // The DS box: Continue (top) or New; New with Hard open
                // asks Normal / Hard first.
                let row = core.raw_read_16(p + W_CURSOR, -1) % 2;
                if row == 1 && pressed & KEY_A != 0 && crate::ds_campaign::hard_open(core) {
                    core.raw_write_8(LEVEL, -1, 3);
                    core.raw_write_8(DIFFICULTY, -1, 0);
                    core.raw_write_16(p + W_CURSOR, -1, ROW0);
                    keys &= !KEY_A;
                } else {
                    core.raw_write_8(crate::ds_campaign::REQUEST, -1, if row == 0 { 2 } else { 1 });
                    if pressed & KEY_A != 0 {
                        core.raw_write_8(crate::ds_campaign::HARD_REQUEST, -1, 0);
                    }
                }
            }
        }
    }
    keys
}

/// `GetCampaignSaveFlag`: in the DS box, whether there is a DS Campaign to
/// continue.
pub fn save_flag(core: &mut Core) {
    if !crate::ds_weather::is_on(core) || core.raw_read_8(LEVEL, -1) != 2 {
        return;
    }
    let v = crate::ds_campaign::has_save(core) as i32;
    let cpu = core.gba_mut().cpu_mut();
    let lr = cpu.gpr(14) as u32;
    cpu.set_gpr(0, v);
    cpu.set_thumb_pc(lr & !1);
}

// --- The chooser's labels ------------------------------------------------------

/// 5x10 glyphs, '#' the text colour.
fn glyph(c: char) -> &'static [&'static str] {
    match c {
        'A' => &[".###.", "#####", "##.##", "##.##", "##.##", "#####", "#####", "##.##", "##.##", "##.##"],
        'W' => &["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", "#####", "#####", "##.##", "#...#"],
        '2' => &[".###.", "#####", "##.##", "...##", "..###", ".###.", "###..", "##...", "#####", "#####"],
        'C' => &[".###.", "#####", "##.##", "##...", "##...", "##...", "##...", "##.##", "#####", ".###."],
        'M' => &["#...#", "##.##", "#####", "#####", "#.#.#", "#.#.#", "#...#", "#...#", "#...#", "#...#"],
        'P' => &["####.", "#####", "##.##", "##.##", "#####", "####.", "##...", "##...", "##...", "##..."],
        'I' => &["##", "##", "##", "##", "##", "##", "##", "##", "##", "##"],
        'G' => &[".####", "#####", "##...", "##...", "##.##", "##.##", "##.##", "##.##", "#####", ".####"],
        'N' => &["#...#", "##..#", "###.#", "#####", "#.###", "#..##", "#...#", "#...#", "#...#", "#...#"],
        'D' => &["####.", "#####", "##.##", "##.##", "##.##", "##.##", "##.##", "##.##", "#####", "####."],
        'S' => &[".####", "#####", "##...", "##...", "####.", ".####", "...##", "...##", "#####", "####."],
        'O' => &[".###.", "#####", "##.##", "##.##", "##.##", "##.##", "##.##", "##.##", "#####", ".###."],
        'R' => &["####.", "#####", "##.##", "##.##", "#####", "####.", "##.##", "##.##", "##.##", "##.##"],
        'L' => &["##...", "##...", "##...", "##...", "##...", "##...", "##...", "##...", "#####", "#####"],
        'H' => &["##.##", "##.##", "##.##", "##.##", "#####", "#####", "##.##", "##.##", "##.##", "##.##"],
        _ => &["...", "...", "...", "...", "...", "...", "...", "...", "...", "..."],
    }
}

const LABEL_W: usize = 80;
const LABEL_H: usize = 16;
pub const LABELS: [&str; 2] = ["AW2 CAMPAIGN", "DS CAMPAIGN"];
pub const DIFFICULTY_LABELS: [&str; 2] = ["NORMAL", "HARD"];

/// A label in the game's style: palette index 15 the outer 2-pixel border, 1
/// the box, 5 the text.
fn label_pixels(text: &str) -> Vec<u8> {
    let mut px = vec![15u8; LABEL_W * LABEL_H];
    for y in 2..LABEL_H - 2 {
        for x in 2..LABEL_W - 2 {
            px[y * LABEL_W + x] = 1;
        }
    }
    let width: usize = text.chars().map(|c| glyph(c)[0].len() + 1).sum::<usize>() - 1;
    let mut x0 = (LABEL_W - width) / 2;
    for c in text.chars() {
        let g = glyph(c);
        for (r, row) in g.iter().enumerate() {
            for (k, b) in row.bytes().enumerate() {
                if b == b'#' {
                    px[(3 + r) * LABEL_W + x0 + k] = 5;
                }
            }
        }
        x0 += g[0].len() + 1;
    }
    px
}

/// Tiles a label takes: a 64x32 sprite (its lower half empty) and a 16x16.
const LABEL_TILES: u32 = 36;

/// 4bpp tiles of an 80x16 label cut into a 64x32 sprite (the top 16 rows
/// used) and a 16x16 one (1D mapping, each sprite's tiles row by row).
fn label_tiles(text: &str) -> Vec<u8> {
    let px = label_pixels(text);
    let pixel = |x: usize, y: usize| if y < LABEL_H { px[y * LABEL_W + x] } else { 0 };
    let mut out = Vec::new();
    for (x0, w, h) in [(0usize, 64usize, 32usize), (64, 16, 16)] {
        for ty in 0..h / 8 {
            for tx in 0..w / 8 {
                for y in 0..8 {
                    for x in (0..8).step_by(2) {
                        let (cx, cy) = (x0 + tx * 8 + x, ty * 8 + y);
                        out.push(pixel(cx, cy) | (pixel(cx + 1, cy) << 4));
                    }
                }
            }
        }
    }
    out
}

/// Every frame on the Select Mode screen with the pack: the labels' tiles.
pub fn draw(core: &mut Core) {
    if !chooser(core) {
        return;
    }
    let (labels, _) = shown(core);
    for (k, t) in labels.iter().enumerate().filter(|(_, t)| !t.is_empty()) {
        let tiles = label_tiles(t);
        let at = OBJ_VRAM + (TILES + LABEL_TILES * k as u32) * 32;
        let mut now = vec![0u8; tiles.len()];
        core.raw_read_range(at, -1, &mut now);
        if now != tiles {
            core.raw_write_range(at, -1, &tiles);
        }
    }
}

/// At the sprite flush: in the chooser, the game's two label sprites (each
/// a 32x16 and a 16x16 piece) become tangoAW2's 80x16 labels in the same
/// place, palette and drawing order (the same sprite slots). Returns the
/// end of the sprite list.
pub fn flush(core: &mut Core, start: u32, at: u32, end: u32) -> u32 {
    let _ = end;
    if !chooser(core) {
        return at;
    }
    for (k, &first) in GAME_LABEL_TILES.iter().enumerate() {
        let (mut wide, mut square) = (None, None);
        let mut s = start;
        while s < at {
            let tile = core.raw_read_16(s + 4, -1) & 0x3FF;
            if tile == first {
                wide = Some(s);
            } else if tile == first + 8 {
                square = Some(s);
            }
            s += 8;
        }
        let (Some(w), Some(q)) = (wide, square) else { continue };
        let (a0, a1, a2) = (core.raw_read_16(w, -1), core.raw_read_16(w + 2, -1), core.raw_read_16(w + 4, -1));
        // The box is shown through a window from x 144 (the game's labels' x):
        // the wider label grows to the right.
        let (x, y) = ((a1 & 0x1FF) as i32, a0 & 0xFF);
        // The highlighted row in the game's "selected" palette (8), the
        // other in its plain one (10): the game recolours its labels only
        // on its own cursor moves.
        let (_, row) = shown(core);
        let pal = if k == row { SELECTED_PALETTE } else { PLAIN_PALETTE };
        let style = (a2 & 0x0C00) | (pal << 12); // priority, palette
        let tile = (TILES + LABEL_TILES * k as u32) as u16;
        // 64x32 (wide, size 3), then 16x16 (square, size 1).
        core.raw_write_16(w, -1, y | (1 << 14));
        core.raw_write_16(w + 2, -1, (x as u16 & 0x1FF) | (3 << 14));
        core.raw_write_16(w + 4, -1, tile | style);
        core.raw_write_16(q, -1, y);
        core.raw_write_16(q + 2, -1, ((x + 64) as u16 & 0x1FF) | (1 << 14));
        core.raw_write_16(q + 4, -1, (tile + 32) | style);
    }
    // The box's right-hand cursor arrow (OBJ tile 768) moves out past the
    // wider labels, next to the highlighted row.
    let row = shown(core).1 as u16;
    let mut s = start;
    while s < at {
        let (a0, a1, a2) = (core.raw_read_16(s, -1), core.raw_read_16(s + 2, -1), core.raw_read_16(s + 4, -1));
        let x = a1 & 0x1FF;
        if a2 & 0x3FF == ARROW_TILE && (176..=208).contains(&x) {
            core.raw_write_16(s, -1, (a0 & !0xFF) | (ARROW_Y + 16 * row));
            core.raw_write_16(s + 2, -1, (a1 & !0x1FF) | (x + 32));
        }
        s += 8;
    }
    at
}

const ARROW_TILE: u16 = 768;
/// The arrow's y beside the top row.
const ARROW_Y: u16 = 60;
const SELECTED_PALETTE: u16 = 8;
const PLAIN_PALETTE: u16 = 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_follows_the_cursor() {
        // Two entries: both shown, the cursor's row its entry.
        assert_eq!(window(0, 0, 2), (0, 0));
        assert_eq!(window(1, 0, 2), (0, 1));
        // Four: the window moves down and back up with the cursor.
        assert_eq!(window(2, 0, 4), (1, 1));
        assert_eq!(window(3, 1, 4), (2, 1));
        assert_eq!(window(1, 2, 4), (1, 0));
        assert_eq!(window(0, 3, 4), (0, 0));
    }

    #[test]
    fn labels_fit() {
        let sources = crate::campaign_model::SOURCES.iter().map(|s| s.label);
        for t in LABELS.iter().copied().chain(DIFFICULTY_LABELS).chain(sources) {
            let w: usize = t.chars().map(|c| glyph(c)[0].len() + 1).sum::<usize>() - 1;
            assert!(w + 6 <= LABEL_W, "{t}");
            assert_eq!(label_tiles(t).len(), LABEL_TILES as usize * 32);
        }
    }
}
