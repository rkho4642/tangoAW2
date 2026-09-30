//! Dual Strike's battle backgrounds, with the Dual Strike pack: for a
//! Piperunner (Dual Strike's, with the pipe it fires from) and for every
//! battle on a Wasteland map; and Dual Strike's choice of background for
//! the squares AW2 never had units on (pipes, Com Towers).
//!
//! **Dual Strike's backgrounds** (battle overlay 2, `0x023521E4` and
//! `0x023518AC`; the files in `battle/`) are 3D scenes, per half of the
//! screen: a sky (a text BG, `1e8`.. with its own palettes), a horizon
//! strip (a 128x64 4bpp texture on a flat quad, rows 38..83) and the ground
//! (a 128x128 4bpp texture on a plane in perspective, rows 84..191). A
//! terrain's files are a group of twelve from `11a` (ground, ground in
//! desert and wasteland maps, horizon, horizon in desert and wasteland maps,
//! then four ground and four horizon palettes: normal, snow, desert,
//! wasteland), picked by the unit's terrain class ([`group`]); the sky by
//! class too ([`sky`]). A Piperunner stands on a pipe drawn across the
//! ground (`1e6`, palette `1e7`), whatever its square; on a pipe or a pipe
//! seam Dual Strike's background is the plain's.
//!
//! **Here.** The scene is rendered at run time from those files with Dual
//! Strike's own mapping of screen to texture (measured: numbers only, in
//! [`render`]), cropped to AW2's half (Dual Strike's rows 16..191, its
//! columns shifted by 4 so the halves meet at the scene's own middle),
//! quantised to AW2's three 15-colour BG palettes a side (every colour one
//! of Dual Strike's own) and cut into 8x8 tiles. `sub_0804B850`, which
//! loads a side's background (tiles, map, palettes by weather), is replaced
//! for that side. Dual Strike's backgrounds follow the map's tileset, not
//! the weather; AW2's follow the weather, and so these do, to match the
//! other side: in snow Dual Strike's snow tileset, in rain AW2's rain
//! colours ([`RAIN`]). Air units keep AW2's sky. On other maps a pipe or pipe seam gets AW2's
//! plain background and a Com Tower AW2's city, as Dual Strike picks
//! ([`classes`]).
//!
//! Everything is keyed off emulated memory (the sides' units, the map, the
//! biome) and nothing is kept between frames. With the pack off nothing
//! here writes anything.

use mgba::core::Core;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::ds_weather::is_on;

// --- Dual Strike's files -----------------------------------------------------------

fn file(name: u16) -> Option<Vec<u8>> {
    let f = crate::ds_pack::pack()?.file(&format!("battle/{name:03x}"))?;
    Some(crate::ds_art::lz10(f).unwrap_or_else(|| f.to_vec()))
}

fn palette(name: u16, n: usize) -> Option<Vec<u16>> {
    let f = file(name)?;
    (0..n).map(|i| Some(u16::from_le_bytes([*f.get(2 * i)?, *f.get(2 * i + 1)?]) & 0x7FFF)).collect()
}

/// A linear 4bpp texture (rows of `w` texels, low nibble first).
struct Texture {
    w: usize,
    texels: Vec<u8>,
}

impl Texture {
    fn load(name: u16, w: usize) -> Option<Texture> {
        let b = file(name)?;
        let texels = b.iter().flat_map(|&v| [v & 15, v >> 4]).collect();
        Some(Texture { w, texels })
    }
    fn h(&self) -> usize {
        self.texels.len() / self.w
    }
    fn at(&self, u: i64, v: i64) -> u8 {
        let (w, h) = (self.w as i64, self.h() as i64);
        self.texels[(v.rem_euclid(h) * w + u.rem_euclid(w)) as usize]
    }
}

