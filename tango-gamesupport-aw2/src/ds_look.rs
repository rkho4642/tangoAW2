//! Dual Strike's map looks (Wasteland, Desert, Snow) drawn with Dual
//! Strike's own terrain graphics, converted at run time from the pack.
//!
//! Dual Strike draws a map's terrain from one of two tilesets (`bmap/000`
//! for its Normal and Snow looks, `bmap/001` for Desert and Wasteland,
//! 736 tiles each), coloured by the look's palette file (`bmap/006`
//! Normal, `00a` Snow, `008` Desert, `009` Wasteland: 9 sub-palettes, 0-4
//! terrain, 5 its one grey fog palette, 6-8 buildings; the table at arm9
//! `0x02167DD4` names them per look). Its metatile table (arm9
//! `0x02143F40`) is AW2's own layout (AW2's tile ids, 4 quadrants each),
//! plus an upper table (`0x02145F40`) for what spills into the cell above
//! (mountain peaks, treetops, building roofs), drawn on a second layer
//! (`0x020F6E74`). Mountain cells are drawn with one of three mountains
//! (`0x020`, `0x146`, `0x147`) picked by position (`0x020F6F34`: table
//! `0x02169E58` at `(x + x/4 + 2y + y/8) & 15`), woods with two (`0x086`,
//! `0x087`). The tile layout is AW2's too: tiles `0x100..0x1FF` are the
//! sea's, `0x200..0x25F` the river's, and the Normal tileset's animation
//! frames are `bmap/004` (4 sea frames) and `bmap/005` (8 river frames),
//! laid out as AW2's own (`0x080C1FC4`, `0x080C9FC4`).
//!
//! The conversion ([`build`]) makes, per look, everything AW2 draws its
//! terrain from: 768 tiles, the sea's and river's frames, a metatile table,
//! the colours, the three mountains and the upper parts. For each of AW2's
//! metatiles:
//! - cells AW2 draws as plain under a building's sprite (every metatile
//!   with plain's quadrants, the Black Crystal's and Obelisk's, army 5's
//!   properties) get Dual Strike's plain;
//! - mountains get Dual Strike's mountain (the one of the three for the
//!   cell's position, [`Look::mountains`]), woods one of its two woods (by
//!   id parity);
//! - AW2's bridges Dual Strike lacks (`0x13`, `0x14`, `0x36`) its bridges,
//!   AW2's plains with a peak or treetop of the cell below drawn in (`0x03`,
//!   `0x43`, `0x106`, `0x107`, `0x126`, `0x127`) its plain (the peak comes
//!   from Dual Strike's upper part instead);
//! - everything else Dual Strike has, its own metatile (shoals, sea,
//!   reefs, rivers, roads, pipes, ...);
//! - what it lacks (some road corners and the class-0 sea strips
//!   `0x200..0x245`, `0x280`, `0x282`) keeps AW2's tiles with each colour
//!   the look's colour it most often lands on where both games have a
//!   metatile (not animated: such tiles live in the static part).
//!
//! Upper parts: a mountain's peak (its upper part's bottom 4 rows) and a
//! wood's treetops (2 rows) are drawn over the bottom half of the cell
//! above, as Dual Strike's second layer does. AW2 has one map layer, so the
//! cell above is drawn with composite tiles ([`Look::composite`]: its own
//! bottom tile with the upper part over it, in the palette of the 4 that
//! draws it best), made while the map is drawn into the tiles no metatile
//! uses ([`Look::pool`], [`crate::wasteland`]).
//!
//! Animation: the Snow look uses `bmap/004`/`005` as they are. Desert and
//! Wasteland (the other tileset) have no frames of their own in the ROM,
//! so theirs are derived: a pixel of the look's tile that equals the
//! Normal tileset's frame-0 pixel follows the Normal frames, the rest
//! (shores, banks in the look's own shapes) stays.
//!
//! Colours: AW2 has 4 terrain palettes (BG 0-3, 4-7 the same darkened for
//! fog); Dual Strike's terrain uses 5 (0-4). The 5 are grouped into 4 (the
//! grouping with the least mean colour error per tile, trying every
//! partition), each group cut to 15 colours by repeatedly folding together
//! the two closest colours (the less used goes; colours stay Dual Strike's
//! own, and a colour unlike the others, a wood's green, stays). Fog is
//! Dual Strike's: a fogged cell's terrain is drawn with its sub-palette 5,
//! index for index, so each colour of the 4 takes the fog colour of the
//! Dual Strike colours folded into it ([`Look::fog`]: AW2's palettes 4-7).
//! Weather changes no colour ([`crate::wasteland`]).

use std::collections::{BTreeMap, BTreeSet};

