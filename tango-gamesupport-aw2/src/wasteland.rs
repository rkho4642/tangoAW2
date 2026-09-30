//! Dual Strike's Wasteland biome for design maps, with the Dual Strike pack.
//!
//! In Dual Strike, Wasteland is one of a map's tilesets: the same terrain,
//! drawn in dry colours (tan plains, olive woods, rocky pink mountains, a
//! red sea). It changes no rules. Here it is a per-map setting of design
//! maps, drawn by giving the map AW2's own tiles Wasteland's colours.
//!
//! - Colours: AW2 draws its terrain with BG palettes 0-3 (and 4-7, the
//!   same darkened for fog), one set per weather (`0x0849BD20`, loaded by
//!   `sub_08035020`). Wasteland's are worked out from the pack the first
//!   time it is seen: every metatile is drawn both with AW2's tiles and with
//!   Dual Strike's Wasteland tiles (`bmap/001`, palette `bmap/009`, metatile
//!   table arm9 `0x02143F40`, laid out as AW2's), and each of AW2's colours
//!   takes the Wasteland colour it most often lands on. Roads and bridges
//!   keep AW2's greys, warmed (Wasteland's are too close to its plains to
//!   read). Fog and rain are AW2's own relation to its clear colours, fitted
//!   once. Snow stays AW2's.
//! - Storage: the last byte of the design-map record (+0x723, after the
//!   unit cells), as [`MAGIC`] | biome, saved with the map
//!   ([`crate::design5::save_record`]'s trap) and read when a design map is
//!   loaded for the editor or a battle. In play it is kept in the high bits
//!   of [`crate::sandstorm::STATE`] (saved with a suspended game).

use mgba::core::Core;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::ds_weather::is_on;
use crate::sandstorm::STATE;

const BIOME_SHIFT: u8 = 4;
const BIOME_BITS: u8 = 0x70;
pub const NORMAL: u8 = 0;
pub const WASTELAND: u8 = 1;

/// The record's biome byte: [`MAGIC`] | biome, anything else Normal (maps
/// saved before 0.3.0 hold whatever the game left there).
const RECORD_BIOME: u32 = 0x723;
const MAGIC: u8 = 0xB0;
const MAGIC_BITS: u8 = 0xF8;

/// The map being played (design maps are 0xB4..0xBF).
const MAP_ID: u32 = 0x0300_3FC2;

pub fn biome(core: &Core) -> u8 {
    (core.raw_read_8(STATE, -1) & BIOME_BITS) >> BIOME_SHIFT
}

pub fn set_biome(core: &mut Core, b: u8) {
    let s = core.raw_read_8(STATE, -1);
    let want = (s & !BIOME_BITS) | ((b << BIOME_SHIFT) & BIOME_BITS);
    if want != s {
        core.raw_write_8(STATE, -1, want);
    }
}

/// Whether the map is drawn as Wasteland now.
pub fn is_wasteland(core: &Core) -> bool {
    is_on(core) && biome(core) == WASTELAND && colours().is_some()
}

/// At every map start: tangoAW2's Wasteland maps are Wasteland
/// ([`crate::five_map::is_wasteland_map`]); any other map that is not a
/// design map (0xB4..0xB7, their biome is read from their record) is Normal.
pub fn map_start(core: &mut Core) {
    let id = core.raw_read_8(MAP_ID, -1);
    let want = if crate::five_map::is_wasteland_map(id) {
        WASTELAND
    } else if (0xB4..=0xB7).contains(&id) {
        return;
    } else {
        NORMAL
    };
    if biome(core) != want {
        set_biome(core, want);
    }
}

/// `sub_0803D2F8` (a design-map record, r1, laid out for the editor or a
/// battle): its biome.
pub const LOAD_RECORD: u32 = 0x0803_D2F8;
pub fn load_record(core: &mut Core) {
    let record = core.gba().cpu().gpr(1) as u32;
    let b = core.raw_read_8(record + RECORD_BIOME, -1);
    let biome = if b & MAGIC_BITS == MAGIC { b & 7 } else { NORMAL };
    if biome != NORMAL || self::biome(core) != NORMAL {
        set_biome(core, biome);
    }
}