/// The first file of the terrain groups (twelve files each).
const GROUPS: u16 = 0x11A;
const GROUP_FILES: u16 = 12;
const PLAIN: u8 = 0;
const SEA: u8 = 5;
/// The pipe a Piperunner stands on.
const PIPE_TEXTURE: u16 = 0x1E6;
const PIPE_PALETTE: u16 = 0x1E7;

/// Dual Strike's tilesets (a map's look): the textures and palettes used.
pub const NORMAL: u8 = 0;
pub const SNOW: u8 = 1;
pub const WASTELAND: u8 = 3;

/// AW2's battle backgrounds' rain colours from their clear ones, fitted
/// over all of them (x256, per 5-bit channel: rows the clear colour's r, g,
/// b and 1, columns the result's r, g, b).
const RAIN: [[i32; 3]; 4] = [[188, -6, 58], [-31, 198, 0], [40, -1, 157], [364, 387, 432]];

fn rain(c: u16) -> u16 {
    let v = [(c & 31) as i32, ((c >> 5) & 31) as i32, ((c >> 10) & 31) as i32];
    let mut out = 0u16;
    for j in 0..3 {
        let x = v[0] * RAIN[0][j] + v[1] * RAIN[1][j] + v[2] * RAIN[2][j] + RAIN[3][j];
        out |= (((x + 128).div_euclid(256)).clamp(0, 31) as u16) << (5 * j);
    }
    out
}

/// Dual Strike's background group for a terrain class (AW2's classes are
/// Dual Strike's terrain ids) and, for an HQ, its army colour (1..5).
fn group(class: u8, colour: u8) -> u8 {
    match class {
        2 => 4,              // river
        3 => 2,              // mountain
        4 => 1,              // wood
        5 => 3,              // road
        6 | 17 | 18 | 22 => 8, // city, silos, Com Tower
        7 | 19 => SEA,       // sea, reef
        8 => 11 + colour.clamp(1, 5), // HQs, one per army
        10 => 10,            // airport
        11 => 11,            // port
        12 => 7,             // bridge
        13 => 6,             // shoal
        14 | 20 => 9,        // base, lab
        _ => PLAIN,          // plain, pipe, pipe seam, ...
    }
}

/// The sky: tiles, map, and palette by tileset.
fn sky(class: u8, tileset: u8) -> (u16, u16, u16) {
    let t = tileset as usize & 3;
    let (a, b, pals): (u16, u16, [u16; 4]) = match class {
        3 => (0x1F8, 0x1FC, [0x1FA, 0x1FB, 0x1FE, 0x1FF]),
        4 => (0x1F0, 0x1F4, [0x1F2, 0x1F3, 0x1F6, 0x1F7]),
        6 | 8 | 10 | 11 | 14 | 17 | 18 | 20 | 22 => (0x200, 0x204, [0x202, 0x203, 0x206, 0x207]),
        _ => (0x1E8, 0x1E8, [0x1EA, 0x1EB, 0x1EA, 0x1EA]),
    };
    let tiles = if t >= 2 { b } else { a };
    (tiles, tiles + 1, pals[t])
}

// --- Rendering ------------------------------------------------------------------

/// What a side shows: the ground's group, the sky's class, the tileset, the
/// half (0 left) and whether a Piperunner's pipe crosses it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Scene {
    pub group: u8,
    pub sky_class: u8,
    pub tileset: u8,
    pub half: u8,
    pub pipe: bool,
    /// AW2's rain: its backgrounds' rain colours.
    pub rain: bool,
}

/// Where each pixel came from (palettes are seeded by it).
const FROM_SKY: u8 = 0;
const FROM_HORIZON: u8 = 1;
const FROM_GROUND: u8 = 2;
const FROM_PIPE: u8 = 3;
/// A pipe's colours weigh more in its palette (they would otherwise be
/// outnumbered by the ground's around it).
const PIPE_WEIGHT: u32 = 6;

/// Rows of Dual Strike's half: the horizon strip, then the ground.
const HORIZON_TOP: i64 = 38;
const GROUND_TOP: i64 = 84;

