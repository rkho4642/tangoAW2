//! Means to an End's Grand Bolt, as Dual Strike draws it, with the Dual
//! Strike pack.
//!
//! - Dual Strike draws its battle map in 3D, each cell a 16x16 texture. Its
//!   map stores the Grand Bolt as a picture (cell `(x, y)` holds `8 + 0x20 *
//!   y + x`), which only Means to an End (map 0xF8) reads through its own
//!   table (arm9 `0x02157F84`; every other map `0x02157BC4`): per terrain
//!   id a texture number and two flips (0x4000 left-right, 0x8000
//!   top-bottom). The Grand Bolt is one quarter drawn four times, textures
//!   0x78..0xA7, 16x16 at 4 bits a pixel, laid out one after another in
//!   `bmap/024` (`025`, `026` are the same with other edges, the textures
//!   Dual Strike loads for its other looks' borders), coloured by its
//!   terrain palette 2 (its cells' class, 0x1B, picks palette 2 from the
//!   table at arm9 `0x02157AE4`), from Means to an End's palette file
//!   `bmap/00b` (+0x40): the greys, the core's pinks and the sand, as Dual
//!   Strike's screen shows them (read back from its texture palettes in
//!   melonDS).
//! - Here: its cells are AW2's underlay (a structure's footprint: no unit
//!   enters), drawn by [`crate::wasteland`]'s painter with the Grand Bolt's
//!   own tiles (one per texture quadrant, mirrored with the tilemap's flips),
//!   put in the look's pool of free tiles as its composites are, then in
//!   static terrain tiles no other cell of the map draws with (one already
//!   there is used again), in BG palette 7 (the fogged copy of palette 3:
//!   Means to an End has no fog), set every frame of the mission
//!   ([`tick`]). The texture's ground around the dome (its sand colours
//!   7..11 and dots 12) takes the colours of the map's own plain as it is
//!   drawn now (weather included).
//! - Its three weak points (Dual Strike's parts 0xB..0xD at
//!   [`GRAND_BOLT_WEAK_POINTS`]) are AW2 inventions (a minicannon on tile
//!   [`PART_TILE`], crate::obelisk: no sprite, no fire, no heal, "G Bolt" in
//!   the terrain panel), drawn by the picture (its discs). A weak point
//!   whose Black Crystal stands is closed: it is no target, and anything
//!   else that hits it (a power) is undone ([`tick`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use mgba::core::Core;

pub use crate::ds_campaign_data::GRAND_BOLT_WEAK_POINTS;

/// The weak points' tile (unused by AW2; its class a minicannon's).
pub const PART_TILE: u16 = 0x194;
pub const PART_CLASS: u8 = 0x15;
const KIND_MINICANNON: u16 = 4;

const REMAP: u32 = 0x0215_7F84;
const REMAP_LEN: usize = 0x200;
const FIRST: u16 = 0x78;
const TEXTURES: usize = 48;
const TEXTURE_FILE: &str = "bmap/024";
const PALETTE_FILE: &str = "bmap/00b";
const PALETTE_AT: usize = 0x40;

/// BG palette 7: the fogged copy of terrain palette 3.
pub const PALETTE: u16 = 7;
const PAL_BUFFER: u32 = 0x0300_20C0;
const PAL_RAM: u32 = 0x0500_0000;

pub struct Bolt {
    /// Per texture, its 16x16 pixels (colour indices 1..15).
    textures: Vec<[u8; 256]>,
    pub colours: [u16; 16],
    /// Per map (Normal, Hard): cell -> (texture, flip left-right, flip
    /// top-bottom).
    cells: [BTreeMap<(u32, u32), (usize, bool, bool)>; 2],
}