/// A design map about to be saved (the record at `record`): its biome goes
/// with it, with the pack on (a map saved without it keeps the byte as the
/// game left it).
pub fn save_record(core: &mut Core, record: u32) {
    if !is_on(core) {
        return;
    }
    let b = biome(core);
    let at = record + RECORD_BIOME;
    let now = core.raw_read_8(at, -1);
    if b != NORMAL {
        core.raw_write_8(at, -1, MAGIC | b);
    } else if now & MAGIC_BITS == MAGIC {
        core.raw_write_8(at, -1, 0);
    }
}

// --- Colours -------------------------------------------------------------

/// AW2's terrain: tiles (LZ77), metatiles, and the clear, rain colour sets
/// (128 each: palettes 0-3, then 4-7 for fog).
const AW2_TILES: u32 = 0x080B_D1EC;
const AW2_METATILES: u32 = 0x080B_FBC4;
const CLEAR: u32 = 0x080B_F8C4;
const RAIN: u32 = 0x080B_F9C4;
const METATILES: usize = 1024;
const EMPTY_TILE: u16 = 0x100;
/// Dual Strike's Wasteland tiles, colours and metatiles.
const DS_TILES: &str = "bmap/001";
const DS_COLOURS: &str = "bmap/009";
const DS_METATILES: u32 = 0x0214_3F40;

/// AW2's road and bridge colours (palette 3, colours 4-12).
const ROAD_LINE: usize = 3;
const ROAD_COLOURS: std::ops::RangeInclusive<usize> = 4..=12;

/// AW2's own colour relations, fitted over its 60 terrain colours (x256,
/// per 5-bit channel): rows are the clear colour's r, g, b and 1, columns
/// the result's r, g, b.
const FOG: [[i32; 3]; 4] = [[136, 22, 14], [40, 156, 101], [19, 4, 117], [-305, -2, -335]];
const RAIN_FIT: [[i32; 3]; 4] = [[211, -12, -6], [26, 251, 85], [7, 18, 171], [-296, -367, 89]];
const RAIN_FOG: [[i32; 3]; 4] = [[106, 4, 12], [70, 157, 112], [7, 6, 107], [-219, 316, -442]];

fn apply(m: &[[i32; 3]; 4], c: u16) -> u16 {
    let v = [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32];
    let mut out = 0u16;
    for j in 0..3 {
        let x = v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + m[3][j];
        let x = ((x + 128).div_euclid(256)).clamp(0, 31) as u16;
        out |= x << (5 * j);
    }
    out
}

/// Warm a grey towards dry earth (a quarter of the way).
fn warm(c: u16) -> u16 {
    const EARTH: [u16; 3] = [28, 24, 18];
    let mut out = 0;
    for (j, e) in EARTH.iter().enumerate() {
        let v = (c >> (5 * j)) & 31;
        out |= ((v * 3 + e + 2) / 4) << (5 * j);
    }
    out
}

fn pixel(tiles: &[u8], v: u16, x: usize, y: usize) -> Option<u8> {
    let t = (v & 0x3FF) as usize;
    let (sx, sy) = (if v >> 10 & 1 != 0 { 7 - x } else { x }, if v >> 11 & 1 != 0 { 7 - y } else { y });
    let b = *tiles.get(32 * t + 4 * sy + sx / 2)?;
    Some((b >> (4 * (sx & 1))) & 15)
}

pub struct Colours {
    /// Clear and rain, 128 colours each (as AW2's sets).
    pub clear: Vec<u16>,
    pub rain: Vec<u16>,
}

static COLOURS: OnceLock<Option<Colours>> = OnceLock::new();

fn colours() -> Option<&'static Colours> {
    COLOURS.get()?.as_ref()
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

