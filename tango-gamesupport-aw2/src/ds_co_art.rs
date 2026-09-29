//! Dual Strike's CO pictures, converted at run time from the Dual Strike
//! pack ([`crate::ds_pack`]) into AW2's CO presentation formats.
//! Converting only: the caller puts the data where AW2's presentation
//! table (`0x084A0090`, 0x44 bytes a CO) and HUD sheet point.
//!
//! The COs AW2 lacks, by Dual Strike id ([`NEW_COS`]): 11 Von Bolt,
//! 12 Jugger, 14 Koal, 20 Jake, 21 Rachel, 22 Sasha, 23 Javier, 24 Grimm,
//! 25 Kindle. Every Dual Strike CO id 1..27 converts, which is how the
//! conversion is checked against AW2's own art (Dual Strike's Andy, id 2,
//! against AW2's, id 1).
//!
//! **Where Dual Strike keeps them.** Arm9 `0x02152B8C + 0x54 * (id - 1)` is
//! an appearance record of 21 words per CO: file names (pointers to
//! 3-hex-digit names of `syogun/` files) for body, legs, two alternate
//! outfit parts, the mugs normal/happy/angry, alternates, battle faces and
//! more; +0x40 and +0x44 point at raw tiles in arm9 (the select-screen mug
//! and the menu mug); +0x50 at the palette. The CO record (arm9
//! `0x0215360C + 0x220 * id`) points at +0x58 to its name graphic. The
//! files are not in id order (Grimm and Javier swap), so they are always
//! found through the record.
//!
//! Dual Strike kept the GBA games' picture formats for all of these, so
//! most assets are copied as they are; only the full body is cut:
//!
//! - **Face** (AW2 +0x0C, three LZ77 pictures, `sub_08043E3C`): 48x48,
//!   6x6 tiles in rows. Dual Strike's mugs (`mug` normal/happy/angry) are
//!   the same size and order. AW2's third face is sad (the defeat face);
//!   Dual Strike's third is angry, used in the same places, so it stands
//!   in. Dual Strike draws its mugs closer in than AW2 (the head fills the
//!   square) and outlines them in white; they are kept as drawn.
//! - **Mini portrait** (+0x18, raw 12 tiles, `sub_08043FA8`): 32x24 as two
//!   16x24 columns, tiles 0..5 the left column and 6..11 the right, each
//!   2x3 in rows (found by drawing AW2's Andy, Sturm and Sonja). Dual
//!   Strike's select-screen mug has the same layout.
//! - **HUD face** (`0x08102F64 + co * 0x100`, 8 tiles): one 32x16 sprite
//!   (`0x084A003A`: wide, size 2), 4x2 tiles in rows, OBJ palette 14.
//!   Dual Strike's menu mug is the same.
//! - **Name** (+0x04, LZ77 384 bytes): six 8x16 sprites (`0x084A0730`),
//!   48x16, tile 2k the top of column k and 2k+1 its bottom. It is drawn
//!   with the fixed name palette `0x080F6164` (`sub_08043B44`), not the
//!   CO's. Dual Strike's name graphic (LZ77, 384 bytes) has the same layout
//!   and colour indices, so it is copied.
//! - **Full body** (+0x00, `{top, bottom}`, LZ77 4096 + 6144 bytes, OBJ
//!   tile b and b + 0x80): 128x160, drawn by `sub_08043C28` from
//!   `0x084A0756` as six sprites anchored at the bottom centre: 64x64 at
//!   (-64, 0) and (0, 0) (the top file), 64x64 at (-64, 64) and (0, 64),
//!   64x32 at (-64, 128) and (0, 128) (the bottom file); each sprite's
//!   tiles in rows. Dual Strike's body file is 128x192, the same 64x64
//!   blocks in the same order (left, right, then the next row), with the
//!   legs in another file. The head is at the same place and the same
//!   scale as AW2's (Andy's head is 5 px from the top in both), so the body
//!   is cut to its top 160 rows without scaling: the top two block rows as
//!   they are, and the top half (32 tile rows) of each block of the third.
//!   What falls below (the hips and legs) is dropped, as AW2 does.
//! - **Palette** (+0x08, 0x100 bytes): 8 colour schemes of 16 colours,
//!   one per colour edit (`GetLoadedCoPalette`); every asset of the CO but
//!   the name uses the chosen one (`sub_08043AC0`: palette + 0x20 *
//!   scheme). Dual Strike's palette is the same: 8 schemes of 16 colours,
//!   the same colour roles (Andy's first scheme is AW2's but for one
//!   shade), so it is copied. Scheme 0 is the CO's usual colours.

