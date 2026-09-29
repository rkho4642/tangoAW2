//! Dual Strike's map-unit pictures for the seven units AW2 lacks, converted
//! at run time from the Dual Strike pack ([`crate::ds_pack`]) into AW2's
//! own map-unit formats. Converting only: the caller puts the tiles in VRAM.
//!
//! The units, by Dual Strike id ([`NEW_UNITS`]): 4 Megatank, 9 Piperunner,
//! 12 Stealth, 13 Black Bomb, 18 Black Boat, 25 Carrier, 26 Oozium. Dual
//! Strike numbers its units like AW2 (Infantry 1 .. Sub 24), so every id
//! 1..24 converts too, which is how the conversion is checked against
//! AW2's own art.
//!
//! **Idle (standing) units.** AW2: 16x16 BG tiles, 4bpp, four tiles per
//! slot in the order TL, TR, BL, BR, three frames (`0x0810BE60`, 0xD80
//! bytes a frame). Dual Strike: `bmap/01d`, `01e`, `01f` (LZ10), the same
//! three frames, each 34 bitmaps of 32x32, 4bpp rows (low nibble first),
//! stacked. Overlay 0 at `0x022F5D88` is u16 `[5][27]`, the bitmap per
//! country and unit id (the countries differ only for Infantry and Mech;
//! [`map_tiles`] uses the first row, Orange Star's).
//!   The pictures are drawn at twice AW2's size: 2x2 blocks aligned on even
//! pixels, with some hand edits, and the 32x32 box is AW2's 16x16 cell
//! (Tank: x 2..27, y 6..31 in Dual Strike, x 1..13, y 3..15 in AW2). Each
//! 2x2 block becomes the colour most of its pixels have; a tie goes to
//! transparent, then to the darker colour (by Orange Star's palette), so
//! outlines survive. Both games draw the units facing left (AW2 flips
//! armies 1 and 3 itself).
//!
//! **Moving units.** AW2: an OBJ sheet per unit type (LZ77, table
//! `0x0849CD88`), 24x24 frames of 9 tiles, tile k at column 2 - k%3, row
//! 2 - k/3 (the animation descriptor places tile 0 bottom right), frames
//! side (facing left; right is the same flipped), down, up, three each for
//! foot, vehicle and plane types and two each for copters and ships.
//! Dual Strike: `mu/001..026`, raw 4bpp, 32x32 frames of 16 tiles in rows,
//! the same frame order and counts (nine or six frames). Between them are
//! small animation headers (`mu/000`, `003`, `00e`, `014`, `016`); the
//! n-th picture file is unit id n. The headers' scripts match AW2's frame
//! for frame (foot 8/5/8/5 ticks, vehicle 5/3/5, plane 8/5/8, ship 32/9,
//! copter 1/2), so AW2's descriptor for the class plays them as Dual Strike
//! does. Dual Strike's 32x32 box is anchored at (-16, -23), AW2's 24x24 at
//! (-12, -15), so AW2's frame is x 4..28, y 8..32 of Dual Strike's.
//!
//! **Colours.** Dual Strike's unit palettes (`bmap/046..04a`, Orange Star
//! .. Black Hole) are AW2's map-unit palettes (`0x0810E6E0` rows 0..4)
//! colour for colour, so the idle pictures keep their indices
//! ([`MAP_ROLES`]) and AW2's palette per army (and its greyed "done" rows)
//! colours them. The moving pictures use the same palettes in Dual Strike,
//! but AW2's moving-unit palettes (`0x0810EA60`) order the colours
//! differently, so they are moved by role ([`MOVE_ROLES`]). The art is
//! shared by all armies in both games (only Infantry and Mech differ), so
//! one conversion serves all five.

use std::sync::OnceLock;

use crate::ds_art::lz10;

/// The Dual Strike units AW2 does not have.
pub const NEW_UNITS: [u8; 7] = [4, 9, 12, 13, 18, 25, 26];

