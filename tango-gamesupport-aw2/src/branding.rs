//! A "tangoAW2" badge on the title screen and the mode menu.
//!
//! Drawn over the finished picture ([`overlay`]), like the Design Room's
//! labels: nothing in the game's memory changes, so it can't affect a
//! match or a save, and the ROM file stays untouched.
//!
//! The screens are found the way the game finds them, by the process
//! scripts running (the aw2bhr decompilation's names).

use mgba::core::Core;

/// The game's process pool (`sProcArray`): 0x6C bytes each, script first.
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
/// `ProcScr_TitleScreen`: the "PRESS START!" screen.
const TITLE_SCREEN: u32 = 0x0858_1CF8;
/// `ProcScr_MainMenu`: the SELECT MODE menu.
const MAIN_MENU: u32 = 0x0849_E818;

fn running(core: &Core, script: u32) -> bool {
    (PROCS..PROCS_END)
        .step_by(PROC_SIZE as usize)
        .any(|p| core.raw_read_32(p, -1) == script)
}

/// 5x7 glyphs, rows top to bottom.
fn glyph(c: char) -> [&'static str; 7] {
    match c {
        't' => [".#...", ".#...", "####.", ".#...", ".#...", ".#..#", "..##."],
        'a' => [".....", ".....", ".###.", "....#", ".####", "#...#", ".####"],
        'n' => [".....", ".....", "#.##.", "##..#", "#...#", "#...#", "#...#"],
        'g' => [".....", ".####", "#...#", "#...#", ".####", "....#", ".###."],
        'o' => [".....", ".....", ".###.", "#...#", "#...#", "#...#", ".###."],
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'W' => ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"],
        '2' => [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
        _ => [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    }
}

const NAVY: [u8; 3] = [24, 40, 104];
const WHITE: [u8; 3] = [248, 248, 248];
const YELLOW: [u8; 3] = [248, 208, 48];
const SHADOW: [u8; 3] = [8, 16, 48];
const TEXT: &str = "tangoAW2";

fn put(rgba: &mut [u8], x: i32, y: i32, c: [u8; 3]) {
    if (0..240).contains(&x) && (0..160).contains(&y) {
        let i = ((y * 240 + x) * 4) as usize;
        rgba[i..i + 3].copy_from_slice(&c);
        rgba[i + 3] = 0xff;
    }
}

/// Width of the badge at `scale`.
fn badge_width(scale: i32) -> i32 {
    TEXT.len() as i32 * 6 * scale - scale + 6
}

/// Draws the badge with its top-left corner at (x0, y0).
fn badge(rgba: &mut [u8], x0: i32, y0: i32, scale: i32) {
    let (w, h) = (badge_width(scale), 7 * scale + 6);
    for y in 0..h {
        for x in 0..w {
            let corner = (x == 0 || x == w - 1) && (y == 0 || y == h - 1);
            if corner {
                continue;
            }
            let edge = x == 0 || x == w - 1 || y == 0 || y == h - 1;
            put(rgba, x0 + x, y0 + y, if edge { WHITE } else { NAVY });
        }
    }
    for pass in 0..2 {
        for (i, ch) in TEXT.chars().enumerate() {
            let colour = if i < 5 { WHITE } else { YELLOW };
            for (r, row) in glyph(ch).iter().enumerate() {
                for (c, b) in row.bytes().enumerate() {
                    if b != b'#' {
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let x = x0 + 3 + i as i32 * 6 * scale + c as i32 * scale + dx;
                            let y = y0 + 3 + r as i32 * scale + dy;
                            if pass == 0 {
                                put(rgba, x + 1, y + 1, SHADOW);
                            } else {
                                put(rgba, x, y, colour);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn overlay(core: &Core, rgba: &mut [u8]) {
    if running(core, TITLE_SCREEN) {
        // Under the Advance Wars 2 logo, right-aligned with it.
        badge(rgba, 240 - badge_width(2) - 6, 66, 2);
    } else if running(core, MAIN_MENU) {
        // Beside the SELECT MODE heading.
        badge(rgba, 150, 8, 1);
    }
}