pub const TILES: usize = 768;
const TILE: usize = 32;
const EMPTY: u16 = 0x100;
pub const SEA_FRAME: usize = 0x2000;
pub const SEA_FRAMES: usize = 4;
pub const RIVER_FRAME: usize = 0xC00;
pub const RIVER_FRAMES: usize = 8;
const SEA0: usize = 0x100;
const SEA_N: usize = 256;
const RIVER0: usize = 0x200;
const RIVER_N: usize = 96;
pub const METATILES: usize = 1024;

const DS_LOWER: u32 = 0x0214_3F40;
const DS_UPPER: u32 = 0x0214_5F40;
const DS_MOUNTAIN_PICK: u32 = 0x0216_9E58;
const NORMAL_TILES: &str = "bmap/000";
const SEA_FRAMES_FILE: &str = "bmap/004";
const RIVER_FRAMES_FILE: &str = "bmap/005";

const PLAIN: usize = 0x01;
/// Dual Strike's three mountains and two woods: the upper parts
/// ([`Look::composites`]'s overlay index is the index here).
pub const TALL: [usize; 5] = [0x20, 0x146, 0x147, 0x86, 0x87];
const MOUNTAINS: usize = 3;
const WOOD: usize = 3;
/// Cells drawn as plain under a sprite whose quadrants are not plain's in
/// the ROM table: the Black Crystal and Obelisk, army 5's properties.
const PLAIN_CELLS: [usize; 8] = [0x192, 0x193, 0x1B4, 0x1B5, 0x1B6, 0x1B7, 0x1B8, 0x1B9];
const AS: [(usize, usize); 9] = [
    (0x13, 0x15),
    (0x14, 0x15),
    (0x36, 0x16),
    (0x03, PLAIN),
    (0x43, PLAIN),
    (0x106, PLAIN),
    (0x107, PLAIN),
    (0x126, PLAIN),
    (0x127, PLAIN),
];
const MOUNTAIN_CLASS: u8 = 3;
const WOOD_CLASS: u8 = 4;
const ROAD_CLASS: u8 = 5;
/// Roads drawn darker than Dual Strike's (its Wasteland and Desert roads
/// are faint tracks): each colour of a road's tiles this many 5-bit steps
/// darker, as pictures in the palette that draws them best. 0: Dual
/// Strike's own roads.
pub const ROAD_SHADE: u16 = 0;
const TRANSPARENT: u16 = 0x8000;
/// Dual Strike's fog: a fogged cell's terrain (sub-palettes 0-4) is drawn
/// with sub-palette 5, colour for colour (the same index); weather changes
/// no colour (checked in melonDS: Verdant Hills, Crystal Calamity in rain).
const FOG_PALETTE: usize = 5;
/// [`Look::fog`]: a colour no tile has.
pub const NO_FOG: u16 = 0x8000;

/// [`Look::tall`]: not tall.
pub const NOT_TALL: u8 = 0xFF;
/// [`Look::tall`]: a mountain, which of the three by position.
pub const MOUNTAIN_BY_POSITION: u8 = 0xFE;

/// A Dual Strike look's source files.
pub struct Source {
    pub tiles: &'static str,
    pub palette: &'static str,
}

/// AW2's own terrain, read from the ROM image.
pub struct Aw2 {
    pub tiles: Vec<u8>,
    pub metatiles: Vec<u16>,
    pub clear: Vec<u16>,
    pub classes: Vec<u8>,
}

/// A converted look.
pub struct Look {
    /// 768 tiles (the animated parts at their first frame).
    pub tiles: Vec<u8>,
    pub sea: Vec<u8>,
    pub river: Vec<u8>,
    pub metatiles: Vec<u16>,
    /// Palettes 0-3 (clear; 4-7 still AW2's, filled by the caller).
    pub colours: Vec<u16>,
    /// Palettes 4-7: each colour of palettes 0-3 as Dual Strike draws it
    /// in fog ([`FOG_PALETTE`]), [`NO_FOG`] where no tile has it.
    pub fog: Vec<u16>,
    /// What each AW2 metatile was drawn from (for tests and the doc).
    pub kinds: Vec<Kind>,
    /// Per AW2 metatile: the upper part it puts over the cell above (an
    /// index into [`TALL`]), [`MOUNTAIN_BY_POSITION`] or [`NOT_TALL`].
    pub tall: Vec<u8>,
    /// Dual Strike's three mountains' quadrants.
    pub mountains: [[u16; 4]; MOUNTAINS],
    /// Which of the three mountains by `(x + x/4 + 2y + y/8) & 15`.
    pub mountain_pick: [u8; 16],
    /// Each upper part's bottom-left and bottom-right quadrants as colours
    /// ([`TRANSPARENT`] where it draws nothing).
    pub overlays: Vec<[Vec<u16>; 2]>,
    /// The tiles no metatile uses (none animated): where the composites go
    /// while the map is drawn ([`crate::wasteland`]).
    pub pool: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Unused,
    Plain,
    Mountain,
    Wood,
    Ds,
    Aw2,
}

