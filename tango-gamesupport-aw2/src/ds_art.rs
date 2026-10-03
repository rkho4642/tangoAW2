//! The Black Crystal's and Black Obelisk's pictures, imported once from the
//! player's own Advance Wars: Dual Strike ROM.
//!
//! tangoAW2 ships none of Dual Strike's art. When a Dual Strike ROM (.nds,
//! game code AWRE) is in the ROMs folder, the library scan offers it here
//! ([`offer`]) and the two map sprites are read from it: file `bmap/015` of
//! the DS file system, LZ77-compressed, holds 4bpp bitmaps (rows of pixels,
//! low nibble first) of the Crystal (16x32 at 0x1600) and the Obelisk
//! (64x64 at 0x3F00, its 3x3 footprint at x 8..56, y 0..48). They are drawn
//! with sub-palette 12 of `bmap/00e`, Dual Strike's version of Black Hole's
//! palette; each colour is mapped to the nearest of Advance Wars 2's
//! (`0x080D3E84`, which the sprites are shown with). The scan then saves
//! them next to the ROMs ([`CACHE_NAME`], [`cache`]) and offers that file
//! on later scans, so the .nds is needed only once.
//!
//! Without them the Crystal and Obelisk are drawn with Advance Wars 2's own
//! minicannon and Black Cannon sprites ([`crate::obelisk`]). The sprite
//! layout is the same either way, so netplay peers run the same code
//! whatever art each has.
//!
//! `TANGOAW2_DS_ROM=<file>` (a DS ROM or a saved import) points the scripts
//! and tests at the art.

use std::sync::OnceLock;

/// The pictures as GBA 4bpp tiles.
pub struct Art {
    /// 16x32, one sprite (2x4 tiles).
    pub crystal: Vec<u8>,
    /// The 48x48 footprint as four sprites, tiles in this order: 32x32 at
    /// (0, 0), 16x32 at (32, 0), 32x16 at (0, 32), 16x16 at (32, 32).
    pub obelisk: Vec<u8>,
    /// 16x32 for the terrain panel and the Design Room's bar.
    pub obelisk_small: Vec<u8>,
}

/// Tile ranges of the Obelisk's four sprites within [`Art::obelisk`].
pub const OBELISK_PIECES: [(usize, usize); 4] = [(0, 16), (16, 24), (24, 32), (32, 36)];

/// The file the scan saves next to the ROMs: the whole Dual Strike pack
/// ([`crate::ds_pack`]), which these pictures are read from. Imports saved
/// by 0.2 (the pictures alone, `Dual Strike Black Obelisk art.tangoaw2`)
/// still load.
pub const CACHE_NAME: &str = crate::ds_pack::CACHE_NAME;
const MAGIC: &[u8; 8] = b"TAW2DSOB";
const CRYSTAL_LEN: usize = 8 * 32;
const OBELISK_LEN: usize = 36 * 32;
const SMALL_LEN: usize = 8 * 32;

static DS: OnceLock<Art> = OnceLock::new();

/// The imported art, if any (a DS ROM or a saved import offered by the
/// scan, or named by `TANGOAW2_DS_ROM`).
pub fn art() -> Option<&'static Art> {
    if let Some(a) = DS.get() {
        return Some(a);
    }
    if let Some(path) = std::env::var_os("TANGOAW2_DS_ROM") {
        if let Ok(buf) = std::fs::read(&path) {
            offer(&buf);
        }
    }
    DS.get()
}

/// Bit 7 of the match subtype: set when both netplay peers have the Dual
/// Strike pack (the lobby sets it, and replays record it); the Crystal and
/// Obelisk go with it online.
pub const SHARED_ART: u8 = 0x80;

/// Whether the Crystal and Obelisk (their maps, and the Design Room's
/// entries) are shown: for a netplay match or its replay, as the match
/// says ([`SHARED_ART`]), so both peers and every replay agree; played
/// alone, when this player has imported the art. A console without the art
/// in a match that has it (a replay of someone else's) draws them as
/// nothing.
pub fn features(mode: Option<(u8, u8)>) -> bool {
    match mode {
        Some((_, subtype)) => subtype & SHARED_ART != 0,
        None => art().is_some(),
    }
}

