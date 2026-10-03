//! A "tangoAW2" badge on the title screen and the Select Mode menu, drawn
//! by the game's own sprite hardware.
//!
//! While either screen is up (found by its process script, names from the
//! aw2bhr decompilation), the badge's tiles go into unused OBJ VRAM and its
//! colours into an unused OBJ palette, and a trap on the game's VBlank
//! sprite flush appends the badge's sprites to that frame's sprite list.
//! It is part of the game's own picture; the ROM file is untouched, and it
//! runs inside the emulated frame, so both netplay peers draw it alike.

use mgba::core::Core;

/// The game's process pool (`sProcArray`): 0x6C bytes each, script first.
const PROCS: u32 = 0x0200_D610;
const PROCS_END: u32 = 0x0200_E418;
const PROC_SIZE: u32 = 0x6C;
/// `ProcScr_TitleScreen`: the "PRESS START!" screen.
const TITLE_SCREEN: u32 = 0x0858_1CF8;
/// `ProcScr_MainMenu`: the SELECT MODE menu.
const MAIN_MENU: u32 = 0x0849_E818;

/// The VBlank flush of the frame's sprites (shadow OAM -> OAM, then clear).
pub const SPRITE_FLUSH: u32 = 0x0801_BBC4;
/// Where the next sprite of the frame goes, and the flushed area's
/// descriptor (source, destination, halfword pair count at + 0xA).
const NEXT_SPRITE: u32 = 0x0300_2F2C;
const FLUSH_AREA: u32 = 0x0300_0268;

/// OBJ tiles (1D mapping) and palette nothing on these screens uses.
const TITLE_TILES: u32 = 928;
const MENU_TILES: u32 = 992;
const PALETTE: u32 = 15;
const OBJ_VRAM: u32 = 0x0601_0000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

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
        'v' => [".....", ".....", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
        '.' => [".....", ".....", ".....", ".....", ".....", ".##..", ".##.."],
        '0' => [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
        '3' => ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
        '4' => ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
        '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
        '6' => ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."],
        '7' => ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
        '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        '9' => [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."],
        _ => [".....", ".....", ".....", ".....", ".....", ".....", "....."],
    }
}

/// Palette indices: 0 transparent, then navy, white, yellow, shadow.
const NAVY: u8 = 1;
const WHITE: u8 = 2;
const YELLOW: u8 = 3;
const SHADOW: u8 = 4;
const COLOURS: [u16; 5] = [
    0,
    bgr(24, 40, 104),
    bgr(248, 248, 248),
    bgr(248, 208, 48),
    bgr(8, 16, 48),
];

const fn bgr(r: u16, g: u16, b: u16) -> u16 {
    (r >> 3) | ((g >> 3) << 5) | ((b >> 3) << 10)
}

const TEXT: &str = "tangoAW2";

/// The app's version ("v0.3.1"), from the app crate's manifest at build
/// time, so every build of one version draws the same picture (netplay
/// peers of one version stay identical).
fn version() -> String {
    let manifest = include_str!("../../tango/Cargo.toml");
    let v = manifest
        .lines()
        .find_map(|l| l.trim().strip_prefix("version = \""))
        .and_then(|l| l.strip_suffix('"'))
        .unwrap_or("");
    format!("v{v}")
}

/// Draws `text` at (x0, y0) in the 5x7 font, 1 pixel per dot, with a shadow.
fn small_text(px: &mut [u8], w: usize, h: usize, x0: usize, y0: usize, text: &str, colour: u8) {
    for pass in 0..2 {
        for (i, ch) in text.chars().enumerate() {
            for (r, row) in glyph(ch).iter().enumerate() {
                for (c, b) in row.bytes().enumerate() {
                    if b != b'#' {
                        continue;
                    }
                    let (x, y) = (x0 + i * 6 + c + (pass == 0) as usize, y0 + r + (pass == 0) as usize);
                    if x < w && y < h {
                        px[y * w + x] = if pass == 0 { SHADOW } else { colour };
                    }
                }
            }
        }
    }
}

/// The badge at `scale` on a `w` x `h` canvas of palette indices, with the
/// version under it, right-aligned.
fn badge(scale: usize, w: usize, h: usize) -> Vec<u8> {
    let mut px = badge_only(scale, w, h);
    let bw = TEXT.len() * 6 * scale - scale + 6;
    let bh = 7 * scale + 6;
    // The version on its own small plate, right-aligned under the badge.
    let v = version();
    let (pw, ph) = (v.len() * 6 - 1 + 5, 7 + 5);
    let (px0, py0) = (bw.saturating_sub(pw), bh + 1);
    for y in 0..ph {
        for x in 0..pw {
            let corner = (x == 0 || x == pw - 1) && (y == 0 || y == ph - 1);
            let (cx, cy) = (px0 + x, py0 + y);
            if !corner && cx < w && cy < h {
                let edge = x == 0 || x == pw - 1 || y == 0 || y == ph - 1;
                px[cy * w + cx] = if edge { WHITE } else { NAVY };
            }
        }
    }
    small_text(&mut px, w, h, px0 + 2, py0 + 2, &v, YELLOW);
    px
}

