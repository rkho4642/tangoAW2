//! A second front in the sky (Victory or Death!'s and Omens and Signs'),
//! drawn as Dual Strike draws it, with the Dual Strike pack.
//!
//! - Dual Strike plays the second front on the top screen, its 2D engine
//!   (engine B; read back in melonDS). On these two fronts its map layer
//!   (BG0) draws only the units: the map's sea, shoals and island are not
//!   drawn. Under them, BG1 is a field of clouds: the 256x256 picture
//!   `bmap/098` (65 tiles) drawn by the tilemap `bmap/099` (32x32
//!   entries, flips), in the colours `bmap/09a` (BG palette 2), blended
//!   over BG3, the far ground (`bmap/092`, `093`, `094`). The Black Arc is
//!   a 64x64 sprite over the fortress's 4x4 (`bmap/0a7`, OBJ palette 8 from
//!   `bmap/0aa`) where a sea map draws its fortress (`bmap/0a6`, the same
//!   bytes as AW2's own fortress picture); its minicannons are sprites in
//!   that palette too.
//! - Here: every cell of the front is drawn with the clouds (the picture's
//!   16x16 cell at the cell's position, repeating every 16 cells) by
//!   [`crate::wasteland`]'s painter, its tiles in static terrain tiles
//!   [`SLOT0`].. (no cell draws anything else there) and BG palette
//!   [`PALETTE`] (and its fogged copy) in the clouds' colours (colour 0,
//!   see-through in Dual Strike over its ground, takes the closest other
//!   colour), each colour blended over the ground's mean colour as Dual
//!   Strike blends the clouds over its ground (`BLDCNT` 0x3C42,
//!   `BLDALPHA` 0x1008: clouds 8/16, ground 16/16; one colour for the
//!   ground, AW2 having one map layer). The cells keep their terrain (sea: the air units' space, as
//!   before). The fortress's picture (AW2's `LoadInventionGraphics`, OBJ
//!   tile [`STRUCTURE_TILE`]) becomes the Black Arc's, and OBJ palette
//!   [`OBJ_PALETTE`] (the fortress's and its minicannons') the Black Arc's
//!   colours, kept while the front is on the screen and put back after. No
//!   ground under the clouds (AW2 has one map layer) and no blend; the
//!   minicannons keep AW2's picture in the Black Arc's colours.
//! - The front's weather is clear ([`crate::ds_campaign_data`]): the main
//!   front's sandstorm does not reach it.

use std::sync::OnceLock;

use mgba::core::Core;

const CLOUD_TILES: &str = "bmap/098";
const CLOUD_MAP: &str = "bmap/099";
const CLOUD_PALETTE: &str = "bmap/09a";
const ARC_TILES: &str = "bmap/0a7";
const ARC_PALETTE: &str = "bmap/0aa";
const FORTRESS_TILES: &str = "bmap/0a6";
const GROUND_TILES: &str = "bmap/092";
const GROUND_MAP: &str = "bmap/094";
const GROUND_PALETTE: &str = "bmap/093";
/// Engine B's blend of the clouds over the ground (`BLDCNT` 0x3C42: BG1
/// over BG2/BG3/OBJ/backdrop; `BLDALPHA` 0x1008): the clouds at 8/16, the
/// ground at 16/16.
const EVA: u32 = 8;
const EVB: u32 = 16;

/// The first terrain tile the clouds' tiles go in (BG character base 2:
/// static terrain tiles, before the sea's animated ones at 0x100).
pub const SLOT0: u16 = 1;
/// The BG palette the clouds are drawn in (and its fogged copy, + 4).
pub const PALETTE: u16 = 1;
/// AW2's OBJ tile of a map's 4x4 structure picture (`LoadInventionGraphics`
/// `0x0803FD80`: its base 0x48 + 0xE8) and the OBJ palette of the
/// structures and minicannons.
const STRUCTURE_TILE: u32 = 0x130;
const OBJ_PALETTE: u32 = 10;

const VRAM_TILES: u32 = 0x0600_8000;
const OBJ_VRAM: u32 = 0x0601_0000;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