/// What [`offer`] made of a file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Offered {
    /// Neither a Dual Strike ROM nor a saved import.
    No,
    /// A Dual Strike ROM; `imported` if the art was read from it (save it
    /// with [`cache`]).
    DsRom { imported: bool },
    /// A saved import, now in use.
    Import,
}

/// A file from the ROMs folder: a Dual Strike (USA) ROM, the pack saved
/// from one earlier, or the art 0.2 saved.
pub fn offer(buf: &[u8]) -> Offered {
    if crate::ds_pack::is_saved_pack(buf) {
        if !crate::ds_pack::offer_saved(buf) {
            return Offered::No;
        }
        if let Some(a) = crate::ds_pack::pack().and_then(|p| extract(&|path| p.file(path))) {
            let _ = DS.set(a);
        }
        return Offered::Import;
    }
    if let Some(a) = saved_by_0_2(buf) {
        let _ = DS.set(a);
        return Offered::Import;
    }
    if buf.len() < 0x200 || &buf[0x0C..0x10] != b"AWRE" {
        return Offered::No;
    }
    if !crate::ds_pack::offer_rom(buf) {
        return Offered::DsRom { imported: false };
    }
    let imported = match crate::ds_pack::pack().and_then(|p| extract(&|path| p.file(path))) {
        Some(a) => {
            let _ = DS.set(a);
            true
        }
        None => false,
    };
    Offered::DsRom { imported }
}

/// The pictures as 0.2 saved them (`Dual Strike Black Obelisk art.tangoaw2`).
fn saved_by_0_2(buf: &[u8]) -> Option<Art> {
    if buf.len() != 8 + CRYSTAL_LEN + OBELISK_LEN + SMALL_LEN || &buf[..8] != MAGIC {
        return None;
    }
    let (c, rest) = buf[8..].split_at(CRYSTAL_LEN);
    let (o, s) = rest.split_at(OBELISK_LEN);
    Some(Art {
        crystal: c.to_vec(),
        obelisk: o.to_vec(),
        obelisk_small: s.to_vec(),
    })
}

/// The imported pack as a file to keep next to the ROMs.
pub fn cache() -> Option<Vec<u8>> {
    crate::ds_pack::cache()
}

fn extract<'a>(file: &dyn Fn(&str) -> Option<&'a [u8]>) -> Option<Art> {
    let data = lz10(file("bmap/015")?)?;
    if data.len() != 20224 {
        return None;
    }
    let palette = file("bmap/00e")?.get(0x180..0x1A0)?;
    let map = colour_map(palette);
    let recolour = |b: &[u8]| -> Vec<u8> {
        b.iter()
            .map(|&x| map[(x & 15) as usize] | map[(x >> 4) as usize] << 4)
            .collect()
    };
    let crystal_bmp = recolour(data.get(0x1600..0x1700)?);
    let obelisk_bmp = recolour(data.get(0x3F00..0x4700)?);
    let (crystal_bmp, obelisk_bmp) = (&crystal_bmp[..], &obelisk_bmp[..]);
    let crystal = tiles(crystal_bmp, 16, &[(0, 0, 16, 32)]);
    // The footprint: x 8..56, y 0..48 of the 64x64 picture.
    let obelisk = tiles(
        obelisk_bmp,
        64,
        &[(8, 0, 32, 32), (40, 0, 16, 32), (8, 32, 32, 16), (40, 32, 16, 16)],
    );
    // The panel's 16x32: the footprint shrunk, every third column and the
    // rows spread over 32.
    let mut small = vec![0u8; 16 * 32 / 2];
    for y in 0..32 {
        for x in 0..16 {
            let (sx, sy) = (8 + x * 3 + 1, y * 48 / 32);
            let b = obelisk_bmp[sy * 32 + sx / 2];
            let v = if sx & 1 == 1 { b >> 4 } else { b & 15 };
            let o = &mut small[y * 8 + x / 2];
            *o |= if x & 1 == 1 { v << 4 } else { v };
        }
    }
    let obelisk_small = tiles(&small, 16, &[(0, 0, 16, 32)]);
    Some(Art {
        crystal,
        obelisk,
        obelisk_small,
    })
}