impl Look {
    /// The mountain (index into [`TALL`]) Dual Strike draws at (x, y).
    pub fn mountain_at(&self, x: u32, y: u32) -> u8 {
        self.mountain_pick[((x + x / 4 + 2 * y + y / 8) & 15) as usize]
    }

    /// The upper part (index into [`TALL`]) a cell with tile `t` at (x, y)
    /// puts over the cell above it, if any.
    pub fn upper_at(&self, t: u16, x: u32, y: u32) -> Option<u8> {
        match self.tall.get(t as usize).copied().unwrap_or(NOT_TALL) {
            NOT_TALL => None,
            MOUNTAIN_BY_POSITION => Some(self.mountain_at(x, y)),
            k => Some(k),
        }
    }

    /// The four tilemap entries of tile `t` at (x, y), before any upper
    /// part of the cell below goes over it ([`Look::composite`]).
    pub fn entries(&self, t: u16, x: u32, y: u32) -> [u16; 4] {
        let t = t as usize;
        if self.tall.get(t) == Some(&MOUNTAIN_BY_POSITION) {
            self.mountains[self.mountain_at(x, y) as usize]
        } else {
            match self.metatiles.get(4 * t..4 * t + 4) {
                Some(q) => [q[0], q[1], q[2], q[3]],
                None => [0; 4],
            }
        }
    }

    /// A bottom quadrant (entry `e`, side 0 left / 1 right) with upper part
    /// `o` over it: the tile and its palette (the one of the 4 that draws it
    /// best), or None when the upper part draws nothing on that side.
    pub fn composite(&self, e: u16, o: u8, side: u8) -> Option<([u8; TILE], u16)> {
        let over = self.overlays.get(o as usize)?.get(side as usize)?;
        if over.iter().all(|&c| c == TRANSPARENT) {
            return None;
        }
        let b = (e >> 12) as usize & 3;
        let px: Vec<u16> = (0..64)
            .map(|k| match over[k] {
                TRANSPARENT => match quad_px(&self.tiles, e, k % 8, k / 8) {
                    0 => TRANSPARENT,
                    i => self.colours[16 * b + i as usize],
                },
                c => c,
            })
            .collect();
        let blocks: Vec<&[u16]> = (0..4).map(|i| &self.colours[16 * i + 1..16 * i + 16]).collect();
        let mut h: BTreeMap<u16, u64> = BTreeMap::new();
        for &c in px.iter().filter(|&&c| c != TRANSPARENT) {
            *h.entry(c).or_default() += 1;
        }
        let best = (0..4).min_by_key(|&i| (hist_error(&h, blocks[i]), i))?;
        let mut t = [0u8; TILE];
        for (k, &c) in px.iter().enumerate() {
            let i = if c == TRANSPARENT { 0 } else { nearest(blocks[best], c) as u8 + 1 };
            set_px(&mut t, k % 8, k / 8, i);
        }
        Some((t, best as u16))
    }
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn tpx(tiles: &[u8], t: usize, x: usize, y: usize) -> u8 {
    tiles.get(TILE * t + 4 * y + x / 2).map_or(0, |b| (b >> (4 * (x & 1))) & 15)
}

fn quad_px(tiles: &[u8], v: u16, x: usize, y: usize) -> u8 {
    let sx = if v >> 10 & 1 != 0 { 7 - x } else { x };
    let sy = if v >> 11 & 1 != 0 { 7 - y } else { y };
    tpx(tiles, (v & 0x3FF) as usize, sx, sy)
}

fn set_px(tile: &mut [u8], x: usize, y: usize, v: u8) {
    tile[4 * y + x / 2] |= (v & 15) << (4 * (x & 1));
}

pub fn dist(a: u16, b: u16) -> u32 {
    (0..3)
        .map(|j| {
            let d = ((a >> (5 * j)) & 31) as i32 - ((b >> (5 * j)) & 31) as i32;
            (d * d) as u32
        })
        .sum()
}

/// Frames for a tileset that is not the Normal one: each pixel equal to
/// the Normal tileset's (frame 0) follows the Normal frames.
fn derive_frames(normal: &[u8], look: &[u8], frames: &[u8], start: usize, n: usize, size: usize) -> Vec<u8> {
    let mut out = vec![0u8; frames.len()];
    for f in 0..frames.len() / size {
        for k in 0..n {
            let t = start + k;
            for y in 0..8 {
                for x in 0..8 {
                    let b0 = tpx(normal, t, x, y);
                    let bf = tpx(&frames[f * size..], k, x, y);
                    let l = tpx(look, t, x, y);
                    set_px(&mut out[f * size + TILE * k..], x, y, if l == b0 { bf } else { l });
                }
            }
        }
    }
    out
}

/// A tile's pixels as colours.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Unit {
    /// A Dual Strike tile with its sub-palette.
    Ds(u16, u8),
    /// Colours ([`TRANSPARENT`] for none), unflipped.
    Px(Vec<u16>),
}

