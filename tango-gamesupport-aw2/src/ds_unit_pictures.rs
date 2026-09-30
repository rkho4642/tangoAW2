//! Dual Strike's own unit pictures for the seven new units, with the Dual
//! Strike pack: the big picture of the build menu's right-hand panel and of
//! the unit information screens (`sub_0803A07C`, `sub_0803A190`).
//!
//! AW2 draws that picture from two tables (grown to 64 units by
//! [`crate::roster`], which gives each new unit its template's rows):
//! - `0x0849DC18`, 15 words per unit: words 0..4 an OAM layout per
//!   country (`u16` count, then `attr0, attr1, attr2` per sprite, relative
//!   to the panel; tiles from 0x2E8, OBJ palette 3), words 5..9 the
//!   picture per country (LZ77, 4bpp tiles, 0x800 bytes at most, copied to
//!   OBJ tiles at 0x06015D00). The country is the CO's
//!   (`sub_08042DE0`).
//! - `0x08555D30`, 5 words per unit from unit 1: two 16-colour palettes
//!   per army colour (OBJ palettes 3 and 4).
//!
//! Dual Strike keeps the same design: its table at overlay 0 `0x0234F384`
//! has 15 words per Dual Strike unit, words 0..4 the OAM layout per army
//! (the same format and origin as AW2's), words 5..9 the name of the
//! picture in `xinfo/` (LZ77, the same tile format; the new units have one
//! picture for every army, the Megatank a second one without its crew for
//! Black Hole), words 10..14 the name of the palette in `battle/` (the
//! battle scene's, two halves). tangoAW2 copies them, per new unit and per
//! army (Orange Star, Blue Moon, Green Earth, Yellow Comet, Black Hole: the
//! five colours of AW2 and of tangoAW2's five-army mode), into free ROM
//! ([`DATA`]) and points the new units' rows at the copies. Nothing is
//! written without the pack; the tables themselves are switched every
//! frame by [`crate::roster::tick`].

use mgba::core::Core;

use crate::roster::{ds_id, NEW_UNITS};

const OVERLAY_BASE: u32 = 0x022A_D560;
const DS_TABLE: u32 = 0x0234_F384;
const DS_ROW: u32 = 60;
const ARMIES: usize = 5;
/// AW2 copies this much of a decompressed picture to OBJ tiles.
const PICTURE_MAX: usize = 0x800;
/// Palettes: two halves of 16 colours.
const PALETTE: usize = 0x40;
/// OAM layouts: a count and at most this many sprites (the ships' 5).
const MAX_SPRITES: usize = 8;

/// Free ROM: layouts, palettes, then pictures.
pub const DATA: u32 = 0x087D_0000;
const LAYOUTS: u32 = DATA;
const LAYOUT_SIZE: u32 = 2 + 6 * MAX_SPRITES as u32;
const PALETTES: u32 = DATA + 0x0800;
const PICTURES: u32 = DATA + 0x1800;
pub const DATA_END: u32 = DATA + 0x1_0000;

/// One new unit's picture data for one army.
pub struct Army {
    pub layout: Vec<u8>,
    pub picture: Vec<u8>,
    pub palette: [u8; PALETTE],
}