/// Wasteland's colour sets, worked out from AW2's ROM image and the pack.
pub fn derive(core: &Core) -> Option<Colours> {
    let pack = crate::ds_pack::pack()?;
    let mut packed = vec![0u8; 0x8000];
    core.raw_read_range(AW2_TILES, -1, &mut packed);
    let aw2_tiles = crate::ds_art::lz10(&packed)?;
    let mut mt = vec![0u8; 8 * METATILES];
    core.raw_read_range(AW2_METATILES, -1, &mut mt);
    let aw2_meta = u16s(&mt);
    let ds_tiles = crate::ds_art::lz10(pack.file(DS_TILES)?)?;
    let ds_colours = u16s(pack.file(DS_COLOURS)?);
    let ds_meta = u16s(pack.arm9_at(DS_METATILES, 8 * METATILES)?);
    let read_set = |at: u32| {
        let mut b = vec![0u8; 256];
        core.raw_read_range(at, -1, &mut b);
        u16s(&b)
    };
    let (aw2_clear, aw2_rain) = (read_set(CLEAR), read_set(RAIN));

    // (AW2 palette, colour) -> Wasteland colour -> pixels.
    let mut count: BTreeMap<(usize, u8), BTreeMap<u16, u32>> = BTreeMap::new();
    for m in 0..METATILES {
        let (a, d) = (&aw2_meta[4 * m..4 * m + 4], &ds_meta[4 * m..4 * m + 4]);
        if a.contains(&EMPTY_TILE) || d.contains(&EMPTY_TILE) {
            continue;
        }
        for k in 0..4 {
            for y in 0..8 {
                for x in 0..8 {
                    let (Some(ia), Some(id)) = (pixel(&aw2_tiles, a[k], x, y), pixel(&ds_tiles, d[k], x, y)) else {
                        continue;
                    };
                    if ia == 0 {
                        continue;
                    }
                    let Some(&c) = ds_colours.get(16 * (d[k] >> 12) as usize + id as usize) else {
                        continue;
                    };
                    *count.entry(((a[k] >> 12) as usize, ia)).or_default().entry(c).or_default() += 1;
                }
            }
        }
    }
    let mut clear = aw2_clear.clone();
    for ((line, i), seen) in &count {
        if *line >= 4 {
            continue;
        }
        // The most frequent; on a tie the lowest colour value.
        if let Some((&c, _)) = seen.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) {
            clear[16 * line + *i as usize] = c;
        }
    }
    for i in ROAD_COLOURS {
        clear[16 * ROAD_LINE + i] = warm(aw2_clear[16 * ROAD_LINE + i]);
    }
    let mut rain = aw2_rain.clone();
    for i in 0..64 {
        if i % 16 == 0 || clear[i] == aw2_clear[i] {
            continue;
        }
        clear[64 + i] = apply(&FOG, clear[i]);
        rain[i] = apply(&RAIN_FIT, clear[i]);
        rain[64 + i] = apply(&RAIN_FOG, clear[i]);
    }
    Some(Colours { clear, rain })
}

/// tangoAW2's Wasteland data in the ROM image's free space: the clear,
/// rain and sandstorm sets.
const DATA: u32 = 0x0867_1000;
pub const CLEAR_AT: u32 = DATA;
pub const RAIN_AT: u32 = DATA + 0x100;
pub const SAND_AT: u32 = DATA + 0x200;
/// The Design Room bar entry's name ([`WASTE_NAME`]).
pub const NAME_AT: u32 = DATA + 0x300;
const DATA_SENTINEL: u32 = DATA + 0xFFC;
const DATA_MAGIC: u32 = 0x3257_5344; // "DSW2"

/// Every frame with the pack on: the colour sets in the ROM image (worked
/// out once).
pub fn tick(core: &mut Core, on: bool) {
    if !on {
        return;
    }
    if COLOURS.get().is_none() {
        let c = derive(core);
        let _ = COLOURS.set(c);
    }
    let Some(c) = colours() else {
        return;
    };
    if core.raw_read_32(DATA_SENTINEL, -1) == DATA_MAGIC {
        return;
    }
    for (at, set) in [(CLEAR_AT, &c.clear), (RAIN_AT, &c.rain)] {
        for (i, v) in set.iter().enumerate() {
            core.raw_write_16(at + 2 * i as u32, -1, *v);
        }
    }
    for (i, v) in c.clear.iter().enumerate() {
        core.raw_write_16(SAND_AT + 2 * i as u32, -1, crate::sandstorm::sand_colour(*v));
    }
    core.raw_write_range(NAME_AT, -1, &WASTE_NAME);
    core.raw_write_32(DATA_SENTINEL, -1, DATA_MAGIC);
}