struct Quad {
    unit: Unit,
    flips: u16,
}

/// Every partition of `items` into at most `k` blocks.
fn partitions(items: &[u8], k: usize) -> Vec<Vec<Vec<u8>>> {
    if items.is_empty() {
        return vec![vec![]];
    }
    let (first, rest) = (items[0], &items[1..]);
    let mut out = Vec::new();
    for p in partitions(rest, k) {
        for i in 0..p.len() {
            let mut q = p.clone();
            q[i].insert(0, first);
            out.push(q);
        }
        if p.len() < k {
            let mut q = vec![vec![first]];
            q.extend(p.iter().cloned());
            out.push(q);
        }
    }
    out
}

/// Cut colours (with their pixel counts) to 15: the closest pair is folded
/// together, the less used going into the other.
fn reduce15(w: &BTreeMap<u16, u64>) -> Vec<u16> {
    let mut w = w.clone();
    while w.len() > 15 {
        let cols: Vec<(u16, u64)> = w.iter().map(|(&c, &n)| (c, n)).collect();
        let mut best: Option<((u32, u64), u16, u16)> = None;
        for i in 0..cols.len() {
            for j in i + 1..cols.len() {
                let (a, b) = (cols[i], cols[j]);
                let (lo, hi) = if (a.1, a.0) < (b.1, b.0) { (a, b) } else { (b, a) };
                let cost = (dist(a.0, b.0), lo.1);
                if best.map_or(true, |(c, _, _)| cost < c) {
                    best = Some((cost, lo.0, hi.0));
                }
            }
        }
        let (_, lo, hi) = best.unwrap();
        let n = w.remove(&lo).unwrap();
        *w.get_mut(&hi).unwrap() += n;
    }
    w.keys().copied().collect()
}

fn nearest(cols: &[u16], c: u16) -> usize {
    (0..cols.len()).min_by_key(|&i| (dist(cols[i], c), i)).unwrap_or(0)
}

/// A tile's mean colour error (x256) drawn with `block`.
fn hist_error(h: &BTreeMap<u16, u64>, block: &[u16]) -> u64 {
    let n: u64 = h.values().sum();
    let e: u64 = h
        .iter()
        .map(|(&c, &k)| k * block.iter().map(|&b| dist(b, c) as u64).min().unwrap_or(u32::MAX as u64))
        .sum();
    256 * e / n.max(1)
}

pub struct Pack<'a> {
    pub tiles: &'a [u8],
    pub normal_tiles: &'a [u8],
    pub palette: &'a [u16],
    pub lower: &'a [u16],
    pub upper: &'a [u16],
    pub mountain_pick: &'a [u16],
    pub sea: &'a [u8],
    pub river: &'a [u8],
}

pub struct Files {
    pub tiles: Vec<u8>,
    pub normal_tiles: Vec<u8>,
    pub palette: Vec<u16>,
    pub lower: Vec<u16>,
    pub upper: Vec<u16>,
    pub mountain_pick: Vec<u16>,
    pub sea: Vec<u8>,
    pub river: Vec<u8>,
}

impl Files {
    pub fn pack(&self) -> Pack<'_> {
        Pack {
            tiles: &self.tiles,
            normal_tiles: &self.normal_tiles,
            palette: &self.palette,
            lower: &self.lower,
            upper: &self.upper,
            mountain_pick: &self.mountain_pick,
            sea: &self.sea,
            river: &self.river,
        }
    }
}

/// Reads a look's source from the Dual Strike pack.
pub fn from_pack(src: &Source) -> Option<Files> {
    let pack = crate::ds_pack::pack()?;
    let f = Files {
        tiles: crate::ds_art::lz10(pack.file(src.tiles)?)?,
        normal_tiles: crate::ds_art::lz10(pack.file(NORMAL_TILES)?)?,
        palette: u16s(pack.file(src.palette)?),
        lower: u16s(pack.arm9_at(DS_LOWER, 8 * METATILES)?),
        upper: u16s(pack.arm9_at(DS_UPPER, 8 * METATILES)?),
        mountain_pick: u16s(pack.arm9_at(DS_MOUNTAIN_PICK, 32)?),
        sea: pack.file(SEA_FRAMES_FILE)?.to_vec(),
        river: pack.file(RIVER_FRAMES_FILE)?.to_vec(),
    };
    if f.palette.len() < 144 || f.sea.len() < SEA_FRAMES * SEA_FRAME || f.river.len() < RIVER_FRAMES * RIVER_FRAME {
        return None;
    }
    Some(f)
}