/// Colour roles, the index in Dual Strike's unit palette:
/// 0 transparent, 1 white, 2..7 the army colour light to dark, 8 light
/// orange, 9 brown, 10 light blue (glass, wakes), 11..13 greys light to
/// dark (army-tinted), 14 unused, 15 outline black.
///
/// AW2's map-unit palette has the same layout.
pub const MAP_ROLES: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
/// AW2's moving-unit palette (`0x0810EA60`): 1 white, 2 army colour 3,
/// 3 light orange, 4..5 army 6..7, 6..8 army 2, 4, 5, 9 black, 10 brown,
/// 11..12 greys, 13 light blue, 14 unused, 15 a dark grey (the three
/// greys are AW2's own shades, a little off Dual Strike's).
pub const MOVE_ROLES: [u8; 16] = [0, 1, 6, 2, 7, 8, 4, 5, 3, 10, 13, 11, 12, 15, 14, 9];

/// Overlay 0's load address, and its bitmap-per-unit table.
const OV0: u32 = 0x022A_D560;
const SPRITE_TABLE: u32 = 0x022F_5D88;
const IDS: usize = 27;
const SPRITES: usize = 34;
const DS_SIZE: usize = 32;
const DS_SPRITE: usize = DS_SIZE * DS_SIZE / 2;
/// AW2's moving frame within Dual Strike's.
const MOVE_X: usize = 4;
const MOVE_Y: usize = 8;
const MOVE_SIZE: usize = 24;
const TILE: usize = 32;

/// Dual Strike's three idle sheets, decompressed, and the darkness rank of
/// each colour.
struct Idle {
    frames: [Vec<u8>; 3],
    dark: [u32; 16],
}

static IDLE: OnceLock<Idle> = OnceLock::new();

fn idle() -> Option<&'static Idle> {
    if let Some(i) = IDLE.get() {
        return Some(i);
    }
    let pack = crate::ds_pack::pack()?;
    let sheet = |name: &str| -> Option<Vec<u8>> {
        let d = lz10(pack.file(name)?)?;
        (d.len() == SPRITES * DS_SPRITE).then_some(d)
    };
    let frames = [sheet("bmap/01d")?, sheet("bmap/01e")?, sheet("bmap/01f")?];
    // Darkness from Orange Star's palette (BGR555), for breaking ties.
    let pal = pack.file("bmap/046")?.get(..32)?;
    let mut dark = [0u32; 16];
    for (i, d) in dark.iter_mut().enumerate() {
        let c = u16::from_le_bytes([pal[2 * i], pal[2 * i + 1]]) as u32;
        let luma = 2 * (c & 31) + 4 * ((c >> 5) & 31) + ((c >> 10) & 31);
        *d = 7 * 31 - luma;
    }
    let _ = IDLE.set(Idle { frames, dark });
    IDLE.get()
}

/// Dual Strike's idle bitmap for a unit id (Orange Star's row).
fn sprite_of(ds_id: u8) -> Option<usize> {
    if !(1..IDS as u8).contains(&ds_id) {
        return None;
    }
    let pack = crate::ds_pack::pack()?;
    let b = pack.overlay_at(0, OV0, SPRITE_TABLE + 2 * ds_id as u32, 2)?;
    let s = u16::from_le_bytes([b[0], b[1]]) as usize;
    (s < SPRITES).then_some(s)
}

fn pixel(bmp: &[u8], width: usize, x: usize, y: usize) -> u8 {
    let b = bmp[(y * width + x) / 2];
    if x & 1 == 1 {
        b >> 4
    } else {
        b & 15
    }
}

/// 4bpp tile bytes from 64 pixels in rows.
fn put_tile(out: &mut Vec<u8>, px: impl Fn(usize, usize) -> u8) {
    for y in 0..8 {
        for x in (0..8).step_by(2) {
            out.push(px(x, y) | px(x + 1, y) << 4);
        }
    }
}