pub struct Sky {
    /// The clouds' tiles (4bpp, 32 bytes each; colour 0 replaced).
    tiles: Vec<u8>,
    /// The clouds' tilemap: 32x32 entries (tile, flips).
    map: Vec<u16>,
    colours: [u16; 16],
    /// The Black Arc's 64x64 picture (64 tiles) and colours; the fortress's
    /// picture (to put back).
    arc: Vec<u8>,
    arc_colours: [u16; 16],
    fortress: Vec<u8>,
}

fn colours(b: &[u8]) -> Option<[u16; 16]> {
    let mut c = [0u16; 16];
    for (k, v) in c.iter_mut().enumerate() {
        *v = u16::from_le_bytes([*b.get(2 * k)?, *b.get(2 * k + 1)?]);
    }
    Some(c)
}

fn build() -> Option<Sky> {
    let pack = crate::ds_pack::pack()?;
    let file = |name: &str| -> Option<Vec<u8>> {
        let f = pack.file(name)?;
        if f.first() == Some(&0x10) {
            crate::ds_art::lz10(f)
        } else {
            Some(f.to_vec())
        }
    };
    let mut colours_ = colours(&file(CLOUD_PALETTE)?)?;
    // Colour 0: Dual Strike's ground shows through; here, the closest other.
    let near0 = (1..16).min_by_key(|&k| crate::ds_look::dist(colours_[0], colours_[k])).unwrap_or(1) as u8;
    let mut tiles = file(CLOUD_TILES)?;
    tiles.truncate(tiles.len() / 32 * 32);
    for b in tiles.iter_mut() {
        let (lo, hi) = (*b & 15, *b >> 4);
        *b = (if lo == 0 { near0 } else { lo }) | (if hi == 0 { near0 } else { hi }) << 4;
    }
    // The ground's mean colour (its picture's pixels, colour 0 aside), and
    // the clouds blended over it.
    let (gt, gm, gp) = (file(GROUND_TILES)?, file(GROUND_MAP)?, file(GROUND_PALETTE)?);
    let mut sum = [0u32; 3];
    let mut n = 0u32;
    for e in gm.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])) {
        let (t, row) = ((e & 0x3FF) as usize, (e >> 12) as usize);
        let Some(tile) = gt.get(32 * t..32 * t + 32) else { continue };
        for i in 0..64 {
            let ci = (tile[i / 2] >> (4 * (i & 1))) & 15;
            let at = 2 * (16 * row + ci as usize);
            if ci == 0 || at + 1 >= gp.len() {
                continue;
            }
            let c = u16::from_le_bytes([gp[at], gp[at + 1]]) as u32;
            for (k, v) in sum.iter_mut().enumerate() {
                *v += (c >> (5 * k)) & 31;
            }
            n += 1;
        }
    }
    if n == 0 {
        return None;
    }
    for c in colours_.iter_mut() {
        let mut out = 0u16;
        for (k, &g) in sum.iter().enumerate() {
            let v = ((*c as u32 >> (5 * k)) & 31) * EVA / 16 + (g * EVB + 8 * n) / (16 * n);
            out |= (v.min(31) as u16) << (5 * k);
        }
        *c = out;
    }
    let m = file(CLOUD_MAP)?;
    let map: Vec<u16> = m.chunks_exact(2).take(32 * 32).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    if map.len() != 32 * 32 || map.iter().any(|&e| (e & 0x3FF) as usize >= tiles.len() / 32) {
        return None;
    }
    let arc = file(ARC_TILES)?;
    let fortress = file(FORTRESS_TILES)?;
    if arc.len() < 0x800 || fortress.len() < 0x800 {
        return None;
    }
    Some(Sky {
        tiles,
        map,
        colours: colours_,
        arc: arc[..0x800].to_vec(),
        arc_colours: colours(&file(ARC_PALETTE)?)?,
        fortress: fortress[..0x800].to_vec(),
    })
}

pub fn sky() -> Option<&'static Sky> {
    static SKY: OnceLock<Option<Sky>> = OnceLock::new();
    SKY.get_or_init(build).as_ref()
}