/// Dual Strike's half rendered (RGB555 and source per pixel) over `xs` x
/// `ys` (its own coordinates; outside 0..127 the scene's mapping goes on).
///
/// The mapping, measured from Dual Strike rendering coordinate-coded
/// textures (per half, `d = y - 57.33`):
/// - ground: `u = gu + 39.79 (x - gx) / d`, `v = 415.94 - 10279 / d`;
/// - horizon: `u = hu + 1.375 x`, `v = 1.3908 y - 53.318` (clamped 0..63);
/// - sky: the BG at the screen position (both halves share it);
/// - pipe (on the Piperunner's half, mirrored on the right): with
///   `p = 39.79 (x' - 159.84) / d` (`x'` = `x`, or `127 - x` on the right),
///   where `-61.33 <= p <= -29.40`: `u = (p + 61.33) 2.0045`,
///   `v = 96.016 - 10023.93 / (y - 57.30)`.
fn render(s: &Scene, xs: std::ops::Range<i64>, ys: std::ops::Range<i64>) -> Option<Vec<(u16, u8)>> {
    let base = GROUPS + GROUP_FILES * s.group as u16;
    let alt = (s.tileset >= 2) as u16;
    let ts = (s.tileset & 3) as u16;
    let ground = Texture::load(base + alt, 128)?;
    let horizon = Texture::load(base + 2 + alt, 128)?;
    let gpal = palette(base + 4 + ts, 16)?;
    let hpal = palette(base + 8 + ts, 16)?;
    let (st, sm, sp) = sky(s.sky_class, s.tileset);
    let sky_tiles = file(st)?;
    let sky_map = file(sm)?;
    let sky_pal = palette(sp, 64)?;
    let pipe = if s.pipe { Some((Texture::load(PIPE_TEXTURE, 64)?, palette(PIPE_PALETTE, 16)?)) } else { None };
    let right = s.half == 1;
    let (gx, gu, hu) = if right { (-31.95, 15.70, 27.68) } else { (159.84, 335.25, 115.58) };
    let sky_at = |x: i64, y: i64| -> u16 {
        let (x, y) = ((x + 128 * s.half as i64).rem_euclid(256), y.rem_euclid(256));
        let e = u16::from_le_bytes([sky_map[(2 * ((y / 8) * 32 + x / 8)) as usize], sky_map[(2 * ((y / 8) * 32 + x / 8) + 1) as usize]]);
        let (mut px, mut py) = (x % 8, y % 8);
        if e & 0x400 != 0 {
            px = 7 - px;
        }
        if e & 0x800 != 0 {
            py = 7 - py;
        }
        let o = 32 * (e & 0x3FF) as usize + (py * 4 + px / 2) as usize;
        let b = sky_tiles.get(o).copied().unwrap_or(0);
        let c = if px & 1 == 1 { b >> 4 } else { b & 15 } as usize;
        sky_pal[if c == 0 { 0 } else { 16 * (e >> 12) as usize + c }]
    };
    let mut out = Vec::with_capacity(((xs.end - xs.start) * (ys.end - ys.start)) as usize);
    for y in ys.clone() {
        for x in xs.clone() {
            let mut px: Option<(u16, u8)> = None;
            if (HORIZON_TOP..GROUND_TOP).contains(&y) {
                let v = ((1.390_81 * y as f64 - 53.318).floor() as i64).clamp(0, 63);
                let u = (hu + 1.375 * x as f64).floor() as i64;
                let c = horizon.at(u, v);
                if c != 0 {
                    px = Some((hpal[c as usize], FROM_HORIZON));
                }
            } else if y >= GROUND_TOP {
                let d = y as f64 - 57.33;
                let u = (gu + 39.79 * (x as f64 - gx) / d).floor() as i64;
                let v = (415.94 - 10279.0 / d).floor() as i64;
                let c = ground.at(u, v);
                if c != 0 {
                    px = Some((gpal[c as usize], FROM_GROUND));
                }
                if let Some((tex, pal)) = &pipe {
                    let xm = if right { 127 - x } else { x } as f64;
                    let p = 39.79 * (xm - 159.84) / d;
                    if (-61.33..=-29.40).contains(&p) {
                        let u = (((p + 61.33) * 2.0045).floor() as i64).clamp(0, 63);
                        let v = (96.016 - 10023.93 / (y as f64 - 57.30)).floor() as i64;
                        let c = tex.at(u, v);
                        if c != 0 {
                            px = Some((pal[c as usize], FROM_PIPE));
                        }
                    }
                }
            }
            out.push(px.unwrap_or_else(|| (sky_at(x, y), FROM_SKY)));
        }
    }
    Some(out)
}