/// Black Hole's sprite palette in Advance Wars 2 (`0x080D3E84`), BGR555.
const AW2_PALETTE: [u16; 16] = [
    0x3DEF, 0x7FFF, 0x5F3B, 0x4E95, 0x4D11, 0x392C, 0x3BFF, 0x7F9C, 0x7EF7, 0x1013, 0x7F39, 0x4F30, 0x63DF, 0x5F5C,
    0x1C1C, 0x3612,
];

/// Each Dual Strike colour index -> the nearest Advance Wars 2 one (0, the
/// transparent colour, stays 0).
fn colour_map(ds: &[u8]) -> [u8; 16] {
    let rgb = |c: u16| [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32];
    let mut map = [0u8; 16];
    for (i, m) in map.iter_mut().enumerate().skip(1) {
        let c = rgb(u16::from_le_bytes([ds[2 * i], ds[2 * i + 1]]));
        *m = (1..16)
            .min_by_key(|&j| {
                let g = rgb(AW2_PALETTE[j]);
                (0..3).map(|k| (c[k] - g[k]).pow(2)).sum::<i32>()
            })
            .unwrap() as u8;
    }
    map
}

/// GBA 4bpp tiles (1D order within each sprite) of rectangles (x, y, w, h)
/// of a 4bpp bitmap `width` pixels wide.
pub(crate) fn tiles(bmp: &[u8], width: usize, rects: &[(usize, usize, usize, usize)]) -> Vec<u8> {
    let row = width / 2;
    let mut out = Vec::new();
    for &(x0, y0, w, h) in rects {
        for ty in 0..h / 8 {
            for tx in 0..w / 8 {
                for y in 0..8 {
                    let at = (y0 + ty * 8 + y) * row + (x0 + tx * 8) / 2;
                    out.extend_from_slice(&bmp[at..at + 4]);
                }
            }
        }
    }
    out
}

/// A file of the DS ROM's file system by path.
/// LZ77 (type 0x10) as the GBA/DS BIOS decompresses it.
pub fn lz10(b: &[u8]) -> Option<Vec<u8>> {
    if b.first() != Some(&0x10) || b.len() < 4 {
        return None;
    }
    let size = u32::from_le_bytes([b[1], b[2], b[3], 0]) as usize;
    let mut out = Vec::with_capacity(size);
    let mut p = 4;
    while out.len() < size {
        let flags = *b.get(p)?;
        p += 1;
        for bit in 0..8 {
            if out.len() >= size {
                break;
            }
            if flags & (0x80 >> bit) != 0 {
                let (b1, b2) = (*b.get(p)? as usize, *b.get(p + 1)? as usize);
                p += 2;
                let disp = ((b1 & 15) << 8 | b2) + 1;
                if disp > out.len() {
                    return None;
                }
                for _ in 0..(b1 >> 4) + 3 {
                    out.push(out[out.len() - disp]);
                }
            } else {
                out.push(*b.get(p)?);
                p += 1;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_import_saved_by_0_2_still_loads() {
        // (read without the process's kept art: other tests offer the real
        // pack, with TANGOAW2_DS_ROM, at the same time, and the art kept is
        // whichever came first)
        let mut buf = super::MAGIC.to_vec();
        buf.extend((0..(super::CRYSTAL_LEN + super::OBELISK_LEN + super::SMALL_LEN)).map(|i| i as u8));
        let art = super::saved_by_0_2(&buf).unwrap();
        assert_eq!(art.crystal, buf[8..8 + super::CRYSTAL_LEN]);
        assert_eq!(art.obelisk_small, buf[buf.len() - super::SMALL_LEN..]);
        assert!(super::saved_by_0_2(&buf[..buf.len() - 1]).is_none());
        // The pictures alone are not a Dual Strike pack: nothing to save.
        assert!(!crate::ds_pack::is_saved_pack(&buf));
        assert_eq!(super::offer(b"not a rom"), super::Offered::No);
    }
}