/// A second front in the sky is on the screen (its rounds, or looked at).
pub fn on(core: &Core) -> bool {
    crate::two_front::second_live(core) && crate::two_front::battle(core).is_some_and(|b| b.fronts.sky)
}

/// Cell (x, y)'s four tilemap entries on a front in the sky (None
/// elsewhere): the clouds' 16x16 at its position.
pub fn entries(core: &Core, x: u32, y: u32) -> Option<[u16; 4]> {
    if !on(core) {
        return None;
    }
    let s = sky()?;
    Some([0usize, 1, 2, 3].map(|q| {
        let (cx, cy) = ((2 * x as usize + q % 2) % 32, (2 * y as usize + q / 2) % 32);
        let e = s.map[32 * cy + cx];
        (SLOT0 + (e & 0x3FF)) | (e & 0x0C00) | PALETTE << 12
    }))
}

/// 1 while OBJ palette [`OBJ_PALETTE`] holds the Black Arc's colours, then
/// the palette as it was (32 bytes) (crate::two_front's state, cleared at a
/// battle's start).
const BORROWED: u32 = crate::two_front::SKY_STATE;
const SAVED: u32 = BORROWED + 4;

fn write_if(core: &mut Core, at: u32, bytes: &[u8]) {
    let mut now = vec![0u8; bytes.len()];
    core.raw_read_range(at, -1, &mut now);
    if now != bytes {
        core.raw_write_range(at, -1, bytes);
    }
}

fn palette_bytes(c: &[u16; 16]) -> Vec<u8> {
    c.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Every frame of a two-front battle: on a front in the sky, its tiles and
/// colours in place; off it, the fortress's picture and the OBJ palette put
/// back.
pub fn tick(core: &mut Core) {
    let Some(s) = sky() else { return };
    let obj_pal = |base: u32| base + 0x200 + 32 * OBJ_PALETTE;
    let structure = OBJ_VRAM + 32 * STRUCTURE_TILE;
    if on(core) {
        write_if(core, VRAM_TILES + 32 * SLOT0 as u32, &s.tiles);
        let bg = palette_bytes(&s.colours);
        for base in [PAL_BUFFER, PAL_RAM] {
            for p in [PALETTE, PALETTE + 4] {
                write_if(core, base + 32 * p as u32, &bg);
            }
        }
        // The fortress's picture (as AW2 loaded it) becomes the Black Arc's.
        let mut now = vec![0u8; 0x800];
        core.raw_read_range(structure, -1, &mut now);
        if now == s.fortress {
            core.raw_write_range(structure, -1, &s.arc);
        }
        if core.raw_read_8(BORROWED, -1) != 1 {
            let mut p = [0u8; 32];
            core.raw_read_range(obj_pal(PAL_BUFFER), -1, &mut p);
            core.raw_write_range(SAVED, -1, &p);
            core.raw_write_8(BORROWED, -1, 1);
        }
        let arc = palette_bytes(&s.arc_colours);
        for base in [PAL_BUFFER, PAL_RAM] {
            write_if(core, obj_pal(base), &arc);
        }
    } else if core.raw_read_8(BORROWED, -1) == 1 && !crate::two_front::swapping(core) {
        let mut now = vec![0u8; 0x800];
        core.raw_read_range(structure, -1, &mut now);
        if now == s.arc {
            core.raw_write_range(structure, -1, &s.fortress);
        }
        let mut p = [0u8; 32];
        core.raw_read_range(SAVED, -1, &mut p);
        for base in [PAL_BUFFER, PAL_RAM] {
            write_if(core, obj_pal(base), &p);
        }
        core.raw_write_8(BORROWED, -1, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With `TANGOAW2_DS_ROM`: the clouds' tiles fit before the sea's
    /// animated tiles, and the Black Arc is not the fortress.
    #[test]
    fn data() {
        let Some(s) = sky() else { return };
        assert!(SLOT0 as usize + s.tiles.len() / 32 <= 0x100, "{} tiles", s.tiles.len() / 32);
        assert_ne!(s.arc, s.fortress);
        assert!(s.tiles.chunks(32).all(|t| t.iter().all(|&b| b & 15 != 0 && b >> 4 != 0)), "no see-through pixel");
    }
}