// --- AW2's format -----------------------------------------------------------------

/// A side's background converted: 4bpp tiles, the map (a 17x22 block of
/// entries, tile | palette << 12 with palettes 0..2 of the side's three) and
/// the three palettes (colour 0 unused: AW2's backdrop).
pub struct Backdrop {
    pub tiles: Vec<[u8; 32]>,
    pub map: Vec<u16>,
    pub palettes: [[u16; 16]; 3],
}

/// The block of the side's BG map it fills: columns (from the first,
/// wrapping at 32) and rows.
const COLS: i64 = 17;
const ROWS: i64 = 22;
/// AW2 BG pixel (0, 0) of the block, in Dual Strike's half: side 0's
/// block starts one tile left of the screen (its shakes), side 1's at BG
/// column 14.
const FIRST_COL: [i64; 2] = [-1, 14];
const DS_X: [i64; 2] = [4, -117];
const DS_Y: i64 = 16;

fn dist(a: u16, b: u16) -> u32 {
    let ch = |c: u16, s: u16| ((c >> s) & 31) as i32;
    let (dr, dg, db) = (ch(a, 0) - ch(b, 0), ch(a, 5) - ch(b, 5), ch(a, 10) - ch(b, 10));
    (3 * dr * dr + 4 * dg * dg + 2 * db * db) as u32
}