/// The conversion (see the module doc).
pub fn build(aw2: &Aw2, ds: &Pack) -> Option<Look> {
    let dt = ds.tiles;
    let dp = ds.palette;
    let derived = dt != ds.normal_tiles;
    let (sea, river) = if derived {
        (
            derive_frames(ds.normal_tiles, dt, &ds.sea[..SEA_FRAMES * SEA_FRAME], SEA0, SEA_N, SEA_FRAME),
            derive_frames(ds.normal_tiles, dt, &ds.river[..RIVER_FRAMES * RIVER_FRAME], RIVER0, RIVER_N, RIVER_FRAME),
        )
    } else {
        (ds.sea[..SEA_FRAMES * SEA_FRAME].to_vec(), ds.river[..RIVER_FRAMES * RIVER_FRAME].to_vec())
    };
    let colour = |v: u16, i: u8| dp.get(16 * (v >> 12) as usize + i as usize).copied().unwrap_or(0);
    let quad = |t: &[u16], m: usize| [t[4 * m], t[4 * m + 1], t[4 * m + 2], t[4 * m + 3]];
    let ds_quads = |m: usize| -> Vec<Quad> {
        quad(ds.lower, m)
            .iter()
            .map(|&v| Quad { unit: Unit::Ds(v & 0x3FF, (v >> 12) as u8), flips: v & 0xC00 })
            .collect()
    };

    // AW2 colour -> the look's colour it most often lands on.
    let mut count: BTreeMap<(u16, u8), BTreeMap<u16, u64>> = BTreeMap::new();
    for m in 0..METATILES {
        let (a, d) = (quad(&aw2.metatiles, m), quad(ds.lower, m));
        if a.contains(&EMPTY) || d.contains(&EMPTY) {
            continue;
        }
        for k in 0..4 {
            for y in 0..8 {
                for x in 0..8 {
                    let ia = quad_px(&aw2.tiles, a[k], x, y);
                    if ia == 0 {
                        continue;
                    }
                    let c = colour(d[k], quad_px(dt, d[k], x, y));
                    *count.entry((a[k] >> 12, ia)).or_default().entry(c).or_default() += 1;
                }
            }
        }
    }
    let amap: BTreeMap<(u16, u8), u16> = count
        .iter()
        .filter_map(|(k, seen)| seen.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(&c, _)| (*k, c)))
        .collect();
    let aw2_quads = |m: usize| -> Vec<Quad> {
        quad(&aw2.metatiles, m)
            .iter()
            .map(|&v| {
                let (t, p) = ((v & 0x3FF) as usize, v >> 12);
                let mut px = Vec::with_capacity(64);
                for y in 0..8 {
                    for x in 0..8 {
                        let i = tpx(&aw2.tiles, t, x, y);
                        px.push(if i == 0 {
                            TRANSPARENT
                        } else {
                            amap.get(&(p, i))
                                .copied()
                                .unwrap_or_else(|| aw2.clear.get(16 * p as usize + i as usize).copied().unwrap_or(0))
                        });
                    }
                }
                Quad { unit: Unit::Px(px), flips: v & 0xC00 }
            })
            .collect()
    };

    // Each metatile's quadrants, then Dual Strike's three mountains.
    let plain_quad = quad(&aw2.metatiles, PLAIN);
    let class = |m: usize| aw2.classes.get(m).copied().unwrap_or(0);
    let mut kinds = vec![Kind::Unused; METATILES];
    let mut tall = vec![NOT_TALL; METATILES];
    // Shaded roads' colours, with the sub-palette they come from.
    let mut shaded: BTreeMap<u8, BTreeMap<u16, u64>> = BTreeMap::new();
    let mut out: Vec<Option<Vec<Quad>>> = Vec::with_capacity(METATILES + MOUNTAINS);
    for m in 0..METATILES {
        let a = quad(&aw2.metatiles, m);
        let (k, q) = if a.contains(&EMPTY) {
            (Kind::Unused, None)
        } else if a == plain_quad || PLAIN_CELLS.contains(&m) {
            (Kind::Plain, Some(ds_quads(PLAIN)))
        } else if class(m) == MOUNTAIN_CLASS {
            tall[m] = MOUNTAIN_BY_POSITION;
            (Kind::Mountain, Some(ds_quads(TALL[0])))
        } else if class(m) == WOOD_CLASS {
            tall[m] = (WOOD + (m & 1)) as u8;
            (Kind::Wood, Some(ds_quads(TALL[WOOD + (m & 1)])))
        } else if let Some(&(_, d)) = AS.iter().find(|&&(x, _)| x == m) {
            (Kind::Ds, Some(ds_quads(d)))
        } else if !quad(ds.lower, m).contains(&EMPTY) && class(m) == ROAD_CLASS && ROAD_SHADE > 0 {
            let shade = |c: u16| (0..3).map(|j| ((c >> (5 * j)) & 31).saturating_sub(ROAD_SHADE) << (5 * j)).sum::<u16>();
            let q = quad(ds.lower, m)
                .iter()
                .map(|&v| {
                    let px: Vec<u16> = (0..64)
                        .map(|k| match tpx(dt, (v & 0x3FF) as usize, k % 8, k / 8) {
                            0 => TRANSPARENT,
                            i => shade(colour(v, i)),
                        })
                        .collect();
                    for &c in px.iter().filter(|&&c| c != TRANSPARENT) {
                        *shaded.entry((v >> 12) as u8).or_default().entry(c).or_default() += 1;
                    }
                    Quad { unit: Unit::Px(px), flips: v & 0xC00 }
                })
                .collect();
            (Kind::Ds, Some(q))
        } else if !quad(ds.lower, m).contains(&EMPTY) {
            (Kind::Ds, Some(ds_quads(m)))
        } else {
            (Kind::Aw2, Some(aw2_quads(m)))
        };
        kinds[m] = k;
        out.push(q);
    }
    for &d in &TALL[..MOUNTAINS] {
        out.push(Some(ds_quads(d)));
    }

    // Colour counts per unit, and per Dual Strike sub-palette.
    let units: BTreeSet<Unit> = out.iter().flatten().flatten().map(|q| q.unit.clone()).collect();
    let mut hist: BTreeMap<Unit, BTreeMap<u16, u64>> = BTreeMap::new();
    let mut by_pal: BTreeMap<u8, BTreeMap<u16, u64>> = BTreeMap::new();
    let ds_hist = |t: usize, p: u8, frames_too: bool| {
        let mut h: BTreeMap<u16, u64> = BTreeMap::new();
        let mut add = |tile: &[u8], k: usize| {
            for y in 0..8 {
                for x in 0..8 {
                    let i = tpx(tile, k, x, y);
                    if i != 0 {
                        *h.entry(colour((p as u16) << 12, i)).or_default() += 1;
                    }
                }
            }
        };
        add(dt, t);
        if frames_too {
            for (frames, start, n, size) in [(&sea, SEA0, SEA_N, SEA_FRAME), (&river, RIVER0, RIVER_N, RIVER_FRAME)] {
                if (start..start + n).contains(&t) {
                    for f in 1..frames.len() / size {
                        add(&frames[f * size..], t - start);
                    }
                }
            }
        }
        h
    };
    for u in &units {
        let h = match u {
            Unit::Ds(t, p) => {
                let h = ds_hist(*t as usize, *p, true);
                let w = by_pal.entry(*p).or_default();
                for (&c, &n) in &h {
                    *w.entry(c).or_default() += n;
                }
                h
            }
            Unit::Px(px) => {
                let mut h: BTreeMap<u16, u64> = BTreeMap::new();
                for &c in px.iter().filter(|&&c| c != TRANSPARENT) {
                    *h.entry(c).or_default() += 1;
                }
                h
            }
        };
        hist.insert(u.clone(), h);
    }
    for (p, w) in &shaded {
        for (&c, &n) in w {
            *by_pal.entry(*p).or_default().entry(c).or_default() += n;
        }
    }
    // The upper parts' colours count too (their own sub-palettes).
    let uppers: Vec<[u16; 2]> = TALL.iter().map(|&d| [ds.upper[4 * d + 2], ds.upper[4 * d + 3]]).collect();
    for &v in uppers.iter().flatten() {
        if v != EMPTY {
            let h = ds_hist((v & 0x3FF) as usize, (v >> 12) as u8, false);
            let w = by_pal.entry((v >> 12) as u8).or_default();
            for (&c, &n) in &h {
                *w.entry(c).or_default() += n;
            }
        }
    }
    // AW2's pictures' colours count for the sub-palette that has them.
    for (u, h) in &hist {
        if let Unit::Px(_) = u {
            for (&c, &n) in h {
                let p = by_pal.iter().find(|(_, w)| w.contains_key(&c)).map(|(&p, _)| p).or_else(|| {
                    (0..9u8).find(|&p| (1..16).any(|i| dp.get(16 * p as usize + i) == Some(&c)))
                });
                if let Some(p) = p {
                    *by_pal.entry(p).or_default().entry(c).or_default() += n;
                }
            }
        }
    }
    let pals: Vec<u8> = by_pal.keys().copied().collect();
    let mut best: Option<((u64, Vec<Vec<u8>>), Vec<Vec<u8>>, Vec<Vec<u16>>)> = None;
    for part in partitions(&pals, 4) {
        let blocks: Vec<Vec<u16>> = part
            .iter()
            .map(|b| {
                let mut w: BTreeMap<u16, u64> = BTreeMap::new();
                for p in b {
                    for (&c, &n) in &by_pal[p] {
                        *w.entry(c).or_default() += n;
                    }
                }
                reduce15(&w)
            })
            .collect();
        let mut err = 0;
        for (u, h) in &hist {
            err += (0..blocks.len())
                .filter(|&i| match u {
                    Unit::Ds(_, p) => part[i].contains(p),
                    Unit::Px(_) => true,
                })
                .map(|i| hist_error(h, &blocks[i]))
                .min()
                .unwrap_or(0);
        }
        let mut sorted: Vec<Vec<u8>> = part
            .iter()
            .map(|b| {
                let mut b = b.clone();
                b.sort();
                b
            })
            .collect();
        sorted.sort();
        let key = (err, sorted);
        if best.as_ref().map_or(true, |(k, _, _)| key < *k) {
            best = Some((key, part, blocks));
        }
    }
    let (_, part, blocks) = best?;
    let block_of = |p: u8| part.iter().position(|b| b.contains(&p));

    // Fog: each colour of the 4 palettes takes the fogged colour of the
    // Dual Strike colours folded into it (weighted by their pixels): a
    // terrain sub-palette's index `i` fogs to sub-palette 5's `i`.
    let mut votes: BTreeMap<usize, BTreeMap<u16, u64>> = BTreeMap::new();
    for (&p, w) in &by_pal {
        let Some(b) = block_of(p) else { continue };
        for (&c, &n) in w {
            let Some(i) = (1..16).find(|&i| dp.get(16 * p as usize + i) == Some(&c)) else { continue };
            let f = if (p as usize) < FOG_PALETTE { dp.get(16 * FOG_PALETTE + i).copied().unwrap_or(c) } else { c };
            *votes.entry(16 * b + 1 + nearest(&blocks[b], c)).or_default().entry(f).or_default() += n;
        }
    }
    let mut fog = vec![NO_FOG; 64];
    for (k, v) in &votes {
        if let Some((&f, _)) = v.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) {
            fog[*k] = f;
        }
    }

    // Tiles: Dual Strike's at their own index (so the animated ones stay
    // where the frames go), pictures in the static slots left.
    let mut tiles = vec![0u8; TILE * TILES];
    let mut used = vec![false; TILES];
    let mut slot: BTreeMap<Unit, (usize, usize)> = BTreeMap::new();
    let remap = |p: u8| -> Option<(usize, [u8; 16])> {
        let b = block_of(p)?;
        let mut r = [0u8; 16];
        for i in 1..16 {
            r[i] = nearest(&blocks[b], colour((p as u16) << 12, i as u8)) as u8 + 1;
        }
        Some((b, r))
    };
    let remapped = |src: &[u8], k: usize, r: &[u8; 16]| {
        let mut t = [0u8; TILE];
        for y in 0..8 {
            for x in 0..8 {
                set_px(&mut t, x, y, r[tpx(src, k, x, y) as usize]);
            }
        }
        t
    };
    let mut pal_of_slot: BTreeMap<usize, u8> = BTreeMap::new();
    for u in &units {
        if let Unit::Ds(t, p) = u {
            let t = *t as usize;
            if t >= TILES || used[t] {
                continue;
            }
            let (b, r) = remap(*p)?;
            tiles[TILE * t..TILE * t + TILE].copy_from_slice(&remapped(dt, t, &r));
            used[t] = true;
            pal_of_slot.insert(t, *p);
            slot.insert(u.clone(), (t, b));
        }
    }
    let mut free: Vec<usize> = (0..SEA0).chain(RIVER0 + RIVER_N..TILES).filter(|&s| !used[s] && s != EMPTY as usize).collect();
    free.reverse();
    let mut placed: BTreeMap<(Vec<u8>, usize), usize> = BTreeMap::new();
    let mut place = |tiles: &mut Vec<u8>, px: &[u16], b: usize| -> Option<usize> {
        let mut t = [0u8; TILE];
        for (k, &c) in px.iter().enumerate() {
            let i = if c == TRANSPARENT { 0 } else { nearest(&blocks[b], c) as u8 + 1 };
            set_px(&mut t, k % 8, k / 8, i);
        }
        let key = (t.to_vec(), b);
        if let Some(&s) = placed.get(&key) {
            return Some(s);
        }
        let s = free.pop()?;
        tiles[TILE * s..TILE * s + TILE].copy_from_slice(&t);
        placed.insert(key, s);
        Some(s)
    };
    let best_block = |h: &BTreeMap<u16, u64>| (0..blocks.len()).min_by_key(|&i| (hist_error(h, &blocks[i]), i));
    for (u, h) in &hist {
        let (px, b): (Vec<u16>, usize) = match u {
            Unit::Ds(t, p) if !slot.contains_key(u) => {
                // The same tile with another sub-palette.
                let (b, _) = remap(*p)?;
                let px = (0..64)
                    .map(|k| {
                        let i = tpx(dt, *t as usize, k % 8, k / 8);
                        if i == 0 { TRANSPARENT } else { colour((*p as u16) << 12, i) }
                    })
                    .collect();
                (px, b)
            }
            Unit::Ds(..) => continue,
            Unit::Px(px) => (px.clone(), best_block(h)?),
        };
        let s = place(&mut tiles, &px, b)?;
        slot.insert(u.clone(), (s, b));
    }

    // Frames, remapped as their tiles.
    let mut sea_out = vec![0u8; SEA_FRAMES * SEA_FRAME];
    let mut river_out = vec![0u8; RIVER_FRAMES * RIVER_FRAME];
    for (src, dst, start, n, size) in [
        (&sea, &mut sea_out, SEA0, SEA_N, SEA_FRAME),
        (&river, &mut river_out, RIVER0, RIVER_N, RIVER_FRAME),
    ] {
        for f in 0..dst.len() / size {
            for k in 0..n {
                let t = start + k;
                let o = f * size + TILE * k;
                match pal_of_slot.get(&t) {
                    Some(&p) => {
                        let (_, r) = remap(p)?;
                        dst[o..o + TILE].copy_from_slice(&remapped(&src[f * size..], k, &r));
                    }
                    None => dst[o..o + TILE].copy_from_slice(&tiles[TILE * t..TILE * t + TILE]),
                }
            }
        }
    }

    // The metatile table, and the three mountains.
    let mut metatiles = aw2.metatiles.clone();
    let mut mountains = [[0u16; 4]; MOUNTAINS];
    for (m, q) in out.iter().enumerate() {
        let Some(q) = q else { continue };
        for (k, quad) in q.iter().enumerate() {
            let (s, b) = slot[&quad.unit];
            let e = s as u16 | quad.flips | (b as u16) << 12;
            if m < METATILES {
                metatiles[4 * m + k] = e;
            } else {
                mountains[m - METATILES][k] = e;
            }
        }
    }
    let mut colours = aw2.clear[..64].to_vec();
    for (i, b) in blocks.iter().enumerate() {
        for (j, &c) in b.iter().enumerate() {
            colours[16 * i + 1 + j] = c;
        }
    }

    // The upper parts' bottom quadrants as colours, and the free tiles
    // left for the composites drawn with them.
    let overlays: Vec<[Vec<u16>; 2]> = uppers
        .iter()
        .map(|up| {
            [0, 1].map(|side| {
                let v = up[side];
                (0..64)
                    .map(|k| {
                        let i = if v == EMPTY { 0 } else { quad_px(dt, v, k % 8, k / 8) };
                        if i == 0 { TRANSPARENT } else { colour(v, i) }
                    })
                    .collect()
            })
        })
        .collect();
    let mut pool = free;
    pool.sort();

    let mut mountain_pick = [0u8; 16];
    for (i, p) in mountain_pick.iter_mut().enumerate() {
        let id = ds.mountain_pick.get(i).copied().unwrap_or(TALL[0] as u16) as usize;
        *p = TALL[..MOUNTAINS].iter().position(|&d| d == id).unwrap_or(0) as u8;
    }
    Some(Look { tiles, sea: sea_out, river: river_out, metatiles, colours, fog, kinds, tall, mountains, mountain_pick, overlays, pool })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitions_and_reduction() {
        // Bell numbers: 5 items into at most 4 blocks = 52 - 1.
        assert_eq!(partitions(&[0, 1, 2, 3, 4], 4).len(), 51);
        assert_eq!(partitions(&[0, 1, 2], 4).len(), 5);
        // 19 shades of grey and one green: the green, however rare, stays.
        let mut w: BTreeMap<u16, u64> = (0..19u16).map(|i| (i * 0x421, 100 + i as u64)).collect();
        w.insert(31 << 5, 1);
        let r = reduce15(&w);
        assert_eq!(r.len(), 15);
        assert!(r.contains(&(31 << 5)));
    }
}