use crate::ds_art::lz10;

/// The Dual Strike COs AW2 does not have.
pub const NEW_COS: [u8; 9] = [11, 12, 14, 20, 21, 22, 23, 24, 25];

/// A CO's pictures in AW2's formats, uncompressed.
pub struct CoArt {
    /// Normal, happy, sad (Dual Strike's angry): 36 tiles each, 6x6 in rows.
    pub face: [Vec<u8>; 3],
    /// 12 tiles: two 16x24 columns, each 2x3 tiles in rows.
    pub mini: Vec<u8>,
    /// 8 tiles, 4x2 in rows, the `0x08102F64` sheet's layout.
    pub hud: Vec<u8>,
    /// The full body's top file (4096 bytes): two 64x64 sprites.
    pub body_top: Vec<u8>,
    /// Its bottom file (6144 bytes): two 64x64 and two 64x32 sprites.
    pub body_bottom: Vec<u8>,
    /// 12 tiles: six 8x16 columns, top tile then bottom.
    pub name: Vec<u8>,
    /// 0x100 bytes: 8 colour schemes of 16 BGR555 colours.
    pub palette: Vec<u8>,
}

const TILE: usize = 32;
pub const FACE_LEN: usize = 36 * TILE;
pub const MINI_LEN: usize = 12 * TILE;
pub const HUD_LEN: usize = 8 * TILE;
pub const NAME_LEN: usize = 12 * TILE;
pub const BODY_TOP_LEN: usize = 128 * TILE;
pub const BODY_BOTTOM_LEN: usize = 192 * TILE;
pub const PALETTE_LEN: usize = 0x100;

/// Dual Strike's appearance records and CO records (arm9).
const APPEARANCE: u32 = 0x0215_2B8C;
const APPEARANCE_LEN: u32 = 0x54;
const CO_RECORD: u32 = 0x0215_360C;
const CO_RECORD_LEN: u32 = 0x220;
/// Word indices in the appearance record.
const BODY: usize = 0;
const MUGS: [usize; 3] = [4, 5, 6];
const MINI: usize = 16;
const HUD: usize = 17;
const PALETTE: usize = 20;
/// Dual Strike's body file: 64x64 blocks of 64 tiles, two a row.
const BLOCK: usize = 64 * TILE;
const DS_BODY_MIN: usize = 6 * BLOCK;