fn word(addr: u32) -> Option<u32> {
    let b = crate::ds_pack::pack()?.overlay_at(0, OVERLAY_BASE, addr, 4)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

fn name(addr: u32) -> Option<String> {
    let b = crate::ds_pack::pack()?.overlay_at(0, OVERLAY_BASE, addr, 4)?;
    let n: Vec<u8> = b.iter().copied().take_while(|&c| c != 0).collect();
    (n.len() == 3 && n.iter().all(u8::is_ascii_alphanumeric)).then(|| String::from_utf8_lossy(&n).into_owned())
}

fn file(path: &str) -> Option<Vec<u8>> {
    let f = crate::ds_pack::pack()?.file(path)?;
    Some(crate::ds_art::lz10(f).unwrap_or_else(|| f.to_vec()))
}

/// Dual Strike's picture of `ds_id` for `army` (0..4), from the pack.
pub fn army(ds_id: u8, army: usize) -> Option<Army> {
    let row = DS_TABLE + DS_ROW * ds_id as u32;
    let layout_at = word(row + 4 * army as u32)?;
    let count = u16::from_le_bytes(crate::ds_pack::pack()?.overlay_at(0, OVERLAY_BASE, layout_at, 2)?.try_into().ok()?);
    if count == 0 || count as usize > MAX_SPRITES {
        return None;
    }
    let layout = crate::ds_pack::pack()?.overlay_at(0, OVERLAY_BASE, layout_at, 2 + 6 * count as usize)?.to_vec();
    let picture = file(&format!("xinfo/{}", name(word(row + 4 * (5 + army as u32))?)?))?;
    if picture.is_empty() || picture.len() > PICTURE_MAX || picture.len() % 32 != 0 {
        return None;
    }
    let pal = crate::ds_pack::pack()?.file(&format!("battle/{}", name(word(row + 4 * (10 + army as u32))?)?))?;
    let palette: [u8; PALETTE] = pal.get(..PALETTE)?.try_into().ok()?;
    Some(Army { layout, picture, palette })
}

/// Writes every new unit's pictures, layouts and palettes to [`DATA`] and
/// points the new units' rows of the grown tables at them: `pictures`
/// (15 words per unit) and `palettes` (5 words per unit, from unit 1).
/// Leaves the rows alone (the template's) when the pack lacks them.
pub fn install(core: &mut Core, pictures: u32, palettes: u32) -> bool {
    let mut all = Vec::new();
    for &t in NEW_UNITS.iter() {
        let Some(d) = ds_id(t) else { return false };
        let mut armies = Vec::new();
        for a in 0..ARMIES {
            let Some(x) = army(d, a) else { return false };
            armies.push(x);
        }
        all.push((t, armies));
    }
    let mut next_picture = PICTURES;
    let mut stored: Vec<(Vec<u8>, u32)> = Vec::new();
    for (k, (t, armies)) in all.iter().enumerate() {
        for (a, x) in armies.iter().enumerate() {
            let slot = (k * ARMIES + a) as u32;
            let layout_at = LAYOUTS + LAYOUT_SIZE.next_multiple_of(4) * slot;
            core.raw_write_range(layout_at, -1, &x.layout);
            let palette_at = PALETTES + PALETTE as u32 * slot;
            core.raw_write_range(palette_at, -1, &x.palette);
            let picture_at = match stored.iter().find(|(p, _)| *p == x.picture) {
                Some(&(_, at)) => at,
                None => {
                    let lz = crate::lz77::compress(&x.picture);
                    let at = next_picture;
                    next_picture = (at + lz.len() as u32).next_multiple_of(4);
                    assert!(next_picture <= DATA_END);
                    core.raw_write_range(at, -1, &lz);
                    stored.push((x.picture.clone(), at));
                    at
                }
            };
            let row = pictures + DS_ROW * *t as u32;
            core.raw_write_32(row + 4 * a as u32, -1, layout_at);
            core.raw_write_32(row + 4 * (5 + a as u32), -1, picture_at);
            core.raw_write_32(palettes + 20 * (*t as u32 - 1) + 4 * a as u32, -1, palette_at);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every new unit has a picture per army that fits AW2's panel (needs
    /// `TANGOAW2_DS_ROM`).
    #[test]
    #[ignore]
    fn pictures_fit() {
        assert!(crate::ds_pack::pack().is_some(), "set TANGOAW2_DS_ROM");
        for &t in NEW_UNITS.iter() {
            for a in 0..ARMIES {
                let x = army(ds_id(t).unwrap(), a).unwrap_or_else(|| panic!("unit {t} army {a}"));
                assert_eq!(x.picture.len(), PICTURE_MAX, "unit {t}");
                let count = u16::from_le_bytes([x.layout[0], x.layout[1]]) as usize;
                // Every sprite's tiles lie inside the picture.
                for s in 0..count {
                    let at = |i: usize| u16::from_le_bytes([x.layout[2 + 6 * s + 2 * i], x.layout[3 + 6 * s + 2 * i]]);
                    assert!((at(2) & 0x3FF) < 64, "unit {t} sprite {s}");
                }
            }
        }
        // The Megatank's Black Hole picture has no crew: a picture of its own.
        assert_ne!(army(4, 0).unwrap().picture, army(4, 4).unwrap().picture);
    }
}