fn badge_only(scale: usize, w: usize, h: usize) -> Vec<u8> {
    let mut px = vec![0u8; w * h];
    let bw = TEXT.len() * 6 * scale - scale + 6;
    let bh = 7 * scale + 6;
    let mut put = |x: usize, y: usize, c: u8| {
        if x < w && y < h {
            px[y * w + x] = c;
        }
    };
    for y in 0..bh {
        for x in 0..bw {
            let corner = (x == 0 || x == bw - 1) && (y == 0 || y == bh - 1);
            if !corner {
                let edge = x == 0 || x == bw - 1 || y == 0 || y == bh - 1;
                put(x, y, if edge { WHITE } else { NAVY });
            }
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
                            let x = 3 + i * 6 * scale + c * scale + dx;
                            let y = 3 + r * scale + dy;
                            if pass == 0 {
                                put(x + 1, y + 1, SHADOW);
                            } else {
                                put(x, y, colour);
                            }
                        }
                    }
                }
            }
        }
    }
    px
}

/// One sprite's tiles (1D mapping: its 8x8 tiles row by row), 4bpp, cut
/// from the canvas at (x0, y0).
fn sprite_tiles(px: &[u8], w: usize, x0: usize, y0: usize, sw: usize, sh: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for ty in 0..sh / 8 {
        for tx in 0..sw / 8 {
            for y in 0..8 {
                for x in (0..8).step_by(2) {
                    let at = |dx: usize| px[(y0 + ty * 8 + y) * w + x0 + tx * 8 + x + dx];
                    out.push(at(0) | (at(1) << 4));
                }
            }
        }
    }
    out
}

/// A badge's sprites: (x, y) on screen, 64x32 each, tiles from `first`.
struct Layout {
    first: u32,
    scale: usize,
    sprites: usize,
    x: i32,
    y: i32,
}

const TITLE: Layout = Layout {
    first: TITLE_TILES,
    scale: 2,
    sprites: 2,
    x: 240 - 102 - 6,
    y: 66,
};
const MENU: Layout = Layout {
    first: MENU_TILES,
    scale: 1,
    sprites: 1,
    x: 150,
    y: 8,
};

fn active(core: &Core) -> Option<&'static Layout> {
    if running(core, TITLE_SCREEN) {
        Some(&TITLE)
    } else if running(core, MAIN_MENU) {
        Some(&MENU)
    } else {
        None
    }
}

/// Every frame: while a badge screen is up, keep its tiles and palette in
/// place (the screen may have loaded over them since).
pub fn tick(core: &mut Core) {
    let Some(l) = active(core) else { return };
    let w = 64 * l.sprites;
    let px = badge(l.scale, w, 32);
    for s in 0..l.sprites {
        let tiles = sprite_tiles(&px, w, 64 * s, 0, 64, 32);
        let at = OBJ_VRAM + (l.first + 32 * s as u32) * 32;
        let mut now = vec![0u8; tiles.len()];
        core.raw_read_range(at, -1, &mut now);
        if now != tiles {
            core.raw_write_range(at, -1, &tiles);
        }
    }
    let mut pal = [0u8; 32];
    for (i, c) in COLOURS.iter().enumerate() {
        pal[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes());
    }
    for base in [PAL_BUFFER, PAL_RAM] {
        core.raw_write_range(base + 0x200 + PALETTE * 32, -1, &pal);
    }
}

/// Trap at [`SPRITE_FLUSH`]: append tangoAW2's sprites (the badge, the
/// Design Room's inventions) to the frame's sprite list just before it is
/// copied to OAM.
pub fn flush(core: &mut Core) {
    let start = core.raw_read_32(FLUSH_AREA, -1);
    let end = start + core.raw_read_16(FLUSH_AREA + 0xA, -1) as u32 * 8;
    let mut at = core.raw_read_32(NEXT_SPRITE, -1);
    if !(start..=end).contains(&at) {
        return;
    }
    crate::volcano::recolour(core, start, at);
    crate::mode_menu::remap(core, start, at);
    at = crate::design::flush_sprites(core, at, end);
    at = crate::survival::flush_sprites(core, at, end);
    at = crate::campaign_menu::flush(core, start, at, end);
    at = crate::ds_worldmap::flush_sprites(core, at, end);
    at = crate::panel_sprites::under_window(core, start, at, end);
    if !crate::panel_sprites::tiles_taken(core, start, at) {
        at = crate::two_front::flush_sprites(core, at, end);
    }
    at = crate::onyx::flush_sprites(core, start, at, end);
    if let Some(l) = active(core) {
        for s in 0..l.sprites {
            if at + 8 > end {
                break;
            }
            let x = (l.x + 64 * s as i32) as u16 & 0x1FF;
            let y = l.y as u16 & 0xFF;
            // 64x32: shape wide (1 << 14), size 3 (3 << 14 in attr1).
            let attr0 = y | (1 << 14);
            let attr1 = x | (3 << 14);
            // Priority 0, so it shows over the backgrounds.
            let attr2 = (l.first as u16 + 32 * s as u16) | ((PALETTE as u16) << 12);
            core.raw_write_16(at, -1, attr0);
            core.raw_write_16(at + 2, -1, attr1);
            core.raw_write_16(at + 4, -1, attr2);
            at += 8;
        }
    }
    at = crate::tag_ui::flush(core, start, at, end);
    at = crate::skills_panel::flush(core, start, at, end);
    crate::grand_bolt::flush_panel(core, start, at);
    core.raw_write_32(NEXT_SPRITE, -1, at);
}