/// Up to 15 of the colours (with counts), by median cut; each box's colour
/// is its most frequent one (so every colour is one of Dual Strike's).
fn median_cut(colours: &[(u16, u32)]) -> Vec<u16> {
    let mut boxes: Vec<Vec<(u16, u32)>> = vec![colours.to_vec()];
    while boxes.len() < 15 {
        // Split the box with the widest channel range (weighted by count).
        let mut best: Option<(usize, usize, i32)> = None;
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            for ch in 0..3 {
                let vals = b.iter().map(|&(c, _)| ((c >> (5 * ch)) & 31) as i32);
                let range = vals.clone().max().unwrap() - vals.min().unwrap();
                let weight: u32 = b.iter().map(|&(_, n)| n).sum();
                let score = range * (1 + (weight as f64).sqrt() as i32);
                if best.map_or(true, |(_, _, s)| score > s) {
                    best = Some((i, ch, score));
                }
            }
        }
        let Some((i, ch, _)) = best else { break };
        let mut b = boxes.swap_remove(i);
        b.sort_by_key(|&(c, _)| (((c >> (5 * ch)) & 31), c));
        let total: u32 = b.iter().map(|&(_, n)| n).sum();
        let mut acc = 0;
        let mut cut = 1;
        for (k, &(_, n)) in b.iter().enumerate() {
            acc += n;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
        boxes.sort_by_key(|b| b[0].0);
    }
    let mut out: Vec<u16> = boxes.iter().map(|b| b.iter().max_by_key(|&&(c, n)| (n, c)).unwrap().0).collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn nearest(pal: &[u16], c: u16) -> (usize, u32) {
    pal.iter().enumerate().map(|(i, &p)| (i, dist(p, c))).min_by_key(|&(i, d)| (d, i)).unwrap()
}

fn convert(s: &Scene, side: usize) -> Option<Backdrop> {
    let x0 = 8 * FIRST_COL[side] + DS_X[side];
    let (w, h) = (8 * COLS, 8 * ROWS);
    let px = render(s, x0..x0 + w, DS_Y..DS_Y + h)?;
    let tile_px = |t: usize| -> Vec<(u16, u8)> {
        let (tx, ty) = ((t as i64 % COLS) * 8, (t as i64 / COLS) * 8);
        (0..64).map(|i| px[((ty + i / 8) * w + tx + i % 8) as usize]).collect()
    };
    let n = (COLS * ROWS) as usize;
    // Seed: each tile's palette is its most common source.
    let mut which: Vec<usize> = (0..n)
        .map(|t| {
            let mut count = [0u32; 3];
            for (_, from) in tile_px(t) {
                count[from.min(FROM_GROUND) as usize] += 1;
            }
            (0..3).max_by_key(|&k| (count[k], 3 - k)).unwrap()
        })
        .collect();
    let mut palettes: Vec<Vec<u16>> = vec![Vec::new(); 3];
    for _ in 0..3 {
        for (p, pal) in palettes.iter_mut().enumerate() {
            let mut counts: HashMap<u16, u32> = HashMap::new();
            for t in (0..n).filter(|&t| which[t] == p) {
                for (c, from) in tile_px(t) {
                    *counts.entry(c).or_insert(0) += if from == FROM_PIPE { PIPE_WEIGHT } else { 1 };
                }
            }
            let mut v: Vec<(u16, u32)> = counts.into_iter().collect();
            v.sort_unstable();
            *pal = if v.is_empty() { vec![0] } else { median_cut(&v) };
        }
        for (t, w) in which.iter_mut().enumerate() {
            let cost = |p: usize| -> u32 { tile_px(t).iter().map(|&(c, _)| nearest(&palettes[p], c).1).sum() };
            *w = (0..3).min_by_key(|&p| (cost(p), p)).unwrap();
        }
    }
    let mut tiles: Vec<[u8; 32]> = Vec::new();
    let mut seen: HashMap<[u8; 32], usize> = HashMap::new();
    let mut map = Vec::with_capacity(n);
    for t in 0..n {
        let pal = &palettes[which[t]];
        let mut b = [0u8; 32];
        for (i, (c, _)) in tile_px(t).into_iter().enumerate() {
            let k = nearest(pal, c).0 as u8 + 1;
            b[i / 2] |= if i & 1 == 1 { k << 4 } else { k };
        }
        let id = *seen.entry(b).or_insert_with(|| {
            tiles.push(b);
            tiles.len() - 1
        });
        map.push(id as u16 | (which[t] as u16) << 12);
    }
    let mut out = [[0u16; 16]; 3];
    for (o, p) in out.iter_mut().zip(&palettes) {
        for (i, &c) in p.iter().enumerate().take(15) {
            o[i + 1] = if s.rain { rain(c) } else { c };
        }
    }
    Some(Backdrop { tiles, map, palettes: out })
}

static CACHE: OnceLock<Mutex<HashMap<(Scene, usize), Option<&'static Backdrop>>>> = OnceLock::new();

/// A side's converted background (converted once per scene and side).
pub fn backdrop(s: &Scene, side: usize) -> Option<&'static Backdrop> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut c = cache.lock().ok()?;
    *c.entry((*s, side)).or_insert_with(|| convert(s, side).map(|b| &*Box::leak(Box::new(b))))
}

// --- AW2's scene -------------------------------------------------------------------

/// The two sides' units (pointers to unit records: +0 type, +2 x, +3 y).
const SIDE_UNITS: u32 = 0x0300_4528;
const MAP: u32 = 0x0201_E450;
const CLASSES: u32 = MAP + 0x1432;
const MAP_ROWS: u32 = MAP + 0x417A;
const PIPE: u8 = 15;
const PIPE_SEAM: u8 = 16;
const CITY: u8 = 6;
const LAB: u8 = 0x14;
const COM_TOWER: u8 = 22;
const HQ: u8 = 8;
/// AW2's backgrounds: the sky (air units), HQs by colour, the plain.
const BG_AIR: u32 = 0x33;
const BG_HQ: u32 = 0x2D;
const BG_PLAIN: u32 = 1;

