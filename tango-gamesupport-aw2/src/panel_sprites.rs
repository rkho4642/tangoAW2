//! The panels' lines (crate::two_front: the Front help line, the Front
//! view's window, the second front's result; the Setup phase's help line)
//! are priority 0 sprites over a window on BG2. Two things keep them clean,
//! at the sprite flush (crate::branding::flush, `start..at` the game's
//! sprites):
//!
//! - [`under_window`]: AW2's own windows on the battle map draw their text on
//!   the BG, so the map's sprites behind BG2 (units, structures' tops) are
//!   simply covered. Under sprite lines they are not: where a sprite of the
//!   lines overlaps a sprite of lower priority, its clear pixels lift that
//!   sprite over BG2 (the GBA's OBJ priority quirk, as mGBA has it): Means to
//!   an End's Black Crystal showed through "Second front", units and
//!   buildings through the help lines. While a panel is drawn, the game's
//!   sprites behind BG2 are taken out of its window: one wholly inside is
//!   hidden; one across its top or bottom keeps its rows of 8 pixels outside
//!   (strips of its tiles, 1D mapping).
//! - [`tiles_taken`]: the lines' OBJ tiles are free on the battle map but for
//!   a few pictures: a capture's (its 64x64 at tile 0x1CA reaches 0x209) and
//!   a Black Crystal's heal (crate::heal_effect's, in the same tiles). While
//!   one is up the lines are left out, so the picture is not written over
//!   (the capture's "20" row showed pieces of "Second front").

use mgba::core::Core;

/// OBJ sizes (width, height) by shape (square, wide, tall) and size.
const OBJ_SIZES: [[(i32, i32); 4]; 4] = [
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    [(8, 16), (8, 32), (16, 32), (32, 64)],
    [(8, 8), (8, 8), (8, 8), (8, 8)],
];
const BG2CNT: u32 = 0x0400_000C;
const DISPCNT: u32 = 0x0400_0000;

/// The game's sprites behind BG2 taken out of the drawn panel's window.
/// Returns the end of the list.
pub fn under_window(core: &mut Core, start: u32, mut at: u32, end: u32) -> u32 {
    let Some((left, top, right, bottom)) = crate::two_front::drawn_panel(core) else { return at };
    let bg2 = core.raw_read_16(BG2CNT, -1) & 3;
    let one_d = core.raw_read_16(DISPCNT, -1) & (1 << 6) != 0;
    let game_end = at;
    let mut s = start;
    while s + 8 <= game_end {
        let (a0, a1, a2) = (core.raw_read_16(s, -1), core.raw_read_16(s + 2, -1), core.raw_read_16(s + 4, -1));
        let entry = s;
        s += 8;
        // Shown, regular (not affine), behind BG2.
        if a0 & 0x300 != 0 || (a2 >> 10) & 3 <= bg2 {
            continue;
        }
        let (sw, sh) = OBJ_SIZES[((a0 >> 14) & 3) as usize][((a1 >> 14) & 3) as usize];
        let (ox, oy) = ((a1 & 0x1FF) as i32, (a0 & 0xFF) as i32);
        let ox = if ox >= 240 { ox - 512 } else { ox };
        let oy = if oy >= 160 { oy - 256 } else { oy };
        if ox + sw <= left || ox >= right || oy + sh <= top || oy >= bottom {
            continue;
        }
        let hide = (a0 & !0x300) | 0x200;
        if ox >= left && oy >= top && ox + sw <= right && oy + sh <= bottom {
            core.raw_write_16(entry, -1, hide);
            continue;
        }
        // Across the top or the bottom (not a side): its rows outside.
        if ox < left || ox + sw > right || !one_d || sw > 32 {
            continue;
        }
        let colour8 = a0 & (1 << 13) != 0;
        let per_row = (sw / 8) as u16 * if colour8 { 2 } else { 1 };
        // w x 8: 8x8 (square, 0), 16x8 (wide, 0), 32x8 (wide, 1).
        let (shape, size) = match sw {
            8 => (0u16, 0u16),
            16 => (1, 0),
            _ => (1, 1),
        };
        core.raw_write_16(entry, -1, hide);
        for r in 0..sh / 8 {
            let ry = oy + 8 * r;
            if ry + 8 > top && ry < bottom {
                continue;
            }
            if at + 8 > end {
                break;
            }
            core.raw_write_16(at, -1, (a0 & !(0xC000 | 0x3FF)) | (shape << 14) | (ry as u16 & 0xFF));
            core.raw_write_16(at + 2, -1, (a1 & !0xC000) | (size << 14));
            core.raw_write_16(at + 4, -1, (a2 & !0x3FF) | (((a2 & 0x3FF) + per_row * r as u16) & 0x3FF));
            core.raw_write_16(at + 6, -1, 0);
            at += 8;
        }
    }
    at
}

/// Whether the game draws from the lines' OBJ tiles this frame (or a heal
/// plays, which draws after the flush's check).
pub fn tiles_taken(core: &Core, start: u32, at: u32) -> bool {
    if crate::heal_effect::playing(core) {
        return true;
    }
    let ours: Vec<(u16, u16)> = crate::two_front::line_tiles().into_iter().map(|t| (t, t + 2)).collect();
    let mut s = start;
    while s + 8 <= at {
        let (a0, a1, a2) = (core.raw_read_16(s, -1), core.raw_read_16(s + 2, -1), core.raw_read_16(s + 4, -1));
        s += 8;
        // (hidden, or parked below the screen)
        if a0 & 0x300 == 0x200 || (160..192).contains(&(a0 & 0xFF)) {
            continue;
        }
        let (w, h) = OBJ_SIZES[((a0 >> 14) & 3) as usize][((a1 >> 14) & 3) as usize];
        let colour8 = a0 & (1 << 13) != 0;
        let first = a2 & 0x3FF;
        let n = ((w / 8) * (h / 8)) as u16 * if colour8 { 2 } else { 1 };
        if ours.iter().any(|&(a, b)| first < b && a < first + n) {
            return true;
        }
    }
    false
}