/// AW2's 4 map tiles (16x16, 4bpp, AW2 tile order TL, TR, BL, BR, as
/// `sub_0802216C` draws a slot) for Dual Strike unit `ds_id`, idle frame
/// 0..2, in the index layout of AW2's country unit palettes (0x0810E6E0
/// rows), so the game's own palette per army colours it; facing the same
/// way as AW2's own map units in the sheet. `None` without the pack.
pub fn map_tiles(ds_id: u8, frame: usize) -> Option<[u8; 128]> {
    let idle = idle()?;
    let s = sprite_of(ds_id)?;
    let bmp = idle.frames.get(frame)?.get(s * DS_SPRITE..(s + 1) * DS_SPRITE)?;
    // One 16x16 pixel per 2x2 block: the most common colour; ties go to
    // transparent, then to the darker colour.
    let mut small = [[0u8; 16]; 16];
    for (y, row) in small.iter_mut().enumerate() {
        for (x, p) in row.iter_mut().enumerate() {
            let mut count = [0u8; 16];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                count[pixel(bmp, DS_SIZE, 2 * x + dx, 2 * y + dy) as usize] += 1;
            }
            let rank = |c: usize| (count[c], if c == 0 { u32::MAX } else { idle.dark[c] });
            let c = (0..16).max_by_key(|&c| rank(c)).unwrap();
            *p = MAP_ROLES[c];
        }
    }
    let mut out = Vec::with_capacity(128);
    for (tx, ty) in [(0, 0), (8, 0), (0, 8), (8, 8)] {
        put_tile(&mut out, |x, y| small[ty + y][tx + x]);
    }
    out.try_into().ok()
}

/// Dual Strike's moving-unit picture file for a unit id: the id-th file of
/// `mu/001..026` that is not an animation header (a whole number of 32x32
/// frames, 9 or 6 of them).
fn move_file(ds_id: u8) -> Option<&'static [u8]> {
    if !(1..IDS as u8).contains(&ds_id) {
        return None;
    }
    let pack = crate::ds_pack::pack()?;
    let f = (1..=0x26)
        .filter_map(|i| pack.file(&format!("mu/{i:03x}")))
        .filter(|f| !f.is_empty() && f.len() % DS_SPRITE == 0)
        .nth(ds_id as usize - 1)?;
    matches!(f.len() / DS_SPRITE, 6 | 9).then_some(f)
}

/// Frames per direction AW2's animation descriptor for a unit type plays:
/// two for copters and ships (types 19..24), three otherwise.
pub fn frames_per_direction(aw2_type: u8) -> Option<usize> {
    match aw2_type {
        1..=18 => Some(3),
        19..=24 => Some(2),
        _ => None,
    }
}

/// The moving-unit OBJ tiles for `ds_id`, uncompressed, laid out exactly
/// like AW2's moving sheet for the AW2 unit type `like_aw2_type` (so AW2's
/// animation descriptor for that class, record +0x14 of `0x0849CD88`,
/// drives it), in AW2's moving-unit palette index layout (0x0810EA60 +
/// (colour-1)*0x20). 9 frames x 9 tiles (0xA20 bytes) for foot, vehicle and
/// plane types, 6 x 9 (0x6C0) for copters and ships.
///
/// Dual Strike's frame counts are kept when they match; a unit with two
/// frames a direction (Black Boat, Carrier, Oozium) given a three-frame
/// class plays 0, 1, 0, and one with three given a two-frame class plays
/// 0, 1.
pub fn move_sheet(ds_id: u8, like_aw2_type: u8) -> Option<Vec<u8>> {
    let want = frames_per_direction(like_aw2_type)?;
    let src = move_file(ds_id)?;
    let have = src.len() / DS_SPRITE / 3;
    let pick: &[usize] = match (have, want) {
        (3, 3) => &[0, 1, 2],
        (2, 2) | (3, 2) => &[0, 1],
        (2, 3) => &[0, 1, 0],
        _ => return None,
    };
    let mut out = Vec::with_capacity(3 * want * 9 * TILE);
    for dir in 0..3 {
        for &f in pick {
            let frame = &src[(dir * have + f) * DS_SPRITE..][..DS_SPRITE];
            // Dual Strike's 16 tiles in rows; pixel (x, y) of the 32x32.
            let px = |x: usize, y: usize| -> u8 {
                let t = (y / 8) * 4 + x / 8;
                let b = frame[t * TILE + (y % 8) * 4 + (x % 8) / 2];
                MOVE_ROLES[(if x & 1 == 1 { b >> 4 } else { b & 15 }) as usize]
            };
            for k in 0..9 {
                let (cx, cy) = (2 - k % 3, 2 - k / 3);
                put_tile(&mut out, |x, y| px(MOVE_X + cx * 8 + x, MOVE_Y + cy * 8 + y));
            }
        }
    }
    Some(out)
}