fn side_unit(core: &Core, side: u32) -> Option<u32> {
    let u = core.raw_read_32(SIDE_UNITS + 4 * side, -1);
    (0x0200_0000..0x0204_0000).contains(&u).then_some(u)
}

/// The terrain class of the side's unit's square.
fn side_class(core: &Core, side: u32) -> Option<u8> {
    let u = side_unit(core, side)?;
    let (x, y) = (core.raw_read_8(u + 2, -1) as u32, core.raw_read_8(u + 3, -1) as u32);
    if x >= 30 || y >= 30 {
        return None;
    }
    let row = core.raw_read_16(MAP_ROWS + 2 * y, -1) as u32;
    Some(core.raw_read_8(CLASSES + row + x, -1) & 0x1F)
}

/// `sub_0804B744(class 0, class 1)`: picks both sides' backgrounds from
/// their terrain classes (and loads them, [`LOAD`]).
pub const CLASSES_IN: u32 = 0x0804_B744;

/// Trap at [`CLASSES_IN`]: a pipe or pipe seam is a plain to the
/// background, and in Versus a Lab (a Com Tower) a city, as in Dual Strike.
fn classes(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let tower = crate::com_tower::active(core);
    for r in 0..2 {
        let c = core.gba().cpu().gpr(r) as u32 & 0xFFFF;
        let to = match c as u8 {
            PIPE | PIPE_SEAM => BG_PLAIN,
            LAB if tower => CITY as u32,
            _ => continue,
        };
        core.gba_mut().cpu_mut().set_gpr(r, to as i32);
    }
}

/// In `sub_0804B744`, just before its backgrounds (`r6`, `r5`) set up the
/// scene's BG layers (a sea's or a sky's scroll with its waves or clouds).
pub const LAYERS: u32 = 0x0804_B7B8;
/// AW2's background table (0x18 each: `+2` its layer type, 0 still).
const BG_TABLE: u32 = 0x0855_5850;

/// Trap at [`LAYERS`]: a side that will show Dual Strike's (still)
/// background gets still layers.
fn layers(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    for (side, r) in [(0u32, 6usize), (1, 5)] {
        let bg = core.gba().cpu().gpr(r) as u32 & 0xFFFF;
        if scene_for(core, side, bg).is_some() && core.raw_read_8(BG_TABLE + 0x18 * bg + 2, -1) != 0 {
            core.gba_mut().cpu_mut().set_gpr(r, BG_PLAIN as i32);
        }
    }
}

/// `sub_0804B850(side, background, tiles, map, [palettes])` loads a side's
/// background: its tiles, its map (32x32 entries) and three palettes (by
/// weather).
pub const LOAD: u32 = 0x0804_B850;

/// The unit table's class byte: 2 plane, 3 copter, 4 ship.
fn unit_class(core: &Core, t: u8) -> u8 {
    core.raw_read_8(crate::roster::table(core) + 0x5C * t as u32 + 0x18, -1)
}

/// What the side shows, if Dual Strike's.
fn scene_for(core: &Core, side: u32, bg: u32) -> Option<Scene> {
    let t = side_unit(core, side).map(|u| core.raw_read_8(u, -1))?;
    let kind = unit_class(core, t);
    if bg == BG_AIR || matches!(kind, 2 | 3) {
        return None;
    }
    let piperunner = t == crate::roster::PIPERUNNER;
    let wasteland = crate::wasteland::is_wasteland(core);
    if !piperunner && !wasteland {
        return None;
    }
    let mut class = side_class(core, side)?;
    if class == LAB && crate::com_tower::active(core) {
        class = COM_TOWER;
    }
    let colour = if class == HQ { bg.saturating_sub(BG_HQ) as u8 } else { 0 };
    let g = if kind == 4 { SEA } else { group(class, colour) };
    // AW2 dresses its backgrounds for the weather; so does this: snow is
    // Dual Strike's snow tileset, rain AW2's rain colours.
    let weather = core.raw_read_16(BATTLE_WEATHER, -1);
    Some(Scene {
        group: g,
        sky_class: if g == SEA { 7 } else { class },
        tileset: if weather == 1 {
            SNOW
        } else if wasteland {
            WASTELAND
        } else {
            NORMAL
        },
        half: side as u8,
        pipe: piperunner,
        rain: weather == 2,
    })
}