// --- The Design Room ---------------------------------------------------------

/// The game's working palettes (copied to palette RAM every frame).
const PAL_BUFFER: u32 = 0x0300_20C0;
const MAP_COLOURS: usize = 128;

/// A on the map with the bar's Wasteland entry
/// ([`crate::design_bar::WASTE_WORD`]): the map switches between Normal and
/// Wasteland.
pub fn toggle(core: &mut Core) {
    let b = if biome(core) == WASTELAND { NORMAL } else { WASTELAND };
    set_biome(core, b);
}

/// Every Design Room frame with the pack on: the map's colours (palettes
/// 0-7) are its biome's. Only a set the editor loaded itself, AW2's clear
/// or Wasteland's, is ever replaced, so nothing else drawn with those
/// palettes is touched.
pub fn editor_tick(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let Some(c) = colours() else {
        return;
    };
    let mut now = vec![0u8; 2 * MAP_COLOURS];
    core.raw_read_range(PAL_BUFFER, -1, &mut now);
    let mut aw2 = vec![0u8; 2 * MAP_COLOURS];
    core.raw_read_range(CLEAR, -1, &mut aw2);
    let waste: Vec<u8> = c.clear.iter().flat_map(|v| v.to_le_bytes()).collect();
    let want = if biome(core) == WASTELAND { &waste } else { &aw2 };
    if &now != want && (now == aw2 || now == waste) {
        core.raw_write_range(PAL_BUFFER, -1, want);
    }
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(LOAD_RECORD, Box::new(load_record))]
}

/// "Waste", the Wasteland entry's name in the Design Room's terrain bar
/// (32x16, 4x2 tiles, the bar's font colours: 1 fill, 15 outline).
pub const WASTE_NAME: [u8; 256] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00,
    0x1F, 0xF1, 0xFF, 0xFF, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00,
    0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0x00, 0x00, 0x1F, 0xF1, 0xF0, 0xFF,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x00, 0xF0, 0x00, 0x00, 0x00, 0xFF, 0x0F, 0xFF, 0xFF, 0x1F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0x00, 0x00, 0x00, 0xF1, 0x00, 0x00, 0x00, 0xF1, 0x0F, 0x00, 0x00, 0x11, 0x0F, 0xFF, 0x0F,
    0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0x1F, 0xF1, 0xFF, 0x11, 0x11, 0x11, 0xF0, 0x1F, 0xF1, 0x1F,
    0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x1F, 0xF1, 0xFF, 0x11, 0x1F, 0xF1, 0x1F, 0x1F, 0x11, 0xFF, 0x1F, 0x1F, 0xF1, 0x0F, 0x1F, 0xF1,
    0xFF, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0x1F, 0xF1, 0xFF, 0xFF, 0xF1, 0xFF, 0xF0, 0xFF, 0xFF, 0xF1, 0xF0, 0xFF, 0x11, 0xFF, 0xF0,
    0xF0, 0xFF, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xF1, 0xFF, 0x1F, 0xFF, 0xF1, 0xF0, 0x11, 0xF1, 0xF1, 0xFF, 0xF1, 0xFF, 0x1F, 0xFF, 0x1F, 0xF1,
    0xFF, 0x0F, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_keep_black_and_white_sane() {
        // White fogs to a light grey-blue, rain darkens it; black stays dark.
        let white = 0x7FFF;
        let f = apply(&FOG, white);
        assert!((f & 31) < 31 && (f & 31) > 15);
        assert!(apply(&RAIN_FIT, 0) & 31 <= 2);
        assert_eq!(warm(0), (5 << 10) | (6 << 5) | 7);
    }
}