/// Pixels of Dual Strike's moving frames that fall outside AW2's 24x24
/// (what [`move_sheet`] drops), for checking.
pub fn move_clipped(ds_id: u8) -> Option<usize> {
    let src = move_file(ds_id)?;
    let mut n = 0;
    for frame in src.chunks(DS_SPRITE) {
        for y in 0..DS_SIZE {
            for x in 0..DS_SIZE {
                let inside = (MOVE_X..MOVE_X + MOVE_SIZE).contains(&x) && (MOVE_Y..MOVE_Y + MOVE_SIZE).contains(&y);
                let t = (y / 8) * 4 + x / 8;
                let b = frame[t * TILE + (y % 8) * 4 + (x % 8) / 2];
                if !inside && (if x & 1 == 1 { b >> 4 } else { b & 15 }) != 0 {
                    n += 1;
                }
            }
        }
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_roles_are_a_permutation() {
        let mut seen = [false; 16];
        for r in MOVE_ROLES {
            seen[r as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    /// With `TANGOAW2_DS_ROM`: all seven units convert, at AW2's sizes.
    /// With `TANGOAW2_AW2_ROM` too: the role tables give AW2's own colours
    /// for Dual Strike's, army by army, and the Tank converts close to
    /// AW2's own. `cargo test -p tango-gamesupport-aw2 ds_unit_art -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_seven_units_convert() {
        let pack = crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        for id in NEW_UNITS {
            for f in 0..3 {
                assert!(map_tiles(id, f).is_some(), "map {id} {f}");
            }
            assert!(map_tiles(id, 3).is_none());
            let own = if matches!(id, 18 | 25) { 21 } else { 5 };
            for like in [own, 5, 16, 19, 21] {
                let s = move_sheet(id, like).unwrap_or_else(|| panic!("move {id} like {like}"));
                assert_eq!(
                    s.len(),
                    frames_per_direction(like).unwrap() * 3 * 9 * TILE,
                    "{id} {like}"
                );
            }
            eprintln!(
                "unit {id}: {} moving pixels outside AW2's 24x24",
                move_clipped(id).unwrap()
            );
        }
        assert!(map_tiles(0, 0).is_none() && map_tiles(27, 0).is_none() && move_sheet(4, 25).is_none());

        let Some(aw2) = std::env::var_os("TANGOAW2_AW2_ROM").map(|p| std::fs::read(p).unwrap()) else {
            return;
        };
        let at = |a: u32, n: usize| &aw2[(a - 0x0800_0000) as usize..][..n];
        let colour = |b: &[u8], i: usize| u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        for (c, name) in ["bmap/046", "bmap/047", "bmap/048", "bmap/049", "bmap/04a"]
            .iter()
            .enumerate()
        {
            let ds = pack.file(name).unwrap();
            let map = at(0x0810_E6E0 + 32 * c as u32, 32);
            let mv = at(0x0810_EA60 + 32 * c as u32, 32);
            // Colours 1..13 are the same (the moving greys, 11..13, are AW2's
            // own: Black Hole's are shades apart); 15 is black (0 or 1).
            for i in 1..14 {
                assert_eq!(colour(map, MAP_ROLES[i] as usize), colour(ds, i), "map {name} {i}");
                if i < 11 {
                    assert_eq!(colour(mv, MOVE_ROLES[i] as usize), colour(ds, i), "move {name} {i}");
                }
            }
            assert!(colour(map, 15) <= 1 && colour(mv, MOVE_ROLES[15] as usize) <= 1);
        }
        // The Tank (AW2 slot 2) against AW2's own, frame by frame.
        for f in 0..3 {
            let own = at(0x0810_BE60 + 0xD80 * f as u32 + 2 * 128, 128);
            let ds = map_tiles(5, f).unwrap();
            let same = (0..256)
                .filter(|&p| (own[p / 2] >> (4 * (p & 1))) & 15 == (ds[p / 2] >> (4 * (p & 1))) & 15)
                .count();
            eprintln!("Tank frame {f}: {same}/256 pixels as AW2's");
            assert!(same > 180);
        }
    }
}