/// The battle's weather (0 clear, 1 snow, 2 rain; a sandstorm is clear).
const BATTLE_WEATHER: u32 = 0x0300_4520;
const BG_PAL_BUFFER: u32 = 0x0300_20C0;
const BG_PAL_RAM: u32 = 0x0500_0000;
/// The sides' first BG palette (their three).
const FIRST_PAL: [u32; 2] = [1, 4];

/// Trap at [`LOAD`]: a side showing Dual Strike's background gets it, and
/// AW2's loader is skipped.
fn load(core: &mut Core) {
    if !is_on(core) {
        return;
    }
    let cpu = core.gba().cpu();
    let (side, bg, tiles_at, map_at) = (cpu.gpr(0) as u32 & 0xFFFF, cpu.gpr(1) as u32 & 0xFFFF, cpu.gpr(2) as u32, cpu.gpr(3) as u32);
    let lr = cpu.gpr(14) as u32;
    if side > 1 {
        return;
    }
    let Some(scene) = scene_for(core, side, bg) else { return };
    let Some(b) = backdrop(&scene, side as usize) else { return };
    let data: Vec<u8> = b.tiles.iter().flatten().copied().collect();
    core.raw_write_range(tiles_at, -1, &data);
    let pal0 = FIRST_PAL[side as usize] as u16;
    let fill = b.map[0] + (pal0 << 12);
    let mut map = [fill; 32 * 32];
    for (k, &e) in b.map.iter().enumerate() {
        let (c, r) = ((FIRST_COL[side as usize] + k as i64 % COLS).rem_euclid(32), k as i64 / COLS);
        map[(r * 32 + c) as usize] = e + (pal0 << 12);
    }
    let bytes: Vec<u8> = map.iter().flat_map(|e| e.to_le_bytes()).collect();
    core.raw_write_range(map_at, -1, &bytes);
    for (k, p) in b.palettes.iter().enumerate() {
        let pal = FIRST_PAL[side as usize] + k as u32;
        let colours: Vec<u8> = p[1..].iter().flat_map(|c| c.to_le_bytes()).collect();
        for base in [BG_PAL_BUFFER, BG_PAL_RAM] {
            core.raw_write_range(base + 32 * pal + 2, -1, &colours);
        }
    }
    core.gba_mut().cpu_mut().set_thumb_pc(lr & !1);
}

pub fn traps() -> Vec<(u32, Box<dyn Fn(&mut Core)>)> {
    vec![(CLASSES_IN, Box::new(classes)), (LAYERS, Box::new(layers)), (LOAD, Box::new(load))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups() {
        assert_eq!(group(1, 0), PLAIN);
        assert_eq!(group(PIPE, 0), PLAIN);
        assert_eq!(group(COM_TOWER, 0), 8);
        assert_eq!(group(HQ, 5), 16);
        assert_eq!(sky(4, WASTELAND), (0x1F4, 0x1F5, 0x1F7));
    }

    #[test]
    fn cut_keeps_real_colours() {
        let colours: Vec<(u16, u32)> = (0..40u16).map(|c| (c * 700, 1 + c as u32)).collect();
        let pal = median_cut(&colours);
        assert!(pal.len() <= 15);
        assert!(pal.iter().all(|p| colours.iter().any(|&(c, _)| c == *p)));
    }
}