/// The pictures of Dual Strike CO `ds_co` (1..27) in AW2's formats.
/// `None` without the pack, for another id, or if a file is not as
/// expected.
pub fn co_art(ds_co: u8) -> Option<CoArt> {
    if !(1..=27).contains(&ds_co) {
        return None;
    }
    let pack = crate::ds_pack::pack()?;
    let rec = pack.arm9_at(APPEARANCE + APPEARANCE_LEN * (ds_co as u32 - 1), APPEARANCE_LEN as usize)?;
    let word = |i: usize| u32::from_le_bytes(rec[4 * i..4 * i + 4].try_into().unwrap());
    // A `syogun/` file named by a word of the record, decompressed to `len`.
    let file = |i: usize, len: usize| -> Option<Vec<u8>> {
        let name = pack.arm9_at(word(i), 4)?;
        let name = std::str::from_utf8(&name[..3]).ok().filter(|_| name[3] == 0)?;
        let d = lz10(pack.file(&format!("syogun/{name}"))?)?;
        (d.len() >= len).then_some(d)
    };
    let raw = |i: usize, len: usize| pack.arm9_at(word(i), len).map(<[u8]>::to_vec);

    let face = [
        file(MUGS[0], FACE_LEN)?[..FACE_LEN].to_vec(),
        file(MUGS[1], FACE_LEN)?[..FACE_LEN].to_vec(),
        file(MUGS[2], FACE_LEN)?[..FACE_LEN].to_vec(),
    ];
    // The body's top 160 rows: block rows 0 and 1 whole, the top halves of
    // the two blocks of row 2.
    let body = file(BODY, DS_BODY_MIN)?;
    let half = BLOCK / 2;
    let body_top = body[..2 * BLOCK].to_vec();
    let mut body_bottom = body[2 * BLOCK..4 * BLOCK].to_vec();
    body_bottom.extend_from_slice(&body[4 * BLOCK..4 * BLOCK + half]);
    body_bottom.extend_from_slice(&body[5 * BLOCK..5 * BLOCK + half]);

    let co = pack.arm9_at(CO_RECORD + CO_RECORD_LEN * ds_co as u32 + 0x58, 4)?;
    let name_at = u32::from_le_bytes(co.try_into().unwrap());
    let name = lz10(pack.arm9.get(name_at.checked_sub(0x0200_0000)? as usize..)?)?;

    let art = CoArt {
        face,
        mini: raw(MINI, MINI_LEN)?,
        hud: raw(HUD, HUD_LEN)?,
        body_top,
        body_bottom,
        name: (name.len() == NAME_LEN).then_some(name)?,
        palette: raw(PALETTE, PALETTE_LEN)?,
    };
    Some(art)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With `TANGOAW2_DS_ROM`: the nine COs and Dual Strike's Andy convert,
    /// at AW2's sizes. With `TANGOAW2_AW2_ROM` too: Andy's first colour
    /// scheme is AW2's but for a shade, and the name graphics use the
    /// colour indices AW2's do.
    /// `cargo test -p tango-gamesupport-aw2 ds_co_art -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_nine_cos_convert() {
        crate::ds_pack::pack().expect("TANGOAW2_DS_ROM");
        for id in NEW_COS.into_iter().chain([2]) {
            let a = co_art(id).unwrap_or_else(|| panic!("CO {id}"));
            for f in &a.face {
                assert_eq!(f.len(), FACE_LEN, "{id}");
            }
            assert_eq!(a.mini.len(), MINI_LEN, "{id}");
            assert_eq!(a.hud.len(), HUD_LEN, "{id}");
            assert_eq!(a.body_top.len(), BODY_TOP_LEN, "{id}");
            assert_eq!(a.body_bottom.len(), BODY_BOTTOM_LEN, "{id}");
            assert_eq!(a.name.len(), NAME_LEN, "{id}");
            assert_eq!(a.palette.len(), PALETTE_LEN, "{id}");
            // Colour 0 is transparent: every picture has some, and the face
            // is not blank.
            assert!(a.face[0].iter().any(|&b| b != 0), "{id}");
        }
        assert!(co_art(0).is_none() && co_art(28).is_none());

        let Some(aw2) = std::env::var_os("TANGOAW2_AW2_ROM").map(|p| std::fs::read(p).unwrap()) else {
            return;
        };
        let at = |a: u32, n: usize| &aw2[(a - 0x0800_0000) as usize..][..n];
        let word = |a: u32| u32::from_le_bytes(at(a, 4).try_into().unwrap());
        // AW2's Andy (1): presentation row +0x08 is the palette.
        let row = 0x084A_0090 + 0x44;
        let own = at(word(row + 8), 0x20);
        let andy = co_art(2).unwrap();
        let same = (0..16).filter(|&i| own[2 * i..2 * i + 2] == andy.palette[2 * i..2 * i + 2]).count();
        eprintln!("Andy scheme 0: {same}/16 colours as AW2's");
        assert!(same >= 14);
        let own_name = lz10(&aw2[(word(row + 4) - 0x0800_0000) as usize..]).unwrap();
        let used = |b: &[u8]| {
            let mut u = [false; 16];
            for &x in b {
                u[(x & 15) as usize] = true;
                u[(x >> 4) as usize] = true;
            }
            u
        };
        let own_used = used(&own_name);
        for id in NEW_COS {
            let u = used(&co_art(id).unwrap().name);
            assert!((0..16).all(|i| !u[i] || own_used[i]), "name colours of {id}");
        }
    }
}