fn build() -> Option<Bolt> {
    let pack = crate::ds_pack::pack()?;
    let ds = crate::ds_campaign_data::Ds::from_pack(pack)?;
    let tex = crate::ds_art::lz10(pack.file(TEXTURE_FILE)?)?;
    let pal = pack.file(PALETTE_FILE)?;
    let mut colours = [0u16; 16];
    for (k, c) in colours.iter_mut().enumerate() {
        *c = u16::from_le_bytes([*pal.get(PALETTE_AT + 2 * k)?, *pal.get(PALETTE_AT + 2 * k + 1)?]);
    }
    // Colour 0 is drawn by Dual Strike's textures; on AW2's map it would be
    // see-through: such pixels take the closest other colour.
    let near0 = (1..16).min_by_key(|&k| crate::ds_look::dist(colours[0], colours[k])).unwrap_or(1) as u8;
    let mut textures = Vec::with_capacity(TEXTURES);
    for t in 0..TEXTURES {
        let mut px = [0u8; 256];
        for (i, p) in px.iter_mut().enumerate() {
            let b = *tex.get(128 * t + i / 2)?;
            let v = (b >> (4 * (i & 1))) & 15;
            *p = if v == 0 { near0 } else { v };
        }
        textures.push(px);
    }
    let remap = pack.arm9_at(REMAP, 2 * REMAP_LEN)?;
    let record = crate::ds_campaign_data::record(&ds, crate::ds_campaign_data::MEANS_TO_AN_END)?;
    let mut cells: [BTreeMap<(u32, u32), (usize, bool, bool)>; 2] = Default::default();
    for (k, &at) in [record.maps.0, record.maps.1].iter().enumerate() {
        let at = if at == 0 { record.maps.0 } else { at };
        let (w, h, ids) = crate::ds_campaign_data::raw_map(&ds, at)?;
        for y in 0..h as u32 {
            for x in 0..w as u32 {
                let id = ids[(y * w as u32 + x) as usize] as usize;
                if id >= REMAP_LEN {
                    continue;
                }
                let r = u16::from_le_bytes([remap[2 * id], remap[2 * id + 1]]);
                let t = (r & 0x3FF).wrapping_sub(FIRST) as usize;
                if t < TEXTURES {
                    cells[k].insert((x, y), (t, r & 0x4000 != 0, r & 0x8000 != 0));
                }
            }
        }
    }
    Some(Bolt { textures, colours, cells })
}

pub fn bolt() -> Option<&'static Bolt> {
    static BOLT: OnceLock<Option<Bolt>> = OnceLock::new();
    BOLT.get_or_init(build).as_ref()
}

impl Bolt {
    /// Quadrant `q` (0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right)
    /// of a cell's picture: the texture's quadrant it shows, as a 4bpp tile
    /// (unflipped: the same tile serves the mirrored cells), and the
    /// tilemap entry's flip bits.
    pub fn tile(&self, (t, fh, fv): (usize, bool, bool), q: usize) -> ([u8; 32], u16) {
        let src = q ^ (fh as usize) ^ (2 * fv as usize);
        let px = &self.textures[t];
        let mut out = [0u8; 32];
        for y in 0..8 {
            for x in 0..8 {
                let (sx, sy) = (8 * (src % 2) + x, 8 * (src / 2) + y);
                out[4 * y + x / 2] |= px[16 * sy + sx] << (4 * (x & 1));
            }
        }
        (out, (fh as u16) << 10 | (fv as u16) << 11)
    }

    /// Cell `(x, y)` of Means to an End's map (`hard`: its Hard map) in the
    /// picture.
    pub fn cell(&self, hard: bool, x: u32, y: u32) -> Option<(usize, bool, bool)> {
        self.cells[hard as usize].get(&(x, y)).copied()
    }

    /// The picture's cells (for the map's conversion).
    pub fn covers(&self, hard: bool) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.cells[hard as usize].keys().copied()
    }
}

/// Means to an End's battle in a DS Campaign session.
pub fn on(core: &Core) -> bool {
    crate::ds_campaign::active(core)
        && crate::ds_campaign::mission(core) == crate::ds_campaign_data::MEANS_TO_AN_END as u8
        && crate::ds_campaign::in_battle(core)
}

/// The Grand Bolt's picture at map cell (x, y), in Means to an End's
/// battle.
pub fn cell_at(core: &Core, x: u32, y: u32) -> Option<(usize, bool, bool)> {
    if !on(core) {
        return None;
    }
    bolt()?.cell(crate::ds_campaign::hard(core), x, y)
}

// --- Weak points ----------------------------------------------------------------

const INVENTIONS: u32 = 0x0202_8360;

/// The invention-list entry of weak point `k` (a minicannon on [`PART_TILE`]).
pub fn part_entry(core: &Core, k: usize) -> Option<u32> {
    let (x, y) = GRAND_BOLT_WEAK_POINTS[k];
    (0..16)
        .map(|i| INVENTIONS + 8 * i)
        .take_while(|&a| (core.raw_read_16(a + 2, -1) >> 6) & 0xF != 0)
        .find(|&a| {
            (core.raw_read_16(a + 2, -1) >> 6) & 0xF == KIND_MINICANNON
                && core.raw_read_8(a, -1) as u32 == x
                && core.raw_read_8(a + 1, -1) as u32 == y
                && crate::obelisk::tile_at(core, x, y) == PART_TILE
        })
}

/// Weak point `k` still stands.
pub fn part_alive(core: &Core, k: usize) -> bool {
    part_entry(core, k).is_some_and(|e| core.raw_read_8(e + 4, -1) > 0)
}

/// Whether an invention-list entry is one of the Grand Bolt's weak points.
pub fn is_part(core: &Core, entry: u32) -> bool {
    let (x, y) = (core.raw_read_8(entry, -1) as u32, core.raw_read_8(entry + 1, -1) as u32);
    (core.raw_read_16(entry + 2, -1) >> 6) & 0xF == KIND_MINICANNON && crate::obelisk::tile_at(core, x, y) == PART_TILE
}

/// The texture's ground colours (its sand 7..11, its dots 12) as the map's
/// plain is drawn: the sand by brightness from the plain's most used
/// colours, the dots its reddest.
const SAND: [usize; 5] = [7, 8, 9, 10, 11];
const DOTS: usize = 12;
const VRAM_TILES: u32 = 0x0600_8000;
const PLAIN: u16 = 1;

fn with_ground(core: &Core, mut colours: [u16; 16]) -> [u16; 16] {
    let Some(l) = crate::wasteland::drawn(core).and_then(crate::wasteland::look) else { return colours };
    let mut count: BTreeMap<u16, u32> = BTreeMap::new();
    for e in l.look.entries(PLAIN, 0, 0) {
        let (t, p) = ((e & 0x3FF) as u32, (e >> 12) as u32);
        for i in 0..32 {
            let b = core.raw_read_8(VRAM_TILES + 32 * t + i, -1);
            for v in [b & 15, b >> 4] {
                if v != 0 {
                    *count.entry(core.raw_read_16(PAL_BUFFER + 32 * p + 2 * v as u32, -1)).or_default() += 1;
                }
            }
        }
    }
    let mut by_use: Vec<(u16, u32)> = count.into_iter().collect();
    by_use.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if by_use.is_empty() {
        return colours;
    }
    let lum = |c: u16| (c & 31) as u32 * 3 + ((c >> 5) & 31) as u32 * 6 + ((c >> 10) & 31) as u32;
    let mut main: Vec<u16> = by_use.iter().take(SAND.len()).map(|e| e.0).collect();
    main.sort_by_key(|&c| std::cmp::Reverse(lum(c)));
    let mut sand = SAND;
    sand.sort_by_key(|&k| std::cmp::Reverse(lum(colours[k])));
    for (k, &slot) in sand.iter().enumerate() {
        colours[slot] = main[k.min(main.len() - 1)];
    }
    let red = |c: u16| (c & 31) as i32 * 2 - ((c >> 5) & 31) as i32 - ((c >> 10) & 31) as i32;
    if let Some(&(c, _)) = by_use.iter().max_by_key(|e| (red(e.0), e.0)) {
        colours[DOTS] = c;
    }
    colours
}

/// Each weak point's hit points while its crystal stands (EWRAM the game
/// never writes, crate::ds_campaign_rules's mission state).
const HP: u32 = crate::ds_campaign_rules::MTE_TOLD + 1;

/// Every frame of Means to an End's battle: the picture's colours in BG
/// palette 7, and a closed weak point (its crystal standing) keeps its hit
/// points.
pub fn tick(core: &mut Core) {
    if !on(core) {
        return;
    }
    if let Some(b) = bolt() {
        let colours = with_ground(core, b.colours);
        for base in [PAL_BUFFER, PAL_RAM] {
            for (k, &c) in colours.iter().enumerate() {
                let a = base + 32 * PALETTE as u32 + 2 * k as u32;
                if core.raw_read_16(a, -1) != c {
                    core.raw_write_16(a, -1, c);
                }
            }
        }
    }
    for k in 0..3 {
        let Some(e) = part_entry(core, k) else { continue };
        let hp = core.raw_read_8(e + 4, -1);
        let kept = core.raw_read_8(HP + k as u32, -1);
        if crate::ds_campaign_rules::crystal_alive(core, k) {
            if kept == 0 || hp > kept {
                core.raw_write_8(HP + k as u32, -1, hp);
            } else if hp < kept {
                core.raw_write_8(e + 4, -1, kept);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_tile_is_free() {
        // Not tangoAW2's Crystal or Obelisk tile.
        assert!(PART_TILE != crate::obelisk::CRYSTAL_TILE && PART_TILE != crate::obelisk::OBELISK_TILE);
    }

    /// With `TANGOAW2_DS_ROM`: the picture covers the weak points, is
    /// mirrored (its left half the right half flipped), and every cell of
    /// it is in the map's first 12 rows.
    #[test]
    fn picture() {
        let Some(b) = bolt() else { return };
        for hard in [false, true] {
            let cells: Vec<_> = b.covers(hard).collect();
            assert!(cells.len() > 150, "{} cells", cells.len());
            for &(x, y) in &GRAND_BOLT_WEAK_POINTS {
                assert!(b.cell(hard, x, y).is_some(), "weak point ({x}, {y}) in the picture");
            }
            assert!(cells.iter().all(|&(_, y)| y < 12));
            let (t, fh, _) = b.cell(hard, 3, 5).unwrap();
            let (t2, fh2, _) = b.cell(hard, 15, 5).unwrap();
            assert_eq!((t, !fh), (t2, fh2), "mirrored");
        }
    }
}
